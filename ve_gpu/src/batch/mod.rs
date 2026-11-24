//! [`Batch`]: shapes and range columns a widget draws through one paint callback, in order.

pub(crate) mod columns;
pub(crate) mod shapes;

use std::ops::Range;
use std::sync::Arc;

use egui::mutex::Mutex;
use egui::{Color32, Pos2, Rect, Stroke};

use crate::plumbing::RetainedCallback;
use crate::renderer::BatchCallback;
use columns::{Axis, Column, ColumnStyle, RunUniform, fold};
use shapes::{CLIP_NONE, Hatch, LineStyle, RectStyle, ShapeInstance, rect4};

/// One draw of a batch, in paint order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Item {
    /// Shapes `[start, end)` of [`BatchData::shapes`].
    Shapes(Range<u32>),
    /// Run `k` of [`BatchData::runs`].
    Columns(u32),
}

/// A run of columns: its uniform, its columns in [`BatchData::columns`] (with a copy of the first before and of
/// the last after them, for the edges' joins), and its band's pixel columns there.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Run {
    pub(crate) uniform: RunUniform,
    pub(crate) columns: Range<u32>,
    pub(crate) band: Range<u32>,
}

/// What a batch draws.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct BatchData {
    pub(crate) shapes: Vec<ShapeInstance>,
    pub(crate) columns: Vec<Column>,
    pub(crate) runs: Vec<Run>,
    pub(crate) items: Vec<Item>,
}

impl BatchData {
    fn clear(&mut self) {
        self.shapes.clear();
        self.columns.clear();
        self.runs.clear();
        self.items.clear();
    }

    /// Becomes a copy of `other`, in the room it already has.
    pub(crate) fn copy_from(&mut self, other: &Self) {
        self.clear();
        self.shapes.extend_from_slice(&other.shapes);
        self.columns.extend_from_slice(&other.columns);
        self.runs.extend(other.runs.iter().cloned());
        self.items.extend_from_slice(&other.items);
    }

    /// Draw calls this data issues.
    pub(crate) fn draws(&self) -> usize {
        self.items
            .iter()
            .map(|item| match item {
                Item::Shapes(_) => 1,
                Item::Columns(k) => self.runs.get(*k as usize).map_or(0, |r| {
                    let n = r.columns.len();
                    let u = &r.uniform;
                    usize::from(u.has_band() && !r.band.is_empty())
                        + 2 * usize::from(u.has_edges() && n > 1)
                        + usize::from(u.has_mean() && n > 1)
                }),
            })
            .sum()
    }
}

/// Shapes and range columns drawn through one paint callback, in the order they were added.
///
/// Keep a batch with the widget and fill it again each frame: [`Batch::clear`], add what to draw, then add
/// [`Batch::paint`]'s shape to egui's painter. All batches painted in a frame go to the GPU in one write per
/// buffer; a warm frame allocates nothing, neither here nor in the callback. Positions are egui points; the
/// shader turns them into pixels at the frame's pixels per point and antialiases every edge.
///
/// Drawing needs [`crate::install`] on the app's wgpu render state; without it a batch draws nothing.
///
/// ```
/// # fn f(ui: &mut egui::Ui, batch: &mut ve_gpu::Batch) {
/// use egui::{Color32, Stroke};
/// use ve_gpu::{LineStyle, RectStyle};
///
/// let rect = ui.max_rect();
/// batch.clear();
/// batch.rect(rect.shrink(4.0), &RectStyle::filled(Color32::DARK_BLUE).with_corner_radius(3.0));
/// batch.segment([rect.left_center(), rect.right_center()], &LineStyle::solid(Stroke::new(1.0, Color32::WHITE)));
/// ui.painter().add(batch.paint(rect));
/// # }
/// ```
pub struct Batch {
    data: BatchData,
    /// The clip of shapes and runs added from now on.
    clip: [f32; 4],
    shared: Arc<Mutex<Shared>>,
    callback: RetainedCallback,
}

/// What the callback reads: the batch as last painted, where the renderer placed it, and the frame it was queued in.
#[derive(Debug, Default)]
pub(crate) struct Shared {
    pub(crate) data: BatchData,
    pub(crate) placed: Placed,
    pub(crate) frame: u64,
}

/// Where a batch's data landed in the renderer's buffers this frame, bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Placed {
    pub(crate) shapes: u64,
    pub(crate) columns: u64,
    pub(crate) runs: u64,
}

impl Default for Batch {
    fn default() -> Self {
        let shared = Arc::new(Mutex::new(Shared::default()));
        Self {
            data: BatchData::default(),
            clip: CLIP_NONE,
            callback: RetainedCallback::new(BatchCallback(shared.clone())),
            shared,
        }
    }
}

