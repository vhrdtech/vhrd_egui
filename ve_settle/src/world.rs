//! The world: bodies, forces, contacts and the fixed step that moves them.

use crate::geom::{Axis, Hit, Poly, Rect, Side, Vec2, contact, sat};
use crate::nest;
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
/// Bodies whose leading edge is within this share of the row's body size
/// behind it belong to the row the yielder runs into (SETL-11).
const ROW_SLACK: f32 = 0.25;
/// Bodies whose middles are this close along gravity count as level when
/// picking the yielder, points.
const YIELD_ROW: f32 = 40.0;
/// A body bulged more than this (points) can be the one that gives way.
const YIELD_BULGE: f32 = 0.5;
/// The share of [`Params::gap`] two bulged bodies' rectangles are kept
/// apart at least: off the tips, slanted edges let them come closer than
/// the gap, never into each other.
const RECT_FLOOR: f32 = 0.25;
/// How much deeper a slide along a face may take a body into one it lies
/// face to face with, as a share of the slide: along a face (a hair where
/// the vertices meet) is about none, across a face at 60° most of it.
const PARTNER_HAIR: f32 = 0.25;
/// How far beyond the gap a body counts as lying on what is on gravity's
/// side of it, points, for a slide not to take it off (SETL-34).
const LIFT_REACH: f32 = 1.0;
/// A stretch being let go that is down to this (points) is dropped
/// (SETL-12): the pull is a share of the distance per second, which alone
/// would never arrive.
const ATTRACT_LANDS: f32 = 0.02;
/// An edge that is pulled stays so up to this many times
/// [`Params::attract_range`] (SETL-12).
const ATTRACT_KEEPS: f32 = 1.1;

/// An edge moved by vertex attraction keeps the world awake down to this
/// share of [`Params::rest_eps`] per step (SETL-12).
const ATTRACT_AWAKE: f32 = 0.2;
/// After this many steps in a row without more than a tremble the world
/// rests (two seconds by default).
const SLEEP_STEPS: u32 = 240;
/// A tremble: a move of at most this many times [`Params::rest_eps`] per
/// step (0.02 points, 2.4 points/s by default).
const SLEEP_COARSE: f32 = 4.0;
/// The most speed a body keeps from a step in which contacts put it back
/// onto a slope it sits on, points/s (SETL-15).
const SEAT_SPEED: f32 = 60.0;
/// A body whose edge on gravity's side is within the gap and this much
/// (points) of a wall or a body rests on it (SETL-12).
const RESTS_SLACK: f32 = 0.25;

/// Steps over which a change of parameters comes in (a quarter second
/// at the default step).
pub const PARAM_RAMP_STEPS: u32 = 30;
/// The core of a body no other body's core may enter, as a share of its
/// width and height: a rhombus around its middle.
pub const CORE: f32 = 0.5;
/// A body let go stays lifted this many steps at most (three seconds
/// at the default step).
const LIFT_STEPS: u32 = 360;
/// A body that does not push and was let go where it finds no free place
/// in this many steps (half a second) goes back where it was picked up
/// (SETL-40).
const REFUSE_STEPS: u32 = 60;
/// Two solid bodies deeper in each other than this (points) after the
/// solver's passes are parted by the hard pass.
const EJECT_FROM: f32 = 0.5;
/// Seconds after the last change from outside at which a world still
/// moving starts to cool, and the seconds that takes; then it rests.
pub const COOL_AFTER: f32 = 12.0;
/// See [`COOL_AFTER`].
pub const COOL_SECS: f32 = 3.0;
/// Passes of the hard constraints (cores, walls) per step, at most.
const HARD_PASSES: u32 = 16;
/// A corner snaps to no body faster than this, points/s (as [`DRAG_FROM`]).
const SNAP_MOVING: f32 = DRAG_FROM;
/// A body let go is still settling in this many steps after it landed
/// (half a second): what a wall moves it by then gives it no speed, even
/// where it landed in a full row (SETL-25).
const LANDED_STEPS: u32 = 60;
/// [`Params::drag`] acts on the speed above this, points/s: twice what the
/// default gravity and damping reach, so slow settling stays as it was.
const DRAG_FROM: f32 = 120.0;
/// Two bodies within the gap and this much more of each other stick
/// together for a group drag (SETL-22), points.
pub const CLUSTER_SLACK: f32 = 1.0;

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
    /// Its outline, a convex polygon in unit coordinates of its rectangle
    /// ([`Poly::fit`]); `None` is the rectangle itself.
    pub outline: Option<Poly>,
    /// Whether it pushes its neighbours out of its way when it is dragged
    /// onto them, dropped there or grows into them ([`Body::pushes`]).
    pub pushes: bool,
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
            outline: None,
            pushes: true,
        }
    }

    /// Give it a convex outline in unit coordinates of its rectangle
    /// ([`Poly::unit_hexagon`] and the like).
    pub fn outline(mut self, outline: Poly) -> Self {
        self.outline = Some(outline);
        self
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

    /// Whether it pushes its neighbours ([`Body::pushes`], on by default).
    pub fn pushes(mut self, pushes: bool) -> Self {
        self.pushes = pushes;
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
    /// Pinned in place ([`World::set_pinned`], SETL-27): it never moves,
    /// gives or stretches; the others rest against it as against a wall.
    pub pinned: bool,
    /// Whether it pushes its neighbours (SETL-40, [`World::set_pushes`]):
    /// dragged over them they glide out from under it, dropped or grown into
    /// them they are pushed aside. Off, nothing moves for it: held, it
    /// floats over the others; let go or growing where they are, it is the
    /// one that gives way, and stops at their edge.
    pub pushes: bool,
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
    /// The jam it took on in the last step: on such an axis it grows no
    /// more than `room` lets it.
    jammed: Vec2,
    /// How far its rectangle may still grow toward its nearest plain
    /// neighbour on each axis, either side, before the two are closer than
    /// [`RECT_FLOOR`] of the gap, as the last step left them, points.
    room: Vec2,
    /// Its outline when it is not the plain rectangle: a convex polygon in
    /// unit coordinates of `rect`, so it is squeezed and resized with it
    /// (SETL-4). Such a body does not bulge.
    pub outline: Option<Poly>,
    /// How far vertex attraction (SETL-12) has moved the left and the top
    /// edge outward, points (negative: inward): the body is that much
    /// larger or smaller than its content wants, so its edges line up with
    /// its neighbours'.
    pub stretch_lo: Vec2,
    /// The same for the right and the bottom edge.
    pub stretch_hi: Vec2,
    /// How far vertex attraction moved the middle in the last step: not
    /// motion, so it gives no speed.
    attract_shift: Vec2,
    /// Contacts put it back onto a slope it sits on in this step
    /// (SETL-15): it keeps next to no speed from that.
    seated: bool,
    /// Which edges vertex attraction pulled in the last step, per axis:
    /// [low, high].
    caught: [[bool; 2]; 2],
    /// Whether it lay face to face with a neighbour's outline in range in
    /// the last step (SETL-20).
    faced: bool,
    /// A slide along a face would have run into something: it waits until
    /// something changes (SETL-20).
    slide_blocked: bool,
    /// How deep contacts found the left and the top edge in something
    /// since the last attraction pass, points.
    pressed_lo: Vec2,
    /// The same for the right and the bottom edge.
    pressed_hi: Vec2,
    drag_target: Vec2,
    /// How far slopes moved it sideways this step, points.
    slid: f32,
    /// Steps it has been lifted without being held.
    aloft: u32,
    /// How far the contacts of this step have moved it in all, if it
    /// or what it touches has an outline: held to a glide per step
    /// (SETL-17).
    net: Vec2,
    /// Whether contacts with an outline moved it this step.
    nudged: bool,
    /// Its speed before the contacts of this step: what a wall moved it by
    /// beyond stopping that gives it no speed (SETL-25).
    coast: Vec2,
    /// Steps since it was let go and landed (0 until it has landed):
    /// [`LANDED_STEPS`] and more for a body never dropped.
    landed: u32,
    /// How far a held body pushed it this step (SETL-25), and contacts
    /// put it back onto a slope it sits on (SETL-37): no speed.
    shoved: Vec2,
    /// How far the walls (their gap lines, `walls_pass`) moved it this step
    /// (SETL-25).
    walled: Vec2,
    /// The free cell it glides into after a drop (SETL-28), while it is on
    /// its way: held by the world, not by the user.
    nest: Option<Vec2>,
    /// Where its middle was when it was grabbed, until it lands again: where
    /// one that does not push goes back to when it finds no room (SETL-40).
    picked_from: Option<Vec2>,
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

    /// Its shape. With an outline: the outline laid into its rectangle,
    /// each vertex where vertex attraction has it (what the UI draws;
    /// contacts go by the outline without that). Without one: the
    /// collision shape, the pillow of its rectangle with [`Body::bulge`]
    /// (the rectangle itself when there is none; the UI draws the plain
    /// rectangle).
    pub fn shape(&self) -> Poly {
        match self.outline {
            Some(_) => self.shape_in(&self.rect),
            None => Poly::pillow(&self.rect, self.bulge),
        }
    }

    /// The outline (or the plain rectangle) as it lies in `rect`.
    fn shape_in(&self, rect: &Rect) -> Poly {
        match self.outline {
            Some(outline) => outline.fit(rect),
            None => Poly::rect(rect),
        }
    }

    /// The free cell it is gliding into after a drop, if it is on its way
    /// there ([`World::release_to_cell`], SETL-28).
    pub fn nesting(&self) -> Option<Vec2> {
        self.nest
    }

    /// How much vertex attraction adds to the size on each axis (negative:
    /// takes off), points (SETL-12).
    pub fn stretch(&self) -> Vec2 {
        self.stretch_lo + self.stretch_hi
    }

    /// Where it would be without vertex attraction: what captures are
    /// decided on, so a pull never keeps itself alive, and the corners
    /// corner snapping measures from.
    pub fn rest_rect(&self) -> Rect {
        Rect {
            min: self.rect.min + self.stretch_lo,
            max: self.rect.max - self.stretch_hi,
        }
    }

    /// How much more it can be squeezed on `axis`. An outline is not
    /// squeezed: its shape is the user's, and the room between its size and
    /// its minimum is vertex attraction's (SETL-20).
    fn give(&self, axis: Axis) -> f32 {
        if self.outline.is_some() || self.pinned {
            return 0.0;
        }
        (self.rect.size()[axis] - self.min_size[axis]).max(0.0)
    }

    /// Whether its size on `axis` is fixed (its minimum is its maximum).
    fn rigid(&self, axis: Axis) -> bool {
        self.max_size[axis] - self.min_size[axis] < 0.01
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
    /// Where the corner is, as snapping measures it: a corner of
    /// [`Body::rest_rect`] (one of [`World::snap_anchors`]).
    pub from: Vec2,
    /// Where it is pulled to: lined up with the other corner, the gap
    /// kept where the two face each other.
    pub to: Vec2,
    /// How firmly it is held, 0 at the edge of [`Params::snap_range`] to
    /// 1 when in place.
    pub grip: f32,
}

/// A vertex being pulled toward a vertex or an edge of another body, or a
/// wall, in the last step (SETL-12, debug overlay). The pull moves the
/// edges that meet at the vertex: it resizes the body.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pull {
    /// The body whose vertex it is.
    pub body: BodyId,
    /// Where the vertex is.
    pub from: Vec2,
    /// Where it is pulled to; on an axis nothing pulls on, where it is.
    pub to: Vec2,
    /// How firmly: 1 (kept for the overlay, which draws snaps and pulls
    /// alike).
    pub grip: f32,
    /// It is there (within [`Pull::LOCKED`] points) and held: the edges
    /// it moved stay lined up until the two are more than 1.1 ×
    /// [`Params::attract_range`] apart without their stretch.
    pub locked: bool,
}

impl Pull {
    /// A pull with less than this left to go (points) is locked.
    pub const LOCKED: f32 = 0.25;
}

/// Bodies in a rectangle, stepped with a fixed time step.
///
/// The same calls in the same order always give the same result, bit for
/// bit: there is no randomness, no clock and no hash-order iteration.
#[derive(Clone, Debug)]
pub struct World {
    /// The parameters the solver works with now: on their way to
    /// `wanted` while a change is ramped in.
    params: Params,
    /// The parameters that were set.
    wanted: Params,
    /// Steps left until `params` are `wanted`.
    ramp: u32,
    bounds: Rect,
    /// Where the hard walls stand: `bounds`, except that a wall moved
    /// inward is followed in by its hard wall only as the bodies it left
    /// outside come in.
    hard: Rect,
    bodies: Vec<Body>,
    contacts: Vec<Contact>,
    snaps: Vec<Snap>,
    /// Which corner of its body each snap is (for [`World::snaps`] after the
    /// step).
    snap_corners: Vec<(bool, bool)>,
    pulls: Vec<Pull>,
    yielder: Option<BodyId>,
    /// Steps in a row in which nothing moved more than a tremble.
    quiet: u32,
    /// Steps since something last changed from outside ([`World::wake`]).
    awake: u32,
    /// How far the field ahead of the yielder may still shift this step.
    band_budget: f32,
    next_id: u64,
    at_rest: bool,
    calm: u32,
    backlog: f32,
    steps: u64,
    last_move: f32,
    settling: Settling,
}

/// Whether gravity and the pulls that line bodies up act (SETL-38).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Settling {
    /// Bodies stay where they are put: no gravity, no corner snap, no vertex
    /// attraction, no grid pull. Contacts, walls, homes, sizes and drags work
    /// as always. The default.
    #[default]
    Hold,
    /// Settle once: the world runs with every force of its [`Params`] until
    /// it rests, then holds again ([`World::settle_once`]).
    Once,
    /// Settle all the time, as a physics toy does.
    Always,
}

