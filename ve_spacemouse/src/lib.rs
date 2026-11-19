//! 3D mouse input for egui apps through spacenavd, and a pie menu on the device's buttons.
//!
//! - [`SpaceMouse`] (SPM-1): a pure-Rust client of spacenavd's Unix socket (`/var/run/spnav.sock`), no libspnav.
//!   A thread reads the events, reconnects when spacenavd restarts and hands them to the UI through a channel;
//!   [`FakeSpacenavd`] serves the same protocol for tests and for running an app without the device.
//! - [`Mapping`] (SPM-2): which raw axis moves, zooms or turns the content, signs, dead zone, speed and which
//!   button is left or right, as per-app settings; by default the content follows the hand ([`Convention`]).
//!   [`Mapping::apply`] gives a [`Nav`] per second: what the content does. [`Calibration`] measures
//!   it with the person: it asks for one motion at a time, records it, works out the mapping and shows a table
//!   to confirm ([`calibration_ui`]); [`mapping_ui`] edits it by hand.
//! - [`Config`] and [`SettingsPanel`] (SPM-5): one mapping for every app, in `spacemouse.toml` files found the
//!   way Cargo finds its config (`.config/spacemouse.toml` in the current folder and each parent, then
//!   `~/.config/spacemouse.toml`), merged per key with the nearest file winning. The panel is what apps embed:
//!   Calibrate…, the mapping with the file each value comes from, Save to ~/.config or Save locally (SPM-6), the
//!   unsaved changes and the question on closing ([`SettingsPanel::leave_guard`]), the files found.
//!   [`Config::adopt`] moves an app's old per-app mapping into the user's file once.
//! - [`PieMenu`] (SPM-3): tap a button to fire its centre slot; hold it and a ring of slots opens at the
//!   pointer, crossing the outer circle or clicking fires the slot under the pointer, and it stays open while
//!   held so actions chain; each button has its own ring and both together a third.
//!
//! ```no_run
//! # use ve_spacemouse::*;
//! # #[derive(Clone)] enum Action { Save }
//! # fn menu() -> PieMenu<Action> { unimplemented!() }
//! # let ui: &mut egui::Ui = unimplemented!();
//! # let (mut state, mapping, mouse) = (PieState::default(), Mapping::default(), SpaceMouse::start(Options::default()));
//! # let (mut pan, mut zoom) = (egui::Vec2::ZERO, 1.0f32);
//! let events = mouse.poll();
//! for fired in menu().show(ui.ctx(), &mut state, &buttons(&events, &mapping)) {
//!     // fired.action at fired.at
//! }
//! let nav = mapping.apply(&mouse.axes());
//! let dt = ui.input(|i| i.stable_dt);
//! pan += nav.pan_screen() * dt;
//! zoom *= (nav.zoom * dt).exp();
//! ```
//!
//! The reference for the menu's behaviour is a pie menu over KiCad driven by the same socket (a Python script
//! and a Quickshell overlay); this crate brings it into egui apps with the theme's look and phosphor icons.

mod calibrate;
#[cfg(unix)]
mod client;
mod config;
mod event;
#[cfg(unix)]
mod fake;
mod mapping;
mod panel;
mod pie;
mod settings_ui;

pub use calibrate::{
    Calibration, CalibrationAction, Measured, Motion, Step, StepResult, calibration_ui,
};
#[cfg(unix)]
pub use client::{DEFAULT_SOCKET, Options, SpaceMouse, Status};
pub use config::{
    Adopted, Candidate, Config, FILE_NAME, Layer, Saved, candidates, display_path, known_keys,
    user_config_dir, user_file,
};
pub use event::{AXIS_NAMES, Axes, EVENT_SIZE, Event};
#[cfg(unix)]
pub use fake::FakeSpacenavd;
pub use mapping::{AxisMap, Buttons, Convention, Mapping, Nav, Output, Side, Speed};
pub use panel::{CalibrateSteps, SaveTarget, SettingsPanel};
pub use pie::{ButtonEvent, Fired, PieMenu, PieState, PieStyle, Ring, Slot, SlotRef, buttons};
pub use settings_ui::{axes_bars, mapping_ui};

/// Phosphor icons for the slots: `icons::FLOPPY_DISK`. The font comes with `ve_theme::setup`.
#[cfg(feature = "icons")]
pub use ve_theme::icons::ph as icons;
