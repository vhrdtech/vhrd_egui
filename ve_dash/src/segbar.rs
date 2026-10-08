//! Segmented percent bar: exactly 5 or 10 segments, for coarse "how full" at a glance.

use egui::{Color32, Rect, Response, Sense, Ui, Vec2, Widget, pos2, vec2};

use crate::theme::{DARK, Theme, heat};

/// How many segments a [`SegBar`] has. Only these two, so bars across an app stay comparable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Segments {
    /// Each segment is 20 %.
    Five,
    /// Each segment is 10 %.
    Ten,
}

impl Segments {
    /// 5 or 10.
    pub fn count(self) -> usize {
        match self {
            Segments::Five => 5,
            Segments::Ten => 10,
        }
    }
}

/// Segments lit for `frac` (clamped to `0..=1`, NaN is 0): rounded to the nearest segment, except that any
/// non-zero value lights at least one so a tiny load never reads as "nothing".
pub fn lit_segments(frac: f32, segments: Segments) -> usize {
    if !frac.is_finite() || frac <= 0.0 {
        return 0;
    }
    let n = segments.count();
    ((frac.min(1.0) * n as f32).round() as usize).clamp(1, n)
}

/// `SegBar::ten(0.63).ui(ui)`: 10 blocks, 6 lit (heat gradient by position), tooltip "63 % (6 of 10 segments)".
#[must_use = "pass to ui.add() or call .ui(ui)"]
pub struct SegBar {
    frac: f32,
    segments: Segments,
    width: f32,
    height: f32,
    color: Option<Color32>,
    theme: Theme,
}

impl SegBar {
    /// `frac` is clamped to `0..=1`.
    pub fn new(frac: f32, segments: Segments) -> Self {
        Self {
            frac: if frac.is_finite() {
                frac.clamp(0.0, 1.0)
            } else {
                0.0
            },
            segments,
            width: 60.0,
            height: 10.0,
            color: None,
            theme: DARK.clone(),
        }
    }

    /// 5 segments.
    pub fn five(frac: f32) -> Self {
        Self::new(frac, Segments::Five)
    }

    /// 10 segments.
    pub fn ten(frac: f32) -> Self {
        Self::new(frac, Segments::Ten)
    }

    /// Total width (default 60).
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    /// One fixed color for lit segments instead of the heat gradient.
    pub fn color(mut self, color: Color32) -> Self {
        self.color = Some(color);
        self
    }

    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }
}

const GAP: f32 = 2.0;

impl Widget for SegBar {
    fn ui(self, ui: &mut Ui) -> Response {
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(self.width, self.height), Sense::hover());
        let n = self.segments.count();
        let lit = lit_segments(self.frac, self.segments);
        if ui.is_rect_visible(rect) {
            let w = (rect.width() - GAP * (n - 1) as f32) / n as f32;
            for i in 0..n {
                let x = rect.left() + i as f32 * (w + GAP);
                let block = Rect::from_min_size(pos2(x, rect.top()), vec2(w, rect.height()));
                let color = if i < lit {
                    self.color
                        .unwrap_or_else(|| heat(&self.theme, i as f32 / (n - 1) as f32))
                } else {
                    self.theme.border.gamma_multiply(0.6)
                };
                ui.painter().rect_filled(block, 1.0, color);
            }
        }
        response.on_hover_text(format!(
            "{:.0} % ({lit} of {n} segments)",
            self.frac * 100.0
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lit_rounds_and_never_hides_nonzero() {
        assert_eq!(lit_segments(0.0, Segments::Ten), 0);
        assert_eq!(lit_segments(0.01, Segments::Ten), 1);
        assert_eq!(lit_segments(0.63, Segments::Ten), 6);
        assert_eq!(lit_segments(0.5, Segments::Five), 3);
        assert_eq!(lit_segments(7.0, Segments::Five), 5);
        assert_eq!(lit_segments(f32::NAN, Segments::Five), 0);
    }
}
