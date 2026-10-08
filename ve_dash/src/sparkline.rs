//! Filled line chart of a short history, btop style: thin line, gradient fill,
//! crosshair with the value on hover.

use std::ops::RangeInclusive;
use std::time::Duration;

use egui::{
    Align2, Color32, FontId, Mesh, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, Vec2, Widget,
    pos2, remap_clamp, vec2,
};

use crate::theme::{DARK, load_color};

/// `Sparkline::new(history.values()).ui(ui)`; override size, range and color as needed.
///
/// One series per sparkline. The surrounding panel or label names the metric;
/// hovering shows the exact value under the cursor.
#[must_use = "pass to ui.add() or call .ui(ui)"]
pub struct Sparkline<'a> {
    values: &'a [f32],
    desired_size: Option<Vec2>,
    height: f32,
    range: Option<RangeInclusive<f32>>,
    color: Color32,
    fill: bool,
    load_max: Option<f32>,
    every: Option<Duration>,
    clock: Option<fn(i64) -> String>,
}

impl<'a> Sparkline<'a> {
    pub fn new(values: &'a [f32]) -> Self {
        Self {
            values,
            desired_size: None,
            height: 48.0,
            range: None,
            color: DARK.accent,
            fill: true,
            load_max: None,
            every: None,
            clock: None,
        }
    }

    /// Exact size. Default: available width × [`Self::height`].
    pub fn size(mut self, size: Vec2) -> Self {
        self.desired_size = Some(size);
        self
    }

    /// Height when the width comes from the available space (default 48).
    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    /// Fixed value range. Default: auto from the data with a little headroom.
    pub fn range(mut self, range: RangeInclusive<f32>) -> Self {
        self.range = Some(range);
        self
    }

    /// Line color (default: theme accent).
    pub fn color(mut self, color: Color32) -> Self {
        self.color = color;
        self
    }

    /// Gradient fill under the line (default on).
    pub fn fill(mut self, fill: bool) -> Self {
        self.fill = fill;
        self
    }

    /// Color each segment by its load, `value / max`, with [`load_color`]:
    /// the line color up to half of `max`, then warn, crit at `max`.
    /// Independent of the range, so an auto-scaled chart keeps its shape.
    pub fn load(mut self, max: f32) -> Self {
        self.load_max = (max.is_finite() && max > 0.0).then_some(max);
        self
    }

    /// The time between two samples: draws a light time axis (few ticks, `now` at the right edge, tabular
    /// digits, muted) along the bottom and puts the age of the sample under the pointer into the hover readout.
    /// Without it the chart has no time scale, as before. The axis needs room (about 26 px of height); a
    /// chart lower than that keeps just the hover time.
    ///
    /// ```ignore
    /// Sparkline::new(cpu.values()).every(Duration::from_secs(1)).height(40.0)
    /// ```
    pub fn every(mut self, every: Duration) -> Self {
        self.every = (!every.is_zero()).then_some(every);
        self
    }

    /// How to write a unix time as a wall-clock time (`21:40`; the app knows the time zone): with
    /// [`every`](Self::every), the hover readout then shows the sample's absolute time beside its age
    /// (`1.2 · -12m · 21:40`). The newest sample counts as taken now.
    pub fn clock(mut self, clock: fn(i64) -> String) -> Self {
        self.clock = Some(clock);
        self
    }

    /// Color of a point with value `v`.
    fn color_at(&self, v: f32) -> Color32 {
        match self.load_max {
            Some(max) => load_color(&DARK, self.color, v / max),
            None => self.color,
        }
    }

    fn value_range(&self) -> RangeInclusive<f32> {
        if let Some(r) = &self.range {
            return r.clone();
        }
        let (mut min, mut max) = (f32::INFINITY, f32::NEG_INFINITY);
        for &v in self.values {
            min = min.min(v);
            max = max.max(v);
        }
        if !min.is_finite() || !max.is_finite() {
            return 0.0..=1.0;
        }
        let pad = ((max - min) * 0.1).max(f32::EPSILON);
        (min - pad)..=(max + pad)
    }
}

