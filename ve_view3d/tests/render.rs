//! Headless renders of ve_view3d through egui_kittest on a real GPU (V3D-1): the picture, hover and selection
//! highlights, edges over faces, marks, the view menu, a big mesh, and a snapshot.
//! After an intended visual change: `UPDATE_SNAPSHOTS=1 cargo test -p ve_view3d --test render` and commit the PNGs.

use egui::{Color32, Pos2, pos2, vec2};
use egui_kittest::kittest::Queryable;
use egui_kittest::wgpu::{WgpuTestRenderer, create_render_state, default_wgpu_setup};
use egui_kittest::{Harness, SnapshotOptions};
use glam::Vec3;
use image::RgbaImage;
use ve_view3d::{Item, Mark, Scene, View3d, ViewPreset, shapes};

const SIZE: egui::Vec2 = vec2(320.0, 240.0);
const FACE: Color32 = Color32::from_rgb(200, 60, 40);

fn cube_scene() -> Scene {
    let (lo, hi) = (Vec3::splat(-1.0), Vec3::splat(1.0));
    let mut edges = shapes::cuboid_edges(lo, hi);
    edges.color = Color32::from_rgb(10, 10, 10);
    edges.width = 2.0;
    Scene {
        meshes: vec![shapes::cuboid(lo, hi, FACE)],
        lines: vec![edges],
    }
}

/// A harness whose state is the view, shown over the whole window, looking at `scene` from `preset`.
fn harness(scene: Scene, preset: ViewPreset) -> Harness<'static, View3d> {
    let rs = create_render_state(
        default_wgpu_setup(),
        egui_wgpu::RendererOptions::PREDICTABLE,
    );
    let mut view = View3d::new(&rs);
    assert!(view.has_gpu());
    view.set_scene(scene);
    view.view(preset);
    let mut h = Harness::builder()
        .with_size(SIZE)
        .with_theme(egui::Theme::Dark)
        .renderer(WgpuTestRenderer::from_render_state(rs))
        .build_ui_state(
            |ui, view: &mut View3d| {
                egui::Frame::NONE.show(ui, |ui| {
                    view.show(ui);
                });
            },
            view,
        );
    h.run();
    h
}

fn pixel(img: &RgbaImage, p: Pos2) -> [u8; 4] {
    img.get_pixel(p.x as u32, p.y as u32).0
}

fn dist(a: [u8; 4], b: [u8; 4]) -> i32 {
    (0..3).map(|i| (a[i] as i32 - b[i] as i32).abs()).sum()
}

/// A world point on the screen (1 pixel per point).
fn screen(h: &Harness<'_, View3d>, p: Vec3) -> Pos2 {
    let rect = h.state().rect().expect("shown");
    h.state().projector(rect).project(p).expect("in front").0
}

/// A spot inside the view beside the model.
fn empty(h: &Harness<'_, View3d>) -> Pos2 {
    h.state().rect().expect("shown").min + vec2(6.0, 6.0)
}

fn click(h: &mut Harness<'_, View3d>, pos: Pos2, modifiers: egui::Modifiers) {
    h.event(egui::Event::ModifiersChanged(modifiers));
    h.hover_at(pos);
    h.run();
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers,
        });
    }
    h.run();
    h.event(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
}

#[test]
fn the_model_renders_with_its_colour_and_edges() {
    let mut h = harness(cube_scene(), ViewPreset::Front);
    let img = h.render().expect("render");
    let centre = screen(&h, Vec3::new(0.0, -1.0, 0.0));
    let face = pixel(&img, centre);
    // Head-on, the headlight gives the face about its own colour (plus a faint specular).
    assert!(
        dist(face, [FACE.r(), FACE.g(), FACE.b(), 255]) < 70,
        "face {face:?}"
    );
    // Outside the model the window's background shows through the transparent clear.
    let bg = pixel(&img, empty(&h));
    assert!(dist(bg, face) > 100, "background {bg:?}");
    // The front face's bottom edge is drawn dark over the face.
    let edge = pixel(
        &img,
        screen(&h, Vec3::new(0.2, -1.0, -1.0)) + vec2(0.0, -0.5),
    );
    assert!(edge[0] < 80 && edge[1] < 80, "edge {edge:?}");
}

