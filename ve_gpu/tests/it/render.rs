//! Batches rendered through the headless harness and checked pixel by pixel: placement, clip, paint order, pixels
//! per point, several batches a frame, the stats, the software adapter and a device held to WebGL2's limits.

use egui::{Color32, ColorImage, Context, Rect, Stroke, pos2, vec2};
use ve_gpu::headless::{AdapterPick, Headless, HeadlessOptions};
use ve_gpu::{Batch, Column, ColumnStyle, LineStyle, RectStyle};

use crate::scenes::ALL;

const RED: Color32 = Color32::from_rgb(220, 30, 30);
const BLUE: Color32 = Color32::from_rgb(30, 30, 220);

fn headless(options: HeadlessOptions) -> Headless {
    Headless::new(options).expect("a wgpu adapter: the software one at least")
}

fn small() -> HeadlessOptions {
    HeadlessOptions {
        size_px: [64, 48],
        ..HeadlessOptions::default()
    }
}

fn px(img: &ColorImage, x: usize, y: usize) -> Color32 {
    img.pixels[y * img.width() + x]
}

/// The pixels of `img` equal to `c`.
fn count(img: &ColorImage, c: Color32) -> usize {
    img.pixels.iter().filter(|p| **p == c).count()
}

/// Draws one frame of `fill` into `h`, a batch over the whole screen.
fn draw(
    h: &mut Headless,
    ctx: &Context,
    batch: &mut Batch,
    fill: impl Fn(&mut Batch),
) -> ColorImage {
    h.frame(ctx, |ui| {
        batch.clear();
        fill(batch);
        ui.painter().add(batch.paint(ui.max_rect()));
    })
    .expect("a frame");
    h.read().expect("the pixels")
}

#[test]
fn a_rect_on_whole_pixels_covers_exactly_them() {
    let mut h = headless(small());
    let (ctx, mut batch) = (Context::default(), Batch::new());
    let img = draw(&mut h, &ctx, &mut batch, |b| {
        b.rect(
            Rect::from_min_size(pos2(10.0, 5.0), vec2(20.0, 10.0)),
            &RectStyle::filled(RED),
        );
    });
    assert_eq!(count(&img, RED), 200);
    assert_eq!(px(&img, 10, 5), RED);
    assert_eq!(px(&img, 29, 14), RED);
    assert_eq!(px(&img, 9, 5), Color32::BLACK);
    assert_eq!(px(&img, 30, 14), Color32::BLACK);
}

#[test]
fn a_snapped_rect_rounds_to_pixels_and_a_free_one_blends_its_edges() {
    let mut h = headless(small());
    let (ctx, mut batch) = (Context::default(), Batch::new());
    let r = Rect::from_min_size(pos2(10.4, 5.0), vec2(20.0, 10.0));
    let img = draw(&mut h, &ctx, &mut batch, |b| {
        b.rect(r, &RectStyle::filled(RED))
    });
    assert_eq!(count(&img, RED), 200, "snapped to 10..30");
    let mut free = RectStyle::filled(RED);
    free.snap = false;
    let img = draw(&mut h, &ctx, &mut batch, |b| b.rect(r, &free));
    assert_eq!(count(&img, RED), 190, "19 whole columns");
    let edge = px(&img, 10, 8);
    assert!(
        edge.r() > 100 && edge.r() < 160,
        "60 % of the edge pixel: {edge:?}"
    );
}

#[test]
fn the_batch_clip_cuts_shapes() {
    let mut h = headless(small());
    let (ctx, mut batch) = (Context::default(), Batch::new());
    let img = draw(&mut h, &ctx, &mut batch, |b| {
        b.set_clip(Rect::from_min_max(pos2(0.0, 0.0), pos2(20.0, 48.0)));
        b.rect(
            Rect::from_min_size(pos2(10.0, 5.0), vec2(20.0, 10.0)),
            &RectStyle::filled(RED),
        );
        b.reset_clip();
        b.rect(
            Rect::from_min_size(pos2(40.0, 5.0), vec2(10.0, 10.0)),
            &RectStyle::filled(BLUE),
        );
    });
    assert_eq!(count(&img, RED), 100);
    assert_eq!(count(&img, BLUE), 100);
}

