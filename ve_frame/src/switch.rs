//! The two size switches in the header (FRM-11): width and height, each a small painted glyph, icon only,
//! in the family of ve_dash's ⓘ and ☰ (`ve_dash::ICON` box, `ICON_STROKE` lines, `icon_ink` colours). The
//! word (auto, min, nom, wide, tall) is in the tooltip and the accessible label.
//!
//! The glyph is a tiny body whose length along the switch's axis shows the step, with a tick for each of the
//! three steps (min, nom, wide or tall) on that axis, the current one lit in the accent. On auto the outline
//! is dashed and a double arrow runs along the axis; the tick of the step auto shows is lit, softer. The width
//! glyph lies along x with its ticks below, the height glyph is the same turned onto y with its ticks right.

use egui::{Color32, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, pos2, vec2};
use ve_dash::{ICON, ICON_STROKE, Theme, icon_center, icon_ink};

/// Which axis a switch sets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Axis {
    Width,
    Height,
}

/// What a switch shows.
pub(crate) struct Look<'a> {
    pub axis: Axis,
    /// The word: the mode it is set to (in the accessible label, not drawn).
    pub word: &'a str,
    /// Set to auto: dashed outline, arrows.
    pub auto: bool,
    /// The step shown now, 0 (min) to 2 (wide or tall).
    pub shown: usize,
    /// Clickable; a disabled one is drawn faint and its tooltip says why.
    pub enabled: bool,
}

/// The glyph's length along its axis and its depth across it (the body, then the ticks): it fits the
/// family's [`ICON`] box.
const ALONG: f32 = 12.0;
const ACROSS: f32 = 9.0;
/// The body's length along the axis for min, nom and the large step.
const STEPS: [f32; 3] = [4.0, 7.5, 11.0];
/// Padding around the icon inside the switch's box, as the ☰ button has.
const PAD: f32 = 3.0;

/// The size of one switch: the icon box and its padding.
pub(crate) fn switch_size() -> egui::Vec2 {
    vec2(ICON + 2.0 * PAD, ICON + 2.0 * PAD)
}

/// The width of the pair, with the gap between them.
pub(crate) fn pair_width(gap: f32) -> f32 {
    2.0 * switch_size().x + gap
}

/// Draw one switch; `tip` is its tooltip (what it does, what the next click gives, or why it is disabled).
pub(crate) fn show(ui: &mut Ui, theme: &Theme, look: &Look<'_>, tip: &str) -> Response {
    let sense = if look.enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(switch_size(), sense);
    let axis = match look.axis {
        Axis::Width => "width",
        Axis::Height => "height",
    };
    let label = format!("{axis}: {}", look.word);
    let enabled = look.enabled;
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label.clone())
    });
    if ui.is_rect_visible(rect) {
        paint(ui, theme, look, rect, &response);
    }
    let response = if look.enabled {
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        response
    };
    response.on_hover_text(tip)
}

fn paint(ui: &Ui, theme: &Theme, look: &Look<'_>, rect: Rect, response: &Response) {
    let painter = ui.painter();
    let lit = look.enabled && (response.hovered() || response.has_focus());
    let fill = if !look.enabled {
        None
    } else if response.is_pointer_button_down_on() {
        Some(theme.border)
    } else if lit {
        Some(theme.border.gamma_multiply(0.6))
    } else if !look.auto {
        // pinned: a quiet pressed-in look, as a toggle that is on
        Some(theme.border.gamma_multiply(0.35))
    } else {
        None
    };
    if let Some(fill) = fill {
        painter.rect_filled(rect, 4.0, fill);
    }
    if lit {
        painter.rect_stroke(
            rect,
            4.0,
            Stroke::new(1.0, theme.border),
            egui::StrokeKind::Inside,
        );
    }
    let faint = |c: Color32| {
        if look.enabled {
            c
        } else {
            c.gamma_multiply(0.45)
        }
    };
    let ink = faint(icon_ink(theme, lit));
    let accent = faint(theme.accent);
    let size = match look.axis {
        Axis::Width => vec2(ALONG, ACROSS),
        Axis::Height => vec2(ACROSS, ALONG),
    };
    let glyph = Rect::from_center_size(icon_center(painter, rect), size);
    for shape in glyph_shapes(look, glyph, ink, accent) {
        painter.add(shape);
    }
}

