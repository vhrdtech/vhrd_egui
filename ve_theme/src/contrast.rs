//! WCAG 2 contrast: the check behind "text stays AA" (THM-2).

use egui::Color32;

/// WCAG 2 AA minimum for normal text.
pub const AA_TEXT: f32 = 4.5;

/// Relative luminance of an sRGB color (alpha ignored), WCAG 2 definition.
pub fn luminance(c: Color32) -> f32 {
    fn lin(v: u8) -> f32 {
        let v = v as f32 / 255.0;
        if v <= 0.040_45 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b())
}

/// WCAG 2 contrast ratio between two opaque colors, 1.0 ..= 21.0.
pub fn contrast(a: Color32, b: Color32) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// `fg` if it reaches `min` contrast on `bg`, otherwise `fg` moved toward `toward` (usually the text color)
/// just far enough to reach it. Keeps the hue recognisable (a warning stays orange) while making it legible.
pub fn ensure_contrast(fg: Color32, bg: Color32, toward: Color32, min: f32) -> Color32 {
    if contrast(fg, bg) >= min {
        return fg;
    }
    for step in 1..=20 {
        let c = fg.lerp_to_gamma(toward, step as f32 / 20.0);
        if contrast(c, bg) >= min {
            return c;
        }
    }
    toward
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn black_on_white_is_21() {
        assert!((contrast(Color32::BLACK, Color32::WHITE) - 21.0).abs() < 0.01);
        assert!((contrast(Color32::WHITE, Color32::WHITE) - 1.0).abs() < 0.01);
    }

    #[test]
    fn ensure_contrast_darkens_until_legible() {
        let bg = Color32::from_rgb(0xf6, 0xf6, 0xf6);
        let orange = Color32::from_rgb(0xe8, 0x71, 0x0a);
        assert!(contrast(orange, bg) < AA_TEXT);
        let fixed = ensure_contrast(orange, bg, Color32::from_rgb(0x1a, 0x1a, 0x1a), AA_TEXT);
        assert!(contrast(fixed, bg) >= AA_TEXT);
        assert!(fixed.r() > fixed.b(), "still orange-ish: {fixed:?}");
    }
}
