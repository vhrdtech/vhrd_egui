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
    /// Stiffness of the pull onto grid steps, 1/s² near a step
    /// (critically damped there); it is a washboard, strongest a quarter
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
        }
    }
}
