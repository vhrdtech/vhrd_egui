//! The world: bodies, forces, contacts and the fixed step that moves them.

use crate::geom::{Axis, Hit, Poly, Rect, Side, Vec2, sat};
use crate::params::Params;

/// Overlaps below this (points) count as touching.
const SLOP: f32 = 0.01;
/// Squeeze passes per step when pushing alone leaves bodies without room.
const SQUEEZE_PASSES: u32 = 4;
/// Most fixed steps one [`World::advance`] runs, so a slow frame never
/// turns into a long catch-up.
const MAX_STEPS_PER_ADVANCE: u32 = 8;
/// The least share of a squeeze that counts as pressure (`Body::jam`), so
/// rigid worlds (`tension` 0) still build up pressure and bulge.
const JAM_SHARE: f32 = 0.25;
/// Two bodies (or a body and a wall) this much farther apart than the gap
/// still touch for the grid pull (SETL-10), points.
const GRID_REACH: f32 = 1.0;
/// Relative speed along a contact at which the grid pull has faded out,
/// points/s: it grows as two bodies slow down against each other.
const GRID_CALM_SPEED: f32 = 120.0;
/// How fast the home of a fully bulged body drifts to where it is, 1/s
/// (on top of [`Params::home_drift`]): pushed out of a full row, it makes
/// its new place home instead of pressing back in.
const PRESSED_DRIFT: f32 = 2.0;
/// The sideways push along a slanted contact per point of bulge (both
/// bodies') and unit of slope, 1/s²: at 10 points of bulge each on a
/// 150 high edge it is about twice gravity's default pull.
const SLIDE_STIFFNESS: f32 = 400.0;
/// The most the slopes push a body sideways, points/s² (four times
/// gravity's default): with the default damping it slides off at about
/// 250 points/s, a glide, not a jump.
const SLIDE_MAX: f32 = 2400.0;
/// A body bulged more than this (points) can be the one that gives way.
const YIELD_BULGE: f32 = 0.5;
/// The share of [`Params::gap`] two bulged bodies' rectangles are kept
/// apart at least: off the tips, slanted edges let them come closer than
/// the gap, never into each other.
const RECT_FLOOR: f32 = 0.25;

/// Names a body for as long as it lives; never reused within a world.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BodyId(pub u64);

/// What a new body starts with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodyDesc {
    /// Where its middle starts.
    pub center: Vec2,
    /// The size it wants (its content's natural size).
    pub want: Vec2,
    /// The smallest it can be squeezed to.
    pub min_size: Vec2,
    /// The largest it may grow to.
    pub max_size: Vec2,
    /// Its home spot (where its middle is pulled to), if it has one.
    pub home: Option<Vec2>,
}

impl BodyDesc {
    /// A rigid body of `size` at `center`, at home where it starts.
    pub fn new(center: Vec2, size: Vec2) -> Self {
        Self {
            center,
            want: size,
            min_size: size,
            max_size: Vec2::INFINITY,
            home: Some(center),
        }
    }

    /// Let it be squeezed down to `min_size`.
    pub fn min_size(mut self, min_size: Vec2) -> Self {
        self.min_size = min_size;
        self
    }

    /// Cap its size at `max_size`.
    pub fn max_size(mut self, max_size: Vec2) -> Self {
        self.max_size = max_size;
        self
    }

    /// Set or clear its home spot.
    pub fn home(mut self, home: Option<Vec2>) -> Self {
        self.home = home;
        self
    }
}

/// One rectangle in the world. Read it through [`World::bodies`] or
/// [`World::body`]; change it through the world's methods.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Body {
    /// Its name.
    pub id: BodyId,
    /// Where it is now.
    pub rect: Rect,
    /// The size it wants.
    pub want: Vec2,
    /// The smallest it can be squeezed to.
    pub min_size: Vec2,
    /// The largest it may grow to.
    pub max_size: Vec2,
    /// The size it has when nothing squeezes it: follows `want` (within
    /// the limits) at [`Params::grow_speed`].
    pub natural: Vec2,
    /// How much is squeezed off `natural` on each axis right now.
    pub squeeze: Vec2,
    /// Its home spot, if any.
    pub home: Option<Vec2>,
    /// Its velocity, points/s.
    pub vel: Vec2,
    /// Lifted above the others: just added or being dragged, or dropped
    /// where there is no room yet. Others glide out from under it, and it
    /// turns solid once it touches nothing.
    pub lifted: bool,
    /// Held by [`World::grab`].
    pub dragged: bool,
    /// Gravity's pull in the last step, points/s² (debug overlay).
    pub gravity_pull: Vec2,
    /// The home spring's pull in the last step, points/s² (debug overlay).
    pub home_pull: Vec2,
    /// How much overlap contacts took out of this body in the last step,
    /// points: how hard it is pressed.
    pub pressure: f32,
    /// The pull of its corners toward the corners they snap to in the last
    /// step, damping included, points/s² (debug overlay).
    pub snap_pull: Vec2,
    /// The pull onto grid steps along its contacts in the last step,
    /// damping included, points/s² (SETL-10, debug overlay).
    pub grid_pull: Vec2,
    /// The sideways push of slanted contacts in the last step: where a
    /// bulged edge presses on a slope, part of the pressure pushes along
    /// it, points/s² (SETL-9, debug overlay).
    pub slide_pull: Vec2,
    /// How far the middle of each edge stands out in the solver's eyes,
    /// points: `x` on the left and right edges, `y` on the top and bottom
    /// ones. Grows with [`Body::load`] ([`Params::bulge_gain`]); see
    /// [`Body::shape`].
    pub bulge: Vec2,
    /// This body is the one that gives way: of the bulged bodies the most
    /// bulged (the newest on a tie), kept while it stays bulged. Where two
    /// bulged edges meet tip to tip it is pushed out of the row or column
    /// (away from the side gravity pulls to), and it lets go of its corner
    /// snaps and its home in step with its bulge; ties between other
    /// bodies are split straight along the axis, so only one body leaves
    /// a full row at a time.
    pub yielding: bool,
    /// Squeeze the body would have taken on each axis but could not give
    /// (rigid, or at its minimum size), points; let go at
    /// [`Params::restore`] like squeeze.
    pub jam: Vec2,
    drag_target: Vec2,
    /// How far slopes moved it sideways this step, points.
    slid: f32,
    prev: Rect,
}

