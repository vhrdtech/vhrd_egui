//! Filled line chart of a short history, btop style: thin line, gradient fill,
//! crosshair with the value on hover.

use std::ops::RangeInclusive;

use egui::{
    Align2, Color32, FontId, Mesh, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, Vec2, Widget,
    pos2, remap_clamp, vec2,
};

use crate::theme::DARK;

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
            let top_color = self.color.gamma_multiply(0.25);
            let mut mesh = Mesh::default();
            for p in &points {
                mesh.colored_vertex(*p, top_color);
                mesh.colored_vertex(pos2(p.x, rect.bottom()), Color32::TRANSPARENT);
            }
            for i in 0..n - 1 {
                let a = (i * 2) as u32;
                mesh.add_triangle(a, a + 1, a + 2);
                mesh.add_triangle(a + 1, a + 3, a + 2);
            }
            painter.add(Shape::mesh(mesh));
        }
        painter.add(Shape::line(points.clone(), Stroke::new(1.5, self.color)));

        if let Some(hover) = response.hover_pos() {
            let i = (remap_clamp(hover.x, rect.left()..=rect.right(), 0.0..=(n - 1) as f32).round()
                as usize)
                .min(n - 1);
            let p = points[i];
            painter.vline(p.x, rect.y_range(), Stroke::new(1.0, DARK.border));
            painter.circle_filled(p, 2.5, self.color);
            let label = format_value(self.values[i]);
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
