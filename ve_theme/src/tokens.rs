//! Design tokens (THM-1) and the brand dark / light sets (THM-2).
//!
//! [`Tokens`] is the whole design system in one value: named colors ([`Palette`]), a spacing scale
//! ([`Space`]), corner radii ([`Radius`]), stroke widths ([`Strokes`]), shadows ([`Elevation`]) and the
//! type scale ([`TypeScale`]). [`Tokens::apply`] turns it into an egui [`Style`] for one theme, so plain egui
//! widgets already look right; the [`crate::UiExt`] helpers read the same tokens for what `Style` cannot say.
//!
//! Colors come from the vhrd_brand export: `brand/palette.json` is a copy of its `web/palette.json` (`just
//! brand` refreshes it) and `build.rs` turns it into the constants of [`vhrd`], so no brand hex is typed here
//! (THM-11). The hex values left below are this crate's own. Deviations, each for legibility in a dense
//! tool UI rather than a web page:
//! - `surface_raised` in dark is a step above `surface` (the brand has them equal) so menus and buttons
//!   stand off the panels.
//! - `line` in light is the brand `line-strong` (#d0d0d0): the brand hairline #e6e6e6 vanishes on `surface`.
//! - `text_muted` in light is #666666 (brand #6e6e6e) so it stays AA on `surface` too.
//! - Interaction (`primary`, `focus`, `selection`) uses the brand CAN teal, not the brand red: red is
//!   close to `crit` / danger, and a red "OK" button next to a red "Delete" one would say nothing.
//!
//! Status colors come from the brand status groups ([`Status`]: error, warning, ok, info, neutral, each a
//! [`Ramp`] of solid, soft, line and hover); `good`, `warn`, `crit` are the solids of ok, warning and error,
//! `off` the neutral line. Error is a crimson apart from the brand `red`, warning an amber apart from the
//! power orange. They mark state and are always paired with a text label
//! ([`crate::UiExt::badge`] carries one by construction); never the only carrier of meaning.

use egui::epaint::Shadow;
use egui::{
    Color32, CornerRadius, Id, Margin, Stroke, Style, Theme, Visuals, style::WidgetVisuals,
};
use std::sync::Arc;

use crate::contrast::{AA_TEXT, contrast, ensure_contrast};
use crate::typography::TypeScale;

/// The vhrd_brand palette as constants, generated from `brand/palette.json`: `vhrd::brand::RED`,
/// `vhrd::grey::S700`, `vhrd::status_dark::error::SOFT`, `vhrd::categorical::TEAL`, ...
pub mod vhrd {
    include!(concat!(env!("OUT_DIR"), "/brand.rs"));
}

/// One status group: the steps a state is drawn with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ramp {
    /// Text or icon: AA on the surfaces and on this tone's `soft` and `hover`.
    pub solid: Color32,
    /// Background of a badge, banner or row in this state.
    pub soft: Color32,
    /// Border or outline (3:1 or more on the surfaces).
    pub line: Color32,
    /// Highlight fill: a hovered or cross-referenced item, a step past `soft`.
    pub hover: Color32,
}

/// The brand status groups for one theme (vhrd_brand `status` / `status-dark`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Status {
    pub error: Ramp,
    pub warning: Ramp,
    pub ok: Ramp,
    /// Neutral information; its `hover` is the cross-reference highlight.
    pub info: Ramp,
    /// Off, disabled, no data.
    pub neutral: Ramp,
}

macro_rules! tone {
    ($set:ident, $group:ident) => {
        Ramp {
            solid: vhrd::$set::$group::SOLID,
            soft: vhrd::$set::$group::SOFT,
            line: vhrd::$set::$group::LINE,
            hover: vhrd::$set::$group::HOVER,
        }
    };
}

macro_rules! status {
    ($set:ident) => {
        Status {
            error: tone!($set, error),
            warning: tone!($set, warning),
            ok: tone!($set, ok),
            info: tone!($set, info),
            neutral: tone!($set, neutral),
        }
    };
}

macro_rules! categorical {
    ($set:ident) => {
        [
            vhrd::$set::VIOLET,
            vhrd::$set::TEAL,
            vhrd::$set::BLUE,
            vhrd::$set::PINK,
            vhrd::$set::LIME,
        ]
    };
}

