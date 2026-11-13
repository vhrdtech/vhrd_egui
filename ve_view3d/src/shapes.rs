//! Procedural shapes for examples, tests and simple scene parts (a board plane, a marker): boxes, cylinders,
//! spheres, with faces and edges the way a CAD model has them.

use std::f32::consts::TAU;

use egui::Color32;
use glam::Vec3;

use crate::scene::{LineSet, Mesh};

/// A few distinct face colours.
pub const PALETTE: [Color32; 6] = [
    Color32::from_rgb(0x8c, 0x9c, 0xb0),
    Color32::from_rgb(0xc8, 0x8a, 0x4a),
    Color32::from_rgb(0x5e, 0xa8, 0x7a),
    Color32::from_rgb(0xb0, 0x5e, 0x6e),
    Color32::from_rgb(0x6a, 0x7e, 0xc8),
    Color32::from_rgb(0xd0, 0xc0, 0x5a),
];

/// An axis-aligned box from `min` to `max`, six faces in the order -X, +X, -Y, +Y, -Z, +Z, all in `color`.
pub fn cuboid(min: Vec3, max: Vec3, color: Color32) -> Mesh {
    let mut m = Mesh::default();
    for (axis, sign) in [
        (0, -1.0f32),
        (0, 1.0),
        (1, -1.0),
        (1, 1.0),
        (2, -1.0),
        (2, 1.0),
    ] {
        let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
        let mut normal = Vec3::ZERO;
        normal[axis] = sign;
        let corner = |a: bool, b: bool| {
            let mut p = Vec3::ZERO;
            p[axis] = if sign > 0.0 { max[axis] } else { min[axis] };
            p[u] = if a { max[u] } else { min[u] };
            p[v] = if b { max[v] } else { min[v] };
            p
        };
        let base = m.positions.len() as u32;
        m.positions.extend([
            corner(false, false),
            corner(true, false),
            corner(true, true),
            corner(false, true),
        ]);
        m.normals.extend([normal; 4]);
        let face = m.face_colors.len() as u32;
        if sign > 0.0 {
            m.triangles
                .extend([[base, base + 1, base + 2], [base, base + 2, base + 3]]);
        } else {
            m.triangles
                .extend([[base, base + 2, base + 1], [base, base + 3, base + 2]]);
        }
        m.face_of_triangle.extend([face, face]);
        m.face_colors.push(color);
    }
    m
}

/// The twelve edges of a box, one polyline each.
pub fn cuboid_edges(min: Vec3, max: Vec3) -> LineSet {
    let c = |i: u8| {
        Vec3::new(
            if i & 1 == 0 { min.x } else { max.x },
            if i & 2 == 0 { min.y } else { max.y },
            if i & 4 == 0 { min.z } else { max.z },
        )
    };
    let mut polylines = Vec::new();
    for i in 0..8u8 {
        for bit in [1u8, 2, 4] {
            if i & bit == 0 {
                polylines.push(vec![c(i), c(i | bit)]);
            }
        }
    }
    LineSet {
        polylines,
        ..Default::default()
    }
}

/// A cylinder standing on `base` along +Z: three faces (side 0, bottom 1, top 2) in `color`, `segments` around.
pub fn cylinder(base: Vec3, radius: f32, height: f32, segments: usize, color: Color32) -> Mesh {
    let n = segments.max(3);
    let mut m = Mesh {
        face_colors: vec![color; 3],
        ..Default::default()
    };
    let ring = |i: usize| {
        let a = i as f32 / n as f32 * TAU;
        Vec3::new(a.cos(), a.sin(), 0.0)
    };
    // Side: smooth normals.
    for i in 0..n {
        let d = ring(i);
        m.positions
            .extend([base + d * radius, base + d * radius + Vec3::Z * height]);
        m.normals.extend([d, d]);
    }
    for i in 0..n {
        let (a, b) = (2 * i as u32, 2 * ((i + 1) % n) as u32);
        m.triangles.extend([[a, b, b + 1], [a, b + 1, a + 1]]);
        m.face_of_triangle.extend([0, 0]);
    }
    // Caps: a fan each, flat normals.
    for (face, z, normal) in [(1u32, 0.0, -Vec3::Z), (2, height, Vec3::Z)] {
        let center = m.positions.len() as u32;
        m.positions.push(base + Vec3::Z * z);
        m.normals.push(normal);
        for i in 0..n {
            m.positions.push(base + ring(i) * radius + Vec3::Z * z);
            m.normals.push(normal);
        }
        for i in 0..n as u32 {
            let (a, b) = (center + 1 + i, center + 1 + (i + 1) % n as u32);
            m.triangles.push(if face == 2 {
                [center, a, b]
            } else {
                [center, b, a]
            });
            m.face_of_triangle.push(face);
        }
    }
    m
}

/// The two circles of a cylinder's rims.
pub fn cylinder_edges(base: Vec3, radius: f32, height: f32, segments: usize) -> LineSet {
    let n = segments.max(3);
    let circle = |z: f32| {
        (0..=n)
            .map(|i| {
                let a = i as f32 / n as f32 * TAU;
                base + Vec3::new(a.cos() * radius, a.sin() * radius, z)
            })
            .collect()
    };
    LineSet {
        polylines: vec![circle(0.0), circle(height)],
        ..Default::default()
    }
}

/// A sphere of `rings` × `segments` quads (two triangles each), one face, smooth normals.
pub fn uv_sphere(center: Vec3, radius: f32, segments: usize, rings: usize, color: Color32) -> Mesh {
    let (n, r) = (segments.max(3), rings.max(2));
    let mut positions = Vec::with_capacity((n + 1) * (r + 1));
    let mut normals = Vec::with_capacity(positions.capacity());
    for j in 0..=r {
        let phi = j as f32 / r as f32 * std::f32::consts::PI;
        for i in 0..=n {
            let theta = i as f32 / n as f32 * TAU;
            let d = Vec3::new(phi.sin() * theta.cos(), phi.sin() * theta.sin(), phi.cos());
            positions.push(center + d * radius);
            normals.push(d);
        }
    }
    let mut triangles = Vec::with_capacity(n * r * 2);
    let row = (n + 1) as u32;
    for j in 0..r as u32 {
        for i in 0..n as u32 {
            let a = j * row + i;
            let b = a + row;
            triangles.push([a, b, a + 1]);
            triangles.push([a + 1, b, b + 1]);
        }
    }
    Mesh::single_face(positions, normals, triangles, color)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shapes_are_sound() {
        let c = cuboid(Vec3::ZERO, Vec3::ONE, Color32::RED);
        assert!(c.problems().is_empty());
        assert_eq!((c.triangles.len(), c.face_count()), (12, 6));
        // Outward winding: the flat normal agrees with the vertex normal.
        for (i, t) in c.triangles.iter().enumerate() {
            let [a, b, d] = c.corners(i).unwrap();
            let flat = (b - a).cross(d - a).normalize();
            assert!(
                (flat - c.normals[t[0] as usize]).length() < 1e-5,
                "triangle {i}"
            );
        }
        assert_eq!(cuboid_edges(Vec3::ZERO, Vec3::ONE).polylines.len(), 12);
        let cyl = cylinder(Vec3::ZERO, 1.0, 2.0, 24, Color32::RED);
        assert!(cyl.problems().is_empty());
        assert_eq!(cyl.triangles.len(), 24 * 4);
        let s = uv_sphere(Vec3::ZERO, 1.0, 16, 8, Color32::RED);
        assert!(s.problems().is_empty());
        assert!((s.bounds().radius() - 3f32.sqrt()).abs() < 0.05);
    }
}
