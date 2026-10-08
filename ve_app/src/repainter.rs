//! Waking the UI from background work.

use std::time::Duration;

/// A handle that requests a repaint of the app; clone it into background tasks (threads, tokio tasks)
/// that change what the UI shows. Cheap to clone, `Send + Sync`, needs no runtime.
///
/// Replaces eframe_template's `UiRepainter`, which forwarded through a tokio channel: `egui::Context` is
/// already thread-safe.
#[derive(Clone)]
pub struct Repainter {
    ctx: egui::Context,
}

impl Repainter {
    /// A repainter for `ctx`.
    pub fn new(ctx: egui::Context) -> Self {
        Self { ctx }
    }

    /// Repaint as soon as possible.
    pub fn repaint(&self) {
        self.ctx.request_repaint();
    }

    /// Repaint within `after`, for polling at a steady rate without a busy loop.
    pub fn repaint_after(&self, after: Duration) {
        self.ctx.request_repaint_after(after);
    }
}
