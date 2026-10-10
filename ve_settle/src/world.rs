//! The world: bodies, forces, contacts and the fixed step that moves them.

use crate::geom::{Axis, Rect, Side, Vec2};
use crate::params::Params;

/// Overlaps below this (points) count as touching.
const SLOP: f32 = 0.01;
/// Squeeze passes per step when pushing alone leaves bodies without room.
const SQUEEZE_PASSES: u32 = 4;
/// Most fixed steps one [`World::advance`] runs, so a slow frame never
/// turns into a long catch-up.
const MAX_STEPS_PER_ADVANCE: u32 = 8;

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
    drag_target: Vec2,
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

    /// How much more it can be squeezed on `axis`.
    fn give(&self, axis: Axis) -> f32 {
        (self.rect.size()[axis] - self.min_size[axis]).max(0.0)
    }

    fn shift(&mut self, axis: Axis, d: f32) {
        self.rect.min[axis] += d;
        self.rect.max[axis] += d;
    }
}

/// A contact the solver worked on in the last step (debug overlay).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Contact {
    /// The body.
    pub a: BodyId,
    /// The other body, or `None` for a wall.
    pub b: Option<BodyId>,
    /// The axis they were pushed apart on.
    pub axis: Axis,
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
            drag_target: desc.center,
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
            b.prev = b.rect;
            b.pressure = 0.0;
            resize(b, &p, dt);
        }
        self.snap_pass();
        for b in &mut self.bodies {
            // A home still drifting keeps the world awake, like a moving body.
            home_moved = home_moved.max(integrate(b, &p, dt));
        }

        self.drag_pass(dt);
        let mut worst = 0.0f32;
        for it in 0..p.iterations.max(1) {
            worst = self.walls_pass(0.0, dt, it == 0);
            worst = worst.max(self.pairs_pass(0.0, dt, it == 0));
            if worst <= SLOP {
                break;
            }
        }
        // Pushing alone did not make room: the bodies are held between
        // walls, so those that can give are squeezed.
        if worst > SLOP && p.tension > 0.0 {
            for _ in 0..SQUEEZE_PASSES {
                self.walls_pass(p.tension, dt, false);
                self.pairs_pass(p.tension, dt, false);
            }
        }
        // Bodies last: where both cannot hold, a wall gives, not a body.
        for _ in 0..p.iterations.max(1) {
            if self.pairs_pass(0.0, dt, false) <= SLOP {
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

    /// Corner snapping: each corner of a free body looks for the nearest
    /// corner of another body, or of the walls, within
    /// [`Params::snap_range`] and is pulled to line up with it; where the
    /// two bodies face each other the gap stays between them. The pull
    /// fades to nothing at the edge of the range, so entering it is not a
    /// jolt, and it is damped against the other body's motion, so two
    /// bodies moving together are not slowed. Only the corner of a body
    /// that is nearest to its target pulls: two corners wanting different
    /// places (tops or bottoms of panels of nearly the same height) would
    /// otherwise leave the body lined up with neither.
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
        for i in 0..self.bodies.len() {
            let a = &self.bodies[i];
            if a.dragged {
                continue;
            }
            let mut winner: Option<(Snap, Vec2)> = None;
            for ca in CORNERS {
                let from = corner(&a.rect, ca);
                // (distance, error, the other side's velocity)
                let mut best: Option<(f32, Vec2, Vec2)> = None;
                let mut consider = |error: Vec2, vel: Vec2| {
                    let dist = error.length();
                    if dist < p.snap_range && best.is_none_or(|b| dist < b.0) {
                        best = Some((dist, error, vel));
                    }
                };
                let (wall_x, wall_y) = corner_sides(ca);
                if p.walls.has(wall_x) && p.walls.has(wall_y) {
                    consider(corner(&inner, ca) - from, Vec2::ZERO);
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
                        consider(error, o.vel);
                    }
                }
                if let Some((dist, error, vel)) = best {
                    let grip = 1.0 - dist / p.snap_range;
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

    /// Contacts with a dragged body, once per step: the held body does not
    /// give way and may overlap the others, which glide out from under it
    /// at [`Params::glide_speed`].
    fn drag_pass(&mut self, dt: f32) {
        let gap = self.params.gap;
        let glide = self.params.glide_speed * dt;
        for i in 0..self.bodies.len() {
            for j in i + 1..self.bodies.len() {
                let (a, b) = pair_mut(&mut self.bodies, i, j);
                if a.dragged == b.dragged {
                    continue;
                }
                let Some((axis, depth, a_low)) = contact(&a.rect, &b.rect, gap) else {
                    continue;
                };
                self.contacts.push(Contact {
                    a: a.id,
                    b: Some(b.id),
                    axis,
                    depth,
                });
                let push = depth.min(glide);
                let (lo, hi) = if a_low { (a, b) } else { (b, a) };
                if lo.dragged {
                    hi.shift(axis, push);
                } else {
                    lo.shift(axis, -push);
                }
            }
        }
    }

    /// Hold every free body inside the closed walls. A body that starts
    /// the step outside (the walls moved) only has to come in by
    /// [`Params::glide_speed`]. With `share` above 0 the body also gives
    /// that share of the depth by squeezing. Returns the deepest violation
    /// found.
    fn walls_pass(&mut self, share: f32, dt: f32, record: bool) -> f32 {
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
                        depth,
                    });
                }
                let squeeze = (depth * share / (1.0 + share)).min(b.give(axis));
                b.squeeze[axis] += squeeze;
                if side.is_low() {
                    b.rect.min[axis] += depth;
                    b.rect.max[axis] += depth - squeeze;
                } else {
                    b.rect.max[axis] -= depth;
                    b.rect.min[axis] -= depth - squeeze;
                }
            }
        }
        worst
    }

    /// Put overlapping free bodies apart along the axis of least overlap,
    /// half each. Two that already overlapped when the step began (one was
    /// just added or dropped there) only have to part by
    /// [`Params::glide_speed`], so nothing jumps. With `share` above 0 each
    /// body that can give also takes that share by squeezing its facing
    /// side. Returns the deepest overlap found.
    fn pairs_pass(&mut self, share: f32, dt: f32, record: bool) -> f32 {
        let gap = self.params.gap;
        let glide = self.params.glide_speed * dt;
        let mut worst = 0.0f32;
        for i in 0..self.bodies.len() {
            for j in i + 1..self.bodies.len() {
                let (a, b) = pair_mut(&mut self.bodies, i, j);
                if a.dragged || b.dragged {
                    continue;
                }
                let Some((axis, depth, a_low)) = contact(&a.rect, &b.rect, gap) else {
                    continue;
                };
                let before = a.prev.overlap(&b.prev, gap);
                let allowed = if before.x > 0.0 && before.y > 0.0 {
                    (before[axis] - glide).max(0.0)
                } else {
                    0.0
                };
                let depth = depth - allowed;
                if depth <= 0.0 {
                    continue;
                }
                worst = worst.max(depth);
                a.pressure += depth;
                b.pressure += depth;
                if record {
                    self.contacts.push(Contact {
                        a: a.id,
                        b: Some(b.id),
                        axis,
                        depth,
                    });
                }
                let (lo, hi) = if a_low { (a, b) } else { (b, a) };
                let w_lo = if lo.give(axis) > 0.0 { share } else { 0.0 };
                let w_hi = if hi.give(axis) > 0.0 { share } else { 0.0 };
                let total = 2.0 + w_lo + w_hi;
                let s_lo = (depth * w_lo / total).min(lo.give(axis));
                let s_hi = (depth * w_hi / total).min(hi.give(axis));
                let each = (depth - s_lo - s_hi) * 0.5;
                lo.squeeze[axis] += s_lo;
                lo.rect.max[axis] -= s_lo;
                lo.shift(axis, -each);
                hi.squeeze[axis] += s_hi;
                hi.rect.min[axis] += s_hi;
                hi.shift(axis, each);
            }
        }
        worst
    }

    /// A lifted body that is let go and touches nothing turns solid.
    fn land_lifted(&mut self) {
        let gap = self.params.gap;
        for i in 0..self.bodies.len() {
            if !self.bodies[i].lifted || self.bodies[i].dragged {
                continue;
            }
            let rect = self.bodies[i].rect;
            let free = self.bodies.iter().enumerate().all(|(j, o)| {
                let ov = rect.overlap(&o.rect, gap);
                j == i || ov.x.min(ov.y) <= SLOP
            });
            if free {
                self.bodies[i].lifted = false;
            }
        }
    }
}

