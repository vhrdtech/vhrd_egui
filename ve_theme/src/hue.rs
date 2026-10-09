//! Perceptual checks for categorical colours (labels, chips, series, hosts, models): OKLab distance, OKLCH hue
//! and the hue range reserved for errors and warnings (THM-10).
//!
//! Categorical colours must never read as an error or a warning, so they stay out of [`ALERT_HUES`], and the
//! colours that share a view must be clearly apart. Use these in a test next to the palette:
//!
//! ```
//! use ve_theme::{Tokens, hue};
//! let c = Tokens::dark().colors;
//! assert!(!hue::is_alert_hue(c.accent));
//! assert!(hue::delta_e(c.accent, c.accent_alt) > 0.1);
//! ```

use egui::Color32;

/// The OKLCH hue range (degrees, inclusive) reserved for error and warning: red, orange and the red-ish
/// browns / ambers between them. Pure yellow (about 100°) and magenta-pink (about 340°) stay free.
pub const ALERT_HUES: (f32, f32) = (5.0, 85.0);

/// Below this OKLCH chroma a colour reads as grey and has no meaningful hue.
const GREY_CHROMA: f32 = 0.03;

fn lin(c: u8) -> f32 {
    let c = c as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// The colour as OKLab `(L, a, b)`.
pub fn oklab(c: Color32) -> (f32, f32, f32) {
    let (r, g, b) = (lin(c.r()), lin(c.g()), lin(c.b()));
    let l = (0.412_221_47 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    (
        0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
    )
}

/// OKLab Euclidean distance (ΔE): about 0.02 is just noticeable, 0.1 or more is clearly different.
pub fn delta_e(a: Color32, b: Color32) -> f32 {
    let (a, b) = (oklab(a), oklab(b));
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2) + (a.2 - b.2).powi(2)).sqrt()
}

/// OKLCH hue in degrees `0..360`, or `None` for greys.
pub fn hue_deg(c: Color32) -> Option<f32> {
    let (_, a, b) = oklab(c);
    (a.hypot(b) >= GREY_CHROMA).then(|| b.atan2(a).to_degrees().rem_euclid(360.0))
}

/// Angle between two hues, `0..=180`; `None` if either colour is grey.
pub fn hue_gap(a: Color32, b: Color32) -> Option<f32> {
    let d = (hue_deg(a)? - hue_deg(b)?).abs();
    Some(d.min(360.0 - d))
}

/// Whether the colour sits in [`ALERT_HUES`] (greys do not).
pub fn is_alert_hue(c: Color32) -> bool {
    hue_deg(c).is_some_and(|h| h >= ALERT_HUES.0 && h <= ALERT_HUES.1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reds_and_oranges_are_alert_hues() {
        for c in [
            Color32::from_rgb(0xf2, 0x4c, 0x44),
            Color32::from_rgb(0xff, 0x9a, 0x3c),
            Color32::from_rgb(0xb8, 0x44, 0x2e),
            Color32::from_rgb(0xf0, 0x8c, 0x78),
        ] {
            assert!(is_alert_hue(c), "{c:?}");
        }
        for c in [
            Color32::from_rgb(0x11, 0x8e, 0xa1),
            Color32::from_rgb(0xaa, 0x74, 0xd4),
            Color32::from_rgb(0x5c, 0x9c, 0xf5),
            Color32::from_rgb(0x80, 0x80, 0x80),
        ] {
            assert!(!is_alert_hue(c), "{c:?}");
        }
    }

    #[test]
    fn distance_is_symmetric_and_zero_for_equal() {
        let (a, b) = (
            Color32::from_rgb(10, 200, 30),
            Color32::from_rgb(200, 10, 90),
        );
        assert_eq!(delta_e(a, a), 0.0);
        assert!((delta_e(a, b) - delta_e(b, a)).abs() < 1e-6);
    }
}
