//! The playground as a piece of UI, so the example and the tests share it: a slider for every setting, a
//! switch between the shader and the CPU painter, docked cards and floating windows that feed the backdrop
//! their rectangles. Run it: `cargo run -p ve_backdrop --example backdrop_playground`.

use egui::{CentralPanel, Frame, Id, Order, Panel, RichText, ScrollArea, Ui, Window};
use ve_theme::UiExt as _;

use crate::{Backdrop, Lit, Renderer};

/// The playground: a backdrop, its controls and some frames on top of it.
pub struct Playground {
    /// The backdrop being played with.
    pub backdrop: Backdrop,
    /// Floating windows to show (0-3).
    pub windows: usize,
}

const WINDOWS: [(&str, &str, [f32; 2]); 3] = [
    (
        "Scope",
        "Drag me: a dragged window floats and gathers more light.",
        [380.0, 300.0],
    ),
    (
        "Log",
        "Click me: the window on top is the focused one.",
        [700.0, 160.0],
    ),
    (
        "Notes",
        "Close me with the slider on the left: the light fades out.",
        [760.0, 440.0],
    ),
];

impl Playground {
    /// A playground around `backdrop`.
    pub fn new(backdrop: Backdrop) -> Self {
        Self {
            backdrop,
            windows: WINDOWS.len(),
        }
    }

    /// The whole playground: controls on the left, the backdrop with its frames in the rest of `ui`.
    pub fn ui(&mut self, ui: &mut Ui) {
        Panel::left("backdrop_controls")
            .min_size(340.0)
            .show(ui, |ui| {
                ScrollArea::vertical().show(ui, |ui| self.controls(ui));
            });
        CentralPanel::default().frame(Frame::NONE).show(ui, |ui| {
            // First in the panel so everything else is on top; filled once the frames are laid out.
            let slot = self.backdrop.reserve(ui.painter());
            let area = ui.max_rect();
            let mut lit = Vec::new();
            let margin = ui.tokens().space.xl;
            Frame::NONE.inner_margin(margin).show(ui, |ui| {
                self.docked(ui, &mut lit);
            });
            self.floating(ui.ctx(), &mut lit);
            self.backdrop.fill(ui.painter(), slot, area, &lit);
        });
    }

    fn controls(&mut self, ui: &mut Ui) {
        ui.section_header("Playground", "how the backdrop is drawn and what is on it");
        ui.horizontal(|ui| {
            ui.label("Renderer")
                .on_hover_text("what draws the backdrop; both show the same picture");
            let mut renderer = self.backdrop.renderer();
            ui.add_enabled_ui(self.backdrop.has_shader(), |ui| {
                ui.selectable_value(&mut renderer, Renderer::Shader, "Shader")
                    .on_hover_text("the wgpu shader: one paint callback, the grid is computed per pixel")
                    .on_disabled_hover_text("not available: this window does not render through wgpu");
            });
            ui.selectable_value(&mut renderer, Renderer::Painter, "CPU painter")
                .on_hover_text(
                    "plain egui shapes, one per mark: the fallback for backends without wgpu (glow, wasm)",
                );
            self.backdrop.set_renderer(renderer);
        });
        ui.horizontal(|ui| {
            ui.label("Theme")
                .on_hover_text("the backdrop takes its colours from the theme's tokens");
            egui::global_theme_preference_buttons(ui);
        });
        ui.add(egui::Slider::new(&mut self.windows, 0..=WINDOWS.len()).text("Windows"))
            .on_hover_text(
                "how many floating windows are open; closing one lets its light fade out",
            );

        self.backdrop.settings.ui(ui);

        ui.section_header(
            "Glass status",
            "what the emulated transparency is doing: which wallpaper it shows, or why none",
        );
        let status = self.backdrop.glass_status(ui.ctx());
        ui.add_enabled_ui(self.backdrop.settings.glass, |ui| {
            ui.muted_label(status.text());
            if ui
                .button("Look again")
                .on_hover_text("look for the desktop wallpaper again and reload it (it changed)")
                .clicked()
            {
                self.backdrop.reload_wallpaper();
            }
        })
        .response
        .on_disabled_hover_text("off: turn on Emulated transparency under Glass");
    }

    /// The floating windows; returns what the backdrop should react to.
    fn floating(&self, ctx: &egui::Context, lit: &mut Vec<Lit>) {
        let tokens = ve_theme::Tokens::of(ctx, ctx.theme());
        let top = ctx.memory(|m| m.areas().top_layer_id(Order::Middle));
        for (title, text, pos) in WINDOWS.iter().take(self.windows) {
            let frame =
                Frame::window(&ctx.global_style()).fill(tokens.colors.surface.gamma_multiply(0.82));
            let shown = Window::new(*title)
                .default_pos(*pos)
                .default_width(220.0)
                .collapsible(false)
                .frame(frame)
                .show(ctx, |ui| {
                    ui.label(*text);
                });
            if let Some(shown) = shown {
                let r = shown.response;
                lit.push(
                    Lit::new(r.rect)
                        .corner_radius(f32::from(tokens.radius.m))
                        .focused(top == Some(r.layer_id))
                        .floating(r.dragged())
                        .id(Id::new(title)),
                );
            }
        }
    }

    /// Two cards laid out in the central panel.
    fn docked(&self, ui: &mut Ui, lit: &mut Vec<Lit>) {
        let tokens = ui.tokens();
        let cards = [
            (
                "Docked card",
                "A plain frame: light gathers under and around it.",
            ),
            (
                "Another one",
                "Frames close to each other pool their light.",
            ),
        ];
        ui.horizontal_top(|ui| {
            for (title, text) in cards {
                let card = Frame::new()
                    .fill(tokens.colors.surface.gamma_multiply(0.82))
                    .stroke((tokens.stroke.thin, tokens.colors.line))
                    .corner_radius(tokens.radius.m)
                    .inner_margin(tokens.space.l)
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                            ui.set_max_width(180.0);
                            ui.label(RichText::new(title).strong());
                            ui.label(text);
                        });
                    });
                lit.push(
                    Lit::new(card.response.rect)
                        .corner_radius(f32::from(tokens.radius.m))
                        .id(Id::new(title)),
                );
                ui.add_space(tokens.space.xl);
            }
        });
    }
}
