//! The SpaceMouse demo: a canvas of dots panned and zoomed by the puck, the pie menu on its buttons, the
//! mapping as settings and the calibrate step.
//!
//! ```sh
//! cargo run -p ve_spacemouse --example demo                              # spacenavd's socket
//! cargo run -p ve_spacemouse --example demo -- --socket /tmp/spnav.sock  # another one, e.g. a fake
//! ```
//!
//! The mapping is the shared one in `spacemouse.toml` (`~/.config`, or a project's `.config` above the current
//! folder): a calibration here applies to every app on ve_spacemouse.

mod menu;

use std::path::PathBuf;
use std::time::Duration;

use egui::emath::Rot2;
use egui::{Pos2, Sense, Ui, Vec2, pos2, vec2};
use menu::{Action, Can};
use ve_spacemouse::{
    CalibrateSteps, Options, PieState, SettingsPanel, SpaceMouse, Status, buttons,
};
use ve_theme::UiExt;

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
            // All six motions: twist turns this canvas.
            let settings = SettingsPanel::load(CalibrateSteps::Full);
            let mouse = SpaceMouse::start(Options {
                socket,
                ..Default::default()
            })
            .with_repaint(&cc.egui_ctx);
            Ok(Box::new(Demo::new(mouse, settings)))
        }),
    )
}

struct Demo {
    mouse: SpaceMouse,
    settings: SettingsPanel,
    pie: PieState,
    /// Canvas: points in canvas units; where the canvas origin sits from the centre, its scale and its turn
    /// (clockwise on screen, radians).
    dots: Vec<Pos2>,
    offset: Vec2,
    scale: f32,
    angle: f32,
    copied: Option<Pos2>,
    undo: Vec<Vec<Pos2>>,
    redo: Vec<Vec<Pos2>>,
    dirty: bool,
    snap: bool,
    grid: bool,
    log: Vec<String>,
}

impl Demo {
    fn new(mouse: SpaceMouse, settings: SettingsPanel) -> Self {
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
            settings,
            pie: PieState::default(),
            dots,
            offset: Vec2::ZERO,
            scale: 1.0,
            angle: 0.0,
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
        let v = Rot2::from_angle(-self.angle) * (screen - center - self.offset) / self.scale;
        v.to_pos2()
    }

    fn to_screen(&self, center: Pos2, canvas: Pos2) -> Pos2 {
        center + self.offset + Rot2::from_angle(self.angle) * canvas.to_vec2() * self.scale
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
                self.angle = 0.0;
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
        ui.heading("Settings");
        let live = self.mouse.axes();
        self.settings.ui(ui, Some(&live));
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
        if self.grid && 20.0 * self.scale > 4.0 {
            // The canvas grid points that can land in the view, whatever the turn.
            let corners = [
                rect.left_top(),
                rect.right_top(),
                rect.left_bottom(),
                rect.right_bottom(),
            ]
            .map(|c| self.to_canvas(center, c));
            let lo = corners
                .iter()
                .fold(pos2(f32::MAX, f32::MAX), |a, c| a.min(*c));
            let hi = corners
                .iter()
                .fold(pos2(f32::MIN, f32::MIN), |a, c| a.max(*c));
            let mut x = (lo.x / 20.0).floor() * 20.0;
            while x <= hi.x {
                let mut y = (lo.y / 20.0).floor() * 20.0;
                while y <= hi.y {
                    p.circle_filled(self.to_screen(center, pos2(x, y)), 1.0, t.colors.line);
                    y += 20.0;
                }
                x += 20.0;
            }
        }
        for d in &self.dots {
            p.circle_filled(
                self.to_screen(center, *d),
                6.0 * self.scale.sqrt(),
                t.colors.accent,
            );
        }
        p.text(
            rect.left_top() + vec2(8.0, 8.0),
            egui::Align2::LEFT_TOP,
            format!(
                "zoom {:.2}×  ·  turn {:.0}°  ·  {}  ·  tap a button for its centre slot, hold it for the ring",
                self.scale,
                self.angle.to_degrees(),
                self.settings.mapping().convention.name()
            ),
            egui::FontId::proportional(13.0),
            t.colors.text_muted,
        );
    }
}

impl eframe::App for Demo {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let events = self.mouse.poll();
        let dt = ui.input(|i| i.stable_dt).min(0.1);

        self.settings.feed(&events, ui.input(|i| i.time));

        egui::Panel::right("side")
            .min_size(320.0)
            .show(ui, |ui| self.side_panel(ui));
        let mut canvas_center = Pos2::ZERO;
        egui::CentralPanel::default().show(ui, |ui| {
            canvas_center = ui.max_rect().center();
            self.canvas(ui);
        });

        if self.settings.calibration_window(ui.ctx()) {
            self.log
                .push(format!("calibrated: {:?}", self.settings.mapping()));
        }
        if !self.settings.calibrating() {
            // Navigation and menu only while not calibrating: the motions there are measurements.
            let nav = self.settings.mapping().apply(&self.mouse.axes());
            if !nav.is_zero() {
                // Nav is what the content does: move, grow and turn it about the view's centre.
                self.offset += nav.pan_screen() * dt;
                let k = (nav.zoom * dt).exp();
                let scale = (self.scale * k).clamp(0.05, 40.0);
                self.offset *= scale / self.scale;
                self.scale = scale;
                let turn = nav.rotate[2] * dt;
                self.offset = Rot2::from_angle(turn) * self.offset;
                self.angle += turn;
                ui.ctx().request_repaint_after(Duration::from_millis(16));
            }
            let can = Can {
                paste: self.copied.is_some(),
                save: self.dirty,
                undo: !self.undo.is_empty(),
                redo: !self.redo.is_empty(),
            };
            let menu = menu::menu(can);
            for fired in menu.show(
                ui.ctx(),
                &mut self.pie,
                &buttons(&events, self.settings.mapping()),
            ) {
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
