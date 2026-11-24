//! One scene per primitive, filled into a batch over a rect, in the theme's colours.

use egui::{Color32, Pos2, Rect, Stroke, vec2};
use ve_gpu::{Axis, Batch, Column, ColumnStyle, Dash, Hatch, LineStyle, RectStyle};
use ve_theme::Palette;

/// A scene: its name and what it fills.
pub type Scene = (&'static str, fn(&mut Batch, Rect, &Palette, f32));

pub const ALL: [Scene; 5] = [
    ("rects", rects),
    ("segments", segments),
    ("diamonds", diamonds),
    ("hatch", hatch),
    ("columns", columns),
];

fn at(r: Rect, x: f32, y: f32) -> Pos2 {
    r.min + vec2(x, y)
}

pub fn rects(b: &mut Batch, r: Rect, c: &Palette, _ppp: f32) {
    let cell = |x: f32, y: f32, w: f32, h: f32| Rect::from_min_size(at(r, x, y), vec2(w, h));
    b.rect(cell(8.0, 8.0, 40.0, 24.0), &RectStyle::filled(c.accent));
    b.rect(
        cell(56.0, 8.0, 40.0, 24.0),
        &RectStyle::stroked(Stroke::new(1.0, c.text)),
    );
    b.rect(
        cell(104.0, 8.0, 40.0, 24.0),
        &RectStyle::filled(c.cat[1])
            .with_stroke(Stroke::new(2.0, c.text))
            .with_corner_radius(6.0),
    );
    b.rect(
        cell(152.0, 8.0, 40.0, 24.0),
        &RectStyle::filled(c.warn).with_corner_radius(12.0),
    );
    // fractional edges, snapped and not
    b.rect(cell(8.3, 44.6, 40.4, 24.2), &RectStyle::filled(c.cat[2]));
    let mut soft = RectStyle::filled(c.cat[2]);
    soft.snap = false;
    b.rect(cell(56.3, 44.6, 40.4, 24.2), &soft);
    // see-through over another
    b.rect(cell(104.0, 44.0, 40.0, 24.0), &RectStyle::filled(c.crit));
    b.rect(
        cell(120.0, 52.0, 40.0, 24.0),
        &RectStyle::filled(c.accent_alt.gamma_multiply(0.5)),
    );
}

pub fn segments(b: &mut Batch, r: Rect, c: &Palette, _ppp: f32) {
    let line = |w: f32, col: Color32| LineStyle::solid(Stroke::new(w, col));
    for (k, w) in [1.0, 1.3, 2.0, 3.0].into_iter().enumerate() {
        let y = 10.0 + 12.0 * k as f32;
        b.segment([at(r, 8.0, y), at(r, 100.0, y)], &line(w, c.text));
        let x = 112.0 + 10.0 * k as f32;
        b.segment([at(r, x, 8.0), at(r, x, 60.0)], &line(w, c.accent));
    }
    b.segment(
        [at(r, 160.0, 8.0), at(r, 230.0, 70.0)],
        &line(1.5, c.cat[3]),
    );
    b.segment(
        [at(r, 8.0, 70.0), at(r, 150.0, 70.0)],
        &LineStyle::dashed(
            Stroke::new(1.0, c.text_muted),
            Dash {
                length: 3.0,
                gap: 3.0,
            },
        ),
    );
    b.segment(
        [at(r, 8.0, 80.0), at(r, 150.0, 100.0)],
        &LineStyle::dashed(
            Stroke::new(2.0, c.warn),
            Dash {
                length: 6.0,
                gap: 2.0,
            },
        ),
    );
}

pub fn diamonds(b: &mut Batch, r: Rect, c: &Palette, _ppp: f32) {
    for (k, half) in [2.0, 3.5, 6.0, 10.0, 16.0].into_iter().enumerate() {
        let x = 16.0 + 40.0 * k as f32;
        b.diamond(
            at(r, x, 24.0),
            half,
            c.cat[k % 5],
            Stroke::new(1.0, c.cat[k % 5]),
        );
        b.diamond(
            at(r, x + 0.5, 64.0),
            half,
            Color32::TRANSPARENT,
            Stroke::new(1.0, c.text),
        );
    }
}

pub fn hatch(b: &mut Batch, r: Rect, c: &Palette, _ppp: f32) {
    let a = Rect::from_min_size(at(r, 8.0, 8.0), vec2(100.0, 40.0));
    b.rect(a, &RectStyle::filled(c.surface_raised));
    b.hatch(
        a,
        &Hatch {
            stroke: Stroke::new(1.0, c.accent),
            period: 4.0,
            slant: 1.0,
            anchor: a.left_bottom(),
        },
    );
    let d = Rect::from_min_size(at(r, 120.0, 8.0), vec2(100.0, 40.0));
    b.hatch(
        d,
        &Hatch {
            stroke: Stroke::new(2.0, c.text_muted),
            period: 8.0,
            slant: -1.0,
            anchor: d.left_top(),
        },
    );
    // the same phase in two rects: lines continue from one into the next
    for x in [8.0, 60.0] {
        let e = Rect::from_min_size(at(r, x, 60.0), vec2(48.0, 30.0));
        b.hatch(
            e,
            &Hatch {
                stroke: Stroke::new(1.0, c.warn),
                period: 6.0,
                slant: 0.5,
                anchor: at(r, 0.0, 0.0),
            },
        );
    }
}

/// A deterministic wave: lowest, highest and mean per column.
pub fn wave(n: usize, x0: f32, step: f32) -> impl Iterator<Item = Column> {
    (0..n).map(move |i| {
        let t = i as f32 * 0.05;
        let mid = t.sin() * 0.6 + (t * 3.7).sin() * 0.2;
        let spread = 0.1 + 0.25 * (t * 0.7).cos().abs();
        Column {
            along: x0 + i as f32 * step,
            min: mid - spread,
            max: mid + spread * 0.7,
            mean: mid,
        }
    })
}

pub fn columns(b: &mut Batch, r: Rect, c: &Palette, ppp: f32) {
    let lane = Rect::from_min_size(at(r, 4.0, 4.0), vec2(r.width() - 8.0, r.height() - 8.0));
    b.rect(lane, &RectStyle::stroked(Stroke::new(1.0, c.line)));
    let style = ColumnStyle {
        axis: Axis::X,
        clip: lane.shrink(1.0),
        value_lo: -1.0,
        value_hi: 1.0,
        across_lo: lane.bottom() - 4.0,
        across_hi: lane.top() + 4.0,
        band: c.accent.gamma_multiply(0.45),
        edge: Stroke::new(1.0, c.accent.gamma_multiply(0.8)),
        mean: Stroke::new(1.2, c.accent),
    };
    // two columns a point, as a zoomed-out series
    let n = ((lane.width() - 2.0) * 2.0) as usize;
    b.columns(&style, ppp, wave(n, lane.left() + 1.0, 0.5));
    // a second run, the band alone, flat values in a narrow strip
    let strip = ColumnStyle {
        value_lo: 0.0,
        value_hi: 0.0,
        across_lo: lane.bottom() - 6.0,
        across_hi: lane.bottom() - 2.0,
        band: c.warn,
        edge: Stroke::NONE,
        mean: Stroke::NONE,
        ..style
    };
    b.columns(
        &strip,
        ppp,
        (0..40).map(|i| Column {
            along: lane.left() + 10.0 + i as f32,
            ..Column::default()
        }),
    );
}
