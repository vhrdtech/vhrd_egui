//! A measurement between two picks: the offset along X, Y and Z, the distance, and what the two faces say about
//! it (V3D-2): two parallel flat faces give their gap along the normal, two points on one cylindrical face its
//! diameter.

use glam::Vec3;

use crate::pick::Pick;
use crate::scene::{Mesh, Scene};

/// Normals closer than this angle (radians) count as parallel: 0.5°.
const PARALLEL: f32 = 0.5 * std::f32::consts::PI / 180.0;
/// How far a face's normal may lean off the plane square to a cylinder's axis, as a cosine: about 1.5°.
const ON_AXIS: f32 = 0.026;
/// The two picks' normals must differ by at least this (the length of their difference, about 11°) to find a
/// diameter; closer, the radius is too uncertain.
const SPREAD: f32 = 0.2;
/// The two centres a cylinder fit finds may be this share of the radius apart.
const FIT: f32 = 0.02;

/// What the two picked faces tell about the distance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Relation {
    /// Two flat faces facing the same or the opposite way: the gap between their planes along the normal.
    ParallelFaces {
        /// The gap, in the scene's unit.
        gap: f32,
        /// The planes' normal (the first pick's).
        normal: Vec3,
    },
    /// Two points on one cylindrical face: its diameter.
    Cylinder {
        /// The diameter, in the scene's unit.
        diameter: f32,
        /// The cylinder's axis direction.
        axis: Vec3,
    },
}

/// From the first pick to the second.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measurement {
    /// The first point.
    pub from: Vec3,
    /// The second point.
    pub to: Vec3,
    /// `to - from`, signed, in the model's axes.
    pub delta: Vec3,
    /// The straight distance.
    pub distance: f32,
    /// What the faces say, when they say something.
    pub relation: Option<Relation>,
}

impl Measurement {
    /// Measure from `a` to `b`; `scene` is where they were picked, for the faces' shapes.
    pub fn between(a: &Pick, b: &Pick, scene: &Scene) -> Self {
        let delta = b.point - a.point;
        Self {
            from: a.point,
            to: b.point,
            delta,
            distance: delta.length(),
            relation: relation(a, b, scene),
        }
    }

    /// One line, as the footprint review's ruler writes it: `12.345 mm · dx 1.000 · dy -2.000 · dz 0.500`.
    pub fn line(&self, unit: &str) -> String {
        let d = self.delta;
        format!(
            "{:.3} {unit} · dx {:.3} · dy {:.3} · dz {:.3}",
            self.distance, d.x, d.y, d.z
        )
    }

    /// What the faces say, for a second line: `parallel faces 1.600 mm apart`, `cylinder Ø 4.000 mm`.
    pub fn relation_line(&self, unit: &str) -> Option<String> {
        self.relation.map(|r| match r {
            Relation::ParallelFaces { gap, .. } => format!("parallel faces {gap:.3} {unit} apart"),
            Relation::Cylinder { diameter, .. } => format!("cylinder Ø {diameter:.3} {unit}"),
        })
    }

    /// The label for the segment in the view: the distance, or the relation's number when there is one.
    pub fn label(&self, unit: &str) -> String {
        match self.relation {
            Some(Relation::ParallelFaces { gap, .. }) => format!("{gap:.3} {unit} ⟂"),
            Some(Relation::Cylinder { diameter, .. }) => format!("Ø {diameter:.3} {unit}"),
            None => format!("{:.3} {unit}", self.distance),
        }
    }
}

/// The normal at a picked point: the vertex normals blended across the triangle, else the triangle's own.
fn normal_at(mesh: &Mesh, triangle: usize, point: Vec3) -> Option<Vec3> {
    let [p0, p1, p2] = mesh.corners(triangle)?;
    let flat = (p1 - p0).cross(p2 - p0).try_normalize()?;
    let t = mesh.triangles.get(triangle)?;
    let n = |i: u32| mesh.normals.get(i as usize).copied();
    let (Some(n0), Some(n1), Some(n2)) = (n(t[0]), n(t[1]), n(t[2])) else {
        return Some(flat);
    };
    // Barycentric weights of the point.
    let (v0, v1, v2) = (p1 - p0, p2 - p0, point - p0);
    let (d00, d01, d11) = (v0.dot(v0), v0.dot(v1), v1.dot(v1));
    let (d20, d21) = (v2.dot(v0), v2.dot(v1));
    let den = d00 * d11 - d01 * d01;
    if den.abs() < f32::EPSILON {
        return Some(flat);
    }
    let v = (d11 * d20 - d01 * d21) / den;
    let w = (d00 * d21 - d01 * d20) / den;
    let u = 1.0 - v - w;
    (n0 * u + n1 * v + n2 * w).try_normalize().or(Some(flat))
}