/// Named color tokens. Field docs say where egui (or a helper) uses each.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    /// Window / central panel background (`panel_fill`).
    pub bg: Color32,
    /// Cards, windows, side panels' content frames (`window_fill`, `faint_bg_color`).
    pub surface: Color32,
    /// Raised things on a surface: buttons, menus, check boxes, title bars.
    pub surface_raised: Color32,
    /// Hairlines: separators, panel and window borders.
    pub line: Color32,
    /// Stronger lines: widget outlines, the inactive button border.
    pub line_strong: Color32,
    /// Panel and section titles (a step quieter than `text`).
    pub title: Color32,
    /// Primary text.
    pub text: Color32,
    /// Secondary text: captions, units, hints (`weak_text_color`).
    pub text_muted: Color32,
    /// Primary series color (charts).
    pub accent: Color32,
    /// Secondary series color (charts).
    pub accent_alt: Color32,
    /// Fill of the one main action per view ([`crate::UiExt::primary_button`]).
    pub primary: Color32,
    /// Status: healthy / online (`status.ok.solid`).
    pub good: Color32,
    /// Status: degraded / attention (`status.warning.solid`).
    pub warn: Color32,
    /// Status: failing / critical; danger buttons (`status.error.solid`).
    pub crit: Color32,
    /// Status: offline / disabled (`status.neutral.line`).
    pub off: Color32,
    /// The status groups in full: soft backgrounds, lines and highlight fills beside the solids above.
    pub status: Status,
    /// Categorical label colors (models, series, tags): violet, teal, blue, pink, lime. Never a state.
    pub cat: [Color32; 5],
    /// Brand red (wordmark, product name). Use sparingly; not a series or status color.
    pub red: Color32,
    /// Background of selected text and selected items (`selection.bg_fill`).
    pub selection: Color32,
    /// Fill of a hovered widget.
    pub hover: Color32,
    /// Keyboard focus, hovered widget outline, text cursor, links.
    pub focus: Color32,
    /// Identifier text: an agent session's slug (`sajiv`). Distinct from every series, status and model color.
    pub slug: Color32,
    /// Identifier text: a task name or id (`P2605#8oct-1139`). Distinct from `slug` and from every series color.
    pub task: Color32,
}

/// Spacing scale, points: 2, 4, 8, 12, 16, 24.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Space {
    pub xs: f32,
    pub s: f32,
    pub m: f32,
    pub l: f32,
    pub xl: f32,
    pub xxl: f32,
}

/// Corner radii, points: small (badges, check boxes), medium (buttons, menus; the brand 3 px), large
/// (windows, panels).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Radius {
    pub s: u8,
    pub m: u8,
    pub l: u8,
}

/// Stroke widths, points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Strokes {
    /// Hairlines and resting outlines.
    pub thin: f32,
    /// Hovered text and icons.
    pub medium: f32,
    /// Pressed widgets, focus ring, text cursor.
    pub thick: f32,
}

/// Shadows by elevation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Elevation {
    /// Menus, combo boxes, tooltips.
    pub popup: Shadow,
    /// Floating windows.
    pub window: Shadow,
}

/// The design system in one value. Start from [`Tokens::dark`] / [`Tokens::light`] and override fields.
#[derive(Clone, Debug, PartialEq)]
pub struct Tokens {
    pub colors: Palette,
    pub space: Space,
    pub radius: Radius,
    pub stroke: Strokes,
    pub shadow: Elevation,
    pub type_scale: TypeScale,
}

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

const SPACE: Space = Space {
    xs: 2.0,
    s: 4.0,
    m: 8.0,
    l: 12.0,
    xl: 16.0,
    xxl: 24.0,
};
const RADIUS: Radius = Radius { s: 2, m: 3, l: 6 };
const STROKES: Strokes = Strokes {
    thin: 1.0,
    medium: 1.5,
    thick: 2.0,
};

const fn shadow(offset_y: i8, blur: u8, alpha: u8) -> Shadow {
    Shadow {
        offset: [0, offset_y],
        blur,
        spread: 0,
        color: Color32::from_black_alpha(alpha),
    }
}

