//! Where a frame sits and what the user picked in its menu (FRM-4).

use serde::{Deserialize, Serialize};

use crate::size::{Height, HeightMode, Size, SizeMode};

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
    /// What the width switch is set to.
    pub size: SizeMode,
    /// What the height switch is set to (FRM-11); auto in states saved before it existed.
    pub height: HeightMode,
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
    /// Whether the frame pushes its neighbours out of its way in a physics layout (ve_settle SETL-40): when it is
    /// dragged onto them, dropped there or grows. Off (the default), it stops at their edge and nothing else
    /// moves. The ☰ menu's "Push neighbours" ([`crate::WidgetFrame::layout_menu`]).
    pub push: bool,
    /// The height the user gave the frame in a grid with its resize handle (ve_settle SETL-39), in points, all
    /// of the frame; `None` (the default): as tall as its content. The body scrolls, or warns, where its
    /// content wants more. A floating frame has its own size in `window_size`.
    pub grid_height: Option<f32>,
    /// The width the user gave the frame in a grid with its resize handle (FRM-15, ve_settle SETL-42), in points, all of
    /// the frame; `None` (the default): the width its layout gives it. With [`FrameState::grid_height`] it is
    /// the size the user dragged the frame to, which it keeps: on auto the switches pick the forms that fit it,
    /// and the body scrolls where its content wants more.
    pub grid_width: Option<f32>,
    /// The size shown last frame: auto's memory for its hysteresis. Not saved.
    #[serde(skip)]
    pub(crate) last_size: Option<Size>,
    /// The height shown last frame, the same for the height.
    #[serde(skip)]
    pub(crate) last_height: Option<Height>,
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
    /// Since when the frame waits to be seen: it landed in a grid outside the view (egui's clock, seconds).
    #[serde(skip)]
    pub(crate) away_since: Option<f64>,
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
    /// Where the width and the height switch were drawn last, if the frame has them.
    #[serde(skip)]
    pub(crate) switch_at: Option<egui::Pos2>,
    #[serde(skip)]
    pub(crate) height_switch_at: Option<egui::Pos2>,
    /// Which of the two switches the anchor belongs to: the height switch when true.
    #[serde(skip)]
    pub(crate) anchor_height: bool,
    /// The header's alignment line, screen y: the icons' centres and the title text's optical centre.
    #[serde(skip)]
    pub(crate) header_line: Option<f32>,
    /// Whether the ☰ menu was open when the frame was drawn last, and where its button is.
    #[serde(skip)]
    pub(crate) menu_open: bool,
    #[serde(skip)]
    pub(crate) menu_at: Option<egui::Rect>,
    /// A size switch was clicked at this screen point: it should be there again once the frame has its new
    /// size (the window moves, or the grid scrolls). `anchor_fresh` in the frame of the click, `anchor_due` in
    /// the one after, when the new layout is known.
    #[serde(skip)]
    pub(crate) anchor: Option<egui::Pos2>,
    #[serde(skip)]
    pub(crate) anchor_fresh: bool,
    #[serde(skip)]
    pub(crate) anchor_due: bool,
    /// What a layout picked for auto ([`FrameState::set_auto_pick`]).
    #[serde(skip)]
    pub(crate) auto_pick: Option<Size>,
    /// What a layout picked for the height on auto ([`FrameState::set_auto_pick_height`]).
    #[serde(skip)]
    pub(crate) auto_pick_height: Option<Height>,
    /// How much the body's content wanted more than the frame gave it last frame, per axis
    /// ([`crate::FrameResponse::overflow`]), and the axes it scrolls on as the fallback for that.
    #[serde(skip)]
    pub(crate) overflow: egui::Vec2,
    #[serde(skip)]
    pub(crate) overflow_scroll: egui::Vec2b,
    /// The size a layout gives a hexagon in a grid ([`FrameState::set_layout_size`]).
    #[serde(skip)]
    pub(crate) layout_size: Option<[f32; 2]>,
    /// The most height a layout has for a rectangle in a grid ([`FrameState::set_layout_height`]).
    #[serde(skip)]
    pub(crate) layout_height: Option<f32>,
    /// Where the body was laid out last, in screen points.
    #[serde(skip)]
    pub(crate) body: Option<egui::Rect>,
    /// The size the frame was drawn with the last time it was drawn in full (not cut off by folding).
    #[serde(skip)]
    pub(crate) open_size: Option<[f32; 2]>,
}

