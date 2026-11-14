//! Colours from the desktop's theme (THM-18).
//!
//! On omarchy the desktop theme's colours are in `~/.local/state/omarchy/current/theme/colors.toml` (`accent`,
//! `background`, `foreground`, ... as `"#rrggbb"`, and `mode`); dot's KiCad pie menu reads the same file.
//! [`DesktopColors::parse`] reads it, [`Tokens::with_desktop`] puts its colours into the tokens where they fit
//! the token scheme, [`Tokens::with_accent`] takes only an accent (a fixed colour), and [`ColorSource`] is the
//! setting that chooses between them and the brand colours. Every text and line colour is made legible again
//! afterwards ([`Tokens::contrast_problems`] lists what would not be), so a theme can't make the app unreadable.
//!
//! Reading and watching the file is native only: [`crate::Look`]. This module is plain data, for the web too.

use egui::{Color32, Theme};

use crate::Tokens;
use crate::contrast::{AA_TEXT, contrast};

/// The colours a desktop theme gives: omarchy's `colors.toml`. A key the file lacks is `None`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DesktopColors {
    /// The theme's name (omarchy's `theme.name`, e.g. `tokyo-night`), when known.
    pub name: Option<String>,
    /// `mode = "dark"` or `"light"`.
    pub mode: Option<Theme>,
    /// The accent: buttons, focus, the lit pie slice, the first chart series.
    pub accent: Option<Color32>,
    /// The window background.
    pub background: Option<Color32>,
    /// A step lighter than the background: panels and windows.
    pub lighter_background: Option<Color32>,
    /// The text.
    pub foreground: Option<Color32>,
    /// Quieter text.
    pub dark_foreground: Option<Color32>,
    /// Selected text and items.
    pub selection: Option<Color32>,
}

impl DesktopColors {
    /// Read omarchy's `colors.toml`: `key = "#rrggbb"` lines and `mode = "dark"`. Comments, unknown keys and
    /// lines that are not a colour are skipped, so a newer file still gives what it can.
    pub fn parse(text: &str) -> Self {
        let mut d = Self::default();
        for line in text.lines() {
            if line.trim_start().starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let value = value.trim().trim_matches('"').trim_matches('\'');
            if key == "mode" {
                d.mode = match value {
                    "dark" => Some(Theme::Dark),
                    "light" => Some(Theme::Light),
                    _ => None,
                };
                continue;
            }
            let Some(color) = parse_hex(value) else {
                continue;
            };
            let slot = match key {
                "accent" => &mut d.accent,
                "background" => &mut d.background,
                "lighter_background" => &mut d.lighter_background,
                "foreground" => &mut d.foreground,
                "dark_foreground" => &mut d.dark_foreground,
                "selection" => &mut d.selection,
                _ => continue,
            };
            *slot = Some(color);
        }
        d
    }

    /// Whether the file gave anything this crate uses.
    pub fn is_empty(&self) -> bool {
        self.accent.is_none() && self.background.is_none() && self.foreground.is_none()
    }

    /// Dark or light: `mode`, else from the background's luminance, else dark.
    pub fn theme(&self) -> Theme {
        self.mode.unwrap_or(match self.background {
            Some(bg) if crate::contrast::luminance(bg) > 0.4 => Theme::Light,
            _ => Theme::Dark,
        })
    }
}

