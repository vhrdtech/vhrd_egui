//! Segmented horizontal meter, btop style: small blocks lighting up along a
//! good → warn → crit gradient.

use egui::{Align2, Color32, FontId, Rect, Response, Sense, Ui, Vec2, Widget, pos2, vec2};

use crate::theme::{DARK, Theme, heat};

/// How many blocks a [`Meter`] shows. A fixed count keeps bars across an app comparable (the default,
/// [`Blocks::Ten`]); use [`Blocks::Auto`] only where the width itself should carry meaning (today's
/// behaviour: the block count grows with the available width).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Blocks {
    /// Each block is 10 %.
    #[default]
    Ten,
    /// Each block is 5 %.
    Twenty,
    /// Each block is 4 %.
    TwentyFive,
    /// Each block is 2 %.
    Fifty,
    /// Each block is 1 %.
    Hundred,
    /// A custom count, clamped to `1..=200`.
    Custom(u16),
    /// Grows with the width (today's default behaviour before fixed counts: a 4 px block, 2 px gap, as
    /// many as fit); use where the width itself should carry meaning.
    Auto,
}

impl Blocks {
    /// Every fixed count, in display order, plus [`Blocks::Auto`]: for a settings combo box.
    /// [`Blocks::Custom`] takes its own count and is left out.
    pub const ALL: [Blocks; 6] = [
        Blocks::Ten,
        Blocks::Twenty,
        Blocks::TwentyFive,
        Blocks::Fifty,
        Blocks::Hundred,
        Blocks::Auto,
    ];

    /// Label for a settings combo box (`"10"`, `"25"`, `"Auto"`, ...).
    pub fn label(self) -> String {
        match self {
            Blocks::Ten => "10".into(),
            Blocks::Twenty => "20".into(),
            Blocks::TwentyFive => "25".into(),
            Blocks::Fifty => "50".into(),
            Blocks::Hundred => "100".into(),
            Blocks::Custom(n) => format!("Custom ({n})"),
            Blocks::Auto => "Auto".into(),
        }
    }

    /// Block count for a meter `bar_w` points wide. Ignores `bar_w` except for [`Blocks::Auto`], which
    /// grows with it (a 4 px block, 2 px gap, as many as fit, at least one).
    pub fn count(self, bar_w: f32) -> usize {
        match self {
            Blocks::Ten => 10,
            Blocks::Twenty => 20,
            Blocks::TwentyFive => 25,
            Blocks::Fifty => 50,
            Blocks::Hundred => 100,
            Blocks::Custom(n) => n.clamp(1, 200) as usize,
            Blocks::Auto => ((bar_w + GAP_W) / (BLOCK_W + GAP_W)).floor().max(1.0) as usize,
        }
    }
}

/// Blocks lit for `frac` (clamped to `0..=1`, NaN is 0): rounded to the nearest block, except that any
/// non-zero value lights at least one so a tiny load never reads as "nothing".
pub(crate) fn lit_blocks(frac: f32, n: usize) -> usize {
    if !frac.is_finite() || frac <= 0.0 {
        return 0;
    }
    ((frac.min(1.0) * n as f32).round() as usize).clamp(1, n)
}

/// Block width and gap for `n` fixed blocks in `bar_w` points: stretches the block to fill, shrinking the
/// gap (2 -> 1 -> 0 px) before letting a block drop under 1 px; never drops a block.
fn layout_blocks(n: usize, bar_w: f32) -> (f32, f32) {
    for gap in [GAP_W, 1.0, 0.0] {
        let block_w = (bar_w - gap * (n - 1) as f32) / n as f32;
        if block_w >= 1.0 || gap == 0.0 {
            return (block_w.max(1.0), gap);
        }
    }
    unreachable!("the gap == 0.0 branch always returns")
}