#[test]
fn shapes_draw_in_order_and_egui_paints_over_the_batch() {
    let mut h = headless(small());
    let (ctx, mut batch) = (Context::default(), Batch::new());
    h.frame(&ctx, |ui| {
        batch.clear();
        batch.rect(
            Rect::from_min_size(pos2(0.0, 0.0), vec2(30.0, 30.0)),
            &RectStyle::filled(RED),
        );
        batch.rect(
            Rect::from_min_size(pos2(10.0, 10.0), vec2(30.0, 30.0)),
            &RectStyle::filled(BLUE),
        );
        ui.painter().add(batch.paint(ui.max_rect()));
        ui.painter().rect_filled(
            Rect::from_min_size(pos2(20.0, 20.0), vec2(4.0, 4.0)),
            0.0,
            Color32::WHITE,
        );
    })
    .expect("a frame");
    let img = h.read().expect("the pixels");
    assert_eq!(px(&img, 5, 5), RED);
    assert_eq!(px(&img, 15, 15), BLUE, "the later shape over the earlier");
    assert_eq!(
        px(&img, 21, 21),
        Color32::WHITE,
        "egui's shape after the callback over it"
    );
}

#[test]
fn sizes_stay_in_points_at_two_pixels_a_point() {
    let mut h = headless(HeadlessOptions {
        size_px: [128, 96],
        pixels_per_point: 2.0,
        ..HeadlessOptions::default()
    });
    let (ctx, mut batch) = (Context::default(), Batch::new());
    let img = draw(&mut h, &ctx, &mut batch, |b| {
        b.rect(
            Rect::from_min_size(pos2(10.0, 5.0), vec2(20.0, 10.0)),
            &RectStyle::filled(RED),
        );
        // a vertical line one point wide: two pixel columns
        b.segment(
            [pos2(50.0, 10.0), pos2(50.0, 40.0)],
            &LineStyle::solid(Stroke::new(1.0, BLUE)),
        );
    });
    assert_eq!(count(&img, RED), 800);
    let row: Vec<Color32> = (96..104).map(|x| px(&img, x, 40)).collect();
    assert_eq!(row.iter().filter(|p| **p == BLUE).count(), 2, "{row:?}");
}

#[test]
fn batches_of_a_frame_go_up_together() {
    let mut h = headless(small());
    let ctx = Context::default();
    let mut batches = [Batch::new(), Batch::new(), Batch::new()];
    for _ in 0..3 {
        h.frame(&ctx, |ui| {
            for (k, b) in batches.iter_mut().enumerate() {
                b.clear();
                let x = 4.0 + 20.0 * k as f32;
                b.rect(
                    Rect::from_min_size(pos2(x, 4.0), vec2(10.0, 10.0)),
                    &RectStyle::filled(RED),
                );
                b.segment(
                    [pos2(x, 30.0), pos2(x + 10.0, 30.0)],
                    &LineStyle::solid(Stroke::new(1.0, BLUE)),
                );
                ui.painter().add(b.paint(ui.max_rect()));
            }
        })
        .expect("a frame");
    }
    let img = h.read().expect("the pixels");
    assert_eq!(count(&img, RED), 300);
    let stats = ve_gpu::frame_stats(h.render_state()).expect("installed");
    assert_eq!(stats.batches, 3);
    assert_eq!(stats.shapes, 6);
    assert_eq!(
        stats.draws, 3,
        "one draw a batch: its shapes in one instanced call"
    );
    assert_eq!(stats.upload_bytes, 16 + 6 * 64);
}

#[test]
fn a_batch_left_unpainted_draws_nothing() {
    let mut h = headless(small());
    let (ctx, mut batch) = (Context::default(), Batch::new());
    let img = draw(&mut h, &ctx, &mut batch, |b| {
        b.rect(
            Rect::from_min_size(pos2(10.0, 5.0), vec2(20.0, 10.0)),
            &RectStyle::filled(RED),
        );
    });
    assert_eq!(count(&img, RED), 200);
    h.frame(&ctx, |_| {}).expect("a frame");
    assert_eq!(count(&h.read().expect("the pixels"), RED), 0);
}