impl Tokens {
    /// The VHRD brand dark variant (the same colors ve_dash's dark theme has always used).
    pub const fn dark() -> Self {
        Self {
            colors: Palette {
                bg: vhrd::dark::BG,
                surface: vhrd::dark::SURFACE,
                surface_raised: rgb(0x212123), // between dark.surface and grey.800
                line: vhrd::dark::LINE,
                line_strong: vhrd::grey::S700,
                title: vhrd::grey::S400,
                text: vhrd::dark::TEXT,
                text_muted: vhrd::dark::MUTED,
                accent: rgb(0x118ea1),     // signal.can, chart-darkened
                accent_alt: rgb(0xaa74d4), // signal.analog, chart-darkened
                primary: vhrd::signal_dark::CAN,
                good: vhrd::status_dark::ok::SOLID,
                warn: vhrd::status_dark::warning::SOLID,
                crit: vhrd::status_dark::error::SOLID,
                off: vhrd::status_dark::neutral::LINE,
                status: status!(status_dark),
                cat: categorical!(categorical_dark),
                red: vhrd::dark::RED,
                selection: rgb(0x0d4a54), // signal.can, deep
                hover: rgb(0x2c2c2e),
                focus: vhrd::signal_dark::CAN,
                slug: rgb(0xa6e3c4), // pale mint
                task: rgb(0xe8c44e), // gold
            },
            space: SPACE,
            radius: RADIUS,
            stroke: STROKES,
            shadow: Elevation {
                popup: shadow(4, 12, 110),
                window: shadow(8, 24, 140),
            },
            type_scale: TypeScale::new(),
        }
    }

    /// The VHRD brand light set (`tokens.css` `:root`).
    pub const fn light() -> Self {
        Self {
            colors: Palette {
                bg: vhrd::brand::PAPER,
                surface: vhrd::grey::S100,
                surface_raised: vhrd::grey::S0,
                line: vhrd::grey::S300, // the brand line-strong, see module docs
                line_strong: vhrd::grey::S400,
                title: vhrd::grey::S700,
                text: vhrd::brand::BLACK,
                text_muted: rgb(0x666666), // brand.grey, a touch darker for AA
                accent: vhrd::signal::CAN,
                accent_alt: vhrd::signal::ANALOG,
                primary: vhrd::signal::CAN,
                good: vhrd::status::ok::SOLID,
                warn: vhrd::status::warning::SOLID,
                crit: vhrd::status::error::SOLID,
                off: vhrd::status::neutral::LINE,
                status: status!(status),
                cat: categorical!(categorical),
                red: vhrd::brand::RED,
                selection: rgb(0xc4e5ea), // signal.can, pale
                hover: vhrd::grey::S200,
                focus: vhrd::signal::CAN,
                slug: rgb(0x1f8a54), // deep mint
                task: rgb(0xa48200), // deep gold
            },
            space: SPACE,
            radius: RADIUS,
            stroke: STROKES,
            shadow: Elevation {
                popup: shadow(4, 12, 28),
                window: shadow(8, 24, 36),
            },
            type_scale: TypeScale::new(),
        }
    }

    /// The built-in set for `theme`.
    pub const fn for_theme(theme: Theme) -> Self {
        match theme {
            Theme::Dark => Self::dark(),
            Theme::Light => Self::light(),
        }
    }

    /// Text color for something filled with `fill`: whichever of `text`, `bg` and `surface_raised` reads best.
    pub fn on(&self, fill: Color32) -> Color32 {
        let c = &self.colors;
        [c.text, c.bg, c.surface_raised]
            .into_iter()
            .max_by(|a, b| contrast(*a, fill).total_cmp(&contrast(*b, fill)))
            .unwrap_or(c.text)
    }

    /// `color` as text on `bg`, nudged toward `text` until it is AA (status colors as text, links).
    pub fn legible(&self, color: Color32, bg: Color32) -> Color32 {
        ensure_contrast(color, bg, self.colors.text, AA_TEXT)
    }

    /// The tokens installed for `theme` by [`crate::setup`] / [`Tokens::apply`], or the built-in set.
    pub fn of(ctx: &egui::Context, theme: Theme) -> Arc<Tokens> {
        ctx.data(|d| d.get_temp::<Arc<Tokens>>(storage_id(theme)))
            .unwrap_or_else(|| Arc::new(Self::for_theme(theme)))
    }

    /// The tokens matching `ui`'s current visuals (dark or light).
    pub fn of_ui(ui: &egui::Ui) -> Arc<Tokens> {
        let theme = if ui.visuals().dark_mode {
            Theme::Dark
        } else {
            Theme::Light
        };
        Self::of(ui.ctx(), theme)
    }

