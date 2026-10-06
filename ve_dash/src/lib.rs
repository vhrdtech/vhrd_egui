//! btop-style dashboard building blocks for egui.
//!
//! Small, composable pieces for status dashboards: a dark [`Theme`] with
//! CVD-validated colors, a [`History`] ring buffer, and widgets —
//! [`Sparkline`] (filled line chart with hover readout), [`Meter`]
//! (segmented block meter with a heat gradient), [`StatusLight`]
//! (glowing dot + label), [`StatTile`] (headline number) and a titled
//! [`panel`] frame.
//!
//! Run the demo: `cargo run -p ve_dash --example mesh_dash`.

mod history;
mod meter;
mod panel;
mod sparkline;
mod stat;
mod status;
mod theme;

pub use history::History;
pub use meter::Meter;
pub use panel::panel;
pub use sparkline::Sparkline;
pub use stat::StatTile;
pub use status::{Status, StatusLight};
pub use theme::{DARK, Theme, heat};
