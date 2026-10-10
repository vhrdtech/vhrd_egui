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
            max_speed: 2400.0,
            walls: Walls::ALL,
            iterations: 16,
            rest_eps: 0.005,
            rest_steps: 24,
            dt: 1.0 / 120.0,
        }
    }
}
