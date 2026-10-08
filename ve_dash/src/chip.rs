//! A clickable chip: text on a rounded background that lights up under the pointer and presses in when
//! clicked, with a pointing-hand cursor and a tooltip. For the small actions of a dashboard's bar (switch the
//! language, flip a colour mode, measure again), where a full button is too heavy and bare text gives no sign
//! that it does something.

use egui::{Response, Sense, Ui, Vec2, WidgetText};

use crate::theme::Theme;

/// A chip showing `text`. Check `.clicked()` on the response; the tooltip is attached already.
///
/// ```ignore
/// if ActionChip::new("русский", &theme).hover("Switch the language").show(ui).clicked() {
///     toggle_language();
/// }
/// ```
#[must_use = "call .show(ui)"]
pub struct ActionChip<'a> {
    text: WidgetText,
    theme: &'a Theme,
    hover: String,
    min_width: f32,
    active: bool,
}

impl<'a> ActionChip<'a> {
    pub fn new(text: impl Into<WidgetText>, theme: &'a Theme) -> Self {
        Self {
            text: text.into(),
            theme,
            hover: String::new(),
            min_width: 0.0,
            active: false,
        }
    }

    /// The tooltip: what the chip does.
    pub fn hover(mut self, hover: impl Into<String>) -> Self {
        self.hover = hover.into();
        self
    }

    /// At least this wide (a learned width from [`crate::steady_of`]).
    pub fn min_width(mut self, min_width: f32) -> Self {
        self.min_width = min_width;
        self
    }

    /// Drawn as switched on (a mode that is currently active).
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    /// The width the chip would have, padding included, for [`crate::steady_of`].
    pub fn natural_width(&self, ui: &Ui) -> f32 {
        let galley = self.text.clone().into_galley(
            ui,
            Some(egui::TextWrapMode::Extend),
            f32::INFINITY,
            egui::TextStyle::Body,
        );
        galley.size().x + 2.0 * PAD.x
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let galley = self.text.clone().into_galley(
            ui,
            Some(egui::TextWrapMode::Extend),
            f32::INFINITY,
            egui::TextStyle::Body,
        );
        let width = (galley.size().x + 2.0 * PAD.x).max(self.min_width);
        let size = Vec2::new(width, galley.size().y + 2.0 * PAD.y);
        let (rect, response) = ui.allocate_exact_size(size, Sense::click());
        let label = galley.text().to_owned();
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label.clone())
        });
        if ui.is_rect_visible(rect) {
            let fill = if response.is_pointer_button_down_on() {
                Some(self.theme.border)
            } else if response.hovered() || response.has_focus() {
                Some(self.theme.border.gamma_multiply(0.6))
            } else if self.active {
                Some(self.theme.border.gamma_multiply(0.35))
            } else {
                None
            };
            let painter = ui.painter();
            if let Some(fill) = fill {
                painter.rect_filled(rect, 4.0, fill);
            }
            if response.hovered() {
                painter.rect_stroke(
                    rect,
                    4.0,
                    egui::Stroke::new(1.0, self.theme.border),
                    egui::StrokeKind::Inside,
                );
            }
            let pos = rect.left_center() + Vec2::new(PAD.x, -galley.size().y / 2.0);
            painter.galley(pos, galley, self.theme.text);
        }
        let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
        if self.hover.is_empty() {
            response
        } else {
            response.on_hover_text(self.hover)
        }
    }
}

/// Padding around the text.
const PAD: Vec2 = Vec2::new(5.0, 2.0);
