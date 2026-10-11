//! Every knob of the solver in one plain struct, so a sandbox can put a slider on each.

use crate::geom::Side;

/// Which sides of the world hold bodies in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Walls {
    /// The top side is closed.
    pub top: bool,
    /// The bottom side is closed.
    pub bottom: bool,
    /// The left side is closed.
    pub left: bool,
    /// The right side is closed.
    pub right: bool,
}

impl Walls {
    /// All four sides closed.
    pub const ALL: Walls = Walls {
        top: true,
        bottom: true,
        left: true,
        right: true,
    };

    /// Whether `side` is closed.
    pub fn has(&self, side: Side) -> bool {
        match side {
            Side::Top => self.top,
            Side::Bottom => self.bottom,
            Side::Left => self.left,
            Side::Right => self.right,
        }
    }

    /// Open or close `side`.
    pub fn set(&mut self, side: Side, closed: bool) {
        match side {
            Side::Top => self.top = closed,
            Side::Bottom => self.bottom = closed,
            Side::Left => self.left = closed,
            Side::Right => self.right = closed,
        }
    }
}

/// The solver's parameters. Lengths are in points, times in seconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Params {
    /// The side gravity pulls toward, `None` for no gravity.
    pub gravity_side: Option<Side>,
    /// Gravity's acceleration, points/s².
    pub gravity: f32,
    /// Stiffness of the spring pulling a body to its home spot, 1/s²
    /// (acceleration per point of distance). 0 turns homes off.
    pub home_stiffness: f32,
    /// How fast a home spot follows its body, 1/s. 0 keeps homes fixed;
    /// above 0 a body pushed away for long makes the new place its home.
    pub home_drift: f32,
    /// Velocity damping, 1/s: the share of speed lost per second.
    pub damping: f32,
    /// Free space kept between bodies and to the walls.
    pub gap: f32,
    /// How readily bodies give when there is no room, 0..=1: the share of
    /// an overlap that cannot be pushed away which squeezing takes per
    /// pass. 0 is rigid (bodies stick out past the walls instead).
    pub tension: f32,
    /// How fast a squeezed body gets its size back, 1/s. Against
    /// `tension` this sets how much stays squeezed when room is short.
    pub restore: f32,
    /// How fast a body's size follows the size it wants, points/s, so
    /// growing content pushes its neighbours instead of jumping into them.
    pub grow_speed: f32,
    /// How fast things glide where a jump would be needed, points/s:
    /// bodies away from a lifted (dragged, just added) body, and bodies
    /// back inside walls that moved.
    pub glide_speed: f32,
    /// How near a corner has to be to a corner of another body, or of the
    /// walls, to be pulled onto it, points. 0 turns corner snapping off.
    pub snap_range: f32,
    /// Stiffness of the pull that lines a corner up with the one it snaps
    /// to, 1/s² (critically damped, so it closes in without swinging).
    /// What is left of a misalignment at rest is about the other pulls
    /// over this: gravity 600 against 1500 leaves 0.4 points.
    pub snap_stiffness: f32,
    /// How near a vertex has to be to a vertex or an edge of another body,
    /// or to a wall, to be pulled to line up with it, points (SETL-12). The
    /// pull resizes the body. 0 turns vertex attraction off.
    pub attract_range: f32,
    /// How fast an edge closes in on what its vertices are pulled to, 1/s:
    /// the share of the distance per second (the world stays awake until
    /// about a hundredth of a point is left). 0 turns vertex attraction
    /// off.
    pub attract_strength: f32,
    /// How steep a slope of an outline holds a body that gravity presses
    /// onto it: the most its tangent may be (sideways over along gravity).
    /// 2 holds on a hexagon's 60° edges; 0 lets everything slide, as on
    /// ice (SETL-15). Only between bodies of which one has an outline.
    pub friction: f32,
    /// Bulge of a body's pillow with no pressure on it, points: how far
    /// the middle of each edge stands out of the rectangle in the solver's
    /// eyes. 0 (the default) keeps a calm row lined up exactly.
    pub bulge_rest: f32,
    /// Pressure below which nothing bulges, points: a stack resting under
    /// gravity keeps a little (the solver leaves a few hundredths of a
    /// point per step to its last passes), and must stay plain rectangles.
    pub bulge_from: f32,
    /// How much the bulge grows with pressure beyond `bulge_from`, points of
    /// bulge per point of pressure (the squeeze a body has on an axis, or
    /// would have if it could give). The edges facing the squeeze bulge, like a balloon, so
    /// bodies pressed too hard in a row or column slide off each other
    /// instead of being crushed. 0 turns it off.
    pub bulge_gain: f32,
    /// The most an edge bulges, points; never more than a quarter of the
    /// edge's length either.
    pub bulge_max: f32,
    /// How fast the bulge follows the pressure, points/s.
    pub bulge_speed: f32,
    /// Grid pitch, points: touching bodies are guided to line up their
    /// edges along the contact at whole steps of this apart (SETL-10).
    pub grid_pitch: f32,
    /// Stiffness of the pull onto grid steps, 1/s² near a step (half
    /// critically damped there); it is a washboard, strongest a quarter
    /// pitch off a step, and only grows as two bodies slow down against
    /// each other, so free motion stays smooth. 0 turns it off.
    pub grid_stiffness: f32,
    /// Speed limit, points/s.
    pub max_speed: f32,
    /// Which sides are closed.
    pub walls: Walls,
    /// Contact solver passes per step, at most.
    pub iterations: u32,
    /// A step in which nothing moved more than this (points) is calm.
    pub rest_eps: f32,
    /// This many calm steps in a row put the world to rest.
    pub rest_steps: u32,
    /// The fixed time step, seconds.
    pub dt: f32,
    /// In a world open away from gravity, a contact along gravity's axis
    /// moves only the body farther out, by the whole of it, instead of
    /// both by half: a push runs down a whole stack in one pass, and
    /// nothing is pushed toward gravity's wall. For stacks that gravity
    /// packs (no homes); off by default, where a push is shared.
    pub push_out: bool,
}

