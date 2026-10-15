//! The frame every vhrd egui widget sits in.
//!
//! One frame for dashboard panels, shell widgets and, later, bodies of a physics layout, in place of the
//! pieces that grew apart (the old `ve_dash::Panel` with its size switch, the status light, floating windows):
//!
//! - [`WidgetFrame`] draws it: a header with an optional LED, the title, an optional ⓘ (what the widget is,
//!   plus its own stats), a size switch and a ☰ menu; the body; an optional footer; optional sides — two for
//!   a rectangle, four for a hexagon ([`Shape::Hex`], items rotated along the slanted faces).
//! - Sizes ([`SizeMode`], [`Size`], [`SizeHints`]): the switch cycles auto / min / nom / wide. *min* is a
//!   fixed size the widget provides; *nom* and *wide* are layout hints its body reads; *auto* picks among the
//!   three from the room given, with a hysteresis so nothing flips ([`auto_size`]).
//! - Moves ([`FrameState`], [`Placement`], [`FrameAction`], [`Dock`]): the menu pops a widget out as an egui
//!   `Window` and back into the grid, or sends it to another OS window (egui multi-viewport) where it again
//!   sits in the grid or floats. The widget value and its [`FrameState`] travel as they are.
//! - Another machine ([`Remote`], [`Machine`], [`Parcel`]): offered when the app implements [`Remote`];
//!   otherwise the entry is there, disabled, and says why.
//!
//! - The app's own UI ([`WidgetFrame::header_item`], [`WidgetFrame::menu`], footer and sides): the frame
//!   gives it room and knows nothing of what it is (README "The app's own UI in the frame").
//!
//! The frame keeps no state and moves nothing itself: `show` returns the menu pick and the widget's owner (a
//! [`Dock`], or the app's own structure) carries it out.
//!
//! ```
//! # use ve_frame::{Dock, SizeHints, ViewportSlot, WidgetFrame};
//! # egui::__run_test_ui(|ui| {
//! let mut dock: Dock<String> = Dock::new();
//! dock.add("supply".to_owned());
//! let moves = dock.moves(None); // no transport to other machines
//! let mut picked = Vec::new();
//! for id in dock.ids(ViewportSlot::MAIN, false) {
//!     let slot = dock.get_mut(id).unwrap();
//!     let r = WidgetFrame::new(slot.widget.as_str())
//!         .sizes(SizeHints::new([80.0, 30.0], [200.0, 80.0], [360.0, 80.0]))
//!         .moves(&moves)
//!         .show(ui, &mut slot.frame, |ui, size| { ui.label(size.label()); });
//!     picked.extend(r.action.map(|action| (id, action)));
//! }
//! for (id, action) in picked {
//!     dock.apply(id, action);
//! }
//! # });
//! ```
//!
//! Try everything: `cargo run -p ve_frame --example playground`.

mod dock;
mod frame;
mod ghost;
pub mod hex;
mod menu;
mod remote;
mod size;
mod state;
mod switch;

pub use dock::{Applied, Dock, Slot, SlotId};
pub use frame::{
    BodyArea, FloatLook, FrameResponse, Shape, WidgetFrame, forward_scroll, header_fill,
    menu_open_look, outer_width, quiet_fill,
};
pub use ghost::ghost_slot;
pub use hex::{HexFace, SideItem, Wing};
pub use menu::{Moves, NO_REMOTE};
pub use remote::{Machine, Parcel, Remote};
pub use size::{
    AUTO_HYSTERESIS, Height, HeightMode, Size, SizeHints, SizeMode, auto_height, auto_size,
};
pub use state::{
    AWAY_SECS, FOLD_SECS, FrameAction, FrameState, GLOW_SECS, LandMode, PLUCK_SECS, Placement,
    ViewportSlot, ViewportTarget,
};

/// The status an LED shows, from `ve_dash`.
pub use ve_dash::Status;
