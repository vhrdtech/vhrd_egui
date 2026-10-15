//! Small header icons drawn as geometry, one family: the ⓘ (a circle with a dot and a stem) and the ☰ (three
//! bars), the same box, the same stroke, muted at rest and in the text colour under the pointer. Painted
//! instead of Unicode glyphs, whose look depends on the font (the brand fonts lack ⓘ, and a font's "i" sits
//! where its metrics put it, not in the middle of a circle). Other icons of the family (ve_frame's size
//! switches) take [`ICON`], [`ICON_STROKE`] and [`icon_ink`] from here.

use egui::emath::GuiRounding;
use egui::{Color32, Painter, Pos2, Rect, Response, Sense, Stroke, Ui, vec2};

use crate::theme::Theme;

/// The box of a header icon, points: the ⓘ, the ☰ and the size switches are drawn in a square this size.
pub const ICON: f32 = 13.0;

/// The stroke of the family's lines, points.
pub const ICON_STROKE: f32 = 1.2;

/// The colour of an icon: muted at rest, the text colour when `lit` (under the pointer, focused, open).
pub fn icon_ink(theme: &Theme, lit: bool) -> Color32 {
    if lit { theme.text } else { theme.text_muted }
}

/// The centre of `rect` on the pixel grid, so a 1-pixel line drawn through it is crisp.
pub fn icon_center(painter: &Painter, rect: Rect) -> Pos2 {
    rect.center()
        .round_to_pixel_center(painter.pixels_per_point())
}

/// A round "i" icon, `size` points across, muted; brighter under the pointer. Attach the tooltip to the
/// returned response (`.on_hover_ui(..)` for rich content).
///
/// ```ignore
/// info_icon(ui, &theme, 12.0).on_hover_text("plan: max 5x\nread by: gpd, m1");
/// ```
pub fn info_icon(ui: &mut Ui, theme: &Theme, size: f32) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Label, ui.is_enabled(), "info")
    });
    if ui.is_rect_visible(rect) {
        let color = icon_ink(theme, response.hovered());
        paint_info(ui.painter(), rect, color);
    }
    response
}

/// The ⓘ into `rect`: a circle, and inside it a round dot over a stem with rounded ends, optically centred
/// (the dot and the stem together are centred, the gap between them a little above the middle).
pub fn paint_info(painter: &Painter, rect: Rect, color: Color32) {
    let size = rect.width().min(rect.height());
    let c = icon_center(painter, rect);
    let s = size / ICON;
    painter.circle_stroke(
        c,
        size * 0.5 - ICON_STROKE * 0.5,
        Stroke::new(ICON_STROKE, color),
    );
    // the stem from 1 above the centre to 3 below, the dot 2.9 above: together a hair above the middle,
    // which reads as centred since the dot weighs less than the stem
    let w = 1.5 * s;
    let stem = Rect::from_min_max(c + vec2(-w * 0.5, -s), c + vec2(w * 0.5, 3.0 * s));
    painter.rect_filled(stem, w * 0.5, color);
    painter.circle_filled(c + vec2(0.0, -2.9 * s), 0.85 * s, color);
}

/// The ☰ into `rect`: three bars with rounded ends, centred.
pub fn paint_menu(painter: &Painter, rect: Rect, color: Color32) {
    let size = rect.width().min(rect.height());
    let c = icon_center(painter, rect);
    let s = size / ICON;
    let half = 4.5 * s;
    for dy in [-3.5, 0.0, 3.5] {
        let y = c.y + dy * s;
        let bar = Rect::from_min_max(
            Pos2::new(c.x - half, y - ICON_STROKE * 0.5),
            Pos2::new(c.x + half, y + ICON_STROKE * 0.5),
        );
        painter.rect_filled(bar, ICON_STROKE * 0.5, color);
    }
}
