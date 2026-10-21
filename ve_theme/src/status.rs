//! Status glyph tinted by colour (THM-15): tpm's status vocabulary (`~/git/tpm/config.toml`
//! `[task_status]`, `[project_status]`, `[log_kinds]`, checked 21 Oct 2026) mapped onto a [`Tone`], drawn
//! with the glyph the emoji-fallback font already carries (THM-14).
//!
//! egui 0.36 rasterises every glyph — emoji included — from vector outlines into a single-channel
//! coverage mask (`skrifa` in epaint); there is no COLR/CPAL or CBDT/CBLC colour-glyph table support, so a
//! glyph always takes the colour it is drawn with, the same as any other character. [`status_icon_with`]
//! is exactly `ui.label(RichText::new(glyph).color(tone.color(..)))` plus the tooltip: no special
//! rendering path exists or is needed, and none of egui's built-in or bundled fonts carry colour glyphs to
//! lose by relying on this.

use egui::{Response, RichText, Ui, WidgetText};

use crate::{Tone, UiExt};

/// One status: its glyph and the [`Tone`] it draws in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StatusGlyph {
    /// tpm's config.toml key (`"done"`, `"partial"`, ...), so a status read from tpm data can be looked up
    /// by [`status_glyph`].
    pub name: &'static str,
    /// The code point kept by the emoji-fallback font (`fonts/emoji-glyphs.txt`, THM-14).
    pub glyph: char,
    pub tone: Tone,
}

macro_rules! status_table {
    ($($name:literal => $glyph:literal, $tone:ident;)*) => {
        /// tpm's status words collapsed onto one glyph each where two of its groups share a picture
        /// (`done` is both a task status and a log kind; `testing` task status and `validation` project
        /// status both draw 🧪; `idea`, `waiting`, `blocked`/`blocker`, `dropped` are each one task status
        /// and one log kind). The tone is a judgement call made here, not read from tpm: tpm's config.toml
        /// has the glyph and the word, no colour.
        pub const STATUS_GLYPHS: &[StatusGlyph] = &[
            $(StatusGlyph { name: $name, glyph: $glyph, tone: Tone::$tone },)*
        ];
    };
}

status_table! {
    // task_status
    "done" => '✅', Good;
    "partial" => '🚧', Warn;
    "testing" => '🧪', Accent;
    "buggy" => '🐛', Crit;
    "stub" => '⬜', Neutral;
    "planned" => '📋', Neutral;
    "idea" => '💡', Neutral;
    "blocked" => '⛔', Crit;
    "waiting" => '⏳', Warn;
    "verify" => '🔍', Accent;
    "dropped" => '❌', Crit;
    // project_status (names task_status does not already carry; "validation" 🧪 reuses "testing")
    "research" => '🔬', Accent;
    "prototype" => '🛠', Accent;
    "development" => '🏗', Accent;
    "released" => '🚀', Good;
    "maintenance" => '🔧', Accent;
    "paused" => '⏸', Off;
    "archived" => '🗄', Off;
    // log_kinds (names above do not already carry; "blocker" ⛔ reuses "blocked")
    "code" => '💻', Accent;
    "decision" => '⚖', Accent;
    "info" => 'ℹ', Accent;
    "note" => '📝', Accent;
    "up" => '👍', Good;
    "new" => '🆕', Accent;
    "meeting" => '🗣', Accent;
}

/// The glyph and tone for tpm status `name` (`STATUS_GLYPHS`), or `None` if it is not one of tpm's words.
pub fn status_glyph(name: &str) -> Option<StatusGlyph> {
    STATUS_GLYPHS.iter().copied().find(|s| s.name == name)
}

/// Draw `glyph` tinted by `tone`'s colour, with `tooltip` saying what it means (status colour is never the
/// only carrier of meaning, AGENTS.md "Generous tooltips"). For a glyph outside `STATUS_GLYPHS`: an app's
/// own vocabulary, or one of tpm's groups this table does not cover yet.
pub fn status_icon_with(
    ui: &mut Ui,
    glyph: char,
    tone: Tone,
    tooltip: impl Into<WidgetText>,
) -> Response {
    let color = tone.color(&ui.tokens());
    ui.label(RichText::new(glyph.to_string()).color(color))
        .on_hover_text(tooltip)
}

/// [`status_icon_with`] for a tpm status `name`. A name outside `STATUS_GLYPHS` still draws something —
/// the bare word in the neutral tone — rather than nothing, so an app ahead of this table's vocabulary
/// does not lose the status silently (AGENTS.md "Nothing is hidden").
pub fn status_icon(ui: &mut Ui, name: &str, tooltip: impl Into<WidgetText>) -> Response {
    match status_glyph(name) {
        Some(s) => status_icon_with(ui, s.glyph, s.tone, tooltip),
        None => {
            let color = Tone::Neutral.color(&ui.tokens());
            ui.label(RichText::new(name).color(color))
                .on_hover_text(tooltip)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_name_resolves_and_unknown_does_not() {
        let done = status_glyph("done").expect("done is in the table");
        assert_eq!(done.glyph, '✅');
        assert_eq!(done.tone, Tone::Good);
        assert!(status_glyph("not-a-tpm-status").is_none());
    }

    /// Every name is unique: a lookup never has to pick between two entries.
    #[test]
    fn names_are_unique() {
        let mut names: Vec<&str> = STATUS_GLYPHS.iter().map(|s| s.name).collect();
        names.sort_unstable();
        let mut unique = names.clone();
        unique.dedup();
        assert_eq!(names, unique);
    }
}
