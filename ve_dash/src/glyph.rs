//! A status glyph in its own colour in front of a painted label (DASH-25): a [`GanttRow`](crate::GanttRow)'s
//! label, a [`BurnSeries`](crate::BurnSeries)' legend entry. Labels these widgets paint are plain text in one
//! colour; a glyph drawn into that text can't take its status colour, so it comes apart, as here.

use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontId};

/// One glyph painted in `color` before a label, a space between. Status colour never carries the meaning
/// alone: say the status in the row's or series' hover too.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Glyph {
    pub glyph: char,
    pub color: Color32,
}

impl Glyph {
    pub const fn new(glyph: char, color: Color32) -> Self {
        Self { glyph, color }
    }

    /// `text` laid out after the glyph: the glyph in its colour, the text in `text_color`, both in `font`.
    pub fn job(glyph: Option<Self>, text: &str, font: FontId, text_color: Color32) -> LayoutJob {
        let mut job = LayoutJob::default();
        if let Some(g) = glyph {
            let mut buf = [0u8; 4];
            job.append(
                g.glyph.encode_utf8(&mut buf),
                0.0,
                TextFormat::simple(font.clone(), g.color),
            );
            job.append(" ", 0.0, TextFormat::simple(font.clone(), text_color));
        }
        job.append(text, 0.0, TextFormat::simple(font, text_color));
        job
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_glyph_has_its_own_colour_the_text_the_text_colour() {
        let font = FontId::proportional(12.0);
        let job = Glyph::job(
            Some(Glyph::new('✅', Color32::GREEN)),
            "#a1",
            font.clone(),
            Color32::WHITE,
        );
        assert_eq!(job.text, "✅ #a1");
        assert_eq!(job.sections[0].format.color, Color32::GREEN);
        assert_eq!(job.sections.last().unwrap().format.color, Color32::WHITE);
        let plain = Glyph::job(None, "#a1", font, Color32::WHITE);
        assert_eq!(plain.text, "#a1");
        assert_eq!(plain.sections.len(), 1);
    }
}
