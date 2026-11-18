//! The keymap file as the user wrote it, and the edits rebinds make to it.
//!
//! Saving patches the loaded text in place: a rebound entry gets its new value where it stands, a reset one
//! loses its line, a new one goes in before the first table header. Comments, entries for ids no action has
//! (a plugin not linked), refused chords and the order all stay as written.

use super::keymap::{Problem, RawEntry};
use std::fmt;
use std::ops::Range;

/// Saving the keymap was refused: the loaded file could not be read, and writing over it would lose it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeymapBroken {
    /// Why the loaded file could not be read.
    pub problem: Problem,
}

impl fmt::Display for KeymapBroken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "keymap not saved, the file has an error to fix first: {}",
            self.problem
        )
    }
}

impl std::error::Error for KeymapBroken {}

/// What a rebind did to an entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Edit {
    Unchanged,
    Set(Vec<String>),
    Removed,
}

pub(super) struct DocEntry {
    pub(super) id: String,
    pub(super) edit: Edit,
    /// Key and value spans in the loaded text; `None` for an entry a rebind added.
    spans: Option<(Range<usize>, Range<usize>)>,
}

#[derive(Default)]
pub(super) struct KeymapDoc {
    text: String,
    broken: Option<Problem>,
    pub(super) entries: Vec<DocEntry>,
}

impl KeymapDoc {
    pub(super) fn loaded(text: &str, entries: &[RawEntry]) -> Self {
        Self {
            text: text.to_owned(),
            broken: None,
            entries: entries
                .iter()
                .map(|e| DocEntry {
                    id: e.id.clone(),
                    edit: Edit::Unchanged,
                    spans: Some((e.key_span.clone(), e.value_span.clone())),
                })
                .collect(),
        }
    }

    pub(super) fn broken(text: &str, problem: Problem) -> Self {
        Self {
            text: text.to_owned(),
            broken: Some(problem),
            entries: Vec::new(),
        }
    }

    /// The first entry for `id` gets these chords, or a new entry is added.
    pub(super) fn set(&mut self, id: &str, chords: Vec<String>) {
        match self.entries.iter_mut().find(|e| e.id == id) {
            Some(e) => e.edit = Edit::Set(chords),
            None => self.entries.push(DocEntry {
                id: id.to_owned(),
                edit: Edit::Set(chords),
                spans: None,
            }),
        }
    }

    /// Every entry for `id` goes.
    pub(super) fn remove(&mut self, id: &str) {
        self.entries.retain(|e| e.id != id || e.spans.is_some());
        for e in self.entries.iter_mut().filter(|e| e.id == id) {
            e.edit = Edit::Removed;
        }
    }

    /// The file with the edits applied. `defaults` gives an action's declared chords, written where an entry
    /// cannot lose its line (it shares it with others, in an inline table).
    pub(super) fn text(
        &self,
        defaults: impl Fn(&str) -> Vec<String>,
    ) -> Result<String, KeymapBroken> {
        if let Some(problem) = &self.broken {
            return Err(KeymapBroken {
                problem: problem.clone(),
            });
        }
        let text = &self.text;
        let mut edits: Vec<(Range<usize>, String)> = Vec::new();
        let mut added = String::new();
        for e in &self.entries {
            match (&e.edit, &e.spans) {
                (Edit::Unchanged, _) | (Edit::Removed, None) => {}
                (Edit::Set(chords), Some((_, value))) => {
                    edits.push((value.clone(), value_text(chords)))
                }
                (Edit::Set(chords), None) => {
                    added.push_str(&format!("\"{}\" = {}\n", e.id, value_text(chords)));
                }
                (Edit::Removed, Some((key, value))) => match removal(text, key, value) {
                    Some(range) => edits.push((range, String::new())),
                    None => edits.push((value.clone(), value_text(&defaults(&e.id)))),
                },
            }
        }
        if !added.is_empty() {
            let at = self.first_header();
            if at == text.len() && !text.is_empty() && !text.ends_with('\n') {
                added.insert(0, '\n');
            }
            edits.push((at..at, added));
        }
        edits.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
        let mut out = text.clone();
        for (range, with) in edits {
            out.replace_range(range, &with);
        }
        Ok(out)
    }

    /// The offset of the first table header's line, or the end: new root entries must go before it.
    fn first_header(&self) -> usize {
        let values: Vec<&Range<usize>> = self
            .entries
            .iter()
            .filter_map(|e| e.spans.as_ref().map(|s| &s.1))
            .collect();
        let mut offset = 0;
        for line in self.text.split_inclusive('\n') {
            let inside = values.iter().any(|v| v.start < offset && offset < v.end);
            if !inside && line.trim_start().starts_with('[') {
                return offset;
            }
            offset += line.len();
        }
        self.text.len()
    }
}

/// The range to cut so an entry goes: its whole line when nothing else is on it, the entry alone when a
/// comment follows it, `None` when other entries share the line.
fn removal(text: &str, key: &Range<usize>, value: &Range<usize>) -> Option<Range<usize>> {
    let line_start = text[..key.start].rfind('\n').map_or(0, |i| i + 1);
    if !text[line_start..key.start].trim().is_empty() {
        return None;
    }
    let line_end = text[value.end..]
        .find('\n')
        .map_or(text.len(), |i| value.end + i);
    let rest = text[value.end..line_end].trim();
    if rest.is_empty() {
        Some(line_start..(line_end + 1).min(text.len()))
    } else if rest.starts_with('#') {
        let blank = text[value.end..line_end].len() - text[value.end..line_end].trim_start().len();
        Some(key.start..value.end + blank)
    } else {
        None
    }
}

fn value_text(chords: &[String]) -> String {
    let quoted: Vec<String> = chords
        .iter()
        .map(|c| format!("\"{}\"", c.replace('\\', "\\\\").replace('"', "\\\"")))
        .collect();
    format!("[{}]", quoted.join(", "))
}
