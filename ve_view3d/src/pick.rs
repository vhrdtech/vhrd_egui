//! Picking on the CPU: a ray against the triangles, a bounding volume hierarchy for big meshes, edges within a few
//! pixels of the pointer (V3D-1).
//!
//! Deterministic and testable without a GPU. A mesh up to [`Picker::BVH_THRESHOLD`] triangles is tested triangle by
//! triangle (the median model has 600); a bigger one gets a BVH (the largest seen has 176,000).

use egui::Pos2;
use glam::Vec3;

use crate::camera::{Projector, Ray};
use crate::scene::{Bounds, Scene};

/// A hit on a mesh's surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FacePick {
    /// The mesh, an index into [`Scene::meshes`].
    pub mesh: usize,
    /// The face id of the triangle hit ([`crate::Mesh::face_of_triangle`]).
    pub face: u32,
    /// The triangle hit.
    pub triangle: usize,
    /// The triangle's unit normal, turned toward the viewer.
    pub normal: Vec3,
    /// The point hit.
    pub point: Vec3,
    /// Distance along the ray.
    pub t: f32,
}

/// An edge near the pointer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EdgePick {
    /// The line set, an index into [`Scene::lines`].
    pub set: usize,
    /// The edge, an index into the set's polylines.
    pub edge: usize,
    /// The point on the edge closest to the pointer's ray.
    pub point: Vec3,
    /// How far from the pointer it is on the screen, in points.
    pub distance: f32,
}

/// What is under the pointer: a face, an edge, or both (an edge wins the point: it is what the person aimed at).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pick {
    /// The surface hit, if any.
    pub face: Option<FacePick>,
    /// The edge within the tolerance, if any (not one hidden behind a face).
    pub edge: Option<EdgePick>,
    /// The picked point: on the edge when there is one, else on the face.
    pub point: Vec3,
}

/// Which thing a pick refers to, for hover and selection: a face of a mesh or an edge of a line set.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Item {
    /// Mesh index and face id.
    Face { mesh: usize, face: u32 },
    /// Line set index and edge index.
    Edge { set: usize, edge: usize },
}

impl Pick {
    /// The item the person aimed at: the edge when there is one, else the face.
    pub fn item(&self) -> Option<Item> {
        if let Some(e) = self.edge {
            Some(Item::Edge {
                set: e.set,
                edge: e.edge,
            })
        } else {
            self.face.map(|f| Item::Face {
                mesh: f.mesh,
                face: f.face,
            })
        }
    }
}

/// A ray against a triangle, two-sided (Möller–Trumbore): the distance along the ray, `None` for a miss.
pub fn ray_triangle(ray: &Ray, [a, b, c]: [Vec3; 3]) -> Option<f32> {
    let e1 = b - a;
    let e2 = c - a;
    let p = ray.dir.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let s = ray.origin - a;
    let u = s.dot(p) * inv;
    if !(-1e-6..=1.0 + 1e-6).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = ray.dir.dot(q) * inv;
    if v < -1e-6 || u + v > 1.0 + 1e-6 {
        return None;
    }
    let t = e2.dot(q) * inv;
    (t >= 0.0).then_some(t)
}

/// The slab test: the ray's entry distance into the box, `None` for a miss or a box beyond `max_t`.
fn ray_box(origin: Vec3, inv_dir: Vec3, b: &Bounds, max_t: f32) -> Option<f32> {
    let t0 = (b.min - origin) * inv_dir;
    let t1 = (b.max - origin) * inv_dir;
    let near = t0.min(t1).max_element().max(0.0);
    let far = t0.max(t1).min_element().min(max_t);
    (near <= far).then_some(near)
}

/// A node of the hierarchy: a box with two children or a range of triangles.
#[derive(Clone, Debug)]
struct Node {
    bounds: Bounds,
    /// Leaf: first index into `order`; inner: the index of the second child (the first follows the node).
    start: u32,
    /// Leaf: the number of triangles; 0 marks an inner node.
    count: u32,
}

