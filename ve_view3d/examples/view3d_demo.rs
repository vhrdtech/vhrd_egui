//! The 3D view demo: procedural meshes (a board, a chip with leads, a capacitor, a ball), the camera's preset views,
//! picking with hover and selection, a two-pick measure, and the SpaceMouse on all six axes.
//!
//! ```sh
//! cargo run -p ve_view3d --example view3d_demo                               # spacenavd's socket
//! cargo run -p ve_view3d --example view3d_demo -- --socket /tmp/spnav.sock   # another one, e.g. a fake
//! ```
//!
//! The camera is saved with the window state (eframe persistence); the puck's mapping is the shared one in
//! `spacemouse.toml` (ve_spacemouse's settings panel), so a calibration here applies to every app.

use std::path::PathBuf;

use egui::{Color32, RichText};
use glam::Vec3;
use ve_spacemouse::{CalibrateSteps, Options, SettingsPanel, SpaceMouse, Status};
use ve_view3d::{
    Camera, Mark, Measurement, Mesh, Pick, Projection, Scene, View3d, ViewPreset, shapes,
};

const CAMERA_KEY: &str = "ve_view3d_camera";

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
                println!("3D view demo: turn, pan, zoom, pick and measure procedural meshes.\n");
                println!(
                    "Usage: view3d_demo [--socket PATH]   (default {})",
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
            .with_title("3D view demo"),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native(
        "ve_view3d demo",
        options,
        Box::new(move |cc| {
            ve_theme::setup(&cc.egui_ctx);
            let mut view = match &cc.wgpu_render_state {
                Some(rs) => View3d::new(rs),
                None => View3d::without_gpu(),
            };
            view.set_scene(demo_scene(false));
            match cc
                .storage
                .and_then(|s| eframe::get_value::<Camera>(s, CAMERA_KEY))
            {
                Some(camera) => view.camera = camera,
                None => view.view(ViewPreset::Iso),
            }
            let settings = SettingsPanel::load(CalibrateSteps::Full);
            let mouse = SpaceMouse::start(Options {
                socket,
                ..Default::default()
            })
            .with_repaint(&cc.egui_ctx);
            Ok(Box::new(Demo {
                view,
                mouse,
                settings,
                big: false,
                measure: Vec::new(),
            }))
        }),
    )
}

struct Demo {
    view: View3d,
    mouse: SpaceMouse,
    settings: SettingsPanel,
    /// The 176,000-triangle ball instead of the small one (the largest model seen).
    big: bool,
    /// Up to two picks; with two, the measurement between them is shown.
    measure: Vec<Pick>,
}

/// A board with a chip on it (body and gull-wing leads, each lead its own face), a capacitor and a ball.
fn demo_scene(big: bool) -> Scene {
    let board_color = Color32::from_rgb(0x2e, 0x5e, 0x3a);
    let mut meshes = vec![shapes::cuboid(
        Vec3::new(-12.0, -8.0, -1.6),
        Vec3::new(12.0, 8.0, 0.0),
        board_color,
    )];
    let mut lines = vec![shapes::cuboid_edges(
        Vec3::new(-12.0, -8.0, -1.6),
        Vec3::new(12.0, 8.0, 0.0),
    )];

    // An SOIC-8-like chip: a body and eight leads in one mesh, one face per lead.
    let body = (Vec3::new(-6.4, -2.0, 0.1), Vec3::new(-1.4, 2.0, 1.75));
    let mut chip = shapes::cuboid(body.0, body.1, Color32::from_gray(40));
    let mut chip_edges = shapes::cuboid_edges(body.0, body.1);
    for i in 0..8 {
        let x = -6.4 + 0.6 + 1.27 * (i % 4) as f32;
        let (y0, y1) = if i < 4 { (-3.0, -2.0) } else { (2.0, 3.0) };
        let lead = shapes::cuboid(
            Vec3::new(x - 0.2, y0, 0.0),
            Vec3::new(x + 0.2, y1, 0.25),
            Color32::from_gray(190),
        );
        append(&mut chip, &lead, true);
        chip_edges.polylines.extend(
            shapes::cuboid_edges(Vec3::new(x - 0.2, y0, 0.0), Vec3::new(x + 0.2, y1, 0.25))
                .polylines,
        );
    }
    meshes.push(chip);
    lines.push(chip_edges);

    let cap = Vec3::new(4.0, -3.0, 0.0);
    meshes.push(shapes::cylinder(
        cap,
        2.0,
        5.5,
        64,
        Color32::from_rgb(0x3a, 0x5a, 0xa0),
    ));
    lines.push(shapes::cylinder_edges(cap, 2.0, 5.5, 64));

    let (segments, rings) = if big { (420, 210) } else { (48, 24) };
    meshes.push(shapes::uv_sphere(
        Vec3::new(7.5, 4.0, 1.5),
        1.5,
        segments,
        rings,
        shapes::PALETTE[5],
    ));
    for l in &mut lines {
        l.color = Color32::from_gray(15);
    }
    Scene { meshes, lines }
}

