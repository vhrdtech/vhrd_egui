//! The ⓘ icon: a small painted circle with an "i", whose tooltip carries the details a row has no room for.
//! Painted instead of the Unicode glyph ⓘ, which the brand fonts lack.

use egui::{Align2, FontId, Response, Sense, Stroke, Ui, vec2};

use crate::theme::Theme;

/// A round "i" icon, `size` points across, muted; brighter under the pointer. Attach the tooltip to the
/// returned response (`.on_hover_ui(..)` for rich content).
///
/// ```ignore
/// info_icon(ui, &theme, 12.0).on_hover_text("plan: max 5x\nread by: gpd, m1");
/// ```
pub fn info_icon(ui: &mut Ui, theme: &Theme, size: f32) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    if ui.is_rect_visible(rect) {
        let color = if response.hovered() {
            theme.text
        } else {
            theme.text_muted
        };
        let painter = ui.painter();
        painter.circle_stroke(rect.center(), size * 0.5 - 0.5, Stroke::new(1.0, color));
        painter.text(
            rect.center() + vec2(0.0, 0.5),
            Align2::CENTER_CENTER,
            "i",
            FontId::proportional(size * 0.72),
            color,
        );
    }
    response
}
