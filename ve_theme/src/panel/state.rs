//! What a side panel remembers between frames and runs (its place, width, folds), and its settings.

use std::collections::BTreeMap;

use egui::{Pos2, pos2};

use crate::LightStyle;

/// Folds a panel remembers at most.
pub const FOLDS_MAX: usize = 10_000;

/// The window edge a panel docks to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Side {
    /// The window's left edge.
    #[default]
    Left,
    /// The window's right edge.
    Right,
}

impl Side {
    /// The other edge.
    pub fn flip(self) -> Self {
        match self {
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }
}

/// How much of a panel shows (S7).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Shown {
    /// The whole panel: header, filter, list.
    #[default]
    Open,
    /// Collapsed to its rail (Ctrl+B).
    Rail,
    /// Hidden; a vertical tab on its edge brings it back (Ctrl+Shift+B).
    Hidden,
}

/// The panel's form (S6, scope window).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PanelForm {
    /// B2, the pick: a list with groups that collapses to its own rail of the groups.
    #[default]
    ListToRail,
    /// B1: a rail of panel views beside the list, for an app with several views.
    RailAndList,
    /// B3: the rail only, each item with a hover card.
    RailOnly,
}

/// Which side of a linked tree and view keeps the other in step (S28, scope window).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Follow {
    /// Each scrolls on its own; a click still reveals.
    #[default]
    Off,
    /// The tree keeps the view's top row in sight.
    TreeFollowsView,
    /// The view keeps the tree's top row in sight.
    ViewFollowsTree,
}

/// A panel's settings, kept with the app's settings: form (S6), follow (S28), and its status lights (S29)
/// when they differ from the app's (`Tokens::chrome.lights`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(default)
)]
pub struct PanelSettings {
    /// List to rail, views rail and list, or rail only (S6).
    pub form: PanelForm,
    /// Which of a linked tree and view keeps the other in step (S28).
    pub follow: Follow,
    /// `None` follows `Tokens::chrome.lights`.
    pub lights: Option<LightStyle>,
}

/// A side panel's visual state: its place, width and folds (serialized with the layout), and its filter text
/// (not saved).
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(default)
)]
pub struct SidePanelState {
    /// The edge it docks to, the edge its tab sits on while hidden, and where it docks again from floating.
    pub side: Side,
    /// A window of its own over the work, instead of docked.
    pub floating: bool,
    /// Open, collapsed to its rail, or hidden.
    pub shown: Shown,
    /// Width in points; `None` is the token default (`Chrome::side_panel_default`).
    pub width: Option<f32>,
    /// The floating window's top left corner, points; `None` places it beside its edge.
    pub float_at: Option<[f32; 2]>,
    /// Folds the person changed: a stable hash of the row's key or the group's title or key, and whether it is
    /// open. Others follow their default. At most [`FOLDS_MAX`].
    pub folds: BTreeMap<u64, bool>,
    /// The view picked on the views rail (form B1).
    pub view: usize,
    /// The filter text.
    #[cfg_attr(feature = "serde", serde(skip))]
    pub filter: String,
    /// The "Not matching" fold is open.
    #[cfg_attr(feature = "serde", serde(skip))]
    pub misses_open: bool,
}

impl SidePanelState {
    /// A panel docked to `side`, open, at the default width.
    pub fn docked(side: Side) -> Self {
        Self {
            side,
            ..Self::default()
        }
    }

    /// Whether the fold `key` (a row's or group's fold key) is open, given its default.
    pub(crate) fn is_open(&self, key: u64, default: bool) -> bool {
        if self.folds.is_empty() {
            return default;
        }
        self.folds.get(&key).copied().unwrap_or(default)
    }

    /// Open or fold `key`. At most [`FOLDS_MAX`] folds are kept: past it, the one with the lowest key goes.
    pub(crate) fn set_open(&mut self, key: u64, open: bool) {
        if self.folds.len() >= FOLDS_MAX && !self.folds.contains_key(&key) {
            self.folds.pop_first();
        }
        self.folds.insert(key, open);
    }

    pub(crate) fn float_pos(&self) -> Option<Pos2> {
        self.float_at.map(|[x, y]| pos2(x, y))
    }
}

#[cfg(all(test, feature = "serde"))]
mod tests {
    use super::*;

    /// The filter is not saved; place, width and folds are.
    #[test]
    fn state_round_trips_without_its_filter() {
        let mut s = SidePanelState::docked(Side::Right);
        s.width = Some(312.0);
        s.set_open(7, false);
        s.filter = "ph".to_owned();
        let json = serde_json::to_string(&s).expect("serializes");
        let back: SidePanelState = serde_json::from_str(&json).expect("deserializes");
        assert_eq!(back.side, Side::Right);
        assert_eq!(back.width, Some(312.0));
        assert!(!back.is_open(7, true));
        assert!(back.filter.is_empty());
    }
}