impl Params {
    /// Gravity's axis when the wall across from gravity's side is open (a
    /// dash running past the bottom): nothing holds the bodies on that
    /// axis, so what the solver leaves of a tall stack there is neither
    /// squeezed nor felt as pressure; the stack settles by moving.
    pub fn free_axis(&self) -> Option<crate::geom::Axis> {
        let side = self.gravity_side?;
        let behind = match side {
            Side::Top => Side::Bottom,
            Side::Bottom => Side::Top,
            Side::Left => Side::Right,
            Side::Right => Side::Left,
        };
        (!self.walls.has(behind)).then(|| side.axis())
    }
}

impl Params {
    /// The same parameters with every number inside the range the solver
    /// can work with: nothing negative that is a size, a speed or a rate,
    /// nothing that is not a number (it becomes the default), a step
    /// between 1/1000 and 1/20 s, at least one pass. [`crate::World`]
    /// takes parameters through this, so no slider can break it.
    pub fn sanitized(mut self) -> Self {
        let d = Params::default();
        let fix = |v: &mut f32, default: f32, lo: f32, hi: f32| {
            *v = if v.is_finite() {
                v.clamp(lo, hi)
            } else {
                default
            };
        };
        let big = 1e6;
        fix(&mut self.gravity, d.gravity, 0.0, big);
        fix(&mut self.home_stiffness, d.home_stiffness, 0.0, big);
        fix(&mut self.home_drift, d.home_drift, 0.0, big);
        fix(&mut self.damping, d.damping, 0.0, big);
        fix(&mut self.gap, d.gap, 0.0, big);
        fix(&mut self.tension, d.tension, 0.0, 1.0);
        fix(&mut self.restore, d.restore, 0.0, big);
        fix(&mut self.grow_speed, d.grow_speed, 1.0, big);
        fix(&mut self.glide_speed, d.glide_speed, 1.0, big);
        fix(&mut self.snap_range, d.snap_range, 0.0, big);
        fix(&mut self.snap_stiffness, d.snap_stiffness, 0.0, big);
        fix(&mut self.attract_range, d.attract_range, 0.0, big);
        fix(&mut self.attract_strength, d.attract_strength, 0.0, big);
        fix(&mut self.friction, d.friction, 0.0, big);
        fix(&mut self.bulge_rest, d.bulge_rest, 0.0, big);
        fix(&mut self.bulge_from, d.bulge_from, 0.0, big);
        fix(&mut self.bulge_gain, d.bulge_gain, 0.0, big);
        fix(&mut self.bulge_max, d.bulge_max, 0.0, big);
        fix(&mut self.bulge_speed, d.bulge_speed, 1.0, big);
        fix(&mut self.grid_pitch, d.grid_pitch, 0.0, big);
        fix(&mut self.grid_stiffness, d.grid_stiffness, 0.0, big);
        fix(&mut self.max_speed, d.max_speed, 1.0, big);
        fix(&mut self.rest_eps, d.rest_eps, 1e-6, big);
        fix(&mut self.dt, d.dt, 1e-3, 0.05);
        self.iterations = self.iterations.clamp(1, 256);
        self.rest_steps = self.rest_steps.max(1);
        self
    }

