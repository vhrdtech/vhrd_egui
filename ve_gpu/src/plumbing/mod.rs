//! Paint-callback plumbing any wgpu widget needs (GPU-1): the pass egui draws into ([`Target`]), the screen
//! uniform every shader shares ([`ScreenUniform`], [`COMMON_WGSL`]), buffers that grow and are reused
//! ([`GrowBuffer`]), and a paint callback built once and handed to egui every frame without allocating
//! ([`RetainedCallback`]).

mod grow;
mod retained;
mod screen;
mod target;

pub use grow::GrowBuffer;
pub use retained::RetainedCallback;
pub use screen::{COMMON_WGSL, ScreenUniform};
pub use target::Target;
