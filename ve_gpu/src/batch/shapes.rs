//! The shape batch's shapes (GPU-2): what a caller describes, and the 64-byte instance the shader reads.

use bytemuck::{Pod, Zeroable};
use egui::{Color32, Pos2, Rect, Stroke};

pub(crate) const KIND_RECT: u32 = 0;
pub(crate) const KIND_SEGMENT: u32 = 1;
pub(crate) const KIND_DIAMOND: u32 = 2;
pub(crate) const KIND_HATCH: u32 = 3;
pub(crate) const FLAG_SNAP: u32 = 256;

/// One shape as `shapes.wgsl` reads it: see `ShapeIn` there.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub(crate) struct ShapeInstance {
    pub(crate) a: [f32; 4],
    pub(crate) clip: [f32; 4],
    pub(crate) p: [f32; 4],
    pub(crate) c: [u32; 4],
}

const _: () = assert!(
    size_of::<ShapeInstance>() == 64,
    "shapes.wgsl reads four vec4s"
);

/// Bytes of one shape instance.
pub(crate) const SHAPE_BYTES: u64 = size_of::<ShapeInstance>() as u64;

pub(crate) fn rgba(c: Color32) -> u32 {
    u32::from_le_bytes(c.to_array())
}

pub(crate) fn rect4(r: Rect) -> [f32; 4] {
    [r.min.x, r.min.y, r.max.x, r.max.y]
}

/// A rect's look: fill, a stroke inside its edge, corners.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RectStyle {
    /// The fill inside the stroke.
    pub fill: Color32,
    /// The stroke, drawn inside the rect's edge (egui's `StrokeKind::Inside`); width in points.
    pub stroke: Stroke,
    /// The corner radius, points.
    pub corner_radius: f32,
    /// Round the corners to whole pixels as egui does for its rects (on by default).
    pub snap: bool,
}

impl RectStyle {
    /// A filled rect without a stroke, square cornered.
    pub fn filled(fill: Color32) -> Self {
        Self {
            fill,
            stroke: Stroke::NONE,
            corner_radius: 0.0,
            snap: true,
        }
    }

    /// An outline only.
    pub fn stroked(stroke: Stroke) -> Self {
        Self {
            fill: Color32::TRANSPARENT,
            ..Self::filled(Color32::TRANSPARENT)
        }
        .with_stroke(stroke)
    }

    /// The same with `stroke`.
    #[must_use]
    pub fn with_stroke(mut self, stroke: Stroke) -> Self {
        self.stroke = stroke;
        self
    }

    /// The same with corners of `radius` points.
    #[must_use]
    pub fn with_corner_radius(mut self, radius: f32) -> Self {
        self.corner_radius = radius;
        self
    }
}

/// Dashes along a segment: drawn `length`, then a `gap`, from the segment's start; points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dash {
    /// The length of a dash, points.
    pub length: f32,
    /// The gap after a dash, points.
    pub gap: f32,
}

/// A segment's look.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineStyle {
    /// Width (points) and colour.
    pub stroke: Stroke,
    /// Dashes; `None` is a solid line.
    pub dash: Option<Dash>,
    /// Place an axis-aligned segment on pixels as egui's tessellator does: across on pixel centres (edges for an
    /// even width), a solid one shortened by a quarter pixel at each end (on by default).
    pub snap: bool,
}

impl LineStyle {
    /// A solid line.
    pub fn solid(stroke: Stroke) -> Self {
        Self {
            stroke,
            dash: None,
            snap: true,
        }
    }

    /// A dashed line.
    pub fn dashed(stroke: Stroke, dash: Dash) -> Self {
        Self {
            dash: Some(dash),
            ..Self::solid(stroke)
        }
    }
}

/// Parallel lines across a rect, clipped to it, in bands of whole pixels.
///
/// A line is the set of pixel centres where `x + slant * y` lies within `stroke.width` past a line through
/// `anchor`, repeated every `period` along x; all in pixels, with the anchor rounded to a pixel. A slant of 1 rises
/// to the right at 45 degrees, -1 falls. The anchor lets a caller keep the lines in phase with its own axis while
/// the rect moves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hatch {
    /// Line width (points) and colour.
    pub stroke: Stroke,
    /// The distance between lines along x, points; above zero.
    pub period: f32,
    /// `dx / dy` of the lines: x moves this much back per point down.
    pub slant: f32,
    /// A point a line passes through.
    pub anchor: Pos2,
}

/// The clip that lets everything through.
pub(crate) const CLIP_NONE: [f32; 4] = [-1.0e9, -1.0e9, 1.0e9, 1.0e9];

pub(crate) fn rect(r: Rect, style: &RectStyle, clip: [f32; 4]) -> Option<ShapeInstance> {
    let width = if style.stroke.color == Color32::TRANSPARENT {
        0.0
    } else {
        style.stroke.width.max(0.0)
    };
    if style.fill == Color32::TRANSPARENT && width == 0.0 || !r.is_finite() {
        return None;
    }
    Some(ShapeInstance {
        a: rect4(r),
        clip,
        p: [width, style.corner_radius.max(0.0), 0.0, 0.0],
        c: [
            KIND_RECT | snap(style.snap),
            rgba(style.fill),
            rgba(style.stroke.color),
            0,
        ],
    })
}

pub(crate) fn segment(
    [a, b]: [Pos2; 2],
    style: &LineStyle,
    clip: [f32; 4],
) -> Option<ShapeInstance> {
    if style.stroke.is_empty() || !(a.is_finite() && b.is_finite()) {
        return None;
    }
    let (dash, gap) = style
        .dash
        .filter(|d| d.length > 0.0 && d.gap >= 0.0)
        .map_or((0.0, 0.0), |d| (d.length, d.gap));
    Some(ShapeInstance {
        a: [a.x, a.y, b.x, b.y],
        clip,
        p: [style.stroke.width, 0.0, dash, gap],
        c: [
            KIND_SEGMENT | snap(style.snap),
            0,
            rgba(style.stroke.color),
            0,
        ],
    })
}

pub(crate) fn diamond(
    centre: Pos2,
    half: f32,
    fill: Color32,
    stroke: Stroke,
    clip: [f32; 4],
) -> Option<ShapeInstance> {
    let width = if stroke.color == Color32::TRANSPARENT {
        0.0
    } else {
        stroke.width.max(0.0)
    };
    if fill == Color32::TRANSPARENT && width == 0.0
        || !centre.is_finite()
        || half.is_nan()
        || half < 0.0
    {
        return None;
    }
    Some(ShapeInstance {
        a: [centre.x, centre.y, half, 0.0],
        clip,
        p: [width, 0.0, 0.0, 0.0],
        c: [KIND_DIAMOND, rgba(fill), rgba(stroke.color), 0],
    })
}

pub(crate) fn hatch(r: Rect, h: &Hatch, clip: [f32; 4]) -> Option<ShapeInstance> {
    if h.stroke.is_empty()
        || h.period.is_nan()
        || h.period <= 0.0
        || !h.slant.is_finite()
        || !r.is_finite()
        || !h.anchor.is_finite()
    {
        return None;
    }
    Some(ShapeInstance {
        a: rect4(r),
        clip,
        p: [h.stroke.width, h.period, h.anchor.x, h.anchor.y],
        c: [KIND_HATCH, 0, rgba(h.stroke.color), h.slant.to_bits()],
    })
}

fn snap(on: bool) -> u32 {
    if on { FLAG_SNAP } else { 0 }
}
