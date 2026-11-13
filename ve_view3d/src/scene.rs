//! What the view shows: meshes with faces, line sets of edges, overlay marks (V3D-1).
//!
//! Everything is in one world frame, in the caller's unit (millimetres for a footprint model), Z up. The view
//! knows meshes, not CAD: a face is just an id the caller gives a group of triangles, an edge a polyline with an
//! id, so a STEP reader (or a procedural shape) maps its own B-rep onto them.

use egui::Color32;
use glam::Vec3;

/// A triangle mesh whose triangles are grouped into faces, each face with a colour.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh {
    /// Vertex positions.
    pub positions: Vec<Vec3>,
    /// One normal per vertex, or empty: then each triangle is shaded flat with its own normal.
    pub normals: Vec<Vec3>,
    /// Triangles as indices into `positions`, counterclockwise seen from outside (shading is two-sided, so the
    /// winding only decides the flat normal's sign).
    pub triangles: Vec<[u32; 3]>,
    /// The face of each triangle: an index into `face_colors`, and the id [`crate::Pick::face`] returns.
    pub face_of_triangle: Vec<u32>,
    /// One colour per face.
    pub face_colors: Vec<Color32>,
}

impl Mesh {
    /// A mesh of one face in one colour.
    pub fn single_face(
        positions: Vec<Vec3>,
        normals: Vec<Vec3>,
        triangles: Vec<[u32; 3]>,
        color: Color32,
    ) -> Self {
        let n = triangles.len();
        Self {
            positions,
            normals,
            triangles,
            face_of_triangle: vec![0; n],
            face_colors: vec![color],
        }
    }

    /// Number of faces (the length of `face_colors`).
    pub fn face_count(&self) -> usize {
        self.face_colors.len()
    }

    /// The three corners of a triangle, `None` when an index is out of range.
    pub fn corners(&self, triangle: usize) -> Option<[Vec3; 3]> {
        let t = self.triangles.get(triangle)?;
        Some([
            *self.positions.get(t[0] as usize)?,
            *self.positions.get(t[1] as usize)?,
            *self.positions.get(t[2] as usize)?,
        ])
    }

    /// The face of a triangle; a missing entry counts as face 0.
    pub fn face(&self, triangle: usize) -> u32 {
        self.face_of_triangle.get(triangle).copied().unwrap_or(0)
    }

    /// The colour of a face; a face without one is grey.
    pub fn color(&self, face: u32) -> Color32 {
        self.face_colors
            .get(face as usize)
            .copied()
            .unwrap_or(Color32::GRAY)
    }

    /// The axis-aligned box around the vertices that triangles use.
    pub fn bounds(&self) -> Bounds {
        let mut b = Bounds::EMPTY;
        for i in 0..self.triangles.len() {
            if let Some(c) = self.corners(i) {
                c.iter().for_each(|p| b.add(*p));
            }
        }
        b
    }

    /// Problems that would make triangles disappear: indices out of range, a face without a colour, normals of
    /// the wrong count. Empty when the mesh is sound.
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        let n = self.positions.len();
        if let Some(t) = self
            .triangles
            .iter()
            .position(|t| t.iter().any(|&i| i as usize >= n))
        {
            out.push(format!("triangle {t} points past the {n} vertices"));
        }
        if !self.normals.is_empty() && self.normals.len() != n {
            out.push(format!("{} normals for {n} vertices", self.normals.len()));
        }
        if self.face_of_triangle.len() != self.triangles.len() {
            out.push(format!(
                "{} face ids for {} triangles",
                self.face_of_triangle.len(),
                self.triangles.len()
            ));
        }
        let faces = self.face_colors.len() as u32;
        if let Some(f) = self.face_of_triangle.iter().find(|&&f| f >= faces) {
            out.push(format!("face {f} has no colour ({faces} colours)"));
        }
        out
    }
}

/// Edges: polylines, each one edge with the id [`crate::EdgePick::edge`] returns (its index).
#[derive(Clone, Debug, PartialEq)]
pub struct LineSet {
    /// One polyline per edge.
    pub polylines: Vec<Vec<Vec3>>,
    /// The colour of every edge in the set.
    pub color: Color32,
    /// Width on screen in points; edges keep it at every zoom.
    pub width: f32,
}

impl Default for LineSet {
    fn default() -> Self {
        Self {
            polylines: Vec::new(),
            color: Color32::from_gray(20),
            width: 1.5,
        }
    }
}

impl LineSet {
    /// The segments of all polylines as `(edge, a, b)`.
    pub fn segments(&self) -> impl Iterator<Item = (usize, Vec3, Vec3)> + '_ {
        self.polylines
            .iter()
            .enumerate()
            .flat_map(|(e, line)| line.windows(2).map(move |w| (e, w[0], w[1])))
    }

    /// The box around every point.
    pub fn bounds(&self) -> Bounds {
        let mut b = Bounds::EMPTY;
        self.polylines.iter().flatten().for_each(|p| b.add(*p));
        b
    }
}