    /// These parameters moved `share` (0 to 1) of the way to `to`: every
    /// number that acts as a force, a speed or a size. What is a choice
    /// (gravity's side, the walls) or belongs to the solver's clock (the
    /// step, the passes, when it rests) is taken from `to` as it is.
    pub fn toward(&self, to: &Params, share: f32) -> Params {
        let t = share.clamp(0.0, 1.0);
        let mix = |a: f32, b: f32| if t >= 1.0 { b } else { a + (b - a) * t };
        Params {
            gravity_side: to.gravity_side,
            gravity: mix(self.gravity, to.gravity),
            home_stiffness: mix(self.home_stiffness, to.home_stiffness),
            home_drift: mix(self.home_drift, to.home_drift),
            damping: mix(self.damping, to.damping),
            gap: mix(self.gap, to.gap),
            tension: mix(self.tension, to.tension),
            restore: mix(self.restore, to.restore),
            grow_speed: mix(self.grow_speed, to.grow_speed),
            glide_speed: mix(self.glide_speed, to.glide_speed),
            snap_range: mix(self.snap_range, to.snap_range),
            snap_stiffness: mix(self.snap_stiffness, to.snap_stiffness),
            attract_range: mix(self.attract_range, to.attract_range),
            attract_strength: mix(self.attract_strength, to.attract_strength),
            friction: mix(self.friction, to.friction),
            bulge_rest: mix(self.bulge_rest, to.bulge_rest),
            bulge_from: mix(self.bulge_from, to.bulge_from),
            bulge_gain: mix(self.bulge_gain, to.bulge_gain),
            bulge_max: mix(self.bulge_max, to.bulge_max),
            bulge_speed: mix(self.bulge_speed, to.bulge_speed),
            grid_pitch: mix(self.grid_pitch, to.grid_pitch),
            grid_stiffness: mix(self.grid_stiffness, to.grid_stiffness),
            max_speed: mix(self.max_speed, to.max_speed),
            walls: to.walls,
            iterations: to.iterations,
            rest_eps: to.rest_eps,
            rest_steps: to.rest_steps,
            dt: to.dt,
            push_out: to.push_out,
        }
    }
}

impl Default for Params {
    fn default() -> Self {
        Self {
            gravity_side: Some(Side::Top),
            gravity: 600.0,
            home_stiffness: 30.0,
            home_drift: 0.0,
            damping: 10.0,
            gap: 6.0,
            tension: 0.5,
            restore: 2.0,
            grow_speed: 400.0,
            glide_speed: 900.0,
            snap_range: 20.0,
            snap_stiffness: 1500.0,
            attract_range: 16.0,
            attract_strength: 12.0,
            friction: 2.0,
            bulge_rest: 0.0,
            bulge_from: 2.0,
            bulge_gain: 1.0,
            bulge_max: 24.0,
            bulge_speed: 120.0,
            grid_pitch: 24.0,
            grid_stiffness: 0.0,
            max_speed: 2400.0,
            walls: Walls::ALL,
            iterations: 16,
            rest_eps: 0.005,
            rest_steps: 24,
            dt: 1.0 / 120.0,
            push_out: false,
        }
    }
}
