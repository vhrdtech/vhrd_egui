//! Brand fonts: IBM Plex Sans and IBM Plex Mono, the vhrd_brand web faces (DASH-9).
//!
//! Embedded so dashboards look the same everywhere without system fonts, and match
//! the datasheets and web pages. [`install_fonts`] puts Plex Sans first in the
//! proportional family and Plex Mono first in the monospace family; egui's built-in
//! fonts stay as fallback for glyphs Plex lacks (emoji, box drawing).
//!
//! The faces are © IBM Corp., SIL Open Font License 1.1 (`ve_dash/fonts/LICENSE.txt`);
//! OFL permits embedding, the fonts themselves remain under the OFL.

use egui::FontFamily;
use egui::epaint::text::{FontData, FontInsert, FontPriority, InsertFontFamily};

/// IBM Plex Sans Regular, TTF bytes.
pub const PLEX_SANS: &[u8] = include_bytes!("../fonts/IBMPlexSans-Regular.ttf");
/// IBM Plex Mono Regular, TTF bytes.
pub const PLEX_MONO: &[u8] = include_bytes!("../fonts/IBMPlexMono-Regular.ttf");

/// Install the brand fonts as the primary proportional and monospace faces.
///
/// Call once at startup, before or after [`crate::Theme::apply`]; calling it again
/// is harmless (egui replaces fonts by name).
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
}