/// How long the glow around a frame that just landed takes to fade, in seconds.
pub const GLOW_SECS: f64 = 0.6;

/// How long a widget that landed outside the view waits to be seen, in seconds: that long its place is pointed
/// at from the edge (or the view goes after it), and its landing glow is held back for the moment it shows.
pub const AWAY_SECS: f64 = 60.0;

/// What happens when a widget lands in a grid at a place that is scrolled out of view (it was put back from a
/// floating window, or arrived from another window), so it doesn't look as if it vanished
/// ([`crate::WidgetFrame::land_mode`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LandMode {
    /// The view stays. A marker with the widget's title sits on the edge of the view toward it (in a corner
    /// when it is off along both axes); a click on it scrolls there. The landing glow waits until the widget
    /// comes into view, [`AWAY_SECS`] at most.
    #[default]
    Stay,
    /// The view scrolls to the widget's place, then it glows there as usual.
    Follow,
}

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

    /// The width step the body was given last frame; `None` before the first one.
    pub fn shown_size(&self) -> Option<Size> {
        self.last_size
    }

    /// The height step the body was given last frame; `None` before the first one.
    pub fn shown_height(&self) -> Option<Height> {
        self.last_height
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

    /// Whether the frame landed outside the view and still waits to be seen ([`LandMode`]).
    pub fn is_away(&self) -> bool {
        self.away_since.is_some()
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

    /// Where the body was laid out last, in screen points. With [`Self::drawn_rect`] it tells a layout how much
    /// the frame takes around a body: header, footer, sides, padding.
    pub fn body_rect(&self) -> Option<egui::Rect> {
        self.body
    }

    /// The size the frame had the last time it was drawn in full: what a folded frame unfolds to.
    pub fn open_size(&self) -> Option<egui::Vec2> {
        self.open_size.map(Into::into)
    }

    /// Whether the frame drawn last is a hexagon.
    pub fn is_hex(&self) -> bool {
        self.hex
    }

    /// A layout that chooses sizes itself (a physics layout picks by the room it can make) says which of the
    /// three a frame on auto shows: with `Some`, auto shows that size and fills the room given instead of
    /// choosing by the room's width ([`crate::auto_size`]). `None` gives the choice back to the frame. Not
    /// saved; the pinned sizes of the switch are not touched by it.
    pub fn set_auto_pick(&mut self, pick: Option<Size>) {
        self.auto_pick = pick;
    }

    /// What a layout picked for auto, if it did.
    pub fn auto_pick(&self) -> Option<Size> {
        self.auto_pick
    }

    /// The same as [`Self::set_auto_pick`] for the height: with `Some`, a frame whose height switch is on auto
    /// shows that height step instead of choosing by the room's height ([`crate::auto_height`]). Ignored for
    /// a widget without heights of its own ([`crate::SizeHints::has_heights`]). Not saved.
    pub fn set_auto_pick_height(&mut self, pick: Option<Height>) {
        self.auto_pick_height = pick;
    }

    /// What a layout picked for the height on auto, if it did.
    pub fn auto_pick_height(&self) -> Option<Height> {
        self.auto_pick_height
    }

    /// A layout that sizes frames itself gives a hexagon in a grid its whole size, outline included, as a
    /// floating hexagon has the size of its window: its body is what that leaves (never under the hint's
    /// minimum), its faces stay at 60°. `None`: the hints make the outline. A rectangle takes the room it is
    /// given and ignores this. Not saved.
    pub fn set_layout_size(&mut self, size: Option<[f32; 2]>) {
        self.layout_size = size;
    }

    /// A layout that has less height for a rectangle in a grid than its content wants (a physics layout whose
    /// frame does not push its neighbours, ve_settle SETL-40) says how much: the frame takes that height, and its
    /// body scrolls or warns as for [`Self::grid_height`]. `None` gives it its own height back. Not saved; a
    /// hexagon ignores it.
    pub fn set_layout_height(&mut self, height: Option<f32>) {
        self.layout_height = height;
    }

    /// The height a layout capped a rectangle at, if it did.
    pub fn layout_height(&self) -> Option<f32> {
        self.layout_height
    }

    /// The size a layout gave a hexagon in a grid, if it did.
    pub fn layout_size(&self) -> Option<[f32; 2]> {
        self.layout_size
    }

    /// How much the body's content wanted more than the frame gave it last frame, in points per axis; 0
    /// where it fits ([`crate::FrameResponse::overflow`]).
    pub fn overflow(&self) -> egui::Vec2 {
        self.overflow
    }

    /// The axes the body scrolls on as the fallback for content that overflowed
    /// ([`crate::WidgetFrame::scroll_fallback`]); the header shows the warning while one is on.
    pub fn overflow_scroll(&self) -> egui::Vec2b {
        self.overflow_scroll
    }

    /// The line, screen y, the header was drawn on last frame: the centre of its icons (LED, ⓘ, switches, ☰,
    /// fold) and the optical centre of its title text. An app's own header item
    /// ([`crate::WidgetFrame::header_item`]) centres its icon on it to read as one row.
    pub fn header_line(&self) -> Option<f32> {
        self.header_line
    }

    /// Whether the ☰ menu was open when the frame was drawn last; its button is drawn pressed then.
    pub fn menu_open(&self) -> bool {
        self.menu_open
    }

    /// Where the ☰ button was drawn last, if the frame has one.
    pub fn menu_rect(&self) -> Option<egui::Rect> {
        self.menu_at
    }

    /// Where the width switch was drawn last, if the frame has one.
    pub fn switch_pos(&self) -> Option<egui::Pos2> {
        self.switch_at
    }

    /// Where the height switch was drawn last, if the frame has one.
    pub fn height_switch_pos(&self) -> Option<egui::Pos2> {
        self.height_switch_at
    }

    /// Where the switch that was clicked last is now: the one an anchor holds in place.
    pub(crate) fn anchored_switch(&self) -> Option<egui::Pos2> {
        if self.anchor_height {
            self.height_switch_at
        } else {
            self.switch_at
        }
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
    /// Tidy the layout up toward a side: a physics layout settles once that way (ve_settle SETL-38, SETL-43).
    /// From the ☰ menu's "Tidy up" ([`crate::WidgetFrame::layout_menu`]).
    Tidy(Toward),
}

/// Which way a physics layout's "Tidy up" packs the widgets (FRM-18, ve_settle SETL-43): every widget slides
/// that way against what lies before it, then everything stays put again.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Toward {
    #[default]
    Up,
    Left,
    Down,
    Right,
}

impl Toward {
    /// The four, in the order a menu lists them.
    pub const ALL: [Self; 4] = [Self::Up, Self::Left, Self::Down, Self::Right];

    /// Its word: `up`, `left`, `down`, `right`.
    pub fn label(self) -> &'static str {
        match self {
            Self::Up => "up",
            Self::Left => "left",
            Self::Down => "down",
            Self::Right => "right",
        }
    }

    /// The one with this word; `None` for anything else.
    pub fn from_label(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.label() == word)
    }

    /// An arrow that way, for a menu entry or a chip.
    pub fn arrow(self) -> &'static str {
        match self {
            Self::Up => "⇡",
            Self::Left => "⇠",
            Self::Down => "⇣",
            Self::Right => "⇢",
        }
    }
}
