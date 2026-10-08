//! The ve_theme gallery: every egui widget and every `UiExt` helper, dark and light side by side.
//! The tool for tuning tokens (THM-5).
//!
//! Run: `cargo run -p ve_theme --example gallery`

use eframe::egui;
use ve_theme::gallery::Gallery;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 820.0])
            .with_title("ve_theme gallery"),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native(
        "ve_theme gallery",
        options,
        Box::new(|cc| {
            ve_theme::setup(&cc.egui_ctx);
            Ok(Box::new(App::default()))
        }),
    )
}

#[derive(Default)]
struct App {
    gallery: Gallery,
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::ScrollArea::vertical().show(ui, |ui| self.gallery.side_by_side(ui));
    }
}
