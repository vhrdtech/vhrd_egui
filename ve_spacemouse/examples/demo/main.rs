//! The SpaceMouse demo: a canvas of dots panned and zoomed by the puck, the pie menu on its buttons, the
//! mapping as settings and the calibrate step.
//!
//! ```sh
//! cargo run -p ve_spacemouse --example demo                              # spacenavd's socket
//! cargo run -p ve_spacemouse --example demo -- --socket /tmp/spnav.sock  # another one, e.g. a fake
//! ```
//!
//! The mapping is saved with the window state (eframe persistence), so a calibration is kept between runs.

mod menu;

use std::path::PathBuf;
use std::time::Duration;

use egui::{Pos2, Sense, Ui, Vec2, pos2, vec2};
use menu::{Action, Can};
use ve_spacemouse::{
    Calibration, CalibrationAction, Mapping, Options, PieState, SpaceMouse, Status, buttons,
    calibration_ui, mapping_ui,
};
use ve_theme::UiExt;

const MAPPING_KEY: &str = "ve_spacemouse_mapping";

fn main() -> eframe::Result {
    let mut socket = PathBuf::from(ve_spacemouse::DEFAULT_SOCKET);
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--socket" => match args.next() {
                Some(p) => socket = PathBuf::from(p),
                None => {
                    eprintln!("--socket needs a path");
                    std::process::exit(2);
                }
            },
            "-h" | "--help" => {
                println!(
                    "SpaceMouse demo: pan and zoom a canvas with the puck, pie menus on its buttons.\n"
                );
                println!(
                    "Usage: demo [--socket PATH]   (default {})",
                    ve_spacemouse::DEFAULT_SOCKET
                );
                return Ok(());
            }
            other => {
                eprintln!("unknown argument {other:?}; see --help");
                std::process::exit(2);
            }
        }
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_title("SpaceMouse demo"),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native(
        "ve_spacemouse demo",
        options,
        Box::new(move |cc| {
            ve_theme::setup(&cc.egui_ctx);
            ve_spacemouse::install_icons(&cc.egui_ctx);
            let mapping = cc
                .storage
                .and_then(|s| eframe::get_value::<Mapping>(s, MAPPING_KEY))
                .unwrap_or_default();
            let mouse = SpaceMouse::start(Options {
                socket,
                ..Default::default()
            })
            .with_repaint(&cc.egui_ctx);
            Ok(Box::new(Demo::new(mouse, mapping)))
        }),
    )
}

struct Demo {
    mouse: SpaceMouse,
    mapping: Mapping,
    pie: PieState,
    calibration: Option<Calibration>,
    /// Canvas: points in canvas units, the view's offset and scale.
    dots: Vec<Pos2>,
    offset: Vec2,
    scale: f32,
    copied: Option<Pos2>,
    undo: Vec<Vec<Pos2>>,
    redo: Vec<Vec<Pos2>>,
    dirty: bool,
    snap: bool,
    grid: bool,
    log: Vec<String>,
}

impl Demo {
    fn new(mouse: SpaceMouse, mapping: Mapping) -> Self {
        let dots = (0..12)
            .map(|i| {
                let a = i as f32 * 0.52;
                pos2(
                    a.cos() * (80.0 + 12.0 * i as f32),
                    a.sin() * (80.0 + 12.0 * i as f32),
                )
            })
            .collect();
        Self {
            mouse,
            mapping,
            pie: PieState::default(),
            calibration: None,
            dots,
            offset: Vec2::ZERO,
            scale: 1.0,
            copied: None,
            undo: Vec::new(),
            redo: Vec::new(),
            dirty: false,
            snap: false,
            grid: true,
            log: Vec::new(),
        }
    }

    fn to_canvas(&self, center: Pos2, screen: Pos2) -> Pos2 {
        pos2(
            (screen.x - center.x - self.offset.x) / self.scale,
            (screen.y - center.y - self.offset.y) / self.scale,
        )
    }

    fn nearest(&self, p: Pos2) -> Option<usize> {
        self.dots
            .iter()
            .enumerate()
            .min_by(|a, b| a.1.distance(p).total_cmp(&b.1.distance(p)))
            .map(|(i, _)| i)
    }

    fn edit(&mut self) {
        self.undo.push(self.dots.clone());
        self.redo.clear();
        self.dirty = true;
    }

    fn act(&mut self, action: Action, at: Pos2) {
        let snap = |p: Pos2, on: bool| {
            if on {
                pos2((p.x / 20.0).round() * 20.0, (p.y / 20.0).round() * 20.0)
            } else {
                p
            }
        };
        match action {
            Action::AddDot => {
                self.edit();
                self.dots.push(snap(at, self.snap));
            }
            Action::DeleteDot | Action::Cut => {
                if let Some(i) = self.nearest(at) {
                    self.edit();
                    let p = self.dots.remove(i);
                    if action == Action::Cut {
                        self.copied = Some(p);
                    }
                }
            }
            Action::Copy => self.copied = self.nearest(at).map(|i| self.dots[i]),
            Action::Paste => {
                if self.copied.is_some() {
                    self.edit();
                    self.dots.push(snap(at, self.snap));
                }
            }
            Action::ZoomToFit | Action::Recenter => {
                self.offset = Vec2::ZERO;
                if action == Action::ZoomToFit {
                    self.scale = 1.0;
                }
            }
            Action::Snap => self.snap = !self.snap,
            Action::Grid => self.grid = !self.grid,
            Action::Save => self.dirty = false,
            Action::Undo => {
                if let Some(prev) = self.undo.pop() {
                    self.redo.push(std::mem::replace(&mut self.dots, prev));
                    self.dirty = true;
                }
            }
            Action::Redo => {
                if let Some(next) = self.redo.pop() {
                    self.undo.push(std::mem::replace(&mut self.dots, next));
                    self.dirty = true;
                }
            }
            Action::Measure | Action::Highlight | Action::Route | Action::Flip | Action::Fill => {}
        }
    }