/// A bounding volume hierarchy over one mesh's triangles: median splits on the longest axis, four triangles a leaf.
#[derive(Clone, Debug, Default)]
pub struct Bvh {
    nodes: Vec<Node>,
    /// Triangle indices, leaves pointing at runs of them.
    order: Vec<u32>,
}

impl Bvh {
    const LEAF: usize = 4;

    /// Build over the given triangle corners (a triangle with an index out of range is left out).
    fn build(tris: &[Option<[Vec3; 3]>]) -> Self {
        let mut order: Vec<u32> = (0..tris.len() as u32)
            .filter(|&i| tris[i as usize].is_some())
            .collect();
        let centroid = |i: u32| {
            let [a, b, c] = tris[i as usize].unwrap_or_default();
            (a + b + c) / 3.0
        };
        let mut nodes = Vec::with_capacity(order.len() / 2 + 1);
        if !order.is_empty() {
            Self::split(tris, &centroid, &mut order, 0, &mut nodes);
        }
        Self { nodes, order }
    }

    fn split(
        tris: &[Option<[Vec3; 3]>],
        centroid: &impl Fn(u32) -> Vec3,
        order: &mut [u32],
        offset: usize,
        nodes: &mut Vec<Node>,
    ) {
        let mut bounds = Bounds::EMPTY;
        for &i in order.iter() {
            if let Some(c) = tris[i as usize] {
                c.iter().for_each(|p| bounds.add(*p));
            }
        }
        let me = nodes.len();
        nodes.push(Node {
            bounds,
            start: offset as u32,
            count: order.len() as u32,
        });
        if order.len() <= Self::LEAF {
            return;
        }
        let c = Bounds::from_points(order.iter().map(|&i| centroid(i)));
        let axis = (c.max - c.min).max_position();
        let mid = order.len() / 2;
        order.select_nth_unstable_by(mid, |&a, &b| {
            centroid(a)[axis].total_cmp(&centroid(b)[axis])
        });
        let (left, right) = order.split_at_mut(mid);
        Self::split(tris, centroid, left, offset, nodes);
        let second = nodes.len() as u32;
        Self::split(tris, centroid, right, offset + mid, nodes);
        nodes[me].start = second;
        nodes[me].count = 0;
    }

    /// The nearest hit as `(triangle, t)`.
    fn hit(&self, tris: &[Option<[Vec3; 3]>], ray: &Ray) -> Option<(usize, f32)> {
        let inv = ray.dir.recip();
        let mut best: Option<(usize, f32)> = None;
        let mut stack = Vec::with_capacity(64);
        if !self.nodes.is_empty() {
            stack.push(0usize);
        }
        while let Some(n) = stack.pop() {
            let node = &self.nodes[n];
            let max_t = best.map_or(f32::INFINITY, |b| b.1);
            if ray_box(ray.origin, inv, &node.bounds, max_t).is_none() {
                continue;
            }
            if node.count > 0 {
                let run = &self.order[node.start as usize..(node.start + node.count) as usize];
                for &i in run {
                    if let Some(t) = tris[i as usize].and_then(|c| ray_triangle(ray, c))
                        && best.is_none_or(|b| t < b.1)
                    {
                        best = Some((i as usize, t));
                    }
                }
            } else {
                // Visit the nearer child first so the farther one is mostly culled by `max_t`.
                let (a, b) = (n + 1, node.start as usize);
                let ta = ray_box(ray.origin, inv, &self.nodes[a].bounds, max_t);
                let tb = ray_box(ray.origin, inv, &self.nodes[b].bounds, max_t);
                match (ta, tb) {
                    (Some(x), Some(y)) if x <= y => stack.extend([b, a]),
                    (Some(_), Some(_)) => stack.extend([a, b]),
                    (Some(_), None) => stack.push(a),
                    (None, Some(_)) => stack.push(b),
                    (None, None) => {}
                }
            }
        }
        best
    }
}

/// The picking index of one mesh: its triangles' corners, and a BVH when it is big.
#[derive(Clone, Debug)]
struct MeshIndex {
    tris: Vec<Option<[Vec3; 3]>>,
    bvh: Option<Bvh>,
}

