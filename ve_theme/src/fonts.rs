//! Brand fonts: IBM Plex Sans and IBM Plex Mono, the vhrd_brand web faces (THM-3).
//!
//! Embedded so apps look the same everywhere without system fonts, and match the datasheets and web pages.
//! [`install_fonts`] puts Plex Sans first in the proportional family and Plex Mono first in the monospace
//! family; egui's built-in fonts stay as fallback for glyphs Plex lacks (emoji, box drawing).
//!
//! The faces are © IBM Corp., SIL Open Font License 1.1 (`ve_theme/fonts/LICENSE.txt`); the OFL permits
//! embedding, the fonts themselves remain under the OFL.

use egui::epaint::text::{FontData, FontInsert, FontPriority, InsertFontFamily};
use egui::{FontFamily, FontId, RichText};

/// IBM Plex Sans Regular, TTF bytes.
pub const PLEX_SANS: &[u8] = include_bytes!("../fonts/IBMPlexSans-Regular.ttf");
/// IBM Plex Mono Regular, TTF bytes.
pub const PLEX_MONO: &[u8] = include_bytes!("../fonts/IBMPlexMono-Regular.ttf");

/// IBM Plex Mono Bold, TTF bytes: the face of [`number_font`].
pub const PLEX_MONO_BOLD: &[u8] = include_bytes!("../fonts/IBMPlexMono-Bold.ttf");

/// IBM Plex Sans Bold, TTF bytes: the face of [`brand_font`].
pub const PLEX_SANS_BOLD: &[u8] = include_bytes!("../fonts/IBMPlexSans-Bold.ttf");

/// Name of the bold sans family [`install_fonts`] registers for wordmarks and titles.
pub const BRAND_FAMILY: &str = "ve-brand";

/// The wordmark font: IBM Plex Sans Bold at `size` (take the size from `Tokens::type_scale.brand`). Falls back
/// to the proportional face until [`install_fonts`] has taken effect (egui applies it at the start of the next
/// frame).
pub fn brand_font(ctx: &egui::Context, size: f32) -> FontId {
    let family = FontFamily::Name(BRAND_FAMILY.into());
    let bound = ctx.fonts(|f| f.definitions().families.contains_key(&family));
    FontId::new(
        size,
        if bound {
            family
        } else {
            FontFamily::Proportional
        },
    )
}

/// Name of the bold monospace family [`install_fonts`] registers for numbers.
pub const NUMBER_FAMILY: &str = "ve-number";

/// The font for live numbers: IBM Plex Mono Bold. Every digit has the same advance (tabular figures), so a
/// value ticking from `199` to `200` does not move its neighbours; bold makes the number the first thing
/// the eye finds. Falls back to the plain monospace face (still tabular) until [`install_fonts`] (or
/// [`crate::setup`]) has taken effect, which egui does at the start of the next frame.
pub fn number_font(ctx: &egui::Context, size: f32) -> FontId {
    let family = FontFamily::Name(NUMBER_FAMILY.into());
    let bound = ctx.fonts(|f| f.definitions().families.contains_key(&family));
    FontId::new(size, if bound { family } else { FontFamily::Monospace })
}

/// `text` in [`number_font`] at `size`; colour it with `.color(..)`.
///
/// ```ignore
/// ui.label(number_text(ui.ctx(), "1 234", 13.0).color(tokens.colors.text));
/// ```
pub fn number_text(ctx: &egui::Context, text: impl Into<String>, size: f32) -> RichText {
    RichText::new(text).font(number_font(ctx, size))
}

/// Install the brand fonts as the primary proportional and monospace faces.
///
/// [`crate::setup`] calls this; call it directly only when not using `setup`. Calling it again is harmless
/// (egui replaces fonts by name).
pub fn install_fonts(ctx: &egui::Context) {
    ctx.add_font(FontInsert::new(
        "IBM Plex Sans",
        FontData::from_static(PLEX_SANS),
        vec![InsertFontFamily {
            family: FontFamily::Proportional,
            priority: FontPriority::Highest,
        }],
    ));
    ctx.add_font(FontInsert::new(
        "IBM Plex Mono",
        FontData::from_static(PLEX_MONO),
        vec![InsertFontFamily {
            family: FontFamily::Monospace,
            priority: FontPriority::Highest,
        }],
    ));
    ctx.add_font(FontInsert::new(
        "IBM Plex Sans Bold",
        FontData::from_static(PLEX_SANS_BOLD),
        vec![InsertFontFamily {
            family: FontFamily::Name(BRAND_FAMILY.into()),
            priority: FontPriority::Highest,
        }],
    ));
    ctx.add_font(FontInsert::new(
        "IBM Plex Sans",
        FontData::from_static(PLEX_SANS),
        vec![InsertFontFamily {
            family: FontFamily::Name(BRAND_FAMILY.into()),
            priority: FontPriority::Lowest,
        }],
    ));
    // The number family: Plex Mono Bold first, then the regular monospace stack for glyphs Bold lacks.
    let number = || FontFamily::Name(NUMBER_FAMILY.into());
    ctx.add_font(FontInsert::new(
        "IBM Plex Mono Bold",
        FontData::from_static(PLEX_MONO_BOLD),
        vec![InsertFontFamily {
            family: number(),
            priority: FontPriority::Highest,
        }],
    ));
    ctx.add_font(FontInsert::new(
        "IBM Plex Mono",
        FontData::from_static(PLEX_MONO),
        vec![InsertFontFamily {
            family: number(),
            priority: FontPriority::Lowest,
        }],
    ));
}
