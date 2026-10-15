//! The two size switches in the header (FRM-11): width and height, each a small painted glyph with its word.
//!
//! The glyph is a tiny body whose length along the switch's axis shows the step, with a tick for each of the
//! three steps (min, nom, wide or tall) on that axis, the current one lit in the accent. On auto the outline
//! is dashed and a double arrow runs along the axis; the tick of the step auto shows is lit, softer. The width
//! glyph lies along x with its ticks below, the height glyph is the same turned onto y with its ticks right.

use egui::{Color32, FontId, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, Vec2, pos2, vec2};
use ve_dash::Theme;

use crate::size::{HeightMode, SizeMode};

/// Which axis a switch sets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Axis {
    Width,
    Height,
}

/// What a switch shows.
pub(crate) struct Look<'a> {
    pub axis: Axis,
    /// The word: the mode it is set to.
    pub word: &'a str,
    /// Set to auto: dashed outline, arrows.
    pub auto: bool,
    /// The step shown now, 0 (min) to 2 (wide or tall).
    pub shown: usize,
    /// Clickable; a disabled one is drawn faint and its tooltip says why.
    pub enabled: bool,
}

/// The side of the glyph's square.
const GLYPH: f32 = 13.0;
/// The body's length along the axis for min, nom and the large step.
const STEPS: [f32; 3] = [4.0, 8.0, 12.0];
/// Padding left and right of the switch, and between glyph and word.
const PAD_X: f32 = 4.0;
const GAP: f32 = 3.0;
const HEIGHT: f32 = 18.0;

fn word_font() -> FontId {
    FontId::monospace(9.5)
}

/// The widest word on either switch, so the header doesn't change as they cycle.
fn widest_word(ui: &Ui, theme: &Theme) -> f32 {
    let words = SizeMode::ALL
        .iter()
        .map(|m| m.label())
        .chain(HeightMode::ALL.iter().map(|m| m.label()));
    words
        .map(|w| {
            ui.painter()
                .layout_no_wrap(w.to_owned(), word_font(), theme.text)
                .size()
                .x
        })
        .fold(0.0, f32::max)
}

/// The width of one switch.
pub(crate) fn switch_width(ui: &Ui, theme: &Theme) -> f32 {
    2.0 * PAD_X + GLYPH + GAP + widest_word(ui, theme)
}

/// The width of the pair, with the gap between them.
pub(crate) fn pair_width(ui: &Ui, theme: &Theme, gap: f32) -> f32 {
    2.0 * switch_width(ui, theme) + gap
}

/// Draw one switch; `tip` is its tooltip (what it does, what the next click gives, or why it is disabled).
pub(crate) fn show(ui: &mut Ui, theme: &Theme, look: &Look<'_>, tip: &str) -> Response {
    let width = switch_width(ui, theme);
    let sense = if look.enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(vec2(width, HEIGHT), sense);
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
    let ink = faint(if lit { theme.text } else { theme.text_muted });
    let accent = faint(theme.accent);
    let glyph = Rect::from_min_size(
        pos2(rect.left() + PAD_X, rect.center().y - GLYPH / 2.0),
        Vec2::splat(GLYPH),
    );
    glyph_shapes(look, glyph, ink, accent)
        .into_iter()
        .for_each(|s| {
            painter.add(s);
        });
    let galley = painter.layout_no_wrap(look.word.to_owned(), word_font(), ink);
    let pos = pos2(glyph.right() + GAP, rect.center().y - galley.size().y / 2.0);
    painter.galley(pos, galley, ink);
}

/// The glyph's shapes in `g`, a square [`GLYPH`] wide. Kept apart from the painter for a test.
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
    // the body: from the start of the axis to the step, across 1..9
    let (a0, a1) = (0.5, 0.5 + len);
    let (c0, c1) = (1.0, 9.0);
    let corners = [at(a0, c0), at(a1, c0), at(a1, c1), at(a0, c1)];
    let body = Rect::from_two_pos(corners[0], corners[2]);
    if look.auto {
        let outline = Stroke::new(1.0, ink);
        let mut path = corners.to_vec();
        path.push(corners[0]);
        shapes.extend(Shape::dashed_line(&path, outline, 2.0, 1.5));
        // the double arrow along the axis, over the whole length the body can take
        let mid = (c0 + c1) / 2.0;
        let (s, e) = (0.5, 0.5 + STEPS[2]);
        let arrow = Stroke::new(1.2, accent);
        shapes.push(Shape::line_segment([at(s, mid), at(e, mid)], arrow));
        for (tip, back) in [(s, s + 2.5), (e, e - 2.5)] {
            shapes.push(Shape::line(
                vec![at(back, mid - 2.2), at(tip, mid), at(back, mid + 2.2)],
                arrow,
            ));
        }
    } else {
        shapes.push(Shape::rect_filled(body, 1.0, accent.gamma_multiply(0.35)));
        shapes.push(Shape::rect_stroke(
            body,
            1.0,
            Stroke::new(1.0, accent),
            egui::StrokeKind::Inside,
        ));
    }
    // the ticks: one per step where its body would end, across 10.5..13
    for (i, step) in STEPS.iter().enumerate() {
        let x = 0.5 + step;
        let (color, width, from) = if i == shown {
            let color = if look.auto {
                accent.gamma_multiply(0.7)
            } else {
                accent
            };
            (color, 1.6, 9.8)
        } else {
            (ink.gamma_multiply(0.6), 1.0, 10.8)
        };
        shapes.push(Shape::line_segment(
            [at(x, from), at(x, GLYPH)],
            Stroke::new(width, color),
        ));
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
        let g = Rect::from_min_size(Pos2::ZERO, Vec2::splat(GLYPH));
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
        let g = Rect::from_min_size(Pos2::ZERO, Vec2::splat(GLYPH));
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
}
