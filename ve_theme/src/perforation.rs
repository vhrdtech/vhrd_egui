//! The perforated light: a small square of holes in the metal with the light behind them, for status lights,
//! recording, level meters and the selected item's bar.
//!
//! Every edge sits on the pixel grid at any scale: the hole and the gap between holes
//! ([`Perforation`](crate::Perforation) tokens) are rounded to whole pixels, the field's corner snaps to a
//! pixel, and the holes are drawn as one untextured mesh of axis-aligned quads, so egui's edge feathering
//! never blurs them. The glow is light leaking into the metal between two lit holes, nothing outside the
//! field.
//!
//! ```ignore
//! ui.add(ve_theme::PerforatedLight::tone(Tone::Good)).on_hover_text("dev0 answers");
//! ```

use egui::{Color32, Mesh, Pos2, Rect, Response, Sense, Shape, Ui, Vec2, Widget, pos2, vec2};

use crate::{Tokens, Tone};

/// Holes per side by default: a 3 by 3 square.
pub const HOLES_DEFAULT: u8 = 3;
/// Holes per side at most.
pub const HOLES_MAX: u8 = 16;

/// A square of `n` by `n` holes, lit in one colour or unlit.
#[derive(Clone, Copy, Debug, PartialEq)]
#[must_use = "pass to ui.add() or call .paint()"]
pub struct PerforatedLight {
    color: Color32,
    lit: bool,
    holes: u8,
}

impl PerforatedLight {
    /// A light lit in `color`.
    pub fn new(color: Color32) -> Self {
        Self {
            color,
            lit: true,
            holes: HOLES_DEFAULT,
        }
    }

    /// A light in a tone's colour; [`Tone::Off`] draws it unlit.
    pub fn tone(tone: Tone, tokens: &Tokens) -> Self {
        Self::new(tone.color(tokens)).lit(tone != Tone::Off)
    }

    /// Lit (the default), or dark: holes in `line_strong`, no glow.
    pub fn lit(mut self, lit: bool) -> Self {
        self.lit = lit;
        self
    }

    /// Holes per side, 1 to [`HOLES_MAX`]; from 4 up the corner holes are left out, a disc of holes.
    ///
    /// # Panics
    ///
    /// When `holes` is 0 or more than [`HOLES_MAX`].
    pub fn holes(mut self, holes: u8) -> Self {
        assert!(holes >= 1, "a perforated light needs at least one hole");
        assert!(
            holes <= HOLES_MAX,
            "a perforated light has at most {HOLES_MAX} holes per side"
        );
        self.holes = holes;
        self
    }

    /// The field's side in pixels at `pixels_per_point`.
    pub fn side_px(&self, perf: &crate::Perforation, pixels_per_point: f32) -> f32 {
        let (hole, gap) = grid(perf, pixels_per_point);
        f32::from(self.holes) * hole + f32::from(self.holes - 1) * gap
    }

    /// Paint the light with its field's top left corner at `min` (snapped to the pixel grid).
    pub fn paint(&self, painter: &egui::Painter, tokens: &Tokens, min: Pos2) {
        let ppp = painter.pixels_per_point();
        painter.add(Shape::mesh(self.mesh(tokens, min, ppp)));
    }