impl Widget for Sparkline<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let size = self
            .desired_size
            .unwrap_or_else(|| vec2(ui.available_width().max(16.0), self.height));
        let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
        if !ui.is_rect_visible(rect) || rect.width() < 2.0 || rect.height() < 2.0 {
            return response;
        }
        if self.values.len() < 2 {
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                "…",
                FontId::proportional(12.0),
                DARK.text_muted,
            );
            return response;
        }

        let range = self.value_range();
        let n = self.values.len();
        let x_at =
            |i: usize| remap_clamp(i as f32, 0.0..=(n - 1) as f32, rect.left()..=rect.right());
        let y_at = |v: f32| remap_clamp(v, range.clone(), rect.bottom()..=rect.top());
        let points: Vec<Pos2> = self
            .values
            .iter()
            .enumerate()
            .map(|(i, &v)| pos2(x_at(i), y_at(v)))
            .collect();

        let painter = ui.painter().with_clip_rect(rect);
        if self.fill {
            // Per-segment quads down to the baseline; the top edge carries a faint
            // version of the line color, the bottom fades out: a vertical gradient.
            let mut mesh = Mesh::default();
            for (p, &v) in points.iter().zip(self.values) {
                mesh.colored_vertex(*p, self.color_at(v).gamma_multiply(0.25));
                mesh.colored_vertex(pos2(p.x, rect.bottom()), Color32::TRANSPARENT);
            }
            for i in 0..n - 1 {
                let a = (i * 2) as u32;
                mesh.add_triangle(a, a + 1, a + 2);
                mesh.add_triangle(a + 1, a + 3, a + 2);
            }
            painter.add(Shape::mesh(mesh));
        }
        if self.load_max.is_some() {
            // One stroke per segment, in the color of its higher end.
            for (i, w) in points.windows(2).enumerate() {
                let v = self.values[i].max(self.values[i + 1]);
                painter.line_segment([w[0], w[1]], Stroke::new(1.5, self.color_at(v)));
            }
        } else {
            painter.add(Shape::line(points.clone(), Stroke::new(1.5, self.color)));
        }

        if let (Some(every), true) = (self.every, rect.height() >= AXIS_MIN_HEIGHT) {
            let span = every.as_secs_f32() * (n - 1) as f32;
            let font = FontId::monospace(10.0);
            // Stronger than the muted text but still lighter than the data.
            let color = DARK.text_muted.lerp_to_gamma(DARK.text, 0.4);
            let y = rect.bottom() - 1.0;
            for (frac, align) in [
                (0.0, Align2::LEFT_BOTTOM),
                (0.5, Align2::CENTER_BOTTOM),
                (1.0, Align2::RIGHT_BOTTOM),
            ] {
                let x = rect.left() + frac * rect.width();
                painter.text(
                    pos2(x, y),
                    align,
                    age_label(span * (1.0 - frac)),
                    font.clone(),
                    color,
                );
                painter.vline(
                    x.clamp(rect.left() + 0.5, rect.right() - 0.5),
                    (rect.bottom() - 4.0)..=rect.bottom(),
                    Stroke::new(1.0, color),
                );
            }
        }

        if let Some(hover) = response.hover_pos() {
            let i = (remap_clamp(hover.x, rect.left()..=rect.right(), 0.0..=(n - 1) as f32).round()
                as usize)
                .min(n - 1);
            let p = points[i];
            painter.vline(p.x, rect.y_range(), Stroke::new(1.0, DARK.border));
            painter.circle_filled(p, 2.5, self.color_at(self.values[i]));
            let mut label = format_value(self.values[i]);
            if let Some(every) = self.every {
                let ago = every.as_secs_f32() * (n - 1 - i) as f32;
                label = format!("{label} · {}", age_label(ago));
                if let Some(clock) = self.clock {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_or(0, |d| d.as_secs() as i64);
                    label = format!("{label} · {}", clock(now - ago.round() as i64));
                }
            }
            let font = FontId::monospace(11.0);
            let above = Rect::from_min_max(rect.min, pos2(rect.max.x, p.y)).height() > 16.0;
            let (anchor_pos, align) = if above {
                (pos2(p.x, p.y - 4.0), Align2::CENTER_BOTTOM)
            } else {
                (pos2(p.x, p.y + 4.0), Align2::CENTER_TOP)
            };
            let text_size = painter
                .layout_no_wrap(label.clone(), font.clone(), DARK.text)
                .size();
            let text_rect = align.anchor_size(anchor_pos, text_size);
            painter.rect_filled(text_rect.expand(3.0), 3.0, DARK.bg.gamma_multiply(0.9));
            painter.text(anchor_pos, align, label, font, DARK.text);
        }
        response
    }
}

fn format_value(v: f32) -> String {
    if v.abs() >= 100.0 {
        format!("{v:.0}")
    } else if v.abs() >= 10.0 {
        format!("{v:.1}")
    } else {
        format!("{v:.2}")
    }
}

/// Charts at least this high get the time axis.
const AXIS_MIN_HEIGHT: f32 = 26.0;

/// An age for the time axis and the hover: `now`, `-45s`, `-5m`, `-1h 30m`, `-2h`, `-3d`.
pub fn age_label(secs: f32) -> String {
    let s = secs.max(0.0).round() as u64;
    match s {
        0 => "now".into(),
        1..=59 => format!("-{s}s"),
        60..=3599 => format!("-{}m", (s + 30) / 60),
        3600..=86_399 => {
            let m = (s % 3600 + 30) / 60;
            let h = s / 3600 + m / 60;
            let m = m % 60;
            if m == 0 {
                format!("-{h}h")
            } else {
                format!("-{h}h {m}m")
            }
        }
        _ => format!("-{}d", (s + 43_200) / 86_400),
    }
}

#[cfg(test)]
mod tests {
    use super::age_label;

    #[test]
    fn ages_read_short() {
        assert_eq!(age_label(0.0), "now");
        assert_eq!(age_label(45.0), "-45s");
        assert_eq!(age_label(300.0), "-5m");
        assert_eq!(age_label(1800.0), "-30m");
        assert_eq!(age_label(3600.0), "-1h");
        assert_eq!(age_label(5400.0), "-1h 30m");
        assert_eq!(age_label(3.0 * 86_400.0), "-3d");
        assert_eq!(age_label(f32::NAN), "now");
    }
}
