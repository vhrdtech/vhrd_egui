//! The ve_settle sandbox: widgets of different sizes on a physics layout,
//! a slider on every parameter, a debug overlay, drag, resize, add and
//! remove, pause and single-step (SETL-2).
//!
//! `cargo run -p ve_settle --example sandbox`

mod app;

use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1600.0, 1000.0])
            .with_title("ve_settle sandbox"),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native(
        "ve_settle sandbox",
        options,
        Box::new(|cc| Ok(Box::new(app::Sandbox::new(cc)))),
    )
}
