//! The camera: orbit, pan, zoom about a point, fit, preset views, perspective or orthographic, and the SpaceMouse's
//! six axes (V3D-1).
//!
//! The world is Z up. The camera looks at `target` from `distance` along its own +Z (camera frame: x right, y up,
//! z toward the viewer, as in OpenGL), so `rotation` alone says where it looks from. Every motion is phrased the
//! way the person sees it: the content follows the pointer when dragged, and the point under the pointer stays
//! put while zooming.

use egui::{Pos2, Rect, Vec2, pos2};
use glam::{Mat3, Mat4, Quat, Vec3, Vec4};
use serde::{Deserialize, Serialize};

use crate::scene::Bounds;

/// Perspective (things farther away look smaller) or orthographic (parallel lines stay parallel; for measuring).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Projection {
    /// Perspective with [`Camera::fov_y`].
    #[default]
    Perspective,
    /// Orthographic, showing as much at the target as the perspective view would.
    Orthographic,
}

impl Projection {
    /// "perspective" or "orthographic".
    pub fn name(self) -> &'static str {
        match self {
            Projection::Perspective => "perspective",
            Projection::Orthographic => "orthographic",
        }
    }
}

/// A preset view direction. Z is up, front looks along +Y (from -Y), right looks along -X (from +X).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ViewPreset {
    /// From above: +X right, +Y up on the screen.
    Top,
    /// From below: +X right, -Y up.
    Bottom,
    /// From -Y: +X right, +Z up.
    Front,
    /// From +Y: -X right, +Z up.
    Back,
    /// From -X: -Y right, +Z up.
    Left,
    /// From +X: +Y right, +Z up.
    Right,
    /// From the front, right and above (+X, -Y, +Z), Z up.
    Iso,
}

impl ViewPreset {
    /// Every preset, in menu order.
    pub const ALL: [ViewPreset; 7] = [
        ViewPreset::Top,
        ViewPreset::Bottom,
        ViewPreset::Front,
        ViewPreset::Back,
        ViewPreset::Left,
        ViewPreset::Right,
        ViewPreset::Iso,
    ];

    /// A short name for menus: "top".
    pub fn name(self) -> &'static str {
        match self {
            ViewPreset::Top => "top",
            ViewPreset::Bottom => "bottom",
            ViewPreset::Front => "front",
            ViewPreset::Back => "back",
            ViewPreset::Left => "left",
            ViewPreset::Right => "right",
            ViewPreset::Iso => "iso",
        }
    }

    /// The camera rotation of this view.
    pub fn rotation(self) -> Quat {
        let (back, up) = match self {
            ViewPreset::Top => (Vec3::Z, Vec3::Y),
            ViewPreset::Bottom => (-Vec3::Z, -Vec3::Y),
            ViewPreset::Front => (-Vec3::Y, Vec3::Z),
            ViewPreset::Back => (Vec3::Y, Vec3::Z),
            ViewPreset::Left => (-Vec3::X, Vec3::Z),
            ViewPreset::Right => (Vec3::X, Vec3::Z),
            ViewPreset::Iso => (Vec3::new(1.0, -1.0, 1.0), Vec3::Z),
        };
        looking_from(back, up)
    }
}

/// The rotation of a camera that sits along `back` from its target, with `up_hint` as near its up as possible.
pub fn looking_from(back: Vec3, up_hint: Vec3) -> Quat {
    let back = back.normalize_or(Vec3::Z);
    let mut right = up_hint.cross(back);
    if right.length_squared() < 1e-12 {
        right = back.any_orthonormal_vector();
    }
    let right = right.normalize();
    let up = back.cross(right);
    Quat::from_mat3(&Mat3::from_cols(right, up, back)).normalize()
}

