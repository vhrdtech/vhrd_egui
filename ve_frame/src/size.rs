//! Widget sizes (FRM-2, FRM-11): what the user picks on the two size switches (width and height), what the
//! widget's body is told, and how *auto* chooses on each axis.

use egui::Vec2;
use serde::{Deserialize, Serialize};

/// What the width switch of a frame is set to. *Auto* lets the frame pick among the three from the room it is
/// given ([`auto_size`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SizeMode {
    #[default]
    Auto,
    Min,
    Nom,
    Wide,
}

impl SizeMode {
    /// All four, in the order the switch cycles them.
    pub const ALL: [Self; 4] = [Self::Auto, Self::Min, Self::Nom, Self::Wide];

    /// The next one in the cycle auto → min → nom → wide → auto (what a click on the switch does).
    pub fn next(self) -> Self {
        match self {
            Self::Auto => Self::Min,
            Self::Min => Self::Nom,
            Self::Nom => Self::Wide,
            Self::Wide => Self::Auto,
        }
    }

    /// The word on the switch.
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Min => "min",
            Self::Nom => "nom",
            Self::Wide => "wide",
        }
    }

    /// The size this mode pins the widget to; `None` for auto.
    pub fn fixed(self) -> Option<Size> {
        match self {
            Self::Auto => None,
            Self::Min => Some(Size::Min),
            Self::Nom => Some(Size::Nom),
            Self::Wide => Some(Size::Wide),
        }
    }

    /// The size the body gets with `width` of room: the pinned one, or [`auto_size`].
    pub fn resolve(self, prev: Option<Size>, width: f32, hints: &SizeHints) -> Size {
        self.fixed()
            .unwrap_or_else(|| auto_size(prev, width, hints))
    }
}

/// The width step a widget's body arranges itself for.
///
/// - `Min`: the body is exactly [`SizeHints::min`] wide, a fixed size the widget provides.
/// - `Nom` and `Wide`: layout hints. The body reads them to arrange its elements (one column or two, with or
///   without the chart); the frame gives it at least the hinted width.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub enum Size {
    Min,
    #[default]
    Nom,
    Wide,
}

impl Size {
    /// The word for it, as on the switch.
    pub fn label(self) -> &'static str {
        match self {
            Self::Min => "min",
            Self::Nom => "nom",
            Self::Wide => "wide",
        }
    }

    /// 0, 1, 2 from min to wide.
    pub fn index(self) -> usize {
        self as usize
    }
}

/// What the height switch of a frame is set to, the vertical twin of [`SizeMode`]. *Auto* picks among the
/// three from the room's height ([`auto_height`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HeightMode {
    #[default]
    Auto,
    Min,
    Nom,
    Tall,
}

impl HeightMode {
    /// All four, in the order the switch cycles them.
    pub const ALL: [Self; 4] = [Self::Auto, Self::Min, Self::Nom, Self::Tall];

    /// The next one in the cycle auto → min → nom → tall → auto (what a click on the switch does).
    pub fn next(self) -> Self {
        match self {
            Self::Auto => Self::Min,
            Self::Min => Self::Nom,
            Self::Nom => Self::Tall,
            Self::Tall => Self::Auto,
        }
    }

    /// The word on the switch.
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Min => "min",
            Self::Nom => "nom",
            Self::Tall => "tall",
        }
    }

    /// The height this mode pins the widget to; `None` for auto.
    pub fn fixed(self) -> Option<Height> {
        match self {
            Self::Auto => None,
            Self::Min => Some(Height::Min),
            Self::Nom => Some(Height::Nom),
            Self::Tall => Some(Height::Tall),
        }
    }
}

/// The height step a widget's body arranges itself for, the vertical twin of [`Size`].
///
/// - `Min`: the body is exactly [`SizeHints::min`] high.
/// - `Nom` and `Tall`: the body gets at least the hinted height ([`SizeHints::height`]) and may grow.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub enum Height {
    Min,
    #[default]
    Nom,
    Tall,
}

impl Height {
    /// The word for it, as on the switch.
    pub fn label(self) -> &'static str {
        match self {
            Self::Min => "min",
            Self::Nom => "nom",
            Self::Tall => "tall",
        }
    }

    /// 0, 1, 2 from min to tall.
    pub fn index(self) -> usize {
        self as usize
    }
}

/// The body sizes a widget offers, in points, without the frame's own header, footer and sides. A layout
/// reads them too: `min` is the least it may give, the widths of `nom` and `wide` what to aim for.
///
/// Per axis:
/// - widths: `min.x`, `nom.x`, `wide.x` (the width switch);
/// - heights: `min.y`, the nominal height of the width form shown (`nom.y`, or `wide.y` for wide), and
///   [`Self::tall`] (the height switch). Without a tall height the widget has one height per width form: the
///   height switch is shown disabled and the height step follows the width (min with min, else nom).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SizeHints {
    /// The fixed size of the smallest form: its width and its height.
    pub min: Vec2,
    /// The size the normal form wants; it may grow.
    pub nom: Vec2,
    /// The size the wide form wants; it may grow.
    pub wide: Vec2,
    /// The height of the tall form; `None`: the widget has no height steps of its own.
    pub tall: Option<f32>,
}

