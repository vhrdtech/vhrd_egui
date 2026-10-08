//! Dashboard blocks demo with synthetic data.
//!
//! A btop-style node dashboard built from the ve_dash blocks: stat tiles,
//! status lights, sparklines, meters and titled panels, fed by a small
//! simulation. The real mesh dashboard (live `tpm_mesh` data) lives in the
//! tpm_mesh_dash repo; this demo stays here so the crate shows off and
//! exercises every block without external dependencies.
//!
//! Renders through wgpu and repaints continuously: vsync paces the loop at the
//! display's refresh rate, the charts scroll one sample per frame.
//!
//! Run: `cargo run -p ve_dash --example dash_demo`

use eframe::egui;
use egui::{Align2, FontId, RichText, Ui, vec2};
use ve_dash::{
    Decay, History, Meter, Sparkline, StatTile, Status, StatusLight, SteadyColumn, Theme, panel,
};

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 520.0])
            .with_title("ve_dash demo"),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native(
        "ve_dash demo",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}

struct Node {
    name: &'static str,
    online: bool,
    is_self: bool,
    rtt: History,
    traffic: History,
    /// Link load with analog-needle smoothing: the raw per-frame value is
    /// too jittery for the eye, the decayed one reads like a VU meter.
    load: Decay,
    sim_phase: f32,
    sim_rng: u32,
    sim_spike: f32,
}

impl Node {
    fn new(name: &'static str, seed: u32) -> Self {
        Self {
            name,
            online: true,
            is_self: false,
            rtt: History::new(360),
            traffic: History::new(360),
            load: Decay::new(),
            sim_phase: seed as f32 * 1.7,
            sim_rng: seed.max(1),
            sim_spike: 0.0,
        }
    }

    fn noise(&mut self) -> f32 {
        // xorshift32, mapped to 0..1
        let mut x = self.sim_rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.sim_rng = x;
        (x as f32) / (u32::MAX as f32)
    }

    fn sim_step(&mut self, dt: f32) {
        self.sim_phase += dt;
        if !self.online {
            self.rtt.push(0.0);
            self.traffic.push(0.0);
            self.load.update(0.0, dt);
            return;
        }
        // Rare RTT spikes that decay over a few frames, so they stay visible
        // when a sample is pushed every frame (~60 Hz).
        if self.noise() > 1.0 - dt * 0.3 {
            self.sim_spike = 20.0 + self.noise() * 15.0;
        }
        self.sim_spike *= (-dt * 6.0).exp();
        let base = if self.is_self { 0.1 } else { 4.0 };
        let rtt =
            base + (self.sim_phase * 0.6).sin().abs() * 2.0 + self.noise() * 0.8 + self.sim_spike;
        self.rtt.push(rtt);
        let traffic = ((self.sim_phase * 0.3).sin() * 0.5 + 0.5) * 40.0 + self.noise() * 8.0;
        self.traffic.push(traffic);
        self.load.update((traffic / 60.0).clamp(0.0, 1.0), dt);
    }
}

