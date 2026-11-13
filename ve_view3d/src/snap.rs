//! Snap to a side: when a turn by the SpaceMouse or the mouse lets go, the rotation finishes onto the nearest of
//! the 24 axis-aligned orientations, eased, so the view lands exactly on a drawing's view (V3D-3).
//!
//! The 24 orientations are the rotations that map each axis onto an axis: 6 directions to look from (top,
//! bottom, front, back, left, right), each with 4 ways up. They apply to any rotation, a camera's (where it looks
//! from) or a model's (how it is turned).

use glam::{Mat3, Quat, Vec3};
use serde::{Deserialize, Serialize};

/// The 24 axis-aligned rotations: every proper rotation whose matrix is a signed permutation.
pub fn axis_rotations() -> [Quat; 24] {
    let axes = [
        Vec3::X,
        Vec3::NEG_X,
        Vec3::Y,
        Vec3::NEG_Y,
        Vec3::Z,
        Vec3::NEG_Z,
    ];
    let mut out = [Quat::IDENTITY; 24];
    let mut n = 0;
    for x in axes {
        for y in axes {
            if x.dot(y) != 0.0 {
                continue;
            }
            // z = x × y keeps it a rotation (determinant +1), not a mirror.
            out[n] = Quat::from_mat3(&Mat3::from_cols(x, y, x.cross(y))).normalize();
            n += 1;
        }
    }
    debug_assert_eq!(n, 24);
    out
}

/// The axis-aligned rotation nearest to `q` (the smallest turn between them), on `q`'s side of the quaternion
/// sphere so a slerp from `q` to it takes the short way.
pub fn nearest_axis_rotation(q: Quat) -> Quat {
    let q = q.normalize();
    let best = axis_rotations()
        .into_iter()
        .max_by(|a, b| a.dot(q).abs().total_cmp(&b.dot(q).abs()))
        .unwrap_or(Quat::IDENTITY);
    if best.dot(q) < 0.0 { -best } else { best }
}

/// The angle in radians from `q` to its nearest axis-aligned rotation: 0 when it sits on one.
pub fn angle_to_nearest(q: Quat) -> f32 {
    q.normalize().angle_between(nearest_axis_rotation(q))
}

/// Whether and how fast a turn finishes onto a side. Serializable: keep it with the app's settings.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SnapSettings {
    /// Snap when a turn lets go; off: every angle stays where it was left.
    pub enabled: bool,
    /// Seconds the finishing turn takes.
    pub duration: f32,
}

impl Default for SnapSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            duration: 0.25,
        }
    }
}

impl SnapSettings {
    /// Off: no snap (the free angle).
    pub const OFF: SnapSettings = SnapSettings {
        enabled: false,
        duration: 0.25,
    };

    /// The settings as a checkbox and a time, each with its tooltip. Returns whether one changed.
    pub fn ui(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = ui
            .checkbox(&mut self.enabled, "Snap to a side on release")
            .on_hover_text(
                "When a turn by the SpaceMouse or the mouse lets go, finish it onto the nearest of the 24 \
                 axis-aligned orientations (top, bottom, front, back, left, right, each in 90° steps). Hold Alt \
                 while letting go to keep the free angle",
            )
            .changed();
        ui.add_enabled_ui(self.enabled, |ui| {
            ui.horizontal(|ui| {
                ui.label("Snap time");
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut self.duration)
                            .range(0.0..=2.0)
                            .speed(0.01)
                            .suffix(" s"),
                    )
                    .on_hover_text("How long the finishing turn takes; 0 jumps at once")
                    .on_disabled_hover_text("Snap is off: every angle stays where it was left")
                    .changed();
            });
        });
        changed
    }
}

/// A finishing turn under way: from where the turn let go to the nearest side, eased out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SnapAnim {
    from: Quat,
    to: Quat,
    start: f64,
    duration: f32,
}

impl SnapAnim {
    /// The turn from `from` to its nearest side, starting at `now` (seconds); `None` when `from` already sits
    /// there (within a hundredth of a degree).
    pub fn new(from: Quat, now: f64, duration: f32) -> Option<Self> {
        let from = from.normalize();
        let to = nearest_axis_rotation(from);
        (from.angle_between(to) > 1e-4).then_some(Self {
            from,
            to,
            start: now,
            duration: duration.max(0.0),
        })
    }

    /// The turn from `from` to `to` (a side picked, say), the short way, starting at `now`; `None` when they are
    /// the same.
    pub fn towards(from: Quat, to: Quat, now: f64, duration: f32) -> Option<Self> {
        let from = from.normalize();
        let to = to.normalize();
        let to = if to.dot(from) < 0.0 { -to } else { to };
        (from.angle_between(to) > 1e-4).then_some(Self {
            from,
            to,
            start: now,
            duration: duration.max(0.0),
        })
    }

    /// Where the turn finishes.
    pub fn target(&self) -> Quat {
        self.to
    }

