//! What the CPU painter costs per frame (BACK-3): building the shapes (the field sampled per mark) and
//! tessellating them, with the vertex and index counts, for a laptop and a desktop window at 1 and 2 pixels
//! per point (a phone or HiDPI screen).
//!
//! Run in release, as an app ships: `cargo run --release -p ve_backdrop --example backdrop_bench`
//! (`-- --squares` for squares).

// Native-only: a timing tool, never in a web build.
#![allow(clippy::disallowed_methods)]

use std::time::{Duration, Instant};

use egui::epaint::{ClippedShape, Primitive};
use egui::{Context, Id, Rect, pos2, vec2};
use ve_backdrop::{Backdrop, Lit, Mark, Settings};

const ROUNDS: u32 = 200;

fn main() {
    let mark = if std::env::args().any(|a| a == "--squares") {
        Mark::Squares
    } else {
        Mark::Dots
    };
    println!("CPU painter, {mark:?}, mean of {ROUNDS} frames");
    println!("size        ppp  marks  build ms  tess ms  vertices  indices");
    for (w, h) in [(1400.0, 900.0), (1920.0, 1200.0)] {
        for ppp in [1.0, 2.0] {
            bench(w, h, ppp, mark);
        }
    }
}

fn bench(w: f32, h: f32, ppp: f32, mark: Mark) {
    let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(w, h));
    let ctx = Context::default();
    ctx.set_pixels_per_point(ppp);
    let mut backdrop = Backdrop::new().with_settings(Settings {
        mark,
        ..Settings::default()
    });
    // A dashboard's worth of frames, one focused, and the pointer over the grid.
    let lit: Vec<Lit> = (0..12)
        .map(|i| {
            let at = pos2(40.0 + (i % 4) as f32 * 330.0, 40.0 + (i / 4) as f32 * 280.0);
            Lit::new(Rect::from_min_size(at, vec2(300.0, 240.0)))
                .corner_radius(6.0)
                .focused(i == 5)
                .id(Id::new(i))
        })
        .collect();

    let mut build = Duration::ZERO;
    let mut tess = Duration::ZERO;
    let (mut marks, mut vertices, mut indices) = (0, 0, 0);
    for round in 0..=ROUNDS {
        let input = egui::RawInput {
            time: Some(f64::from(round) / 30.0),
            screen_rect: Some(rect),
            events: vec![egui::Event::PointerMoved(pos2(w * 0.5, h * 0.5))],
            ..Default::default()
        };
        let mut shape = egui::Shape::Noop;
        let mut output = ctx.run_ui(input, |ui| {
            let start = Instant::now();
            shape = backdrop.shape(ui.ctx(), rect, &lit);
            if round > 0 {
                build += start.elapsed();
            }
        });
        output.textures_delta.clear();
        marks = match &shape {
            egui::Shape::Vec(shapes) => shapes.len().saturating_sub(1),
            _ => 0,
        };
        let shapes = vec![ClippedShape {
            clip_rect: rect,
            shape,
        }];
        let start = Instant::now();
        let meshes = ctx.tessellate(shapes, ppp);
        if round > 0 {
            tess += start.elapsed();
        }
        (vertices, indices) = meshes.iter().fold((0, 0), |(v, i), m| match &m.primitive {
            Primitive::Mesh(mesh) => (v + mesh.vertices.len(), i + mesh.indices.len()),
            Primitive::Callback(_) => (v, i),
        });
    }
    let ms = |d: Duration| d.as_secs_f64() * 1000.0 / f64::from(ROUNDS);
    println!(
        "{:>4} x {:<4}  {ppp:>3}  {marks:>5}  {:>8.3}  {:>7.3}  {vertices:>8}  {indices:>7}",
        w,
        h,
        ms(build),
        ms(tess)
    );
}
