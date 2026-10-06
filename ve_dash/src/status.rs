//! Status LED with a soft glow.
//!
//! Color never carries the state alone: [`StatusLight`] pairs the dot with a
//! text label by default. Use [`StatusLight::dot_only`] only where the label
//! sits right next to it anyway.

use egui::{Color32, Response, Sense, Ui, Vec2, Widget};

use crate::theme::{DARK, Theme};

/// Node / link / service state, mapped onto the theme's status colors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Good,
    Warn,
    Crit,
    Off,
}

impl Status {
    pub fn color(self, theme: &Theme) -> Color32 {
        match self {
            Self::Good => theme.good,
            Self::Warn => theme.warn,
            Self::Crit => theme.crit,
            Self::Off => theme.off,
        }
    }
}

/// A small glowing dot plus a label: `StatusLight::new(Status::Good, "direct").ui(ui)`.
#[must_use = "pass to ui.add() or call .ui(ui)"]
pub struct StatusLight {
    status: Status,
    label: Option<String>,
    diameter: f32,
    theme: Theme,
}

impl StatusLight {
    pub fn new(status: Status, label: impl Into<String>) -> Self {
        Self {
            status,
            label: Some(label.into()),
            diameter: 8.0,
            theme: DARK.clone(),
        }
    }

    /// Dot without text, for places where the label is adjacent anyway.
    pub fn dot_only(status: Status) -> Self {
        Self {
            status,
            label: None,
            diameter: 8.0,
            theme: DARK.clone(),
        }
    }

    pub fn diameter(mut self, diameter: f32) -> Self {
        self.diameter = diameter;
        self
    }

    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    fn paint_dot(&self, ui: &mut Ui) -> Response {
        let glow = self.diameter * 0.9;
        let (rect, response) =
            ui.allocate_exact_size(Vec2::splat(self.diameter + glow), Sense::hover());
        if ui.is_rect_visible(rect) {
            let color = self.status.color(&self.theme);
            let painter = ui.painter();
            let c = rect.center();
            if self.status != Status::Off {
                painter.circle_filled(
                    c,
                    self.diameter * 0.5 + glow * 0.5,
                    color.gamma_multiply(0.12),
                );
                painter.circle_filled(
                    c,
                    self.diameter * 0.5 + glow * 0.25,
                    color.gamma_multiply(0.25),
                );
            }
            painter.circle_filled(c, self.diameter * 0.5, color);
        }
        response
    }
}

impl Widget for StatusLight {
    fn ui(self, ui: &mut Ui) -> Response {
        match self.label.clone() {
            None => self.paint_dot(ui),
            Some(label) => {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 5.0;
                    let text_color = self.theme.text_muted;
                    let r = self.paint_dot(ui);
                    r | ui.label(egui::RichText::new(label).color(text_color).size(11.0))
                })
                .inner
            }
        }
    }
}
