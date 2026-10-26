//! A dropdown of checkboxes that stays open while they are ticked (BAS-6).

use egui::{Button, InnerResponse, IntoAtoms, Popup, PopupCloseBehavior, Response, Ui, WidgetText};

/// A button that opens a list of checkboxes (a filter's values, say) and keeps it open while they are ticked:
/// only a click outside the list, Esc, or the button again closes it. `ui.menu_button` closes after every click
/// inside, so ticking three values took three trips to the button.
///
/// The rows are the caller's own (a checkbox with a tooltip, a greyed one saying why, a
/// [`FoldRest`](crate::FoldRest) with the rest); [`checklist_item`] is the plain one. A long list scrolls
/// inside the popup.
///
/// ```ignore
/// Checklist::new(format!("tracks: {}", picked.len())).ui(ui, |ui| {
///     for t in &tracks {
///         checklist_item(ui, &mut picked, t, t.as_str()).on_hover_text("…");
///     }
/// })
/// .response
/// .on_hover_text("Only tasks with one of these tracks");
/// ```
pub struct Checklist<'a> {
    button: Button<'a>,
    max_height: Option<f32>,
}

impl<'a> Checklist<'a> {
    pub fn new(atoms: impl IntoAtoms<'a>) -> Self {
        Self::from_button(Button::new(atoms))
    }

    /// From a styled button of the caller's.
    pub fn from_button(button: Button<'a>) -> Self {
        Self {
            button,
            max_height: None,
        }
    }

    /// The tallest the list grows before it scrolls; default: what is left of the screen below the button.
    pub fn max_height(mut self, height: f32) -> Self {
        self.max_height = Some(height);
        self
    }

    /// Draw the button and, while open, the list `content` adds. The response is the button's (for a
    /// tooltip); the inner value is `content`'s while the list is open.
    pub fn ui<R>(
        self,
        ui: &mut Ui,
        content: impl FnOnce(&mut Ui) -> R,
    ) -> InnerResponse<Option<R>> {
        let response = ui.add(self.button);
        let max_height = self.max_height;
        let inner = Popup::menu(&response)
            .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                // the popup's area hands its ui last frame's size: a list that grew (a fold opened) would
                // stay clipped at the old height, its new rows out of reach; room to the screen's bottom instead
                let room = ui.ctx().content_rect().bottom()
                    - ui.max_rect().top()
                    - ui.spacing().menu_margin.sum().y;
                let height = max_height.unwrap_or(room).min(room).max(0.0);
                ui.set_max_height(height);
                egui::ScrollArea::vertical()
                    .max_height(height)
                    .show(ui, content)
                    .inner
            });
        InnerResponse::new(inner.map(|i| i.inner), response)
    }

    /// Whether the list of the button with this `response` is open (tests, or a caller that draws a hint).
    pub fn is_open(ui: &Ui, response: &Response) -> bool {
        Popup::is_id_open(ui.ctx(), Popup::default_response_id(response))
    }
}

/// One checkbox of a [`Checklist`] over a `picked` list: ticked while `value` is in it, ticking adds it at the
/// end, unticking removes it. Returns the checkbox's response (`changed()` after a click).
pub fn checklist_item<T: PartialEq + Clone>(
    ui: &mut Ui,
    picked: &mut Vec<T>,
    value: &T,
    text: impl Into<WidgetText>,
) -> Response {
    let mut on = picked.contains(value);
    let response = ui.checkbox(&mut on, text);
    if response.changed() {
        picked.retain(|p| p != value);
        if on {
            picked.push(value.clone());
        }
    }
    response
}
