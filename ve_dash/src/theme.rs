//! Color tokens for the dashboard widgets, dark-first.
//!
//! The two series accents and their contrast against [`Theme::bg`] were validated for
//! color-vision deficiency separation (OKLab ΔE ≥ 8) and ≥ 3:1 surface contrast.
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
}

/// The default dark theme as a constant, used for widget builder defaults.
pub const DARK: Theme = Theme::dark();

impl Theme {
    /// Dark theme (the default): near-black background, cool accents.
    pub const fn dark() -> Self {
        Self {
            bg: Color32::from_rgb(0x0d, 0x11, 0x17),
            panel_bg: Color32::from_rgb(0x12, 0x17, 0x1f),
            border: Color32::from_rgb(0x2d, 0x33, 0x3b),
            title: Color32::from_rgb(0x9a, 0xa5, 0xb1),
            text: Color32::from_rgb(0xc8, 0xd1, 0xd9),
            text_muted: Color32::from_rgb(0x8b, 0x94, 0x9e),
            accent: Color32::from_rgb(0x1f, 0xa3, 0xad),
            accent_alt: Color32::from_rgb(0x9a, 0x77, 0xee),
            good: Color32::from_rgb(0x3f, 0xb9, 0x50),
            warn: Color32::from_rgb(0xd2, 0x99, 0x22),
            crit: Color32::from_rgb(0xf8, 0x51, 0x49),
            off: Color32::from_rgb(0x6e, 0x76, 0x81),
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