    /// The light as one mesh of pixel-aligned quads; positions in points.
    pub fn mesh(&self, tokens: &Tokens, min: Pos2, pixels_per_point: f32) -> Mesh {
        let ppp = pixels_per_point;
        let (hole, gap) = grid(&tokens.perforation, ppp);
        let pitch = hole + gap;
        let origin = (min.to_vec2() * ppp).round();
        let n = usize::from(self.holes);
        let present = |i: usize, j: usize| -> bool {
            if n < 4 || i >= n || j >= n {
                return i < n && j < n;
            }
            // The disc: corners whose taxicab distance from the centre passes 0.8 of the half-span go.
            let (c, span) = ((n - 1) as f32 / 2.0, (n - 1) as f32);
            (i as f32 - c).abs() + (j as f32 - c).abs() <= span * 0.8
        };
        let px_rect = |x: f32, y: f32, w: f32, h: f32| {
            Rect::from_min_size(pos2(origin.x + x, origin.y + y) / ppp, vec2(w, h) / ppp)
        };
        let mut mesh = Mesh::default();
        if self.lit && tokens.perforation.glow > 0.0 {
            let glow = self.color.gamma_multiply(tokens.perforation.glow);
            for j in 0..n {
                for i in 0..n {
                    if !present(i, j) {
                        continue;
                    }
                    let (x, y) = (i as f32 * pitch, j as f32 * pitch);
                    let right = present(i + 1, j);
                    let below = present(i, j + 1);
                    if right {
                        mesh.add_colored_rect(px_rect(x + hole, y, gap, hole), glow);
                    }
                    if below {
                        mesh.add_colored_rect(px_rect(x, y + hole, hole, gap), glow);
                    }
                    if right && below && present(i + 1, j + 1) {
                        mesh.add_colored_rect(px_rect(x + hole, y + hole, gap, gap), glow);
                    }
                }
            }
        }
        let color = if self.lit {
            self.color
        } else {
            tokens.colors.line_strong
        };
        for j in 0..n {
            for i in 0..n {
                if present(i, j) {
                    hole_quads(
                        &mut mesh,
                        px_rect(i as f32 * pitch, j as f32 * pitch, hole, hole),
                        hole,
                        color,
                        ppp,
                    );
                }
            }
        }
        mesh
    }
}

impl Widget for PerforatedLight {
    fn ui(self, ui: &mut Ui) -> Response {
        let tokens = Tokens::of_ui(ui);
        let ppp = ui.ctx().pixels_per_point();
        // One pixel more than the field, so snapping the corner never pushes it out of its rectangle.
        let side = (self.side_px(&tokens.perforation, ppp) + 1.0) / ppp;
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(side), Sense::hover());
        if ui.is_rect_visible(rect) {
            let min = (rect.min.to_vec2() * ppp).ceil() / ppp;
            self.paint(ui.painter(), &tokens, min.to_pos2());
        }
        response
    }
}

/// How status lights are drawn (S29, an app setting): the perforated light, the pick, or a plain dot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum LightStyle {
    /// A [`PerforatedLight`] of 3 by 3 holes.
    #[default]
    Perforated,
    /// A plain filled dot, `Space::m` across.
    Dot,
}

impl LightStyle {
    /// The light's side in points at `pixels_per_point`.
    pub fn side(self, tokens: &Tokens, pixels_per_point: f32) -> f32 {
        match self {
            Self::Perforated => {
                PerforatedLight::new(Color32::WHITE).side_px(&tokens.perforation, pixels_per_point)
                    / pixels_per_point
            }
            Self::Dot => (tokens.space.m * pixels_per_point).round().max(1.0) / pixels_per_point,
        }
    }

    /// Paint a light in `tone` centred on `center`, snapped to the pixel grid.
    pub fn paint(self, painter: &egui::Painter, tokens: &Tokens, tone: Tone, center: Pos2) {
        let ppp = painter.pixels_per_point();
        let side = self.side(tokens, ppp);
        let min = ((center.to_vec2() - Vec2::splat(side * 0.5)) * ppp).round() / ppp;
        match self {
            Self::Perforated => {
                PerforatedLight::tone(tone, tokens).paint(painter, tokens, min.to_pos2())
            }
            Self::Dot => {
                let color = if tone == Tone::Off {
                    tokens.colors.line_strong
                } else {
                    tone.color(tokens)
                };
                painter.circle_filled(min.to_pos2() + Vec2::splat(side * 0.5), side * 0.5, color);
            }
        }
    }
}

/// Hole and gap in whole pixels, at least one each.
fn grid(perf: &crate::Perforation, ppp: f32) -> (f32, f32) {
    (
        (perf.hole * ppp).round().max(1.0),
        (perf.gap * ppp).round().max(1.0),
    )
}