/// The glyph's shapes in `g`, [`ALONG`] long on its axis and [`ACROSS`] deep. Kept apart from the painter for
/// a test.
fn glyph_shapes(look: &Look<'_>, g: Rect, ink: Color32, accent: Color32) -> Vec<Shape> {
    // Work in (along, across) coordinates of the axis and turn them into points at the end.
    let at = |along: f32, across: f32| -> Pos2 {
        match look.axis {
            Axis::Width => pos2(g.left() + along, g.top() + across),
            Axis::Height => pos2(g.left() + across, g.top() + along),
        }
    };
    let mut shapes = Vec::new();
    let shown = look.shown.min(2);
    let len = STEPS[shown];
    // the body: from the start of the axis to the step, the first part of the depth
    let (a0, a1) = (0.5, 0.5 + len);
    let (c0, c1) = (0.5, 5.5);
    let corners = [at(a0, c0), at(a1, c0), at(a1, c1), at(a0, c1)];
    let body = Rect::from_two_pos(corners[0], corners[2]);
    if look.auto {
        let outline = Stroke::new(ICON_STROKE, ink);
        let mut path = corners.to_vec();
        path.push(corners[0]);
        shapes.extend(Shape::dashed_line(&path, outline, 1.5, 1.5));
        // the double arrow along the axis, over the whole length the body can take
        let mid = (c0 + c1) / 2.0;
        let (s, e) = (0.5, 0.5 + STEPS[2]);
        let arrow = Stroke::new(ICON_STROKE, accent);
        shapes.push(Shape::line_segment([at(s, mid), at(e, mid)], arrow));
        for (tip, back) in [(s, s + 2.2), (e, e - 2.2)] {
            shapes.push(Shape::line(
                vec![at(back, mid - 2.0), at(tip, mid), at(back, mid + 2.0)],
                arrow,
            ));
        }
    } else {
        shapes.push(Shape::rect_filled(body, 1.0, accent.gamma_multiply(0.35)));
        shapes.push(Shape::rect_stroke(
            body,
            1.0,
            Stroke::new(ICON_STROKE, accent),
            egui::StrokeKind::Inside,
        ));
    }
    // the ticks: one per step where its body would end, in the rest of the depth
    for (i, step) in STEPS.iter().enumerate() {
        let x = 0.5 + step;
        let (color, width, from) = if i == shown {
            let color = if look.auto {
                accent.gamma_multiply(0.7)
            } else {
                accent
            };
            (color, ICON_STROKE + 0.4, 6.0)
        } else {
            (ink.gamma_multiply(0.6), ICON_STROKE * 0.85, 7.0)
        };
        shapes.push(Shape::line_segment(
            [at(x, from), at(x, ACROSS)],
            Stroke::new(width, color),
        ));
    }
    // centred on what is drawn, not on the box: the ticks and strokes make it lopsided by up to a point
    let drawn = Shape::Vec(shapes.clone()).visual_bounding_rect();
    let off = g.center() - drawn.center();
    for shape in &mut shapes {
        shape.translate(off);
    }
    shapes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn look(axis: Axis, auto: bool, shown: usize) -> Look<'static> {
        Look {
            axis,
            word: "nom",
            auto,
            shown,
            enabled: true,
        }
    }

    /// The glyph of the height switch is the width one turned onto the other axis: the same shapes with x
    /// and y swapped.
    #[test]
    fn height_glyph_mirrors_the_width_glyph() {
        let g = Rect::from_min_size(Pos2::ZERO, vec2(ALONG, ACROSS));
        for auto in [false, true] {
            for shown in 0..3 {
                let bounds = |axis| {
                    let shapes =
                        glyph_shapes(&look(axis, auto, shown), g, Color32::WHITE, Color32::RED);
                    Shape::Vec(shapes).visual_bounding_rect()
                };
                let (w, h) = (bounds(Axis::Width), bounds(Axis::Height));
                assert!((w.width() - h.height()).abs() < 0.01, "{w:?} {h:?}");
                assert!((w.height() - h.width()).abs() < 0.01, "{w:?} {h:?}");
                assert!(g.expand(1.0).contains_rect(w), "inside its square: {w:?}");
            }
        }
    }

    /// The body grows with the step along its axis.
    #[test]
    fn the_body_shows_the_step() {
        let g = Rect::from_min_size(Pos2::ZERO, vec2(ALONG, ACROSS));
        let body_len = |shown| {
            let shapes = glyph_shapes(
                &look(Axis::Width, false, shown),
                g,
                Color32::WHITE,
                Color32::RED,
            );
            shapes[0].visual_bounding_rect().width()
        };
        assert!(body_len(0) < body_len(1) && body_len(1) < body_len(2));
    }

    /// The glyph's drawing is centred in its box within half a point on both axes, in every state, so the
    /// switch's centre is the icon's centre (the header lines up icon centres).
    #[test]
    fn the_glyph_is_centred_in_its_box() {
        for axis in [Axis::Width, Axis::Height] {
            let size = match axis {
                Axis::Width => vec2(ALONG, ACROSS),
                Axis::Height => vec2(ACROSS, ALONG),
            };
            let g = Rect::from_center_size(Pos2::ZERO, size);
            for auto in [false, true] {
                for shown in 0..3 {
                    let shapes =
                        glyph_shapes(&look(axis, auto, shown), g, Color32::WHITE, Color32::RED);
                    let drawn = Shape::Vec(shapes).visual_bounding_rect();
                    let off = drawn.center() - g.center();
                    assert!(
                        off.x.abs() <= 0.5 && off.y.abs() <= 0.5,
                        "{axis:?} auto {auto} step {shown}: off by {off:?}"
                    );
                }
            }
        }
    }
}
