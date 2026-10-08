//! btop-style dashboard building blocks for egui.
//!
//! Small, composable pieces for status dashboards: a dark [`Theme`] with
//! CVD-validated colors, a [`History`] ring buffer, a [`Decay`]
//! VU-style value smoother, and widgets —
//! [`Sparkline`] (filled line chart with hover readout, optional load colors), [`Meter`]
//! (segmented block meter with a heat gradient), [`SegBar`] (exactly 5 or 10 segments), [`StatusLight`]
//! (glowing dot + label), [`StatTile`] (headline number) and a titled
//! [`panel`] frame. [`steady_width`] keeps live values from shoving their
//! neighbours around, and [`SteadyColumn`] aligns the first cell of list rows without a hard-coded width.
//!
//! Colors come from `ve_theme`'s design tokens ([`Theme::from_tokens`]); apps turn the whole
//! look on with `ve_theme::setup(ctx)`, which also installs the brand fonts. ve_dash's own
//! `install_fonts` is deprecated and forwards to `ve_theme::install_fonts`.
//!
//! Run the demo: `cargo run -p ve_dash --example dash_demo`.

mod column;
mod decay;
mod history;
mod meter;
mod panel;
mod segbar;
mod sparkline;
mod stat;
mod status;
mod steady;
mod theme;

pub use column::SteadyColumn;
pub use decay::Decay;
pub use history::History;
pub use meter::Meter;
pub use panel::panel;
pub use segbar::{SegBar, Segments, lit_segments};
pub use sparkline::Sparkline;
pub use stat::StatTile;
pub use status::{Status, StatusLight};
pub use steady::{steady_of, steady_width};
pub use theme::{DARK, Theme, heat, load_color};

/// IBM Plex Sans Regular, TTF bytes.
#[cfg(feature = "fonts")]
#[deprecated(since = "0.9.0", note = "moved to ve_theme::PLEX_SANS")]
pub const PLEX_SANS: &[u8] = ve_theme::PLEX_SANS;

/// IBM Plex Mono Regular, TTF bytes.
#[cfg(feature = "fonts")]
#[deprecated(since = "0.9.0", note = "moved to ve_theme::PLEX_MONO")]
pub const PLEX_MONO: &[u8] = ve_theme::PLEX_MONO;

/// Install the brand fonts. Moved to `ve_theme`; prefer `ve_theme::setup`, which installs them together
/// with the dark and light styles.
#[cfg(feature = "fonts")]
#[deprecated(
    since = "0.9.0",
    note = "use ve_theme::setup (fonts + styles) or ve_theme::install_fonts"
)]
pub fn install_fonts(ctx: &egui::Context) {
    ve_theme::install_fonts(ctx);
}