/// Where the camera is and how it projects. Serializable: keep it as view state.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Camera {
    /// The point looked at, which orbiting turns around.
    pub target: Vec3,
    /// From the target to the eye.
    pub distance: f32,
    /// Camera to world: the camera looks along its -Z, its +Y is up on the screen.
    pub rotation: Quat,
    /// The vertical field of view in radians (perspective); the orthographic view shows as much at the target.
    pub fov_y: f32,
    /// Perspective or orthographic.
    pub projection: Projection,
    /// Radians the content turns per point of drag.
    pub orbit_speed: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            target: Vec3::ZERO,
            distance: 10.0,
            rotation: ViewPreset::Iso.rotation(),
            fov_y: 35f32.to_radians(),
            projection: Projection::Perspective,
            orbit_speed: 0.008,
        }
    }
}

impl Camera {
    /// The eye position.
    pub fn eye(&self) -> Vec3 {
        self.target + self.back() * self.distance
    }

    /// The unit vector from the target toward the eye.
    pub fn back(&self) -> Vec3 {
        self.rotation * Vec3::Z
    }

    /// Screen right in the world.
    pub fn right(&self) -> Vec3 {
        self.rotation * Vec3::X
    }

    /// Screen up in the world.
    pub fn up(&self) -> Vec3 {
        self.rotation * Vec3::Y
    }

    /// Half the height the view shows at the target, in world units.
    pub fn half_height(&self) -> f32 {
        self.distance * (self.fov_y * 0.5).tan()
    }

    /// World to camera.
    pub fn view_matrix(&self) -> Mat4 {
        Mat4::from_rotation_translation(self.rotation, self.eye()).inverse()
    }

    /// Look at the scene's box from the current direction so all of it shows in a view of `aspect`
    /// (width / height), with some margin.
    pub fn fit(&mut self, bounds: &Bounds, aspect: f32) {
        if bounds.is_empty() {
            return;
        }
        let r = bounds.radius().max(1e-4) * 1.1;
        let aspect = aspect.max(1e-3);
        let tan_y = (self.fov_y * 0.5).tan();
        self.target = bounds.center();
        self.distance = match self.projection {
            // The narrower of the two fields of view decides.
            Projection::Perspective => {
                let tan_min = tan_y * aspect.min(1.0);
                r / tan_min.atan().sin()
            }
            Projection::Orthographic => r / (tan_y * aspect.min(1.0)),
        };
    }

    /// Orthographic at a fixed scale: one world unit is `points_per_unit` points on the screen in a view `height`
    /// points high (a model in mm over a drawing at the drawing's scale).
    pub fn set_ortho_scale(&mut self, points_per_unit: f32, height: f32) {
        self.projection = Projection::Orthographic;
        self.set_scale(points_per_unit, height);
    }

    /// A fixed scale at the target, the projection kept: one world unit at the target's depth is `points_per_unit`
    /// points on the screen in a view `height` points high. Orthographic, that holds everywhere; in perspective only
    /// on the plane through the target facing the camera, nearer things larger, farther ones smaller.
    pub fn set_scale(&mut self, points_per_unit: f32, height: f32) {
        let hh = 0.5 * height.max(1.0) / points_per_unit.max(1e-9);
        self.distance = hh / (self.fov_y * 0.5).tan();
    }

    /// A new vertical field of view (radians, held between [`Camera::FOV_MIN`] and [`Camera::FOV_MAX`]), the view's
    /// scale at the target kept: the eye moves in or out so the target's plane shows as much as before, and only
    /// the depth looks different.
    pub fn set_fov_y(&mut self, fov_y: f32) {
        let hh = self.half_height();
        self.fov_y = fov_y.clamp(Self::FOV_MIN, Self::FOV_MAX);
        self.distance = hh / (self.fov_y * 0.5).tan();
    }

    /// The narrowest field of view [`Camera::set_fov_y`] takes: 5°.
    pub const FOV_MIN: f32 = 5.0 * std::f32::consts::PI / 180.0;
    /// The widest field of view [`Camera::set_fov_y`] takes: 120°.
    pub const FOV_MAX: f32 = 120.0 * std::f32::consts::PI / 180.0;