/// Append `other` to `mesh`, its faces after `mesh`'s (each with its colour), or all as one face when `own_faces` is off.
fn append(mesh: &mut Mesh, other: &Mesh, own_faces: bool) {
    let base = mesh.positions.len() as u32;
    let face_base = mesh.face_colors.len() as u32;
    mesh.positions.extend(&other.positions);
    mesh.normals.extend(&other.normals);
    mesh.triangles
        .extend(other.triangles.iter().map(|t| t.map(|i| i + base)));
    if own_faces {
        mesh.face_of_triangle
            .extend(other.face_of_triangle.iter().map(|f| f + face_base));
        mesh.face_colors.extend(&other.face_colors);
    } else {
        mesh.face_of_triangle
            .extend(std::iter::repeat_n(face_base, other.triangles.len()));
        mesh.face_colors
            .push(other.face_colors.first().copied().unwrap_or(Color32::GRAY));
    }
}

fn describe(p: &Pick) -> String {
    let mut s = String::new();
    if let Some(f) = p.face {
        s += &format!("mesh {} face {} (triangle {})", f.mesh, f.face, f.triangle);
    }
    if let Some(e) = p.edge {
        if !s.is_empty() {
            s += ", ";
        }
        s += &format!("edge {} of set {}", e.edge, e.set);
    }
    format!(
        "{s}\nat ({:.3}, {:.3}, {:.3}) mm",
        p.point.x, p.point.y, p.point.z
    )
}

impl eframe::App for Demo {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, CAMERA_KEY, &self.view.camera);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let events = self.mouse.poll();
        self.settings.feed(&events, ui.input(|i| i.time));
        if !self.settings.calibrating() {
            self.view
                .navigate(self.settings.mapping().apply(&self.mouse.axes()));
        }
        let measured = match self.measure.as_slice() {
            [a, b] => Some(Measurement::between(a, b, self.view.scene())),
            _ => None,
        };

        egui::Panel::right("controls")
            .min_size(260.0)
            .show(ui, |ui| {
                ui.heading("View");
                ui.horizontal_wrapped(|ui| {
                    for preset in ViewPreset::ALL {
                        if ui.button(preset.name()).clicked() {
                            self.view.view(preset);
                        }
                    }
                    if ui.button("fit").clicked() {
                        self.view.fit();
                    }
                });
                ui.horizontal(|ui| {
                    for p in [Projection::Perspective, Projection::Orthographic] {
                        ui.radio_value(&mut self.view.camera.projection, p, p.name());
                    }
                });
                let tris = self.view.scene().triangle_count();
                if ui
                    .checkbox(&mut self.big, "Big ball (176,000 triangles)")
                    .on_hover_text("The largest model seen; picking goes through a BVH")
                    .changed()
                {
                    self.view.set_scene(demo_scene(self.big));
                    self.measure.clear();
                }
                ui.weak(format!("{tris} triangles"));

                ui.separator();
                ui.heading("Pick");
                match self.view.hovered() {
                    Some(p) => ui.label(describe(&p)),
                    None => ui.weak("Point at the model"),
                };
                ui.label(format!("{} selected", self.view.selection.len()));

                ui.separator();
                ui.heading("Measure");
                ui.weak("Click two points; the third click starts again.");
                if let Some(m) = &measured {
                    ui.label(RichText::new(format!("{:.3} mm", m.distance)).strong());
                    egui::Grid::new("delta").show(ui, |ui| {
                        for (axis, v) in [("dX", m.delta.x), ("dY", m.delta.y), ("dZ", m.delta.z)] {
                            ui.label(axis);
                            ui.label(RichText::new(format!("{v:+.3} mm")).monospace())
                                .on_hover_text("Second point minus first, in the model's axes");
                            ui.end_row();
                        }
                    });
                    if let Some(r) = m.relation_line("mm") {
                        ui.label(RichText::new(r).strong());
                    }
                }

                ui.separator();
                ui.heading("SpaceMouse");
                let status = self.mouse.status();
                let color = match status {
                    Status::Connected => ui.visuals().text_color(),
                    _ => ui.visuals().warn_fg_color,
                };
                ui.colored_label(color, status.line());
                egui::CollapsingHeader::new("Settings").show(ui, |ui| {
                    self.settings.ui(ui, Some(&self.mouse.axes()));
                });
            });
        self.settings.calibration_window(ui.ctx());
        self.settings.leave_guard(ui.ctx());

        egui::CentralPanel::default().show(ui, |ui| {
            let r = self.view.show(ui);
            if let Some(p) = r.clicked {
                if self.measure.len() == 2 {
                    self.measure.clear();
                }
                self.measure.push(p);
            } else if r.response.clicked() {
                self.measure.clear();
            }
            self.view.marks = match self.measure.as_slice() {
                [a] => vec![Mark::point(a.point, "1")],
                [a, b] => {
                    let m = Measurement::between(a, b, self.view.scene());
                    vec![Mark::segment(a.point, b.point, m.label("mm"))]
                }
                _ => vec![],
            };
        });
    }
}
