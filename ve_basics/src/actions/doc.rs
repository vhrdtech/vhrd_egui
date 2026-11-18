//! The keymap file as the user wrote it, and the edits rebinds and resets make to it.
//!
//! Saving patches a file's text in place: a rebound action's entry gets its new value where it stands, a reset
//! one's entries lose their lines, a new one goes in before the first table header. Comments, entries for ids
//! no action has (a plugin not linked), refused chords and the order all stay as written. The edits apply onto
//! the file as it is on disk now; an entry the user changed there since it was loaded refuses the save.

use super::keymap::{self, Lines, Parsed, Problem, RawEntry};
use std::collections::HashMap;
use std::fmt;
use std::ops::Range;

/// Why the keymap was not saved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeymapSaveError {
    /// The file to save onto cannot be read; writing over it would lose it.
    Unreadable(Problem),
    /// The file's entry for an edited action changed on disk since it was loaded: the user's edit and the
    /// app's would collide. Load the file again ([`super::Actions::apply_keymap`]) and redo the change.
    Changed {
        /// The action whose entry changed.
        id: String,
        /// The entry's line in the file now, or `None` when it was removed or added there.
        line: Option<usize>,
    },
}

impl fmt::Display for KeymapSaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreadable(problem) => {
                write!(
                    f,
                    "keymap not saved, the file has an error to fix first: {problem}"
                )
            }
            Self::Changed {
                id,
                line: Some(line),
            } => write!(
                f,
                "keymap not saved: line {line} for {id} changed on disk since it was loaded; load it again"
            ),
            Self::Changed { id, line: None } => write!(
                f,
                "keymap not saved: the entry for {id} changed on disk since it was loaded; load it again"
            ),
        }
    }
}

impl std::error::Error for KeymapSaveError {}

/// What a rebind or a reset did to an action's entries.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Edit {
    Set(Vec<String>),
    Removed,
}

#[derive(Default)]
pub(super) struct KeymapDoc {
    /// The file as loaded, or as last saved.
    text: String,
    file: String,
    broken: Option<Problem>,
    /// The value texts of each id's entries as loaded, in file order.
    loaded: HashMap<String, Vec<String>>,
    /// Edits by id, in the order first made.
    edits: Vec<(String, Edit)>,
}

impl KeymapDoc {
    pub(super) fn loaded(text: &str, file: &str, entries: &[RawEntry]) -> Self {
        let mut loaded: HashMap<String, Vec<String>> = HashMap::new();
        for e in entries {
            loaded
                .entry(e.id.clone())
                .or_default()
                .push(text[e.value_span.clone()].to_owned());
        }
        Self {
            text: text.to_owned(),
            file: file.to_owned(),
            broken: None,
            loaded,
            edits: Vec::new(),
        }
    }

    pub(super) fn broken(text: &str, file: &str, problem: Problem) -> Self {
        Self {
            text: text.to_owned(),
            file: file.to_owned(),
            broken: Some(problem),
            ..Self::default()
        }
    }

    /// The action's entry gets these chords.
    pub(super) fn set(&mut self, id: &str, chords: Vec<String>) {
        self.edit(id, Edit::Set(chords));
    }

    /// The action's entries go, every one of them: an ignored repeat left behind would apply on the next load.
    pub(super) fn remove(&mut self, id: &str) {
        self.edit(id, Edit::Removed);
    }

    fn edit(&mut self, id: &str, edit: Edit) {
        match self.edits.iter_mut().find(|(i, _)| i == id) {
            Some((_, e)) => *e = edit,
            None => self.edits.push((id.to_owned(), edit)),
        }
    }

    /// The loaded file with the edits applied. Refused when it could not be read.
    pub(super) fn text(
        &self,
        defaults: impl Fn(&str) -> Vec<String>,
    ) -> Result<String, KeymapSaveError> {
        if let Some(problem) = &self.broken {
            return Err(KeymapSaveError::Unreadable(problem.clone()));
        }
        self.onto(&self.text, defaults)
    }

