//! The scenes ve_gpu's benchmarks draw: batches of about 1000 shapes and runs of dense range columns, the same
//! every run.

use egui::{Color32, Pos2, Rect, Stroke, pos2, vec2};
use ve_gpu::{Axis, Batch, Column, ColumnStyle, Dash, Hatch, LineStyle, RectStyle};

/// Shapes a dense batch holds, about.
pub const SHAPES_PER_BATCH: usize = 1000;

/// A small deterministic generator (an LCG), so every run draws the same.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> f32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
}

const COLOURS: [Color32; 4] = [
    Color32::from_rgb(120, 90, 220),
    Color32::from_rgb(40, 170, 160),
    Color32::from_rgb(220, 150, 40),
    Color32::from_rgb(210, 70, 120),
];

/// Batch `k` of a dense frame in `rect`: about [`SHAPES_PER_BATCH`] shapes, a mix of 60 % rects, 35 % segments
/// and the rest diamonds and hatching, as a busy row of glyphs.
pub fn dense(batch: &mut Batch, rect: Rect, k: usize) {
    let mut r = Lcg(k as u64 + 1);
    batch.set_clip(rect);
    let (w, h) = (rect.width(), rect.height());
    let mut x = rect.left();
    for i in 0..SHAPES_PER_BATCH {
        let c = COLOURS[i % COLOURS.len()];
        x += w / SHAPES_PER_BATCH as f32;
        let roll = r.next();
        let y = rect.top() + h * (0.2 + 0.6 * r.next());
        if roll < 0.6 {
            let bar = Rect::from_min_max(pos2(x, y), pos2(x + 1.6, rect.bottom() - 1.0));
            let style = if i % 7 == 0 {
                RectStyle::filled(c.gamma_multiply(0.4))
                    .with_stroke(Stroke::new(1.0, c))
                    .with_corner_radius(2.0)
            } else {
                RectStyle::filled(c.gamma_multiply(0.55))
            };
            batch.rect(bar, &style);
        } else if roll < 0.95 {
            let style = if i % 5 == 0 {
                LineStyle::dashed(
                    Stroke::new(1.0, c),
                    Dash {
                        length: 3.0,
                        gap: 3.0,
                    },
                )
            } else {
                LineStyle::solid(Stroke::new(1.0, c))
            };
            batch.segment([pos2(x, y), pos2(x, rect.bottom() - 1.0)], &style);
        } else if roll < 0.98 {
            batch.diamond(pos2(x, y), 3.5, c, Stroke::new(1.0, c));
        } else {
            let span = Rect::from_min_max(
                pos2(x, rect.top() + 2.0),
                pos2(x + 12.0, rect.bottom() - 2.0),
            );
            batch.hatch(
                span,
                &Hatch {
                    stroke: Stroke::new(1.0, c),
                    period: 4.0,
                    slant: 1.0,
                    anchor: rect.left_bottom(),
                },
            );
        }
    }
    batch.reset_clip();
}

/// Run `k` of a series frame in `rect`: two columns a point, band, edges and mean.
pub fn series(batch: &mut Batch, rect: Rect, k: usize, pixels_per_point: f32) {
    let style = ColumnStyle {
        axis: Axis::X,
        clip: rect,
        value_lo: -1.0,
        value_hi: 1.0,
        across_lo: rect.bottom() - 2.0,
        across_hi: rect.top() + 2.0,
        band: COLOURS[k % COLOURS.len()].gamma_multiply(0.45),
        edge: Stroke::new(1.0, COLOURS[k % COLOURS.len()].gamma_multiply(0.8)),
        mean: Stroke::new(1.2, COLOURS[k % COLOURS.len()]),
    };
    let n = (rect.width() * 2.0) as usize;
    let phase = k as f32 * 0.7;
    let columns = (0..n).map(move |i| {
        let t = i as f32 * 0.01 + phase;
        let mid = t.sin() * 0.5 + (t * 7.3).sin() * 0.2;
        let spread = 0.05 + 0.2 * (t * 1.7).cos().abs();
        Column {
            along: rect.left() + i as f32 * 0.5,
            min: mid - spread,
            max: mid + spread,
            mean: mid,
        }
    });
    batch.columns(&style, pixels_per_point, columns);
}

/// The row of batch `k` among `n` stacked in `screen`.
pub fn row(screen: Rect, k: usize, n: usize) -> Rect {
    let h = screen.height() / n as f32;
    Rect::from_min_size(
        Pos2::new(screen.left(), screen.top() + h * k as f32),
        vec2(screen.width(), h),
    )
}
