//! A digital-phosphor scope view for egui on wgpu, made to show what an HDR screen adds.
//!
//! A compute shader rasterises many sweeps per frame (100k+ waveforms per second) into a hit-count buffer;
//! the hits fold into a persistent density that fades with an adjustable time constant (milliseconds to
//! infinite); a log tone map turns density into light. On an extended-range target the densest trace goes up
//! to a peak several times SDR white while UI chrome stays at 1.0, so rare events (a runt pulse in 100 000
//! sweeps, a spike in a million) read as faint ghosts beside a glowing main trace. The split mode puts the
//! classic SDR tone map beside it on the same screen.
//!
//! - [`Phosphor`]: the GPU state on one device; [`PhosphorView`] draws it in an egui paint callback.
//! - [`Settings`]: scene ([`Scene`]: clock with glitches, spectrum persistence, eye diagram, streamed sweeps),
//!   [`Mode`] (HDR, SDR, split), [`Palette`] per side, persistence, peak, bloom, graticule.
//! - [`tonemap`]: the maths of decay and tone map, mirrored by the shaders.
//! - With the feature `host`: [`host`], a winit + wgpu + egui window on an `Rgba16Float` extended-sRGB surface
//!   when the platform offers one (macOS EDR), else an 8-bit one; [`headless`] renders frames to PNG files;
//!   the `ve_phosphor` demo binary.
//!
//! The target format decides the encoding the view writes: values past 1.0 only reach the screen on a float
//! surface in an extended color space; egui's own gamma path writes sRGB-encoded values, which is exactly what
//! `SurfaceColorSpace::ExtendedSrgb` expects, so egui needs no change for it ([`host`] has the details).

mod gpu;
mod settings;
pub mod tonemap;
mod widget;

#[cfg(feature = "host")]
pub mod demo;
#[cfg(feature = "host")]
pub mod headless;
#[cfg(feature = "host")]
pub mod host;
#[cfg(feature = "host")]
pub mod paint;

pub use gpu::{Frame, Output, Phosphor, Stats};
pub use settings::{Mode, Palette, Scene, Settings, Signal};
pub use widget::{PhosphorView, settings_ui};
