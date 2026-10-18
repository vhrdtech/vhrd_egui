//! Headless UI tests of notices (BACK-4) through egui_kittest: a widget out of view below the window sends
//! a red ring across the backdrop and gets an arrow on the bottom edge; both renderers draw the ring alike,
//! the settings turn it off, reduced motion leaves only the arrow, a click on the arrow asks to go there.
//! After an intended visual change: `UPDATE_SNAPSHOTS=1 cargo test -p ve_backdrop --test notice`.

// Native-only tests; the lib stays wasm-gated (BACK-3).
#![allow(clippy::disallowed_methods)]

use std::cell::RefCell;
use std::rc::Rc;

use egui::{Id, Rect, Theme, pos2, vec2};
use egui_kittest::kittest::Queryable;
use egui_kittest::wgpu::{WgpuTestRenderer, create_render_state, default_wgpu_setup};
use egui_kittest::{Harness, SnapshotOptions};
use image::RgbaImage;
use ve_backdrop::{Backdrop, Notice, Settings, Severity};

const SIZE: egui::Vec2 = vec2(480.0, 320.0);

/// What the test's app saw: whether a click asked to go to the widget.
#[derive(Default)]
struct Seen {
    go: bool,
    /// Time of the first and the last frame.
    first: Option<f64>,
    last: f64,
}

fn notice() -> Notice {
    Notice {
        id: Id::new("supply"),
        rect: Rect::from_center_size(pos2(240.0, 700.0), vec2(160.0, 100.0)),
        severity: Severity::Crit,
        title: "Supply".to_owned(),
    }
}

/// A harness that announces a widget out of view below the window (when `notify`) and paints the backdrop.
fn harness(
    shader: bool,
    settings: Settings,
    notify: bool,
) -> (Harness<'static>, Rc<RefCell<Seen>>) {
    let builder = Harness::builder()
        .with_size(SIZE)
        .with_theme(Theme::Dark)
        .with_step_dt(0.05);
    let (builder, backdrop) = if shader {
        let render_state = create_render_state(
            default_wgpu_setup(),
            egui_wgpu::RendererOptions::PREDICTABLE,
        );
        let backdrop = Backdrop::with_wgpu(&render_state);
        (
            builder.renderer(WgpuTestRenderer::from_render_state(render_state)),
            backdrop,
        )
    } else {
        (builder.wgpu(), Backdrop::new())
    };
    let mut backdrop = backdrop.with_settings(settings);
    let seen = Rc::new(RefCell::new(Seen::default()));
    let app = seen.clone();
    let h = builder.build_ui(move |ui| {
        if notify {
            ve_backdrop::notify(ui.ctx(), notice());
        }
        let mut seen = app.borrow_mut();
        if ve_backdrop::take_go_to(ui.ctx(), notice().id) {
            seen.go = true;
        }
        let now = ui.input(|i| i.time);
        seen.first.get_or_insert(now);
        seen.last = now;
        let rect = ui.ctx().content_rect();
        backdrop.paint(ui.painter(), rect, &[]);
    });
    ve_theme::setup(&h.ctx);
    (h, seen)
}

/// Calm motion: no drift, so pictures compare; rings and the lens still work.
fn calm() -> Settings {
    Settings {
        ambient: 0.0,
        ..Settings::default()
    }
}

fn render(h: &mut Harness<'_>) -> RgbaImage {
    h.render().expect("the wgpu test renderer renders")
}

fn mean_diff(a: &RgbaImage, b: &RgbaImage) -> f64 {
    let sum: u64 = a
        .as_raw()
        .iter()
        .zip(b.as_raw())
        .map(|(x, y)| u64::from(x.abs_diff(*y)))
        .sum();
    sum as f64 / a.as_raw().len() as f64
}

/// Mean red minus mean blue over a horizontal band: how red the marks there are.
fn redness(image: &RgbaImage, y: std::ops::Range<u32>) -> f64 {
    let (mut sum, mut n) = (0i64, 0i64);
    for y in y {
        for x in 0..image.width() {
            let p = image.get_pixel(x, y);
            sum += i64::from(p[0]) - i64::from(p[2]);
            n += 1;
        }
    }
    sum as f64 / n as f64
}

