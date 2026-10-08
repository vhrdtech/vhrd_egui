//! Steady columns: rows of a list whose first cell (a login name, a session name) is as wide as the widest
//! row has needed, so everything right of it lines up, without anyone hard-coding that width.

use egui::{
    AsIdSalt, Id, InnerResponse, Label, Response, RichText, TextStyle, TextWrapMode, Ui, WidgetText,
};

use crate::steady::{remember, remembered};

/// A column whose width is learned across its rows (see [`crate::steady_width`] for a single piece).
///
/// Create it once per list with an id, call [`cell`](Self::cell) or [`label`](Self::label) for the column's
/// cell in every row. The column is as wide as the widest cell seen so far (never narrower within the
/// session), at most `max_width`: content wider than that is cut. Names get an ellipsis and the full text on
/// hover with [`label`](Self::label).
///
/// ```ignore
/// let names = SteadyColumn::new(ui, "logins").max_width(ui.available_width() * 0.3);
/// for row in rows {
///     ui.horizontal(|ui| {
///         names.label(ui, &row.name);
///         ui.add(Meter::new(row.used));
///     });
/// }
/// ```
#[derive(Clone, Copy, Debug)]
pub struct SteadyColumn {
    id: Id,
    max: f32,
}

impl SteadyColumn {
    /// A column named `id_salt` within `ui` (the id is scoped to `ui.id()`, so create it in the list's own
    /// ui, outside the per-row `horizontal`s).
    pub fn new(ui: &Ui, id_salt: impl AsIdSalt) -> Self {
        Self {
            id: ui.id().with("ve_dash::SteadyColumn").with(id_salt),
            max: f32::INFINITY,
        }
    }

    /// The widest the column may grow, e.g. a share of the available width. Default: unlimited.
    pub fn max_width(mut self, max: f32) -> Self {
        self.max = max;
        self
    }

    /// The width learned so far, 0 before the first row.
    pub fn width(&self, ui: &Ui) -> f32 {
        remembered(ui, self.id).min(self.max)
    }

    /// This row's cell: `add_contents` laid out at least as wide as the column and at most `max_width`.
    pub fn cell<R>(
        &self,
        ui: &mut Ui,
        add_contents: impl FnOnce(&mut Ui) -> R,
    ) -> InnerResponse<R> {
        let min = self.width(ui);
        let max = self.max;
        let inner = ui.scope(|ui| {
            ui.set_min_width(min);
            if max.is_finite() {
                ui.set_max_width(max.max(min));
            }
            add_contents(ui)
        });
        remember(ui, self.id, inner.response.rect.width().min(self.max));
        inner
    }

    /// A text cell: truncated with an ellipsis when wider than `max_width`, the full text on hover then.
    pub fn label(&self, ui: &mut Ui, text: impl Into<RichText>) -> Response {
        self.label_hover(ui, text, "")
    }

    /// [`label`](Self::label) with a tooltip of its own (shown with the full text below it when the text is
    /// cut). One tooltip per cell: attach none yourself, that would stack a second one.
    pub fn label_hover(
        &self,
        ui: &mut Ui,
        text: impl Into<RichText>,
        hover: impl Into<String>,
    ) -> Response {
        let widget: WidgetText = text.into().into();
        let full = widget.text().to_owned();
        let natural = widget
            .clone()
            .into_galley(
                ui,
                Some(TextWrapMode::Extend),
                f32::INFINITY,
                TextStyle::Body,
            )
            .size()
            .x;
        let r = self
            .cell(ui, |ui| ui.add(Label::new(widget).truncate()))
            .inner;
        let hover = hover.into();
        let cut = natural > self.max;
        match (hover.is_empty(), cut) {
            (true, false) => r,
            (true, true) => r.on_hover_text(full),
            (false, false) => r.on_hover_text(hover),
            (false, true) => r.on_hover_text(format!("{hover}\n\n{full}")),
        }
    }
}
