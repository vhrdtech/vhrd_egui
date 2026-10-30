//! The maths of persistence and tone mapping, once in Rust and mirrored in `phosphor.wgsl` (change both together).
//!
//! Per pixel the GPU keeps a *density* `d`: the hits of the sweeps drawn into it, fading with the persistence
//! time constant (`d' = d·k + hits`, `k = exp(-dt/τ)`), and the *age*, the seconds since the last hit. The tone
//! map turns density into light on a log scale: `level = ln(1 + d·intensity) / ln(1 + full)`, where `full` is the
//! density of a pixel every sweep went through (it follows the same recursion, so the scale is right at any
//! persistence and while the picture builds up). A level of 1 is drawn at the side's peak (8× SDR white on an
//! HDR screen, 1.0 on SDR), a single hit just above the ghost floor, everything between spaced evenly in stops.

/// Linear light of one colour, BT.709 primaries (extended sRGB): 1.0 is SDR white, more is brighter than that.
pub type Rgb = [f32; 3];

/// The decay factor of one frame: `exp(-dt/τ)`, 1 when the persistence is infinite.
pub fn decay_factor(dt: f32, persistence_s: f32, infinite: bool) -> f32 {
    if infinite || !persistence_s.is_finite() {
        1.0
    } else if persistence_s <= 0.0 {
        0.0
    } else {
        (-dt.max(0.0) / persistence_s).exp()
    }
}

/// One frame of a pixel's density: the old one faded by `k`, plus this frame's hits.
pub fn step_density(d: f32, hits: u32, k: f32) -> f32 {
    d * k + hits as f32
}

/// The age of a pixel after one frame: 0 when it was hit, else older by `dt` (capped, so it stays finite).
pub fn step_age(age: f32, hits: u32, dt: f32) -> f32 {
    if hits > 0 {
        0.0
    } else {
        (age + dt).min(AGE_CAP)
    }
}

/// Ages stop growing here (seconds): far beyond any colour change.
pub const AGE_CAP: f32 = 1.0e6;

/// The level of a density on the log scale, 0..=1: 1 at `full` (every sweep hit the pixel) for intensity 1.
pub fn level(d: f32, full: f32, intensity: f32) -> f32 {
    if full <= 0.0 || d <= 0.0 {
        return 0.0;
    }
    ((d * intensity).ln_1p() / full.ln_1p()).clamp(0.0, 1.0)
}

/// How much of a pixel shows: 0 below `cutoff` hits, fading in to 1 at four times that, so a ghost that has
/// decayed away goes dark instead of sitting at the floor for ever.
pub fn visibility(d: f32, cutoff: f32) -> f32 {
    smoothstep(cutoff, 4.0 * cutoff, d)
}

/// The luminance of a level: `ghost` (a single hit) at 0 up to `peak` at 1, evenly spaced in stops.
pub fn luminance(level: f32, ghost: f32, peak: f32) -> f32 {
    let ghost = ghost.max(1e-4);
    let peak = peak.max(ghost);
    ghost * (peak / ghost).powf(level.clamp(0.0, 1.0))
}

/// BT.709 luma of linear light.
pub fn luma(c: Rgb) -> f32 {
    0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
}

/// The colour scaled to luma 1 (black stays black).
pub fn unit_luma(c: Rgb) -> Rgb {
    let y = luma(c);
    if y <= 0.0 {
        [0.0; 3]
    } else {
        scale(c, 1.0 / y)
    }
}

/// Keep every channel at or below `peak`, scaling the colour as a whole so its hue stays.
pub fn fit_peak(c: Rgb, peak: f32) -> Rgb {
    let m = c[0].max(c[1]).max(c[2]);
    if m > peak && m > 0.0 {
        scale(c, peak / m)
    } else {
        c
    }
}

/// The phosphor palette: the phosphor's colour at the tone-mapped luminance, turning whiter in the hottest
/// part of the trace (as a real phosphor or film does).
pub fn phosphor(level: f32, lum: f32, color: Rgb) -> Rgb {
    let white = smoothstep(0.75, 1.0, level) * 0.7;
    scale(mix(unit_luma(color), [1.0; 3], white), lum)
}

/// The classic false-colour scale of digital-phosphor scopes (sRGB-encoded stops): dark blue for the rarest,
/// through cyan, green, yellow and red to white for the densest.
const TEMPERATURE: [(f32, Rgb); 7] = [
    (0.0, [0.10, 0.10, 0.55]),
    (0.22, [0.0, 0.45, 1.0]),
    (0.42, [0.0, 0.85, 0.75]),
    (0.60, [0.35, 1.0, 0.1]),
    (0.76, [1.0, 0.9, 0.0]),
    (0.90, [1.0, 0.3, 0.05]),
    (1.0, [1.0, 1.0, 1.0]),
];