impl Body {
    /// The share of its natural size squeezed off, 0..=1, on the axis
    /// squeezed most.
    pub fn tension(&self) -> f32 {
        let mut worst = 0.0f32;
        for axis in Axis::BOTH {
            if self.natural[axis] > 0.0 {
                worst = worst.max(self.squeeze[axis] / self.natural[axis]);
            }
        }
        worst.clamp(0.0, 1.0)
    }

    /// The pressure on each axis: squeeze plus [`Body::jam`], points. The
    /// bulge follows it.
    pub fn load(&self) -> Vec2 {
        self.squeeze + self.jam
    }

    /// The collision shape the solver sees: the pillow of its rectangle
    /// with [`Body::bulge`] (the rectangle itself when there is none). The
    /// UI draws the plain rectangle.
    pub fn shape(&self) -> Poly {
        Poly::pillow(&self.rect, self.bulge)
    }

    /// How much more it can be squeezed on `axis`.
    fn give(&self, axis: Axis) -> f32 {
        (self.rect.size()[axis] - self.min_size[axis]).max(0.0)
    }

    fn shift(&mut self, d: Vec2) {
        self.rect = self.rect.translate(d);
    }
}

/// A contact the solver worked on in the last step (debug overlay).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Contact {
    /// The body.
    pub a: BodyId,
    /// The other body, or `None` for a wall.
    pub b: Option<BodyId>,
    /// The axis they were pushed apart on mostly.
    pub axis: Axis,
    /// The direction `b` (or the body, away from the wall) was pushed in,
    /// a unit vector: off the axis when a bulged edge slid along another.
    pub normal: Vec2,
    /// How deep they were in each other, points.
    pub depth: f32,
}

/// A corner being pulled onto another corner in the last step, at most
/// one per body (debug overlay).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Snap {
    /// The body whose corner it is.
    pub body: BodyId,
    /// Where the corner is.
    pub from: Vec2,
    /// Where it is pulled to: lined up with the other corner, the gap
    /// kept where the two face each other.
    pub to: Vec2,
    /// How firmly it is held, 0 at the edge of [`Params::snap_range`] to
    /// 1 when in place.
    pub grip: f32,
}

/// Bodies in a rectangle, stepped with a fixed time step.
///
/// The same calls in the same order always give the same result, bit for
/// bit: there is no randomness, no clock and no hash-order iteration.
#[derive(Clone, Debug)]
pub struct World {
    params: Params,
    bounds: Rect,
    bodies: Vec<Body>,
    contacts: Vec<Contact>,
    snaps: Vec<Snap>,
    yielder: Option<BodyId>,
    next_id: u64,
    at_rest: bool,
    calm: u32,
    backlog: f32,
    steps: u64,
    last_move: f32,
}

impl World {
    /// An empty world inside `bounds`.
    pub fn new(bounds: Rect, params: Params) -> Self {
        Self {
            params,
            bounds,
            bodies: Vec::new(),
            contacts: Vec::new(),
            snaps: Vec::new(),
            yielder: None,
            next_id: 1,
            at_rest: false,
            calm: 0,
            backlog: 0.0,
            steps: 0,
            last_move: 0.0,
        }
    }

    /// The parameters in use.
    pub fn params(&self) -> &Params {
        &self.params
    }

    /// Replace the parameters; wakes the world if anything changed.
    pub fn set_params(&mut self, params: Params) {
        if self.params != params {
            self.params = params;
            self.wake();
        }
    }

    /// The rectangle the walls stand on.
    pub fn bounds(&self) -> Rect {
        self.bounds
    }

    /// Move the walls; bodies left outside glide back in.
    pub fn set_bounds(&mut self, bounds: Rect) {
        if self.bounds != bounds {
            self.bounds = bounds;
            self.wake();
        }
    }

    /// All bodies, in the order they were added.
    pub fn bodies(&self) -> &[Body] {
        &self.bodies
    }

    /// One body by id.
    pub fn body(&self, id: BodyId) -> Option<&Body> {
        self.bodies.iter().find(|b| b.id == id)
    }

    /// The corners that were pulled onto another corner in the last step.
    pub fn snaps(&self) -> &[Snap] {
        &self.snaps
    }

    /// The contacts of the last step.
    pub fn contacts(&self) -> &[Contact] {
        &self.contacts
    }

    /// Add a body. It starts lifted, so if it lands on others they glide
    /// out from under it instead of jumping.
    pub fn add(&mut self, desc: BodyDesc) -> BodyId {
        let id = BodyId(self.next_id);
        self.next_id += 1;
        let natural = desc.want.max(desc.min_size).min(desc.max_size);
        let rect = Rect::from_center_size(desc.center, natural);
        self.bodies.push(Body {
            id,
            rect,
            want: desc.want,
            min_size: desc.min_size,
            max_size: desc.max_size,
            natural,
            squeeze: Vec2::ZERO,
            home: desc.home,
            vel: Vec2::ZERO,
            lifted: true,
            dragged: false,
            gravity_pull: Vec2::ZERO,
            home_pull: Vec2::ZERO,
            pressure: 0.0,
            snap_pull: Vec2::ZERO,
            grid_pull: Vec2::ZERO,
            slide_pull: Vec2::ZERO,
            bulge: Vec2::ZERO,
            jam: Vec2::ZERO,
            yielding: false,
            drag_target: desc.center,
            slid: 0.0,
            prev: rect,
        });
        self.wake();
        id
    }

