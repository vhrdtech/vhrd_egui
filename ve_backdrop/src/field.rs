//! The backdrop as plain math: what one grid mark looks like in a [`Scene`].
//!
//! Both renderers draw from this: the CPU painter calls [`Scene::sample`] per mark, the shader
//! (`backdrop.wgsl`) is the same function per pixel, fed with the same numbers ([`Scene`] becomes its uniform
//! block). Keep the two in step.

use egui::{Color32, Pos2, Rect, Vec2, vec2};

use crate::settings::Settings;

/// At most this many lit frames reach the renderer; the strongest win.
pub const MAX_LIT: usize = 32;
/// At most this many click ripples are alive at once; a new one replaces the oldest.
pub const MAX_RIPPLES: usize = 4;
/// At most this many notice rings ([`crate::Notice`]) are alive at once; a new one replaces the oldest.
pub const MAX_NOTICE_RINGS: usize = 4;
/// Every ring the renderer takes: click ripples and notice rings (the shader's array size).
#[cfg_attr(not(feature = "wgpu"), allow(dead_code))]
pub const MAX_RINGS: usize = MAX_RIPPLES + MAX_NOTICE_RINGS;

/// The peak of `q * exp(-q²)` (at `q = 1/√2`), so `lens_push` is the largest displacement.
const PUSH_PEAK: f32 = 0.428_881_94;

/// The backdrop's four colours.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Colors {
    /// Fill behind the marks.
    pub bg: Color32,
    /// A mark at rest.
    pub mark: Color32,
    /// Marks lit by a frame.
    pub glow: Color32,
    /// Marks lit by the pointer lens or a click ripple.
    pub pointer: Color32,
    /// Rings and arrows of a warning notice.
    pub warn: Color32,
    /// Rings and arrows of a critical notice.
    pub crit: Color32,
}

impl Colors {
    /// The colours for a token set: marks in `line_strong` on `bg` (low contrast in both themes), frames
    /// gather `accent` light, the pointer `text_muted`; notices in `warn` and `crit`.
    pub fn from_tokens(tokens: &ve_theme::Tokens) -> Self {
        let c = &tokens.colors;
        Self {
            bg: c.bg,
            mark: c.line_strong,
            glow: c.accent,
            pointer: c.text_muted,
            warn: c.warn,
            crit: c.crit,
        }
    }
}

/// A frame as the renderer sees it: boosts and fades already applied.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LitRect {
    pub rect: Rect,
    pub corner_radius: f32,
    pub strength: f32,
}

/// A ring at one moment: a click ripple or a notice ring.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ring {
    pub origin: Pos2,
    /// Radius of the ring now.
    pub radius: f32,
    /// 1 when new, 0 when gone.
    pub fade: f32,
    /// Width of the ring.
    pub width: f32,
    /// Opacity it adds to marks on it.
    pub light: f32,
    /// How much marks on it grow.
    pub grow: f32,
    /// How far it pushes marks outward.
    pub push: f32,
    /// The colour of its light.
    pub color: Color32,
}

impl Ring {
    /// A click ripple: the look from the settings, the pointer's colour.
    pub fn click(s: &Settings, colors: &Colors, origin: Pos2, radius: f32, fade: f32) -> Self {
        Self {
            origin,
            radius,
            fade,
            width: s.ripple_width,
            light: s.ripple_light,
            grow: s.ripple_grow,
            push: s.ripple_push,
            color: colors.pointer,
        }
    }

    /// How strongly the ring touches a mark `dist` from its origin: 1 on its crest, towards 0 away from it.
    pub fn touch(&self, dist: f32) -> f32 {
        let q = (dist - self.radius) / self.width.max(1e-3);
        (-q * q).exp() * self.fade
    }
}

/// Everything one frame of the backdrop depends on.
#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    /// The area covered; the grid is anchored at its top-left corner.
    pub rect: Rect,
    pub settings: Settings,
    pub colors: Colors,
    /// Opacity of the background fill (0 = none).
    pub bg_alpha: f32,
    /// Phase of the drift wave.
    pub phase: f32,
    /// The pointer, when the lens is on.
    pub pointer: Option<Pos2>,
    pub lit: Vec<LitRect>,
    pub rings: Vec<Ring>,
}

/// One grid mark, ready to draw.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
    pub center: Pos2,
    /// Radius of a dot, half the side of a square.
    pub half: f32,
    /// Straight (not premultiplied) gamma-space colour, 0-1.
    pub rgba: [f32; 4],
}

/// Straight gamma-space RGBA of a colour, 0-1.
pub fn rgba(color: Color32) -> [f32; 4] {
    color.to_srgba_unmultiplied().map(|c| f32::from(c) / 255.0)
}