struct App {
    theme: Theme,
    nodes: Vec<Node>,
    /// Exponentially smoothed frames per second, for the header tile.
    fps: f32,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let theme = Theme::dark();
        theme.apply(&cc.egui_ctx);
        ve_theme::install_fonts(&cc.egui_ctx);
        let mut nodes = vec![
            Node::new("demo-pc", 0x9e37),
            Node::new("node-two", 0x9e38),
            Node::new("node-three", 0x9e39),
        ];
        nodes[0].is_self = true;
        nodes[2].online = false;
        Self {
            theme,
            nodes,
            fps: 0.0,
        }
    }

    fn header(&self, ui: &mut Ui) {
        let online = self.nodes.iter().filter(|n| n.online).count();
        let total = self.nodes.len();
        let all_up = online == total && total > 0;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 28.0;
            // Brand wordmark: red, used sparingly — this is the one place.
            ui.label(
                RichText::new("ve_dash")
                    .color(self.theme.red)
                    .size(20.0)
                    .strong(),
            );
            ui.add(
                StatTile::new("nodes online", format!("{online}/{total}")).value_color(if all_up {
                    self.theme.good
                } else {
                    self.theme.warn
                }),
            );
            let worst_rtt = self
                .nodes
                .iter()
                .filter(|n| n.online && !n.is_self)
                .filter_map(|n| n.rtt.last())
                .fold(0.0f32, f32::max);
            ui.add(
                StatTile::new("worst rtt", format!("{worst_rtt:.1}"))
                    .unit("ms")
                    .steady(),
            );
            ui.add(StatTile::new("data", "synthetic").min_width(0.0));
            ui.add(
                StatTile::new("fps", format!("{:.0}", self.fps))
                    .min_width(0.0)
                    .steady(),
            );
        });
    }

    fn node_panel(&mut self, ui: &mut Ui, idx: usize) {
        let theme = self.theme.clone();
        let node = &mut self.nodes[idx];
        let title = if node.is_self {
            format!("{} (this pc)", node.name)
        } else {
            node.name.to_owned()
        };
        panel(ui, &theme, &title, |ui| {
            let (status, label) = match (node.online, node.is_self) {
                (_, true) => (Status::Good, "local"),
                (true, false) => (Status::Good, "connected"),
                (false, false) => (Status::Off, "offline"),
            };
            ui.add(StatusLight::new(status, label));
            // Steady column + info icon: the fields of every node panel line up whatever
            // the name's length; the full name is on hover when it is cut.
            let fields = SteadyColumn::new(ui, "fields").max_width(ui.available_width() * 0.5);
            ui.horizontal(|ui| {
                fields.label(
                    ui,
                    RichText::new(format!("{} · demo node", node.name))
                        .size(11.0)
                        .color(theme.text_muted),
                );
                ve_dash::info_icon(ui, &theme, 12.0)
                    .on_hover_text("synthetic data: nothing here is measured");
            });
            ui.add_space(6.0);

            ui.label(RichText::new("rtt").color(theme.text_muted).size(11.0));
            let rtt_now = node.rtt.last().unwrap_or(0.0);
            ui.add(
                Sparkline::new(node.rtt.values())
                    .height(42.0)
                    .color(theme.accent),
            );
            ui.painter().text(
                ui.min_rect().right_top() + vec2(0.0, 24.0),
                Align2::RIGHT_TOP,
                if node.online {
                    format!("{rtt_now:.1} ms")
                } else {
                    "—".into()
                },
                FontId::monospace(11.0),
                theme.text,
            );
            ui.add_space(6.0);

            ui.label(RichText::new("events").color(theme.text_muted).size(11.0));
            ui.add(
                Sparkline::new(node.traffic.values())
                    .height(28.0)
                    .color(theme.accent_alt)
                    .range(0.0..=60.0),
            );
            ui.add_space(6.0);

            ui.label(
                RichText::new("link load")
                    .color(theme.text_muted)
                    .size(11.0),
            );
            let load = node.load.value();
            ui.add(Meter::new(load).text(format!("{:.0} %", load * 100.0)));
        });
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // One sample per frame: with continuous repaint and vsync the charts
        // scroll at the display's refresh rate.
        let dt = ctx.input(|i| i.stable_dt).clamp(1e-4, 0.1);
        for node in &mut self.nodes {
            node.sim_step(dt);
        }
        self.fps = if self.fps == 0.0 {
            1.0 / dt
        } else {
            self.fps * 0.95 + 0.05 / dt
        };
        ctx.request_repaint();

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(self.theme.bg).inner_margin(12))
            .show(ui, |ui| {
                self.header(ui);
                ui.add_space(10.0);
                let cols = self.nodes.len().clamp(1, 3);
                ui.columns(cols, |columns| {
                    for idx in 0..self.nodes.len() {
                        self.node_panel(&mut columns[idx % cols], idx);
                    }
                });
            });
    }
}
