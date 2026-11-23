//! A paint callback made once and handed to egui every frame.

use std::any::Any;
use std::sync::Arc;

use egui_wgpu::CallbackTrait;

/// A paint callback built once: [`RetainedCallback::shape`] hands egui the same callback every frame, which
/// costs a reference count instead of the two allocations of `egui_wgpu::Callback::new_paint_callback`.
///
/// What changes per frame reaches the callback through state it shares with its owner (an `Arc` of a mutex),
/// not through a new callback.
#[derive(Clone)]
pub struct RetainedCallback(Arc<dyn Any + Send + Sync>);

impl RetainedCallback {
    /// The callback that runs `callback`'s prepare, finish and paint steps.
    pub fn new(callback: impl CallbackTrait + 'static) -> Self {
        Self(egui_wgpu::Callback::new_paint_callback(egui::Rect::ZERO, callback).callback)
    }

    /// The shape that paints this callback over `rect` (points), within the painter's clip.
    pub fn shape(&self, rect: egui::Rect) -> egui::Shape {
        egui::Shape::Callback(egui::PaintCallback {
            rect,
            callback: self.0.clone(),
        })
    }
}

impl std::fmt::Debug for RetainedCallback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RetainedCallback")
    }
}