impl SizeHints {
    /// Hints from the three sizes, without a tall form (add one with [`Self::tall`]). Keeps them ordered: nom
    /// is at least as wide as min, wide as nom.
    pub fn new(min: impl Into<Vec2>, nom: impl Into<Vec2>, wide: impl Into<Vec2>) -> Self {
        let min = min.into();
        let mut nom = nom.into();
        let mut wide = wide.into();
        nom.x = nom.x.max(min.x);
        wide.x = wide.x.max(nom.x);
        Self {
            min,
            nom,
            wide,
            tall: None,
        }
    }

    /// The height of the tall form; turns the height switch on. Kept at least as tall as the nominal heights.
    pub fn tall(mut self, height: f32) -> Self {
        self.tall = Some(height.max(self.nom.y).max(self.wide.y));
        self
    }

    /// Whether the widget offers heights of its own (a tall form): the height switch is live.
    pub fn has_heights(&self) -> bool {
        self.tall.is_some()
    }

    /// The hinted body size of the width form `size` at its nominal height.
    pub fn of(&self, size: Size) -> Vec2 {
        match size {
            Size::Min => self.min,
            Size::Nom => self.nom,
            Size::Wide => self.wide,
        }
    }

    /// The hinted body height of `height` beside the width form `size`.
    pub fn height(&self, size: Size, height: Height) -> f32 {
        match height {
            Height::Min => self.min.y,
            Height::Nom => self.of(size).y.max(self.min.y),
            Height::Tall => self.tall.unwrap_or(self.of(size).y).max(self.of(size).y),
        }
    }

    /// The hinted body size of the width form `size` and the height step `height`.
    pub fn body(&self, size: Size, height: Height) -> Vec2 {
        Vec2::new(self.of(size).x, self.height(size, height))
    }

    /// The height step a widget without heights of its own shows beside the width form `size`.
    pub fn follow(size: Size) -> Height {
        if size == Size::Min {
            Height::Min
        } else {
            Height::Nom
        }
    }

    /// The body width a layout should give a frame set to `mode`; with auto, whatever it has (`room`).
    pub fn width_for(&self, mode: SizeMode, room: f32) -> f32 {
        match mode.fixed() {
            Some(size) => self.of(size).x,
            None => room.max(self.min.x),
        }
    }

    /// The body height a layout should give a frame set to `mode` beside the width form `size`; with auto,
    /// whatever it has (`room`).
    pub fn height_for(&self, size: Size, mode: HeightMode, room: f32) -> f32 {
        match mode.fixed() {
            Some(height) => self.height(size, height),
            None => room.max(self.min.y),
        }
    }
}

/// Extra room, in points, a step up needs over the step down, so a frame whose room sits right on a threshold
/// doesn't flip between two sizes every frame. The same on both axes.
pub const AUTO_HYSTERESIS: f32 = 24.0;

/// The largest of three steps (thresholds `need`, smallest first) that fits into `room`, else the first. A
/// step above `prev` must fit with [`AUTO_HYSTERESIS`] to spare; staying or stepping down needs none.
fn auto_step(prev: Option<usize>, room: f32, need: [f32; 3]) -> usize {
    let fits = |step: usize| {
        let spare = match prev {
            Some(prev) if step > prev => AUTO_HYSTERESIS,
            _ => 0.0,
        };
        room >= need[step] + spare
    };
    (1..3).rev().find(|&step| fits(step)).unwrap_or(0)
}

/// What *auto* picks for the width: the widest of min / nom / wide whose hinted width fits into `width`, else
/// min. A size wider than `prev` (what was shown last frame) must fit with [`AUTO_HYSTERESIS`] to spare;
/// staying or stepping down needs none.
pub fn auto_size(prev: Option<Size>, width: f32, hints: &SizeHints) -> Size {
    let need = [hints.min.x, hints.nom.x, hints.wide.x];
    match auto_step(prev.map(Size::index), width, need) {
        2 => Size::Wide,
        1 => Size::Nom,
        _ => Size::Min,
    }
}

