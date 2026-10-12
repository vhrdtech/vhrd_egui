//! The ve_settle sandbox: real `ve_frame` widgets on a physics layout, on
//! the `ve_backdrop` grid, a slider on every parameter, a debug overlay,
//! drag, add and remove, pause and single-step (SETL-2, SETL-7).
//!
//! `cargo run -p ve_settle --example sandbox` starts with rectangles;
//! `cargo run -p ve_settle --example sandbox -- hexagons` with hexagons.

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
        Box::new(|cc| {
            let sandbox = if std::env::args().any(|arg| arg == "hexagons") {
                app::Sandbox::with_scene(cc, app::Scene::Hexagons)
            } else {
                app::Sandbox::new(cc)
            };
            Ok(Box::new(sandbox))
        }),
    )
}
