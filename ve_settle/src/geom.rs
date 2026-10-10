//! The little geometry the solver needs: a 2D vector, an axis-aligned rectangle, a side.
//!
//! Screen coordinates: x grows to the right, y grows down.

use std::ops::{Add, AddAssign, Index, IndexMut, Mul, Neg, Sub, SubAssign};

/// One of the two axes; indexes a [`Vec2`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Axis {
    /// Left to right.
    X,
    /// Top to bottom.
    Y,
}

impl Axis {
    /// Both axes, for loops.
    pub const BOTH: [Axis; 2] = [Axis::X, Axis::Y];
}

/// A 2D vector or point, in points (logical pixels).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    /// Right is positive.
    pub x: f32,
    /// Down is positive.
    pub y: f32,
}

impl Vec2 {
    /// The zero vector.
    pub const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };
    /// No upper limit: the default maximum size of a body.
    pub const INFINITY: Vec2 = Vec2 {
        x: f32::INFINITY,
        y: f32::INFINITY,
    };

    /// A vector from its two components.
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Both components the same.
    pub const fn splat(v: f32) -> Self {
        Self { x: v, y: v }
    }

    /// Euclidean length.
    pub fn length(self) -> f32 {
        self.x.hypot(self.y)
    }

    /// Component-wise minimum.
    pub fn min(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x.min(o.x), self.y.min(o.y))
    }

    /// Component-wise maximum.
    pub fn max(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x.max(o.x), self.y.max(o.y))
    }

    /// The larger of the two absolute components.
    pub fn max_abs(self) -> f32 {
        self.x.abs().max(self.y.abs())
    }

    /// The same direction, no longer than `max`.
    pub fn clamp_length(self, max: f32) -> Vec2 {
        let len = self.length();
        if len > max && len > 0.0 {
            self * (max / len)
        } else {
            self
        }
    }
}

impl Index<Axis> for Vec2 {
    type Output = f32;
    fn index(&self, axis: Axis) -> &f32 {
        match axis {
            Axis::X => &self.x,
            Axis::Y => &self.y,
        }
    }
}

impl IndexMut<Axis> for Vec2 {
    fn index_mut(&mut self, axis: Axis) -> &mut f32 {
        match axis {
            Axis::X => &mut self.x,
            Axis::Y => &mut self.y,
        }
    }
}

impl Add for Vec2 {
    type Output = Vec2;
    fn add(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x + o.x, self.y + o.y)
    }
}

impl Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x - o.x, self.y - o.y)
    }
}

impl Mul<f32> for Vec2 {
    type Output = Vec2;
    fn mul(self, k: f32) -> Vec2 {
        Vec2::new(self.x * k, self.y * k)
    }
}

impl Neg for Vec2 {
    type Output = Vec2;
    fn neg(self) -> Vec2 {
        Vec2::new(-self.x, -self.y)
    }
}

impl AddAssign for Vec2 {
    fn add_assign(&mut self, o: Vec2) {
        *self = *self + o;
    }
}

impl SubAssign for Vec2 {
    fn sub_assign(&mut self, o: Vec2) {
        *self = *self - o;
    }
}

/// An axis-aligned rectangle.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    /// Top left corner.
    pub min: Vec2,
    /// Bottom right corner.
    pub max: Vec2,
}

impl Rect {
    /// A rectangle from its top left corner and its size.
    pub fn from_min_size(min: Vec2, size: Vec2) -> Self {
        Self {
            min,
            max: min + size,
        }
    }

    /// A rectangle from its middle and its size.
    pub fn from_center_size(center: Vec2, size: Vec2) -> Self {
        let half = size * 0.5;
        Self {
            min: center - half,
            max: center + half,
        }
    }

    /// The middle.
    pub fn center(&self) -> Vec2 {
        (self.min + self.max) * 0.5
    }

    /// Width and height.
    pub fn size(&self) -> Vec2 {
        self.max - self.min
    }

    /// The same rectangle moved by `d`.
    pub fn translate(&self, d: Vec2) -> Rect {
        Rect {
            min: self.min + d,
            max: self.max + d,
        }
    }

    /// How deep the two rectangles are in each other on each axis, after
    /// growing both by `gap / 2`. They overlap only when both components
    /// are positive; the smaller one is the cheapest way apart.
    pub fn overlap(&self, o: &Rect, gap: f32) -> Vec2 {
        Vec2::new(
            self.max.x.min(o.max.x) - self.min.x.max(o.min.x) + gap,
            self.max.y.min(o.max.y) - self.min.y.max(o.min.y) + gap,
        )
    }

    /// Whether `p` is inside (edges included).
    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.min.x && p.x <= self.max.x && p.y >= self.min.y && p.y <= self.max.y
    }
}

/// A side of the world; gravity pulls toward one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    /// Toward smaller y (gravity up: the dash default).
    Top,
    /// Toward larger y.
    Bottom,
    /// Toward smaller x.
    Left,
    /// Toward larger x.
    Right,
}

impl Side {
    /// All four, for loops and pickers.
    pub const ALL: [Side; 4] = [Side::Top, Side::Bottom, Side::Left, Side::Right];

    /// The unit vector pointing at this side.
    pub fn dir(self) -> Vec2 {
        match self {
            Side::Top => Vec2::new(0.0, -1.0),
            Side::Bottom => Vec2::new(0.0, 1.0),
            Side::Left => Vec2::new(-1.0, 0.0),
            Side::Right => Vec2::new(1.0, 0.0),
        }
    }

    /// The axis this side is at the end of.
    pub fn axis(self) -> Axis {
        match self {
            Side::Top | Side::Bottom => Axis::Y,
            Side::Left | Side::Right => Axis::X,
        }
    }

    /// Whether this is the low end of its axis (top or left).
    pub fn is_low(self) -> bool {
        matches!(self, Side::Top | Side::Left)
    }
}
