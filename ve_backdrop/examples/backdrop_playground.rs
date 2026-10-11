//! Playground for the animated backdrop (BACK-1): a slider for every setting, a switch between the shader
//! and the CPU painter, docked cards and floating windows that feed the backdrop their rectangles.
//!
//! Move the pointer over the grid (lens), click it (ripple), drag a window (it floats, light follows),
//! click a window (it becomes the focused one), switch the theme.
//!
//! Run: `cargo run -p ve_backdrop --example backdrop_playground`
//! (`-- --painter` starts on the CPU painter).

use eframe::egui;
use ve_backdrop::playground::Playground;
use ve_backdrop::{Backdrop, Renderer};

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1180.0, 760.0])
            .with_title("ve_backdrop playground"),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native(
        "ve_backdrop playground",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}

struct App {
    playground: Playground,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        ve_theme::setup(&cc.egui_ctx);
        // The shader when eframe runs on wgpu, the CPU painter otherwise.
        let mut backdrop = match &cc.wgpu_render_state {
            Some(render_state) => Backdrop::with_wgpu(render_state),
            None => Backdrop::new(),
        };
        if std::env::args().any(|a| a == "--painter") {
            backdrop.set_renderer(Renderer::Painter);
        }
        Self {
            playground: Playground::new(backdrop),
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.playground.ui(ui);
    }
}