/// Where the crest of the first ring is straight above the widget as of the last frame: it starts 380 pt
/// away (the view's bottom) less one and a half widths.
fn crest_y(seen: &Rc<RefCell<Seen>>) -> f32 {
    let s = Settings::default();
    let seen = seen.borrow();
    let age = (seen.last - seen.first.unwrap_or_default()) as f32;
    700.0 - (380.0 - 1.5 * s.notify_width + age * s.notify_speed)
}

#[test]
fn a_red_ring_crosses_the_view_from_below() {
    for shader in [false, true] {
        let (mut quiet, _) = harness(shader, calm(), false);
        let (mut h, seen) = harness(shader, calm(), true);
        quiet.run_steps(3);
        h.run_steps(3);
        let (before, after) = (render(&mut quiet), render(&mut h));
        let y = crest_y(&seen);
        assert!(y > 30.0 && y < 290.0, "{y}");
        let band = (y - 30.0) as u32..(y + 30.0) as u32;
        assert!(
            redness(&after, band.clone()) > redness(&before, band) + 1.0,
            "shader {shader}: the ring tints the marks red"
        );
        // The arrow on the bottom edge, toward the widget.
        let arrow = h.get_by_label("go to Supply");
        let at = arrow.rect();
        assert!(at.bottom() <= SIZE.y && at.bottom() > SIZE.y - 40.0);
        assert!((at.center().x - 240.0).abs() < 1.0);
    }
}

#[test]
fn shader_and_painter_draw_the_ring_alike() {
    let (mut shader, _) = harness(true, calm(), true);
    let (mut painter, _) = harness(false, calm(), true);
    shader.run_steps(6);
    painter.run_steps(6);
    let diff = mean_diff(&render(&mut shader), &render(&mut painter));
    assert!(diff < 0.8, "mean channel difference {diff}");
}

#[test]
fn snapshot_notice_ring() {
    let (mut h, _) = harness(true, calm(), true);
    h.run_steps(6);
    h.snapshot_options(
        "backdrop_notice_ring",
        &SnapshotOptions::new().threshold(1.5).max_failed_pixels(60),
    );
}

/// Notices off: the picture is the one without any notice, and there is no arrow.
#[test]
fn notices_off_draw_nothing() {
    let off = Settings {
        notify: false,
        ..calm()
    };
    let (mut quiet, _) = harness(false, off, false);
    let (mut h, _) = harness(false, off, true);
    quiet.run_steps(6);
    h.run_steps(6);
    assert_eq!(mean_diff(&render(&mut quiet), &render(&mut h)), 0.0);
    assert!(h.query_by_label("go to Supply").is_none());
}

/// Reduced motion: no ring (the picture is the one without the notice, but for the arrow), the arrow stays,
/// and nothing asks for a repaint.
#[test]
fn reduced_motion_shows_the_arrow_only() {
    let (mut quiet, _) = harness(false, Settings::reduced(), false);
    let (mut h, _) = harness(false, Settings::reduced(), true);
    quiet.run();
    h.run();
    let (a, b) = (render(&mut quiet), render(&mut h));
    assert_eq!(redness(&a, 0..250), redness(&b, 0..250));
    h.get_by_label("go to Supply");
}

#[test]
fn a_click_on_the_arrow_asks_to_go_there() {
    let (mut h, seen) = harness(false, calm(), true);
    h.run_steps(1);
    h.get_by_label("go to Supply").click();
    h.run_steps(6);
    assert!(seen.borrow().go);
}

#[test]
fn a_click_on_the_ring_asks_to_go_there() {
    let (mut h, seen) = harness(false, calm(), true);
    h.run_steps(3);
    let at = pos2(240.0, crest_y(&seen));
    assert!(at.y > 0.0 && at.y < SIZE.y - 30.0, "{at:?}");
    h.hover_at(at);
    h.drag_at(at);
    h.drop_at(at);
    h.run_steps(6);
    assert!(seen.borrow().go);
}
