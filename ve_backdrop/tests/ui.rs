//! Headless UI tests for ve_backdrop through egui_kittest (BACK-1).
//!
//! Both renderers draw the same scene into a wgpu test renderer: the shader through its paint callback, the
//! CPU painter as plain egui shapes. The tests check that each renders, that the two agree, that reduced
//! motion freezes the picture, and pixel-compare against `tests/snapshots/` with a tolerance (renderers
//! differ by a few pixels between PCs, DASH-21).
//! After an intended visual change: `UPDATE_SNAPSHOTS=1 cargo test -p ve_backdrop --test ui` and commit the PNGs.

use egui::{Color32, Rect, Stroke, StrokeKind, Theme, pos2, vec2};
use egui_kittest::kittest::Queryable;
use egui_kittest::wgpu::{WgpuTestRenderer, create_render_state, default_wgpu_setup};
use egui_kittest::{Harness, SnapshotOptions};
use image::RgbaImage;
use ve_backdrop::playground::Playground;
use ve_backdrop::{Backdrop, Lit, Mark, Renderer, Settings};

const SIZE: egui::Vec2 = vec2(480.0, 320.0);

/// A plain frame top left, a focused floating one bottom right.
fn frames() -> [Lit; 2] {
    [
        Lit::new(Rect::from_min_max(pos2(40.0, 40.0), pos2(200.0, 150.0))).corner_radius(6.0),
        Lit::new(Rect::from_min_max(pos2(250.0, 130.0), pos2(440.0, 280.0)))
            .corner_radius(6.0)
            .focused(true)
            .floating(true),
    ]
}

/// A harness that paints the backdrop over the whole window with the two frames outlined on top.
fn harness(shader: bool, theme: Theme, settings: Settings) -> Harness<'static> {
    let builder = Harness::builder().with_size(SIZE).with_theme(theme);
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
    assert_eq!(
        backdrop.renderer(),
        if shader {
            Renderer::Shader
        } else {
            Renderer::Painter
        }
    );
    let h = builder.build_ui(move |ui| {
        let rect = ui.ctx().content_rect();
        let lit = frames();
        backdrop.paint(ui.painter(), rect, &lit);
        let tokens = ve_theme::Tokens::of(ui.ctx(), theme);
        let painter = ui.painter().with_clip_rect(rect);
        for l in &lit {
            painter.rect_stroke(
                l.rect,
                l.corner_radius,
                Stroke::new(1.0, tokens.colors.line_strong),
                StrokeKind::Inside,
            );
        }
    });
    ve_theme::setup(&h.ctx);
    h
}

fn render(h: &mut Harness<'_>) -> RgbaImage {
    h.render().expect("the wgpu test renderer renders")
}

/// Mean absolute difference per colour channel, 0-255.
fn mean_diff(a: &RgbaImage, b: &RgbaImage) -> f64 {
    assert_eq!(a.dimensions(), b.dimensions());
    let sum: u64 = a
        .as_raw()
        .iter()
        .zip(b.as_raw())
        .map(|(x, y)| u64::from(x.abs_diff(*y)))
        .sum();
    sum as f64 / a.as_raw().len() as f64
}

/// How far the pixels of a region are from `bg` on average: the light in it.
fn light_in(image: &RgbaImage, region: Rect, bg: Color32) -> f64 {
    let mut sum = 0u64;
    let mut n = 0u64;
    for y in region.top() as u32..region.bottom() as u32 {
        for x in region.left() as u32..region.right() as u32 {
            let p = image.get_pixel(x, y);
            sum += (0..3)
                .map(|i| u64::from(p[i].abs_diff(bg.to_array()[i])))
                .sum::<u64>();
            n += 3;
        }
    }
    sum as f64 / n as f64
}

fn still() -> Settings {
    Settings::reduced()
}

fn tolerant() -> SnapshotOptions {
    SnapshotOptions::new().threshold(1.5).max_failed_pixels(60)
}

/// Marks are visible, and there is more light under the frames than in the empty corner, most under the
/// focused floating one.
fn assert_frames_gather_light(shader: bool) {
    let mut h = harness(shader, Theme::Dark, still());
    h.run();
    let image = render(&mut h);
    let bg = ve_theme::Tokens::dark().colors.bg;
    let [plain, focused] = frames().map(|l| l.rect.shrink(8.0));
    let empty = Rect::from_min_max(pos2(260.0, 8.0), pos2(470.0, 90.0));
    let (empty, plain, focused) = (
        light_in(&image, empty, bg),
        light_in(&image, plain, bg),
        light_in(&image, focused, bg),
    );
    assert!(empty > 0.05, "marks must be visible at rest: {empty}");
    assert!(
        plain > empty * 1.5 && focused > plain * 1.1,
        "light gathers under frames: empty {empty}, plain {plain}, focused {focused}"
    );
}

#[test]
fn fallback_painter_renders() {
    assert_frames_gather_light(false);
}

#[test]
fn shader_renders() {
    assert_frames_gather_light(true);
}

/// The shader and the CPU painter draw the same picture (their anti-aliasing differs a little).
#[test]
fn shader_and_painter_agree() {
    for mark in [Mark::Dots, Mark::Squares] {
        let settings = Settings {
            mark,
            size: 4.0,
            ..still()
        };
        let mut shader = harness(true, Theme::Dark, settings);
        let mut painter = harness(false, Theme::Dark, settings);
        shader.run();
        painter.run();
        let diff = mean_diff(&render(&mut shader), &render(&mut painter));
        assert!(diff < 0.6, "{mark:?}: mean channel difference {diff}");
    }
}

