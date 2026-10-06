//! Mesh node dashboard prototype (tpm_mesh, P2620).
//!
//! Node names and who-is-connected come from the real daemon
//! (`tpm_mesh status --json`, polled every 3 s); RTT and traffic are
//! simulated until the daemon reports them.
//!
//! Renders through wgpu and repaints continuously: vsync paces the loop at the
//! display's refresh rate, the charts scroll one sample per frame.
//!
//! Run: `cargo run -p ve_dash --example mesh_dash`

use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;

use eframe::egui;
use egui::{Align2, FontId, RichText, Ui, vec2};
use ve_dash::{History, Meter, Sparkline, StatTile, Status, StatusLight, Theme, panel};

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 520.0])
            .with_title("tpm mesh"),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native(
        "tpm mesh dashboard",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}

/// What `tpm_mesh status --json` reports.
#[derive(Clone, Debug, Default)]
struct MeshStatus {
    self_name: String,
    version: String,
    nodes: Vec<String>,
    neighbors: Vec<String>,
}

fn fetch_status() -> Option<MeshStatus> {
    let out = Command::new("tpm_mesh")
        .args(["status", "--json"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    let strings = |key: &str| -> Vec<String> {
        v[key]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|s| s.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    };
    Some(MeshStatus {
        self_name: v["name"].as_str().unwrap_or("?").to_owned(),
        version: v["version"].as_str().unwrap_or("?").to_owned(),
        nodes: strings("nodes"),
        neighbors: strings("neighbors"),
    })
}

/// Poll the daemon in the background so the UI thread never blocks.
fn spawn_status_poller(ctx: egui::Context) -> mpsc::Receiver<Option<MeshStatus>> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        loop {
            if tx.send(fetch_status()).is_err() {
                return; // UI gone
            }
            ctx.request_repaint();
            std::thread::sleep(Duration::from_secs(3));
        }
    });
    rx
}

struct Node {
    name: String,
    /// Connected to us right now (always true for ourselves).
    online: bool,
    is_self: bool,
    rtt: History,
    traffic: History,
    load: f32,
    // Simulation state until the daemon reports real numbers.
    sim_phase: f32,
    sim_rng: u32,
    sim_spike: f32,
}

impl Node {
    fn new(name: &str, seed: u32) -> Self {
        Self {
            name: name.to_owned(),
            online: true,
            is_self: false,
            rtt: History::new(360),
            traffic: History::new(360),
            load: 0.0,
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
            self.load = 0.0;
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
        self.load = (traffic / 60.0).clamp(0.0, 1.0);
    }
}

struct App {
    theme: Theme,
    status: Option<MeshStatus>,
    status_rx: mpsc::Receiver<Option<MeshStatus>>,
    nodes: Vec<Node>,
    /// Exponentially smoothed frames per second, for the header tile.
    fps: f32,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let theme = Theme::dark();
        theme.apply(&cc.egui_ctx);
        Self {
            theme,
            status: None,
            status_rx: spawn_status_poller(cc.egui_ctx.clone()),
            nodes: Vec::new(),
            fps: 0.0,
        }
    }

    fn apply_status(&mut self, st: MeshStatus) {
        for (i, name) in st.nodes.iter().enumerate() {
            if !self.nodes.iter().any(|n| &n.name == name) {
                self.nodes.push(Node::new(name, 0x9e37 + i as u32));
            }
        }
        for node in &mut self.nodes {
            node.is_self = node.name == st.self_name;
            node.online = node.is_self || st.neighbors.contains(&node.name);
        }
        self.nodes
            .sort_by(|a, b| (!a.is_self, &a.name).cmp(&(!b.is_self, &b.name)));
        self.status = Some(st);
    }

    fn demo_fallback(&mut self) {
        self.apply_status(MeshStatus {
            self_name: "demo-pc".into(),
            version: "offline demo".into(),
            nodes: ["demo-pc", "gpd-omarchy", "mail-server"]
                .map(String::from)
                .to_vec(),
            neighbors: vec!["gpd-omarchy".into()],
        });
    }

    fn header(&self, ui: &mut Ui) {
        let online = self.nodes.iter().filter(|n| n.online).count();
        let total = self.nodes.len();
        let all_up = online == total && total > 0;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 28.0;
            // Brand wordmark: red, used sparingly — this is the one place.
            ui.label(
                RichText::new("tpm mesh")
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
            ui.add(StatTile::new("worst rtt", format!("{worst_rtt:.1}")).unit("ms"));
            if let Some(st) = &self.status {
                ui.add(StatTile::new("this pc", st.self_name.clone()));
                let short_ver = st.version.split(' ').next().unwrap_or("?");
                ui.add(StatTile::new("daemon", short_ver).min_width(0.0));
            }
            ui.add(StatTile::new("fps", format!("{:.0}", self.fps)).min_width(0.0));
        });
    }

    fn node_panel(&mut self, ui: &mut Ui, idx: usize) {
        let theme = self.theme.clone();
        let node = &mut self.nodes[idx];
        let title = if node.is_self {
            format!("{} (this pc)", node.name)
        } else {
            node.name.clone()
        };
        panel(ui, &theme, &title, |ui| {
            let (status, label) = match (node.online, node.is_self) {
                (_, true) => (Status::Good, "local"),
                (true, false) => (Status::Good, "connected"),
                (false, false) => (Status::Off, "offline"),
            };
            ui.add(StatusLight::new(status, label));
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
            ui.add(Meter::new(node.load).text(format!("{:.0} %", node.load * 100.0)));
        });
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        while let Ok(st) = self.status_rx.try_recv() {
            match st {
                Some(st) => self.apply_status(st),
                None if self.status.is_none() => self.demo_fallback(),
                None => {} // daemon went away; keep showing the last state
            }
        }

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
                if self.nodes.is_empty() {
                    ui.label(
                        RichText::new("waiting for tpm_mesh status…").color(self.theme.text_muted),
                    );
                    return;
                }
                let cols = self.nodes.len().clamp(1, 3);
                ui.columns(cols, |columns| {
                    for idx in 0..self.nodes.len() {
                        self.node_panel(&mut columns[idx % cols], idx);
                    }
                });
            });
    }
}
