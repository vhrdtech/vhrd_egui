//! Steady columns: rows of a list whose first cell (a login name, a session name) is as wide as the widest
//! row has needed, so everything right of it lines up, without anyone hard-coding that width.

use egui::{
    AsIdSalt, Id, InnerResponse, Label, Response, RichText, Sense, TextStyle, TextWrapMode, Ui,
    WidgetText,
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
        self.text_cell(ui, text.into(), hover.into(), false)
    }

    /// A clickable text cell (a session name that opens its view): like [`label_hover`](Self::label_hover),
    /// with the pointing-hand cursor and an underline under the pointer. Check `.clicked()` on the response.
    pub fn link(
        &self,
        ui: &mut Ui,
        text: impl Into<RichText>,
        hover: impl Into<String>,
    ) -> Response {
        self.text_cell(ui, text.into(), hover.into(), true)
    }

    /// Whether the link or lit cell showing `text` was under the pointer in the last frame.
    pub fn was_hovered(&self, ui: &Ui, text: &str) -> bool {
        let id = self.id.with("hover").with(text);
        ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(false)
    }

    /// A text cell that is not clickable but still lights up under the pointer (the text turns to the strong
    /// colour), so every row of a list answers a hover alike, whatever it can do.
    pub fn lit(
        &self,
        ui: &mut Ui,
        text: impl Into<RichText>,
        hover: impl Into<String>,
    ) -> Response {
        let text: RichText = text.into();
        let id = self.id.with("hover").with(text.text());
        let lit = ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
        let text = if lit {
            text.color(ui.visuals().strong_text_color())
        } else {
            text
        };
        let r = self.text_cell_with(ui, text, hover.into(), false, Sense::hover());
        ui.data_mut(|d| d.insert_temp(id, r.hovered()));
        r
    }

    fn text_cell(&self, ui: &mut Ui, text: RichText, hover: String, clickable: bool) -> Response {
        let sense = if clickable {
            Sense::click()
        } else {
            Sense::hover()
        };
        self.text_cell_with(ui, text, hover, clickable, sense)
    }

    fn text_cell_with(
        &self,
        ui: &mut Ui,
        text: RichText,
        hover: String,
        clickable: bool,
        sense: Sense,
    ) -> Response {
        // One hover memory per cell (keyed by its text), not per column: a column-wide one underlined the next
        // row of the column, and the first row when the last was hovered.
        let id = self.id.with("hover").with(text.text());
        let hovered_before = clickable && ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
        let text = if hovered_before {
            text.underline()
        } else {
            text
        };
        let widget: WidgetText = text.into();
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
        let label = Label::new(widget).truncate().selectable(false);
        let label = label.sense(sense);
        let r = self.cell(ui, |ui| ui.add(label)).inner;
        if clickable {
            ui.data_mut(|d| d.insert_temp(id, r.hovered()));
            if r.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
        }
        let cut = natural > self.max;
        match (hover.is_empty(), cut) {
            (true, false) => r,
            (true, true) => r.on_hover_text(full),
            (false, false) => r.on_hover_text(hover),
            (false, true) => r.on_hover_text(format!("{full}\n\n{hover}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;
    use std::{cell::RefCell, rc::Rc};

    /// Hovering a link marks that link only: not the one drawn after it, and not the first when the last is
    /// hovered (the hover memory was once shared by the whole column).
    #[test]
    fn link_hover_marks_only_its_own_cell() {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let out = seen.clone();
        let mut h = Harness::builder()
            .with_size(egui::vec2(300.0, 120.0))
            .build_ui(move |ui| {
                let col = SteadyColumn::new(ui, "c");
                let names = ["one", "two", "three"];
                for n in names {
                    col.link(ui, n, "");
                }
                for n in ["plain", "other"] {
                    col.lit(ui, n, "");
                }
                *out.borrow_mut() = names
                    .iter()
                    .chain(["plain", "other"].iter())
                    .map(|n| col.was_hovered(ui, n))
                    .collect();
            });
        h.run();
        for (name, want) in [
            ("one", [true, false, false, false, false]),
            ("three", [false, false, true, false, false]),
            ("other", [false, false, false, false, true]),
        ] {
            let at = h.get_by_label_contains(name).rect().center();
            h.hover_at(at);
            h.step();
            h.step();
            assert_eq!(seen.borrow()[..5], want, "hovering {name}");
        }
    }
}