    /// `current` (the file on disk now) with the edits applied; on success it becomes the loaded text, the
    /// edits are done.
    pub(super) fn save_onto(
        &mut self,
        current: &str,
        defaults: impl Fn(&str) -> Vec<String>,
    ) -> Result<String, KeymapSaveError> {
        if current == self.text
            && let Some(problem) = &self.broken
        {
            return Err(KeymapSaveError::Unreadable(problem.clone()));
        }
        let out = self.onto(current, defaults)?;
        let lines = Lines::new(&out);
        if let Parsed::Entries(entries) = keymap::parse(&out, &self.file, &lines) {
            *self = Self::loaded(&out, &self.file, &entries);
        }
        Ok(out)
    }

    fn onto(
        &self,
        base: &str,
        defaults: impl Fn(&str) -> Vec<String>,
    ) -> Result<String, KeymapSaveError> {
        let lines = Lines::new(base);
        let entries = match keymap::parse(base, &self.file, &lines) {
            Parsed::Entries(entries) => entries,
            Parsed::Unreadable(problem) => return Err(KeymapSaveError::Unreadable(problem)),
        };
        let mut patches: Vec<(Range<usize>, String)> = Vec::new();
        let mut added = String::new();
        for (id, edit) in &self.edits {
            let here: Vec<&RawEntry> = entries.iter().filter(|e| &e.id == id).collect();
            let now: Vec<&str> = here.iter().map(|e| &base[e.value_span.clone()]).collect();
            let then: Vec<&str> = self
                .loaded
                .get(id)
                .map_or_else(Vec::new, |v| v.iter().map(String::as_str).collect());
            if now != then {
                return Err(KeymapSaveError::Changed {
                    id: id.clone(),
                    line: here.first().map(|e| e.line),
                });
            }
            match (edit, here.first()) {
                (Edit::Set(chords), Some(first)) => {
                    patches.push((first.value_span.clone(), value_text(chords)))
                }
                (Edit::Set(chords), None) => {
                    added.push_str(&format!("\"{id}\" = {}\n", value_text(chords)))
                }
                (Edit::Removed, _) => {
                    for e in &here {
                        match removal(base, &e.key_span, &e.value_span) {
                            Some(range) => patches.push((range, String::new())),
                            None => patches.push((e.value_span.clone(), value_text(&defaults(id)))),
                        }
                    }
                }
            }
        }
        if !added.is_empty() {
            let at = first_header(base, &entries);
            if at == base.len() && !base.is_empty() && !base.ends_with('\n') {
                added.insert(0, '\n');
            }
            patches.push((at..at, added));
        }
        patches.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
        let mut out = base.to_owned();
        for (range, with) in patches {
            out.replace_range(range, &with);
        }
        Ok(out)
    }
}

/// The offset of the first table header's line, or the end: new root entries must go before it.
fn first_header(text: &str, entries: &[RawEntry]) -> usize {
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let inside = entries
            .iter()
            .any(|e| e.value_span.start < offset && offset < e.value_span.end);
        if !inside && line.trim_start().starts_with('[') {
            return offset;
        }
        offset += line.len();
    }
    text.len()
}

/// The range to cut so an entry goes: its whole line when nothing else is on it, the entry and the blanks
/// after it when a comment follows, `None` when other entries share the line.
fn removal(text: &str, key: &Range<usize>, value: &Range<usize>) -> Option<Range<usize>> {
    let line_start = text[..key.start].rfind('\n').map_or(0, |i| i + 1);
    if !text[line_start..key.start].trim().is_empty() {
        return None;
    }
    let line_end = text[value.end..]
        .find('\n')
        .map_or(text.len(), |i| value.end + i);
    let after = &text[value.end..line_end];
    let rest = after.trim();
    if rest.is_empty() {
        Some(line_start..(line_end + 1).min(text.len()))
    } else if rest.starts_with('#') {
        Some(key.start..value.end + after.len() - after.trim_start().len())
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