/// `#rrggbb` (or `rrggbb`) as a colour.
pub fn parse_hex(s: &str) -> Option<Color32> {
    let h = s.trim().trim_start_matches('#');
    if h.len() != 6 || !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let v = u32::from_str_radix(h, 16).ok()?;
    Some(Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

/// A colour as `#rrggbb`.
pub fn to_hex(c: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
}

/// Where the app's colours come from: a setting. Saved as `"desktop"`, `"brand"` or `"#rrggbb"`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColorSource {
    /// The desktop theme (omarchy's `colors.toml`) when there is one: its accent in both styles, its
    /// background and text in the style of its mode; the brand colours where there is none.
    #[default]
    Desktop,
    /// The vhrd brand colours.
    Brand,
    /// The brand colours with this accent.
    Fixed(Color32),
}

impl ColorSource {
    /// `"desktop"`, `"brand"` or `"#rrggbb"`.
    pub fn as_setting(&self) -> String {
        match self {
            ColorSource::Desktop => "desktop".into(),
            ColorSource::Brand => "brand".into(),
            ColorSource::Fixed(c) => to_hex(*c),
        }
    }

    /// The setting back: `"desktop"`, `"brand"` or `"#rrggbb"`; anything else is `None`.
    pub fn from_setting(s: &str) -> Option<Self> {
        match s.trim() {
            "desktop" => Some(ColorSource::Desktop),
            "brand" => Some(ColorSource::Brand),
            other => parse_hex(other).map(ColorSource::Fixed),
        }
    }

    /// The dark and light tokens under this source; `desktop` is the desktop theme's colours, if any.
    pub fn tokens(&self, desktop: Option<&DesktopColors>) -> (Tokens, Tokens) {
        let (dark, light) = (Tokens::dark(), Tokens::light());
        match self {
            ColorSource::Brand => (dark, light),
            ColorSource::Fixed(c) => (dark.with_accent(*c), light.with_accent(*c)),
            ColorSource::Desktop => match desktop.filter(|d| !d.is_empty()) {
                None => (dark, light),
                Some(d) => match d.theme() {
                    Theme::Dark => (dark.with_desktop(d), light.with_desktop_accent(d)),
                    Theme::Light => (dark.with_desktop_accent(d), light.with_desktop(d)),
                },
            },
        }
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for ColorSource {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.as_setting())
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for ColorSource {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Self::from_setting(&s).ok_or_else(|| {
            serde::de::Error::custom(format!(
                "{s:?}: expected \"desktop\", \"brand\" or \"#rrggbb\""
            ))
        })
    }
}

/// `fg` moved toward `toward` until it reaches `min` on every one of `bgs`.
fn legible_on_all(fg: Color32, bgs: &[Color32], toward: Color32, min: f32) -> Color32 {
    let ok = |c: Color32| bgs.iter().all(|bg| contrast(c, *bg) >= min);
    if ok(fg) {
        return fg;
    }
    (1..=20)
        .map(|k| fg.lerp_to_gamma(toward, k as f32 / 20.0))
        .find(|c| ok(*c))
        .unwrap_or(toward)
}

/// White or black, whichever stands off `bg` more.
fn extreme_from(bg: Color32) -> Color32 {
    if contrast(Color32::WHITE, bg) >= contrast(Color32::BLACK, bg) {
        Color32::WHITE
    } else {
        Color32::BLACK
    }
}

impl Tokens {
    /// These tokens with `accent` as the interaction and first series colour (`accent`, `primary`, `focus`,
    /// and the `selection` made from it), each made legible where it is used.
    pub fn with_accent(mut self, accent: Color32) -> Self {
        let c = &mut self.colors;
        // Icons, outlines and series: 3:1 on the surfaces.
        let accent = legible_on_all(accent, &[c.bg, c.surface], extreme_from(c.bg), 3.0);
        c.accent = accent;
        c.focus = accent;
        c.primary = accent;
        c.selection = accent.lerp_to_gamma(c.bg, 0.7);
        self.fix_contrast();
        self
    }

    /// These tokens with the desktop theme's accent only (for the style of the other mode).
    pub fn with_desktop_accent(self, d: &DesktopColors) -> Self {
        match d.accent {
            Some(a) => self.with_accent(a),
            None => self,
        }
    }

    /// These tokens with the desktop theme's colours: its background as `bg` (and the steps above it made
    /// from it, or its `lighter_background` as `surface`), its foreground as `text`, its `dark_foreground` as
    /// `text_muted`, its `selection`, and its accent ([`Tokens::with_accent`]). Status, categorical and brand
    /// colours stay; every colour is then made legible on the new backgrounds.
    pub fn with_desktop(mut self, d: &DesktopColors) -> Self {
        let c = &mut self.colors;
        if let Some(bg) = d.background {
            // A dark theme's background stays dark enough for the brand's status fills (and a light one light
            // enough): a mid grey is moved toward black or white.
            let end = match d.theme() {
                Theme::Dark => Color32::BLACK,
                Theme::Light => Color32::WHITE,
            };
            let other = extreme_from(end);
            let bg = (0..=20)
                .map(|k| bg.lerp_to_gamma(end, k as f32 / 20.0))
                .find(|b| contrast(other, *b) >= 10.0)
                .unwrap_or(end);
            let toward = d.foreground.unwrap_or(c.text);
            c.bg = bg;
            c.surface = d
                .lighter_background
                .filter(|s| contrast(*s, bg) < 1.6)
                .unwrap_or_else(|| bg.lerp_to_gamma(toward, 0.04));
            c.surface_raised = c.surface.lerp_to_gamma(toward, 0.06);
            c.hover = c.surface.lerp_to_gamma(toward, 0.12);
            c.line = bg.lerp_to_gamma(toward, 0.14);
            c.line_strong = bg.lerp_to_gamma(toward, 0.28);
        }
        if let Some(fg) = d.foreground {
            c.text = fg;
            c.title = fg.lerp_to_gamma(c.bg, 0.15);
            c.text_muted = d.dark_foreground.unwrap_or(fg.lerp_to_gamma(c.bg, 0.35));
        }
        let accent = d.accent.unwrap_or(c.accent);
        let mut t = self.with_accent(accent);
        if let Some(sel) = d.selection {
            t.colors.selection = sel;
            t.fix_contrast();
        }
        t
    }

    /// Make every text and line colour legible on what it sits on again, after a background or text colour
    /// changed: the rules of [`Tokens::contrast_problems`], fixed by moving the colour toward the text (or
    /// toward white or black) only as far as needed, so hues stay recognisable.
    fn fix_contrast(&mut self) {
        let c = &mut self.colors;
        let backgrounds = [c.bg, c.surface, c.surface_raised, c.hover];
        let strong = extreme_from(c.bg);
        c.text = legible_on_all(c.text, &backgrounds, strong, AA_TEXT);
        c.text_muted = legible_on_all(c.text_muted, &backgrounds, c.text, AA_TEXT);
        c.title = legible_on_all(c.title, &backgrounds, c.text, AA_TEXT);
        // Selected text is the text on the selection fill: move the fill toward the background until it reads.
        if contrast(c.text, c.selection) < AA_TEXT {
            c.selection = (1..=20)
                .map(|k| c.selection.lerp_to_gamma(c.bg, k as f32 / 20.0))
                .find(|s| contrast(c.text, *s) >= AA_TEXT)
                .unwrap_or(c.bg);
        }
        for color in [&mut c.slug, &mut c.task, &mut c.accent_alt] {
            *color = legible_on_all(*color, &[c.bg, c.surface], c.text, 3.0);
        }
        for r in [
            &mut c.status.error,
            &mut c.status.warning,
            &mut c.status.ok,
            &mut c.status.info,
            &mut c.status.neutral,
        ] {
            r.solid = legible_on_all(
                r.solid,
                &[c.bg, c.surface, r.soft, r.hover],
                c.text,
                AA_TEXT,
            );
            r.line = legible_on_all(r.line, &[c.bg, c.surface], c.text, 3.0);
        }
        c.good = c.status.ok.solid;
        c.warn = c.status.warning.solid;
        c.crit = c.status.error.solid;
        c.off = c.status.neutral.line;
        // A filled button's text: whichever of text, bg and surface_raised reads best; else the fill moves away
        // from it.
        let candidates = [c.text, c.bg, c.surface_raised];
        let on = |fill: Color32| {
            candidates
                .into_iter()
                .map(|t| contrast(t, fill))
                .fold(0.0, f32::max)
        };
        for fill in [&mut c.primary, &mut c.crit] {
            if on(*fill) < AA_TEXT {
                let best = candidates
                    .into_iter()
                    .max_by(|a, b| contrast(*a, *fill).total_cmp(&contrast(*b, *fill)))
                    .unwrap_or(c.text);
                let away = if contrast(best, Color32::WHITE) > contrast(best, Color32::BLACK) {
                    Color32::WHITE
                } else {
                    Color32::BLACK
                };
                *fill = (1..=20)
                    .map(|k| fill.lerp_to_gamma(away, k as f32 / 20.0))
                    .find(|f| on(*f) >= AA_TEXT)
                    .unwrap_or(away);
            }
        }
    }

    /// What does not read: every pair the theme promises a contrast for (THM-2) that falls short, as
    /// `"text on hover: 3.2"`. Empty for the brand sets and for any set made by [`Tokens::with_desktop`] or
    /// [`Tokens::with_accent`].
    pub fn contrast_problems(&self, theme: Theme) -> Vec<String> {
        let c = &self.colors;
        let mut out = Vec::new();
        let mut need = |what: &str, fg: Color32, bg: Color32, min: f32| {
            let k = contrast(fg, bg);
            if k < min {
                out.push(format!("{what}: {k:.2}"));
            }
        };
        for (bg_name, bg) in [
            ("bg", c.bg),
            ("surface", c.surface),
            ("surface_raised", c.surface_raised),
            ("hover", c.hover),
        ] {
            need(&format!("text on {bg_name}"), c.text, bg, AA_TEXT);
            need(
                &format!("text_muted on {bg_name}"),
                c.text_muted,
                bg,
                AA_TEXT,
            );
        }
        need("text on selection", c.text, c.selection, AA_TEXT);
        for (n, fill) in [("primary", c.primary), ("crit", c.crit)] {
            need(&format!("on({n})"), self.on(fill), fill, AA_TEXT);
        }
        for (n, fg) in [("slug", c.slug), ("task", c.task), ("accent", c.accent)] {
            for (bg_name, bg) in [("bg", c.bg), ("surface", c.surface)] {
                need(&format!("{n} on {bg_name}"), fg, bg, 3.0);
            }
        }
        let st = c.status;
        for (group, r) in [
            ("error", st.error),
            ("warning", st.warning),
            ("ok", st.ok),
            ("info", st.info),
            ("neutral", st.neutral),
        ] {
            for (bg_name, bg) in [
                ("bg", c.bg),
                ("surface", c.surface),
                ("soft", r.soft),
                ("hover", r.hover),
            ] {
                need(&format!("{group} solid on {bg_name}"), r.solid, bg, AA_TEXT);
            }
            for (bg_name, bg) in [("bg", c.bg), ("surface", c.surface)] {
                need(&format!("{group} line on {bg_name}"), r.line, bg, 3.0);
            }
        }
        let v = self.visuals(theme);
        for (what, fg, bg) in [
            ("warn text", v.warn_fg_color, c.bg),
            ("error text", v.error_fg_color, c.bg),
            ("link", v.hyperlink_color, c.bg),
            (
                "selected text",
                v.selection.stroke.color,
                v.selection.bg_fill,
            ),
        ] {
            need(what, fg, bg, AA_TEXT);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tokyo Night, as omarchy ships it (a copy, so the test needs no desktop).
    pub(crate) const TOKYO_NIGHT: &str = r##"mode = "dark"

accent = "#7aa2f7"
selection = "#292e42"
muted = "#414868"

background = "#1a1b26"
dark_background = "#13141c"
lighter_background = "#24283b"

foreground = "#a9b1d6"
dark_foreground = "#565f89"
bright_foreground = "#c0caf5"

red = "#f7768e"
"##;

    const LIGHT: &str = r##"mode = "light"
accent = "#1e66f5"
background = "#eff1f5"
foreground = "#4c4f69"
dark_foreground = "#9ca0b0"
selection = "#ccd0da"
"##;

    /// Mid grey everywhere: nothing reads until it is fixed.
    const MUDDY: &str = r##"accent = "#808080"
background = "#7a7a7a"
foreground = "#8a8a8a"
selection = "#858585"
"##;

    #[test]
    fn parses_omarchy_colors() {
        let d = DesktopColors::parse(TOKYO_NIGHT);
        assert_eq!(d.mode, Some(Theme::Dark));
        assert_eq!(d.accent, Some(Color32::from_rgb(0x7a, 0xa2, 0xf7)));
        assert_eq!(d.background, Some(Color32::from_rgb(0x1a, 0x1b, 0x26)));
        assert_eq!(d.foreground, Some(Color32::from_rgb(0xa9, 0xb1, 0xd6)));
        assert_eq!(d.theme(), Theme::Dark);
        assert!(DesktopColors::parse("garbage\n= \nfoo = 3").is_empty());
        assert_eq!(DesktopColors::parse(LIGHT).theme(), Theme::Light);
        assert_eq!(DesktopColors::parse(MUDDY).theme(), Theme::Dark);
    }

    #[test]
    fn the_setting_round_trips() {
        for s in [
            ColorSource::Desktop,
            ColorSource::Brand,
            ColorSource::Fixed(Color32::from_rgb(1, 0xa2, 0xff)),
        ] {
            assert_eq!(ColorSource::from_setting(&s.as_setting()), Some(s));
        }
        assert_eq!(ColorSource::from_setting("blue"), None);
    }

    #[test]
    fn desktop_colours_go_where_they_fit() {
        let d = DesktopColors::parse(TOKYO_NIGHT);
        let (dark, light) = ColorSource::Desktop.tokens(Some(&d));
        assert_eq!(dark.colors.accent, d.accent.unwrap());
        assert_eq!(dark.colors.focus, d.accent.unwrap());
        assert_eq!(dark.colors.bg, d.background.unwrap());
        assert_eq!(dark.colors.surface, d.lighter_background.unwrap());
        assert_eq!(dark.colors.text, d.foreground.unwrap());
        // The light style keeps the brand's paper, with the theme's accent made legible on it.
        assert_eq!(light.colors.bg, Tokens::light().colors.bg);
        assert_ne!(light.colors.accent, Tokens::light().colors.accent);
        // Brand: as before; no desktop file: brand.
        assert_eq!(ColorSource::Brand.tokens(Some(&d)).0, Tokens::dark());
        assert_eq!(ColorSource::Desktop.tokens(None).0, Tokens::dark());
    }

    /// The contrast checks pass for the brand sets and for every set made from a desktop theme or an accent,
    /// muddy ones included.
    #[test]
    fn every_source_keeps_the_contrast() {
        let mut sets = vec![
            ("brand dark".to_string(), Tokens::dark(), Theme::Dark),
            ("brand light".to_string(), Tokens::light(), Theme::Light),
        ];
        for (name, text) in [("tokyo", TOKYO_NIGHT), ("light", LIGHT), ("muddy", MUDDY)] {
            let d = DesktopColors::parse(text);
            let (dark, light) = ColorSource::Desktop.tokens(Some(&d));
            sets.push((format!("{name} dark"), dark, Theme::Dark));
            sets.push((format!("{name} light"), light, Theme::Light));
        }
        for hex in ["#ffff00", "#000080", "#808080", "#ffffff", "#000000"] {
            let c = parse_hex(hex).unwrap();
            let (dark, light) = ColorSource::Fixed(c).tokens(None);
            sets.push((format!("{hex} dark"), dark, Theme::Dark));
            sets.push((format!("{hex} light"), light, Theme::Light));
        }
        for (name, t, theme) in sets {
            let problems = t.contrast_problems(theme);
            assert!(problems.is_empty(), "{name}: {problems:?}");
        }
    }
}
