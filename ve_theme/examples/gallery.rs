//! The ve_theme gallery: every egui widget and every `UiExt` helper, dark and light side by side, and a second
//! page with the property-panel widgets. The tool for tuning tokens (THM-5, THM-17).
//!
//! Run: `cargo run -p ve_theme --example gallery`

use eframe::egui;
use ve_theme::gallery::{Gallery, PropsGallery};

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 860.0])
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

#[derive(Default, PartialEq)]
enum Page {
    #[default]
    Widgets,
    Props,
}

#[derive(Default)]
struct App {
    page: Page,
    gallery: Gallery,
    props: PropsGallery,
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.page, Page::Widgets, "Widgets")
                .on_hover_text("Every egui widget and UiExt helper");
            ui.selectable_value(&mut self.page, Page::Props, "Property panel")
                .on_hover_text(
                    "Pane, sub-pane, group, property rows, number and axis fields, list (THM-17)",
                );
        });
        egui::ScrollArea::vertical().show(ui, |ui| match self.page {
            Page::Widgets => self.gallery.side_by_side(ui),
            Page::Props => self.props.side_by_side(ui),
        });
    }
}
