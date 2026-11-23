//! A snapshot of each primitive in both themes, drawn through egui_kittest's wgpu renderer (the software adapter
//! where there is one) with ve_gpu installed in it.

use egui::{Theme, vec2};
use egui_kittest::wgpu::{WgpuTestRenderer, create_render_state, default_wgpu_setup};
use egui_kittest::{Harness, SnapshotOptions};
use ve_gpu::Batch;

use crate::scenes::{ALL, Scene};

const SIZE: egui::Vec2 = vec2(240.0, 110.0);

/// GPUs differ in a few antialiased pixels; a real change moves far more.
fn tolerant() -> SnapshotOptions {
    SnapshotOptions::new().threshold(1.0).max_failed_pixels(20)
}

fn harness(scene: Scene, theme: Theme, ppp: f32) -> Harness<'static, Batch> {
    let rs = create_render_state(
        default_wgpu_setup(),
        egui_wgpu::RendererOptions::PREDICTABLE,
    );
    ve_gpu::install(&rs);
    let mut h = Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(ppp)
        .with_theme(theme)
        .renderer(WgpuTestRenderer::from_render_state(rs))
        .build_ui_state(
            move |ui, batch: &mut Batch| {
                let rect = ui.max_rect();
                let tokens = ve_theme::Tokens::of(ui.ctx(), theme);
                ui.painter().rect_filled(rect, 0.0, tokens.colors.bg);
                batch.clear();
                (scene.1)(batch, rect, &tokens.colors, ui.ctx().pixels_per_point());
                ui.painter().add(batch.paint(rect));
            },
            Batch::new(),
        );
    ve_theme::setup(&h.ctx);
    h.run();
    h
}

fn snapshots(scenes: &[Scene], theme: Theme, ppp: f32, suffix: &str) {
    let mut failed = Vec::new();
    for &scene in scenes {
        let mut h = harness(scene, theme, ppp);
        if let Err(e) = h.try_snapshot_options(format!("{}_{suffix}", scene.0), &tolerant()) {
            failed.push(format!("{e}"));
        }
    }
    assert!(failed.is_empty(), "{failed:#?}");
}

#[test]
fn each_primitive_dark() {
    snapshots(&ALL, Theme::Dark, 1.0, "dark");
}

#[test]
fn each_primitive_light() {
    snapshots(&ALL, Theme::Light, 1.0, "light");
}

/// At two pixels a point the shapes keep their size in points and their edges stay one pixel soft.
#[test]
fn shapes_at_two_pixels_a_point() {
    snapshots(&[ALL[0], ALL[4]], Theme::Dark, 2.0, "ppp2");
}
