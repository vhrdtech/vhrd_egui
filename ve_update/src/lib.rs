//! The update bar of a native app (CHR-9).
//!
//! An installer may put a new release of the app in place while the old process keeps running, and keep older
//! releases for a way back. This crate shows that in the app's title or status bar and acts on it:
//!
//! - [`UpdateBar`]: "New version installed: 0.4.2 (a1b2c3d, 2 h ago) · Restart" when the installed release is not
//!   the running one, the running version otherwise; its menu lists every kept release with "Switch to this one",
//!   disabled with the reason where the installer would refuse. Restart asks first while the app says it is busy.
//! - [`Releases`]: where the bar's data comes from. The app's installer implements it (which release runs, which is
//!   installed, which are kept, how to switch); the crate knows no installer.
//! - [`Relaunch`]: the restart itself. The bar asks for it and closes the window; after `eframe::run_native`
//!   returns, the app calls [`Relaunch::exec`], which starts the installed program with the same arguments (on
//!   Unix in the same process, so its pid and parent stay). [`Placement`] carries the window's size and position
//!   over where the platform lets the app set them.
//!
//! Not a web crate: it starts processes and reads files.

mod bar;
mod model;
mod relaunch;

pub use bar::{RESTART, SWITCH, UpdateBar};
pub use model::{Release, Releases, Running, Tone, View};
pub use relaunch::{Placement, PlacementStore, Planned, Relaunch};