    /// Remove a body; `false` if there is none with this id.
    pub fn remove(&mut self, id: BodyId) -> bool {
        let before = self.bodies.len();
        self.bodies.retain(|b| b.id != id);
        let removed = self.bodies.len() != before;
        if removed {
            self.wake();
        }
        removed
    }

    /// Tell a body the size it wants now (its content grew or shrank).
    /// Its size follows at [`Params::grow_speed`].
    pub fn set_want(&mut self, id: BodyId, want: Vec2) {
        self.change(id, |b| {
            let changed = b.want != want;
            b.want = want;
            changed
        });
    }

    /// Change the smallest and largest size of a body.
    pub fn set_limits(&mut self, id: BodyId, min_size: Vec2, max_size: Vec2) {
        self.change(id, |b| {
            let changed = b.min_size != min_size || b.max_size != max_size;
            b.min_size = min_size;
            b.max_size = max_size;
            changed
        });
    }

    /// Set or clear a body's home spot.
    pub fn set_home(&mut self, id: BodyId, home: Option<Vec2>) {
        self.change(id, |b| {
            let changed = b.home != home;
            b.home = home;
            changed
        });
    }

    /// Make every body's current place its home.
    pub fn adopt_homes(&mut self) {
        for b in &mut self.bodies {
            b.home = Some(b.rect.center());
        }
        self.wake();
    }

    /// Take hold of a body: it follows [`World::drag_to`] and floats above
    /// the others, which glide out of its way.
    pub fn grab(&mut self, id: BodyId) {
        self.change(id, |b| {
            let changed = !b.dragged;
            b.dragged = true;
            b.lifted = true;
            b.drag_target = b.rect.center();
            b.vel = Vec2::ZERO;
            changed
        });
    }

    /// Move a grabbed body's middle to `center`.
    pub fn drag_to(&mut self, id: BodyId, center: Vec2) {
        self.change(id, |b| {
            let changed = b.dragged && b.drag_target != center;
            b.drag_target = center;
            changed
        });
    }

    /// Let go of a grabbed body. With `rehome` the place it was dropped
    /// becomes its home. It stays lifted until it touches nothing.
    pub fn release(&mut self, id: BodyId, rehome: bool) {
        self.change(id, |b| {
            let changed = b.dragged;
            b.dragged = false;
            if rehome && changed {
                b.home = Some(b.rect.center());
            }
            changed
        });
    }

    fn change(&mut self, id: BodyId, f: impl FnOnce(&mut Body) -> bool) {
        let changed = self.bodies.iter_mut().find(|b| b.id == id).is_some_and(f);
        if changed {
            self.wake();
        }
    }

    /// Whether the world has settled: nothing moves, and [`World::step`]
    /// does nothing until something changes.
    pub fn is_at_rest(&self) -> bool {
        self.at_rest
    }

    /// Start stepping again. Every call that changes something does this
    /// by itself.
    pub fn wake(&mut self) {
        self.at_rest = false;
        self.calm = 0;
    }

    /// Fixed steps taken so far.
    pub fn steps(&self) -> u64 {
        self.steps
    }

    /// The farthest any edge of a body that is not being dragged moved in
    /// the last step, points.
    pub fn last_move(&self) -> f32 {
        self.last_move
    }

    /// The deepest overlap between two solid (not lifted) bodies, points;
    /// 0 when none overlap.
    pub fn max_overlap(&self) -> f32 {
        self.worst_pair(0.0)
    }

    /// The most two solid bodies are closer than [`Params::gap`], points.
    pub fn max_gap_violation(&self) -> f32 {
        self.worst_pair(self.params.gap)
    }

    fn worst_pair(&self, gap: f32) -> f32 {
        let mut worst = 0.0f32;
        for (i, a) in self.bodies.iter().enumerate() {
            for b in &self.bodies[i + 1..] {
                if a.lifted || b.lifted {
                    continue;
                }
                let o = a.rect.overlap(&b.rect, gap);
                worst = worst.max(o.x.min(o.y));
            }
        }
        worst
    }

    /// Run the fixed steps that fit into `dt` seconds of real time (at
    /// most a few, a long frame is not caught up). Returns how many ran;
    /// 0 when at rest.
    pub fn advance(&mut self, dt: f32) -> u32 {
        if self.at_rest {
            self.backlog = 0.0;
            return 0;
        }
        let step = self.params.dt.max(1e-4);
        self.backlog = (self.backlog + dt.max(0.0)).min(step * MAX_STEPS_PER_ADVANCE as f32);
        let mut ran = 0;
        // Half a step of slack: frame times that are a multiple of the step
        // must not lose one to rounding.
        while self.backlog >= step * 0.5 && ran < MAX_STEPS_PER_ADVANCE && self.step() {
            self.backlog = (self.backlog - step).max(0.0);
            ran += 1;
        }
        ran
    }

    /// Step until at rest. Returns the steps it took, or `None` if it was
    /// still moving after `max_steps`.
    pub fn settle(&mut self, max_steps: u32) -> Option<u32> {
        for n in 0..=max_steps {
            if self.at_rest {
                return Some(n);
            }
            if n < max_steps {
                self.step();
            }
        }
        None
    }