/// With reduced motion nothing moves: not with time, not with the pointer, not on a click.
#[test]
fn reduced_motion_freezes() {
    for shader in [false, true] {
        let mut h = harness(shader, Theme::Dark, still());
        h.run();
        let before = render(&mut h);
        h.hover_at(pos2(150.0, 220.0));
        h.run_steps(4);
        h.drag_at(pos2(150.0, 220.0));
        h.run_steps(2);
        h.drop_at(pos2(150.0, 220.0));
        h.remove_cursor();
        h.run_steps(4);
        let after = render(&mut h);
        assert_eq!(
            mean_diff(&before, &after),
            0.0,
            "shader {shader}: frozen picture changed"
        );
    }
}

/// The counterpart: with motion on, time, the pointer and a click all change the picture.
#[test]
fn motion_moves() {
    for shader in [false, true] {
        let mut h = harness(shader, Theme::Dark, Settings::default());
        h.run_steps(2);
        let start = render(&mut h);
        h.run_steps(8);
        let drifted = render(&mut h);
        assert!(
            mean_diff(&start, &drifted) > 0.001,
            "shader {shader}: the drift must move"
        );

        let bg = ve_theme::Tokens::dark().colors.bg;
        let around = Rect::from_center_size(pos2(120.0, 240.0), vec2(80.0, 80.0));
        let dark = light_in(&drifted, around, bg);
        h.hover_at(around.center());
        h.run_steps(1);
        // The harness draws a cursor over the pointer: look next to it.
        let beside = Rect::from_min_max(around.min, around.center());
        let (unlit, lit) = (
            light_in(&drifted, beside, bg),
            light_in(&render(&mut h), beside, bg),
        );
        assert!(
            lit > unlit * 1.2,
            "shader {shader}: the lens lights marks: {unlit} -> {lit} ({dark})"
        );
    }
}

#[test]
fn snapshot_shader_dark() {
    let mut h = harness(true, Theme::Dark, still());
    h.run();
    h.snapshot_options("backdrop_shader_dark", &tolerant());
}

#[test]
fn snapshot_shader_light() {
    let mut h = harness(true, Theme::Light, still());
    h.run();
    h.snapshot_options("backdrop_shader_light", &tolerant());
}

#[test]
fn snapshot_painter_squares_dark() {
    let settings = Settings {
        mark: Mark::Squares,
        size: 3.0,
        ..still()
    };
    let mut h = harness(false, Theme::Dark, settings);
    h.run();
    h.snapshot_options("backdrop_painter_squares_dark", &tolerant());
}

/// The pointer lens and a click ripple, at fixed harness time steps.
#[test]
fn snapshot_shader_pointer_and_ripple() {
    let mut h = harness(true, Theme::Dark, Settings::default());
    h.run_steps(2);
    h.drag_at(pos2(140.0, 230.0));
    h.run_steps(1);
    // Half a second later the ring has left the lens, with the pointer over the other frame.
    h.hover_at(pos2(300.0, 90.0));
    h.run_steps(2);
    h.snapshot_options("backdrop_shader_pointer", &tolerant());
}

/// The settings form: every slider is labelled and explains itself on hover; reduced motion disables the
/// motion controls instead of hiding them.
#[test]
fn settings_form_has_tooltips_and_toggles() {
    let mut h = Harness::builder()
        .with_size(vec2(360.0, 1100.0))
        .build_ui_state(
            |ui, settings: &mut Settings| {
                settings.ui(ui);
            },
            Settings::default(),
        );
    ve_theme::setup(&h.ctx);
    h.run();
    for label in [
        "Spacing",
        "Size",
        "Opacity",
        "Drift",
        "Speed",
        "Frame rate",
        "Wave length",
        "Lens radius",
        "Lens push",
        "Lens grow",
        "Lens light",
        "Ripple light",
        "Ripple grow",
        "Ripple push",
        "Ripple speed",
        "Ripple width",
        "Ripple life",
        "Glow reach",
        "Glow light",
        "Glow grow",
        "Focused",
        "Floating",
        "Glow fade",
    ] {
        assert!(
            h.query_all_by_label_contains(label).next().is_some(),
            "a control named {label:?}"
        );
    }
    h.get_by_label("Squares").click();
    h.get_by_label("Reduced motion").click();
    h.run();
    assert_eq!(h.state().mark, Mark::Squares);
    assert!(h.state().reduced_motion);

    h.get_by_label("Reduced motion").hover();
    h.run_steps(8);
    assert!(
        h.query_by_label_contains("freeze the backdrop").is_some(),
        "the checkbox explains itself on hover"
    );

    h.get_by_label("Reset to defaults").click();
    h.run();
    assert_eq!(*h.state(), Settings::default());
}

/// The playground: controls on the left, docked cards and floating windows lit on the backdrop.
#[test]
fn snapshot_playground() {
    let render_state = create_render_state(
        default_wgpu_setup(),
        egui_wgpu::RendererOptions::PREDICTABLE,
    );
    let mut playground = Playground::new(Backdrop::with_wgpu(&render_state).with_settings(still()));
    let mut h = Harness::builder()
        .with_size(vec2(1180.0, 760.0))
        .renderer(WgpuTestRenderer::from_render_state(render_state))
        .build_ui(move |ui| playground.ui(ui));
    ve_theme::setup(&h.ctx);
    h.run();
    for label in ["Shader", "CPU painter", "Windows", "Scope", "Docked card"] {
        assert!(
            h.query_all_by_label_contains(label).next().is_some(),
            "the playground shows {label:?}"
        );
    }
    h.snapshot_options("backdrop_playground", &tolerant().max_failed_pixels(400));
}
