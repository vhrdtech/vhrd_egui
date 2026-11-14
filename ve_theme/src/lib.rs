//! The VHRD design system for egui: one call gives an app the brand look in dark and light.
//!
//! ```ignore
//! // in eframe's app creator:
//! ve_theme::setup(&cc.egui_ctx);
//! ```
//!
//! [`setup`] installs the brand fonts (feature `fonts`) and a dark and a light [`egui::Style`] built from
//! [`Tokens`] (`ctx.set_style_of(Theme::Dark | Theme::Light, ..)`), so egui follows the system preference
//! and every plain egui widget already looks right. [`setup_with`] takes your own tokens.
//!
//! - [`Tokens`]: named colors ([`Palette`]), spacing, radii, strokes, shadows, type scale; [`Tokens::apply`]
//!   fills a whole egui style from them (THM-1). [`Tokens::dark`] / [`Tokens::light`] are the vhrd_brand sets
//!   (THM-2); [`Tokens::of_ui`] gives the tokens matching a `Ui`'s current theme.
//! - [`typography`]: the type scale on egui's text styles plus `title` and `caption` (THM-3).
//! - [`UiExt`]: what `Style` cannot express — primary / danger buttons, section header, panel title bar,
//!   toolbar, muted label, badge — each with its tooltip (THM-4).
//! - [`ColorSource`]: where the colours come from, a setting: the desktop theme (omarchy's `colors.toml`,
//!   [`DesktopColors`], [`Tokens::with_desktop`]), the brand, or a fixed accent ([`Tokens::with_accent`]);
//!   every text colour stays legible ([`Tokens::contrast_problems`]). [`Look::setup`] (native) installs it in
//!   place of [`setup`] and follows a desktop theme switch live (THM-18).
//! - [`hue`]: OKLab distance and the red / orange hue range reserved for error and warning, to test that
//!   categorical colours (labels, chips, series, hosts, models) stay out of it and apart from each other (THM-10).
//! - [`props`]: property-panel widgets: pane, sub-pane, group, property row, number and axis fields,
//!   segmented choice, toggle chip, list view (THM-17).
//! - [`topbar`]: `Brand`, the strip every app's top bar starts with — icon, wordmark, the version badge
//!   that opens `ve_basics::AppMenu` (THM-19).
//! - [`gallery`]: every egui widget and helper in one view, the tuning tool (THM-5);
//!   `cargo run -p ve_theme --example gallery`.
//!
//! Labels stay selectable unless the app also calls `ve_basics::setup_labels` (after `setup`).
//!
//! # Sources and licences
//!
//! Checked 8 Oct 2026. Ideas only were taken from these; no code or assets were copied:
//! - Rerun's `re_ui` (MIT OR Apache-2.0): tokens kept apart from widgets and applied per theme with
//!   `set_style_of`, a `UiExt` trait for styles `Style` can't hold, the panel title bar. Its fonts (Inter,
//!   Hack) are not used.
//! - catppuccin-egui (MIT): filling every `WidgetVisuals` state from one palette.
//! - egui_aesthetix (MIT): a theme as a complete `Style`, spacing included, not only colors.
//!
//! Colors come from vhrd_brand (`web/palette.json`, `tokens.css`). The bundled fonts are IBM Plex Sans and
//! IBM Plex Mono, © IBM Corp., SIL Open Font License 1.1 (`fonts/LICENSE.txt`), which permits embedding.

pub mod contrast;
mod desktop;
#[cfg(feature = "fonts")]
mod fonts;
pub mod gallery;
pub mod hue;
#[cfg(not(target_arch = "wasm32"))]
mod look;
pub mod props;
mod status;
mod tokens;
#[cfg(feature = "fonts")]
pub mod topbar;
pub mod typography;
mod ui_ext;

pub use desktop::{ColorSource, DesktopColors, parse_hex, to_hex};
#[cfg(feature = "fonts")]
pub use fonts::{
    BRAND_FAMILY, NUMBER_FAMILY, PLEX_MONO, PLEX_MONO_BOLD, PLEX_SANS, PLEX_SANS_BOLD, brand_font,
    install_fonts, number_font, number_text,
};
#[cfg(not(target_arch = "wasm32"))]
pub use look::{Look, omarchy_colors_path};
pub use status::{STATUS_GLYPHS, StatusGlyph, status_glyph, status_icon, status_icon_with};
pub use tokens::{Elevation, Palette, Radius, Ramp, Space, Status, Strokes, Tokens, vhrd};
pub use ui_ext::{Tone, UiExt};

/// Turn the VHRD theme on: brand fonts, dark and light styles from [`Tokens::dark`] / [`Tokens::light`].
/// Call once at startup; egui picks dark or light from the system and falls back to dark.
pub fn setup(ctx: &egui::Context) {
    setup_with(ctx, Tokens::dark(), Tokens::light());
}

/// [`setup`] with your own tokens, e.g. `Tokens { colors: Palette { primary: .., ..Tokens::dark().colors },
/// ..Tokens::dark() }`.
pub fn setup_with(ctx: &egui::Context, dark: Tokens, light: Tokens) {
    #[cfg(feature = "fonts")]
    install_fonts(ctx);
    ctx.options_mut(|o| o.fallback_theme = egui::Theme::Dark);
    dark.apply(ctx, egui::Theme::Dark);
    light.apply(ctx, egui::Theme::Light);
}
