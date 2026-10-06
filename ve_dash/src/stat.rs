//! Stat tile: a small label over a big monospace value, for headline numbers
//! that don't need a chart.

use egui::{Align2, Color32, FontId, Response, Sense, Ui, Vec2, Widget, pos2};

use crate::theme::{DARK, Theme};

/// `StatTile::new("nodes online", "3/4").unit("PCs").ui(ui)`
#[must_use = "pass to ui.add() or call .ui(ui)"]
pub struct StatTile {
    label: String,
    value: String,
    unit: Option<String>,
    value_color: Option<Color32>,
    min_width: f32,
    theme: Theme,
}

impl StatTile {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            unit: None,
            value_color: None,
            min_width: 90.0,
            theme: DARK.clone(),
        }
    }

    /// Unit drawn small after the value (`ms`, `%`, `evt/min`).
    pub fn unit(mut self, unit: impl Into<String>) -> Self {
        self.unit = Some(unit.into());
        self
    }

    /// Value color, e.g. a status color for an alarming number. Default: primary text.
    pub fn value_color(mut self, color: Color32) -> Self {
        self.value_color = Some(color);
        self
    }

    pub fn min_width(mut self, min_width: f32) -> Self {
        self.min_width = min_width;
        self
    }

    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }
}

impl Widget for StatTile {
    fn ui(self, ui: &mut Ui) -> Response {
        let label_font = FontId::proportional(11.0);
        let value_font = FontId::monospace(22.0);
        let unit_font = FontId::monospace(11.0);

        let painter = ui.painter();
        let label_size = painter
            .layout_no_wrap(
                self.label.clone(),
                label_font.clone(),
                self.theme.text_muted,
            )
            .size();
        let value_size = painter
            .layout_no_wrap(self.value.clone(), value_font.clone(), self.theme.text)
            .size();
        let unit_size = self
            .unit
            .as_ref()
            .map(|u| {
                painter
                    .layout_no_wrap(u.clone(), unit_font.clone(), self.theme.text_muted)
                    .size()
            })
            .unwrap_or(Vec2::ZERO);

        let width = self
            .min_width
            .max(label_size.x)
            .max(value_size.x + unit_size.x + 4.0);
        let height = label_size.y + 2.0 + value_size.y;
        let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
        if !ui.is_rect_visible(rect) {
            return response;
        }

        let painter = ui.painter();
        painter.text(
            rect.left_top(),
            Align2::LEFT_TOP,
            &self.label,
            label_font,
            self.theme.text_muted,
        );
        let value_pos = pos2(rect.left(), rect.top() + label_size.y + 2.0);
        painter.text(
            value_pos,
            Align2::LEFT_TOP,
            &self.value,
            value_font,
            self.value_color.unwrap_or(self.theme.text),
        );
        if let Some(unit) = &self.unit {
            painter.text(
                pos2(value_pos.x + value_size.x + 4.0, rect.bottom() - 3.0),
                Align2::LEFT_BOTTOM,
                unit,
                unit_font,
                self.theme.text_muted,
            );
        }
        response
    }
}