/// The temperature palette at a level, linear light. At `peak` 1 it is the classic SDR colour scale; with more
/// headroom the dense end grows brighter (`peak^(level²)`), the rare end keeps its colour.
pub fn temperature(level: f32, peak: f32) -> Rgb {
    let l = level.clamp(0.0, 1.0);
    let mut c = TEMPERATURE[TEMPERATURE.len() - 1].1;
    for w in TEMPERATURE.windows(2) {
        let ((a, ca), (b, cb)) = (w[0], w[1]);
        if l <= b {
            c = mix(ca, cb, (l - a) / (b - a));
            break;
        }
    }
    scale(decode_rgb(c), peak.max(1.0).powf(l * l))
}

/// Fresh hits in the age palette: icy white-cyan.
pub const AGE_FRESH: Rgb = [0.75, 0.95, 1.0];
/// Old hits in the age palette: violet.
pub const AGE_OLD: Rgb = [0.5, 0.12, 1.0];

/// The age palette: the hue says how long ago the pixel was last hit (fresh white-cyan to old violet, over
/// `age_span` seconds), the luminance how dense it is. A CRT can't do this: its glow only fades.
pub fn age(level: f32, lum: f32, age_s: f32, age_span: f32) -> Rgb {
    let t = (age_s / age_span.max(1e-3)).clamp(0.0, 1.0).sqrt();
    let hue = mix(
        unit_luma(AGE_FRESH),
        unit_luma(AGE_OLD),
        smoothstep(0.0, 1.0, t),
    );
    let white = smoothstep(0.8, 1.0, level) * 0.5;
    scale(mix(hue, [1.0; 3], white), lum)
}

/// The sRGB transfer function continued past 1.0 (and mirrored below 0): what an extended-sRGB surface
/// (`SurfaceColorSpace::ExtendedSrgb`) and egui's gamma path expect.
pub fn encode(x: f32) -> f32 {
    let a = x.abs();
    let e = if a <= 0.003_130_8 {
        a * 12.92
    } else {
        1.055 * a.powf(1.0 / 2.4) - 0.055
    };
    e.copysign(x)
}

/// The inverse of [`encode`].
pub fn decode(x: f32) -> f32 {
    let a = x.abs();
    let d = if a <= 0.040_45 {
        a / 12.92
    } else {
        ((a + 0.055) / 1.055).powf(2.4)
    };
    d.copysign(x)
}

fn decode_rgb(c: Rgb) -> Rgb {
    [decode(c[0]), decode(c[1]), decode(c[2])]
}

fn scale(c: Rgb, s: f32) -> Rgb {
    [c[0] * s, c[1] * s, c[2] * s]
}

fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// GLSL/WGSL `smoothstep`.
pub fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32, tol: f32) -> bool {
        (a - b).abs() <= tol * b.abs().max(1.0)
    }

    #[test]
    fn decay_falls_to_one_over_e_after_one_time_constant() {
        // 60 frames of 1/60 s at τ = 1 s
        let k = decay_factor(1.0 / 60.0, 1.0, false);
        let mut d = 1000.0;
        for _ in 0..60 {
            d = step_density(d, 0, k);
        }
        assert!(close(d, 1000.0 / std::f32::consts::E, 1e-4), "{d}");
    }

    #[test]
    fn infinite_persistence_keeps_every_hit() {
        let k = decay_factor(0.016, 0.5, true);
        assert_eq!(k, 1.0);
        let mut d = 0.0;
        for _ in 0..1000 {
            d = step_density(d, 3, k);
        }
        assert_eq!(d, 3000.0);
        assert_eq!(decay_factor(0.016, f32::INFINITY, false), 1.0);
        assert_eq!(decay_factor(0.016, 0.0, false), 0.0);
    }

    #[test]
    fn a_pixel_every_sweep_hits_sits_at_level_one() {
        // `full` follows the same recursion as a pixel hit by all sweeps, at any persistence and while it builds
        for (tau, infinite) in [(0.05, false), (2.0, false), (1.0, true)] {
            let k = decay_factor(1.0 / 60.0, tau, infinite);
            let (mut d, mut full) = (0.0, 0.0);
            for frame in 0..400 {
                d = step_density(d, 1700, k);
                full = step_density(full, 1700, k);
                if frame % 50 == 0 {
                    assert!(close(level(d, full, 1.0), 1.0, 1e-5));
                }
            }
        }
    }

    #[test]
    fn decay_reaches_the_steady_state() {
        let k = decay_factor(1.0 / 60.0, 1.0, false);
        let mut d = 0.0;
        for _ in 0..6000 {
            d = step_density(d, 10, k);
        }
        assert!(close(d, 10.0 / (1.0 - k), 1e-3));
    }

    #[test]
    fn a_single_hit_is_a_faint_ghost_far_below_the_trace() {
        // 100k sweeps/s at τ = 1 s: full ≈ 1e5
        let full = 1.0e5;
        let one = level(1.0, full, 1.0);
        assert!(one > 0.0 && one < 0.1, "{one}");
        let (ghost, peak) = (0.03, 8.0);
        let ghost_lum = luminance(one, ghost, peak);
        let trace_lum = luminance(level(full, full, 1.0), ghost, peak);
        assert!(close(trace_lum, peak, 1e-5));
        assert!(ghost_lum > ghost && ghost_lum < 0.05, "{ghost_lum}");
        // HDR gives the trace 8x more room above the ghost than SDR does
        let sdr = luminance(1.0, ghost, 1.0) / luminance(one, ghost, 1.0);
        let hdr = trace_lum / ghost_lum;
        assert!(hdr / sdr > 5.0, "{hdr} vs {sdr}");
    }

    #[test]
    fn luminance_and_level_grow_with_density() {
        let full = 1.0e5;
        let mut last = -1.0;
        for d in [0.5, 1.0, 3.0, 10.0, 100.0, 1e3, 1e4, 1e5] {
            let l = luminance(level(d, full, 1.0), 0.03, 8.0);
            assert!(l > last, "d {d}: {l} <= {last}");
            last = l;
        }
        assert_eq!(level(0.0, full, 1.0), 0.0);
        assert_eq!(level(5.0, 0.0, 1.0), 0.0);
        assert_eq!(level(1e9, full, 1.0), 1.0);
    }

    #[test]
    fn intensity_brightens_the_rare_hits() {
        let full = 1.0e5;
        assert!(level(1.0, full, 10.0) > level(1.0, full, 1.0));
    }

    #[test]
    fn a_decayed_ghost_goes_dark() {
        assert_eq!(visibility(0.01, 0.05), 0.0);
        assert_eq!(visibility(0.2, 0.05), 1.0);
        assert!(visibility(0.1, 0.05) > 0.0 && visibility(0.1, 0.05) < 1.0);
    }

    #[test]
    fn sdr_palettes_never_pass_white_hdr_reaches_the_peak() {
        for i in 0..=100 {
            let l = i as f32 / 100.0;
            for peak in [1.0, 8.0] {
                let lum = luminance(l, 0.03, peak);
                for c in [
                    fit_peak(phosphor(l, lum, [0.3, 1.0, 0.4]), peak),
                    fit_peak(temperature(l, peak), peak),
                    fit_peak(age(l, lum, 0.2, 1.0), peak),
                ] {
                    assert!(
                        c.iter().all(|&v| (0.0..=peak + 1e-4).contains(&v)),
                        "{l} {peak} {c:?}"
                    );
                }
            }
        }
        let top = fit_peak(phosphor(1.0, 8.0, [0.3, 1.0, 0.4]), 8.0);
        assert!(close(top[0].max(top[1]).max(top[2]), 8.0, 1e-5));
        assert_eq!(temperature(1.0, 8.0), [8.0; 3]);
        assert_eq!(temperature(1.0, 1.0), [1.0; 3]);
    }

    #[test]
    fn fit_peak_keeps_the_hue() {
        let c = fit_peak([2.0, 16.0, 4.0], 8.0);
        assert_eq!(c, [1.0, 8.0, 2.0]);
        assert_eq!(fit_peak([0.5, 0.2, 0.1], 1.0), [0.5, 0.2, 0.1]);
    }

    #[test]
    fn age_turns_from_fresh_to_old() {
        let fresh = age(0.5, 1.0, 0.0, 1.0);
        let old = age(0.5, 1.0, 5.0, 1.0);
        // fresh is the greener (cyan), old the bluer (violet) of the two; both at the same luma
        assert!(fresh[1] > old[1] && old[2] > fresh[2]);
        assert!(close(luma(fresh), 1.0, 1e-4) && close(luma(old), 1.0, 1e-4));
    }

    #[test]
    fn extended_srgb_round_trips_past_white() {
        assert!(close(encode(1.0), 1.0, 1e-6));
        assert!(close(encode(0.0), 0.0, 1e-6));
        assert!(encode(8.0) > 2.0);
        for x in [-0.5, 0.0, 0.002, 0.18, 0.5, 1.0, 2.0, 8.0, 16.0] {
            assert!(close(decode(encode(x)), x, 1e-5), "{x}");
        }
        // the familiar point: 18% grey encodes to about 0.46
        assert!(close(encode(0.18), 0.4614, 1e-3));
    }
}
