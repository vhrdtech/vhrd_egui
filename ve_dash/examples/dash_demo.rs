//! Dashboard blocks demo with synthetic data.
//!
//! A btop-style node dashboard built from the ve_dash blocks: stat tiles,
//! status lights, sparklines, meters and fed by a small
//! simulation; below it a task burndown and a Gantt chart of made-up releases
//! (wheel over the Gantt zooms, drag scrolls in time, a click names the row). The real mesh dashboard (live `tpm_mesh` data) lives in the
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
    Blocks, BurnSeries, Burndown, Day, Decay, Gantt, GanttBar, GanttLink, GanttMarker,
    GanttMilestone, GanttRow, GanttView, History, Ideal, Meter, Sparkline, StatTile, Status,
    StatusLight, SteadyColumn, Theme, Trend,
};
use ve_frame::{FrameState, WidgetFrame};

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 980.0])
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
    frame: FrameState,
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
            frame: FrameState::default(),
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
    /// How many blocks the load meters show; a combo box in the header switches it live.
    blocks: Blocks,
    tasks: Tasks,
}

/// Made-up task data for the burndown and the Gantt.
struct Tasks {
    today: Day,
    days: Vec<Day>,
    series: Vec<BurnSeries>,
    rows: Vec<GanttRow>,
    links: Vec<GanttLink>,
    markers: Vec<GanttMarker>,
    /// The Gantt's zoom and scroll; an app persists it.
    view: GanttView,
    /// The row clicked last, shown under the chart.
    clicked: Option<String>,
    burn_frame: FrameState,
    gantt_frame: FrameState,
}

impl Tasks {
    fn new(theme: &Theme) -> Self {
        let start = Day::from_ymd(2026, 9, 21);
        let days: Vec<Day> = (0..31).map(|i| start + i).collect();
        let today = start + 30;
        let wave = |i: usize, k: f32| ((i as f32 * k).sin() + 1.0) * 1.5;
        let mut fw = BurnSeries::new(
            "fw",
            theme.cat[0],
            (0..31)
                .map(|i| 46.0 - i as f32 * 1.1 + wave(i, 0.7))
                .collect(),
        );
        fw.closed = Some((0..31).map(|i| ((i * 7) % 5) as f32).collect());
        fw.ideas_open = Some((0..31).map(|i| 9.0 + (i % 3) as f32).collect());
        fw.ideal = Some(Ideal {
            from: start,
            value: 46.0,
            to: start + 45,
        });
        fw.trend = Some(Trend {
            per_day: 1.1,
            zero: Some(start + 44),
        });
        let mut hw = BurnSeries::new(
            "hw",
            theme.cat[1],
            (0..31).map(|i| 14.0 + wave(i, 0.3)).collect(),
        );
        hw.closed = Some((0..31).map(|i| ((i * 3) % 4) as f32).collect());
        hw.trend = Some(Trend {
            per_day: 0.0,
            zero: None,
        });
        let d = |m, dd| Day::from_ymd(2026, m, dd);
        let bar = |id: &str, s: Day, e: Day, color, soft_end| GanttBar {
            id: id.into(),
            start: s,
            end: e,
            color,
            soft_end,
            label: format!("release {id}"),
            ..Default::default()
        };
        let row = |id: &str, label: &str, group: &str, hover: &str| GanttRow {
            id: id.into(),
            label: label.into(),
            group: group.into(),
            hover: hover.into(),
            ..Default::default()
        };
        let mut rows = vec![
            row(
                "P1#v1",
                "v1 first light",
                "P1 sensor board",
                "released 30 Sep",
            ),
            row(
                "P1#v2",
                "v2 production",
                "P1 sensor board",
                "month-only date: Dec 2026",
            ),
            row(
                "P1#case",
                "case drawings",
                "P1 sensor board",
                "🚧 in progress",
            ),
            row("P2#app", "app 0.4", "P2 dashboard", "planned"),
            row("P2#web", "web build", "P2 dashboard", "waits for app 0.4"),
        ];
        rows[0]
            .bars
            .push(bar("v1", d(8, 15), d(9, 30), theme.cat[0], false));
        rows[1]
            .bars
            .push(bar("v2", d(10, 6), d(12, 31), theme.cat[0], true));
        rows[1].bars[0].latest_finish = Some(d(12, 20));
        rows[1].bars[0].critical = true;
        rows[1].milestones.push(GanttMilestone {
            id: "v2-due".into(),
            day: d(11, 13),
            color: theme.cat[3],
            label: "due: first batch".into(),
        });
        rows[2]
            .bars
            .push(bar("case", d(10, 12), today, theme.cat[2], false));
        rows[2].bars[0].latest_start = Some(d(10, 20));
        rows[3]
            .bars
            .push(bar("app", d(9, 25), d(11, 4), theme.cat[1], false));
        rows[4]
            .bars
            .push(bar("web", d(11, 10), d(11, 30), theme.cat[1], true));
        let links = vec![
            GanttLink {
                from: "v1".into(),
                to: "v2".into(),
            },
            GanttLink {
                from: "app".into(),
                to: "web".into(),
            },
            GanttLink {
                from: "v2".into(),
                to: "v2-ship".into(),
            },
        ];
        let markers = vec![GanttMarker {
            id: "v2-ship".into(),
            day: d(12, 20),
            at_risk: true,
            label: "milestone: v2 ship 20 Dec".into(),
        }];
        Self {
            today,
            days,
            series: vec![fw, hw],
            rows,
            links,
            markers,
            view: GanttView::default(),
            clicked: None,
            burn_frame: FrameState::default(),
            gantt_frame: FrameState::default(),
        }
    }