impl Settling {
    /// Whether the forces of [`Params`] act now.
    pub fn is_settling(self) -> bool {
        self != Settling::Hold
    }
}

impl World {
    /// An empty world inside `bounds`.
    pub fn new(bounds: Rect, params: Params) -> Self {
        let params = params.sanitized();
        Self {
            params,
            wanted: params,
            ramp: 0,
            bounds,
            hard: bounds,
            bodies: Vec::new(),
            contacts: Vec::new(),
            snaps: Vec::new(),
            snap_corners: Vec::new(),
            pulls: Vec::new(),
            yielder: None,
            quiet: 0,
            awake: 0,
            band_budget: 0.0,
            next_id: 1,
            at_rest: false,
            calm: 0,
            backlog: 0.0,
            steps: 0,
            last_move: 0.0,
            settling: Settling::Hold,
        }
    }

    /// The same world settling as `settling` says ([`World::set_settling`]).
    pub fn with_settling(mut self, settling: Settling) -> Self {
        self.settling = settling;
        self
    }

    /// Whether the world settles now ([`Settling`]; it holds by default).
    pub fn settling(&self) -> Settling {
        self.settling
    }

    /// Hold, settle once or settle all the time (SETL-38). Wakes the world
    /// when that changes anything.
    pub fn set_settling(&mut self, settling: Settling) {
        if self.settling != settling {
            self.settling = settling;
            self.wake();
        }
    }

    /// Tidy up: gravity and the pulls of [`Params`] act until the world
    /// rests, then it holds again (SETL-38). A world that settles all the
    /// time ([`Settling::Always`]) is only woken.
    pub fn settle_once(&mut self) {
        if self.settling == Settling::Always {
            self.wake();
        } else {
            self.set_settling(Settling::Once);
            self.wake();
        }
    }

    /// The parameters that were set. While a change is ramped in, the
    /// solver works with [`World::live_params`].
    // the field `params` is what the solver works with, see `live_params`
    #[allow(clippy::misnamed_getters)]
    pub fn params(&self) -> &Params {
        &self.wanted
    }

    /// The parameters the solver works with right now: those that were
    /// set, or on the way to them for [`PARAM_RAMP_STEPS`] steps after a
    /// change.
    pub fn live_params(&self) -> &Params {
        &self.params
    }

    /// Replace the parameters ([`Params::sanitized`]); wakes the world if
    /// anything changed. A change puts no energy in: forces, speeds and
    /// sizes go to their new values over [`PARAM_RAMP_STEPS`] steps, no
    /// body keeps more speed than a glide, and the world settles again.
    /// Choices (gravity's side, the walls) and the solver's clock apply
    /// at once.
    pub fn set_params(&mut self, params: Params) {
        let params = params.sanitized();
        if self.wanted == params {
            return;
        }
        self.wanted = params;
        self.params = self.params.toward(&params, 0.0);
        self.ramp = PARAM_RAMP_STEPS;
        let calm = self.params.glide_speed.min(params.glide_speed);
        for b in &mut self.bodies {
            b.vel = b.vel.clamp_length(calm);
        }
        self.wake();
    }

    /// The rectangle the walls stand on.
    pub fn bounds(&self) -> Rect {
        self.bounds
    }

    /// Move the walls; bodies left outside glide back in.
    pub fn set_bounds(&mut self, bounds: Rect) {
        if self.bounds != bounds {
            // An empty world, or one that never had walls, has nothing
            // to carry along: the hard walls stand where the new ones do.
            let fresh = self.bodies.is_empty() || self.bounds.size().max_abs() <= 0.0;
            self.bounds = bounds;
            if fresh {
                self.hard = bounds;
            }
            self.wake();
        }
    }

    /// The farthest a body's rectangle is past a closed wall, points; 0
    /// when all are inside. Dragged bodies are in the user's hand and do
    /// not count, nor do lifted ones: dropped outside, they glide in.
    pub fn max_escape(&self) -> f32 {
        let mut worst = 0.0f32;
        for b in self.bodies.iter().filter(|b| !b.dragged && !b.lifted) {
            for side in Side::ALL {
                if !self.wanted.walls.has(side) {
                    continue;
                }
                let axis = side.axis();
                // a body wider than the world is held at the low wall
                let fits = b.rect.size()[axis] <= self.hard.size()[axis];
                let out = if side.is_low() {
                    self.hard.min[axis] - b.rect.min[axis]
                } else if fits {
                    b.rect.max[axis] - self.hard.max[axis]
                } else {
                    0.0
                };
                worst = worst.max(out);
            }
        }
        worst
    }

