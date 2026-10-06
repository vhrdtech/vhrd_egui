//! Titled panel, btop style: a rounded border with the title set into the
//! top border line.

use egui::{Align2, CornerRadius, FontId, Frame, InnerResponse, Margin, Stroke, Ui, pos2};

use crate::theme::Theme;

/// Draw a bordered panel with `title` sitting on the top border.
///
/// ```ignore
/// panel(ui, &theme, "omarchy-m1", |ui| { /* widgets */ });
/// ```
pub fn panel<R>(
    ui: &mut Ui,
    theme: &Theme,
    title: &str,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<R> {
    let frame = Frame::new()
        .fill(theme.panel_bg)
        .stroke(Stroke::new(1.0, theme.border))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin {
            left: 10,
            right: 10,
            top: 14,
            bottom: 10,
        })
        .outer_margin(Margin {
            left: 0,
            right: 0,
            top: 6, // room for the title text above the border line
            bottom: 0,
        });
    let inner = frame.show(ui, add_contents);

    let frame_rect = inner.response.rect;
    if ui.is_rect_visible(frame_rect) && !title.is_empty() {
        let painter = ui.painter();
        let font = FontId::proportional(12.0);
        let text_pos = pos2(frame_rect.left() + 12.0, frame_rect.top());
        let text_size = painter
            .layout_no_wrap(title.to_owned(), font.clone(), theme.title)
            .size();
        // Blank the border segment behind the title, then draw it.
        let bg = Align2::LEFT_CENTER
            .anchor_size(text_pos, text_size)
            .expand(3.0);
        painter.rect_filled(bg, 0.0, theme.bg);
        painter.text(text_pos, Align2::LEFT_CENTER, title, font, theme.title);
    }
    inner
}