    /// How much larger a point `depth` world units nearer the eye than the target looks than one at the target:
    /// `distance / (distance - depth)` in perspective, 1 orthographic.
    pub fn magnification(&self, depth: f32) -> f32 {
        match self.projection {
            Projection::Orthographic => 1.0,
            Projection::Perspective => self.distance / (self.distance - depth).max(1e-6),
        }
    }

    /// Points per world unit at the target in a view `height` points high (orthographic: everywhere).
    pub fn points_per_unit(&self, height: f32) -> f32 {
        0.5 * height / self.half_height().max(1e-9)
    }

    /// Look from a preset direction, then fit the box.
    pub fn view(&mut self, preset: ViewPreset, bounds: &Bounds, aspect: f32) {
        self.rotation = preset.rotation();
        self.fit(bounds, aspect);
    }

    /// Turn the content as the pointer drags it by `delta` points: sideways about the world's Z (turntable),
    /// up and down about the screen's horizontal axis.
    pub fn orbit(&mut self, delta: Vec2) {
        let yaw = Quat::from_rotation_z(-delta.x * self.orbit_speed);
        let pitch = Quat::from_rotation_x(-delta.y * self.orbit_speed);
        self.rotation = (yaw * self.rotation * pitch).normalize();
    }

    /// Move the content with the pointer by `delta` points in a view `height` points high (at the target's depth).
    pub fn pan(&mut self, delta: Vec2, height: f32) {
        let per_point = 2.0 * self.half_height() / height.max(1.0);
        self.target -= (self.right() * delta.x - self.up() * delta.y) * per_point;
    }

    /// Zoom by `factor` (above 1 zooms in) so `anchor` stays where it is on the screen.
    pub fn zoom_about(&mut self, anchor: Vec3, factor: f32) {
        if !(factor.is_finite() && factor > 0.0) {
            return;
        }
        self.target = anchor + (self.target - anchor) / factor;
        self.distance /= factor;
    }

    /// Turn the content about the target by a rotation given in the camera frame (x right, y up, z toward the
    /// viewer).
    pub fn rotate_content(&mut self, in_view: Quat) {
        self.rotation = (self.rotation * in_view.inverse()).normalize();
    }

    /// Apply the SpaceMouse for `dt` seconds in a view `height` points high: the [`ve_spacemouse::Nav`] already
    /// says what the content does (pan in screen points, growth, the turn of [`nav_turn`]), so the puck's
    /// convention setting is honoured as is.
    pub fn apply_nav(&mut self, nav: &ve_spacemouse::Nav, dt: f32, height: f32) {
        if nav.is_zero() {
            return;
        }
        self.pan(nav.pan_screen() * dt, height);
        self.zoom_about(self.target, (nav.zoom * dt).exp());
        self.rotate_content(nav_turn(nav, dt));
    }

    /// The projection for a view of `rect` on the screen, with clip planes around `bounds`.
    pub fn projector(&self, rect: Rect, bounds: &Bounds) -> Projector {
        let aspect = (rect.width() / rect.height().max(1.0)).max(1e-3);
        let (near, far) = self.clip(bounds);
        let proj = match self.projection {
            Projection::Perspective => perspective(self.fov_y, aspect, near, far),
            Projection::Orthographic => {
                let hh = self.half_height();
                let hw = hh * aspect;
                orthographic(hw, hh, near, far)
            }
        };
        let view = self.view_matrix();
        let view_proj = proj * view;
        Projector {
            rect,
            view_proj,
            inv_view_proj: view_proj.inverse(),
            eye: self.eye(),
            forward: -self.back(),
            orthographic: self.projection == Projection::Orthographic,
        }
    }