    /// One fixed step of [`Params::dt`]. Returns `false`, having done
    /// nothing, when the world is at rest.
    pub fn step(&mut self) -> bool {
        if self.at_rest {
            return false;
        }
        let p = self.params;
        let dt = p.dt.max(1e-4);
        self.contacts.clear();
        let mut home_moved = 0.0f32;

        for b in &mut self.bodies {
            // Pushed off by several slopes at once, it still glides.
            b.slide_pull = b.slide_pull.clamp_length(SLIDE_MAX);
            b.slid = 0.0;
            b.prev = b.rect;
            b.pressure = 0.0;
            // A bulge still changing keeps the world awake, like a moving body.
            home_moved = home_moved.max(resize(b, &p, dt));
        }
        self.pick_yielder();
        self.snap_pass();
        self.grid_pass();
        for b in &mut self.bodies {
            // A home still drifting keeps the world awake, like a moving body.
            home_moved = home_moved.max(integrate(b, &p, dt));
        }

        for b in &mut self.bodies {
            b.slide_pull = Vec2::ZERO;
        }
        self.drag_pass(dt);
        let mut worst = 0.0f32;
        for it in 0..p.iterations.max(1) {
            worst = self.walls_pass(None, dt, it == 0);
            worst = worst.max(self.pairs_pass(None, dt, it == 0));
            if worst <= SLOP {
                break;
            }
        }
        // Pushing alone did not make room: the bodies are held between
        // walls, so those that can give are squeezed, and all of them feel
        // the pressure (their edges bulge, and next step they slide off).
        if worst > SLOP {
            for _ in 0..SQUEEZE_PASSES {
                self.walls_pass(Some(p.tension), dt, false);
                self.pairs_pass(Some(p.tension), dt, false);
            }
        }
        // Bodies last: where both cannot hold, a wall gives, not a body.
        for _ in 0..p.iterations.max(1) {
            if self.pairs_pass(None, dt, false) <= SLOP {
                break;
            }
        }
        self.land_lifted();

        let mut moved = home_moved;
        let mut moved_free = 0.0f32;
        for b in &mut self.bodies {
            let d = (b.rect.min - b.prev.min)
                .max_abs()
                .max((b.rect.max - b.prev.max).max_abs());
            moved = moved.max(d);
            if !b.dragged {
                moved_free = moved_free.max(d);
                b.vel =
                    ((b.rect.center() - b.prev.center()) * (1.0 / dt)).clamp_length(p.max_speed);
            }
        }
        self.last_move = moved_free;
        self.steps += 1;
        self.calm = if moved <= p.rest_eps {
            self.calm + 1
        } else {
            0
        };
        if self.calm >= p.rest_steps.max(1) {
            self.at_rest = true;
            self.last_move = 0.0;
            for b in &mut self.bodies {
                b.vel = Vec2::ZERO;
            }
        }
        true
    }

    /// The body that gives way this step ([`Body::yielding`]).
    fn pick_yielder(&mut self) {
        let bulge = |b: &Body| {
            if b.dragged {
                0.0
            } else {
                b.bulge.x.max(b.bulge.y)
            }
        };
        // The most bulged, the newest on a tie.
        let mut best: Option<&Body> = None;
        for b in self.bodies.iter().filter(|b| bulge(b) > YIELD_BULGE) {
            if best.is_none_or(|w| bulge(b) >= bulge(w)) {
                best = Some(b);
            }
        }
        let most = best.map_or(0.0, bulge);
        // The one already giving way keeps at it while it is bulged and
        // not far less than the most bulged one.
        let keep = self.yielder.filter(|&id| {
            self.bodies
                .iter()
                .any(|b| b.id == id && bulge(b) > YIELD_BULGE && bulge(b) * 2.0 >= most)
        });
        self.yielder = keep.or(best.map(|b| b.id));
        for b in &mut self.bodies {
            b.yielding = Some(b.id) == self.yielder;
        }
    }

    /// The direction the yielding body is pushed in where two bulged
    /// edges meet tip to tip: away from the side gravity pulls to, and a
    /// little across it toward the middle of the world, so a full column
    /// spills to the side with more room.
    fn yield_dir(&self) -> Vec2 {
        let away = self
            .params
            .gravity_side
            .map_or(Vec2::new(0.0, 1.0), |s| -s.dir());
        let mut across = Vec2::new(-away.y, away.x);
        if let Some(b) = self.bodies.iter().find(|b| b.yielding)
            && (self.bounds.center() - b.rect.center()).dot(across) < 0.0
        {
            across = -across;
        }
        let dir = away + across * 0.25;
        dir * (1.0 / dir.length())
    }

