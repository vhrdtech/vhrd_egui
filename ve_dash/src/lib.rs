//! btop-style dashboard building blocks for egui.
//!
//! Small, composable pieces for status dashboards: a dark [`Theme`] with
//! CVD-validated colors, a [`History`] ring buffer, a [`Decay`]
//! VU-style value smoother, and widgets —
//! [`Sparkline`] (filled line chart with hover readout, optional load colors), [`Meter`]
//! (segmented block meter with a heat gradient), [`SegBar`] (exactly 5 or 10 segments), [`StatusLight`]
//! (glowing dot + label), [`StatTile`] (headline number) and a titled
//! [`panel`] frame. [`steady_width`] keeps live values from shoving their
//! neighbours around.
//!
//! The `fonts` feature (on by default) embeds the brand faces IBM Plex Sans and
//! IBM Plex Mono; call [`install_fonts`] once at startup.
//!
//! Run the demo: `cargo run -p ve_dash --example dash_demo`.

mod decay;
#[cfg(feature = "fonts")]
mod fonts;
mod history;
mod meter;
mod panel;
mod segbar;
mod sparkline;
mod stat;
mod status;
mod steady;
mod theme;

pub use decay::Decay;
#[cfg(feature = "fonts")]
pub use fonts::{PLEX_MONO, PLEX_SANS, install_fonts};
pub use history::History;
pub use meter::Meter;
pub use panel::panel;
pub use segbar::{SegBar, Segments, lit_segments};
pub use sparkline::Sparkline;
pub use stat::StatTile;
pub use status::{Status, StatusLight};
pub use steady::steady_width;
pub use theme::{DARK, Theme, heat, load_color};
