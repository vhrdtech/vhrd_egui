//! The ve_app shell with two demo widgets. `cargo run -p ve_app --example shell_demo`

pub mod widgets;

use ve_app::{Shell, ShellOptions};
use ve_widget::Widget;
use ve_widget::context::Context;

/// The demo's options; `tests/ui.rs` builds the same shell.
pub fn options() -> ShellOptions {
    ShellOptions::new("Shell demo", ve_basics::build_info!())
        .description("The vhrd_egui application shell with two demo widgets.")
        .default_widgets(|| -> Vec<Box<dyn Widget>> {
            vec![
                Box::new(widgets::Note::default()),
                Box::new(widgets::Level::default()),
            ]
        })
        .side_panel(|ui, _cx| {
            ui.heading("Side panel");
            ui.label("Apps put navigation or a device list here.");
        })
        .settings(|ui, _cx| {
            ui.label("App settings go here.");
        })
}

#[allow(dead_code)] // unused when included by the tests
fn main() -> eframe::Result {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 500.0])
            .with_min_inner_size([300.0, 220.0]),
        ..Default::default()
    };
    eframe::run_native(
        "ve_app shell demo",
        native_options,
        Box::new(|cc| {
            let cx = Context::new(Box::new(()));
            Ok(Box::new(Shell::new(cc, cx, options())))
        }),
    )
}