/// One hole in `rect` (points, `side` pixels square): a plain square up to 2 pixels; from 3 pixels its corner
/// pixels are drawn at reduced strength, so a larger hole reads round and stays on the grid.
fn hole_quads(mesh: &mut Mesh, rect: Rect, side: f32, color: Color32, ppp: f32) {
    if side < 3.0 {
        mesh.add_colored_rect(rect, color);
        return;
    }
    let px = 1.0 / ppp;
    mesh.add_colored_rect(rect.shrink2(vec2(px, 0.0)), color);
    mesh.add_colored_rect(
        Rect::from_min_max(
            pos2(rect.min.x, rect.min.y + px),
            pos2(rect.min.x + px, rect.max.y - px),
        ),
        color,
    );
    mesh.add_colored_rect(
        Rect::from_min_max(
            pos2(rect.max.x - px, rect.min.y + px),
            pos2(rect.max.x, rect.max.y - px),
        ),
        color,
    );
    let corner = color.gamma_multiply(0.4);
    for (x, y) in [
        (rect.min.x, rect.min.y),
        (rect.max.x - px, rect.min.y),
        (rect.min.x, rect.max.y - px),
        (rect.max.x - px, rect.max.y - px),
    ] {
        mesh.add_colored_rect(Rect::from_min_size(pos2(x, y), Vec2::splat(px)), corner);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCALES: [f32; 3] = [1.0, 1.25, 2.0];

    /// Every vertex of every light, both themes, all three scales, lands on a whole pixel: nothing is blurred.
    #[test]
    fn every_edge_sits_on_the_pixel_grid() {
        for tokens in [Tokens::dark(), Tokens::light()] {
            for ppp in SCALES {
                for holes in 1..=6 {
                    for lit in [true, false] {
                        let light = PerforatedLight::new(tokens.colors.good)
                            .holes(holes)
                            .lit(lit);
                        let mesh = light.mesh(&tokens, pos2(10.3, 7.77), ppp);
                        for v in &mesh.vertices {
                            let p = v.pos.to_vec2() * ppp;
                            assert!(
                                (p.x - p.x.round()).abs() < 1e-3
                                    && (p.y - p.y.round()).abs() < 1e-3,
                                "{holes} holes at {ppp}x: vertex {p:?} off the grid"
                            );
                        }
                    }
                }
            }
        }
    }

    /// Holes and gaps are whole pixels and at least one; the default 3 by 3 light is 5 px at 1x and 1.25x,
    /// 10 px at 2x.
    #[test]
    fn sizes_round_to_whole_pixels() {
        let perf = Tokens::dark().perforation;
        let light = PerforatedLight::new(Color32::RED);
        assert_eq!(light.side_px(&perf, 1.0), 5.0);
        assert_eq!(light.side_px(&perf, 1.25), 5.0);
        assert_eq!(light.side_px(&perf, 2.0), 10.0);
        let tiny = crate::Perforation {
            hole: 0.1,
            gap: 0.1,
            glow: 0.0,
        };
        assert_eq!(light.side_px(&tiny, 1.0), 5.0, "never under one pixel");
    }

    /// An unlit light draws no glow, a lit one glows only between its holes: never past the field.
    #[test]
    fn glow_stays_inside_the_field() {
        let tokens = Tokens::dark();
        let light = PerforatedLight::new(tokens.colors.good);
        let min = pos2(4.0, 4.0);
        let field = Rect::from_min_size(min, Vec2::splat(light.side_px(&tokens.perforation, 1.0)));
        let lit = light.mesh(&tokens, min, 1.0);
        assert!(
            lit.vertices
                .iter()
                .all(|v| field.expand(1e-3).contains(v.pos))
        );
        let dark = light.lit(false).mesh(&tokens, min, 1.0);
        assert_eq!(dark.vertices.len(), 9 * 4, "9 holes, no glow quads");
        assert!(lit.vertices.len() > dark.vertices.len());
    }
}