    /// Corner snapping: each corner of a free body looks for the nearest
    /// corner of another body, or of the walls, within
    /// [`Params::snap_range`] and is pulled to line up with it; where the
    /// two bodies face each other the gap stays between them. The pull
    /// fades to nothing at the edge of the range, so entering it is not a
    /// jolt, and it is damped against the other body's motion, so two
    /// bodies moving together are not slowed. Only the corner of a body
    /// that is nearest to its target pulls: two corners wanting different
    /// places (tops or bottoms of panels of nearly the same height) would
    /// otherwise leave the body lined up with neither. A body under
    /// pressure (bulged) lets go in step with its bulge, and is no anchor
    /// for others either: it is about to slide out of line.
    fn snap_pass(&mut self) {
        self.snaps.clear();
        let p = self.params;
        for b in &mut self.bodies {
            b.snap_pull = Vec2::ZERO;
        }
        if p.snap_range <= 0.0 || p.snap_stiffness <= 0.0 {
            return;
        }
        let inner = Rect {
            min: self.bounds.min + Vec2::splat(p.gap),
            max: self.bounds.max - Vec2::splat(p.gap),
        };
        let damping = 2.0 * p.snap_stiffness.sqrt();
        let hold: Vec<f32> = self
            .bodies
            .iter()
            .map(|b| hold(b, p.bulge_rest, p.bulge_max))
            .collect();
        for i in 0..self.bodies.len() {
            let a = &self.bodies[i];
            if a.dragged || hold[i] <= 0.0 {
                continue;
            }
            let mut winner: Option<(Snap, Vec2)> = None;
            for ca in CORNERS {
                let from = corner(&a.rect, ca);
                // (distance, error, the other side's velocity, its hold)
                let mut best: Option<(f32, Vec2, Vec2, f32)> = None;
                let mut consider = |error: Vec2, vel: Vec2, hold: f32| {
                    let dist = error.length();
                    if hold > 0.0 && dist < p.snap_range && best.is_none_or(|b| dist < b.0) {
                        best = Some((dist, error, vel, hold));
                    }
                };
                let (wall_x, wall_y) = corner_sides(ca);
                if p.walls.has(wall_x) && p.walls.has(wall_y) {
                    consider(corner(&inner, ca) - from, Vec2::ZERO, 1.0);
                }
                for (j, o) in self.bodies.iter().enumerate() {
                    if j == i || o.dragged {
                        continue;
                    }
                    for cb in CORNERS {
                        // The same corner of both would put one body on the other.
                        if cb == ca {
                            continue;
                        }
                        let mut error = corner(&o.rect, cb) - from;
                        if cb.0 != ca.0 {
                            error.x -= if ca.0 { p.gap } else { -p.gap };
                        }
                        if cb.1 != ca.1 {
                            error.y -= if ca.1 { p.gap } else { -p.gap };
                        }
                        consider(error, o.vel, hold[j]);
                    }
                }
                if let Some((dist, error, vel, other_hold)) = best {
                    let grip = (1.0 - dist / p.snap_range) * hold[i] * other_hold;
                    if winner.is_none_or(|(w, _)| grip > w.grip) {
                        let pull = (error * p.snap_stiffness + (vel - a.vel) * damping) * grip;
                        let snap = Snap {
                            body: a.id,
                            from,
                            to: from + error,
                            grip,
                        };
                        winner = Some((snap, pull));
                    }
                }
            }
            if let Some((snap, pull)) = winner {
                self.snaps.push(snap);
                self.bodies[i].snap_pull = pull;
            }
        }
    }

    /// The grid (SETL-10): along every contact, the two bodies' edges are
    /// pulled to line up at a whole number of [`Params::grid_pitch`] apart
    /// (a body against a wall: its edge at whole steps from the wall's
    /// corner). The pull is a washboard: a sine of the offset with one
    /// pitch, so it never jumps and each step is a dip, damped near the
    /// dip; it fades in as the two slow down against each other.
    fn grid_pass(&mut self) {
        let p = self.params;
        for b in &mut self.bodies {
            b.grid_pull = Vec2::ZERO;
        }
        if p.grid_pitch <= 0.0 || p.grid_stiffness <= 0.0 {
            return;
        }
        let prefer = self.yield_dir();
        let reach = p.gap + GRID_REACH;
        let corner = self.bounds.min + Vec2::splat(p.gap);
        for i in 0..self.bodies.len() {
            if self.bodies[i].dragged || self.bodies[i].lifted {
                continue;
            }
            for j in i + 1..self.bodies.len() {
                let (a, b) = pair_mut(&mut self.bodies, i, j);
                if b.dragged || b.lifted {
                    continue;
                }
                let Some(hit) = touch(a, b, reach, prefer) else {
                    continue;
                };
                let t = hit.normal.main_axis().other();
                let offset = b.rect.min[t] - a.rect.min[t];
                let pull = washboard(offset, b.vel[t] - a.vel[t], &p) * 0.5;
                a.grid_pull[t] -= pull;
                b.grid_pull[t] += pull;
            }
            let b = &mut self.bodies[i];
            for side in Side::ALL {
                if !p.walls.has(side) {
                    continue;
                }
                let axis = side.axis();
                let room = if side.is_low() {
                    b.rect.min[axis] - (self.bounds.min[axis] + p.gap)
                } else {
                    self.bounds.max[axis] - p.gap - b.rect.max[axis]
                };
                if room > GRID_REACH {
                    continue;
                }
                let t = axis.other();
                let offset = b.rect.min[t] - corner[t];
                b.grid_pull[t] += washboard(offset, b.vel[t], &p);
            }
        }
    }

    /// Contacts with a dragged body, once per step: the held body does not
    /// give way and may overlap the others, which glide out from under it
    /// at [`Params::glide_speed`].
    fn drag_pass(&mut self, dt: f32) {
        let gap = self.params.gap;
        let glide = self.params.glide_speed * dt;
        let prefer = self.yield_dir();
        for i in 0..self.bodies.len() {
            for j in i + 1..self.bodies.len() {
                let (a, b) = pair_mut(&mut self.bodies, i, j);
                if a.dragged == b.dragged {
                    continue;
                }
                let Some(hit) = touch(a, b, gap, prefer) else {
                    continue;
                };
                self.contacts.push(Contact {
                    a: a.id,
                    b: Some(b.id),
                    axis: hit.normal.main_axis(),
                    normal: hit.normal,
                    depth: hit.depth,
                });
                let push = hit.normal * hit.depth.min(glide);
                if a.dragged {
                    b.shift(push);
                } else {
                    a.shift(-push);
                }
            }
        }
    }