#[test]
fn a_band_spans_the_mapped_values_in_whole_pixels() {
    let mut h = headless(small());
    let (ctx, mut batch) = (Context::default(), Batch::new());
    let style = ColumnStyle {
        clip: Rect::EVERYTHING,
        value_lo: 0.0,
        value_hi: 10.0,
        y_lo: 40.0,
        y_hi: 0.0,
        band: RED,
        edge: Color32::TRANSPARENT,
        mean: Stroke::NONE,
    };
    let img = draw(&mut h, &ctx, &mut batch, |b| {
        // two columns a pixel, folded into one band column each: values 2..5 are y 20..32
        let cols = (0..20).map(|i| Column {
            x: 10.0 + i as f32 * 0.5,
            min: 2.0,
            max: if i % 2 == 0 { 4.0 } else { 5.0 },
            mean: 3.0,
        });
        b.columns(&style, 1.0, cols);
    });
    assert_eq!(count(&img, RED), 10 * 12);
    assert_eq!(px(&img, 10, 20), RED);
    assert_eq!(px(&img, 19, 31), RED);
    assert_eq!(px(&img, 10, 19), Color32::BLACK);
    assert_eq!(px(&img, 20, 25), Color32::BLACK);
}

#[test]
fn edges_and_mean_follow_the_columns() {
    let mut h = headless(small());
    let (ctx, mut batch) = (Context::default(), Batch::new());
    let style = ColumnStyle {
        clip: Rect::EVERYTHING,
        value_lo: 0.0,
        value_hi: 10.0,
        y_lo: 40.0,
        y_hi: 0.0,
        band: Color32::TRANSPARENT,
        edge: BLUE,
        mean: Stroke::new(1.0, RED),
    };
    let img = draw(&mut h, &ctx, &mut batch, |b| {
        let cols = (0..40).map(|i| Column {
            x: 10.0 + i as f32,
            min: 1.0,
            max: 9.0,
            mean: 5.0,
        });
        b.columns(&style, 1.0, cols);
    });
    // flat lines at y 36 (min), 4 (max) and 20 (mean); egui's one pixel line spreads over the two pixels it meets
    let column: Vec<u8> = (0..48).map(|y| px(&img, 30, y).b()).collect();
    assert!(column[3] > 50 && column[4] > 50, "{column:?}");
    assert!(column[35] > 50 && column[36] > 50, "{column:?}");
    assert!(px(&img, 30, 19).r() > 50 || px(&img, 30, 20).r() > 50);
    assert_eq!(px(&img, 30, 28), Color32::BLACK);
}

#[test]
fn every_scene_draws_on_the_software_adapter() {
    let mut h = headless(HeadlessOptions {
        adapter: AdapterPick::Software,
        size_px: [240, 110],
        ..HeadlessOptions::default()
    });
    assert!(h.adapter_name().contains("Cpu"), "{}", h.adapter_name());
    every_scene_draws(&mut h);
}

/// A device held to WebGL2's downlevel limits takes every pipeline and draws every scene; wgpu fails a pipeline or
/// a draw past a limit.
#[test]
fn every_scene_draws_within_webgl2_limits() {
    let mut h = headless(HeadlessOptions {
        size_px: [240, 110],
        webgl2_limits: true,
        ..HeadlessOptions::default()
    });
    let limits = h.render_state().device.limits();
    assert_eq!(limits.max_storage_buffers_per_shader_stage, 0);
    every_scene_draws(&mut h);
}

fn every_scene_draws(h: &mut Headless) {
    let ctx = Context::default();
    let mut batch = Batch::new();
    let palette = ve_theme::Tokens::dark().colors;
    for (name, fill) in ALL {
        let img = draw(h, &ctx, &mut batch, |b| {
            fill(
                b,
                Rect::from_min_size(pos2(0.0, 0.0), vec2(240.0, 110.0)),
                &palette,
                1.0,
            )
        });
        let lit = img.pixels.iter().filter(|p| **p != Color32::BLACK).count();
        assert!(lit > 100, "{name}: {lit} pixels drawn");
    }
}

#[test]
fn a_float_target_reads_back_values_past_white() {
    let mut h = headless(HeadlessOptions {
        format: Some(egui_wgpu::wgpu::TextureFormat::Rgba16Float),
        clear: Color32::from_rgb(255, 128, 0),
        ..small()
    });
    let ctx = Context::default();
    h.frame(&ctx, |_| {}).expect("renders");
    let px = h.read_f32().expect("reads back");
    assert_eq!(px.len(), 64 * 48);
    assert_eq!(
        px[0],
        [1.0, 128.0 / 255.0, 0.0, 1.0].map(|v: f32| f16_round(v))
    );
}

/// `v` as an `f16` would hold it.
fn f16_round(v: f32) -> f32 {
    let bits = v.to_bits();
    // round the mantissa to 10 bits, enough for values in 0..=1
    f32::from_bits((bits + 0x1000) & !0x1fff)
}
