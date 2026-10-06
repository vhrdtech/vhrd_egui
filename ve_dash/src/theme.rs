//! Color tokens for the dashboard widgets, dark-first.
//!
//! The dark theme is the VHRD brand dark variant (vhrd_brand `web/palette.json`):
//! background, surface, line, text and status colors come straight from it; the two
//! series accents are the brand CAN-teal and analog-purple signal hues, darkened into
//! the dark-mode chart lightness band. The pair's CVD separation (OKLab ΔE 7.7 deutan)
//! sits in the labels-required band, which ve_dash satisfies: every chart carries a
//! direct text label, and the two series never share one plot unlabeled.
//! Status colors are reserved for state and must always be paired with a text label
//! or icon, never used as the only carrier of meaning.

use egui::Color32;

/// Color tokens the widgets draw from. Start from [`Theme::dark`] and override fields.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Theme {
    /// Window background behind the panels.
    pub bg: Color32,
    /// Panel fill.
    pub panel_bg: Color32,
    /// Panel border and unlit meter segments.
    pub border: Color32,
    /// Panel title text.
    pub title: Color32,
    /// Primary text.
    pub text: Color32,
    /// Secondary / muted text (labels, units).
    pub text_muted: Color32,
    /// Primary series color (sparklines).
    pub accent: Color32,
    /// Secondary series color.
    pub accent_alt: Color32,
    /// Status: healthy / online.
    pub good: Color32,
    /// Status: degraded / attention.
    pub warn: Color32,
    /// Status: failing / critical.
    pub crit: Color32,
    /// Status: offline / disabled.
    pub off: Color32,
    /// Brand red accent (wordmark, product name). Use sparingly; not a series color.
    pub red: Color32,
}

/// The default dark theme as a constant, used for widget builder defaults.
pub const DARK: Theme = Theme::dark();

impl Theme {
    /// Dark theme (the default): the VHRD brand dark variant.
    pub const fn dark() -> Self {
        Self {
            bg: Color32::from_rgb(0x0f, 0x0f, 0x10),         // dark.bg
            panel_bg: Color32::from_rgb(0x16, 0x16, 0x17),   // dark.surface
            border: Color32::from_rgb(0x2a, 0x2a, 0x2b),     // dark.line
            title: Color32::from_rgb(0xa8, 0xa8, 0xa8),      // grey.400
            text: Color32::from_rgb(0xec, 0xec, 0xec),       // dark.text
            text_muted: Color32::from_rgb(0x9a, 0x9a, 0x9a), // dark.muted
            accent: Color32::from_rgb(0x11, 0x8e, 0xa1),     // signal.can, chart-darkened
            accent_alt: Color32::from_rgb(0xaa, 0x74, 0xd4), // signal.analog, chart-darkened
            good: Color32::from_rgb(0x5c, 0xb8, 0x60),       // signal-dark.bus
            warn: Color32::from_rgb(0xff, 0x9a, 0x3c),       // signal-dark.power
            crit: Color32::from_rgb(0xf2, 0x4c, 0x44),       // dark.red
            off: Color32::from_rgb(0x6e, 0x6e, 0x6e),        // brand.grey
            red: Color32::from_rgb(0xf2, 0x4c, 0x44),        // dark.red
        }
    }

    /// Apply the theme to egui's visuals: panel/window fills, text color, widget strokes.
    /// Call once at startup (or when switching themes).
    pub fn apply(&self, ctx: &egui::Context) {
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = self.bg;
        visuals.window_fill = self.panel_bg;
        visuals.override_text_color = Some(self.text);
        visuals.widgets.noninteractive.bg_stroke.color = self.border;
        ctx.set_visuals(visuals);
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

/// Map `t` in `0..=1` onto the good → warn → crit gradient (meter and load colors).
pub fn heat(theme: &Theme, t: f32) -> Color32 {
    let t = if t.is_finite() {
        t.clamp(0.0, 1.0)
    } else {
        0.0
    };
    if t < 0.5 {
        theme.good.lerp_to_gamma(theme.warn, t * 2.0)
    } else {
        theme.warn.lerp_to_gamma(theme.crit, (t - 0.5) * 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heat_endpoints_and_garbage() {
        let th = Theme::dark();
        assert_eq!(heat(&th, 0.0), th.good);
        assert_eq!(heat(&th, 1.0), th.crit);
        assert_eq!(heat(&th, -3.0), th.good);
        assert_eq!(heat(&th, 42.0), th.crit);
        // NaN must not panic and falls back to the low end.
        assert_eq!(heat(&th, f32::NAN), th.good);
    }
}