    /// Hold every free body inside the closed walls (walls see the plain
    /// rectangle, not the pillow). A body that starts the step outside
    /// (the walls moved) only has to come in by [`Params::glide_speed`].
    /// With `squeeze` the body also gives that share of the depth by
    /// squeezing, and what it cannot give adds to its jam. Returns the
    /// deepest violation found.
    fn walls_pass(&mut self, squeeze: Option<f32>, dt: f32, record: bool) -> f32 {
        let p = self.params;
        let glide = p.glide_speed * dt;
        let mut worst = 0.0f32;
        for b in &mut self.bodies {
            if b.dragged {
                continue;
            }
            for side in Side::ALL {
                if !p.walls.has(side) {
                    continue;
                }
                let axis = side.axis();
                let depth = if side.is_low() {
                    let limit = (self.bounds.min[axis] + p.gap).min(b.prev.min[axis] + glide);
                    limit - b.rect.min[axis]
                } else {
                    let limit = (self.bounds.max[axis] - p.gap).max(b.prev.max[axis] - glide);
                    b.rect.max[axis] - limit
                };
                if depth <= 0.0 {
                    continue;
                }
                worst = worst.max(depth);
                b.pressure += depth;
                if record {
                    self.contacts.push(Contact {
                        a: b.id,
                        b: None,
                        axis,
                        normal: -side.dir(),
                        depth,
                    });
                }
                let mut squeezed = 0.0;
                if let Some(share) = squeeze {
                    squeezed = (depth * share / (1.0 + share)).min(b.give(axis));
                    let felt = share.max(JAM_SHARE);
                    b.jam[axis] += (depth * felt / (1.0 + felt) - squeezed).max(0.0);
                }
                b.squeeze[axis] += squeezed;
                if side.is_low() {
                    b.rect.min[axis] += depth;
                    b.rect.max[axis] += depth - squeezed;
                } else {
                    b.rect.max[axis] -= depth;
                    b.rect.min[axis] -= depth - squeezed;
                }
            }
        }
        worst
    }

    /// Put overlapping free bodies apart, half each, along the separating
    /// axis of their shapes (the axis of least overlap for plain
    /// rectangles; off the axis where a bulged edge meets another). Two
    /// that already overlapped when the step began (one was just added or
    /// dropped there) only have to part by [`Params::glide_speed`], so
    /// nothing jumps. With `squeeze` each body that can give also takes
    /// that share by squeezing its facing side, and what it cannot give
    /// adds to its jam. Returns the deepest overlap found.
    fn pairs_pass(&mut self, squeeze: Option<f32>, dt: f32, record: bool) -> f32 {
        let gap = self.params.gap;
        let glide = self.params.glide_speed * dt;
        let prefer = self.yield_dir();
        let mut worst = 0.0f32;
        for i in 0..self.bodies.len() {
            for j in i + 1..self.bodies.len() {
                let (a, b) = pair_mut(&mut self.bodies, i, j);
                if a.dragged || b.dragged {
                    continue;
                }
                let Some(hit) = touch(a, b, gap, prefer) else {
                    continue;
                };
                let (ab, bb) = pair_bulges(a, b);
                let allowed = touch_at(&a.prev, ab, &b.prev, bb, gap, tie_dir(a, b, prefer))
                    .map_or(0.0, |before| (before.depth - glide).max(0.0));
                let depth = hit.depth - allowed;
                if depth <= 0.0 {
                    continue;
                }
                let axis = hit.normal.main_axis();
                worst = worst.max(depth);
                a.pressure += depth;
                b.pressure += depth;
                if record {
                    self.contacts.push(Contact {
                        a: a.id,
                        b: Some(b.id),
                        axis,
                        normal: hit.normal,
                        depth,
                    });
                    // The pressure on a slope pushes along it too: the
                    // bulges stand for the pressure, the slope of the
                    // normal for how much of it goes sideways.
                    let mut along = hit.normal;
                    along[axis] = 0.0;
                    let (ab, bb) = pair_bulges(a, b);
                    let push = along * ((ab[axis] + bb[axis]) * SLIDE_STIFFNESS);
                    a.slide_pull -= push;
                    b.slide_pull += push;
                }
                let a_low = hit.normal[axis] > 0.0;
                let (lo, hi) = if a_low { (a, b) } else { (b, a) };
                let (mut s_lo, mut s_hi) = (0.0, 0.0);
                if let Some(share) = squeeze {
                    let w_lo = if lo.give(axis) > 0.0 { share } else { 0.0 };
                    let w_hi = if hi.give(axis) > 0.0 { share } else { 0.0 };
                    let total = 2.0 + w_lo + w_hi;
                    s_lo = (depth * w_lo / total).min(lo.give(axis));
                    s_hi = (depth * w_hi / total).min(hi.give(axis));
                    let felt = share.max(JAM_SHARE);
                    let each = depth * felt / (2.0 + 2.0 * felt);
                    lo.jam[axis] += (each - s_lo).max(0.0);
                    hi.jam[axis] += (each - s_hi).max(0.0);
                }
                let each = hit.normal * ((depth - s_lo - s_hi) * 0.5);
                lo.squeeze[axis] += s_lo;
                lo.rect.max[axis] -= s_lo;
                hi.squeeze[axis] += s_hi;
                hi.rect.min[axis] += s_hi;
                // `normal` points from `a` to `b`. Along a slope a body
                // moves sideways at most `glide` per step; past that the
                // push is straight, so making room by sliding is a glide.
                let (a, b) = if a_low { (lo, hi) } else { (hi, lo) };
                let mut sideways = each;
                sideways[axis] = 0.0;
                let straight = each - sideways;
                let (slide_a, slide_b) = (
                    spend_slide(a, sideways, glide),
                    spend_slide(b, sideways, glide),
                );
                a.shift(-straight - slide_a);
                b.shift(straight + slide_b);
            }
        }
        worst
    }