/// Picks faces, edges and points in a [`Scene`]. Build it once per scene; it borrows nothing.
#[derive(Clone, Debug, Default)]
pub struct Picker {
    meshes: Vec<MeshIndex>,
}

impl Picker {
    /// Meshes with more triangles than this get a BVH.
    pub const BVH_THRESHOLD: usize = 10_000;

    /// Index a scene.
    pub fn new(scene: &Scene) -> Self {
        Self::with_threshold(scene, Self::BVH_THRESHOLD)
    }

    /// Index a scene, building a BVH for meshes above `threshold` triangles (tests use 0 to compare both ways).
    pub fn with_threshold(scene: &Scene, threshold: usize) -> Self {
        let meshes = scene
            .meshes
            .iter()
            .map(|m| {
                let tris: Vec<_> = (0..m.triangles.len()).map(|i| m.corners(i)).collect();
                let bvh = (tris.len() > threshold).then(|| Bvh::build(&tris));
                MeshIndex { tris, bvh }
            })
            .collect();
        Self { meshes }
    }

    /// Whether mesh `i` has a BVH.
    pub fn has_bvh(&self, mesh: usize) -> bool {
        self.meshes.get(mesh).is_some_and(|m| m.bvh.is_some())
    }

    /// The nearest surface hit along a ray.
    pub fn ray(&self, scene: &Scene, ray: &Ray) -> Option<FacePick> {
        let mut best: Option<(usize, usize, f32)> = None;
        for (mi, index) in self.meshes.iter().enumerate() {
            let hit = match &index.bvh {
                Some(bvh) => bvh.hit(&index.tris, ray),
                None => index
                    .tris
                    .iter()
                    .enumerate()
                    .filter_map(|(i, c)| Some((i, ray_triangle(ray, (*c)?)?)))
                    .min_by(|a, b| a.1.total_cmp(&b.1)),
            };
            if let Some((tri, t)) = hit
                && best.is_none_or(|b| t < b.2)
            {
                best = Some((mi, tri, t));
            }
        }
        let (mesh, triangle, t) = best?;
        let [a, b, c] = self.meshes[mesh].tris[triangle]?;
        let mut normal = (b - a).cross(c - a).normalize_or(-ray.dir);
        if normal.dot(ray.dir) > 0.0 {
            normal = -normal;
        }
        Some(FacePick {
            mesh,
            face: scene.meshes.get(mesh).map_or(0, |m| m.face(triangle)),
            triangle,
            normal,
            point: ray.at(t),
            t,
        })
    }

    /// What is under a screen point: the surface along its ray, and the edge closest to it within `tolerance`
    /// points that no face hides.
    pub fn pick(
        &self,
        scene: &Scene,
        projector: &Projector,
        pos: Pos2,
        tolerance: f32,
    ) -> Option<Pick> {
        let ray = projector.ray(pos);
        let face = self.ray(scene, &ray);
        // How far an edge may lie behind the surface and still count as on it: a couple of the tolerance's
        // world size there, so edges on a face's border always pick.
        let slack = |p: Vec3| {
            let a = projector.project(p).map(|s| s.0);
            let b = projector
                .project(p + projector_right(projector) * 1e-3)
                .map(|s| s.0);
            match (a, b) {
                (Some(a), Some(b)) => 2.0 * tolerance * 1e-3 / (b - a).length().max(1e-9),
                _ => 0.0,
            }
        };
        let mut edge: Option<(EdgePick, f32)> = None;
        for (si, set) in scene.lines.iter().enumerate() {
            for (ei, a, b) in set.segments() {
                let p = closest_on_segment(&ray, a, b);
                let Some((s, _)) = projector.project(p) else {
                    continue;
                };
                if projector.view_depth(p) <= 0.0 && !projector.orthographic {
                    continue;
                }
                let d = (s - pos).length();
                if d > tolerance {
                    continue;
                }
                let depth = (p - ray.origin).dot(ray.dir);
                if let Some(f) = &face
                    && depth > f.t + slack(p)
                {
                    continue;
                }
                let better = match &edge {
                    None => true,
                    Some((e, ed)) => {
                        d < e.distance - 1e-3 || (d <= e.distance + 1e-3 && depth < *ed)
                    }
                };
                if better {
                    edge = Some((
                        EdgePick {
                            set: si,
                            edge: ei,
                            point: p,
                            distance: d,
                        },
                        depth,
                    ));
                }
            }
        }
        let edge = edge.map(|e| e.0);
        let point = edge.map(|e| e.point).or(face.map(|f| f.point))?;
        Some(Pick { face, edge, point })
    }
}

