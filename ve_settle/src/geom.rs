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

impl Vec2 {
    /// Dot product.
    pub fn dot(self, o: Vec2) -> f32 {
        self.x * o.x + self.y * o.y
    }

    /// The axis with the larger absolute component (`Y` on a tie).
    pub fn main_axis(self) -> Axis {
        if self.x.abs() > self.y.abs() {
            Axis::X
        } else {
            Axis::Y
        }
    }
}

impl Axis {
    /// The other axis.
    pub fn other(self) -> Axis {
        match self {
            Axis::X => Axis::Y,
            Axis::Y => Axis::X,
        }
    }
}

/// Most vertices a [`Poly`] holds: a hexagon with a bulge on every edge.
pub const MAX_VERTICES: usize = 12;

/// A convex polygon of at most [`MAX_VERTICES`] vertices, kept on the
/// stack: the collision shape the solver sees.
///
/// A rectangle's shape is its *pillow* ([`Poly::pillow`]): the four
/// corners and a vertex in the middle of each edge pushed outward by a
/// bulge, so two bulged edges pressed together meet at one ridge and slide
/// off each other instead of holding.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Poly {
    pts: [Vec2; MAX_VERTICES],
    len: usize,
}

impl Poly {
    /// A polygon from its vertices in order around it (either way round).
    /// They must form a convex polygon; past [`MAX_VERTICES`] the rest are
    /// dropped.
    pub fn new(points: &[Vec2]) -> Self {
        let len = points.len().min(MAX_VERTICES);
        let mut pts = [Vec2::ZERO; MAX_VERTICES];
        pts[..len].copy_from_slice(&points[..len]);
        Self { pts, len }
    }

    /// The four corners of `r`.
    pub fn rect(r: &Rect) -> Self {
        Self::pillow(r, Vec2::ZERO)
    }

    /// The pillow of `r`: its corners, and in the middle of each edge a
    /// vertex pushed outward by `bulge` (`bulge.x` on the left and right
    /// edges, `bulge.y` on the top and bottom ones). A bulge of 0 leaves
    /// that vertex out, so `pillow(r, Vec2::ZERO)` is `r` itself.
    pub fn pillow(r: &Rect, bulge: Vec2) -> Self {
        let c = r.center();
        let mut out = Self::new(&[]);
        let mut push = |p: Vec2| {
            out.pts[out.len] = p;
            out.len += 1;
        };
        push(r.min);
        if bulge.y > 0.0 {
            push(Vec2::new(c.x, r.min.y - bulge.y));
        }
        push(Vec2::new(r.max.x, r.min.y));
        if bulge.x > 0.0 {
            push(Vec2::new(r.max.x + bulge.x, c.y));
        }
        push(r.max);
        if bulge.y > 0.0 {
            push(Vec2::new(c.x, r.max.y + bulge.y));
        }
        push(Vec2::new(r.min.x, r.max.y));
        if bulge.x > 0.0 {
            push(Vec2::new(r.min.x - bulge.x, c.y));
        }
        out
    }

    /// A regular hexagon around `center`, `radius` to each vertex, with
    /// two vertices on the horizontal through the middle (flat top).
    pub fn hexagon(center: Vec2, radius: f32) -> Self {
        let mut pts = [Vec2::ZERO; 6];
        for (i, p) in pts.iter_mut().enumerate() {
            let a = std::f32::consts::FRAC_PI_3 * i as f32;
            *p = center + Vec2::new(a.cos(), a.sin()) * radius;
        }
        Self::new(&pts)
    }

    /// The vertices, in order around the polygon.
    pub fn points(&self) -> &[Vec2] {
        &self.pts[..self.len]
    }

    /// The mean of the vertices: inside, since the polygon is convex.
    pub fn centroid(&self) -> Vec2 {
        let sum = self.points().iter().fold(Vec2::ZERO, |s, &p| s + p);
        sum * (1.0 / self.len.max(1) as f32)
    }

    /// The same polygon moved by `d`.
    pub fn translate(&self, d: Vec2) -> Poly {
        let mut out = *self;
        for p in &mut out.pts[..out.len] {
            *p += d;
        }
        out
    }

    /// The smallest and largest projection onto `n`.
    fn project(&self, n: Vec2) -> (f32, f32) {
        self.points()
            .iter()
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), p| {
                let d = p.dot(n);
                (lo.min(d), hi.max(d))
            })
    }

    /// The outward unit normal of each edge.
    fn normals(&self) -> impl Iterator<Item = Vec2> + '_ {
        let c = self.centroid();
        (0..self.len).filter_map(move |i| {
            let (p, q) = (self.pts[i], self.pts[(i + 1) % self.len]);
            let e = q - p;
            let len = e.length();
            if len <= 1e-6 {
                return None;
            }
            let n = Vec2::new(e.y, -e.x) * (1.0 / len);
            Some(if (c - p).dot(n) > 0.0 { -n } else { n })
        })
    }
}

/// Where two polygons meet: how to push them apart.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    /// Unit vector from the first polygon toward the second: moving the
    /// second along it (or the first against it) by `depth` parts them.
    pub normal: Vec2,
    /// How far they have to move apart, points.
    pub depth: f32,
}

/// Depths closer than this count as a tie, broken by the preferred direction.
const TIE: f32 = 1e-3;

/// Separating axis test for two convex polygons that are to be kept
/// `gap(normal)` apart: `None` when they are that far apart on some edge
/// normal of either, else the edge normal along which the least push parts
/// them. Ties (two ridges meeting tip to tip) go to the normal that pushes
/// `b` most along `prefer`, then to the first found; with `prefer` zero
/// they are split: the tied normals are averaged (two mirrored slopes
/// give the straight axis between them). The answer depends on nothing
/// but the inputs.
pub fn sat(a: &Poly, b: &Poly, gap: impl Fn(Vec2) -> f32, prefer: Vec2) -> Option<Hit> {
    let mut best: Option<Hit> = None;
    // With no preference: the sum of the tied normals, averaged at the end.
    let mut tied = Vec2::ZERO;
    for n in a.normals().chain(b.normals()) {
        let (a_lo, a_hi) = a.project(n);
        let (b_lo, b_hi) = b.project(n);
        let g = gap(n);
        let (forward, back) = (a_hi - b_lo + g, b_hi - a_lo + g);
        let depth = forward.min(back);
        if depth <= 0.0 {
            return None;
        }
        // Equal both ways (one middle over the other): push `b` toward the
        // larger coordinates, as the rectangle solver always did.
        let normal = if forward < back || (forward == back && n.x + n.y >= 0.0) {
            n
        } else {
            -n
        };
        let hit = Hit { normal, depth };
        match best {
            Some(h) if depth > h.depth + TIE => {}
            Some(h) if depth >= h.depth - TIE => {
                if prefer == Vec2::ZERO {
                    tied += normal;
                    best = Some(Hit {
                        depth: h.depth.min(depth),
                        ..h
                    });
                } else if normal.dot(prefer) > h.normal.dot(prefer) + TIE {
                    best = Some(hit);
                }
            }
            _ => {
                best = Some(hit);
                tied = normal;
            }
        }
    }
    let len = tied.length();
    best.map(|h| {
        if prefer == Vec2::ZERO && len > 1e-3 {
            Hit {
                normal: tied * (1.0 / len),
                ..h
            }
        } else {
            h
        }
    })
}