    fn ui(&mut self, ui: &mut Ui, theme: &Theme) {
        let Self {
            today,
            days,
            series,
            rows,
            links,
            markers,
            view,
            clicked,
            burn_frame,
            gantt_frame,
        } = self;
        WidgetFrame::new("burndown").show(ui, burn_frame, |ui, _| {
            let _ = Burndown::new(days, series)
                .theme(theme.clone())
                .today(*today)
                .height(200.0)
                .show(ui);
        });
        ui.add_space(8.0);
        WidgetFrame::new("gantt").show(ui, gantt_frame, |ui, _| {
            let r = Gantt::new(rows, view)
                .links(links)
                .markers(markers)
                .today(*today)
                .theme(theme.clone())
                .show(ui);
            if r.clicked.is_some() {
                *clicked = r.clicked;
            }
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(match clicked {
                        Some(id) => format!("clicked: {id}"),
                        None => "click a row".into(),
                    })
                    .color(theme.text_muted)
                    .size(11.0),
                );
                if ui
                    .small_button("fit")
                    .on_hover_text("Show all the bars again (the zoom and scroll are reset)")
                    .clicked()
                {
                    view.reset();
                }
            });
        });
    }
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
        let tasks = Tasks::new(&theme);
        Self {
            theme,
            nodes,
            fps: 0.0,
            blocks: Blocks::default(),
            tasks,
        }
    }

    fn header(&mut self, ui: &mut Ui) {
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
            ui.label(RichText::new("load meter blocks").color(self.theme.text_muted).size(11.0));
            egui::ComboBox::from_id_salt("blocks")
                .selected_text(self.blocks.label())
                .show_ui(ui, |ui| {
                    for blocks in Blocks::ALL {
                        ui.selectable_value(&mut self.blocks, blocks, blocks.label());
                    }
                })
                .response
                .on_hover_text(
                    "How many blocks every node's link-load meter shows; Auto grows with the panel's width",
                );
        });
    }

    fn node_panel(&mut self, ui: &mut Ui, idx: usize) {
        let theme = self.theme.clone();
        let blocks = self.blocks;
        let node = &mut self.nodes[idx];
        let title = if node.is_self {
            format!("{} (this pc)", node.name)
        } else {
            node.name.to_owned()
        };
        let frame = &mut node.frame;
        WidgetFrame::new(title).show(ui, frame, |ui, _| {
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
                    .every(std::time::Duration::from_millis(500))
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

            // Colour along the line by value (`color_by`): green low, red high.
            ui.label(
                RichText::new("rtt by value")
                    .color(theme.text_muted)
                    .size(11.0),
            );
            ui.add(
                Sparkline::new(node.rtt.values())
                    .height(28.0)
                    .color_by(|v| ve_dash::heat(&theme, v / 100.0)),
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
            ui.add(
                Meter::new(load)
                    .blocks(blocks)
                    .text(format!("{:.0} %", load * 100.0)),
            );
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
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.header(ui);
                    ui.add_space(10.0);
                    let cols = self.nodes.len().clamp(1, 3);
                    ui.columns(cols, |columns| {
                        for idx in 0..self.nodes.len() {
                            self.node_panel(&mut columns[idx % cols], idx);
                        }
                    });
                    ui.add_space(10.0);
                    let theme = self.theme.clone();
                    self.tasks.ui(ui, &theme);
                });
            });
    }
}
