//! Icons: the Phosphor regular set as a font, and SVG icons with colour slots bound to tokens (CHR-7).
//!
//! - **Phosphor** ([`ph`], feature `phosphor`): the base set, a font installed by [`crate::setup`] /
//!   [`crate::install_fonts`], tinted like text. `Icon::from(ph::GEAR_SIX)`, or the glyph inside a label:
//!   `format!("{} Save", ph::FLOPPY_DISK)`.
//! - **SVG icons** (`SvgIcon`, the set in `own`; feature `icons`, which needs resvg): shapes name a colour
//!   slot instead of a colour, bound to tokens when drawn, so an icon follows the theme and the desktop
//!   accent. An app adds its own: `static PROBE: SvgIcon = SvgIcon::new("probe", include_str!("probe.svg"));`.
//!
//! # The slot convention
//!
//! - The view box is square; the icons here use 24 units with 1.5 unit strokes.
//! - A shape's colour is one class: `f-NAME` fills it, `s-NAME` strokes it; NAME is a [`Slot`] name (ink,
//!   muted, accent, good, warn, crit). A class on a group applies to the shapes inside it. Other attributes
//!   (stroke width, caps, joins, dashes, transforms) are read as usual.
//! - In an icon with any slot class, every shape takes its paint from one; an unclassed shape is an error.
//!   An SVG without slot classes is drawn whole in ink, whatever its colours.
//! - Shapes paint in document order: consecutive shapes of one slot share a mask, and a slot that comes back
//!   after another gets a new one, so the z-order holds.
//!   An icon has at most `raster::LAYERS_MAX` such masks, and one raster at most `raster::RASTER_BYTES_MAX`
//!   of texture.
//! - Not drawn, each an error at load: text, images, `<style>`, gradients and patterns, opacity below 1, clip
//!   paths, masks and filters.
//!
//! An SVG icon is rasterised once per (icon, size in pixels): strokes become filled outlines, each layer a
//! coverage mask uploaded as a texture and cached in the egui context; painting tints each mask with its
//! slot's colour, so a theme switch re-tints without rasterising again (`raster_count`). The mask is drawn
//! texel for pixel at a pixel-aligned corner, so it is as sharp at 1.25x as at 1x.
//!
//! ```ignore
//! ui.add(IconImage::new(&own::CAN_FRAME)).on_hover_text("CAN frame");
//! ui.add(IconImage::new(ph::GEAR_SIX).size(20.0).ink(tokens.colors.text_muted));
//! ```
//!
//! The Phosphor font is MIT licensed, © Phosphor Icons (`ve_theme/fonts/LICENSE-Phosphor.txt`).

#[cfg(feature = "icons")]
mod cache;
#[cfg(feature = "icons")]
pub mod gallery;
#[cfg(feature = "icons")]
pub mod raster;

use egui::emath::GuiRounding as _;
use egui::{
    Color32, FontFamily, FontId, Painter, Rect, Response, Sense, Ui, Widget, WidgetInfo,
    WidgetType, vec2,
};

use crate::Tokens;

#[cfg(feature = "icons")]
pub use cache::{CACHE_BYTES_MAX, CACHE_IDLE_PASSES_MAX, cache_bytes, raster_count};
/// The Phosphor regular glyphs: `ph::GEAR_SIX`, `ph::ICONS` (every name with its glyph).
pub use egui_phosphor::regular as ph;
#[cfg(feature = "icons")]
pub use raster::{
    ICON_PX_MAX, IconError, LAYERS_MAX, RASTER_BYTES_MAX, Shapes, SlotMask, rasterise,
};

/// Name of the font family that holds only the Phosphor icons.
pub const ICON_FAMILY: &str = "ve-icons";

/// The Phosphor regular font, TTF bytes.
pub const PHOSPHOR: &[u8] = egui_phosphor::bytes::regular::FONT;

/// The largest icon size in points.
pub const ICON_SIZE_MAX: f32 = 256.0;

/// A colour slot of an SVG icon, bound to a token when drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Slot {
    /// The icon's main lines: `colors.text`.
    Ink,
    /// Secondary detail: `colors.text_muted`.
    Muted,
    /// The one accent: `colors.focus`, the desktop's accent when the colours follow it.
    Accent,
    /// Healthy: `colors.good`.
    Good,
    /// Attention: `colors.warn`.
    Warn,
    /// Failing: `colors.crit`.
    Crit,
}

