//! Range columns (GPU-3): per column of pixels a value's lowest, highest and mean, mapped and drawn in the shader.

use bytemuck::{Pod, Zeroable};
use egui::{Color32, Rect, Stroke};

use super::shapes::{rect4, rgba};

/// One column: where it is along the run's axis and its values' lowest, highest and mean.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct Column {
    /// Where the column is along the run's axis ([`ColumnStyle::axis`]), points: x across, y down.
    pub along: f32,
    /// The lowest value.
    pub min: f32,
    /// The highest value.
    pub max: f32,
    /// The mean value.
    pub mean: f32,
}

/// The screen axis a run of columns follows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Axis {
    /// Columns left to right along x, values on y.
    #[default]
    X,
    /// Columns top to bottom along y, values on x.
    Y,
}

/// How a run of columns maps values to the screen and what it draws.
///
/// The columns follow `axis`; values map linearly across it: `value_lo` lands on `across_lo`, `value_hi` on
/// `across_hi` (points: y for [`Axis::X`], `across_lo` below `across_hi` on screen for values growing upwards; x
/// for [`Axis::Y`]). Each part is drawn only when its colour is not transparent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColumnStyle {
    /// The screen axis the columns follow.
    pub axis: Axis,
    /// What the run may draw over, points; the painter's clip applies as well.
    pub clip: Rect,
    /// The value that lands on `across_lo`.
    pub value_lo: f32,
    /// The value that lands on `across_hi`; equal to `value_lo`, every value lands halfway between the two.
    pub value_hi: f32,
    /// Where `value_lo` lands across the axis, points.
    pub across_lo: f32,
    /// Where `value_hi` lands across the axis, points.
    pub across_hi: f32,
    /// The band from the lowest to the highest value, in whole pixels, at least one pixel high.
    pub band: Color32,
    /// The two edges along the lowest and the highest values, as egui strokes an open line of this width in points
    /// with a pixel of feathering, mitred at joins.
    pub edge: Stroke,
    /// The mean line; width in points.
    pub mean: Stroke,
}

impl ColumnStyle {
    /// Where `value` lands across the axis, points.
    pub fn across(&self, value: f32) -> f32 {
        let (v0, scale, y0) = self.map();
        y0 + (value - v0) * scale
    }

    /// `(v0, scale, a0)`: across = a0 + (value - v0) * scale.
    fn map(&self) -> (f32, f32, f32) {
        let span = self.value_hi - self.value_lo;
        if span == 0.0 || !span.is_finite() {
            (
                self.value_lo,
                0.0,
                f32::midpoint(self.across_lo, self.across_hi),
            )
        } else {
            (
                self.value_lo,
                (self.across_hi - self.across_lo) / span,
                self.across_lo,
            )
        }
    }
}

/// One run's uniform as `columns.wgsl` reads it: see `Run` there.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub(crate) struct RunUniform {
    clip: [f32; 4],
    map: [f32; 4],
    colours: [u32; 4],
    /// x: 1 when the columns follow y; y: the edges' width, points.
    axis: [f32; 4],
}

/// Bytes of a run's uniform.
pub(crate) const RUN_BYTES: u64 = size_of::<RunUniform>() as u64;
/// Bytes of a column.
pub(crate) const COLUMN_BYTES: u64 = size_of::<Column>() as u64;

impl RunUniform {
    pub(crate) fn new(style: &ColumnStyle, columns: u32) -> Self {
        let (v0, scale, y0) = style.map();
        let shown = |s: &Stroke| {
            if s.is_empty() {
                Color32::TRANSPARENT
            } else {
                s.color
            }
        };
        Self {
            clip: rect4(style.clip),
            map: [v0, scale, y0, style.mean.width.max(0.0)],
            colours: [
                rgba(style.band),
                rgba(shown(&style.mean)),
                rgba(shown(&style.edge)),
                columns,
            ],
            axis: [
                f32::from(u8::from(style.axis == Axis::Y)),
                style.edge.width.max(0.0),
                0.0,
                0.0,
            ],
        }
    }

    pub(crate) fn has_band(&self) -> bool {
        self.colours[0] != 0
    }

    pub(crate) fn has_mean(&self) -> bool {
        self.colours[1] != 0
    }

    pub(crate) fn has_edges(&self) -> bool {
        self.colours[2] != 0
    }
}

/// Folds `col` into the band's pixel columns: the columns whose place along the axis falls in one pixel make one
/// band column, from the pixel the lowest end of any of them falls in to the one the highest end does, each column
/// at least a pixel long about its middle. A band column reaches to the next one, so columns further apart than a
/// pixel leave no gaps; the last is a pixel wide. Band columns are (along, across from, across to, width), points.
pub(crate) fn fold(
    band: &mut Vec<Column>,
    start: usize,
    style: &ColumnStyle,
    pixels_per_point: f32,
    col: &Column,
) {
    let (a, b) = (style.across(col.min), style.across(col.max));
    let mid = f32::midpoint(a, b);
    let half_px = 0.5 / pixels_per_point;
    let (top, bottom) = (a.min(b).min(mid - half_px), a.max(b).max(mid + half_px));
    let x = (col.along * pixels_per_point).floor() / pixels_per_point;
    let fresh = band.len() <= start;
    match band.last_mut() {
        Some(last) if !fresh && last.along == x => {
            last.min = last.min.min(top);
            last.max = last.max.max(bottom);
            return;
        }
        Some(last) if !fresh && x > last.along => last.mean = x - last.along,
        _ => {}
    }
    band.push(Column {
        along: x,
        min: top,
        max: bottom,
        mean: 1.0 / pixels_per_point,
    });
}