/// Follow the wanted size and let go of squeeze, keeping the middle.
fn resize(b: &mut Body, p: &Params, dt: f32) {
    let target = b.want.max(b.min_size).min(b.max_size);
    let grow = p.grow_speed * dt;
    let keep = (-p.restore * dt).exp();
    for axis in Axis::BOTH {
        b.natural[axis] += (target[axis] - b.natural[axis]).clamp(-grow, grow);
        let mut squeeze = b.squeeze[axis] * keep;
        if squeeze < 1e-4 {
            squeeze = 0.0;
        }
        b.squeeze[axis] = squeeze.min((b.natural[axis] - b.min_size[axis]).max(0.0));
        let size = (b.natural[axis] - b.squeeze[axis]).max(0.0);
        let d = size - b.rect.size()[axis];
        // Below this the size is left alone, so a body at rest stays bit
        // for bit where it is.
        if d.abs() > 1e-4 {
            b.rect.min[axis] -= d * 0.5;
            b.rect.max[axis] += d * 0.5;
        }
    }
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
        return 0.0;
    }
    b.gravity_pull = p.gravity_side.map_or(Vec2::ZERO, |s| s.dir() * p.gravity);
    b.home_pull = b
        .home
        .map_or(Vec2::ZERO, |h| (h - center) * p.home_stiffness);
    let damp = (-p.damping * dt).exp();
    let pull = b.gravity_pull + b.home_pull + b.snap_pull;
    b.vel = ((b.vel + pull * dt) * damp).clamp_length(p.max_speed);
    b.rect = b.rect.translate(b.vel * dt);
    if p.home_drift > 0.0
        && let Some(home) = &mut b.home
    {
        let d = (center - *home) * (1.0 - (-p.home_drift * dt).exp());
        *home += d;
        return d.max_abs();
    }
    0.0
}

/// The contact between two rectangles kept `gap` apart: the axis of least
/// overlap, the depth, and whether `a` is the one on the low side. Equal
/// middles put `a` low, so the answer never depends on anything but the
/// order of the bodies.
fn contact(a: &Rect, b: &Rect, gap: f32) -> Option<(Axis, f32, bool)> {
    let o = a.overlap(b, gap);
    if o.x <= 0.0 || o.y <= 0.0 {
        return None;
    }
    let axis = if o.x < o.y { Axis::X } else { Axis::Y };
    Some((axis, o[axis], a.center()[axis] <= b.center()[axis]))
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