    /// A lifted body that is let go and touches nothing turns solid.
    fn land_lifted(&mut self) {
        let gap = self.params.gap;
        let prefer = self.yield_dir();
        for i in 0..self.bodies.len() {
            if !self.bodies[i].lifted || self.bodies[i].dragged {
                continue;
            }
            let a = &self.bodies[i];
            let free =
                self.bodies.iter().enumerate().all(|(j, o)| {
                    j == i || touch(a, o, gap, prefer).is_none_or(|h| h.depth <= SLOP)
                });
            if free {
                self.bodies[i].lifted = false;
            }
        }
    }
}

/// The part of a sideways move `d` that is left in the body's slide budget
/// for this step (`glide` points), taken from it.
fn spend_slide(b: &mut Body, d: Vec2, glide: f32) -> Vec2 {
    let (len, room) = (d.length(), (glide - b.slid).max(0.0));
    let d = if len > room && len > 0.0 {
        d * (room / len)
    } else {
        d
    };
    b.slid += d.length();
    d
}

/// How firmly a body holds on to its corner snaps and its home: 1, but
/// the yielding body lets go in step with its bulge (0 fully bulged) and
/// is no anchor for other bodies' snaps either.
fn hold(b: &Body, rest: f32, max: f32) -> f32 {
    if !b.yielding {
        return 1.0;
    }
    let range = max - rest;
    if range <= 0.0 {
        return 1.0;
    }
    let bulge = (b.bulge.x.max(b.bulge.y) - rest).max(0.0);
    (1.0 - bulge / range).clamp(0.0, 1.0)
}

/// The washboard pull on an offset along a contact: toward the nearest
/// whole step of [`Params::grid_pitch`], a sine of the offset (strongest a
/// quarter pitch off), damped near the step against `rel_vel`, and faded
/// out as `rel_vel` grows to [`GRID_CALM_SPEED`].
fn washboard(offset: f32, rel_vel: f32, p: &Params) -> f32 {
    let pitch = p.grid_pitch;
    let off = offset - pitch * (offset / pitch).round();
    let phase = std::f32::consts::TAU * off / pitch;
    let calm = (1.0 - rel_vel.abs() / GRID_CALM_SPEED).clamp(0.0, 1.0);
    let spring = -p.grid_stiffness * pitch / std::f32::consts::TAU * phase.sin();
    let damp = -p.grid_stiffness.sqrt() * rel_vel * phase.cos().max(0.0);
    (spring + damp) * calm
}

/// Follow the wanted size, let go of squeeze and jam, keeping the middle,
/// and let the bulge follow the pressure. Returns how much the bulge
/// changed.
fn resize(b: &mut Body, p: &Params, dt: f32) -> f32 {
    let target = b.want.max(b.min_size).min(b.max_size);
    let grow = p.grow_speed * dt;
    let keep = (-p.restore * dt).exp();
    let mut bulged = 0.0f32;
    for axis in Axis::BOTH {
        b.natural[axis] += (target[axis] - b.natural[axis]).clamp(-grow, grow);
        let mut squeeze = b.squeeze[axis] * keep;
        if squeeze < 1e-4 {
            squeeze = 0.0;
        }
        b.squeeze[axis] = squeeze.min((b.natural[axis] - b.min_size[axis]).max(0.0));
        b.jam[axis] *= keep;
        if b.jam[axis] < 1e-4 {
            b.jam[axis] = 0.0;
        }
        let size = (b.natural[axis] - b.squeeze[axis]).max(0.0);
        let d = size - b.rect.size()[axis];
        // Below this the size is left alone, so a body at rest stays bit
        // for bit where it is.
        if d.abs() > 1e-4 {
            b.rect.min[axis] -= d * 0.5;
            b.rect.max[axis] += d * 0.5;
        }
        // The edges across `axis` bulge: they are the ones pressed.
        let edge = b.rect.size()[axis.other()];
        let cap = p.bulge_max.min(edge * 0.25).max(0.0);
        let load = (b.load()[axis] - p.bulge_from).max(0.0);
        let want = (p.bulge_rest + p.bulge_gain * load).clamp(0.0, cap);
        let step = p.bulge_speed.max(0.0) * dt;
        let bulge = b.bulge[axis] + (want - b.bulge[axis]).clamp(-step, step);
        bulged = bulged.max((bulge - b.bulge[axis]).abs());
        b.bulge[axis] = if bulge < 1e-4 { 0.0 } else { bulge };
    }
    bulged
}

/// Gravity, the home spring and damping move a free body; a dragged one
/// goes where it is held. Returns how far the home spot drifted.
fn integrate(b: &mut Body, p: &Params, dt: f32) -> f32 {
    let center = b.rect.center();
    if b.dragged {
        b.rect = b.rect.translate(b.drag_target - center);
        b.vel = Vec2::ZERO;
        b.gravity_pull = Vec2::ZERO;
        b.home_pull = Vec2::ZERO;
        b.snap_pull = Vec2::ZERO;
        b.grid_pull = Vec2::ZERO;
        b.slide_pull = Vec2::ZERO;
        return 0.0;
    }
    b.gravity_pull = p.gravity_side.map_or(Vec2::ZERO, |s| s.dir() * p.gravity);
    // A body under pressure is about to slide out of its row or column:
    // its home lets go in step with its bulge and drifts to where it goes.
    let hold = hold(b, p.bulge_rest, p.bulge_max);
    b.home_pull = b
        .home
        .map_or(Vec2::ZERO, |h| (h - center) * (p.home_stiffness * hold));
    let damp = (-p.damping * dt).exp();
    let pull = b.gravity_pull + b.home_pull + b.snap_pull + b.grid_pull + b.slide_pull;
    b.vel = ((b.vel + pull * dt) * damp).clamp_length(p.max_speed);
    b.rect = b.rect.translate(b.vel * dt);
    let drift = p.home_drift + (1.0 - hold) * PRESSED_DRIFT;
    if drift > 0.0
        && let Some(home) = &mut b.home
    {
        let d = (center - *home) * (1.0 - (-drift * dt).exp());
        *home += d;
        return d.max_abs();
    }
    0.0
}