/// `Meter::new(0.63).ui(ui)` draws a block meter filled to 63 %.
///
/// By default blocks take the heat gradient by their position, so a nearly full
/// meter ends in red (load semantics). Use [`Meter::color`] for a neutral fill.
#[must_use = "pass to ui.add() or call .ui(ui)"]
pub struct Meter {
    frac: f32,
    blocks: Blocks,
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
            blocks: Blocks::default(),
            width: None,
            height: 12.0,
            color: None,
            text: None,
            theme: DARK.clone(),
        }
    }

    /// How many blocks to show. Default [`Blocks::Ten`]; only [`Blocks::Auto`] grows with the width.
    pub fn blocks(mut self, blocks: Blocks) -> Self {
        self.blocks = blocks;
        self
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
        let min_bar_w = match self.blocks {
            Blocks::Auto => BLOCK_W,
            fixed => fixed.count(0.0) as f32, // one point per block, no gap: the narrowest that fits all
        };
        let width = self
            .width
            .unwrap_or_else(|| ui.available_width())
            .max(text_w + min_bar_w);
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(width, self.height), Sense::hover());
        if !ui.is_rect_visible(rect) {
            return response;
        }
        let painter = ui.painter();

        let bar_w = rect.width() - text_w;
        let n = self.blocks.count(bar_w);
        let (block_w, gap_w) = match self.blocks {
            Blocks::Auto => (BLOCK_W, GAP_W),
            _ => layout_blocks(n, bar_w),
        };
        let lit = lit_blocks(self.frac, n);
        for i in 0..n {
            let x = rect.left() + i as f32 * (block_w + gap_w);
            let block = Rect::from_min_size(pos2(x, rect.top()), vec2(block_w, rect.height()));
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
        response.on_hover_text(format!("{:.0} % ({lit} of {n} blocks)", self.frac * 100.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lit_rounds_and_never_hides_nonzero() {
        assert_eq!(lit_blocks(0.0, 10), 0);
        assert_eq!(lit_blocks(0.01, 10), 1);
        assert_eq!(lit_blocks(0.63, 10), 6);
        assert_eq!(lit_blocks(0.5, 5), 3);
        assert_eq!(lit_blocks(7.0, 5), 5);
        assert_eq!(lit_blocks(f32::NAN, 5), 0);
    }

    #[test]
    fn count_is_fixed_except_for_auto() {
        assert_eq!(Blocks::Ten.count(1.0), 10);
        assert_eq!(Blocks::Ten.count(1000.0), 10);
        assert_eq!(Blocks::Twenty.count(1.0), 20);
        assert_eq!(Blocks::TwentyFive.count(1.0), 25);
        assert_eq!(Blocks::Fifty.count(1.0), 50);
        assert_eq!(Blocks::Hundred.count(1.0), 100);
        assert_eq!(Blocks::Custom(7).count(1.0), 7);
        // clamped to 1..=200
        assert_eq!(Blocks::Custom(0).count(1.0), 1);
        assert_eq!(Blocks::Custom(1000).count(1.0), 200);
    }

    #[test]
    fn auto_grows_with_the_width() {
        assert_eq!(Blocks::Auto.count(0.0), 1);
        assert_eq!(Blocks::Auto.count(4.0), 1);
        assert_eq!(Blocks::Auto.count(20.0), 3); // (20 + 2) / (4 + 2) = 3.67, floored
        assert!(Blocks::Auto.count(600.0) > Blocks::Auto.count(60.0));
    }

    #[test]
    fn default_is_ten() {
        assert_eq!(Blocks::default(), Blocks::Ten);
    }

    #[test]
    fn layout_shrinks_the_gap_before_the_block_and_never_drops_a_block() {
        // Plenty of room: the full 2 px gap.
        let (w, g) = layout_blocks(10, 100.0);
        assert_eq!(g, 2.0);
        assert!(w >= 1.0);
        // Tight: the gap shrinks to 1, then 0, but all 10 blocks still fit.
        let (w, g) = layout_blocks(10, 15.0);
        assert_eq!(g, 0.0);
        assert!(w >= 1.0);
        // Narrower than one point per block: every block still gets at least 1 px (may overflow).
        let (w, g) = layout_blocks(50, 10.0);
        assert_eq!(g, 0.0);
        assert_eq!(w, 1.0);
    }
}
