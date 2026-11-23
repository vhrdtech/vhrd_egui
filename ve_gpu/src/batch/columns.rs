//! Range columns (GPU-3): per column of pixels a value's lowest, highest and mean, mapped and drawn in the shader.

use bytemuck::{Pod, Zeroable};
use egui::{Color32, Rect, Stroke};

use super::shapes::{rect4, rgba};

/// One column: where it is along x and its values' lowest, highest and mean.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct Column {
    /// x of the column, points.
    pub x: f32,
    /// The lowest value.
    pub min: f32,
    /// The highest value.
    pub max: f32,
    /// The mean value.
    pub mean: f32,
}

/// How a run of columns maps values to the screen and what it draws.
///
/// Values map linearly: `value_lo` lands on `y_lo`, `value_hi` on `y_hi` (points; `y_lo` below `y_hi` on screen
/// for values growing upwards). Each part is drawn only when its colour is not transparent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColumnStyle {
    /// What the run may draw over, points; the painter's clip applies as well.
    pub clip: Rect,
    /// The value that lands on `y_lo`.
    pub value_lo: f32,
    /// The value that lands on `y_hi`; equal to `value_lo`, every value lands halfway between the two.
    pub value_hi: f32,
    /// y of `value_lo`, points.
    pub y_lo: f32,
    /// y of `value_hi`, points.
    pub y_hi: f32,
    /// The band from the lowest to the highest value, in whole pixels, at least one pixel high.
    pub band: Color32,
    /// The two edges along the lowest and the highest values: egui's one-pixel line, mitred at joins.
    pub edge: Color32,
    /// The mean line; width in points.
    pub mean: Stroke,
}

impl ColumnStyle {
    /// The y of `value`, points.
    pub fn y(&self, value: f32) -> f32 {
        let (v0, scale, y0) = self.map();
        y0 + (value - v0) * scale
    }

    /// `(v0, scale, y0)`: y = y0 + (value - v0) * scale.
    fn map(&self) -> (f32, f32, f32) {
        let span = self.value_hi - self.value_lo;
        if span == 0.0 || !span.is_finite() {
            (self.value_lo, 0.0, f32::midpoint(self.y_lo, self.y_hi))
        } else {
            (self.value_lo, (self.y_hi - self.y_lo) / span, self.y_lo)
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
    spare: [f32; 4],
}

/// Bytes of a run's uniform.
pub(crate) const RUN_BYTES: u64 = size_of::<RunUniform>() as u64;
/// Bytes of a column.
pub(crate) const COLUMN_BYTES: u64 = size_of::<Column>() as u64;

impl RunUniform {
    pub(crate) fn new(style: &ColumnStyle, columns: u32) -> Self {
        let (v0, scale, y0) = style.map();
        let mean = if style.mean.is_empty() {
            Color32::TRANSPARENT
        } else {
            style.mean.color
        };
        Self {
            clip: rect4(style.clip),
            map: [v0, scale, y0, style.mean.width.max(0.0)],
            colours: [rgba(style.band), rgba(mean), rgba(style.edge), columns],
            spare: [0.0; 4],
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

/// Folds `col` into the band's pixel columns: the columns whose x falls in one pixel make one band column, from
/// the pixel the lowest end of any of them falls in to the one the highest end does, each column at least a pixel
/// high about its middle. A band column reaches to the next one, so columns further apart than a pixel leave no
/// gaps; the last is a pixel wide. Band columns are (x, top, bottom, width), points.
pub(crate) fn fold(
    band: &mut Vec<Column>,
    start: usize,
    style: &ColumnStyle,
    pixels_per_point: f32,
    col: &Column,
) {
    let (a, b) = (style.y(col.min), style.y(col.max));
    let mid = f32::midpoint(a, b);
    let half_px = 0.5 / pixels_per_point;
    let (top, bottom) = (a.min(b).min(mid - half_px), a.max(b).max(mid + half_px));
    let x = (col.x * pixels_per_point).floor() / pixels_per_point;
    let fresh = band.len() <= start;
    match band.last_mut() {
        Some(last) if !fresh && last.x == x => {
            last.min = last.min.min(top);
            last.max = last.max.max(bottom);
            return;
        }
        Some(last) if !fresh && x > last.x => last.mean = x - last.x,
        _ => {}
    }
    band.push(Column {
        x,
        min: top,
        max: bottom,
        mean: 1.0 / pixels_per_point,
    });
}