/// The normals of a face: the vertex normals of its triangles, else each triangle's own.
fn face_normals(mesh: &Mesh, face: u32) -> Vec<Vec3> {
    let mut out = Vec::new();
    for (i, t) in mesh.triangles.iter().enumerate() {
        if mesh.face(i) != face {
            continue;
        }
        let vertex: Option<Vec<Vec3>> = t
            .iter()
            .map(|&k| mesh.normals.get(k as usize).copied())
            .collect();
        match vertex {
            Some(v) => out.extend(v),
            None => {
                if let Some([p0, p1, p2]) = mesh.corners(i)
                    && let Some(n) = (p1 - p0).cross(p2 - p0).try_normalize()
                {
                    out.push(n);
                }
            }
        }
    }
    out
}

/// Every normal of the face within `PARALLEL` of `n` or its opposite: the face is flat.
fn flat(normals: &[Vec3], n: Vec3) -> bool {
    !normals.is_empty() && normals.iter().all(|m| m.dot(n).abs() >= PARALLEL.cos())
}

fn relation(a: &Pick, b: &Pick, scene: &Scene) -> Option<Relation> {
    // Only face picks: an edge pick's point is on the edge, not on a face.
    let (fa, fb) = (a.face?, b.face?);
    if a.edge.is_some() || b.edge.is_some() {
        return None;
    }
    let (ma, mb) = (scene.meshes.get(fa.mesh)?, scene.meshes.get(fb.mesh)?);
    let na = normal_at(ma, fa.triangle, fa.point)?;
    let nb = normal_at(mb, fb.triangle, fb.point)?;
    let (normals_a, normals_b) = (face_normals(ma, fa.face), face_normals(mb, fb.face));

    if flat(&normals_a, na) && flat(&normals_b, nb) {
        if na.dot(nb).abs() < PARALLEL.cos() {
            return None;
        }
        let gap = (b.point - a.point).dot(na).abs();
        return Some(Relation::ParallelFaces { gap, normal: na });
    }

    // One curved face: a cylinder when all its normals are square to one axis.
    if fa.mesh != fb.mesh || fa.face != fb.face {
        return None;
    }
    let first = *normals_a.first()?;
    let other = normals_a
        .iter()
        .min_by(|x, y| x.dot(first).abs().total_cmp(&y.dot(first).abs()))?;
    let axis = first.cross(*other).try_normalize()?;
    if normals_a.iter().any(|n| n.dot(axis).abs() > ON_AXIS) {
        return None;
    }
    // In the plane square to the axis: a - r·na = b - r·nb, the centre.
    let flatten = |v: Vec3| v - axis * v.dot(axis);
    let (pa, pb, na, nb) = (flatten(a.point), flatten(b.point), flatten(na), flatten(nb));
    let dn = nb - na;
    if dn.length() < SPREAD {
        return None;
    }
    let r = (pb - pa).dot(dn) / dn.length_squared();
    let (ca, cb) = (pa - na * r, pb - nb * r);
    if r.abs() < f32::EPSILON || ca.distance(cb) > FIT * r.abs() {
        return None;
    }
    Some(Relation::Cylinder {
        diameter: 2.0 * r.abs(),
        axis,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::Ray;
    use crate::pick::Picker;
    use crate::shapes;
    use egui::Color32;

    /// The pick where a ray from `from` toward `to` hits the scene.
    fn pick(scene: &Scene, from: Vec3, to: Vec3) -> Pick {
        let ray = Ray {
            origin: from,
            dir: (to - from).normalize(),
        };
        let hit = Picker::new(scene).ray(scene, &ray).expect("a hit");
        Pick {
            face: Some(hit),
            edge: None,
            point: hit.point,
        }
    }

    fn board() -> Scene {
        Scene {
            meshes: vec![shapes::cuboid(
                Vec3::new(-10.0, -5.0, -1.6),
                Vec3::new(10.0, 5.0, 0.0),
                Color32::GRAY,
            )],
            lines: vec![],
        }
    }

    #[test]
    fn delta_and_distance_are_signed_in_the_models_axes() {
        let a = Pick {
            face: None,
            edge: None,
            point: Vec3::new(1.0, 2.0, 3.0),
        };
        let b = Pick {
            point: Vec3::new(4.0, -2.0, 3.0),
            ..a
        };
        let m = Measurement::between(&a, &b, &Scene::default());
        assert_eq!(m.delta, Vec3::new(3.0, -4.0, 0.0));
        assert_eq!(m.distance, 5.0);
        assert_eq!(m.relation, None);
        assert_eq!(m.line("mm"), "5.000 mm · dx 3.000 · dy -4.000 · dz 0.000");
        assert_eq!(m.label("mm"), "5.000 mm");
        assert_eq!(m.relation_line("mm"), None);
    }

    #[test]
    fn top_and_bottom_of_a_board_are_parallel_faces() {
        let s = board();
        let top = pick(&s, Vec3::new(2.0, 1.0, 10.0), Vec3::new(2.0, 1.0, 0.0));
        let bottom = pick(&s, Vec3::new(-3.0, 2.0, -10.0), Vec3::new(-3.0, 2.0, 0.0));
        let m = Measurement::between(&top, &bottom, &s);
        let Some(Relation::ParallelFaces { gap, .. }) = m.relation else {
            panic!("{:?}", m.relation)
        };
        assert!((gap - 1.6).abs() < 1e-4, "{gap}");
        assert!(m.distance > 5.0);
        assert_eq!(
            m.relation_line("mm").unwrap(),
            "parallel faces 1.600 mm apart"
        );
        // Top and a side: not parallel, no relation.
        let side = pick(&s, Vec3::new(20.0, 0.0, -0.8), Vec3::new(0.0, 0.0, -0.8));
        assert_eq!(Measurement::between(&top, &side, &s).relation, None);
    }

    #[test]
    fn two_points_on_a_cylinder_give_its_diameter() {
        let s = Scene {
            meshes: vec![shapes::cylinder(
                Vec3::new(4.0, -3.0, 0.0),
                2.0,
                5.5,
                64,
                Color32::GRAY,
            )],
            lines: vec![],
        };
        let c = Vec3::new(4.0, -3.0, 2.0);
        for (from, to) in [
            (Vec3::new(20.0, -3.0, 2.0), Vec3::new(-20.0, -3.0, 3.0)), // opposite sides
            (Vec3::new(4.0, 20.0, 1.0), Vec3::new(20.0, -3.0, 4.0)),   // a quarter round
        ] {
            let a = pick(&s, from, c);
            let b = pick(&s, to, c);
            let m = Measurement::between(&a, &b, &s);
            let Some(Relation::Cylinder { diameter, axis }) = m.relation else {
                panic!("{:?}", m.relation)
            };
            assert!((diameter - 4.0).abs() < 0.01, "{diameter}");
            assert!(axis.dot(Vec3::Z).abs() > 0.999);
            assert_eq!(m.label("mm"), "Ø 4.000 mm");
        }
        // The cap and the side: no relation.
        let cap = pick(&s, Vec3::new(4.5, -3.0, 20.0), Vec3::new(4.5, -3.0, 0.0));
        let side = pick(&s, Vec3::new(20.0, -3.0, 2.0), c);
        assert_eq!(Measurement::between(&cap, &side, &s).relation, None);
    }

    #[test]
    fn a_sphere_is_no_cylinder() {
        let s = Scene {
            meshes: vec![shapes::uv_sphere(Vec3::ZERO, 1.5, 48, 24, Color32::GRAY)],
            lines: vec![],
        };
        let a = pick(&s, Vec3::new(10.0, 0.0, 0.0), Vec3::ZERO);
        let b = pick(&s, Vec3::new(-10.0, 0.0, 0.3), Vec3::ZERO);
        assert_eq!(Measurement::between(&a, &b, &s).relation, None);
    }
}