    /// Near and far clip distances that hold the box (or the target when it is empty).
    pub fn clip(&self, bounds: &Bounds) -> (f32, f32) {
        let (center, r) = if bounds.is_empty() {
            (self.target, self.distance * 0.5)
        } else {
            (bounds.center(), bounds.radius().max(1e-4))
        };
        let d = (center - self.eye()).dot(-self.back());
        let r = r * 1.05;
        let far = (d + r).max(r * 1e-3);
        match self.projection {
            Projection::Perspective => ((d - r).max(far * 1e-4), far),
            Projection::Orthographic => (d - r, far.max(d - r + 1e-4)),
        }
    }
}

/// The SpaceMouse's turn of the content over `dt` seconds, in the camera frame (x right, y up, z towards the
/// viewer) as [`Camera::rotate_content`] takes it: tilt forward about x (the top goes away), spin clockwise seen
/// from above about y (the right side comes towards you), turn clockwise on the screen about z (the right side goes
/// down). An app that turns its model itself takes this too, so every view turns the same way for the same motion.
pub fn nav_turn(nav: &ve_spacemouse::Nav, dt: f32) -> Quat {
    let [tilt, spin, turn] = nav.rotate.map(|r| r * dt);
    Quat::from_rotation_x(-tilt) * Quat::from_rotation_y(-spin) * Quat::from_rotation_z(-turn)
}

/// Right-handed perspective, depth 0 at `near` to 1 at `far` (wgpu's clip space).
fn perspective(fov_y: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
    let f = 1.0 / (fov_y * 0.5).tan();
    let r = far / (near - far);
    Mat4::from_cols(
        Vec4::new(f / aspect, 0.0, 0.0, 0.0),
        Vec4::new(0.0, f, 0.0, 0.0),
        Vec4::new(0.0, 0.0, r, -1.0),
        Vec4::new(0.0, 0.0, r * near, 0.0),
    )
}

/// Right-handed orthographic of half width `hw` and half height `hh`, depth 0 at `near` to 1 at `far`.
fn orthographic(hw: f32, hh: f32, near: f32, far: f32) -> Mat4 {
    let r = 1.0 / (near - far);
    Mat4::from_cols(
        Vec4::new(1.0 / hw, 0.0, 0.0, 0.0),
        Vec4::new(0.0, 1.0 / hh, 0.0, 0.0),
        Vec4::new(0.0, 0.0, r, 0.0),
        Vec4::new(0.0, 0.0, r * near, 1.0),
    )
}

/// A ray from the eye through a point on the screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ray {
    /// Where it starts (on the near plane).
    pub origin: Vec3,
    /// Unit direction.
    pub dir: Vec3,
}

impl Ray {
    /// The point `t` along the ray.
    pub fn at(&self, t: f32) -> Vec3 {
        self.origin + self.dir * t
    }

    /// Where the ray crosses the plane through `point` with `normal`, `None` when parallel.
    pub fn plane(&self, point: Vec3, normal: Vec3) -> Option<Vec3> {
        let denom = self.dir.dot(normal);
        (denom.abs() > 1e-9).then(|| self.at((point - self.origin).dot(normal) / denom))
    }
}

/// The camera fixed for one frame in one screen rectangle: world to screen and back.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Projector {
    /// The view on the screen, in points.
    pub rect: Rect,
    /// World to clip space (wgpu depth 0 to 1).
    pub view_proj: Mat4,
    /// Clip space to world.
    pub inv_view_proj: Mat4,
    /// The eye.
    pub eye: Vec3,
    /// The view direction.
    pub forward: Vec3,
    /// An orthographic projection: rays are parallel.
    pub orthographic: bool,
}

impl Projector {
    /// A world point on the screen and its depth (0 near, 1 far); `None` behind the eye.
    pub fn project(&self, p: Vec3) -> Option<(Pos2, f32)> {
        let c = self.view_proj * p.extend(1.0);
        if c.w <= 1e-9 {
            return None;
        }
        let n = c.truncate() / c.w;
        Some((
            pos2(
                self.rect.min.x + (n.x + 1.0) * 0.5 * self.rect.width(),
                self.rect.min.y + (1.0 - n.y) * 0.5 * self.rect.height(),
            ),
            n.z,
        ))
    }

