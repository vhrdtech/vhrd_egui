//! Snap to a side (V3D-3) in the widget, without a GPU: a turn by the SpaceMouse or an orbit drag that lets go
//! finishes onto the nearest side; Alt keeps the free angle; a pan does not snap.

use egui::{Pos2, pos2, vec2};
use egui_kittest::Harness;
use glam::Vec3;
use ve_view3d::{Scene, SnapSettings, View3d, ViewPreset, angle_to_nearest, shapes};

fn harness(snap: SnapSettings) -> Harness<'static, View3d> {
    let mut view = View3d::without_gpu();
    let (lo, hi) = (Vec3::splat(-1.0), Vec3::splat(1.0));
    view.set_scene(Scene {
        meshes: vec![shapes::cuboid(lo, hi, egui::Color32::GRAY)],
        lines: vec![],
    });
    view.view(ViewPreset::Front);
    view.snap = snap;
    let mut h = Harness::builder()
        .with_size(vec2(320.0, 240.0))
        .with_step_dt(0.05)
        .build_ui_state(
            |ui, view: &mut View3d| {
                view.show(ui);
            },
            view,
        );
    h.run();
    h
}

fn settle(h: &mut Harness<'static, View3d>) {
    for _ in 0..20 {
        h.step();
    }
}

fn nav_turn(turn: f32) -> ve_spacemouse::Nav {
    ve_spacemouse::Nav {
        rotate: [0.0, 0.0, turn],
        ..Default::default()
    }
}

#[test]
fn the_puck_lets_go_and_the_view_lands_on_the_nearest_side() {
    let mut h = harness(SnapSettings::default());
    // Turn 0.6 rad/s for 0.5 s: about 17° off the front, then let go.
    for _ in 0..10 {
        h.state_mut().navigate(nav_turn(0.6));
        h.step();
    }
    let off = h.state().camera.rotation;
    assert!(angle_to_nearest(off) > 0.1, "turned off the side");
    h.state_mut().navigate(ve_spacemouse::Nav::default());
    h.step();
    assert!(h.state().snapping());
    settle(&mut h);
    assert!(!h.state().snapping());
    let q = h.state().camera.rotation;
    assert!(
        q.angle_between(ViewPreset::Front.rotation()) < 1e-4,
        "back on the front"
    );
}

#[test]
fn a_drag_lets_go_onto_a_side_alt_keeps_the_angle() {
    let mut h = harness(SnapSettings::default());
    let drag = |h: &mut Harness<'static, View3d>, to: Pos2, alt: bool| {
        let m = egui::Modifiers {
            alt,
            ..Default::default()
        };
        h.event(egui::Event::PointerMoved(pos2(160.0, 120.0)));
        h.event(egui::Event::PointerButton {
            pos: pos2(160.0, 120.0),
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: m,
        });
        h.step();
        for i in 1..=8 {
            let p = pos2(160.0, 120.0) + (to - pos2(160.0, 120.0)) * (i as f32 / 8.0);
            h.event(egui::Event::PointerMoved(p));
            h.step();
        }
        h.event(egui::Event::ModifiersChanged(m));
        h.event(egui::Event::PointerButton {
            pos: to,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: m,
        });
        h.step();
        h.event(egui::Event::ModifiersChanged(Default::default()));
        settle(h);
    };
    // 40 points sideways: about 18° about Z, back to the front.
    drag(&mut h, pos2(200.0, 120.0), false);
    assert!(
        h.state()
            .camera
            .rotation
            .angle_between(ViewPreset::Front.rotation())
            < 1e-4
    );
    // 140 points: past 45°, onto the next side (the right or the left).
    drag(&mut h, pos2(300.0, 120.0), false);
    let q = h.state().camera.rotation;
    assert!(angle_to_nearest(q) < 1e-4);
    assert!(q.angle_between(ViewPreset::Front.rotation()) > 1.5);
    // With Alt held: the free angle stays.
    drag(&mut h, pos2(340.0, 120.0), true);
    assert!(angle_to_nearest(h.state().camera.rotation) > 0.1);
}

#[test]
fn snap_off_keeps_every_angle() {
    let mut h = harness(SnapSettings::OFF);
    for _ in 0..10 {
        h.state_mut().navigate(nav_turn(0.6));
        h.step();
    }
    h.state_mut().navigate(ve_spacemouse::Nav::default());
    settle(&mut h);
    assert!(!h.state().snapping());
    assert!(angle_to_nearest(h.state().camera.rotation) > 0.1);
    // A preset still sits exactly on its side.
    h.state_mut().view(ViewPreset::Top);
    h.step();
    assert!(angle_to_nearest(h.state().camera.rotation) < 1e-4);
}