/// The contact between two bodies kept `gap` apart, through their shapes;
/// `yield_dir` from [`World::yield_dir`].
fn touch(a: &Body, b: &Body, gap: f32, yield_dir: Vec2) -> Option<Hit> {
    let (ab, bb) = pair_bulges(a, b);
    touch_at(&a.rect, ab, &b.rect, bb, gap, tie_dir(a, b, yield_dir))
}

/// The bulges a pair meets with: their own when one of them is the
/// yielding body, none otherwise. Two bulged edges meeting tip to tip are
/// unstable by design; between bodies that are not to leave, the smallest
/// offset would grow and send both off, so they meet as rectangles (and
/// squeeze, as before).
fn pair_bulges(a: &Body, b: &Body) -> (Vec2, Vec2) {
    if a.yielding || b.yielding {
        (a.bulge, b.bulge)
    } else {
        (Vec2::ZERO, Vec2::ZERO)
    }
}

/// Which way a tie between two tips goes for this pair (as [`sat`] takes
/// it: the direction `b` is pushed in): the yielding body is pushed along
/// `yield_dir`; between two others it is split, straight along the axis.
fn tie_dir(a: &Body, b: &Body, yield_dir: Vec2) -> Vec2 {
    if b.yielding {
        yield_dir
    } else if a.yielding {
        -yield_dir
    } else {
        Vec2::ZERO
    }
}

/// The contact between two pillows kept `gap` apart.
///
/// The gap between the pillows is the gap less both bulges along the
/// normal (it can go below 0), so two tips meeting keep the edges behind
/// them exactly `gap` apart: the spacing the user sees stays even, and a
/// bulge growing takes no room (it would squeeze its neighbours, which
/// would bulge more). Off the tips the slanted edges let the rectangles
/// come closer, which is the room sliding off frees; the rectangles
/// themselves are still kept a [`RECT_FLOOR`] share of the gap apart, so
/// what the user sees never overlaps. Plain rectangles take the old path:
/// the axis of least overlap, `a` low on equal middles.
fn touch_at(a: &Rect, ab: Vec2, b: &Rect, bb: Vec2, gap: f32, prefer: Vec2) -> Option<Hit> {
    if ab == Vec2::ZERO && bb == Vec2::ZERO {
        return rect_hit(a, b, gap);
    }
    // Grown by the bulges and kept the gap less the bulges apart is the
    // plain overlap at the full gap: nothing touches beyond it.
    let o = a.overlap(b, gap);
    if o.x <= 0.0 || o.y <= 0.0 {
        return None;
    }
    let sum = ab + bb;
    let gap_along = |n: Vec2| gap - n.x.abs() * sum.x - n.y.abs() * sum.y;
    let pillow = sat(
        &Poly::pillow(a, ab),
        &Poly::pillow(b, bb),
        gap_along,
        prefer,
    );
    let Some(floor) = rect_hit(a, b, gap * RECT_FLOOR) else {
        return pillow;
    };
    if pillow.is_some_and(|p| p.depth >= floor.depth) {
        return pillow;
    }
    // Held by the floor: still pushed along the slope where the pillows
    // are in each other, as far as it takes to get the rectangles to the
    // floor, so the body keeps sliding off instead of stopping on a flat.
    let slope = sat(&Poly::pillow(a, ab), &Poly::pillow(b, bb), |_| 0.0, prefer);
    let axis = floor.normal.main_axis();
    Some(match slope {
        Some(s) if s.normal[axis] * floor.normal[axis] > 0.5 => Hit {
            normal: s.normal,
            depth: floor.depth / (s.normal[axis] * floor.normal[axis]),
        },
        _ => floor,
    })
}

/// Two rectangles kept `gap` apart: the axis of least overlap and the
/// depth, the normal pointing from `a` to `b` (`a` low on equal middles,
/// so the answer never depends on anything but the order of the bodies).
fn rect_hit(a: &Rect, b: &Rect, gap: f32) -> Option<Hit> {
    let o = a.overlap(b, gap);
    if o.x <= 0.0 || o.y <= 0.0 {
        return None;
    }
    let axis = if o.x < o.y { Axis::X } else { Axis::Y };
    let mut normal = Vec2::ZERO;
    normal[axis] = if a.center()[axis] <= b.center()[axis] {
        1.0
    } else {
        -1.0
    };
    Some(Hit {
        normal,
        depth: o[axis],
    })
}

/// The four corners as (high x, high y): `false` is the low side (left, top).
const CORNERS: [(bool, bool); 4] = [(false, false), (true, false), (false, true), (true, true)];

fn corner(r: &Rect, (high_x, high_y): (bool, bool)) -> Vec2 {
    Vec2::new(
        if high_x { r.max.x } else { r.min.x },
        if high_y { r.max.y } else { r.min.y },
    )
}

/// The two walls that meet at a corner.
fn corner_sides((high_x, high_y): (bool, bool)) -> (Side, Side) {
    (
        if high_x { Side::Right } else { Side::Left },
        if high_y { Side::Bottom } else { Side::Top },
    )
}

fn pair_mut(bodies: &mut [Body], i: usize, j: usize) -> (&mut Body, &mut Body) {
    debug_assert!(i < j);
    let (head, tail) = bodies.split_at_mut(j);
    (&mut head[i], &mut tail[0])
}
