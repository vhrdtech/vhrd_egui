//! Painting on the pixel grid: every quad the top bar draws starts and ends on a whole pixel, rounded corners
//! are rasterised here into per-pixel coverage and shadows are stacked whole-pixel layers, so egui's edge
//! feathering never runs.

use egui::epaint::{Mesh, Shadow};
use egui::{Color32, Pos2, Rect, Shape, pos2, vec2};

/// Samples per pixel side when rasterising coverage (4 by 4: 17 levels of alpha).
const SAMPLES: usize = 4;

/// A mesh being built in whole pixels for one `pixels_per_point`.
pub(crate) struct Pixels {
    ppp: f32,
    mesh: Mesh,
}

impl Pixels {
    pub(crate) fn new(ppp: f32) -> Self {
        Self {
            ppp,
            mesh: Mesh::default(),
        }
    }

    /// One pixel, in points.
    pub(crate) fn px(&self) -> f32 {
        1.0 / self.ppp
    }

    /// `rect` with each edge rounded to the nearest pixel.
    pub(crate) fn snap(&self, rect: Rect) -> Rect {
        Rect::from_min_max(self.snap_pos(rect.min), self.snap_pos(rect.max))
    }

    /// `pos` rounded to the nearest pixel.
    pub(crate) fn snap_pos(&self, pos: Pos2) -> Pos2 {
        pos2(
            (pos.x * self.ppp).round() / self.ppp,
            (pos.y * self.ppp).round() / self.ppp,
        )
    }

    /// A length in points rounded to whole pixels, at least one.
    pub(crate) fn whole(&self, points: f32) -> f32 {
        (points * self.ppp).round().max(1.0) / self.ppp
    }

    /// A filled rectangle, snapped.
    pub(crate) fn rect(&mut self, rect: Rect, color: Color32) {
        let r = self.snap(rect);
        if r.width() > 0.0 && r.height() > 0.0 && color.a() > 0 {
            self.mesh.add_colored_rect(r, color);
        }
    }

    /// A one pixel line along the bottom edge inside `rect`.
    pub(crate) fn hline_bottom(&mut self, rect: Rect, color: Color32) {
        let r = self.snap(rect);
        let px = self.px();
        self.rect(
            Rect::from_min_max(pos2(r.min.x, r.max.y - px), r.max),
            color,
        );
    }

    /// A one pixel vertical rule at `x`, from `y0` to `y1`.
    pub(crate) fn vline(&mut self, x: f32, y0: f32, y1: f32, color: Color32) {
        let px = self.px();
        self.rect(Rect::from_min_max(pos2(x, y0), pos2(x + px, y1)), color);
    }

    /// A filled rectangle with rounded corners of `radius` points; the corners are rasterised.
    pub(crate) fn rounded(&mut self, rect: Rect, radius: f32, color: Color32) {
        let r = self.snap(rect);
        let (w, h) = (r.width() * self.ppp, r.height() * self.ppp);
        let rad = whole_radius(radius * self.ppp, w, h);
        if rad < 1.0 {
            self.rect(r, color);
            return;
        }
        let c = rad / self.ppp;
        self.rect(
            Rect::from_min_max(pos2(r.min.x + c, r.min.y), pos2(r.max.x - c, r.max.y)),
            color,
        );
        self.rect(
            Rect::from_min_max(pos2(r.min.x, r.min.y + c), pos2(r.min.x + c, r.max.y - c)),
            color,
        );
        self.rect(
            Rect::from_min_max(pos2(r.max.x - c, r.min.y + c), pos2(r.max.x, r.max.y - c)),
            color,
        );
        self.corners(r, rad, color, move |x, y| inside(x, y, w, h, rad, 0.0));
    }

    /// A one pixel outline inside `rect` with rounded corners of `radius` points.
    pub(crate) fn outline(&mut self, rect: Rect, radius: f32, color: Color32) {
        let r = self.snap(rect);
        let w = r.width() * self.ppp;
        let h = r.height() * self.ppp;
        let rad = whole_radius(radius * self.ppp, w, h);
        let px = self.px();
        let c = rad.max(1.0) / self.ppp;
        for (a, b) in [
            (pos2(r.min.x + c, r.min.y), pos2(r.max.x - c, r.min.y + px)),
            (pos2(r.min.x + c, r.max.y - px), pos2(r.max.x - c, r.max.y)),
            (pos2(r.min.x, r.min.y + c), pos2(r.min.x + px, r.max.y - c)),
            (pos2(r.max.x - px, r.min.y + c), pos2(r.max.x, r.max.y - c)),
        ] {
            self.rect(Rect::from_min_max(a, b), color);
        }
        let ring = move |x: f32, y: f32| {
            inside(x, y, w, h, rad, 0.0) && !inside(x, y, w, h, (rad - 1.0).max(0.0), 1.0)
        };
        self.corners(r, rad.max(1.0), color, ring);
    }