/// Signed distance from `p` to a rounded rectangle (negative inside).
pub fn sd_round_rect(p: Pos2, rect: Rect, corner_radius: f32) -> f32 {
    let half = rect.size() * 0.5;
    let r = corner_radius.clamp(0.0, half.min_elem().max(0.0));
    let d = (p - rect.center()).abs() - half + Vec2::splat(r);
    d.max(Vec2::ZERO).length() + d.x.max(d.y).min(0.0) - r
}

impl Scene {
    /// Columns and rows of the grid.
    pub fn cells(&self) -> (usize, usize) {
        let s = self.settings.spacing;
        let n = |len: f32| (len / s).ceil().max(0.0) as usize;
        (n(self.rect.width()), n(self.rect.height()))
    }

    /// The resting centre of cell (`col`, `row`).
    pub fn cell_center(&self, col: usize, row: usize) -> Pos2 {
        let s = self.settings.spacing;
        self.rect.min + vec2((col as f32 + 0.5) * s, (row as f32 + 0.5) * s)
    }

    /// The light frames gather at `p`: 0 away from all of them, towards 1 under the strongest.
    pub fn frame_light(&self, p: Pos2) -> f32 {
        let reach = self.settings.glow_radius;
        let sum: f32 = self
            .lit
            .iter()
            .map(|l| {
                let q = sd_round_rect(p, l.rect, l.corner_radius).max(0.0) / reach;
                l.strength * (-q * q).exp()
            })
            .sum();
        1.0 - (-sum).exp()
    }

    /// The mark whose resting centre is `c`.
    pub fn sample(&self, c: Pos2) -> Sample {
        let s = &self.settings;
        let k = std::f32::consts::TAU / s.wave_length;
        let wave = (c.x * k + self.phase).sin() * (c.y * k * 0.8 - self.phase * 0.7).sin();
        let base = (s.opacity * (1.0 + s.ambient * wave)).max(0.0);

        let mut grow = 0.0;
        let mut offset = Vec2::ZERO;
        let mut pointer_light = 0.0;
        let mut ring_light = 0.0;
        let mut ring_rgba = [0.0; 4];

        if let Some(p) = self.pointer {
            let d = (c - p) / s.lens_radius;
            let f = (-d.length_sq()).exp();
            offset += d * f * (s.lens_push / PUSH_PEAK);
            grow += s.lens_grow * f;
            pointer_light += s.lens_light * f;
        }

        for ring in &self.rings {
            let d = c - ring.origin;
            let dist = d.length();
            let g = ring.touch(dist);
            let light = ring.light * g;
            ring_light += light;
            let rgba = rgba(ring.color);
            for (sum, c) in ring_rgba.iter_mut().zip(rgba) {
                *sum += c * light;
            }
            grow += ring.grow * g;
            if dist > 1e-3 {
                offset += d / dist * (g * ring.push);
            }
        }

        let frame = self.frame_light(c);
        let glow_light = s.glow_light * frame;
        grow += s.glow_grow * frame;

        let half = (s.size * 0.5 * (1.0 + grow)).clamp(0.0, s.spacing * 0.5);
        // A mark never leaves its cell: the shader looks at one cell per pixel.
        let max_offset = (s.spacing * 0.5 - half).max(0.0);
        let len = offset.length();
        if len > max_offset {
            offset *= max_offset / len.max(1e-6);
        }

        let total = base + pointer_light + glow_light + ring_light;
        let (m, p, g) = (
            rgba(self.colors.mark),
            rgba(self.colors.pointer),
            rgba(self.colors.glow),
        );
        let mix = |i: usize| {
            (m[i] * base + p[i] * pointer_light + g[i] * glow_light + ring_rgba[i])
                / total.max(1e-6)
        };
        Sample {
            center: c + offset,
            half,
            rgba: [mix(0), mix(1), mix(2), total.clamp(0.0, 1.0) * mix(3)],
        }
    }
}