    /// Install these tokens as the egui style for `theme` (`ctx.set_style_of`) and remember them for
    /// [`Tokens::of`]. egui shows the dark or light style following the system preference.
    pub fn apply(&self, ctx: &egui::Context, theme: Theme) {
        let mut style = Arc::unwrap_or_clone(ctx.style_of(theme));
        self.apply_to_style(&mut style, theme);
        ctx.set_style_of(theme, style);
        ctx.data_mut(|d| d.insert_temp(storage_id(theme), Arc::new(self.clone())));
    }

    /// Fill `style` from the tokens: text styles, spacing, and every visual (all widget states, windows,
    /// popups, tooltips, selection, links, separators, scroll bars). Keeps fields the tokens don't cover
    /// (interaction, animation, debug) as they are.
    pub fn apply_to_style(&self, style: &mut Style, theme: Theme) {
        self.type_scale.apply(&mut style.text_styles);
        self.apply_spacing(&mut style.spacing);
        style.visuals = self.visuals(theme);
    }

    fn apply_spacing(&self, sp: &mut egui::style::Spacing) {
        let s = &self.space;
        sp.item_spacing = egui::vec2(s.m, s.s);
        sp.button_padding = egui::vec2(s.m, s.xs + 1.0);
        sp.window_margin = Margin::same(s.l as i8);
        sp.menu_margin = Margin::same(s.s as i8 + 2);
        sp.indent = s.xl;
        sp.interact_size.y = self.type_scale.body.line_height + s.xs;
        sp.icon_spacing = s.s + 1.0;
        sp.tooltip_width = 420.0;
        sp.scroll.bar_width = s.m;
        sp.scroll.floating_width = s.xs;
        sp.scroll.bar_inner_margin = s.s;
        sp.scroll.bar_outer_margin = 0.0;
    }

    /// The egui visuals for `theme`.
    pub fn visuals(&self, theme: Theme) -> Visuals {
        let c = &self.colors;
        let st = &self.stroke;
        let dark = theme == Theme::Dark;
        let mut v = if dark {
            Visuals::dark()
        } else {
            Visuals::light()
        };
        let strong = if dark { Color32::WHITE } else { Color32::BLACK };
        let r_s = CornerRadius::same(self.radius.s);
        let r_m = CornerRadius::same(self.radius.m);
        let r_l = CornerRadius::same(self.radius.l);
        let pressed = c.hover.lerp_to_gamma(c.line_strong, 0.35);

        v.dark_mode = dark;
        v.override_text_color = None;
        v.weak_text_color = Some(c.text_muted);

        v.widgets.noninteractive = WidgetVisuals {
            bg_fill: c.surface,
            weak_bg_fill: c.surface,
            bg_stroke: Stroke::new(st.thin, c.line), // separators, frames
            corner_radius: r_s,
            fg_stroke: Stroke::new(st.thin, c.text), // labels
            expansion: 0.0,
        };
        v.widgets.inactive = WidgetVisuals {
            bg_fill: c.surface_raised,      // check box, radio, slider rail, scroll bar
            weak_bg_fill: c.surface_raised, // buttons
            bg_stroke: Stroke::new(st.thin, c.line_strong),
            corner_radius: r_m,
            fg_stroke: Stroke::new(st.thin, c.text),
            expansion: 0.0,
        };
        v.widgets.hovered = WidgetVisuals {
            bg_fill: c.hover,
            weak_bg_fill: c.hover,
            bg_stroke: Stroke::new(st.thin, c.focus),
            corner_radius: r_m,
            fg_stroke: Stroke::new(st.medium, strong),
            expansion: 1.0,
        };
        v.widgets.active = WidgetVisuals {
            bg_fill: pressed,
            weak_bg_fill: pressed,
            bg_stroke: Stroke::new(st.thin, c.focus),
            corner_radius: r_m,
            fg_stroke: Stroke::new(st.thick, strong),
            expansion: 1.0,
        };
        v.widgets.open = WidgetVisuals {
            bg_fill: c.hover,
            weak_bg_fill: c.hover,
            bg_stroke: Stroke::new(st.thin, c.line_strong),
            corner_radius: r_m,
            fg_stroke: Stroke::new(st.thin, c.text),
            expansion: 0.0,
        };

        v.selection.bg_fill = c.selection;
        // Also the text color of a selected item, so it has to read on the selection fill.
        v.selection.stroke = Stroke::new(st.thin, self.legible(c.focus, c.selection));

        v.hyperlink_color = self.legible(c.focus, c.bg);
        v.faint_bg_color = c.surface;
        v.extreme_bg_color = if dark {
            c.bg.lerp_to_gamma(Color32::BLACK, 0.4)
        } else {
            c.surface_raised
        };
        v.text_edit_bg_color = Some(v.extreme_bg_color);
        v.code_bg_color = c.surface;
        v.warn_fg_color = self.legible(c.warn, c.bg);
        v.error_fg_color = self.legible(c.crit, c.bg);

        v.window_corner_radius = r_l;
        v.window_shadow = self.shadow.window;
        v.window_fill = c.surface;
        v.window_stroke = Stroke::new(st.thin, c.line);
        v.menu_corner_radius = r_m;
        v.panel_fill = c.bg;
        v.popup_shadow = self.shadow.popup;

        v.text_cursor.stroke = Stroke::new(st.thick, c.focus);
        v.button_frame = true;
        v.collapsing_header_frame = false;
        v.indent_has_left_vline = true;
        v.striped = false;
        v.slider_trailing_fill = true;
        v
    }
}