    /// A one pixel dashed outline inside `rect`: `on` points drawn, `off` points left, around the edge.
    pub(crate) fn dashed(&mut self, rect: Rect, on: f32, off: f32, color: Color32) {
        let r = self.snap(rect);
        let px = self.px();
        let on = self.whole(on);
        let period = on + self.whole(off);
        let dash = |s: &mut Self, a: Pos2, horizontal: bool, len: f32| {
            let mut d = period / 2.0;
            while d < len - px {
                let e = (d + on).min(len - px);
                let q = if horizontal {
                    Rect::from_min_max(pos2(a.x + d, a.y), pos2(a.x + e, a.y + px))
                } else {
                    Rect::from_min_max(pos2(a.x, a.y + d), pos2(a.x + px, a.y + e))
                };
                s.rect(q, color);
                d += period;
            }
        };
        dash(self, r.left_top(), true, r.width());
        dash(self, pos2(r.left(), r.bottom() - px), true, r.width());
        dash(self, r.left_top(), false, r.height());
        dash(self, pos2(r.right() - px, r.top()), false, r.height());
    }

    /// A shadow under `rect` with corners of `radius` points: whole-pixel layers, one per pixel of blur, each
    /// a share of the shadow's colour, so it fades outwards without a feathered edge.
    pub(crate) fn shadow(&mut self, rect: Rect, radius: f32, shadow: Shadow) {
        let layers = (f32::from(shadow.blur) * self.ppp / 2.0).round().max(1.0) as usize;
        let at = rect.translate(vec2(
            f32::from(shadow.offset[0]),
            f32::from(shadow.offset[1]),
        ));
        let share = shadow.color.gamma_multiply(1.0 / layers as f32);
        let px = self.px();
        for k in 0..layers {
            let grow = (layers - k) as f32 * px + f32::from(shadow.spread);
            self.rounded(at.expand(grow), radius + grow, share);
        }
    }

    /// Rasterises the four `rad` by `rad` pixel corner boxes of `r` with `inside` in box pixel coordinates.
    fn corners(&mut self, r: Rect, rad: f32, color: Color32, inside: impl Fn(f32, f32) -> bool) {
        let n = rad as usize;
        let w = (r.width() * self.ppp).round() as usize;
        let h = (r.height() * self.ppp).round() as usize;
        for (x0, y0) in [
            (0, 0),
            (w.saturating_sub(n), 0),
            (0, h.saturating_sub(n)),
            (w.saturating_sub(n), h.saturating_sub(n)),
        ] {
            self.raster(r.min, (x0, y0), (n, n), color, &inside);
        }
    }

    /// Rasterises the box of `size` pixels at pixel offset `at` from `origin` (a snapped point): each pixel
    /// gets `color` at the share of its samples `inside` (box pixel coordinates) holds.
    fn raster(
        &mut self,
        origin: Pos2,
        at: (usize, usize),
        size: (usize, usize),
        color: Color32,
        inside: &impl Fn(f32, f32) -> bool,
    ) {
        let px = self.px();
        let (x0, y0) = at;
        let (w, h) = size;
        for y in y0..y0 + h {
            let mut run: Option<(usize, u8)> = None;
            for x in x0..=x0 + w {
                let a = if x < x0 + w {
                    coverage(x, y, inside)
                } else {
                    0
                };
                match run {
                    Some((start, ra)) if ra != a => {
                        let c = color.gamma_multiply(f32::from(ra) / (SAMPLES * SAMPLES) as f32);
                        self.mesh.add_colored_rect(
                            Rect::from_min_max(
                                origin + vec2(start as f32 * px, y as f32 * px),
                                origin + vec2(x as f32 * px, (y + 1) as f32 * px),
                            ),
                            c,
                        );
                        run = (a > 0).then_some((x, a));
                    }
                    None if a > 0 => run = Some((x, a)),
                    _ => {}
                }
            }
        }
    }

    /// The finished mesh as a shape.
    pub(crate) fn finish(self) -> Shape {
        Shape::mesh(self.mesh)
    }
}

/// A corner radius of whole pixels for a `w` by `h` pixel box: `rad` rounded, at most half the shorter side
/// rounded down, so an odd side keeps its middle pixel row or column filled.
fn whole_radius(rad: f32, w: f32, h: f32) -> f32 {
    rad.round().min((w.min(h) / 2.0).floor()).max(0.0)
}

/// How many of a pixel's samples are inside.
fn coverage(x: usize, y: usize, inside: &impl Fn(f32, f32) -> bool) -> u8 {
    let mut n = 0u8;
    for sy in 0..SAMPLES {
        for sx in 0..SAMPLES {
            let fx = x as f32 + (sx as f32 + 0.5) / SAMPLES as f32;
            let fy = y as f32 + (sy as f32 + 0.5) / SAMPLES as f32;
            if inside(fx, fy) {
                n += 1;
            }
        }
    }
    n
}