    fn side_panel(&mut self, ui: &mut Ui) {
        let t = ui.tokens();
        let status = self.mouse.status();
        let color = match status {
            Status::Connected => t.colors.good,
            Status::Connecting => t.colors.text_muted,
            Status::Down(_) => t.colors.warn,
        };
        ui.colored_label(color, status.line())
            .on_hover_text(format!("Socket {}", self.mouse.socket().display()));
        ui.add_space(t.space.m);
        ui.heading("Mapping");
        let live = self.mouse.axes();
        mapping_ui(ui, &mut self.mapping, Some(&live));
        ui.add_space(t.space.s);
        let calibrating = self.calibration.is_some();
        if ui
            .add_enabled(!calibrating, egui::Button::new("Calibrate…"))
            .on_hover_text(
                "Measure which way you move the puck for pan and zoom, and which button is which",
            )
            .on_disabled_hover_text("The calibration window is open")
            .clicked()
        {
            self.calibration = Some(Calibration::two_d(self.mapping.clone()));
        }
        ui.add_space(t.space.m);
        ui.heading("Fired");
        egui::ScrollArea::vertical()
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for line in &self.log {
                    ui.label(line);
                }
            });
    }

    fn canvas(&mut self, ui: &mut Ui) {
        let t = ui.tokens();
        let (rect, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
        let center = rect.center();
        let p = ui.painter_at(rect);
        let step = 20.0 * self.scale;
        if self.grid && step > 4.0 {
            let origin = center + self.offset;
            let x0 = origin.x - ((origin.x - rect.left()) / step).floor() * step;
            let y0 = origin.y - ((origin.y - rect.top()) / step).floor() * step;
            let mut x = x0;
            while x < rect.right() {
                let mut y = y0;
                while y < rect.bottom() {
                    p.circle_filled(pos2(x, y), 1.0, t.colors.line);
                    y += step;
                }
                x += step;
            }
        }
        for d in &self.dots {
            p.circle_filled(
                center + self.offset + d.to_vec2() * self.scale,
                6.0 * self.scale.sqrt(),
                t.colors.accent,
            );
        }
        p.text(
            rect.left_top() + vec2(8.0, 8.0),
            egui::Align2::LEFT_TOP,
            format!(
                "zoom {:.2}×  ·  tap a button for its centre slot, hold it for the ring",
                self.scale
            ),
            egui::FontId::proportional(13.0),
            t.colors.text_muted,
        );
    }
}

impl eframe::App for Demo {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, MAPPING_KEY, &self.mapping);
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let events = self.mouse.poll();
        let dt = ui.input(|i| i.stable_dt).min(0.1);

        if let Some(cal) = &mut self.calibration {
            let now = ui.input(|i| i.time);
            for e in &events {
                cal.feed(e, now);
            }
        }

        egui::Panel::right("side")
            .min_size(320.0)
            .show(ui, |ui| self.side_panel(ui));
        let mut canvas_center = Pos2::ZERO;
        egui::CentralPanel::default().show(ui, |ui| {
            canvas_center = ui.max_rect().center();
            self.canvas(ui);
        });

        if let Some(cal) = &mut self.calibration {
            let mut open = true;
            let mut action = CalibrationAction::None;
            egui::Window::new("Calibrate the SpaceMouse")
                .open(&mut open)
                .default_width(460.0)
                .show(ui.ctx(), |ui| action = calibration_ui(ui, cal));
            match action {
                CalibrationAction::Apply(m) => {
                    self.log.push(format!("calibrated: {m:?}"));
                    self.mapping = m;
                    self.calibration = None;
                }
                CalibrationAction::Cancel => self.calibration = None,
                CalibrationAction::None if !open => self.calibration = None,
                CalibrationAction::None => {}
            }
        } else {
            // Navigation and menu only while not calibrating: the motions there are measurements.
            let nav = self.mapping.apply(&self.mouse.axes());
            if !nav.is_zero() {
                self.offset -= nav.pan_screen() * dt;
                let k = (nav.zoom * dt).exp();
                self.scale = (self.scale * k).clamp(0.05, 40.0);
                self.offset *= k;
                ui.ctx().request_repaint_after(Duration::from_millis(16));
            }
            let can = Can {
                paste: self.copied.is_some(),
                save: self.dirty,
                undo: !self.undo.is_empty(),
                redo: !self.redo.is_empty(),
            };
            let menu = menu::menu(can);
            for fired in menu.show(ui.ctx(), &mut self.pie, &buttons(&events, &self.mapping)) {
                let at = self.to_canvas(canvas_center, fired.at);
                let label = menu
                    .slot(fired.ring, fired.slot)
                    .map_or("?", |s| s.label.as_str())
                    .to_owned();
                self.log.push(format!(
                    "{label}{} at {:.0}, {:.0}",
                    if fired.tap { " (tap)" } else { "" },
                    at.x,
                    at.y
                ));
                self.act(fired.action, at);
            }
        }
    }
}