/// Screen right in the world for a projector (from its inverse at the centre).
fn projector_right(p: &Projector) -> Vec3 {
    let c = p.rect.center();
    let a = p.ray(c).origin;
    let b = p.ray(c + egui::vec2(1.0, 0.0)).origin;
    (b - a).normalize_or(Vec3::X)
}

/// The point of segment `a`–`b` closest to the ray's line.
pub fn closest_on_segment(ray: &Ray, a: Vec3, b: Vec3) -> Vec3 {
    let u = b - a;
    let w = a - ray.origin;
    let (uu, ud) = (u.dot(u), u.dot(ray.dir));
    let (uw, dw) = (u.dot(w), ray.dir.dot(w));
    let denom = uu - ud * ud;
    let s = if uu < 1e-18 {
        0.0
    } else if denom.abs() < 1e-12 {
        // Parallel to the ray: any point does; take the nearer end.
        0.0
    } else {
        ((ud * dw - uw) / denom).clamp(0.0, 1.0)
    };
    a + u * s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::{Camera, Projection, ViewPreset};
    use crate::scene::{LineSet, Mesh};
    use crate::shapes;
    use egui::{Color32, Rect, pos2, vec2};

    fn front(projection: Projection, scene: &Scene) -> Projector {
        let mut cam = Camera {
            projection,
            ..Default::default()
        };
        cam.view(ViewPreset::Front, &scene.bounds(), 1.0);
        cam.projector(
            Rect::from_min_size(Pos2::ZERO, vec2(200.0, 200.0)),
            &scene.bounds(),
        )
    }

    fn cube_scene() -> Scene {
        Scene {
            meshes: vec![shapes::cuboid(
                Vec3::splat(-1.0),
                Vec3::splat(1.0),
                shapes::PALETTE[0],
            )],
            lines: vec![shapes::cuboid_edges(Vec3::splat(-1.0), Vec3::splat(1.0))],
        }
    }

    #[test]
    fn the_front_face_is_hit_at_the_centre() {
        let scene = cube_scene();
        for projection in [Projection::Perspective, Projection::Orthographic] {
            let pr = front(projection, &scene);
            let p = Picker::new(&scene)
                .pick(&scene, &pr, pos2(100.0, 100.0), 4.0)
                .expect("hit");
            let f = p.face.expect("a face");
            assert!(p.edge.is_none());
            assert_eq!(f.mesh, 0);
            // cuboid faces: -X, +X, -Y, +Y, -Z, +Z
            assert_eq!(f.face, 2, "{projection:?}");
            assert!((f.point - Vec3::new(0.0, -1.0, 0.0)).length() < 1e-3);
            assert!((f.normal - -Vec3::Y).length() < 1e-4);
            assert_eq!(p.item(), Some(Item::Face { mesh: 0, face: 2 }));
        }
    }

    #[test]
    fn nothing_beside_the_model() {
        let scene = cube_scene();
        let pr = front(Projection::Perspective, &scene);
        assert_eq!(
            Picker::new(&scene).pick(&scene, &pr, pos2(2.0, 2.0), 4.0),
            None
        );
    }

    #[test]
    fn a_visible_edge_wins_and_a_hidden_one_does_not() {
        let scene = cube_scene();
        for projection in [Projection::Perspective, Projection::Orthographic] {
            let pr = front(projection, &scene);
            let picker = Picker::new(&scene);
            // The front face's bottom edge, from (-1,-1,-1) to (1,-1,-1).
            let edge_mid = pr.project(Vec3::new(0.3, -1.0, -1.0)).unwrap().0;
            let p = picker
                .pick(&scene, &pr, edge_mid + vec2(0.0, -2.0), 4.0)
                .expect("hit");
            let e = p.edge.expect("the edge");
            assert!(
                (e.point.y + 1.0).abs() < 1e-3 && (e.point.z + 1.0).abs() < 1e-3,
                "{e:?}"
            );
            assert!(e.distance <= 2.5);
            assert_eq!(p.point, e.point);
            // The back face's top edge sits behind the front face in perspective (its screen spot is inside the
            // front face); it must not pick through the cube.
            if projection == Projection::Perspective {
                let back = pr.project(Vec3::new(0.3, 1.0, 1.0)).unwrap().0;
                let p = picker.pick(&scene, &pr, back, 1.0).expect("the front face");
                assert!(p.edge.is_none(), "{:?}", p.edge);
                assert_eq!(p.face.unwrap().face, 2);
            }
        }
    }

    #[test]
    fn bvh_agrees_with_the_brute_force() {
        let mesh = shapes::uv_sphere(Vec3::new(0.5, -0.2, 0.1), 1.3, 96, 64, Color32::WHITE);
        assert!(mesh.triangles.len() > Picker::BVH_THRESHOLD);
        let scene = Scene {
            meshes: vec![
                mesh,
                shapes::cuboid(
                    Vec3::new(-3.0, -1.0, -1.0),
                    Vec3::new(-2.0, 1.0, 1.0),
                    Color32::RED,
                ),
            ],
            lines: vec![],
        };
        let auto = Picker::new(&scene);
        assert!(auto.has_bvh(0) && !auto.has_bvh(1));
        let all = Picker::with_threshold(&scene, 0);
        let none = Picker::with_threshold(&scene, usize::MAX);
        let mut seed = 12345u32;
        let mut rnd = || {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            (seed >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0
        };
        let mut hits = 0;
        for _ in 0..2000 {
            let origin = Vec3::new(rnd(), rnd(), rnd()).normalize() * 8.0;
            let aim = Vec3::new(rnd(), rnd(), rnd()) * 2.0;
            let ray = Ray {
                origin,
                dir: (aim - origin).normalize(),
            };
            let (a, b) = (all.ray(&scene, &ray), none.ray(&scene, &ray));
            assert_eq!(a.is_some(), b.is_some());
            if let (Some(a), Some(b)) = (a, b) {
                hits += 1;
                assert!((a.t - b.t).abs() < 1e-4, "{a:?} {b:?}");
                assert_eq!(a.mesh, b.mesh);
            }
        }
        assert!(hits > 500, "{hits} hits");
    }

    #[test]
    fn broken_triangles_are_skipped() {
        let mut m = Mesh::single_face(
            vec![Vec3::ZERO, Vec3::X, Vec3::Y],
            vec![],
            vec![[0, 1, 9]],
            Color32::RED,
        );
        m.triangles.push([0, 1, 2]);
        m.face_of_triangle.push(0);
        let scene = Scene {
            meshes: vec![m],
            lines: vec![LineSet::default()],
        };
        let ray = Ray {
            origin: Vec3::new(0.2, 0.2, 5.0),
            dir: -Vec3::Z,
        };
        for t in [0, usize::MAX] {
            let hit = Picker::with_threshold(&scene, t)
                .ray(&scene, &ray)
                .expect("the sound one");
            assert_eq!(hit.triangle, 1);
        }
    }

    #[test]
    fn closest_point_on_a_segment() {
        let ray = Ray {
            origin: Vec3::new(0.5, 0.0, 5.0),
            dir: -Vec3::Z,
        };
        let p = closest_on_segment(&ray, Vec3::new(0.0, 1.0, 0.0), Vec3::new(2.0, 1.0, 0.0));
        assert!((p - Vec3::new(0.5, 1.0, 0.0)).length() < 1e-5);
        let p = closest_on_segment(&ray, Vec3::new(3.0, 0.0, 0.0), Vec3::new(4.0, 0.0, 0.0));
        assert_eq!(p, Vec3::new(3.0, 0.0, 0.0));
    }
}