impl std::fmt::Debug for Batch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Batch")
            .field("shapes", &self.data.shapes.len())
            .field("columns", &self.data.columns.len())
            .field("runs", &self.data.runs.len())
            .finish_non_exhaustive()
    }
}

impl Batch {
    /// An empty batch.
    pub fn new() -> Self {
        Self::default()
    }

    /// Empties the batch and lifts its clip, keeping its room.
    pub fn clear(&mut self) {
        self.data.clear();
        self.clip = CLIP_NONE;
    }

    /// Whether nothing was added since the last [`Batch::clear`].
    pub fn is_empty(&self) -> bool {
        self.data.items.is_empty()
    }

    /// Shapes added since the last [`Batch::clear`].
    pub fn shape_count(&self) -> usize {
        self.data.shapes.len()
    }

    /// Clips what is added from now on to `clip` (points), within the painter's clip.
    pub fn set_clip(&mut self, clip: Rect) {
        self.clip = rect4(clip);
    }

    /// Lifts the clip set by [`Batch::set_clip`].
    pub fn reset_clip(&mut self) {
        self.clip = CLIP_NONE;
    }

    /// A rect with its fill, inner stroke and corners.
    pub fn rect(&mut self, rect: Rect, style: &RectStyle) {
        self.push(shapes::rect(rect, style, self.clip));
    }

    /// A segment from `points[0]` to `points[1]`, ends cut square half a pixel past the points.
    pub fn segment(&mut self, points: [Pos2; 2], style: &LineStyle) {
        self.push(shapes::segment(points, style, self.clip));
    }

    /// A diamond (a square turned 45 degrees) about `centre`, its corners `half` points from it; `stroke` is
    /// centred on its edge.
    pub fn diamond(&mut self, centre: Pos2, half: f32, fill: Color32, stroke: Stroke) {
        self.push(shapes::diamond(centre, half, fill, stroke, self.clip));
    }

    /// Hatching across `rect`, clipped to it.
    pub fn hatch(&mut self, rect: Rect, hatch: &Hatch) {
        self.push(shapes::hatch(rect, hatch, self.clip));
    }

    /// A rect filled from `from` at its start to `to` at its end along `axis` (left to right, or top to bottom),
    /// corners on whole pixels.
    pub fn gradient(&mut self, rect: Rect, axis: Axis, from: Color32, to: Color32) {
        self.push(shapes::gradient(rect, axis, [from, to], self.clip));
    }

    /// A run of range columns in order along the style's axis, one per pixel column or more: the band folds the
    /// columns of one pixel into one, at `pixels_per_point` (egui's `Context::pixels_per_point` of the frame); the
    /// edges and the mean line pass through every column. The batch's clip and `style.clip` both apply. Nothing is
    /// drawn for an empty run.
    pub fn columns(
        &mut self,
        style: &ColumnStyle,
        pixels_per_point: f32,
        columns: impl IntoIterator<Item = Column>,
    ) {
        let ppp = if pixels_per_point > 0.0 && pixels_per_point.is_finite() {
            pixels_per_point
        } else {
            1.0
        };
        let cols = &mut self.data.columns;
        let pad = cols.len();
        cols.push(Column::default());
        let start = cols.len();
        cols.extend(columns);
        let end = cols.len();
        if end == start {
            cols.truncate(pad);
            return;
        }
        let (first, last) = (cols[start], cols[end - 1]);
        cols[pad] = first;
        cols.push(last);
        let band = cols.len();
        for k in start..end {
            let c = cols[k];
            fold(cols, band, style, ppp, &c);
        }
        let mut style = *style;
        style.clip = style.clip.intersect(Rect::from_min_max(
            Pos2::new(self.clip[0], self.clip[1]),
            Pos2::new(self.clip[2], self.clip[3]),
        ));
        let n = (end - start) as u32;
        self.data
            .items
            .push(Item::Columns(self.data.runs.len() as u32));
        self.data.runs.push(Run {
            uniform: RunUniform::new(&style, n),
            columns: start as u32..end as u32,
            band: band as u32..self.data.columns.len() as u32,
        });
    }

    /// The shape that draws the batch as it is now over `rect` (points), to add to egui's painter. The batch is
    /// copied for the callback, so it may change or be painted again afterwards.
    pub fn paint(&self, rect: Rect) -> egui::Shape {
        self.shared.lock().data.copy_from(&self.data);
        self.callback.shape(rect)
    }

    fn push(&mut self, shape: Option<ShapeInstance>) {
        let Some(shape) = shape else { return };
        let k = self.data.shapes.len() as u32;
        self.data.shapes.push(shape);
        match self.data.items.last_mut() {
            Some(Item::Shapes(r)) if r.end == k => r.end = k + 1,
            _ => self.data.items.push(Item::Shapes(k..k + 1)),
        }
    }
}