    /// The rotation at `now`, and whether the turn is over (then it is exactly the target).
    pub fn at(&self, now: f64) -> (Quat, bool) {
        let t = if self.duration <= 0.0 {
            1.0
        } else {
            ((now - self.start) / self.duration as f64).clamp(0.0, 1.0) as f32
        };
        if t >= 1.0 {
            return (self.to, true);
        }
        // Ease out: fast at first, settling softly onto the side.
        let e = 1.0 - (1.0 - t).powi(3);
        (self.from.slerp(self.to, e).normalize(), false)
    }
}

/// Watches a turn and says when it lets go: feed it every frame whether something turned.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Release {
    turning: bool,
}

impl Release {
    /// `turning`: something turned this frame. Returns `true` on the first frame after a turn without one.
    pub fn update(&mut self, turning: bool) -> bool {
        let released = self.turning && !turning;
        self.turning = turning;
        released
    }

    /// A turn is under way.
    pub fn turning(&self) -> bool {
        self.turning
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::ViewPreset;

    #[test]
    fn there_are_24_distinct_proper_rotations() {
        let r = axis_rotations();
        for (i, a) in r.iter().enumerate() {
            let m = Mat3::from_quat(*a);
            assert!((m.determinant() - 1.0).abs() < 1e-5);
            for c in [m.x_axis, m.y_axis, m.z_axis] {
                // Each column is an axis: one entry ±1, the others 0.
                let a = c.abs();
                assert!(
                    (a.max_element() - 1.0).abs() < 1e-5 && (a.element_sum() - 1.0).abs() < 1e-5
                );
            }
            for b in &r[i + 1..] {
                assert!(a.angle_between(*b) > 1.0, "{a} and {b} are the same");
            }
        }
    }

    #[test]
    fn every_preset_but_iso_is_a_side() {
        for p in ViewPreset::ALL {
            let a = angle_to_nearest(p.rotation());
            if p == ViewPreset::Iso {
                assert!(a > 0.5, "iso is between sides");
            } else {
                assert!(a < 1e-3, "{p:?} is {a} off");
            }
        }
    }

    #[test]
    fn the_nearest_side_from_given_rotations() {
        let front = ViewPreset::Front.rotation();
        // 30° off about the view axis: back to front, the same way up.
        let off = front * Quat::from_rotation_z(30f32.to_radians());
        assert!(nearest_axis_rotation(off).angle_between(front) < 1e-4);
        // 50° about the view axis: past half way, the next way up (90°).
        let off = front * Quat::from_rotation_z(50f32.to_radians());
        let next = front * Quat::from_rotation_z(90f32.to_radians());
        assert!(nearest_axis_rotation(off).angle_between(next) < 1e-4);
        // Tilted 40° toward the top: still the front; 50°: the top.
        let tilt = |deg: f32| front * Quat::from_rotation_x(-deg.to_radians());
        assert!(nearest_axis_rotation(tilt(40.0)).angle_between(front) < 1e-4);
        let top = nearest_axis_rotation(tilt(50.0));
        assert!((top * Vec3::Z).distance(Vec3::Z) < 1e-4, "looks from above");
        // A mix of all three: the result is a side, and no side is nearer.
        let q = Quat::from_euler(glam::EulerRot::XYZ, 0.3, -0.7, 2.0);
        let n = nearest_axis_rotation(q);
        assert!(angle_to_nearest(n) < 1e-4);
        for r in axis_rotations() {
            assert!(q.angle_between(n) <= q.angle_between(r) + 1e-5);
        }
        // On the short way: the same hemisphere as q, for the slerp.
        assert!(nearest_axis_rotation(-off).dot(-off) >= 0.0);
    }

    #[test]
    fn the_turn_eases_out_and_lands_exactly() {
        let from = ViewPreset::Top.rotation() * Quat::from_rotation_z(0.2);
        let a = SnapAnim::new(from, 10.0, 0.5).unwrap();
        assert!(a.target().angle_between(ViewPreset::Top.rotation()) < 1e-4);
        let (q0, done) = a.at(10.0);
        assert!(!done && q0.angle_between(from) < 1e-5);
        // Ease out: more than half way at half the time.
        let (q, done) = a.at(10.25);
        assert!(!done);
        assert!(q.angle_between(a.target()) < 0.5 * from.angle_between(a.target()));
        let (q, done) = a.at(10.5);
        assert!(done);
        assert_eq!(q, a.target());
        // Already on a side: nothing to do. A zero time jumps.
        assert!(SnapAnim::new(ViewPreset::Left.rotation(), 0.0, 0.5).is_none());
        assert!(SnapAnim::new(from, 0.0, 0.0).unwrap().at(0.0).1);
        // Towards a side picked: lands on it, the short way.
        let to = ViewPreset::Front.rotation();
        let a = SnapAnim::towards(from, -to, 0.0, 0.2).unwrap();
        assert!(a.target().dot(from) >= 0.0);
        assert!(a.at(0.2).0.angle_between(to) < 1e-4);
        assert!(SnapAnim::towards(to, to, 0.0, 0.2).is_none());
    }

    #[test]
    fn release_fires_once_when_the_turn_stops() {
        let mut r = Release::default();
        assert!(!r.update(false));
        assert!(!r.update(true));
        assert!(r.turning());
        assert!(!r.update(true));
        assert!(r.update(false));
        assert!(!r.update(false));
    }
}