    /// The deepest any body's core is inside another's, as a share of
    /// the way in (0: none touch, 1: two middles on one spot). Cores are
    /// what [`World::step`] never lets two bodies share; dragged bodies do
    /// not count.
    pub fn max_core_overlap(&self) -> f32 {
        let mut worst = 0.0f32;
        for (i, a) in self.bodies.iter().enumerate() {
            for b in &self.bodies[i + 1..] {
                if !a.dragged && !b.dragged {
                    worst = worst.max(core_depth(a, b).0);
                }
            }
        }
        worst
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

    /// The vertices that were pulled toward a neighbour's vertex or edge,
    /// or a wall, in the last step (SETL-12).
    pub fn pulls(&self) -> &[Pull] {
        &self.pulls
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
            jammed: Vec2::ZERO,
            room: Vec2::ZERO,
            outline: desc.outline,
            stretch_lo: Vec2::ZERO,
            stretch_hi: Vec2::ZERO,
            attract_shift: Vec2::ZERO,
            seated: false,
            caught: [[false; 2]; 2],
            faced: false,
            slide_blocked: false,
            pressed_lo: Vec2::ZERO,
            pressed_hi: Vec2::ZERO,
            yielding: false,
            pinned: false,
            pushes: desc.pushes,
            drag_target: desc.center,
            slid: 0.0,
            aloft: 0,
            net: Vec2::ZERO,
            nudged: false,
            coast: Vec2::ZERO,
            walled: Vec2::ZERO,
            shoved: Vec2::ZERO,
            landed: LANDED_STEPS,
            nest: None,
            picked_from: None,
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

    /// Put a body where it belongs, at the size it has, at once: for
    /// one that has not been shown yet (a scene seeded before its first
    /// frame). Nothing glides or grows; its home, if it has one, moves
    /// along, and it keeps no speed. A body on screen is moved with
    /// [`World::grab`] and sized with [`World::set_want`] instead.
    pub fn put(&mut self, id: BodyId, center: Vec2, size: Vec2) {
        self.change(id, |b| {
            let size = size.max(Vec2::ZERO);
            b.rect = Rect::from_center_size(center, size);
            b.prev = b.rect;
            b.want = size;
            b.natural = size;
            b.min_size = b.min_size.min(size);
            b.max_size = b.max_size.max(size);
            b.squeeze = Vec2::ZERO;
            b.stretch_lo = Vec2::ZERO;
            b.stretch_hi = Vec2::ZERO;
            b.vel = Vec2::ZERO;
            b.drag_target = center;
            if b.home.is_some() {
                b.home = Some(center);
            }
            true
        });
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

    /// Give a body a convex outline in unit coordinates of its rectangle,
    /// or with `None` make it the plain rectangle again.
    pub fn set_outline(&mut self, id: BodyId, outline: Option<Poly>) {
        self.change(id, |b| {
            let changed = b.outline != outline;
            b.outline = outline;
            changed
        });
    }

    /// Give a body's outline the shape it takes at the size the body has
    /// now, when only that size changed it (a hexagon frame keeps its faces
    /// at 60° while vertex attraction stretches it, SETL-20). Unlike
    /// [`World::set_outline`] this is no change from outside: it neither
    /// wakes a world at rest nor starts its cooling over. A body without an
    /// outline keeps none.
    pub fn follow_outline(&mut self, id: BodyId, outline: Poly) {
        if let Some(b) = self.bodies.iter_mut().find(|b| b.id == id)
            && b.outline.is_some()
        {
            b.outline = Some(outline);
        }
    }

    /// Put a new vertex on a body's outline at the point of it nearest to
    /// `at` (world coordinates); a plain rectangle gets its four corners
    /// as an outline first. Returns the new vertex's index, or `None`
    /// when there is no such body or its outline is full
    /// ([`MAX_VERTICES`]).
    pub fn insert_vertex(&mut self, id: BodyId, at: Vec2) -> Option<usize> {
        let b = self.bodies.iter_mut().find(|b| b.id == id)?;
        let mut outline = b.outline.unwrap_or_else(Poly::unit_rect);
        let (edge, on, _) = outline.fit(&b.rect).nearest(at);
        let index = outline.insert(edge, Poly::to_unit(&b.rect, on))?;
        b.outline = Some(outline);
        self.wake();
        Some(index)
    }

    /// Take a vertex off a body's outline; `false` when there is no such
    /// vertex or only a triangle is left.
    pub fn remove_vertex(&mut self, id: BodyId, index: usize) -> bool {
        let Some(b) = self.bodies.iter_mut().find(|b| b.id == id) else {
            return false;
        };
        let Some(mut outline) = b.outline else {
            return false;
        };
        if !outline.remove(index) {
            return false;
        }
        b.outline = Some(outline);
        self.wake();
        true
    }

    /// Move a vertex of a body's outline to `to` (world coordinates), or
    /// as near as it can go with the outline staying convex and inside
    /// the body's rectangle ([`Poly::move_vertex`]). Returns where it
    /// ended up, or `None` when there is no such vertex.
    pub fn move_vertex(&mut self, id: BodyId, index: usize, to: Vec2) -> Option<Vec2> {
        let b = self.bodies.iter_mut().find(|b| b.id == id)?;
        let mut outline = b.outline?;
        if index >= outline.points().len() {
            return None;
        }
        let unit = Rect::from_min_size(Vec2::ZERO, Vec2::splat(1.0));
        let at = outline.move_vertex(index, Poly::to_unit(&b.rect, to), &unit);
        let changed = b.outline != Some(outline);
        b.outline = Some(outline);
        let size = b.rect.size();
        let at = b.rect.min + Vec2::new(at.x * size.x, at.y * size.y);
        if changed {
            self.wake();
        }
        Some(at)
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
            // a pinned body stays where it is: unpin it to move it
            if b.pinned {
                return false;
            }
            let changed = !b.dragged || b.nest.is_some();
            // (picked up again on its way down: where it came from first)
            if !b.lifted || b.picked_from.is_none() {
                b.picked_from = Some(b.rect.center());
            }
            b.dragged = true;
            b.lifted = true;
            b.nest = None;
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
            // (on its way into a cell: it stops where it is)
            b.nest = None;
            if changed {
                b.landed = 0;
            }
            if rehome && changed {
                b.home = Some(b.rect.center());
            }
            changed
        });
    }

    /// Let go of a grabbed body with an outline near a free cell (SETL-28):
    /// where its middle is within [`Params::nest_range`] of a place where it
    /// lies face to face with a neighbour, or flat against gravity's wall,
    /// without lying in anything, it glides there (at a glide, over the
    /// others, which it does not push) and lands; else it is let go as by
    /// [`World::release`]. A body without an outline is let go as by
    /// [`World::release`]. Returns the cell's middle.
    pub fn release_to_cell(&mut self, id: BodyId, rehome: bool) -> Option<Vec2> {
        let cell = self
            .body(id)
            .filter(|b| b.dragged && b.outline.is_some())
            .and_then(|b| {
                let cells = self.cells_for(id, &[]);
                nest::nearest(&cells, self.params.nest_range).map(|c| b.rect.center() + c.shift)
            });
        match cell {
            Some(at) => {
                self.change(id, |b| {
                    b.nest = Some(at);
                    if rehome {
                        b.home = Some(at);
                    }
                    true
                });
                Some(at)
            }
            None => {
                self.release(id, rehome);
                None
            }
        }
    }

    /// The middles of the free cells for body `id` (SETL-28): where it lies
    /// face to face with another body, the gap apart, or flat against the
    /// closed wall gravity pulls to, in nothing and within the closed walls.
    pub fn free_cells(&self, id: BodyId) -> Vec<Vec2> {
        let Some(b) = self.body(id) else {
            return Vec::new();
        };
        let center = b.rect.center();
        self.cells_for(id, &[])
            .into_iter()
            .map(|c| center + c.shift)
            .collect()
    }

    /// The middle of the cell a pack puts body `id` in next (SETL-28): of
    /// its free cells ([`World::free_cells`]) the one nearest gravity's side,
    /// then the low end across it, among those against another body when
    /// there are any. Laid out one by one with [`World::put`], bodies with
    /// a hexagon outline nest as a honeycomb. The bodies in `later` are not
    /// there yet (laid out after this one).
    pub fn pack_cell(&self, id: BodyId, later: &[BodyId]) -> Option<Vec2> {
        let b = self.body(id)?;
        let cells = self.cells_for(id, later);
        nest::pack_pick(&b.shape(), &cells, self.params.gravity_side)
            .map(|c| b.rect.center() + c.shift)
    }

    fn cells_for(&self, id: BodyId, skip: &[BodyId]) -> Vec<nest::Cell> {
        let Some(b) = self.body(id) else {
            return Vec::new();
        };
        let others: Vec<Poly> = self
            .bodies
            .iter()
            .filter(|o| o.id != id && !skip.contains(&o.id) && (!o.dragged || o.nest.is_some()))
            .map(|o| match o.nest {
                // one on its way to a cell is as good as there
                Some(at) => o.shape().translate(at - o.rect.center()),
                None => o.shape(),
            })
            .collect();
        let room = nest::Room {
            others: &others,
            bounds: self.bounds,
            walls: self.params.walls,
            gravity: self.params.gravity_side,
            gap: self.params.gap,
        };
        nest::cells(&b.shape(), &room)
    }

    /// The bodies stuck together with `id` (SETL-22): those it touches (the
    /// gap and [`CLUSTER_SLACK`] apart or closer), what they touch, and so
    /// on, as long as none of them lies against a closed wall; `id` first.
    /// A cluster at a wall belongs to the wall (a stack under gravity is not
    /// a group), so then it is `id` alone, as it is for a lifted or unknown
    /// body. Dragging one of a group moves the group.
    pub fn cluster(&self, id: BodyId) -> Vec<BodyId> {
        let Some(start) = self.bodies.iter().position(|b| b.id == id) else {
            return Vec::new();
        };
        let alone = vec![id];
        let solid = |b: &Body| !b.lifted && !b.dragged && !b.pinned;
        if !solid(&self.bodies[start]) {
            return alone;
        }
        let p = &self.params;
        let reach = p.gap + CLUSTER_SLACK;
        let inner = Rect {
            min: self.bounds.min + Vec2::splat(reach),
            max: self.bounds.max - Vec2::splat(reach),
        };
        let at_wall = |b: &Body| {
            let r = b.shape().bounds();
            (p.walls.left && r.min.x < inner.min.x)
                || (p.walls.top && r.min.y < inner.min.y)
                || (p.walls.right && r.max.x > inner.max.x)
                || (p.walls.bottom && r.max.y > inner.max.y)
        };
        let mut members = vec![start];
        let mut next = 0;
        while next < members.len() {
            let a = &self.bodies[members[next]];
            next += 1;
            if at_wall(a) {
                return alone;
            }
            for (j, o) in self.bodies.iter().enumerate() {
                if members.contains(&j) || !solid(o) {
                    continue;
                }
                if touch(a, o, reach, Vec2::ZERO).is_some_and(|h| h.depth > 0.0) {
                    members.push(j);
                }
            }
        }
        members.into_iter().map(|i| self.bodies[i].id).collect()
    }

    /// Pin a body in place, or let it go again (SETL-27). Pinned it stays
    /// exactly where it is: the others rest against it as against a wall
    /// and flow around it, it is not grabbed ([`World::grab`] does nothing)
    /// and belongs to no group ([`World::cluster`]). Unpinned it is an
    /// ordinary body again.
    pub fn set_pinned(&mut self, id: BodyId, pinned: bool) {
        self.change(id, |b| {
            let changed = b.pinned != pinned;
            b.pinned = pinned;
            if pinned {
                b.dragged = false;
                b.lifted = false;
                b.vel = Vec2::ZERO;
            }
            changed
        });
    }

    /// Let a body push its neighbours, or not ([`Body::pushes`], SETL-40).
    pub fn set_pushes(&mut self, id: BodyId, pushes: bool) {
        self.change(id, |b| {
            let changed = b.pushes != pushes;
            b.pushes = pushes;
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
        for b in &mut self.bodies {
            b.slide_blocked = false;
        }
        self.calm = 0;
        self.quiet = 0;
        self.awake = 0;
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
    /// 0 when none overlap. Bodies with an outline are measured by it,
    /// plain rectangles by their rectangles.
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
                // As they are seen, with what vertex attraction added, and
                // as they meet: a plain rectangle meets an outline as its
                // rectangle, whatever it bulges by.
                if a.outline.is_some() || b.outline.is_some() {
                    let (pa, pb) = (a.shape_in(&a.rect), b.shape_in(&b.rect));
                    let hit = sat(&pa, &pb, |_| gap, Vec2::ZERO);
                    worst = worst.max(hit.map_or(0.0, |h| h.depth));
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
        // A change of parameters comes in over a few steps (no energy).
        if self.ramp > 0 {
            self.params = self.params.toward(&self.wanted, 1.0 / self.ramp as f32);
            self.ramp -= 1;
        }
        // Holding, the step runs without the forces that move bodies by
        // themselves (SETL-38); the parameters stay as they were set.
        let live = self.params;
        if !self.settling.is_settling() {
            self.params = live.held();
        }
        self.step_inner();
        self.params = live;
        if self.at_rest && self.settling == Settling::Once {
            self.settling = Settling::Hold;
        }
        true
    }

    fn step_inner(&mut self) {
        let p = self.params;
        let dt = p.dt.max(1e-4);
        // A wall that was moved inward does not push: what it left
        // outside is brought in by the passes below (at a glide, giving
        // way where there is no room), and the hard wall follows the
        // outermost body in, never back out, until it stands on the wall.
        for axis in Axis::BOTH {
            let free = self.bodies.iter().filter(|b| !b.dragged && !b.lifted);
            let (lo, hi) = free.fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), b| {
                (lo.min(b.rect.min[axis]), hi.max(b.rect.max[axis]))
            });
            let (wall_lo, wall_hi) = (self.bounds.min[axis], self.bounds.max[axis]);
            self.hard.min[axis] = wall_lo.min(self.hard.min[axis].max(lo));
            self.hard.max[axis] = wall_hi.max(self.hard.max[axis].min(hi));
        }
        self.contacts.clear();
        self.band_budget = p.glide_speed * dt * 0.5;
        let mut home_moved = 0.0f32;

        for b in &mut self.bodies {
            // Pushed off by several slopes at once, it still glides.
            b.slide_pull = b.slide_pull.clamp_length(SLIDE_MAX);
            b.slid = 0.0;
            b.net = Vec2::ZERO;
            b.walled = Vec2::ZERO;
            b.shoved = Vec2::ZERO;
            // (counted from a drop: one just added is not settling in)
            b.landed = if b.lifted && b.landed < LANDED_STEPS {
                0
            } else {
                b.landed.saturating_add(1)
            };
            b.nudged = false;
            b.seated = false;
            b.prev = b.rect;
            b.pressure = 0.0;
            // A bulge still changing keeps the world awake, like a moving body.
            let jammed = std::mem::replace(&mut b.jammed, Vec2::ZERO);
            home_moved = home_moved.max(resize(b, &p, dt, jammed));
        }
        // Cooling: a world that has not come to rest by itself long
        // after the last change from outside is in a cycle (a corner
        // snap against a home, a row passed back and forth). Its
        // forces and speeds fade out, and then it rests where it is.
        self.awake = self.awake.saturating_add(1);
        let time = self.awake as f32 * dt;
        let cool = (1.0 - (time - COOL_AFTER) / COOL_SECS).clamp(0.0, 1.0);
        // On the way into a cell (SETL-28): a glide a step, then let go.
        let glide = p.glide_speed * dt;
        for b in &mut self.bodies {
            if let Some(at) = b.nest {
                let to = at - b.drag_target;
                b.drag_target += to.clamp_length(glide);
                if (at - b.rect.center()).length() < 1e-3 {
                    b.nest = None;
                    b.dragged = false;
                    b.landed = 0;
                }
            }
        }
        self.pick_yielder();
        let pulled = self.attract_pass(dt, cool);
        self.snap_pass();
        self.grid_pass();
        for b in &mut self.bodies {
            // A home still drifting keeps the world awake, like a moving body.
            home_moved = home_moved.max(integrate(b, &p, dt, cool));
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
        self.eject_pass();
        // Bodies last: where both cannot hold, a wall gives, not a body.
        for _ in 0..p.iterations.max(1) {
            if self.pairs_pass(None, dt, false) <= SLOP {
                break;
            }
        }
        self.hard_pass();
        self.measure_room();
        self.land_lifted();
        // The snaps as the step leaves the bodies: from the corner where it
        // is now, by the same way to go (the overlay draws them after the
        // step; at the start of it they were one step behind, SETL-26).
        for (snap, &ca) in self.snaps.iter_mut().zip(&self.snap_corners) {
            if let Some(b) = self.bodies.iter().find(|b| b.id == snap.body) {
                let now = corner(&b.rest_rect(), ca);
                snap.to += now - snap.from;
                snap.from = now;
            }
        }
        let mut moved = home_moved;
        if self.ramp > 0 {
            // still changing: not a moment to rest in
            moved = moved.max(p.rest_eps * 2.0);
        }
        let mut moved_free = 0.0f32;
        for b in &mut self.bodies {
            let d = (b.rect.min - b.prev.min)
                .max_abs()
                .max((b.rect.max - b.prev.max).max_abs());
            moved = moved.max(d);
            if !b.dragged {
                moved_free = moved_free.max(d);
                // Sliding off a slope is a glide: it does not build up speed.
                let limit = if b.slid > 0.0 || b.nudged {
                    p.glide_speed.min(p.max_speed)
                } else {
                    p.max_speed
                };
                // The body that gives way out of a full row is pushed out
                // by a glide a step; with a glide of its own speed on top
                // it went two and more (SETL-21).
                let limit = if b.yielding {
                    limit.min(p.glide_speed * 0.5)
                } else {
                    limit
                };
                let travelled = b.rect.center() - b.prev.center() - b.attract_shift;
                // Put back onto a slope it sits on, it keeps next to no
                // speed: being put back is not travelling.
                let limit = if b.seated {
                    limit.min(SEAT_SPEED)
                } else {
                    limit
                };
                // A wall only ever takes speed away: what it moved the body
                // beyond stopping it (a body within the gap of it, or brought
                // back in from past it) is position, not speed, so the body
                // stays at the wall instead of coasting off it (user, 12 Oct
                // 2026, SETL-25).
                let mut vel = (travelled * (1.0 / dt)).clamp_length(limit);
                // (A pressed body is part of a full row or column, which a
                // wall sends on its way: SETL-11.)
                let free = b.landed < LANDED_STEPS || b.load().max_abs() <= YIELD_BULGE;
                for axis in Axis::BOTH {
                    let w = b.walled[axis];
                    if w == 0.0 || !free {
                        continue;
                    }
                    let s = w.signum();
                    let stopping = (-b.coast[axis] * s).max(0.0);
                    let excess = (w.abs() / dt - stopping).max(0.0);
                    let along = vel[axis] * s;
                    vel[axis] = s * (along - excess).max(along.min(0.0));
                }
                // Pushed out of a held body's way: as far as that went, it
                // is no speed (a glide out from under it, never two).
                for axis in Axis::BOTH {
                    let w = b.shoved[axis];
                    if w != 0.0 {
                        let s = w.signum();
                        let along = vel[axis] * s;
                        vel[axis] = s * (along - w.abs() / dt).max(along.min(0.0));
                    }
                }
                b.vel = vel;
            }
        }
        // An edge still closing in keeps the world awake, and for smaller
        // moves than a body's: the pull is a share of what is left, so
        // the last tenth of a point is slow, and rest would freeze it
        // there.
        let closing_in = pulled > p.rest_eps * ATTRACT_AWAKE;
        // Sleep: where pieces sit on each other's slopes, or an edge is
        // stopped by a neighbour, they can go on trembling by a few
        // thousandths of a point for ever, now and then just over the
        // rest threshold. After a while (`SLEEP_STEPS`) in which nothing
        // moved more than a few times that threshold, the world rests
        // all the same; only a real change wakes it.
        // One that does not push and is still looking for its place is under
        // way: it finds one or goes back within REFUSE_STEPS (SETL-40).
        if self
            .bodies
            .iter()
            .any(|b| b.lifted && !b.dragged && !b.pushes)
        {
            moved = moved.max(p.rest_eps * SLEEP_COARSE * 2.0);
        }
        self.quiet = if moved > p.rest_eps * SLEEP_COARSE {
            0
        } else {
            self.quiet.saturating_add(1)
        };
        if closing_in && self.quiet < SLEEP_STEPS {
            moved = moved.max(p.rest_eps * 2.0);
        }
        self.last_move = moved_free;
        self.steps += 1;
        self.calm = if moved <= p.rest_eps {
            self.calm + 1
        } else {
            0
        };
        if self.calm >= p.rest_steps.max(1) || self.quiet >= SLEEP_STEPS || cool <= 0.0 {
            self.at_rest = true;
            self.last_move = 0.0;
            for b in &mut self.bodies {
                b.vel = Vec2::ZERO;
            }
        }
    }

    /// The body that gives way this step ([`Body::yielding`]).
    fn pick_yielder(&mut self) {
        let bulge = |b: &Body| {
            if b.dragged || b.pinned {
                0.0
            } else {
                b.bulge.x.max(b.bulge.y)
            }
        };
        let most = self
            .bodies
            .iter()
            .map(bulge)
            .filter(|&b| b > YIELD_BULGE)
            .fold(0.0, f32::max);
        // Of those bulged near the most, the one nearest the side gravity
        // pulls to goes first (the newest on a tie): a full field gives way
        // from the top down, so each row's overflow drops into the row
        // after it and the rows below make room, instead of a lower row's
        // overflow being carried down again by the rows above.
        let toward = self.params.gravity_side.map_or(Vec2::ZERO, |s| s.dir());
        let mut best: Option<&Body> = None;
        for b in self
            .bodies
            .iter()
            .filter(|b| bulge(b) > YIELD_BULGE && bulge(b) * 2.0 >= most)
        {
            let ahead = |b: &Body| (b.rect.center().dot(toward) / YIELD_ROW).round();
            if best.is_none_or(|w| ahead(b) >= ahead(w)) {
                best = Some(b);
            }
        }
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
    /// The corners corner snapping measures from, in world points: those of
    /// [`Body::rest_rect`] of every body that snaps (no outline: a hexagon
    /// lines up by its faces, SETL-20; not held). What the overlay draws the
    /// snap range around.
    pub fn snap_anchors(&self) -> Vec<(BodyId, Vec2)> {
        self.bodies
            .iter()
            .filter(|b| !b.dragged && b.outline.is_none())
            .flat_map(|b| {
                let r = b.rest_rect();
                CORNERS.map(|c| (b.id, corner(&r, c)))
            })
            .collect()
    }

    fn snap_pass(&mut self) {
        self.snaps.clear();
        self.snap_corners.clear();
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
        // Corners as they would be without vertex attraction (SETL-12):
        // that one only resizes, and where a body goes must not depend on
        // which of its edges it has stretched.
        let rest: Vec<Rect> = self.bodies.iter().map(Body::rest_rect).collect();
        let hold: Vec<f32> = self
            .bodies
            .iter()
            .map(|b| hold(b, p.bulge_rest, p.bulge_max))
            .collect();
        for i in 0..self.bodies.len() {
            let a = &self.bodies[i];
            // The corners of an outline's rectangle are not its vertices.
            if a.dragged || a.pinned || hold[i] <= 0.0 || a.outline.is_some() {
                continue;
            }
            let mut winner: Option<(Snap, Vec2, (bool, bool))> = None;
            for ca in CORNERS {
                let from = corner(&rest[i], ca);
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
                    // Nor to one pushed aside faster than settling goes,
                    // which is on its way somewhere. The snap takes on
                    // the other's speed, and a body dropped next to a
                    // stack it pushed out ran off after it, away from the
                    // wall it was dropped at (user, 12 Oct 2026).
                    if j == i || o.dragged || o.outline.is_some() || o.vel.length() > SNAP_MOVING {
                        continue;
                    }
                    for cb in CORNERS {
                        // The same corner of both would put one body on the other.
                        if cb == ca {
                            continue;
                        }
                        let mut error = corner(&rest[j], cb) - from;
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
                    if winner.is_none_or(|(w, ..)| grip > w.grip) {
                        let pull = (error * p.snap_stiffness + (vel - a.vel) * damping) * grip;
                        // drawn from where it is measured (SETL-26)
                        let at = from;
                        let snap = Snap {
                            body: a.id,
                            from: at,
                            to: at + error,
                            grip,
                        };
                        winner = Some((snap, pull, ca));
                    }
                }
            }
            if let Some((snap, pull, ca)) = winner {
                self.snaps.push(snap);
                self.snap_corners.push(ca);
                self.bodies[i].snap_pull = pull;
            }
        }
    }

    /// Vertex attraction (SETL-12): the vertices of a free body pull on
    /// what is within [`Params::attract_range`], and the pull moves the
    /// edges that meet at the vertex, not the body: the body is resized,
    /// within its minimum and maximum size, at
    /// [`Params::attract_strength`]. An edge is pulled
    ///
    /// - into one line with a neighbour's edge that ends near one of its
    ///   vertices (two panels side by side: their bottoms), following the
    ///   mean of its two vertices' pulls, or else
    /// - flush against what it faces (a body's edge or a wall, the gap
    ///   kept), which only ever grows it into free room.
    ///
    /// An edge never goes more than half the way in a step, so two bodies
    /// pulling on each other meet in the middle and one that cannot give
    /// leaves the whole way to the other.
    ///
    /// Every pull that is shown closes: a captured edge goes all the way
    /// (it is then *locked*, [`Pull::locked`], and held until the two are
    /// clearly apart again), whether or not the edge across from it has
    /// anything to line up with. Only the edge on gravity's side is left
    /// alone: the body rests on it (or hangs from its home by it), and
    /// resizing there would only move the body.
    ///
    /// Outlines are the user's: a body with one is resized as a whole,
    /// its outline kept as it is in its rectangle; no vertex is ever
    /// moved by the solver.
    ///
    /// What is in range is decided on where the bodies would be without
    /// any stretch, so a pull does not keep itself alive: when the content
    /// changes and the sizes are far apart again, the edges let go and
    /// return. An edge under pressure does not grow, and gives back what
    /// it grew.
    ///
    /// Returns the farthest it moved an edge.
    fn attract_pass(&mut self, dt: f32, cool: f32) -> f32 {
        let mut most_moved = 0.0f32;
        self.pulls.clear();
        let p = self.params;
        let range = p.attract_range;
        let on = range > 0.0 && p.attract_strength > 0.0;
        let now: Vec<Rect> = self.bodies.iter().map(|b| b.rect).collect();
        let rest: Vec<Rect> = self.bodies.iter().map(Body::rest_rect).collect();
        // Dragged, lifted and pressed (bulged) bodies are on their way
        // somewhere: they neither pull nor are pulled to.
        let free: Vec<bool> = self
            .bodies
            .iter()
            .map(|b| !b.dragged && !b.lifted && b.bulge.max_abs() <= YIELD_BULGE)
            .collect();
        let inner = Rect {
            min: self.bounds.min + Vec2::splat(p.gap),
            max: self.bounds.max - Vec2::splat(p.gap),
        };
        let toward = p.gravity_side.filter(|_| p.gravity > 0.0);
        // How far an edge of body `i` is from the wall it faces and from
        // the nearest body it faces (the gap kept), outward.
        let room = |rects: &[Rect], i: usize, axis: Axis, high: bool| {
            let (r, t) = (&rects[i], axis.other());
            let wall = if !p.walls.has(axis_side(axis, high)) {
                f32::INFINITY
            } else if high {
                inner.max[axis] - r.max[axis]
            } else {
                r.min[axis] - inner.min[axis]
            };
            let mut body = f32::INFINITY;
            for (j, o) in rects.iter().enumerate() {
                if j == i || !free[j] || o.min[t] >= r.max[t] || o.max[t] <= r.min[t] {
                    continue;
                }
                if high && o.center()[axis] > r.center()[axis] {
                    body = body.min(o.min[axis] - p.gap - r.max[axis]);
                } else if !high && o.center()[axis] < r.center()[axis] {
                    body = body.min(r.min[axis] - o.max[axis] - p.gap);
                }
            }
            (wall, body)
        };
        // A rigid outline that lies against faces of two directions (in a
        // notch of a honeycomb, SETL-28) cannot slide along either: each
        // slide would take it into the other face. It is wedged, and no slide
        // is asked of it (they were drawn as pulls that never got done).
        let wedged: Vec<bool> = (0..self.bodies.len())
            .map(|i| {
                let a = &self.bodies[i];
                if !on
                    || !free[i]
                    || a.outline.is_none()
                    || !Axis::BOTH.into_iter().all(|x| a.rigid(x))
                {
                    return false;
                }
                let me = a.shape_in(&rest[i]);
                let mut normals: Vec<Vec2> = Vec::new();
                for (j, o) in self.bodies.iter().enumerate() {
                    if j == i || !free[j] || o.outline.is_none() {
                        continue;
                    }
                    let other = o.shape_in(&rest[j]);
                    normals.extend(
                        face_pairs(&me, &other, p.gap, range)
                            .into_iter()
                            .map(|(.., n)| n),
                    );
                }
                normals.iter().enumerate().any(|(k, n)| {
                    normals[k + 1..]
                        .iter()
                        .any(|m| (n.x * m.y - n.y * m.x).abs() > 0.3)
                })
            })
            .collect();
        for i in 0..self.bodies.len() {
            // Per edge (axis, high end): how far it is pulled along the
            // axis and how firmly, and whether that is into line with a
            // neighbour's edge.
            let mut edges: [[Option<(f32, f32)>; 2]; 2] = [[None; 2]; 2];
            // Along a face of a neighbour, to line the vertices up (SETL-20).
            let (mut slide, mut nearest) = (Vec2::ZERO, f32::INFINITY);
            // the neighbours it lies face to face with
            let mut partners: Vec<usize> = Vec::new();
            // A slide that would have run into something (see below) waits
            // until something changes.
            let blocked = self.bodies[i].slide_blocked || wedged[i];
            let homed = self.bodies[i].home.is_some() && p.home_stiffness > 0.0;
            let mut lined = [[false; 2]; 2];
            let mut held = [[false; 2]; 2];
            if on && free[i] && !self.bodies[i].pinned {
                // Whether nothing on gravity's side holds the body up.
                let mut hangs = false;
                if let Some(side) = toward {
                    let (axis, high) = (side.axis(), !side.is_low());
                    held[axis as usize][high as usize] = true;
                    let (wall, body) = room(&now, i, axis, high);
                    hangs = wall.min(body) > RESTS_SLACK;
                }
                // Faces of outlines that lie against each other (SETL-20):
                // the vertices at the ends of a face meet those across, the
                // gap away. What both ends are off alike along the face is
                // a slide: the body moves along the face (the nearest
                // neighbour's, by the mean of its ends). What is left is a
                // difference in length, which no slide changes: each of the
                // two takes half of it, through its height for a slanted
                // face and its width for a flat one (a face grows with the
                // size across it), on the edges that are free to move.
                let mut faced = [[(0.0f32, 0.0f32); 2]; 2];
                // the neighbour whose face it slides along, and the pulls to draw
                let mut slider: Option<usize> = None;
                let mut pending = Vec::new();
                let shape_rest = self.bodies[i].shape_in(&rest[i]);
                let shape_now = self.bodies[i].shape_in(&now[i]);
                for j in 0..self.bodies.len() {
                    let (a, o) = (&self.bodies[i], &self.bodies[j]);
                    if j == i || !free[j] || a.outline.is_none() || o.outline.is_none() {
                        continue;
                    }
                    let other_rest = o.shape_in(&rest[j]);
                    let other_now = o.shape_in(&now[j]);
                    let keeps = range * ATTRACT_KEEPS;
                    let reach = if self.bodies[i].faced { keeps } else { range };
                    let pairs: Vec<_> = face_pairs(&shape_rest, &other_rest, p.gap, reach)
                        .into_iter()
                        .map(|(ia, ib, n)| {
                            let from = shape_now.points()[ia];
                            let to = other_now.points()[ib] + n * p.gap;
                            (ia, ib, n, from, to)
                        })
                        .collect();
                    if pairs.is_empty() {
                        continue;
                    }
                    partners.push(j);
                    // What both ends are off alike; on a body that cannot
                    // take a difference in length, the end nearer to its
                    // place: that vertex meets the one across, as a
                    // rectangle's corner snaps (round 4), instead of both
                    // ends staying off by half the difference for ever.
                    let mut along = Vec2::ZERO;
                    let mut nearest_end: Option<Vec2> = None;
                    let mut far = 0.0f32;
                    for &(_, _, n, from, to) in &pairs {
                        let error = to - from;
                        let slide = error - n * error.dot(n);
                        along += slide * (1.0 / pairs.len() as f32);
                        if nearest_end.is_none_or(|e| slide.length() < e.length()) {
                            nearest_end = Some(slide);
                        }
                        far = far.max(error.length());
                    }
                    let a = &self.bodies[i];
                    let rigid = Axis::BOTH.into_iter().all(|axis| a.rigid(axis));
                    if rigid {
                        along = nearest_end.unwrap_or(along);
                    }
                    // One face slides the body: the nearest. Toward gravity's
                    // side it is the other one's to slide: this one lies on
                    // the wall or on what is under it, and would only creep.
                    // Nor does a body held by its home slide: the home is
                    // where the user put it, and a slide that took the home
                    // along would walk it off.
                    let mine = far < nearest;
                    let sinks = toward.is_some_and(|side| {
                        let (axis, sign) = (side.axis(), if side.is_low() { -1.0 } else { 1.0 });
                        (now[i].center()[axis] - now[j].center()[axis]) * sign > 0.0
                    });
                    // the other one slides (it lies farther from gravity's side), unless it cannot:
                    // wedged, its slide blocked, or held by its home
                    let o = &self.bodies[j];
                    let theirs = sinks
                        && !wedged[j]
                        && !o.slide_blocked
                        && !(o.home.is_some() && p.home_stiffness > 0.0);
                    if mine {
                        nearest = far;
                        slider = Some(j);
                        slide = if sinks || blocked || homed {
                            Vec2::ZERO
                        } else {
                            along
                        };
                    }
                    if let [(a0, b0, ..), (a1, b1, ..)] = pairs[..] {
                        let (pa, pb) = (shape_now.points(), other_now.points());
                        let face = pa[a1] - pa[a0];
                        let len_a = face.length();
                        let len_b = (pb[b1] - pb[b0]).length();
                        if len_a > 1.0 {
                            let axis = if face.y.abs() < 0.1 * len_a {
                                Axis::X
                            } else {
                                Axis::Y
                            };
                            let x = axis as usize;
                            let grow = (len_b - len_a) * 0.5 * now[i].size()[axis] / len_a;
                            let free = [!held[x][0], !held[x][1]];
                            let share = match free {
                                [true, true] => 0.5,
                                [false, false] => 0.0,
                                _ => 1.0,
                            };
                            for h in 0..2 {
                                if free[h] && share > 0.0 {
                                    let outward = if h == 1 { 1.0 } else { -1.0 };
                                    faced[x][h].0 += grow * share * outward;
                                    faced[x][h].1 += 1.0;
                                }
                            }
                        }
                    }
                    // drawn once it is known which face the body slides along
                    for (.., from, to) in pairs {
                        pending.push((j, from, to, along, theirs, rigid));
                    }
                }
                // Shown with the slide where it is done: by this body along the
                // face it slides on, or by the one nested under it.
                for (j, from, to, along, theirs, rigid) in pending {
                    // (by this body only where its slide is not left to the
                    // other one or held back: a pull nobody does never closes)
                    let done = (slider == Some(j) && slide != Vec2::ZERO) || theirs;
                    let mut to = if done { to } else { to - along };
                    // A rigid body's vertex is pulled as far as the slide takes
                    // it, no farther: the rest along the face is a difference in
                    // length it cannot take, and across it, faces in range but
                    // farther apart than the gap, a way nothing here moves it
                    // (contacts and gravity do, SETL-28).
                    if rigid {
                        to = from + if done { along } else { Vec2::ZERO };
                    }
                    self.pulls.push(Pull {
                        body: self.bodies[i].id,
                        from,
                        to,
                        grip: 1.0,
                        locked: (to - from).max_abs() < Pull::LOCKED,
                    });
                }
                for axis in Axis::BOTH {
                    for high in [false, true] {
                        // A rigid outline's box edges go nowhere: its faces
                        // line up by sliding (above), and a pull of its box
                        // would only be drawn, never done.
                        if held[axis as usize][high as usize]
                            || (self.bodies[i].outline.is_some() && self.bodies[i].rigid(axis))
                        {
                            continue;
                        }
                        // An edge that is pulled stays so a little beyond
                        // the range, so one at its very edge does not
                        // flicker.
                        let reach = if self.bodies[i].caught[axis as usize][high as usize] {
                            range * ATTRACT_KEEPS
                        } else {
                            range
                        };
                        let outward = if high { 1.0 } else { -1.0 };
                        let (wall, body) = room(&now, i, axis, high);
                        // Into line with a neighbour's edge on the same
                        // side that ends where this one does, across the
                        // gap: (errors, grips, vertices pulled).
                        let mut line = (0.0f32, 0.0f32, 0u32);
                        for t_high in [false, true] {
                            let ca = corner_of(axis, high, t_high);
                            let cb = corner_of(axis, high, !t_high);
                            let v_rest = corner(&rest[i], ca);
                            let mut best: Option<(f32, f32)> = None;
                            for j in 0..self.bodies.len() {
                                if j == i || !free[j] {
                                    continue;
                                }
                                let to = gapped(corner(&rest[j], cb), ca, cb, p.gap);
                                let dist = (to - v_rest).length();
                                if dist < reach && best.is_none_or(|b| dist < b.0) {
                                    let to = gapped(corner(&now[j], cb), ca, cb, p.gap);
                                    best = Some((dist, (to - corner(&now[i], ca))[axis]));
                                }
                            }
                            if let Some((_, error)) = best {
                                line = (line.0 + error, line.1 + 1.0, line.2 + 1);
                            }
                        }
                        let (num, den) = faced[axis as usize][high as usize];
                        if den > 0.0 {
                            line = (line.0 + num / den, line.1 + 1.0, line.2 + 1);
                        }
                        let edge = &mut edges[axis as usize][high as usize];
                        if line.2 > 0 {
                            let n = line.2 as f32;
                            // A wall does not give way; nor, for a body
                            // that hangs from its home, does what lies
                            // under it: growing into that would only push
                            // the body itself back.
                            let mut most = wall.max(0.0);
                            if hangs && toward == Some(axis_side(axis, !high)) {
                                most = most.min(body.max(0.0));
                            }
                            let out = (line.0 / n * outward).min(most);
                            *edge = Some((out * outward, line.1 / n));
                            lined[axis as usize][high as usize] = true;
                            continue;
                        }
                        // Flush against what it faces: only ever grows the
                        // body into free room. Where it is in the way,
                        // contacts move it, and the pull just gives back
                        // what it grew.
                        let (wall_rest, body_rest) = room(&rest, i, axis, high);
                        let dist = wall_rest.min(body_rest).abs();
                        if dist < reach {
                            let b = &self.bodies[i];
                            let grown = if high {
                                b.stretch_hi[axis]
                            } else {
                                b.stretch_lo[axis]
                            };
                            let out = wall.min(body).max(-grown.max(0.0));
                            *edge = Some((out * outward, 1.0));
                        }
                    }
                }
                for ca in CORNERS {
                    let from = corner(&now[i], ca);
                    let mut pull = Pull {
                        body: self.bodies[i].id,
                        from,
                        to: from,
                        grip: 0.0,
                        locked: false,
                    };
                    for axis in Axis::BOTH {
                        if let Some((error, grip)) =
                            edges[axis as usize][corner_high(ca, axis) as usize]
                        {
                            pull.to[axis] += error;
                            pull.grip = pull.grip.max(grip);
                        }
                    }
                    if pull.grip > 0.0 {
                        pull.locked = (pull.to - pull.from).max_abs() < Pull::LOCKED;
                        self.pulls.push(pull);
                    }
                }
            }
            // The slide along a face: a share of the way per step, as the
            // edges go, and no more than a glide; it fades out with the
            // world's cooling, as the forces do. Only into free room: a
            // slide that would take the body deeper into anything (the gap
            // kept) or past a closed wall is blocked until something
            // changes. Like a resize it gives the body no speed
            // (`attract_shift`).
            // An outline's edge does not grow into anything but the faces it
            // lines up with (the gap kept): it would press what lies against
            // it, which gives way and comes back, for ever (SETL-20).
            let mut crowded = [[false; 2]; 2];
            if self.bodies[i].outline.is_some() {
                let a = &self.bodies[i];
                for axis in Axis::BOTH {
                    for high in [false, true] {
                        let mut grown = a.rect;
                        if high {
                            grown.max[axis] += 0.5;
                        } else {
                            grown.min[axis] -= 0.5;
                        }
                        let depth = |r: &Rect, o: &Body| {
                            outline_hit(a, r, o, &o.rect, p.gap, Vec2::ZERO)
                                .map_or(0.0, |h| h.depth)
                        };
                        crowded[axis as usize][high as usize] =
                            self.bodies.iter().enumerate().any(|(j, o)| {
                                j != i
                                    && !o.lifted
                                    && !partners.contains(&j)
                                    && depth(&grown, o) > depth(&a.rect, o) + 0.01
                            });
                    }
                }
            }
            let mut slid = Vec2::ZERO;
            if slide != Vec2::ZERO && cool > 0.0 {
                let d = if slide.length() < ATTRACT_LANDS {
                    slide
                } else {
                    (slide * ((p.attract_strength * dt).min(0.5) * cool))
                        .clamp_length(p.glide_speed * dt)
                };
                let a = &self.bodies[i];
                let moved = a.rect.translate(d);
                let depth = |r: &Rect, o: &Body| {
                    outline_hit(a, r, o, &o.rect, p.gap, Vec2::ZERO).map_or(0.0, |h| h.depth)
                };
                // (along the faces it lines up with it goes by them, a hair
                // in or out as they meet: the contacts' to settle; but not
                // into a face of one of them that the slide runs across,
                // which the contacts would push off at a seat's speed, and
                // what lies beyond with it, SETL-36)
                let into_body = self.bodies.iter().enumerate().any(|(j, o)| {
                    let hair = if partners.contains(&j) {
                        0.01 + PARTNER_HAIR * d.length()
                    } else {
                        0.01
                    };
                    j != i && !o.lifted && depth(&moved, o) > depth(&a.rect, o) + hair
                });
                let past = |side: Side| {
                    let out = |r: &Rect| match side {
                        Side::Left => inner.min.x - r.min.x,
                        Side::Right => r.max.x - inner.max.x,
                        Side::Top => inner.min.y - r.min.y,
                        Side::Bottom => r.max.y - inner.max.y,
                    };
                    p.walls.has(side) && out(&moved) > out(&a.rect).max(0.0) + 0.01
                };
                let into_wall = [Side::Left, Side::Right, Side::Top, Side::Bottom]
                    .into_iter()
                    .any(past);
                // Nor does a slide take a body off what holds it on gravity's
                // side: away from gravity along a slanted face, a hexagon that
                // also hangs from another face (or lies on the wall) leaves
                // it, and gravity and the contacts bring it back with a seat's
                // speed, pushing what lies beyond (when room opens under it,
                // SETL-34). Along the face it slides on it stays on that one.
                let lifts = toward.is_some_and(|side| {
                    if d.dot(side.dir()) > -SLOP * 0.1 {
                        return false;
                    }
                    let reach = p.gap + LIFT_REACH;
                    let held = |r: &Rect, o: &Body| {
                        outline_hit(a, r, o, &o.rect, reach, Vec2::ZERO).map_or(0.0, |h| h.depth)
                    };
                    let (axis, sign) = (side.axis(), side.dir()[side.axis()]);
                    let ahead = |r: &Rect| r.center()[axis] * sign;
                    let leaves = self.bodies.iter().enumerate().any(|(j, o)| {
                        j != i
                            && !o.lifted
                            && ahead(&o.rect) > ahead(&a.rect)
                            && held(&a.rect, o) > 0.0
                            && held(&moved, o) < held(&a.rect, o) - 0.01 - PARTNER_HAIR * d.length()
                    });
                    let wall = |r: &Rect| match side {
                        Side::Left => r.min.x - inner.min.x,
                        Side::Right => inner.max.x - r.max.x,
                        Side::Top => r.min.y - inner.min.y,
                        Side::Bottom => inner.max.y - r.max.y,
                    };
                    leaves || (p.walls.has(side) && wall(&a.rect) <= LIFT_REACH)
                });
                if into_body || into_wall || lifts {
                    self.bodies[i].slide_blocked = true;
                } else {
                    slid = d;
                }
            }
            let b = &mut self.bodies[i];
            b.caught = edges.map(|axis| axis.map(|edge| edge.is_some()));
            b.faced = nearest.is_finite();
            let mut shift = Vec2::ZERO;
            if slid != Vec2::ZERO {
                b.rect = b.rect.translate(slid);
                shift += slid;
                most_moved = most_moved.max(slid.max_abs());
            }
            for axis in Axis::BOTH {
                // a pinned body keeps its size as it is
                if b.pinned {
                    continue;
                }
                // Pressed along this axis there is no room to grow into.
                let load = if p.free_axis() == Some(axis) {
                    0.0
                } else {
                    b.load()[axis]
                };
                let calm = if load <= 0.0 {
                    1.0
                } else {
                    (1.0 - load / p.bulge_from.max(1e-3)).clamp(0.0, 1.0)
                };
                // A pull is a share of the distance per second and nothing
                // else: at a steady speed the edges of a packed field
                // would hunt for ever, each one's last points moving its
                // neighbours. Never more than half the distance in a step,
                // so two edges closing in on each other meet.
                let pace = |d: f32| d * (p.attract_strength * dt).min(0.5);
                let stretch = [b.stretch_lo[axis], b.stretch_hi[axis]];
                // Where the stretch has each edge, along the axis.
                let at = [-stretch[0], stretch[1]];
                let pulls = edges[axis as usize].map(|e| e.unwrap_or((0.0, 0.0)));
                // How far each edge is to go along the axis ([low, high]):
                // to where it is pulled as far as the pull grips, and back
                // to no stretch as far as it does not.
                let mut go = [0.0f32; 2];
                for e in 0..2 {
                    let (want, grip) = pulls[e];
                    go[e] = pace(want * grip - (1.0 - grip) * at[e]);
                    // Letting go lands on 0 once it is nearly there.
                    if grip == 0.0 && (at[e] + go[e]).abs() < ATTRACT_LANDS {
                        go[e] = -at[e];
                    }
                }
                for (e, go) in go.iter_mut().enumerate() {
                    let outward = if e == 0 { -1.0 } else { 1.0 };
                    let s = stretch[e];
                    let mut d = *go * outward;
                    if d > 0.0 && crowded[axis as usize][e] {
                        d = 0.0;
                    }
                    if d > 0.0 {
                        d *= calm;
                    }
                    if s + d > 0.0 && calm < 1.0 {
                        d -= ((1.0 - calm) * pace(s).abs()).min(s + d);
                    }
                    // Grown flush against something that now pushes back
                    // (it moved, or the body was pressed onto it): the room
                    // is gone, the growth is given back by as much. So is an
                    // outline's growth to line faces up (SETL-20): what it
                    // presses is not the face it lines up with.
                    if !lined[axis as usize][e] || b.outline.is_some() {
                        let pushed = if e == 0 {
                            b.pressed_lo[axis]
                        } else {
                            b.pressed_hi[axis]
                        };
                        d -= pushed.min((s + d).max(0.0));
                    }
                    let limit = range.max(s.abs());
                    let size = b.rect.size()[axis];
                    let smallest = b.min_size[axis].min(size);
                    let largest = b.max_size[axis].max(size);
                    // Exact where no limit binds, so a stretch let go ends at 0.
                    let mut to = (s + d).clamp(-limit, limit);
                    if size + (to - s) < smallest {
                        to = s + (smallest - size);
                    } else if size + (to - s) > largest {
                        to = s + (largest - size);
                    }
                    let d = to - s;
                    if d == 0.0 {
                        continue;
                    }
                    most_moved = most_moved.max(d.abs());
                    if e == 0 {
                        b.stretch_lo[axis] = to;
                        b.rect.min[axis] -= d;
                    } else {
                        b.stretch_hi[axis] = to;
                        b.rect.max[axis] += d;
                    }
                    shift[axis] += d * 0.5 * outward;
                }
            }
            // The home is of the middle: it goes along, so the other edges
            // stay where they are.
            b.attract_shift = shift;
            b.pressed_lo = Vec2::ZERO;
            b.pressed_hi = Vec2::ZERO;
            if let Some(home) = &mut b.home {
                *home += shift;
            }
        }
        most_moved
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
                // (a held body may lie over a pinned one, which stays)
                // (one gliding into a free cell goes over the others)
                if a.dragged == b.dragged
                    || a.pinned
                    || b.pinned
                    || a.nest.is_some()
                    || b.nest.is_some()
                {
                    continue;
                }
                // one that does not push floats over the others (SETL-40)
                if !(if a.dragged { a.pushes } else { b.pushes }) {
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
                // What the held body pushes is part of the glide a step that
                // lifted bodies push all together (SETL-21).
                let push = hit.normal * hit.depth.min(glide);
                // It is a place to go, not speed: kept as speed, the body
                // coasted a glide and was pushed another on top, 38 points
                // a frame (SETL-25).
                if a.dragged {
                    let d = spend_net(b, push, glide);
                    b.shift(d);
                    b.shoved += d;
                } else {
                    let d = spend_net(a, -push, glide);
                    a.shift(d);
                    a.shoved += d;
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
            if b.dragged || b.pinned {
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
                if side.is_low() {
                    b.pressed_lo[axis] += depth;
                } else {
                    b.pressed_hi[axis] += depth;
                }
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
                if let Some(share) = squeeze.filter(|_| Some(axis) != self.params.free_axis()) {
                    squeezed = (depth * share / (1.0 + share)).min(b.give(axis));
                    let felt = share.max(JAM_SHARE);
                    let jam = (depth * felt / (1.0 + felt) - squeezed).max(0.0);
                    b.jam[axis] += jam;
                    b.jammed[axis] += jam;
                }
                b.squeeze[axis] += squeezed;
                // What the wall takes back counts against what neighbours
                // pushed it by (SETL-17): held between the two it has not
                // moved, and a neighbour can push it again.
                let back = depth - squeezed * 0.5;
                b.walled[axis] -= side.dir()[axis] * back;
                if side.is_low() {
                    b.rect.min[axis] += depth;
                    b.rect.max[axis] += depth - squeezed;
                    b.net[axis] += back;
                } else {
                    b.rect.max[axis] -= depth;
                    b.rect.min[axis] -= depth - squeezed;
                    b.net[axis] -= back;
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
        let free_axis = self.params.free_axis();
        let (hard, walls) = (self.hard, self.params.walls);
        let push_out = self.params.push_out;
        let away_axis = prefer.main_axis();
        let mut away = Vec2::ZERO;
        away[away_axis] = prefer[away_axis].signum();
        // The field ahead of the yielder: how deep it is in it, and where
        // the row it runs into begins.
        let mut band: Option<(f32, f32)> = None;
        let mut worst = 0.0f32;
        for i in 0..self.bodies.len() {
            for j in i + 1..self.bodies.len() {
                let (a, b) = pair_mut(&mut self.bodies, i, j);
                if a.dragged || b.dragged || (a.pinned && b.pinned) {
                    continue;
                }
                let Some(hit) = touch(a, b, gap, prefer) else {
                    continue;
                };
                let outlined = a.outline.is_some() || b.outline.is_some();
                let before = if outlined {
                    outline_hit(a, &a.prev, b, &b.prev, gap, tie_dir(a, b, prefer))
                } else {
                    let (ab, bb) = pair_bulges(a, b);
                    touch_at(&a.prev, ab, &b.prev, bb, gap, tie_dir(a, b, prefer))
                };
                let allowed = before.map_or(0.0, |before| (before.depth - glide).max(0.0));
                let depth = hit.depth - allowed;
                if depth <= 0.0 {
                    continue;
                }
                let axis = hit.normal.main_axis();
                worst = worst.max(depth);
                a.pressure += depth;
                b.pressure += depth;
                // The yielder, pressed out of a full row, runs into the row
                // ahead: that row and everything beyond it shift on
                // together, instead of the yielder wedging into a row that
                // is full as well (SETL-11).
                if self.band_budget > 0.0 && axis == away_axis && a.yielding != b.yielding {
                    let (y, o, ahead) = if a.yielding {
                        (&*a, &*b, hit.normal)
                    } else {
                        (&*b, &*a, -hit.normal)
                    };
                    if ahead.dot(away) > 0.5 && y.bulge[away_axis.other()] > YIELD_BULGE {
                        let front =
                            leading_edge(&o.rect, away) - ROW_SLACK * o.rect.size()[away_axis];
                        band = Some(
                            band.map_or((depth, front), |(d, f)| (d.max(depth), f.min(front))),
                        );
                        continue;
                    }
                }
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
                    // (what a body that does not push lands on or grows into
                    // stays as a wall: no push along it, SETL-40)
                    if !moves_alone(b, a) {
                        a.slide_pull -= push;
                    }
                    if !moves_alone(a, b) {
                        b.slide_pull += push;
                    }
                }
                let a_low = hit.normal[axis] > 0.0;
                let (lo, hi) = if a_low { (a, b) } else { (b, a) };
                lo.pressed_hi[axis] += depth;
                hi.pressed_lo[axis] += depth;
                let (mut s_lo, mut s_hi) = (0.0, 0.0);
                if let Some(share) = squeeze.filter(|_| Some(axis) != free_axis) {
                    let w_lo = if lo.give(axis) > 0.0 { share } else { 0.0 };
                    let w_hi = if hi.give(axis) > 0.0 { share } else { 0.0 };
                    let total = 2.0 + w_lo + w_hi;
                    s_lo = (depth * w_lo / total).min(lo.give(axis));
                    s_hi = (depth * w_hi / total).min(hi.give(axis));
                    let felt = share.max(JAM_SHARE);
                    let each = depth * felt / (2.0 + 2.0 * felt);
                    let (j_lo, j_hi) = ((each - s_lo).max(0.0), (each - s_hi).max(0.0));
                    lo.jam[axis] += j_lo;
                    hi.jam[axis] += j_hi;
                    lo.jammed[axis] += j_lo;
                    hi.jammed[axis] += j_hi;
                }
                let each = hit.normal * ((depth - s_lo - s_hi) * 0.5);
                lo.squeeze[axis] += s_lo;
                lo.rect.max[axis] -= s_lo;
                hi.squeeze[axis] += s_hi;
                hi.rect.min[axis] += s_hi;
                // `normal` points from `a` to `b`. Along a slope a body
                // moves sideways at most half a glide per step; past that the
                // push is straight, so making room by sliding is a glide.
                let (a, b) = if a_low { (lo, hi) } else { (hi, lo) };
                let mut sideways = each;
                sideways[axis] = 0.0;
                let mut straight = each - sideways;
                let mut seated = false;
                let mut on_slope = false;
                // Seated (SETL-15): an outline pressed onto a slope by
                // gravity does not slide along it while the slope is no
                // steeper than the friction holds. The two part along
                // gravity's axis alone, so each stays where it sat down.
                if outlined && let Some(side) = self.params.gravity_side {
                    let g = side.axis();
                    let (along, across) = (hit.normal[g].abs(), hit.normal[g.other()].abs());
                    if along > 1e-3 && across <= self.params.friction * along {
                        let mut up = Vec2::ZERO;
                        up[g] = each[g] / (along * along);
                        (straight, sideways) = (up, Vec2::ZERO);
                        seated = true;
                        on_slope = across > 1e-3;
                    }
                }
                let (slide_a, slide_b) = (
                    spend_slide(a, sideways, glide * 0.5),
                    spend_slide(b, sideways, glide * 0.5),
                );
                // An outline lies on several others at once (they nest):
                // all their pushes together move it no more than a glide
                // per step. That is the distance it ends up moved by, not
                // the way it went: pushed back and forth between two
                // neighbours over the passes it has gone nowhere, and
                // must still be free to give way (SETL-17).
                // (Put back onto a slope it travels twice as far as the
                // overlap is deep: half a glide, as a slide.)
                // Flat on flat nothing is doubled: pushed by a neighbour
                // that parts at a glide, it has to keep up with it.
                let most = if on_slope { glide * 0.5 } else { glide };
                // So does a body that lands lifted on a stack: what it
                // pushes in, by every pass and pair together, goes at a
                // glide (SETL-21).
                let (straight_a, straight_b) = if outlined || a.lifted || b.lifted {
                    (spend_net(a, -straight, most), spend_net(b, straight, most))
                } else {
                    (-straight, straight)
                };
                a.seated |= seated;
                b.seated |= seated;
                // A body that stands at a hard wall does not go through
                // it: it goes up to the wall, and what is left of the push
                // moves the other one. (All of it to the other one left the
                // room at the walls unused: a row a little too full between
                // them pushed its middle back and forth for ever, overlapped.)
                let (mut da, mut db) = (straight_a - slide_a, straight_b + slide_b);
                for axis in Axis::BOTH {
                    let take = |b: &Body, d: f32| hard_take(b, d, axis, &hard, &walls);
                    let ta = take(a, da[axis]);
                    if ta != da[axis] {
                        db[axis] -= da[axis] - ta;
                        da[axis] = ta;
                    } else {
                        let tb = take(b, db[axis]);
                        da[axis] -= db[axis] - tb;
                        db[axis] = tb;
                    }
                }
                // In a world open away from gravity nothing is pushed
                // toward gravity's side by a contact: the one farther out
                // takes the whole of it, outward, where there is always
                // room. So a push runs down a whole stack in one pass
                // instead of halving from body to body (SETL-19).
                if let Some(axis) = free_axis.filter(|_| push_out) {
                    let out = away[axis];
                    let b_is_out = (b.rect.center()[axis] - a.rect.center()[axis]) * out >= 0.0;
                    let apart = (db[axis] - da[axis]) * if b_is_out { out } else { -out };
                    if apart > 0.0 && !a.lifted && !b.lifted {
                        let take = |o: &mut Body, old: f32| {
                            let want = out * apart;
                            // One on top of the other, flat on flat, goes as
                            // far as it is pushed, like a rectangle: what
                            // pushes it comes at a glide. On a slanted face
                            // the push is a share of a sideways one and can
                            // be large: there an outline keeps to its glide.
                            if !outlined || hit.normal[axis].abs() > 0.9 {
                                return want;
                            }
                            let net = (o.net[axis] + want - old).clamp(-glide, glide);
                            let got = old + net - o.net[axis];
                            o.net[axis] = net;
                            got
                        };
                        if b_is_out {
                            (da[axis], db[axis]) = (0.0, take(b, db[axis]));
                        } else {
                            (da[axis], db[axis]) = (take(a, da[axis]), 0.0);
                        }
                    }
                }
                // The yielder leaves its row by a glide a step, whatever pushes it: the slides its
                // neighbours at a hard wall cannot do are handed on to it, and a corner it comes off
                // pushes it out straight by the whole gap; together they moved it three glides in one
                // step (SETL-32). What it does not take now the next passes and steps do.
                if !(outlined || a.lifted || b.lifted) {
                    if a.yielding {
                        da = spend_net(a, da, glide);
                    } else if b.yielding {
                        db = spend_net(b, db, glide);
                    }
                }
                // A pinned body is a wall: the other one takes it all. So is
                // one that a body which does not push lands on or grows
                // into (SETL-40).
                if a.pinned || moves_alone(b, a) {
                    (da, db) = (Vec2::ZERO, db - da);
                    // (put out of it is a place, not speed: it stops at the edge)
                    if !a.pinned {
                        b.shoved += db;
                    }
                } else if b.pinned || moves_alone(a, b) {
                    (da, db) = (da - db, Vec2::ZERO);
                    if !b.pinned {
                        a.shoved += da;
                    }
                }
                // Put back onto the slope it sits on is a place, not speed: kept
                // as speed (up to a seat's), a seat that had been pressed
                // deeper by a neighbour sent the body off at 46 points/s once
                // that one went, and what lay beyond with it (SETL-37).
                if seated && let Some(side) = self.params.gravity_side {
                    let g = side.axis();
                    a.shoved[g] += da[g];
                    b.shoved[g] += db[g];
                }
                a.shift(da);
                b.shift(db);
            }
        }
        if let Some((depth, front)) = band {
            let d = depth.min(self.band_budget);
            self.band_budget -= d;
            for o in &mut self.bodies {
                if !o.dragged && !o.yielding && !o.pinned && leading_edge(&o.rect, away) >= front {
                    o.shift(away * d);
                    // Moved along by the field: a glide, no speed kept.
                    o.slid += d;
                }
            }
        }
        worst
    }

    /// Where the passes left two solid bodies in each other (a row too
    /// full between two hard walls) and the side away from gravity is
    /// open, the one farther out goes out that way: by as much as parts
    /// them, a glide per step at most, every step until they are apart.
    /// The world grows there; nothing has to give in a closed wall. Run
    /// before the last pass over the pairs, which hands the move on to
    /// what lies beyond.
    fn eject_pass(&mut self) {
        let Some(axis) = self.params.free_axis() else {
            return;
        };
        let glide = self.params.glide_speed * self.params.dt;
        let out = self.yield_dir()[axis].signum();
        for i in 0..self.bodies.len() {
            for j in i + 1..self.bodies.len() {
                let (a, b) = pair_mut(&mut self.bodies, i, j);
                if a.dragged || b.dragged || a.lifted || b.lifted {
                    continue;
                }
                let Some(hit) = touch(a, b, 0.0, Vec2::ZERO).filter(|h| h.depth > EJECT_FROM)
                else {
                    continue;
                };
                let b_is_out = (b.rect.center()[axis] - a.rect.center()[axis]) * out >= 0.0;
                let (inner, outer) = if b_is_out { (&*a, b) } else { (&*b, a) };
                if outer.pinned {
                    continue;
                }
                // as far as their rectangles lie in each other that way
                let lap = if out > 0.0 {
                    inner.rect.max[axis] - outer.rect.min[axis]
                } else {
                    outer.rect.max[axis] - inner.rect.min[axis]
                };
                // Only side by side: one on top of the other is parted
                // by the pass over the pairs, along that same axis.
                if hit.normal[axis].abs() > 0.2 {
                    continue;
                }
                let mut by = Vec2::ZERO;
                // (a little under a glide: the pass over the pairs adds to it)
                by[axis] = out * lap.clamp(0.0, glide * 0.8);
                outer.shift(by);
            }
        }
    }

    /// What holds whatever the forces and the passes above did, as
    /// positions, not pushes: no body's core inside another's, and no
    /// body past a closed wall. An open side holds nothing and pulls
    /// nothing back. Where both cannot be had (a closed box too full),
    /// the walls hold.
    fn hard_pass(&mut self) {
        let walls = self.params.walls;
        let away = self.yield_dir();
        let glide = self.params.glide_speed * self.params.dt;
        for pass in 0..HARD_PASSES {
            let mut moved = false;
            for i in 0..self.bodies.len() {
                for j in i + 1..self.bodies.len() {
                    let (a, b) = pair_mut(&mut self.bodies, i, j);
                    if a.dragged || b.dragged || (a.pinned && b.pinned) {
                        continue;
                    }
                    let (depth, out) = core_depth(a, b);
                    if depth <= 0.0 {
                        continue;
                    }
                    // Two middles on one spot part away from gravity.
                    let out = if out == Vec2::ZERO { away } else { out };
                    // One that was just dropped or added there is still
                    // lifted: it parts at a glide, as it does all over.
                    // (once a step: the passes must not add up)
                    let out = match (a.lifted || b.lifted, pass) {
                        (false, _) => out,
                        (true, 0) => out.clamp_length(glide),
                        (true, _) => continue,
                    };
                    if a.pinned || moves_alone(b, a) {
                        b.shift(out);
                    } else if b.pinned || moves_alone(a, b) {
                        a.shift(-out);
                    } else {
                        a.shift(-out * 0.5);
                        b.shift(out * 0.5);
                    }
                    moved = true;
                }
            }
            for b in &mut self.bodies {
                if b.dragged || b.pinned {
                    continue;
                }
                for axis in Axis::BOTH {
                    let (low, high) = match axis {
                        Axis::X => (walls.left, walls.right),
                        Axis::Y => (walls.top, walls.bottom),
                    };
                    let mut d = 0.0;
                    if high {
                        d = (self.hard.max[axis] - b.rect.max[axis]).min(0.0);
                    }
                    // the low wall last: a body wider than the world starts at it
                    if low && b.rect.min[axis] + d < self.hard.min[axis] {
                        d = self.hard.min[axis] - b.rect.min[axis];
                    }
                    // In the user's hand a moment ago and still lifted: it
                    // glides in with the passes above, not here on top.
                    if b.lifted {
                        d = 0.0;
                    }
                    if d != 0.0 {
                        let mut by = Vec2::ZERO;
                        by[axis] = d;
                        b.shift(by);
                        if b.vel[axis] * d < 0.0 {
                            b.vel[axis] = 0.0;
                        }
                        moved = true;
                    }
                }
            }
            if !moved {
                break;
            }
        }
    }

    /// [`Body::room`] of every jammed body: a row too full between hard
    /// walls, held by the walls, takes up the gaps of its pairs, but a
    /// body growing in it must not take its rectangle into a neighbour's
    /// (the yielder makes the room at a glide, slower than it grows).
    fn measure_room(&mut self) {
        let floor = self.params.gap * RECT_FLOOR;
        let (gap, walls, hard) = (self.params.gap, self.params.walls, self.hard);
        for i in 0..self.bodies.len() {
            let a = &self.bodies[i];
            let mut room = Vec2::splat(f32::INFINITY);
            // One that does not push grows only into the room it has, walls
            // included (SETL-40).
            let holds = !a.pushes && !a.lifted && !a.dragged;
            if holds {
                for axis in Axis::BOTH {
                    let (low, high) = match axis {
                        Axis::X => (walls.left, walls.right),
                        Axis::Y => (walls.top, walls.bottom),
                    };
                    if low {
                        room[axis] = room[axis].min(a.rect.min[axis] - hard.min[axis] - gap);
                    }
                    if high {
                        room[axis] = room[axis].min(hard.max[axis] - a.rect.max[axis] - gap);
                    }
                }
            }
            if a.jammed.x > SLOP || a.jammed.y > SLOP || holds {
                for (j, o) in self.bodies.iter().enumerate() {
                    if j == i || o.lifted || a.outline.is_some() || o.outline.is_some() {
                        continue;
                    }
                    for axis in Axis::BOTH {
                        let across = axis.other();
                        let side_by_side = a.rect.max[across].min(o.rect.max[across])
                            - a.rect.min[across].max(o.rect.min[across]);
                        if side_by_side <= 0.0 {
                            continue;
                        }
                        let apart = (o.rect.min[axis] - a.rect.max[axis])
                            .max(a.rect.min[axis] - o.rect.max[axis]);
                        // (one that does not push keeps the whole gap: less would press)
                        let keep = if holds { gap } else { floor };
                        room[axis] = room[axis].min(apart - keep);
                    }
                }
            }
            self.bodies[i].room = room;
        }
    }

    /// A lifted body that is let go and touches nothing turns solid.
    fn land_lifted(&mut self) {
        let gap = self.params.gap;
        let prefer = self.yield_dir();
        for i in 0..self.bodies.len() {
            if !self.bodies[i].lifted || self.bodies[i].dragged {
                self.bodies[i].aloft = 0;
                continue;
            }
            // Lifted is a passing state: one that found no free place in
            // a while lands where it is, and parts from what it lies on
            // as a solid body does (at a glide).
            self.bodies[i].aloft += 1;
            // One that does not push and finds no free place goes back where
            // it was picked up, over the others (SETL-40).
            let b = &mut self.bodies[i];
            if !b.pushes
                && b.aloft > REFUSE_STEPS
                && let Some(from) = b.picked_from.take()
            {
                b.nest = Some(from);
                b.dragged = true;
                b.drag_target = b.rect.center();
                b.aloft = 0;
                continue;
            }
            if self.bodies[i].aloft > LIFT_STEPS {
                self.bodies[i].lifted = false;
                continue;
            }
            let a = &self.bodies[i];
            let free =
                self.bodies.iter().enumerate().all(|(j, o)| {
                    j == i || touch(a, o, gap, prefer).is_none_or(|h| h.depth <= SLOP)
                });
            // Dropped past a closed wall it is still on its way in.
            let walls = self.params.walls;
            let inside = Side::ALL.iter().all(|side| {
                let axis = side.axis();
                !walls.has(*side)
                    || if side.is_low() {
                        a.rect.min[axis] >= self.hard.min[axis] - SLOP
                    } else {
                        a.rect.max[axis] <= self.hard.max[axis] + SLOP
                            || a.rect.size()[axis] > self.hard.size()[axis]
                    }
            });
            if free && inside {
                let b = &mut self.bodies[i];
                b.lifted = false;
                b.picked_from = None;
                // One that does not push came to rest by its neighbours' edge:
                // that is its place now, its home does not pull it back in.
                if !b.pushes && b.home.is_some() {
                    b.home = Some(b.rect.center());
                }
            }
        }
    }
}

/// Whether `a` takes the whole of its contact with `b` (SETL-40): it does
/// not push, and it is the one that came onto `b` (lifted: just added or
/// let go there) or grew into it this step, while `b` did neither.
fn moves_alone(a: &Body, b: &Body) -> bool {
    let active = |o: &Body| {
        let grew = o.rect.size() - o.prev.size();
        let landing = o.landed < LANDED_STEPS;
        o.lifted || landing || o.nest.is_some() || grew.x > 1e-4 || grew.y > 1e-4
    };
    !a.pushes && active(a) && !(active(b) && !b.pushes)
}

/// The part of a move by `d` along `axis` a solid body can make before the
/// hard wall it would go through: all of it where no hard wall is in the way.
fn hard_take(b: &Body, d: f32, axis: Axis, hard: &Rect, walls: &crate::params::Walls) -> f32 {
    if !at_hard_wall(b, d, axis, hard, walls) {
        d
    } else if d < 0.0 {
        (hard.min[axis] - b.rect.min[axis]).min(0.0).max(d)
    } else {
        (hard.max[axis] - b.rect.max[axis]).max(0.0).min(d)
    }
}

/// Whether a move by `d` along `axis` would take a solid body through the
/// hard wall it stands at.
fn at_hard_wall(b: &Body, d: f32, axis: Axis, hard: &Rect, walls: &crate::params::Walls) -> bool {
    if b.lifted || d == 0.0 {
        return false;
    }
    let (low, high) = match axis {
        Axis::X => (walls.left, walls.right),
        Axis::Y => (walls.top, walls.bottom),
    };
    if d < 0.0 {
        low && b.rect.min[axis] + d < hard.min[axis]
    } else {
        high && b.rect.max[axis] + d > hard.max[axis] && b.rect.size()[axis] <= hard.size()[axis]
    }
}

/// How deep the cores of two bodies are in each other, as a share of the
/// way in (0 or less: apart; 1: their middles on one spot), and the
/// shortest move of `b` away from `a` that parts them (half each when
/// both give). A core is a rhombus around the middle, [`CORE`] of the
/// rectangle wide and high.
fn core_depth(a: &Body, b: &Body) -> (f32, Vec2) {
    let reach = (a.rect.size() + b.rect.size()) * (CORE * 0.5);
    if reach.x <= 0.0 || reach.y <= 0.0 {
        return (0.0, Vec2::ZERO);
    }
    let d = b.rect.center() - a.rect.center();
    let depth = 1.0 - d.x.abs() / reach.x - d.y.abs() / reach.y;
    if depth <= 0.0 {
        return (depth, Vec2::ZERO);
    }
    // out through the nearer side of the rhombus: along its shorter half
    let out = if reach.x <= reach.y {
        Vec2::new(depth * reach.x * if d.x < 0.0 { -1.0 } else { 1.0 }, 0.0)
    } else {
        Vec2::new(0.0, depth * reach.y * if d.y < 0.0 { -1.0 } else { 1.0 })
    };
    let out = if d == Vec2::ZERO { Vec2::ZERO } else { out };
    (depth, out)
}

/// How far along `away` (a unit axis vector) a rectangle's edge facing
/// back from it lies, signed so that farther along is larger.
fn leading_edge(r: &Rect, away: Vec2) -> f32 {
    if away.x + away.y > 0.0 {
        r.min.dot(away)
    } else {
        r.max.dot(away)
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

/// The part of a push `d` that keeps all the pushes on the body in this
/// step together within `most` points, taken from that budget.
fn spend_net(b: &mut Body, d: Vec2, most: f32) -> Vec2 {
    let to = (b.net + d).clamp_length(most);
    let d = to - b.net;
    b.net = to;
    b.nudged = true;
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
/// and let the bulge follow the pressure. On an axis where the last step
/// jammed it (`jammed`: a row too full between hard walls, nothing left
/// to give) it grows only as far as [`Body::room`] lets its rectangle:
/// growing on would press the row deeper than its gaps, and the
/// rectangles would overlap while the body that gives way is still on its
/// way out at a glide. The jam in the gaps is pressure enough for that
/// one to go; once there is room it grows on. Returns how much the bulge
/// changed.
fn resize(b: &mut Body, p: &Params, dt: f32, jammed: Vec2) -> f32 {
    let target = b.want.max(b.min_size).min(b.max_size);
    let grow = p.grow_speed * dt;
    let keep = (-p.restore * dt).exp();
    let mut bulged = 0.0f32;
    for axis in Axis::BOTH {
        // (it grows on both sides: half of it on each)
        let up = if jammed[axis] > SLOP || (!b.pushes && !b.lifted && !b.dragged) {
            grow.min(2.0 * b.room[axis]).max(0.0)
        } else {
            grow
        };
        b.natural[axis] += (target[axis] - b.natural[axis]).clamp(-grow, up);
        let mut squeeze = b.squeeze[axis] * keep;
        if squeeze < 1e-4 {
            squeeze = 0.0;
        }
        // Squeezed and stretched at once: the squeeze takes back what
        // vertex attraction added first.
        for stretch in [&mut b.stretch_lo, &mut b.stretch_hi] {
            let back = stretch[axis].clamp(0.0, squeeze);
            stretch[axis] -= back;
            squeeze -= back;
        }
        let room = b.natural[axis] + b.stretch()[axis] - b.min_size[axis];
        b.squeeze[axis] = squeeze.min(room.max(0.0));
        // The content changed size: the stretch stays within the limits.
        let fitted = (b.natural[axis] - b.squeeze[axis] + b.stretch()[axis])
            .clamp(b.min_size[axis], b.max_size[axis].max(b.min_size[axis]));
        let mut excess = b.natural[axis] - b.squeeze[axis] + b.stretch()[axis] - fitted;
        for stretch in [&mut b.stretch_lo, &mut b.stretch_hi] {
            let take = if excess > 0.0 {
                stretch[axis].clamp(0.0, excess)
            } else {
                stretch[axis].clamp(excess, 0.0)
            };
            stretch[axis] -= take;
            excess -= take;
        }
        b.jam[axis] *= keep;
        if b.jam[axis] < 1e-4 {
            b.jam[axis] = 0.0;
        }
        let size = (b.natural[axis] - b.squeeze[axis] + b.stretch()[axis]).max(0.0);
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
        // With the wall across from gravity's side open, nothing holds the
        // field on gravity's axis: what the solver leaves of a tall stack
        // there is no pressure.
        let load = if p.free_axis() == Some(axis) {
            0.0
        } else {
            (b.load()[axis] - p.bulge_from).max(0.0)
        };
        // An outline has its own shape: it does not bulge.
        let want = if b.outline.is_some() {
            0.0
        } else {
            (p.bulge_rest + p.bulge_gain * load).clamp(0.0, cap)
        };
        let step = p.bulge_speed.max(0.0) * dt;
        let bulge = b.bulge[axis] + (want - b.bulge[axis]).clamp(-step, step);
        bulged = bulged.max((bulge - b.bulge[axis]).abs());
        b.bulge[axis] = if bulge < 1e-4 { 0.0 } else { bulge };
    }
    bulged
}

/// Gravity, the home spring and damping move a free body; a dragged one
/// goes where it is held. Returns how far the home spot drifted.
fn integrate(b: &mut Body, p: &Params, dt: f32, cool: f32) -> f32 {
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
    if b.pinned {
        b.vel = Vec2::ZERO;
        b.coast = Vec2::ZERO;
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
    b.vel = ((b.vel + pull * (dt * cool)) * (damp * cool)).clamp_length(p.max_speed);
    // drag growing with the speed above DRAG_FROM, implicit so a large step
    // never turns it round; slow settling is left as it was
    let fast = b.vel.length() - DRAG_FROM;
    if fast > 0.0 {
        b.vel = b.vel * (1.0 / (1.0 + p.drag * fast * dt));
    }
    b.rect = b.rect.translate(b.vel * dt);
    b.coast = b.vel;
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
    if a.outline.is_some() || b.outline.is_some() {
        return outline_hit(a, &a.rect, b, &b.rect, gap, tie_dir(a, b, yield_dir));
    }
    let (ab, bb) = pair_bulges(a, b);
    let mut hit = touch_at(&a.rect, ab, &b.rect, bb, gap, tie_dir(a, b, yield_dir))?;
    // A slope sends the yielder out on the side away from gravity, never
    // back toward it: it may have been pulled a little that way before the
    // pressure built, and would wedge into the full row behind it.
    if a.yielding != b.yielding {
        let away_axis = yield_dir.main_axis();
        let across = hit.normal.main_axis();
        let pushes_yielder = if a.yielding { -hit.normal } else { hit.normal };
        if across != away_axis && pushes_yielder[away_axis] * yield_dir[away_axis] < 0.0 {
            hit.normal[away_axis] = -hit.normal[away_axis];
        }
    }
    Some(hit)
}

/// The contact between two bodies of which at least one has an outline,
/// as they lie in `ar` and `br`, kept `gap` apart: the separating axis of
/// their polygons (a plain rectangle meets an outline as its rectangle,
/// without bulge).
fn outline_hit(a: &Body, ar: &Rect, b: &Body, br: &Rect, gap: f32, prefer: Vec2) -> Option<Hit> {
    let (pa, pb) = (a.shape_in(ar), b.shape_in(br));
    let o = pa.bounds().overlap(&pb.bounds(), gap);
    if o.x <= 0.0 || o.y <= 0.0 {
        return None;
    }
    contact(&pa, &pb, gap, prefer)
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

/// The corner at the `high` end of `axis` and the `t_high` end of the
/// other axis.
fn corner_of(axis: Axis, high: bool, t_high: bool) -> (bool, bool) {
    match axis {
        Axis::X => (high, t_high),
        Axis::Y => (t_high, high),
    }
}

/// Whether a corner is at the high end of `axis`.
fn corner_high((high_x, high_y): (bool, bool), axis: Axis) -> bool {
    match axis {
        Axis::X => high_x,
        Axis::Y => high_y,
    }
}

/// The side at one end of `axis`.
fn axis_side(axis: Axis, high: bool) -> Side {
    match (axis, high) {
        (Axis::X, false) => Side::Left,
        (Axis::X, true) => Side::Right,
        (Axis::Y, false) => Side::Top,
        (Axis::Y, true) => Side::Bottom,
    }
}

/// Where corner `ca` of a body lines up with corner `cb` (at `at`) of
/// another: on it, but the gap away on each axis the two bodies face each
/// other on.
fn gapped(at: Vec2, ca: (bool, bool), cb: (bool, bool), gap: f32) -> Vec2 {
    let mut to = at;
    if cb.0 != ca.0 {
        to.x -= if ca.0 { gap } else { -gap };
    }
    if cb.1 != ca.1 {
        to.y -= if ca.1 { gap } else { -gap };
    }
    to
}

/// The vertices of `a` that line up with vertices of `b` across two faces
/// that lie against each other (SETL-20): faces turned toward each other
/// (antiparallel), the gap apart within `reach`, side by side along their
/// length. Each end of `a`'s face pairs with the end of `b`'s face across
/// from it: (vertex of `a`, vertex of `b`, the outward normal of `b`'s
/// face). Lined up, `a`'s vertex is `b`'s plus the normal times the gap.
fn face_pairs(a: &Poly, b: &Poly, gap: f32, reach: f32) -> Vec<(usize, usize, Vec2)> {
    let (pa, pb) = (a.points(), b.points());
    let (na, nb) = (pa.len(), pb.len());
    let mut out = Vec::new();
    if na < 3 || nb < 3 {
        return out;
    }
    let mid_b = b.centroid();
    for i in 0..na {
        let (a0, a1) = (pa[i], pa[(i + 1) % na]);
        let la = (a1 - a0).length();
        if la < 1e-3 {
            continue;
        }
        let da = (a1 - a0) * (1.0 / la);
        for j in 0..nb {
            let (b0, b1) = (pb[j], pb[(j + 1) % nb]);
            let lb = (b1 - b0).length();
            if lb < 1e-3 {
                continue;
            }
            let db = (b1 - b0) * (1.0 / lb);
            if da.dot(db) > -0.98 {
                continue;
            }
            let mut n = Vec2::new(-db.y, db.x);
            if (mid_b - b0).dot(n) > 0.0 {
                n = -n;
            }
            if ((a0 - b0).dot(n) - gap).abs() > reach {
                continue;
            }
            // side by side: their extents along the face overlap
            let (s0, s1) = ((a0 - b0).dot(db), (a1 - b0).dot(db));
            if s0.max(s1) <= 0.0 || s0.min(s1) >= lb {
                continue;
            }
            for (ia, ib) in [(i, (j + 1) % nb), ((i + 1) % na, j)] {
                if (pb[ib] + n * gap - pa[ia]).length() < reach {
                    out.push((ia, ib, n));
                }
            }
        }
    }
    out
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
