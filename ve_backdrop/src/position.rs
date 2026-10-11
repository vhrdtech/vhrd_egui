//! Where the window is on its monitor, for the emulated transparency to line the blurred wallpaper up with
//! the real one. X11, Windows and macOS tell (egui's `ViewportInfo::inner_rect`); Wayland never does, so
//! there a [`PositionProvider`] asks the compositor in its own way (Hyprland: [`crate::Hyprland`]).

use egui::{Rect, Vec2};

/// The window on its monitor.
#[derive(Clone, Debug, PartialEq)]
pub struct Placement {
    /// The window's content rectangle, relative to the top-left corner of its monitor.
    pub window: Rect,
    /// Size of that monitor, in the same unit as `window` (any unit: only the proportions are used).
    pub monitor: Vec2,
    /// Name of the monitor (`DP-1`, `eDP-1`), for desktops with a wallpaper per output.
    pub output: Option<String>,
}

/// Something that knows where the window is. Asked every frame from `ui()`, so it must answer from memory:
/// whatever talks to the compositor runs on a thread of its own and wakes the UI when the answer changes.
///
/// A [`Placement`] is itself a provider that always gives the same answer (an app that knows, a test).
pub trait PositionProvider: Send {
    /// The window's place now, or `None` when it is not known (the wallpaper is then fitted to the window).
    fn placement(&self) -> Option<Placement>;
}

impl PositionProvider for Placement {
    fn placement(&self) -> Option<Placement> {
        Some(self.clone())
    }
}
