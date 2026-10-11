//! Where a frame sits and what the user picked in its menu (FRM-4).

use serde::{Deserialize, Serialize};

use crate::size::{Size, SizeMode};

/// An OS window of the app: `MAIN` is the one the app started with, the others are numbered as they are
/// opened (egui viewports).
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct ViewportSlot(pub u32);

impl ViewportSlot {
    /// The window the app started with.
    pub const MAIN: Self = Self(0);

    /// Whether this is the app's first window.
    pub fn is_main(self) -> bool {
        self == Self::MAIN
    }

    /// The egui viewport of this window.
    pub fn viewport_id(self) -> egui::ViewportId {
        if self.is_main() {
            egui::ViewportId::ROOT
        } else {
            egui::ViewportId::from_hash_of(("ve_frame::viewport", self.0))
        }
    }

    /// "Main window", "Window 2", ...
    pub fn label(self) -> String {
        if self.is_main() {
            "Main window".to_owned()
        } else {
            format!("Window {}", self.0 + 1)
        }
    }
}

/// Where a frame sits: in which OS window, and there in the grid (tiles, rows, a physics layout: the host's
/// business) or floating over it as an egui `Window`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Placement {
    pub viewport: ViewportSlot,
    pub floating: bool,
}

/// Whether a frame just arrived somewhere (popped out, put back, sent to another window) and shows it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) enum Landing {
    #[default]
    No,
    /// Moved; the glow starts with the next frame drawn.
    Pending,
    /// Glowing since this time (egui's clock, seconds).
    Since(f64),
}

/// The frame's own state, kept by whoever owns the widget and saved with it. It travels with the widget on
/// every move.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FrameState {
    /// What the size switch is set to.
    pub size: SizeMode,
    /// Where the frame sits.
    pub placement: Placement,
    /// The size the user dragged the floating window to, in points; `None` until then: the window is as big
    /// as the widget's content. Never smaller than the frame needs.
    pub window_size: Option<[f32; 2]>,
    /// The size shown last frame: auto's memory for its hysteresis. Not saved.
    #[serde(skip)]
    pub(crate) last_size: Option<Size>,
    #[serde(skip)]
    pub(crate) landing: Landing,
    /// How strong the landing glow was drawn last frame, 1 to 0.
    #[serde(skip)]
    pub(crate) glow: f32,
    /// Whether the frame drawn last can be resized as a floating window (a rectangle can, a hexagon keeps
    /// its shape).
    #[serde(skip)]
    pub(crate) resizable: bool,
}

/// How long the glow around a frame that just landed takes to fade, in seconds.
pub const GLOW_SECS: f64 = 0.6;

impl FrameState {
    /// A frame in the main grid with its size switch on `size` (a widget that arrived in a
    /// [`crate::Parcel`]).
    pub fn sized(size: SizeMode) -> Self {
        Self {
            size,
            ..Self::default()
        }
    }

    /// The size the body was given last frame; `None` before the first one.
    pub fn shown_size(&self) -> Option<Size> {
        self.last_size
    }

    /// The widget just arrived where it is now: its frame glows for [`GLOW_SECS`] so the eye finds it.
    /// [`crate::Dock::apply`] calls this on every move; a host that moves widgets itself calls it too.
    /// Nothing glows when animations are off (`egui::Style::animation_time` 0, the reduced-motion setting).
    pub fn land(&mut self) {
        self.landing = Landing::Pending;
    }

    /// How strong the landing glow was drawn last frame: 1 right after a move, 0 when it is over.
    pub fn glow(&self) -> f32 {
        self.glow
    }

    /// Whether the frame, floating, can be resized by the user (set when it is drawn).
    pub fn is_resizable(&self) -> bool {
        self.resizable
    }
}

/// Which OS window a widget is sent to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewportTarget {
    /// A new OS window, to drag onto another monitor.
    New,
    /// A window that is open already.
    Existing(ViewportSlot),
}

/// What the user picked in a frame's menu. The frame only reports it; the owner of the widget carries it out
/// ([`crate::Dock::apply`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FrameAction {
    /// Out of the grid, into an egui `Window` floating in the same OS window.
    PopOut,
    /// Back into the grid of the OS window it floats in.
    DockBack,
    /// Into the grid of another OS window.
    SendToViewport(ViewportTarget),
    /// To another machine, by [`crate::Machine::id`].
    SendToMachine(String),
    /// Close the widget.
    Close,
}