    /// The ray through a screen point.
    pub fn ray(&self, pos: Pos2) -> Ray {
        let x = (pos.x - self.rect.min.x) / self.rect.width().max(1.0) * 2.0 - 1.0;
        let y = 1.0 - (pos.y - self.rect.min.y) / self.rect.height().max(1.0) * 2.0;
        let unproject = |z: f32| {
            let w = self.inv_view_proj * Vec4::new(x, y, z, 1.0);
            w.truncate() / w.w
        };
        let (near, far) = (unproject(0.0), unproject(1.0));
        Ray {
            origin: near,
            dir: (far - near).normalize_or(self.forward),
        }
    }

    /// The distance from the eye along the view direction (what the depth test compares).
    pub fn view_depth(&self, p: Vec3) -> f32 {
        (p - self.eye).dot(self.forward)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::vec2;

    fn rect() -> Rect {
        Rect::from_min_size(pos2(10.0, 20.0), vec2(400.0, 300.0))
    }

    fn cube() -> Bounds {
        Bounds::from_points([Vec3::splat(-1.0), Vec3::splat(1.0)])
    }

    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1e-4
    }

    #[test]
    fn presets_look_the_way_they_say() {
        let c = |p: ViewPreset| Camera {
            rotation: p.rotation(),
            ..Default::default()
        };
        let top = c(ViewPreset::Top);
        assert!(
            close(top.back(), Vec3::Z) && close(top.right(), Vec3::X) && close(top.up(), Vec3::Y)
        );
        let front = c(ViewPreset::Front);
        assert!(close(front.back(), -Vec3::Y) && close(front.right(), Vec3::X));
        assert!(close(front.up(), Vec3::Z));
        let right = c(ViewPreset::Right);
        assert!(close(right.back(), Vec3::X) && close(right.right(), Vec3::Y));
        let left = c(ViewPreset::Left);
        assert!(close(left.right(), -Vec3::Y) && close(left.up(), Vec3::Z));
        let bottom = c(ViewPreset::Bottom);
        assert!(close(bottom.back(), -Vec3::Z) && close(bottom.right(), Vec3::X));
        let iso = c(ViewPreset::Iso);
        assert!(close(iso.back(), Vec3::new(1.0, -1.0, 1.0).normalize()));
        assert!(iso.up().z > 0.5 && iso.right().z.abs() < 1e-5);
    }

    #[test]
    fn project_and_ray_agree() {
        for projection in [Projection::Perspective, Projection::Orthographic] {
            let mut cam = Camera {
                projection,
                ..Default::default()
            };
            cam.fit(&cube(), 4.0 / 3.0);
            let pr = cam.projector(rect(), &cube());
            let p = Vec3::new(0.3, -0.2, 0.7);
            let (s, depth) = pr.project(p).expect("in front");
            assert!((0.0..=1.0).contains(&depth));
            let ray = pr.ray(s);
            let t = (p - ray.origin).dot(ray.dir);
            assert!(close(ray.at(t), p), "{projection:?}");
            assert!(ray.dir.dot(pr.forward) > 0.0);
        }
    }

