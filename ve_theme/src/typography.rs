//! Type scale (THM-3): sizes and line heights mapped onto egui's [`TextStyle`]s.
//!
//! egui knows a text style only by its font size; the line height is carried here and used through
//! [`TypeStep::format`] and the [`crate::UiExt`] helpers. Two named styles extend egui's five:
//! [`TITLE`] (panel and section titles) and [`CAPTION`] (small explanatory text, units, hints).

use egui::text::TextFormat;
use egui::{Color32, FontFamily, FontId, TextStyle};
use std::collections::BTreeMap;

/// Name of the panel / section title style: `TextStyle::Name("title")`, see [`title`].
pub const TITLE: &str = "title";
/// Name of the caption style: `TextStyle::Name("caption")`, see [`caption`].
pub const CAPTION: &str = "caption";

/// `TextStyle::Name("title")`.
pub fn title() -> TextStyle {
    TextStyle::Name(TITLE.into())
}

/// `TextStyle::Name("caption")`.
pub fn caption() -> TextStyle {
    TextStyle::Name(CAPTION.into())
}

/// Font family of a [`TypeStep`]. (egui's `FontFamily` is not `Copy` and not const-constructible for named
/// families; the scale only ever uses the two built-in ones.)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Face {
    /// IBM Plex Sans with `fonts`, egui's default proportional font without.
    Sans,
    /// IBM Plex Mono with `fonts`, egui's default monospace font without.
    Mono,
}

impl Face {
    /// The egui family.
    pub fn family(self) -> FontFamily {
        match self {
            Self::Sans => FontFamily::Proportional,
            Self::Mono => FontFamily::Monospace,
        }
    }
}

/// One step of the type scale: size and line height in points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TypeStep {
    /// Font size, points.
    pub size: f32,
    /// Line height (baseline to baseline), points.
    pub line_height: f32,
    /// Sans or mono.
    pub face: Face,
}

impl TypeStep {
    /// A proportional step.
    pub const fn sans(size: f32, line_height: f32) -> Self {
        Self {
            size,
            line_height,
            face: Face::Sans,
        }
    }

    /// A monospace step.
    pub const fn mono(size: f32, line_height: f32) -> Self {
        Self {
            size,
            line_height,
            face: Face::Mono,
        }
    }

    /// The egui font.
    pub fn font_id(&self) -> FontId {
        FontId::new(self.size, self.face.family())
    }

    /// A text format with this step's font, line height and `color`, for `LayoutJob`s.
    pub fn format(&self, color: Color32) -> TextFormat {
        TextFormat {
            font_id: self.font_id(),
            line_height: Some(self.line_height),
            color,
            ..Default::default()
        }
    }
}

/// The type scale. Body is the reference size; everything else is relative to it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TypeScale {
    /// `TextStyle::Small`: dense secondary text.
    pub small: TypeStep,
    /// `TextStyle::Body`: labels and running text.
    pub body: TypeStep,
    /// `TextStyle::Button`: text on buttons, check boxes, combo boxes.
    pub button: TypeStep,
    /// `TextStyle::Monospace`: numbers, ids, code.
    pub monospace: TypeStep,
    /// `TextStyle::Heading`: page / window headings.
    pub heading: TypeStep,
    /// `TextStyle::Name("title")`: panel and section titles.
    pub title: TypeStep,
    /// `TextStyle::Name("caption")`: captions, units, hints.
    pub caption: TypeStep,
    /// The wordmark of an app's bar (`brand_font`, bold Plex Sans): its size is the token; the face is not a
    /// text style because it needs the bold family.
    pub brand: TypeStep,
}

impl TypeScale {
    /// The VHRD scale for IBM Plex: body 13 / 18, a bit larger than egui's 12.5 because Plex runs small.
    pub const fn new() -> Self {
        Self {
            small: TypeStep::sans(10.5, 14.0),
            body: TypeStep::sans(13.0, 18.0),
            button: TypeStep::sans(13.0, 18.0),
            monospace: TypeStep::mono(12.5, 18.0),
            heading: TypeStep::sans(20.0, 26.0),
            title: TypeStep::sans(15.0, 20.0),
            caption: TypeStep::sans(11.0, 14.0),
            brand: TypeStep::sans(21.0, 26.0),
        }
    }

    /// The step behind an egui text style; unknown named styles fall back to body.
    pub fn step(&self, style: &TextStyle) -> TypeStep {
        match style {
            TextStyle::Small => self.small,
            TextStyle::Body => self.body,
            TextStyle::Button => self.button,
            TextStyle::Monospace => self.monospace,
            TextStyle::Heading => self.heading,
            TextStyle::Name(n) if &**n == TITLE => self.title,
            TextStyle::Name(n) if &**n == CAPTION => self.caption,
            TextStyle::Name(_) => self.body,
        }
    }

    /// All styles as egui's `text_styles` map (keeps other named styles an app added).
    pub fn apply(&self, text_styles: &mut BTreeMap<TextStyle, FontId>) {
        for style in [
            TextStyle::Small,
            TextStyle::Body,
            TextStyle::Button,
            TextStyle::Monospace,
            TextStyle::Heading,
            title(),
            caption(),
        ] {
            let font = self.step(&style).font_id();
            text_styles.insert(style, font);
        }
    }
}

impl Default for TypeScale {
    fn default() -> Self {
        Self::new()
    }
}
