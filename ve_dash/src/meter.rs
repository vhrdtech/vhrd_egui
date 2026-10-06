//! Segmented horizontal meter, btop style: small blocks lighting up along a
//! good → warn → crit gradient.

use egui::{Align2, Color32, FontId, Rect, Response, Sense, Ui, Vec2, Widget, pos2, vec2};

use crate::theme::{DARK, Theme, heat};

/// `Meter::new(0.63).ui(ui)` draws a block meter filled to 63 %.
///
/// By default blocks take the heat gradient by their position, so a nearly full
/// meter ends in red (load semantics). Use [`Meter::color`] for a neutral fill.
#[must_use = "pass to ui.add() or call .ui(ui)"]
pub struct Meter {
    frac: f32,
    width: Option<f32>,
    height: f32,
    color: Option<Color32>,
    text: Option<String>,
    theme: Theme,
}

impl Meter {
    /// `frac` is clamped to `0..=1`.
    pub fn new(frac: f32) -> Self {
        Self {
            frac: if frac.is_finite() {
                frac.clamp(0.0, 1.0)
            } else {
                0.0
            },
            width: None,
            height: 12.0,
            color: None,
            text: None,
            theme: DARK.clone(),
        }
    }

    /// Fixed width. Default: available width.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    /// One fixed color for lit blocks instead of the heat gradient.
    pub fn color(mut self, color: Color32) -> Self {
        self.color = Some(color);
        self
    }

    /// Text drawn right of the meter (e.g. `"63%"`, `"4.2 ms"`).
    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self
    }

    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }
}

const BLOCK_W: f32 = 4.0;
const GAP_W: f32 = 2.0;

impl Widget for Meter {
    fn ui(self, ui: &mut Ui) -> Response {
        let text_w = if self.text.is_some() { 52.0 } else { 0.0 };
        let width = self
            .width
            .unwrap_or_else(|| ui.available_width())
            .max(BLOCK_W + text_w);
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(width, self.height), Sense::hover());
        if !ui.is_rect_visible(rect) {
            return response;
        }
        let painter = ui.painter();

        let bar_w = rect.width() - text_w;
        let n = ((bar_w + GAP_W) / (BLOCK_W + GAP_W)).floor().max(1.0) as usize;
        let lit = (self.frac * n as f32).round() as usize;
        for i in 0..n {
            let x = rect.left() + i as f32 * (BLOCK_W + GAP_W);
            let block = Rect::from_min_size(pos2(x, rect.top()), vec2(BLOCK_W, rect.height()));
            let color = if i < lit {
                self.color
                    .unwrap_or_else(|| heat(&self.theme, i as f32 / (n - 1).max(1) as f32))
            } else {
                self.theme.border.gamma_multiply(0.6)
            };
            painter.rect_filled(block, 1.0, color);
        }
        if let Some(text) = &self.text {
            painter.text(
                pos2(rect.right(), rect.center().y),
                Align2::RIGHT_CENTER,
                text,
                FontId::monospace(11.0),
                self.theme.text,
            );
        }
        response.on_hover_text(format!("{:.0} %", self.frac * 100.0))
    }
}