impl Default for Tokens {
    fn default() -> Self {
        Self::dark()
    }
}

fn storage_id(theme: Theme) -> Id {
    Id::new(("ve_theme::tokens", theme))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hue;

    /// What vhrd_brand states for its status groups (PAL-5) holds for the values the export gave us (THM-11):
    /// solid reads on the surfaces and on its own soft and hover, line stands off the surfaces, text reads on
    /// the fills.
    #[test]
    fn status_groups_keep_their_contrast() {
        for (name, t) in [("dark", Tokens::dark()), ("light", Tokens::light())] {
            let c = t.colors;
            let st = c.status;
            for (group, r) in [
                ("error", st.error),
                ("warning", st.warning),
                ("ok", st.ok),
                ("info", st.info),
                ("neutral", st.neutral),
            ] {
                for bg in [c.bg, c.surface, r.soft, r.hover] {
                    let k = contrast(r.solid, bg);
                    assert!(k >= AA_TEXT, "{name} {group} solid on {bg:?}: {k:.2}");
                }
                for bg in [c.bg, c.surface] {
                    let k = contrast(r.line, bg);
                    assert!(k >= 3.0, "{name} {group} line on {bg:?}: {k:.2}");
                }
                for bg in [r.soft, r.hover] {
                    let k = contrast(c.text, bg);
                    assert!(k >= 9.5, "{name} {group} text on {bg:?}: {k:.2}");
                }
            }
        }
    }

    /// Error is not the brand red and warning is not the power orange: apart in hue and in OKLab distance,
    /// while both stay in the hue range reserved for alerts. Categorical colours stay out of it.
    #[test]
    fn error_and_warning_are_apart_from_brand_red_and_power_orange() {
        for (name, t, power) in [
            ("dark", Tokens::dark(), vhrd::signal_dark::POWER),
            ("light", Tokens::light(), vhrd::signal::POWER),
        ] {
            let c = t.colors;
            for (what, a, b) in [
                ("error vs brand red", c.status.error.solid, c.red),
                ("warning vs power", c.status.warning.solid, power),
            ] {
                let gap = hue::hue_gap(a, b).unwrap_or(0.0);
                assert!(gap >= 12.0, "{name} {what}: hue gap {gap:.0}");
                let d = hue::delta_e(a, b);
                assert!(d >= 0.05, "{name} {what}: OKLab distance {d:.3}");
                assert!(hue::is_alert_hue(a), "{name} {what}");
            }
            for cat in c.cat {
                assert!(!hue::is_alert_hue(cat), "{name} categorical {cat:?}");
            }
        }
    }

    /// The copy of the export is the export: when vhrd_brand is checked out beside this repo, the two files
    /// are the same (`just brand` refreshes the copy).
    #[test]
    fn palette_copy_matches_the_sibling_export() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let Ok(theirs) = std::fs::read_to_string(dir.join("../../vhrd_brand/web/palette.json"))
        else {
            return;
        };
        let ours = std::fs::read_to_string(dir.join("brand/palette.json")).unwrap_or_default();
        assert!(
            ours == theirs,
            "ve_theme/brand/palette.json is stale: run `just brand`"
        );
    }

    /// The identifier colors (slug, task) are legible on the backgrounds and differ from each other and from
    /// the series colors (APP-44: told apart at a glance in a session row).
    #[test]
    fn identifier_colors_are_legible_and_distinct() {
        let dist = |a: Color32, b: Color32| {
            let d = |x: u8, y: u8| (x as f32 - y as f32).powi(2);
            (d(a.r(), b.r()) + d(a.g(), b.g()) + d(a.b(), b.b())).sqrt()
        };
        for (name, t) in [("dark", Tokens::dark()), ("light", Tokens::light())] {
            let c = t.colors;
            for (n, fg) in [("slug", c.slug), ("task", c.task)] {
                for bg in [c.bg, c.surface] {
                    let r = contrast(fg, bg);
                    assert!(r >= 3.0, "{name}: {n} on a background is {r:.2}");
                }
                for (o, other) in [
                    ("accent", c.accent),
                    ("accent_alt", c.accent_alt),
                    ("text", c.text),
                ] {
                    assert!(dist(fg, other) > 40.0, "{name}: {n} too close to {o}");
                }
            }
            assert!(dist(c.slug, c.task) > 60.0, "{name}: slug vs task");
            // Red and orange are reserved for error and warning (THM-10).
            for (n, fg) in [
                ("slug", c.slug),
                ("task", c.task),
                ("accent", c.accent),
                ("accent_alt", c.accent_alt),
            ] {
                assert!(
                    !crate::hue::is_alert_hue(fg),
                    "{name}: {n} is in the error / warning hue range"
                );
            }
            assert!(
                crate::hue::delta_e(c.slug, c.task) > 0.1,
                "{name}: slug vs task (OKLab)"
            );
        }
    }

    /// THM-2: text and muted text are AA on every background they sit on, in both sets.
    #[test]
    fn text_is_aa_on_every_background() {
        for (name, t) in [("dark", Tokens::dark()), ("light", Tokens::light())] {
            let c = t.colors;
            for (bg_name, bg) in [
                ("bg", c.bg),
                ("surface", c.surface),
                ("surface_raised", c.surface_raised),
                ("hover", c.hover),
            ] {
                for (fg_name, fg) in [("text", c.text), ("text_muted", c.text_muted)] {
                    let r = contrast(fg, bg);
                    assert!(r >= AA_TEXT, "{name}: {fg_name} on {bg_name} is {r:.2}");
                }
            }
            assert!(
                contrast(c.text, c.selection) >= AA_TEXT,
                "{name}: selection"
            );
            for fill in [c.primary, c.crit] {
                assert!(contrast(t.on(fill), fill) >= AA_TEXT, "{name}: on {fill:?}");
            }
        }
    }

    #[test]
    fn derived_text_colors_are_aa() {
        for theme in [Theme::Dark, Theme::Light] {
            let t = Tokens::for_theme(theme);
            let v = t.visuals(theme);
            for (what, fg, bg) in [
                ("warn", v.warn_fg_color, t.colors.bg),
                ("error", v.error_fg_color, t.colors.bg),
                ("link", v.hyperlink_color, t.colors.bg),
                ("selected", v.selection.stroke.color, v.selection.bg_fill),
            ] {
                let r = contrast(fg, bg);
                assert!(r >= AA_TEXT, "{theme:?}: {what} is {r:.2}");
            }
        }
    }

    #[test]
    fn apply_installs_both_styles_and_remembers_tokens() {
        let ctx = egui::Context::default();
        let mut light = Tokens::light();
        light.colors.focus = Color32::from_rgb(1, 2, 3);
        Tokens::dark().apply(&ctx, Theme::Dark);
        light.apply(&ctx, Theme::Light);
        assert_eq!(
            ctx.style_of(Theme::Dark).visuals.panel_fill,
            Tokens::dark().colors.bg
        );
        assert_eq!(
            ctx.style_of(Theme::Light).visuals.panel_fill,
            light.colors.bg
        );
        assert!(!ctx.style_of(Theme::Light).visuals.dark_mode);
        assert_eq!(
            Tokens::of(&ctx, Theme::Light).colors.focus,
            light.colors.focus
        );
    }
}
