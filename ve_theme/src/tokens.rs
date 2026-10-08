//! Design tokens (THM-1) and the brand dark / light sets (THM-2).
//!
//! [`Tokens`] is the whole design system in one value: named colors ([`Palette`]), a spacing scale
//! ([`Space`]), corner radii ([`Radius`]), stroke widths ([`Strokes`]), shadows ([`Elevation`]) and the
//! type scale ([`TypeScale`]). [`Tokens::apply`] turns it into an egui [`Style`] for one theme, so plain egui
//! widgets already look right; the [`crate::UiExt`] helpers read the same tokens for what `Style` cannot say.
//!
//! Colors come from vhrd_brand `web/palette.json` / `tokens.css`. Deviations, each for legibility in a dense
//! tool UI rather than a web page:
//! - `surface_raised` in dark is a step above `surface` (the brand has them equal) so menus and buttons
//!   stand off the panels.
//! - `line` in light is the brand `line-strong` (#d0d0d0): the brand hairline #e6e6e6 vanishes on `surface`.
//! - `text_muted` in light is #666666 (brand #6e6e6e) so it stays AA on `surface` too.
//! - Interaction (`primary`, `focus`, `selection`) uses the brand CAN teal, not the brand red: red is
//!   `crit` / danger here, and a red "OK" button next to a red "Delete" one would say nothing.
//!
//! Status colors (`good`, `warn`, `crit`, `off`) mark state and are always paired with a text label
//! ([`crate::UiExt::badge`] carries one by construction); never the only carrier of meaning.

use egui::epaint::Shadow;
use egui::{
    Color32, CornerRadius, Id, Margin, Stroke, Style, Theme, Visuals, style::WidgetVisuals,
};
use std::sync::Arc;

use crate::contrast::{AA_TEXT, contrast, ensure_contrast};
use crate::typography::TypeScale;

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
    /// Status: healthy / online.
    pub good: Color32,
    /// Status: degraded / attention.
    pub warn: Color32,
    /// Status: failing / critical; danger buttons.
    pub crit: Color32,
    /// Status: offline / disabled.
    pub off: Color32,
    /// Brand red (wordmark, product name). Use sparingly; not a series or status color.
    pub red: Color32,
    /// Background of selected text and selected items (`selection.bg_fill`).
    pub selection: Color32,
    /// Fill of a hovered widget.
    pub hover: Color32,
    /// Keyboard focus, hovered widget outline, text cursor, links.
    pub focus: Color32,
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
                bg: rgb(0x0f0f10),             // dark.bg
                surface: rgb(0x161617),        // dark.surface
                surface_raised: rgb(0x212123), // between dark.surface and grey.800
                line: rgb(0x2a2a2b),           // dark.line
                line_strong: rgb(0x4a4a4a),    // --line-strong (grey.700)
                title: rgb(0xa8a8a8),          // grey.400
                text: rgb(0xececec),           // dark.text
                text_muted: rgb(0x9a9a9a),     // dark.muted
                accent: rgb(0x118ea1),         // signal.can, chart-darkened
                accent_alt: rgb(0xaa74d4),     // signal.analog, chart-darkened
                primary: rgb(0x26c6da),        // signal-dark.can
                good: rgb(0x5cb860),           // signal-dark.bus
                warn: rgb(0xff9a3c),           // signal-dark.power
                crit: rgb(0xf24c44),           // dark.red
                off: rgb(0x6e6e6e),            // brand.grey
                red: rgb(0xf24c44),            // dark.red
                selection: rgb(0x0d4a54),      // signal.can, deep
                hover: rgb(0x2c2c2e),
                focus: rgb(0x26c6da), // signal-dark.can
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
                bg: rgb(0xf6f6f6),             // --bg (paper)
                surface: rgb(0xefefef),        // --surface (grey.100)
                surface_raised: rgb(0xffffff), // --raised
                line: rgb(0xd0d0d0),           // --line-strong, see module docs
                line_strong: rgb(0xa8a8a8),    // grey.400
                title: rgb(0x4a4a4a),          // grey.700
                text: rgb(0x1a1a1a),           // --text (brand.black)
                text_muted: rgb(0x666666),     // --muted, a touch darker for AA
                accent: rgb(0x00838f),         // signal.can
                accent_alt: rgb(0x6a1b9a),     // signal.analog
                primary: rgb(0x00838f),        // signal.can
                good: rgb(0x2e7d32),           // signal.bus
                warn: rgb(0xe8710a),           // signal.power
                crit: rgb(0xc8141c),           // brand.red
                off: rgb(0x8a8a8a),            // grey.500
                red: rgb(0xc8141c),            // brand.red
                selection: rgb(0xc4e5ea),      // signal.can, pale
                hover: rgb(0xe6e6e6),          // grey.200
                focus: rgb(0x00838f),          // signal.can
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
