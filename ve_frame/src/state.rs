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
    /// Where the floating window's top left corner is, in points of its OS window; `None` until it floated.
    /// [`crate::Dock`] sets it when the widget pops out (where it sat in the grid) and follows the user's
    /// drags.
    pub window_pos: Option<[f32; 2]>,
    /// Whether the floating window is folded down to its header. The size it had stays in `window_size` and
    /// comes back when it is unfolded.
    pub collapsed: bool,
    /// The size of the place in the grid the widget floated out of, for a ghost of it there
    /// ([`crate::ghost_slot`]); `None` while it sits in a grid.
    pub left_slot: Option<[f32; 2]>,
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
    /// Whether the frame drawn last is a hexagon (its resize handles sit on its faces).
    #[serde(skip)]
    pub(crate) hex: bool,
    /// The frame drawn last was still learning what its sides and footer take: its size is not one to keep.
    #[serde(skip)]
    pub(crate) settling: bool,
    /// Whether this is the floating window on top of the others (set by [`crate::Dock::show_windows`]).
    #[serde(skip)]
    pub(crate) focused: bool,
    /// How far the frame is unfolded, 0 (header only) to 1, and whether it is short of fully open: then the
    /// size it is drawn with is not its own.
    #[serde(skip)]
    pub(crate) fold: f32,
    #[serde(skip)]
    pub(crate) folding: bool,
    /// Where the frame was drawn last, in screen points: a pop-out opens there.
    #[serde(skip)]
    pub(crate) drawn: Option<egui::Rect>,
    /// The pop-out animation ([`FrameState::pluck`]).
    #[serde(skip)]
    pub(crate) plucking: Landing,
    #[serde(skip)]
    pub(crate) pluck: f32,
    /// Where the size switch was drawn last, if the frame has one.
    #[serde(skip)]
    pub(crate) switch_at: Option<egui::Pos2>,
    /// Whether the ☰ menu was open when the frame was drawn last, and where its button is.
    #[serde(skip)]
    pub(crate) menu_open: bool,
    #[serde(skip)]
    pub(crate) menu_at: Option<egui::Rect>,
    /// The size switch was clicked at this screen point: it should be there again once the frame has its new
    /// size (the window moves, or the grid scrolls). `anchor_fresh` in the frame of the click, `anchor_due` in
    /// the one after, when the new layout is known.
    #[serde(skip)]
    pub(crate) anchor: Option<egui::Pos2>,
    #[serde(skip)]
    pub(crate) anchor_fresh: bool,
    #[serde(skip)]
    pub(crate) anchor_due: bool,
}

/// How long the glow around a frame that just landed takes to fade, in seconds.
pub const GLOW_SECS: f64 = 0.6;

/// How long folding a floating window down to its header, or unfolding it, takes, in seconds.
pub const FOLD_SECS: f32 = 0.2;

/// How long the lift of a widget that was just plucked out of the grid takes, in seconds.
pub const PLUCK_SECS: f64 = 0.3;

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

    /// The widget was just taken out of the grid: its window opens where it sat (same place, same size, unless
    /// the user gave it a size before) and hops up a little for [`PLUCK_SECS`]. [`crate::Dock::apply`] calls
    /// this on a pop-out. With animations off it just sits there.
    pub fn pluck_out(&mut self) {
        if let Some(rect) = self.drawn {
            self.left_slot = Some(rect.size().into());
            self.window_pos = Some(rect.min.into());
            if self.window_size.is_none() {
                self.window_size = Some(rect.size().into());
            }
        }
        self.plucking = Landing::Pending;
    }

    /// How far the pop-out animation is: 1 right after the widget was plucked out, 0 when it rests.
    pub fn pluck(&self) -> f32 {
        self.pluck
    }

    /// Whether this floating window is the one on top: its border takes the accent colour.
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// How far the frame is unfolded: 1 open, 0 folded down to its header, between the two while it moves.
    pub fn openness(&self) -> f32 {
        if self.folding { self.fold } else { 1.0 }
    }

    /// Where the frame was drawn last, in screen points.
    pub fn drawn_rect(&self) -> Option<egui::Rect> {
        self.drawn
    }

    /// Whether the ☰ menu was open when the frame was drawn last; its button is drawn pressed then.
    pub fn menu_open(&self) -> bool {
        self.menu_open
    }

    /// Where the ☰ button was drawn last, if the frame has one.
    pub fn menu_rect(&self) -> Option<egui::Rect> {
        self.menu_at
    }

    /// Where the size switch was drawn last, if the frame has one.
    pub fn switch_pos(&self) -> Option<egui::Pos2> {
        self.switch_at
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
