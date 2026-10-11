//! The ghost of a widget in the place it floated out of.

use egui::{Align2, FontId, Response, Sense, Stroke, Ui, Vec2};
use ve_dash::Theme;
use ve_theme::Tokens;

use crate::frame::Shape;
use crate::hex;

/// The place in a grid a floating widget came from: a dashed outline of `size` in its `shape` with the title
/// faint in the middle. It keeps the slot, so the rest of the grid doesn't close up behind a widget that was
/// popped out, and shows where the widget goes back to. A click means "put it back": the host carries out
/// [`crate::FrameAction::DockBack`].
///
/// A host draws it where the widget would sit, for every floating widget with a
/// [`crate::FrameState::left_slot`]. It reads well in a grid that reflows (rows, a physics layout); in tabs,
/// where a pane's room goes to its neighbours anyway, leave it out.
///
/// ```
/// # use ve_frame::{FrameAction, Shape, ghost_slot};
/// # egui::__run_test_ui(|ui| {
/// if ghost_slot(ui, "supply", egui::vec2(220.0, 120.0), Shape::Rect).clicked() {
///     // dock.apply(id, FrameAction::DockBack);
/// }
/// # });
/// ```
pub fn ghost_slot(ui: &mut Ui, title: &str, size: Vec2, shape: Shape) -> Response {
    let theme = Theme::from_tokens(&Tokens::of_ui(ui));
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let label = format!("put back {title}");
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label.clone()));
    if ui.is_rect_visible(rect) {
        let lit = response.hovered() || response.has_focus();
        let color = if lit { theme.text_muted } else { theme.border };
        let inside = rect.shrink(1.0);
        let mut path = match shape {
            Shape::Rect => vec![
                inside.left_top(),
                inside.right_top(),
                inside.right_bottom(),
                inside.left_bottom(),
            ],
            Shape::Hex => hex::points(inside).to_vec(),
        };
        path.push(path[0]);
        let painter = ui.painter();
        if lit {
            let fill = theme.border.gamma_multiply(0.25);
            let outline = path[..path.len() - 1].to_vec();
            painter.add(egui::Shape::convex_polygon(outline, fill, Stroke::NONE));
        }
        painter.extend(egui::Shape::dashed_line(
            &path,
            Stroke::new(1.0, color),
            6.0,
            4.0,
        ));
        let text = painter.layout(
            title.to_owned(),
            FontId::proportional(12.0),
            theme.text_muted.gamma_multiply(0.7),
            (rect.width() - 16.0).max(0.0),
        );
        if text.size().y <= rect.height() {
            let at = Align2::CENTER_CENTER.anchor_size(rect.center(), text.size());
            painter.galley(at.min, text, theme.text_muted);
        }
    }
    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(format!(
            "{title} floats as a window; this is the place it came from. Click to put it back here."
        ))
}
