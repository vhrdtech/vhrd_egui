//! Deprecated: folded into [`crate::Meter`] and [`crate::Blocks`] (DASH-13).

use egui::{Color32, Response, Ui, Widget};

use crate::meter::{Blocks, Meter, lit_blocks};
use crate::theme::Theme;

/// Deprecated: use [`Blocks`] with [`Meter`] instead.
#[deprecated(
    since = "0.28.0",
    note = "use `ve_dash::Blocks` with `ve_dash::Meter` instead"
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Segments {
    /// Each segment is 20 %.
    Five,
    /// Each segment is 10 %.
    Ten,
}

#[allow(deprecated)]
impl Segments {
    /// 5 or 10.
    pub fn count(self) -> usize {
        match self {
            Segments::Five => 5,
            Segments::Ten => 10,
        }
    }

    fn blocks(self) -> Blocks {
        match self {
            Segments::Five => Blocks::Custom(5),
            Segments::Ten => Blocks::Ten,
        }
    }
}

/// Deprecated: use [`Blocks`] with [`Meter`] instead ([`Segments::Five`] is now [`Blocks::Custom(5)`],
/// [`Segments::Ten`] is [`Blocks::Ten`]).
#[deprecated(
    since = "0.28.0",
    note = "use `ve_dash::Meter` with `ve_dash::Blocks` instead; it lights the same way"
)]
#[allow(deprecated)]
pub fn lit_segments(frac: f32, segments: Segments) -> usize {
    lit_blocks(frac, segments.count())
}

/// `SegBar::ten(0.63).ui(ui)`: 10 blocks, 6 lit (heat gradient by position), tooltip "63 % (6 of 10 blocks)".
///
/// Deprecated: a thin forward to [`Meter`] with [`Blocks`]; use those directly.
#[deprecated(
    since = "0.28.0",
    note = "use `ve_dash::Blocks` with `ve_dash::Meter` instead"
)]
#[must_use = "pass to ui.add() or call .ui(ui)"]
pub struct SegBar {
    meter: Meter,
}

#[allow(deprecated)]
impl SegBar {
    /// `frac` is clamped to `0..=1`.
    pub fn new(frac: f32, segments: Segments) -> Self {
        Self {
            meter: Meter::new(frac)
                .blocks(segments.blocks())
                .width(60.0)
                .height(10.0),
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
        self.meter = self.meter.width(width);
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.meter = self.meter.height(height);
        self
    }

    /// One fixed color for lit segments instead of the heat gradient.
    pub fn color(mut self, color: Color32) -> Self {
        self.meter = self.meter.color(color);
        self
    }

    pub fn theme(mut self, theme: Theme) -> Self {
        self.meter = self.meter.theme(theme);
        self
    }
}

#[allow(deprecated)]
impl Widget for SegBar {
    fn ui(self, ui: &mut Ui) -> Response {
        self.meter.ui(ui)
    }
}

#[cfg(test)]
#[allow(deprecated)]
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