#[test]
fn hover_and_selection_highlight() {
    let mut h = harness(cube_scene(), ViewPreset::Iso);
    let front_face = screen(&h, Vec3::new(0.2, -1.0, 0.1));
    let plain = pixel(&h.render().expect("render"), front_face);

    h.hover_at(front_face);
    h.run();
    let hovered = h.state().hovered().expect("over the cube");
    assert_eq!(hovered.item(), Some(Item::Face { mesh: 0, face: 2 }));
    let lit = pixel(&h.render().expect("render"), front_face);
    assert!(
        dist(plain, lit) > 30,
        "hover changes the face: {plain:?} -> {lit:?}"
    );

    click(&mut h, front_face, egui::Modifiers::NONE);
    assert_eq!(h.state().selection, vec![Item::Face { mesh: 0, face: 2 }]);
    // Ctrl+click on the top face adds it; a plain click on empty space clears.
    let top = screen(&h, Vec3::new(0.1, 0.1, 1.0));
    click(&mut h, top, egui::Modifiers::COMMAND);
    assert_eq!(
        h.state().selection,
        vec![
            Item::Face { mesh: 0, face: 2 },
            Item::Face { mesh: 0, face: 5 }
        ]
    );
    h.hover_at(empty(&h));
    h.run();
    let selected = pixel(&h.render().expect("render"), front_face);
    assert!(
        dist(plain, selected) > 30,
        "selection shows without hover: {plain:?} -> {selected:?}"
    );
    let beside = empty(&h);
    click(&mut h, beside, egui::Modifiers::NONE);
    assert!(h.state().selection.is_empty());
}

#[test]
fn an_edge_under_the_pointer_wins_over_the_face() {
    let mut h = harness(cube_scene(), ViewPreset::Iso);
    let on_edge = screen(&h, Vec3::new(0.3, -1.0, 1.0));
    click(&mut h, on_edge + vec2(0.0, 1.0), egui::Modifiers::NONE);
    let sel = h.state().selection.clone();
    assert_eq!(sel.len(), 1);
    assert!(matches!(sel[0], Item::Edge { set: 0, .. }), "{sel:?}");
}

#[test]
fn marks_draw_over_the_model() {
    let mut h = harness(cube_scene(), ViewPreset::Front);
    let mark_color = Color32::from_rgb(40, 220, 90);
    // A point hidden behind the cube still shows: marks are an overlay.
    let behind = Vec3::new(0.5, 1.0, 0.5);
    h.state_mut().marks = vec![
        Mark::point(behind, "").color(mark_color),
        Mark::segment(
            Vec3::new(-1.0, -1.0, -1.0),
            Vec3::new(1.0, -1.0, -1.0),
            "2.000 mm",
        ),
    ];
    h.run();
    let img = h.render().expect("render");
    let at = pixel(&img, screen(&h, behind));
    assert!(
        dist(at, [mark_color.r(), mark_color.g(), mark_color.b(), 255]) < 40,
        "mark {at:?}"
    );
}

#[test]
fn the_view_menu_changes_projection_and_view() {
    let mut h = harness(cube_scene(), ViewPreset::Iso);
    h.event(egui::Event::PointerButton {
        pos: pos2(160.0, 120.0),
        button: egui::PointerButton::Secondary,
        pressed: true,
        modifiers: Default::default(),
    });
    h.event(egui::Event::PointerButton {
        pos: pos2(160.0, 120.0),
        button: egui::PointerButton::Secondary,
        pressed: false,
        modifiers: Default::default(),
    });
    h.run();
    h.get_by_label("Orthographic").click();
    h.run();
    assert_eq!(
        h.state().camera.projection,
        ve_view3d::Projection::Orthographic
    );
    // Disabled things say why: nothing is selected yet.
    assert!(
        h.query_by_label("Clear selection").is_none(),
        "the menu closed"
    );
}