    #[test]
    fn fit_shows_every_corner() {
        for projection in [Projection::Perspective, Projection::Orthographic] {
            for preset in ViewPreset::ALL {
                for aspect_rect in [rect(), Rect::from_min_size(Pos2::ZERO, vec2(100.0, 500.0))] {
                    let mut cam = Camera {
                        projection,
                        ..Default::default()
                    };
                    let b =
                        Bounds::from_points([Vec3::new(5.0, 6.0, 7.0), Vec3::new(9.0, 7.0, 7.5)]);
                    cam.view(preset, &b, aspect_rect.aspect_ratio());
                    let pr = cam.projector(aspect_rect, &b);
                    for i in 0..8 {
                        let corner = Vec3::new(
                            if i & 1 == 0 { b.min.x } else { b.max.x },
                            if i & 2 == 0 { b.min.y } else { b.max.y },
                            if i & 4 == 0 { b.min.z } else { b.max.z },
                        );
                        let (s, depth) = pr.project(corner).expect("in front");
                        assert!(aspect_rect.contains(s), "{projection:?} {preset:?} {s:?}");
                        assert!(
                            (0.0..=1.0).contains(&depth),
                            "{projection:?} {preset:?} depth {depth}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn zoom_keeps_the_anchor_on_screen() {
        for projection in [Projection::Perspective, Projection::Orthographic] {
            let mut cam = Camera {
                projection,
                ..Default::default()
            };
            cam.fit(&cube(), 4.0 / 3.0);
            let anchor = Vec3::new(0.8, 0.1, -0.3);
            let before = cam.projector(rect(), &cube()).project(anchor).unwrap().0;
            let other = cam
                .projector(rect(), &cube())
                .project(Vec3::ZERO)
                .unwrap()
                .0;
            cam.zoom_about(anchor, 1.7);
            let pr = cam.projector(rect(), &cube());
            let after = pr.project(anchor).unwrap().0;
            assert!(
                (before - after).length() < 1e-2,
                "{projection:?} {before:?} {after:?}"
            );
            // Everything else moves away from the anchor: zoomed in.
            let other_after = pr.project(Vec3::ZERO).unwrap().0;
            assert!((other_after - after).length() > (other - before).length() * 1.5);
        }
    }

    /// A new field of view or the projection switched keeps the size at the target; `set_scale` keeps the
    /// projection, and in perspective a nearer point looks larger by `magnification`
    #[test]
    fn the_scale_at_the_target_holds() {
        let r = rect();
        let mut cam = Camera {
            target: Vec3::new(1.0, 2.0, 0.0),
            ..Default::default()
        };
        cam.set_scale(20.0, r.height());
        assert_eq!(cam.projection, Projection::Perspective);
        let at = |cam: &Camera, p: Vec3| cam.projector(r, &cube()).project(p).unwrap().0;
        let x = cam.right();
        let span = |cam: &Camera| (at(cam, cam.target + x) - at(cam, cam.target)).length();
        assert!((span(&cam) - 20.0).abs() < 1e-2, "{}", span(&cam));
        for deg in [10.0f32, 60.0, 100.0] {
            cam.set_fov_y(deg.to_radians());
            assert!((span(&cam) - 20.0).abs() < 1e-2, "{deg}°: {}", span(&cam));
            // Nearer the eye by 1: larger by the magnification
            let near = cam.target + cam.back();
            let s = (at(&cam, near + x) - at(&cam, near)).length();
            assert!(
                (s / 20.0 - cam.magnification(1.0)).abs() < 1e-3,
                "{deg}°: {s}"
            );
            assert!(cam.magnification(1.0) > 1.0);
        }
        cam.set_fov_y(500.0);
        assert_eq!(cam.fov_y, Camera::FOV_MAX);
        cam.projection = Projection::Orthographic;
        assert!((span(&cam) - 20.0).abs() < 1e-2);
        assert_eq!(cam.magnification(1.0), 1.0);
    }

    #[test]
    fn pan_moves_the_content_with_the_pointer() {
        for projection in [Projection::Perspective, Projection::Orthographic] {
            let mut cam = Camera {
                projection,
                ..Default::default()
            };
            cam.fit(&cube(), 4.0 / 3.0);
            let before = cam
                .projector(rect(), &cube())
                .project(cam.target)
                .unwrap()
                .0;
            let target = cam.target;
            cam.pan(vec2(30.0, -12.0), rect().height());
            let after = cam.projector(rect(), &cube()).project(target).unwrap().0;
            assert!(
                ((after - before) - vec2(30.0, -12.0)).length() < 1e-2,
                "{projection:?}"
            );
        }
    }

    #[test]
    fn dragging_right_turns_the_front_to_the_right() {
        let mut cam = Camera {
            rotation: ViewPreset::Front.rotation(),
            ..Default::default()
        };
        cam.fit(&cube(), 1.0);
        let front_point = Vec3::new(0.0, -1.0, 0.0);
        let x0 = cam
            .projector(rect(), &cube())
            .project(front_point)
            .unwrap()
            .0
            .x;
        cam.orbit(vec2(40.0, 0.0));
        let x1 = cam
            .projector(rect(), &cube())
            .project(front_point)
            .unwrap()
            .0
            .x;
        assert!(x1 > x0 + 5.0, "{x0} -> {x1}");
        // Dragging down brings the top toward the viewer: the front face's centre goes down the screen.
        let mut cam = Camera {
            rotation: ViewPreset::Front.rotation(),
            ..Default::default()
        };
        cam.fit(&cube(), 1.0);
        let y0 = cam
            .projector(rect(), &cube())
            .project(front_point)
            .unwrap()
            .0
            .y;
        cam.orbit(vec2(0.0, 40.0));
        let y1 = cam
            .projector(rect(), &cube())
            .project(front_point)
            .unwrap()
            .0
            .y;
        assert!(y1 > y0 + 5.0, "{y0} -> {y1}");
    }

    /// The puck's outputs say what the content does; the camera turns that into the motion the person expects.
    #[test]
    fn spacemouse_moves_the_content_as_its_outputs_say() {
        let start = || {
            let mut cam = Camera {
                rotation: ViewPreset::Front.rotation(),
                projection: Projection::Orthographic,
                ..Default::default()
            };
            cam.fit(&cube(), 1.0);
            cam
        };
        let r = Rect::from_min_size(Pos2::ZERO, vec2(300.0, 300.0));
        let nav = |pan: Vec2, zoom: f32, rotate: [f32; 3]| ve_spacemouse::Nav { pan, zoom, rotate };
        let screen = |cam: &Camera, p: Vec3| cam.projector(r, &cube()).project(p).unwrap();

        // Move right and up (Nav's y is up): the target moves right and up on the screen.
        let mut cam = start();
        cam.apply_nav(&nav(vec2(100.0, 50.0), 0.0, [0.0; 3]), 0.5, r.height());
        let (s, _) = screen(&cam, Vec3::ZERO);
        assert!(
            (s - pos2(150.0 + 50.0, 150.0 - 25.0)).length() < 1e-2,
            "{s:?}"
        );

        // Grow: the cube's corner moves away from the centre.
        let mut cam = start();
        let corner = Vec3::new(1.0, -1.0, 1.0);
        let before = (screen(&cam, corner).0 - pos2(150.0, 150.0)).length();
        cam.apply_nav(&nav(Vec2::ZERO, 1.0, [0.0; 3]), 0.5, r.height());
        assert!((screen(&cam, corner).0 - pos2(150.0, 150.0)).length() > before * 1.5);

        // Tilt forward: the top (+Z, up on the screen) goes away from the viewer.
        let mut cam = start();
        let top = Vec3::Z;
        let d0 = screen(&cam, top).1;
        cam.apply_nav(&nav(Vec2::ZERO, 0.0, [0.5, 0.0, 0.0]), 0.5, r.height());
        assert!(screen(&cam, top).1 > d0 + 1e-3);

        // Spin clockwise seen from above: the right side (+X) comes towards the viewer.
        let mut cam = start();
        let right = Vec3::X;
        let d0 = screen(&cam, right).1;
        cam.apply_nav(&nav(Vec2::ZERO, 0.0, [0.0, 0.5, 0.0]), 0.5, r.height());
        assert!(screen(&cam, right).1 < d0 - 1e-3);

        // Turn clockwise: the top point moves right on the screen.
        let mut cam = start();
        let x0 = screen(&cam, top).0.x;
        cam.apply_nav(&nav(Vec2::ZERO, 0.0, [0.0, 0.0, 0.5]), 0.5, r.height());
        assert!(screen(&cam, top).0.x > x0 + 5.0);
    }
}