/// Whether (`x`, `y`) lies inside a `w` by `h` box shrunk by `inset` on every side, with corners of radius
/// `rad`, all in pixels.
fn inside(x: f32, y: f32, w: f32, h: f32, rad: f32, inset: f32) -> bool {
    let (x0, y0, x1, y1) = (inset, inset, w - inset, h - inset);
    if x < x0 || y < y0 || x > x1 || y > y1 {
        return false;
    }
    let cx = x.clamp(x0 + rad, (x1 - rad).max(x0 + rad));
    let cy = y.clamp(y0 + rad, (y1 - rad).max(y0 + rad));
    vec2(x - cx, y - cy).length_sq() <= rad * rad
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every vertex the painter makes sits on a whole pixel, at 1x, 1.25x, 1.5x and 2x, whatever the input.
    #[test]
    fn every_vertex_sits_on_the_pixel_grid() {
        for ppp in [1.0, 1.25, 1.5, 2.0] {
            let mut p = Pixels::new(ppp);
            let r = Rect::from_min_size(pos2(10.3, 4.77), vec2(57.1, 21.4));
            p.rect(r, Color32::RED);
            p.rounded(r, 3.0, Color32::RED);
            p.outline(r, 9.0, Color32::RED);
            p.dashed(r, 3.0, 2.0, Color32::RED);
            p.vline(33.3, 2.2, 30.1, Color32::RED);
            p.hline_bottom(r, Color32::RED);
            p.shadow(
                r,
                3.0,
                Shadow {
                    offset: [0, 4],
                    blur: 12,
                    spread: 0,
                    color: Color32::from_black_alpha(110),
                },
            );
            let Shape::Mesh(mesh) = p.finish() else {
                panic!("a mesh")
            };
            assert!(!mesh.vertices.is_empty());
            for v in &mesh.vertices {
                let q = v.pos.to_vec2() * ppp;
                assert!(
                    (q.x - q.x.round()).abs() < 1e-3 && (q.y - q.y.round()).abs() < 1e-3,
                    "{ppp}x: vertex {q:?} off the grid"
                );
            }
        }
    }

    /// A round shape of any whole-pixel size is solid through its middle: no empty row or column at an odd
    /// size (a 6 point dot is 7 pixels at 1.25x and 9 at 1.5x).
    #[test]
    fn a_round_shape_has_no_gap_through_its_middle() {
        for ppp in [1.0, 1.25, 1.5, 2.0] {
            for side_px in 3..=12_u16 {
                let side = f32::from(side_px) / ppp;
                let mut p = Pixels::new(ppp);
                let r = Rect::from_min_size(pos2(8.0, 8.0), vec2(side, side));
                p.rounded(r, side, Color32::RED);
                let Shape::Mesh(mesh) = p.finish() else {
                    panic!("a mesh")
                };
                let covered = |q: Pos2| {
                    mesh.vertices.chunks(4).any(|quad| {
                        Rect::from_points(&quad.iter().map(|v| v.pos).collect::<Vec<_>>())
                            .contains(q)
                    })
                };
                let mid = (f32::from(side_px) / 2.0).floor();
                for i in 1..side_px - 1 {
                    let along = (f32::from(i) + 0.5) / ppp;
                    let across = (mid + 0.5) / ppp;
                    for q in [pos2(along, across), pos2(across, along)] {
                        assert!(
                            covered(r.min + q.to_vec2()),
                            "{ppp}x, {side_px} px: pixel at {:?} empty",
                            q.to_vec2() * ppp
                        );
                    }
                }
            }
        }
    }

    /// A shadow is darkest next to its rectangle and fades out: more layers cover a pixel nearer to it.
    #[test]
    fn a_shadow_fades_outwards() {
        let mut p = Pixels::new(1.0);
        let r = Rect::from_min_size(pos2(20.0, 20.0), vec2(40.0, 20.0));
        p.shadow(
            r,
            0.0,
            Shadow {
                offset: [0, 0],
                blur: 8,
                spread: 0,
                color: Color32::from_black_alpha(100),
            },
        );
        let Shape::Mesh(mesh) = p.finish() else {
            panic!("a mesh")
        };
        let layers_at = |q: Pos2| {
            mesh.vertices
                .chunks(4)
                .filter(|quad| {
                    let b = Rect::from_points(&quad.iter().map(|v| v.pos).collect::<Vec<_>>());
                    b.contains(q) && b.width() > 0.0
                })
                .count()
        };
        assert!(layers_at(pos2(40.0, 19.5)) > layers_at(pos2(40.0, 16.5)));
        assert_eq!(layers_at(pos2(40.0, 10.0)), 0, "nothing past the blur");
    }
}