/// The largest model seen has 176,000 triangles; it uploads, renders and picks through the BVH.
#[test]
fn a_big_mesh_renders_and_picks() {
    let sphere = shapes::uv_sphere(Vec3::ZERO, 1.0, 420, 210, Color32::LIGHT_BLUE);
    assert!(sphere.triangles.len() >= 176_000);
    let scene = Scene {
        meshes: vec![sphere],
        lines: vec![],
    };
    let mut h = harness(scene, ViewPreset::Front);
    assert!(h.state().picker().has_bvh(0));
    let centre = h.state().rect().expect("shown").center();
    h.hover_at(centre);
    h.run();
    let p = h.state().hovered().expect("the sphere");
    assert!((p.point.length() - 1.0).abs() < 1e-2);
    let img = h.render().expect("render");
    // Beside the pointer: kittest draws the cursor into the picture.
    let px = pixel(&img, centre - vec2(20.0, 10.0));
    assert!(px[2] > 150, "{px:?}");
}

#[test]
fn snapshot_of_the_iso_view() {
    let mut scene = cube_scene();
    scene.meshes.push(shapes::cylinder(
        Vec3::new(1.6, 0.0, -1.0),
        0.5,
        1.5,
        48,
        shapes::PALETTE[2],
    ));
    scene.lines.push(shapes::cylinder_edges(
        Vec3::new(1.6, 0.0, -1.0),
        0.5,
        1.5,
        48,
    ));
    let mut h = harness(scene, ViewPreset::Iso);
    h.state_mut().marks = vec![Mark::segment(
        Vec3::new(-1.0, -1.0, 1.0),
        Vec3::new(1.0, -1.0, 1.0),
        "2.000",
    )];
    h.run();
    // Renderers differ by a few pixels between GPUs.
    h.snapshot_options(
        "iso",
        &SnapshotOptions::new().threshold(2.0).max_failed_pixels(400),
    );
}

/// The overlay at a fixed scale (V3D-3): a 2-unit cube at 20 points per unit, seen from the front, spans 40
/// points about the target's spot, painted see-through with the tint.
#[test]
fn the_overlay_draws_at_its_scale() {
    let rs = create_render_state(
        default_wgpu_setup(),
        egui_wgpu::RendererOptions::PREDICTABLE,
    );
    let mut view = View3d::new(&rs);
    view.set_scene(cube_scene());
    view.camera.rotation = ViewPreset::Front.rotation();
    view.camera.target = Vec3::ZERO;
    let mut h = Harness::builder()
        .with_size(SIZE)
        .with_theme(egui::Theme::Dark)
        .renderer(WgpuTestRenderer::from_render_state(rs))
        .build_ui_state(
            |ui, view: &mut View3d| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE.fill(Color32::WHITE))
                    .show(ui, |ui| {
                        let rect = ui.max_rect();
                        view.camera.set_ortho_scale(20.0, rect.height());
                        let painter = ui.painter_at(rect);
                        view.paint_overlay(ui, &painter, rect, Color32::WHITE);
                    });
            },
            view,
        );
    h.run();
    let rect = h.state().rect().expect("painted");
    assert!((h.state().camera.points_per_unit(rect.height()) - 20.0).abs() < 1e-3);
    let img = h.render().expect("render");
    let c = rect.center();
    let is_face = |p: Pos2| dist(pixel(&img, p), [FACE.r(), FACE.g(), FACE.b(), 255]) < 90;
    let is_white = |p: Pos2| dist(pixel(&img, p), [255, 255, 255, 255]) < 30;
    assert!(is_face(c), "the face at the centre: {:?}", pixel(&img, c));
    // The face's right edge 20 points right of the centre: inside at 17, outside at 23.
    assert!(!is_white(c + vec2(17.0, 0.0)));
    assert!(
        is_white(c + vec2(23.0, 0.0)),
        "{:?}",
        pixel(&img, c + vec2(23.0, 0.0))
    );
    assert!(is_white(c + vec2(0.0, -23.0)));
    assert!(!is_white(c + vec2(0.0, -17.0)));
    // Rendered without showing, for an app's own pass: a texture and its wgpu view of the size asked.
    let ctx = h.ctx.clone();
    let (_, view) = h
        .state_mut()
        .render_offscreen(&ctx, vec2(40.0, 30.0))
        .expect("a picture");
    let size = view.texture().size();
    assert_eq!([size.width, size.height], [40, 30]);
}
