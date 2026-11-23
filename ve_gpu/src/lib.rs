//! wgpu building blocks for egui widgets that draw more than egui's shapes carry cheaply.
//!
//! - [`Batch`]: shapes and range columns drawn through one egui paint callback, one instance each, antialiased
//!   in the shader: rects with corners and an inner stroke, segments with dashes, diamonds, hatching
//!   ([`RectStyle`], [`LineStyle`], [`Hatch`]), and runs of [`Column`]s (a value's lowest, highest and mean per
//!   pixel column, mapped by [`ColumnStyle`]). A warm frame allocates nothing on the CPU and writes each GPU buffer
//!   once for all batches.
//! - [`plumbing`]: what any paint callback needs: [`Target`] (the pass egui draws into), [`ScreenUniform`] and
//!   [`COMMON_WGSL`] (points to pixels, egui's colours and rounding), [`GrowBuffer`], [`RetainedCallback`].
//! - [`headless`] (native only): a device without a window, egui frames rendered offscreen, read back and timed.
//!
//! Shaders stay within wgpu's downlevel WebGL2 limits: instanced vertex attributes and uniforms, no storage
//! buffers, so the GL backend, the software adapter and the web all draw them.
//!
//! Once, with the app's render state (eframe: `cc.wgpu_render_state`): [`install`]. Then each frame, per widget:
//! fill a [`Batch`] and add [`Batch::paint`]'s shape to the painter. Texts and the rest stay on egui's painter:
//! what is added after the batch draws over it.

mod batch;
#[cfg(not(target_arch = "wasm32"))]
pub mod headless;
pub mod plumbing;
mod renderer;

pub use batch::Batch;
pub use batch::columns::{Column, ColumnStyle};
pub use batch::shapes::{Dash, Hatch, LineStyle, RectStyle};
pub use plumbing::{COMMON_WGSL, GrowBuffer, RetainedCallback, ScreenUniform, Target};
pub use renderer::FrameStats;

use egui_wgpu::RenderState;
use renderer::Renderer;

/// Sets up ve_gpu's pipelines in `render_state`'s egui renderer, for eframe's default pass (no multisampling, no
/// depth). Again with the same target does nothing.
pub fn install(render_state: &RenderState) {
    install_with(render_state, Target::new(render_state.target_format));
}

/// [`install`] for a pass of `target`: give the multisampling and depth format the app gives eframe.
pub fn install_with(render_state: &RenderState, target: Target) {
    let mut renderer = render_state.renderer.write();
    install_in(&mut renderer, &render_state.device, target);
}

/// [`install`] into an egui renderer of one's own.
pub fn install_in(
    renderer: &mut egui_wgpu::Renderer,
    device: &egui_wgpu::wgpu::Device,
    target: Target,
) {
    let resources = &mut renderer.callback_resources;
    if resources
        .get::<Renderer>()
        .is_some_and(|r| r.target() == target)
    {
        return;
    }
    resources.insert(Renderer::new(device, target));
}

/// Whether [`install`] was done on `render_state`.
pub fn is_installed(render_state: &RenderState) -> bool {
    render_state
        .renderer
        .read()
        .callback_resources
        .get::<Renderer>()
        .is_some()
}

/// What the batches of the last frame drew; `None` before [`install`].
pub fn frame_stats(render_state: &RenderState) -> Option<FrameStats> {
    render_state
        .renderer
        .read()
        .callback_resources
        .get::<Renderer>()
        .map(Renderer::last_stats)
}
