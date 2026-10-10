//! Color tokens for the dashboard widgets, taken from `ve_theme`'s design tokens.
//!
//! [`Theme::from_tokens`] maps a `ve_theme::Tokens` set onto the fields the widgets
//! draw from; [`Theme::dark`] and [`Theme::light`] are the VHRD brand sets. In dark,
//! the two series accents are the brand CAN-teal and analog-purple signal hues,
//! darkened into the dark-mode chart lightness band. The pair's CVD separation (OKLab ΔE 7.7 deutan)
//! sits in the labels-required band, which ve_dash satisfies: every chart carries a
//! direct text label, and the two series never share one plot unlabeled.
//! Status colors are reserved for state and must always be paired with a text label
//! or icon, never used as the only carrier of meaning.

use egui::Color32;
use ve_theme::{Status, Tokens};

/// Color tokens the widgets draw from. Start from [`Theme::dark`] / [`Theme::light`]
/// (or [`Theme::from_tokens`] with your own tokens) and override fields.
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
    /// The brand status groups in full (error, warning, ok, info, neutral: solid, soft, line, hover).
    /// `status.info.hover` with `status.info.line` is the cross-reference highlight ([`Theme::highlight`]).
    pub status: Status,
    /// Categorical label colors (models, series, tags): violet, teal, blue, pink, lime. Never a state.
    pub cat: [Color32; 5],
    /// Brand red accent (wordmark, product name). Use sparingly; not a series color.
    pub red: Color32,
    /// Identifier text: an agent session's slug. Distinct from the series and status colors.
    pub slug: Color32,
    /// Identifier text: a task name or id. Distinct from `slug` and the series colors.
    pub task: Color32,
}

/// The default dark theme as a constant, used for widget builder defaults.
pub const DARK: Theme = Theme::dark();

impl Theme {
    /// The widget colors of a `ve_theme` token set.
    pub const fn from_tokens(tokens: &Tokens) -> Self {
        let c = &tokens.colors;
        Self {
            bg: c.bg,
            panel_bg: c.surface,
            border: c.line,
            title: c.title,
            text: c.text,
            text_muted: c.text_muted,
            accent: c.accent,
            accent_alt: c.accent_alt,
            good: c.good,
            warn: c.warn,
            crit: c.crit,
            off: c.off,
            status: c.status,
            cat: c.cat,
            red: c.red,
            slug: c.slug,
            task: c.task,
        }
    }

    /// Dark theme (the default): the VHRD brand dark variant, `ve_theme::Tokens::dark`.
    pub const fn dark() -> Self {
        Self::from_tokens(&Tokens::dark())
    }

    /// Light theme: the VHRD brand light set, `ve_theme::Tokens::light`.
    pub const fn light() -> Self {
        Self::from_tokens(&Tokens::light())
    }

    /// Fill and outline of a cross-reference highlight (hover one thing, its counterpart lights up): the
    /// info group's hover and line steps.
    pub const fn highlight(&self) -> (Color32, Color32) {
        (self.status.info.hover, self.status.info.line)
    }

    /// Apply the theme to egui's visuals: panel/window fills, text color, widget strokes.
    /// A minimal setup for dashboard-only apps; `ve_theme::setup` styles every egui widget
    /// in dark and light and is the better choice for apps.
    pub fn apply(&self, ctx: &egui::Context) {
        let mut visuals = if ve_theme::contrast::luminance(self.bg) < 0.5 {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
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

/// A series color that turns into an alarm as its load `t` (fraction of
/// capacity) nears 1: the series' own color up to half load, then toward
/// warn at 3/4 and crit at full. Keeps a chart's identity at normal load
/// and says "near full" without a separate meter.
pub fn load_color(theme: &Theme, base: Color32, t: f32) -> Color32 {
    let t = if t.is_finite() {
        t.clamp(0.0, 1.0)
    } else {
        0.0
    };
    if t <= 0.5 {
        base
    } else if t <= 0.75 {
        base.lerp_to_gamma(theme.warn, (t - 0.5) * 4.0)
    } else {
        theme.warn.lerp_to_gamma(theme.crit, (t - 0.75) * 4.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The token mapping keeps the colors ve_dash has always used (the snapshot depends on it).
    #[test]
    fn dark_from_tokens_is_unchanged() {
        let th = Theme::dark();
        assert_eq!(th.bg, Color32::from_rgb(0x0f, 0x0f, 0x10));
        assert_eq!(th.panel_bg, Color32::from_rgb(0x16, 0x16, 0x17));
        assert_eq!(th.border, Color32::from_rgb(0x2a, 0x2a, 0x2b));
        assert_eq!(th.title, Color32::from_rgb(0xa8, 0xa8, 0xa8));
        assert_eq!(th.text_muted, Color32::from_rgb(0x9a, 0x9a, 0x9a));
        assert_eq!(th.accent, Color32::from_rgb(0x11, 0x8e, 0xa1));
        assert_ne!(Theme::light(), th);
    }

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

    #[test]
    fn load_color_keeps_series_then_alarms() {
        let th = Theme::dark();
        assert_eq!(load_color(&th, th.accent, 0.0), th.accent);
        assert_eq!(load_color(&th, th.accent, 0.5), th.accent);
        assert_eq!(load_color(&th, th.accent, 0.75), th.warn);
        assert_eq!(load_color(&th, th.accent, 1.0), th.crit);
        assert_eq!(load_color(&th, th.accent, 7.0), th.crit);
        assert_eq!(load_color(&th, th.accent, f32::NAN), th.accent);
    }
}
