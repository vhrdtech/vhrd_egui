//! Widget sizes (FRM-2): what the user picks on the size switch, what the widget's body is told, and how
//! *auto* chooses.

use egui::Vec2;
use serde::{Deserialize, Serialize};

/// What the size switch of a frame is set to. *Auto* lets the frame pick among the three from the room it is
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

/// The size a widget's body arranges itself for.
///
/// - `Min`: the body is exactly [`SizeHints::min`], a fixed size the widget provides.
/// - `Nom` and `Wide`: layout hints. The body reads them to arrange its elements (one column or two, with or
///   without the chart); the frame gives it at least the hinted size and lets it grow.
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
}

/// The body sizes a widget offers, in points, without the frame's own header, footer and sides.
/// A layout reads them too: `min` is the least it may give, the widths of `nom` and `wide` what to aim for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SizeHints {
    /// The fixed size of the smallest form.
    pub min: Vec2,
    /// The size the normal form wants; it may grow.
    pub nom: Vec2,
    /// The size the wide form wants; it may grow.
    pub wide: Vec2,
}

impl SizeHints {
    /// Hints from the three sizes. Keeps them ordered: nom is at least as wide as min, wide as nom.
    pub fn new(min: impl Into<Vec2>, nom: impl Into<Vec2>, wide: impl Into<Vec2>) -> Self {
        let min = min.into();
        let mut nom = nom.into();
        let mut wide = wide.into();
        nom.x = nom.x.max(min.x);
        wide.x = wide.x.max(nom.x);
        Self { min, nom, wide }
    }

    /// The hinted body size of `size`.
    pub fn of(&self, size: Size) -> Vec2 {
        match size {
            Size::Min => self.min,
            Size::Nom => self.nom,
            Size::Wide => self.wide,
        }
    }

    /// The body width a layout should give a frame set to `mode`; with auto, whatever it has (`room`).
    pub fn width_for(&self, mode: SizeMode, room: f32) -> f32 {
        match mode.fixed() {
            Some(size) => self.of(size).x,
            None => room.max(self.min.x),
        }
    }
}

/// Extra room, in points, a step up needs over the step down, so a frame whose room sits right on a threshold
/// doesn't flip between two sizes every frame.
pub const AUTO_HYSTERESIS: f32 = 24.0;

/// What *auto* picks: the widest of min / nom / wide whose hinted width fits into `width`, else min.
/// A size wider than `prev` (what was shown last frame) must fit with [`AUTO_HYSTERESIS`] to spare; staying or
/// stepping down needs none. Height plays no part: bodies grow downward.
pub fn auto_size(prev: Option<Size>, width: f32, hints: &SizeHints) -> Size {
    let fits = |size: Size| {
        let spare = match prev {
            Some(prev) if size > prev => AUTO_HYSTERESIS,
            _ => 0.0,
        };
        width >= hints.of(size).x + spare
    };
    if fits(Size::Wide) {
        Size::Wide
    } else if fits(Size::Nom) {
        Size::Nom
    } else {
        Size::Min
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn pinned_modes_ignore_the_room() {
        let h = hints();
        assert_eq!(SizeMode::Wide.resolve(None, 10.0, &h), Size::Wide);
        assert_eq!(SizeMode::Min.resolve(None, 9999.0, &h), Size::Min);
        assert_eq!(h.width_for(SizeMode::Nom, 999.0), 200.0);
        assert_eq!(h.width_for(SizeMode::Auto, 10.0), 80.0);
    }

    #[test]
    fn hints_stay_ordered() {
        let h = SizeHints::new([100.0, 10.0], [50.0, 10.0], [20.0, 10.0]);
        assert_eq!((h.nom.x, h.wide.x), (100.0, 100.0));
    }
}
