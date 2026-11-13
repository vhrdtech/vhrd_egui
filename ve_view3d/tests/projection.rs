//! Orthographic or perspective and the field of view (V3D-5) in the widget's control, as a person uses it: the
//! switch picks the projection, the field of view is there but disabled in orthographic, and neither changes the
//! model's size at the target.

use egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use ve_view3d::{Camera, Projection, View3d};

/// The field of view's value can't be changed: its drag value is disabled
fn fov_disabled(h: &Harness<'static, View3d>) -> bool {
    use egui_kittest::kittest::NodeT;
    h.get_by_role(egui::accesskit::Role::SpinButton)
        .accesskit_node()
        .is_disabled()
}

#[test]
fn the_switch_and_the_field_of_view() {
    let mut view = View3d::without_gpu();
    view.camera.projection = Projection::Orthographic;
    view.camera.set_scale(20.0, 400.0);
    let mut h = Harness::builder()
        .with_size(vec2(360.0, 120.0))
        .build_ui_state(
            |ui, view: &mut View3d| {
                view.projection_ui(ui);
            },
            view,
        );
    h.run();
    // Orthographic: the field of view shown, disabled
    assert!(fov_disabled(&h));
    h.get_by_label("Perspective").click();
    h.run();
    assert_eq!(h.state().camera.projection, Projection::Perspective);
    assert!(!fov_disabled(&h));
    let scale = |c: &Camera| c.points_per_unit(400.0);
    assert!((scale(&h.state().camera) - 20.0).abs() < 1e-3);
    // A wider field of view: the eye nearer, the scale at the target the same
    let d = h.state().camera.distance;
    h.state_mut().camera.set_fov_y(70f32.to_radians());
    h.run();
    assert!(h.state().camera.distance < d);
    assert!((scale(&h.state().camera) - 20.0).abs() < 1e-3);
    h.get_by_label("Orthographic").click();
    h.run();
    assert_eq!(h.state().camera.projection, Projection::Orthographic);
}