impl Slot {
    /// Every slot.
    pub const ALL: [Self; 6] = [
        Self::Ink,
        Self::Muted,
        Self::Accent,
        Self::Good,
        Self::Warn,
        Self::Crit,
    ];

    /// The name used in the SVG classes (`f-NAME`, `s-NAME`).
    pub const fn name(self) -> &'static str {
        match self {
            Self::Ink => "ink",
            Self::Muted => "muted",
            Self::Accent => "accent",
            Self::Good => "good",
            Self::Warn => "warn",
            Self::Crit => "crit",
        }
    }

    /// The slot named `name`, if any.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.name() == name)
    }

    /// The token colour the slot is drawn in.
    pub fn color(self, tokens: &Tokens) -> Color32 {
        let c = &tokens.colors;
        match self {
            Self::Ink => c.text,
            Self::Muted => c.text_muted,
            Self::Accent => c.focus,
            Self::Good => c.good,
            Self::Warn => c.warn,
            Self::Crit => c.crit,
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

/// An SVG icon on a square view box, its shapes' colours given as [`Slot`] classes.
#[cfg(feature = "icons")]
#[derive(Debug, PartialEq, Eq)]
pub struct SvgIcon {
    /// A short kebab-case name: tooltips, accessibility, texture names.
    pub name: &'static str,
    /// The SVG text.
    pub svg: &'static str,
}

#[cfg(feature = "icons")]
impl SvgIcon {
    /// An icon from its name and SVG text (`include_str!`).
    pub const fn new(name: &'static str, svg: &'static str) -> Self {
        Self { name, svg }
    }
}

/// The SVG icons of this crate, in `ve_theme/icons/`.
#[cfg(feature = "icons")]
pub mod own {
    use super::SvgIcon;

    /// A CAN frame: the frame, its id field in the accent, the data bits.
    pub static CAN_FRAME: SvgIcon =
        SvgIcon::new("can-frame", include_str!("../../icons/can-frame.svg"));
    /// A debug probe with its cable.
    pub static PROBE: SvgIcon = SvgIcon::new("probe", include_str!("../../icons/probe.svg"));
    /// A USB dongle with its status light in the accent.
    pub static DONGLE: SvgIcon = SvgIcon::new("dongle", include_str!("../../icons/dongle.svg"));
    /// A tape (a recording) with the play mark in the accent.
    pub static TAPE: SvgIcon = SvgIcon::new("tape", include_str!("../../icons/tape.svg"));

    /// Every icon of this module.
    pub static ALL: [&SvgIcon; 4] = [&CAN_FRAME, &PROBE, &DONGLE, &TAPE];
}

/// An icon: a Phosphor glyph or (feature `icons`) an SVG icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    /// A glyph of the icon font ([`ph`]), drawn in the ink colour.
    Glyph(&'static str),
    /// An SVG icon with colour slots.
    #[cfg(feature = "icons")]
    Svg(&'static SvgIcon),
}

impl From<&'static str> for Icon {
    fn from(glyph: &'static str) -> Self {
        Self::Glyph(glyph)
    }
}

#[cfg(feature = "icons")]
impl From<&'static SvgIcon> for Icon {
    fn from(icon: &'static SvgIcon) -> Self {
        Self::Svg(icon)
    }
}

impl Icon {
    /// The icon's name: the SVG icon's, or the Phosphor name in kebab case (`gear-six`); `icon` for a glyph
    /// that is not in the Phosphor set.
    pub fn name(self) -> String {
        match self {
            #[cfg(feature = "icons")]
            Self::Svg(s) => s.name.to_owned(),
            Self::Glyph(g) => ph::ICONS.iter().find(|(_, glyph)| *glyph == g).map_or_else(
                || "icon".to_owned(),
                |(n, _)| n.to_lowercase().replace('_', "-"),
            ),
        }
    }
}

/// An icon at a size, its slots in token colours unless set: `ui.add(IconImage::new(ph::GEAR_SIX))`.
#[derive(Clone, Copy, Debug)]
#[must_use = "pass to ui.add() or call .paint()"]
pub struct IconImage {
    icon: Icon,
    size: Option<f32>,
    colors: [Option<Color32>; 6],
}

impl IconImage {
    /// The icon at the chrome's icon size ([`crate::Tokens::chrome`] `icon`), in its slots' token colours.
    pub fn new(icon: impl Into<Icon>) -> Self {
        Self {
            icon: icon.into(),
            size: None,
            colors: [None; 6],
        }
    }

    /// Side in points.
    ///
    /// # Panics
    ///
    /// When `points` is not above 0 or is more than [`ICON_SIZE_MAX`].
    pub fn size(mut self, points: f32) -> Self {
        assert!(points > 0.0, "an icon needs a positive size");
        assert!(
            points <= ICON_SIZE_MAX,
            "an icon is at most {ICON_SIZE_MAX} points"
        );
        self.size = Some(points);
        self
    }

    /// Draw `slot` in `color` instead of its token (a glyph is all [`Slot::Ink`]).
    pub fn color(mut self, slot: Slot, color: Color32) -> Self {
        self.colors[slot.index()] = Some(color);
        self
    }

    /// Draw the ink slot (a glyph's only colour) in `color`: muted toolbars, text on a primary fill.
    pub fn ink(self, color: Color32) -> Self {
        self.color(Slot::Ink, color)
    }

    /// The side in points with `tokens`.
    pub fn side(&self, tokens: &Tokens) -> f32 {
        self.size.unwrap_or(tokens.chrome.icon)
    }

    fn slot_color(&self, slot: Slot, tokens: &Tokens) -> Color32 {
        self.colors[slot.index()].unwrap_or_else(|| slot.color(tokens))
    }

    /// Paint the icon centred in `rect`, its corner on the pixel grid.
    pub fn paint(&self, painter: &Painter, tokens: &Tokens, rect: Rect) {
        let ppp = painter.pixels_per_point();
        let side = self.side(tokens);
        let px = (side * ppp).round().max(1.0);
        let box_side = px / ppp;
        let min = (rect.center() - vec2(box_side, box_side) / 2.0).round_to_pixels(ppp);
        let rect = Rect::from_min_size(min, vec2(box_side, box_side));
        match self.icon {
            Icon::Glyph(g) => self.paint_glyph(painter, tokens, g, rect),
            #[cfg(feature = "icons")]
            Icon::Svg(svg) => {
                let masks = cache::masks(painter.ctx(), svg, px as u32);
                match masks.as_ref() {
                    Ok(textures) => {
                        let uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                        for (slot, tex) in textures {
                            painter.image(tex.id(), rect, uv, self.slot_color(*slot, tokens));
                        }
                    }
                    // A broken icon shows as a box in `crit`, never as nothing.
                    Err(_) => {
                        painter.rect_stroke(
                            rect.shrink(0.5 / ppp),
                            0.0,
                            egui::Stroke::new(1.0 / ppp, tokens.colors.crit),
                            egui::StrokeKind::Inside,
                        );
                    }
                }
            }
        }
    }

    fn paint_glyph(&self, painter: &Painter, tokens: &Tokens, glyph: &str, rect: Rect) {
        // The icon family is bound from the frame after `install_fonts`; until then the proportional stack.
        let family = FontFamily::Name(ICON_FAMILY.into());
        let bound = painter
            .ctx()
            .fonts(|f| f.definitions().families.contains_key(&family));
        let family = if bound {
            family
        } else {
            FontFamily::Proportional
        };
        let font = FontId::new(rect.height(), family);
        let galley =
            painter.layout_no_wrap(glyph.to_owned(), font, self.slot_color(Slot::Ink, tokens));
        // Centred on the glyph's own ink, not its line box (which carries the font's ascent and descent).
        let pos = (rect.center() - galley.mesh_bounds.center().to_vec2())
            .round_to_pixels(painter.pixels_per_point());
        painter.galley(pos, galley, Color32::PLACEHOLDER);
    }
}

impl Widget for IconImage {
    fn ui(self, ui: &mut Ui) -> Response {
        let tokens = Tokens::of_ui(ui);
        let side = self.side(&tokens);
        let (rect, response) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
        if ui.is_rect_visible(rect) {
            self.paint(ui.painter(), &tokens, rect);
        }
        let icon = self.icon;
        let enabled = ui.is_enabled();
        response.widget_info(|| WidgetInfo::labeled(WidgetType::Image, enabled, icon.name()));
        response
    }
}