#[cfg(test)]
pub(crate) fn test_scene() -> Scene {
    Scene {
        rect: Rect::from_min_size(Pos2::ZERO, vec2(400.0, 300.0)),
        settings: Settings::default(),
        colors: Colors::from_tokens(&ve_theme::Tokens::dark()),
        bg_alpha: 1.0,
        phase: 0.0,
        pointer: None,
        lit: Vec::new(),
        rings: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::pos2;

    #[test]
    fn rounded_rect_distance() {
        let r = Rect::from_min_max(pos2(10.0, 10.0), pos2(110.0, 60.0));
        assert!((sd_round_rect(pos2(60.0, 35.0), r, 0.0) + 25.0).abs() < 1e-4);
        assert!((sd_round_rect(pos2(120.0, 35.0), r, 0.0) - 10.0).abs() < 1e-4);
        assert!(sd_round_rect(pos2(110.0, 60.0), r, 0.0).abs() < 1e-4);
        // A rounded corner is further from the sharp corner point.
        assert!(sd_round_rect(pos2(110.0, 60.0), r, 8.0) > 3.0);
    }

    #[test]
    fn grid_covers_the_rect() {
        let scene = test_scene();
        let (cols, rows) = scene.cells();
        assert_eq!((cols, rows), (19, 14));
        assert_eq!(scene.cell_center(0, 0), pos2(11.0, 11.0));
    }

    #[test]
    fn a_mark_at_rest_is_dim_and_small() {
        let mut scene = test_scene();
        scene.settings.ambient = 0.0;
        let s = scene.sample(pos2(11.0, 11.0));
        assert_eq!(s.center, pos2(11.0, 11.0));
        assert_eq!(s.half, 1.0);
        assert!((s.rgba[3] - scene.settings.opacity).abs() < 1e-6);
    }

    #[test]
    fn frames_gather_light_and_the_focused_one_more() {
        let mut scene = test_scene();
        scene.settings.ambient = 0.0;
        let rect = Rect::from_min_max(pos2(100.0, 100.0), pos2(200.0, 200.0));
        let lit = |strength| LitRect {
            rect,
            corner_radius: 6.0,
            strength,
        };
        let rest = scene.sample(pos2(150.0, 150.0));
        scene.lit = vec![lit(1.0)];
        let under = scene.sample(pos2(150.0, 150.0));
        let near = scene.sample(pos2(220.0, 150.0));
        let far = scene.sample(pos2(390.0, 150.0));
        assert!(under.rgba[3] > near.rgba[3] && near.rgba[3] > far.rgba[3]);
        assert!(under.half > rest.half);
        assert!((far.rgba[3] - rest.rgba[3]).abs() < 1e-3);
        scene.lit = vec![lit(1.8)];
        assert!(scene.sample(pos2(150.0, 150.0)).rgba[3] > under.rgba[3]);
    }

    #[test]
    fn the_lens_pushes_marks_away_but_never_out_of_their_cell() {
        let mut scene = test_scene();
        scene.settings.lens_push = 100.0;
        scene.pointer = Some(pos2(100.0, 100.0));
        let (cols, rows) = scene.cells();
        let half_cell = scene.settings.spacing * 0.5;
        for row in 0..rows {
            for col in 0..cols {
                let c = scene.cell_center(col, row);
                let s = scene.sample(c);
                let d = (s.center - c).abs();
                assert!(d.x.max(d.y) + s.half <= half_cell + 1e-3);
                if c.x > 110.0 {
                    assert!(s.center.x >= c.x, "pushed away from the pointer");
                }
            }
        }
    }

    #[test]
    fn a_ring_lights_marks_at_its_radius() {
        let mut scene = test_scene();
        scene.settings.ambient = 0.0;
        scene.rings = vec![Ring::click(
            &scene.settings,
            &scene.colors,
            pos2(200.0, 150.0),
            80.0,
            1.0,
        )];
        let on = scene.sample(pos2(280.0, 150.0)).rgba[3];
        let inside = scene.sample(pos2(200.0, 150.0)).rgba[3];
        assert!(on > inside + 0.3);
    }

    #[test]
    fn a_notice_ring_tints_marks_in_its_colour() {
        let mut scene = test_scene();
        scene.settings.ambient = 0.0;
        let red = Color32::from_rgb(230, 40, 40);
        // Centred far below the view: only its arc crosses it, curved toward the origin.
        let origin = pos2(200.0, 900.0);
        scene.rings = vec![Ring {
            color: red,
            light: 0.9,
            ..Ring::click(&scene.settings, &scene.colors, origin, 700.0, 1.0)
        }];
        // On the crest straight above the origin the mark is mostly red.
        let on = scene.sample(pos2(200.0, 200.0));
        assert!(on.rgba[0] > on.rgba[2] + 0.3, "{:?}", on.rgba);
        // The arc bends toward the origin: at the sides the crest lies lower than in the middle.
        let crest_x = |x: f32| {
            (0..300)
                .map(|y| (y as f32, scene.sample(pos2(x, y as f32)).rgba[3]))
                .max_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(y, _)| y)
                .unwrap_or_default()
        };
        assert!(crest_x(0.0) > crest_x(200.0) + 10.0);
        // A faded ring leaves the marks as they were.
        scene.rings[0].fade = 0.0;
        let rest = scene.sample(pos2(200.0, 200.0));
        scene.rings.clear();
        assert_eq!(rest, scene.sample(pos2(200.0, 200.0)));
    }
}