/// What an overlay mark draws.
#[derive(Clone, Debug, PartialEq)]
pub enum MarkShape {
    /// A dot at a point.
    Point(Vec3),
    /// A line between two points with a dot at each end, as a measurement draws.
    Segment(Vec3, Vec3),
}

/// An overlay mark: drawn over the model (never hidden by it), with an optional label, by egui's painter.
#[derive(Clone, Debug, PartialEq)]
pub struct Mark {
    /// Where.
    pub shape: MarkShape,
    /// Text beside the point or at the segment's middle; empty for none.
    pub label: String,
    /// The colour; `None` takes the theme's accent.
    pub color: Option<Color32>,
}

impl Mark {
    /// A dot with a label.
    pub fn point(p: Vec3, label: impl Into<String>) -> Self {
        Self {
            shape: MarkShape::Point(p),
            label: label.into(),
            color: None,
        }
    }

    /// A segment with a label at its middle.
    pub fn segment(a: Vec3, b: Vec3, label: impl Into<String>) -> Self {
        Self {
            shape: MarkShape::Segment(a, b),
            label: label.into(),
            color: None,
        }
    }

    /// The same mark in another colour.
    pub fn color(mut self, color: Color32) -> Self {
        self.color = Some(color);
        self
    }
}

/// Everything the view shows. Meshes and line sets are uploaded to the GPU when the scene is set
/// ([`crate::View3d::set_scene`]); marks are cheap to change every frame ([`crate::View3d::marks`]).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scene {
    /// The meshes; a [`crate::Pick`] names one by its index here.
    pub meshes: Vec<Mesh>,
    /// The edges; a [`crate::EdgePick`] names a set by its index here.
    pub lines: Vec<LineSet>,
}

impl Scene {
    /// The box around every mesh and edge.
    pub fn bounds(&self) -> Bounds {
        let mut b = Bounds::EMPTY;
        self.meshes.iter().for_each(|m| b.union(&m.bounds()));
        self.lines.iter().for_each(|l| b.union(&l.bounds()));
        b
    }

    /// Total triangles over all meshes.
    pub fn triangle_count(&self) -> usize {
        self.meshes.iter().map(|m| m.triangles.len()).sum()
    }
}

/// An axis-aligned box; [`Bounds::EMPTY`] until a point is added.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    /// The low corner.
    pub min: Vec3,
    /// The high corner.
    pub max: Vec3,
}

impl Bounds {
    /// No points yet: `min` above `max`.
    pub const EMPTY: Bounds = Bounds {
        min: Vec3::splat(f32::INFINITY),
        max: Vec3::splat(f32::NEG_INFINITY),
    };

    /// The box around points.
    pub fn from_points(points: impl IntoIterator<Item = Vec3>) -> Self {
        let mut b = Self::EMPTY;
        points.into_iter().for_each(|p| b.add(p));
        b
    }

    /// Grow to hold a point.
    pub fn add(&mut self, p: Vec3) {
        self.min = self.min.min(p);
        self.max = self.max.max(p);
    }

    /// Grow to hold another box.
    pub fn union(&mut self, other: &Bounds) {
        self.min = self.min.min(other.min);
        self.max = self.max.max(other.max);
    }

    /// True until a point is added.
    pub fn is_empty(&self) -> bool {
        !(self.min.x <= self.max.x && self.min.y <= self.max.y && self.min.z <= self.max.z)
    }

    /// The middle.
    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    /// The radius of the sphere around the box from its centre.
    pub fn radius(&self) -> f32 {
        (self.max - self.min).length() * 0.5
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tri() -> Mesh {
        Mesh::single_face(
            vec![Vec3::ZERO, Vec3::X, Vec3::Y],
            vec![],
            vec![[0, 1, 2]],
            Color32::RED,
        )
    }

    #[test]
    fn bounds_and_problems() {
        let m = tri();
        assert!(m.problems().is_empty());
        let b = m.bounds();
        assert_eq!(b.min, Vec3::ZERO);
        assert_eq!(b.max, Vec3::new(1.0, 1.0, 0.0));
        assert!(Bounds::EMPTY.is_empty());
        assert!(!b.is_empty());

        let mut bad = tri();
        bad.triangles.push([0, 1, 7]);
        bad.face_of_triangle.push(3);
        let p = bad.problems();
        assert_eq!(p.len(), 2, "{p:?}");
        // The broken triangle is left out of the bounds instead of panicking.
        assert_eq!(bad.bounds(), b);
    }

    #[test]
    fn segments_of_polylines() {
        let l = LineSet {
            polylines: vec![vec![Vec3::ZERO, Vec3::X, Vec3::Y], vec![Vec3::Z]],
            ..Default::default()
        };
        let s: Vec<_> = l.segments().collect();
        assert_eq!(s, vec![(0, Vec3::ZERO, Vec3::X), (0, Vec3::X, Vec3::Y)]);
    }
}