/// What *auto* picks for the height beside the width form `size`: the tallest of min / nom / tall whose hinted
/// height fits into `height`, else min, with the same hysteresis as [`auto_size`]. A widget without heights
/// of its own ([`SizeHints::has_heights`]) follows its width ([`SizeHints::follow`]).
pub fn auto_height(prev: Option<Height>, height: f32, size: Size, hints: &SizeHints) -> Height {
    if !hints.has_heights() {
        return SizeHints::follow(size);
    }
    let need = [Height::Min, Height::Nom, Height::Tall].map(|h| hints.height(size, h));
    match auto_step(prev.map(Height::index), height, need) {
        2 => Height::Tall,
        1 => Height::Nom,
        _ => Height::Min,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::vec2;

    fn hints() -> SizeHints {
        SizeHints::new([80.0, 40.0], [200.0, 100.0], [400.0, 100.0])
    }

    #[test]
    fn switch_cycles_through_all_four() {
        let mut mode = SizeMode::Auto;
        let mut seen = Vec::new();
        for _ in 0..4 {
            seen.push(mode.label());
            mode = mode.next();
        }
        assert_eq!(seen, ["auto", "min", "nom", "wide"]);
        assert_eq!(mode, SizeMode::Auto);
        let mut mode = HeightMode::Auto;
        let mut seen = Vec::new();
        for _ in 0..4 {
            seen.push(mode.label());
            mode = mode.next();
        }
        assert_eq!(seen, ["auto", "min", "nom", "tall"]);
        assert_eq!(mode, HeightMode::Auto);
    }

    #[test]
    fn auto_picks_the_widest_that_fits() {
        let h = hints();
        assert_eq!(auto_size(None, 50.0, &h), Size::Min);
        assert_eq!(auto_size(None, 199.0, &h), Size::Min);
        assert_eq!(auto_size(None, 200.0, &h), Size::Nom);
        assert_eq!(auto_size(None, 399.0, &h), Size::Nom);
        assert_eq!(auto_size(None, 4000.0, &h), Size::Wide);
        assert_eq!(auto_size(None, f32::NAN, &h), Size::Min);
    }

    /// Room wobbling around a threshold keeps the size it had.
    #[test]
    fn auto_does_not_flip_on_the_threshold() {
        let h = hints();
        let mut size = auto_size(None, 190.0, &h);
        assert_eq!(size, Size::Min);
        for width in [201.0, 199.0, 205.0, 198.0, 215.0] {
            size = auto_size(Some(size), width, &h);
            assert_eq!(size, Size::Min, "at {width}");
        }
        size = auto_size(Some(size), 200.0 + AUTO_HYSTERESIS, &h);
        assert_eq!(size, Size::Nom);
        for width in [201.0, 210.0, 200.0] {
            size = auto_size(Some(size), width, &h);
            assert_eq!(size, Size::Nom, "at {width}");
        }
        assert_eq!(auto_size(Some(size), 199.0, &h), Size::Min);
        // two steps up at once still need the spare room of the wide step only
        assert_eq!(auto_size(Some(Size::Min), 430.0, &h), Size::Wide);
    }

    #[test]
    fn auto_height_picks_the_tallest_that_fits_without_flipping() {
        let h = hints().tall(260.0);
        assert_eq!(auto_height(None, 30.0, Size::Nom, &h), Height::Min);
        assert_eq!(auto_height(None, 100.0, Size::Nom, &h), Height::Nom);
        assert_eq!(auto_height(None, 300.0, Size::Nom, &h), Height::Tall);
        let mut height = Height::Nom;
        for room in [261.0, 259.0, 270.0, 280.0] {
            height = auto_height(Some(height), room, Size::Nom, &h);
            assert_eq!(height, Height::Nom, "at {room}");
        }
        height = auto_height(Some(height), 260.0 + AUTO_HYSTERESIS, Size::Nom, &h);
        assert_eq!(height, Height::Tall);
        for room in [261.0, 260.0] {
            height = auto_height(Some(height), room, Size::Nom, &h);
            assert_eq!(height, Height::Tall, "at {room}");
        }
        assert_eq!(auto_height(Some(height), 259.0, Size::Nom, &h), Height::Nom);
    }

    #[test]
    fn without_heights_the_height_follows_the_width() {
        let h = hints();
        assert!(!h.has_heights());
        assert_eq!(auto_height(None, 9999.0, Size::Wide, &h), Height::Nom);
        assert_eq!(auto_height(None, 9999.0, Size::Min, &h), Height::Min);
        assert_eq!(h.body(Size::Min, Height::Min), vec2(80.0, 40.0));
        assert_eq!(h.body(Size::Wide, Height::Nom), h.wide);
    }

    #[test]
    fn pinned_modes_ignore_the_room() {
        let h = hints().tall(300.0);
        assert_eq!(SizeMode::Wide.resolve(None, 10.0, &h), Size::Wide);
        assert_eq!(SizeMode::Min.resolve(None, 9999.0, &h), Size::Min);
        assert_eq!(h.width_for(SizeMode::Nom, 999.0), 200.0);
        assert_eq!(h.width_for(SizeMode::Auto, 10.0), 80.0);
        assert_eq!(h.height_for(Size::Nom, HeightMode::Tall, 10.0), 300.0);
        assert_eq!(h.height_for(Size::Nom, HeightMode::Min, 999.0), 40.0);
        assert_eq!(h.height_for(Size::Nom, HeightMode::Auto, 10.0), 40.0);
    }

    #[test]
    fn hints_stay_ordered() {
        let h = SizeHints::new([100.0, 10.0], [50.0, 10.0], [20.0, 10.0]);
        assert_eq!((h.nom.x, h.wide.x), (100.0, 100.0));
        assert_eq!(hints().tall(50.0).tall, Some(100.0));
    }
}
