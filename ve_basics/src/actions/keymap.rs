//! The keymap file the user edits, and binding chords to actions.
//!
//! The file is TOML: an action's id, quoted, set to a chord, a list of chords, or an empty list to unbind it.
//! Tables nest ids, so `[file]` then `save = "Ctrl+S"` is the same as `"file.save" = "Ctrl+S"`.
//!
//! ```toml
//! "file.save" = "Ctrl+S"
//! "edit.redo" = ["Ctrl+Shift+Z", "Ctrl+Y"]
//! "view.theme" = []
//! ```
//!
//! Every problem is reported with the file and the line, and binding goes on without the bad entry: an unknown
//! id, a chord that does not parse, a chord another action holds. Never a panic.

use super::{Actions, Chord, ChordError};
use std::fmt;
use toml::Spanned;
use toml::de::{DeTable, DeValue};

/// Where a binding or a problem comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Origin {
    /// The action's declaration in the code.
    Declared,
    /// A line of a keymap file.
    File {
        /// The file's name as the app passed it.
        file: String,
        /// The line, from 1.
        line: usize,
    },
    /// [`Actions::rebind`] at run time (a settings window).
    Rebind,
}

impl fmt::Display for Origin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Declared => write!(f, "declared actions"),
            Self::File { file, line } => write!(f, "{file}:{line}"),
            Self::Rebind => write!(f, "rebind"),
        }
    }
}

/// What is wrong.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProblemKind {
    /// An id with characters other than lower case, digits, `.`, `_`, `-`, or empty, or too long.
    BadId {
        /// The id.
        id: String,
    },
    /// Two declarations with one id; the first by registry order is kept.
    RepeatedId {
        /// The id.
        id: String,
    },
    /// A chord that does not parse; the action keeps its other chords.
    BadChord {
        /// The action's id.
        id: String,
        /// The chord as written.
        text: String,
        /// Why it did not parse.
        error: ChordError,
    },
    /// A chord another action holds already; the one claiming it later goes without it.
    ChordTaken {
        /// The action that wanted it.
        id: String,
        /// The chord.
        chord: Chord,
        /// The action holding it.
        holder: &'static str,
    },
    /// A keymap entry for an id no action has.
    UnknownAction {
        /// The id as written.
        id: String,
    },
    /// A keymap value that is neither a chord nor a list of chords.
    BadValue {
        /// The id.
        id: String,
    },
    /// The file is not valid TOML; nothing of it applies.
    Syntax {
        /// The parser's message.
        message: String,
    },
    /// The file is longer than [`KEYMAP_BYTES_MAX`] or has more than [`KEYMAP_ENTRIES_MAX`] entries; nothing of it
    /// applies.
    TooLarge,
}

/// A problem in the declarations, the keymap file or a rebind, with where it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problem {
    /// Where.
    pub origin: Origin,
    /// What.
    pub kind: ProblemKind,
}

impl Problem {
    pub(super) fn declared(kind: ProblemKind) -> Self {
        Self {
            origin: Origin::Declared,
            kind,
        }
    }
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: ", self.origin)?;
        match &self.kind {
            ProblemKind::BadId { id } => write!(
                f,
                "\"{id}\" is no action id (lower case, digits, '.', '_', '-'; at most 96)"
            ),
            ProblemKind::RepeatedId { id } => {
                write!(f, "\"{id}\" is declared twice; the first is kept")
            }
            ProblemKind::BadChord { id, text, error } => {
                write!(f, "\"{text}\" for {id}: {error}; left out")
            }
            ProblemKind::ChordTaken { id, chord, holder } if *holder == id.as_str() => {
                write!(f, "{chord} is given to {id} twice")
            }
            ProblemKind::ChordTaken { id, chord, holder } => write!(
                f,
                "{chord} for {id} is taken by {holder}; left out (unbind it there first: \"{holder}\" = [])"
            ),
            ProblemKind::UnknownAction { id } => write!(f, "no action \"{id}\"; entry ignored"),
            ProblemKind::BadValue { id } => write!(
                f,
                "{id}: expected a chord (\"Ctrl+S\") or a list of chords; entry ignored"
            ),
            ProblemKind::Syntax { message } => {
                write!(f, "not valid TOML, nothing applied: {message}")
            }
            ProblemKind::TooLarge => write!(
                f,
                "longer than {KEYMAP_BYTES_MAX} bytes or {KEYMAP_ENTRIES_MAX} entries, nothing applied"
            ),
        }
    }
}

/// Longest keymap file read.
pub const KEYMAP_BYTES_MAX: usize = 256 * 1024;
/// Most entries a keymap file may have.
pub const KEYMAP_ENTRIES_MAX: usize = 4096;
/// Deepest table nesting followed in a keymap file.
const KEYMAP_DEPTH_MAX: usize = 8;

/// One action's chords replaced, from a keymap line or a rebind.
pub(super) struct Override {
    entry: usize,
    keys: Vec<Chord>,
    origin: Origin,
}

impl Actions {
    /// Apply a keymap file's text: its entries replace the default chords of the actions they name, and a
    /// previous keymap's entries and runtime rebinds are dropped. `file` names it in problems. Returns this
    /// keymap's problems (also kept in [`Actions::problems`]); whatever is fine in the file applies.
    pub fn apply_keymap(&mut self, text: &str, file: &str) -> Vec<Problem> {
        let mut problems = Vec::new();
        self.overrides = parse(self, text, file, &mut problems);
        self.keymap_problems.clone_from(&problems);
        self.bind();
        problems.extend(
            self.bind_problems
                .iter()
                .filter(|p| matches!(p.origin, Origin::File { .. }))
                .cloned(),
        );
        problems
    }

    /// Bind other chords to one action at run time (a settings window). Refused, with nothing changed, when a
    /// chord is held by another action.
    pub fn rebind(&mut self, id: &str, keys: Vec<Chord>) -> Result<(), Problem> {
        let entry = self.index(id).ok_or_else(|| Problem {
            origin: Origin::Rebind,
            kind: ProblemKind::UnknownAction { id: id.to_owned() },
        })?;
        let keep = self.overrides.len();
        self.overrides.push(Override {
            entry,
            keys,
            origin: Origin::Rebind,
        });
        self.bind();
        let taken = self
            .bind_problems
            .iter()
            .find(|p| {
                p.origin == Origin::Rebind
                    && matches!(&p.kind, ProblemKind::ChordTaken { id: i, .. } if i == id)
            })
            .cloned();
        match taken {
            Some(problem) => {
                self.overrides.truncate(keep);
                self.bind();
                Err(problem)
            }
            None => Ok(()),
        }
    }

    /// Back to the declared chords for one action.
    pub fn reset_keys(&mut self, id: &str) {
        if let Some(entry) = self.index(id) {
            self.overrides.retain(|o| o.entry != entry);
            self.bind();
        }
    }

    /// The keymap file that reproduces the bindings now: one line per action whose chords differ from its
    /// declared ones, sorted by id. [`Actions::apply_keymap`] reads it back.
    pub fn keymap_text(&self) -> String {
        let mut out = String::new();
        for e in &self.entries {
            if e.keys == e.defaults {
                continue;
            }
            let chords: Vec<String> = e.keys.iter().map(|c| format!("\"{c}\"")).collect();
            out.push_str(&format!("\"{}\" = [{}]\n", e.action.id, chords.join(", ")));
        }
        out
    }

    /// Bind every chord again: declared chords of actions without an override first (by id), then the
    /// overrides in order. A chord already held stays with its holder and the later claim is a problem.
    pub(super) fn bind(&mut self) {
        let overridden: Vec<bool> = {
            let mut v = vec![false; self.entries.len()];
            for o in &self.overrides {
                v[o.entry] = true;
            }
            v
        };
        self.by_chord.clear();
        let mut problems = Vec::new();
        for e in &mut self.entries {
            e.keys.clear();
        }
        let mut claims: Vec<(usize, Chord, Origin)> = Vec::new();
        for (i, e) in self.entries.iter().enumerate() {
            if !overridden[i] {
                claims.extend(e.defaults.iter().map(|&c| (i, c, Origin::Declared)));
            }
        }
        // A later override of the same action replaces an earlier one.
        for (n, o) in self.overrides.iter().enumerate() {
            if self.overrides[n + 1..]
                .iter()
                .any(|later| later.entry == o.entry)
            {
                continue;
            }
            claims.extend(o.keys.iter().map(|&c| (o.entry, c, o.origin.clone())));
        }
        for (i, chord, origin) in claims {
            if let Some(&holder) = self.by_chord.get(&chord) {
                problems.push(Problem {
                    origin,
                    kind: ProblemKind::ChordTaken {
                        id: self.entries[i].action.id.to_owned(),
                        chord,
                        holder: self.entries[holder].action.id,
                    },
                });
                continue;
            }
            self.by_chord.insert(chord, i);
            self.entries[i].keys.push(chord);
        }
        self.bind_problems = problems;
    }
}

/// Read the file into overrides in line order; problems for what does not apply.
fn parse(actions: &Actions, text: &str, file: &str, problems: &mut Vec<Problem>) -> Vec<Override> {
    let at = |offset: usize| Origin::File {
        file: file.to_owned(),
        line: line_of(text, offset),
    };
    if text.len() > KEYMAP_BYTES_MAX {
        problems.push(Problem {
            origin: at(0),
            kind: ProblemKind::TooLarge,
        });
        return Vec::new();
    }
    let table = match DeTable::parse(text) {
        Ok(t) => t,
        Err(e) => {
            problems.push(Problem {
                origin: at(e.span().map_or(0, |s| s.start)),
                kind: ProblemKind::Syntax {
                    message: e.message().to_owned(),
                },
            });
            return Vec::new();
        }
    };
    let mut leaves: Vec<(String, &Spanned<DeValue<'_>>)> = Vec::new();
    flatten(table.get_ref(), "", 0, &mut leaves);
    if leaves.len() > KEYMAP_ENTRIES_MAX {
        problems.push(Problem {
            origin: at(0),
            kind: ProblemKind::TooLarge,
        });
        return Vec::new();
    }
    leaves.sort_by_key(|(_, v)| v.span().start);
    let mut overrides = Vec::with_capacity(leaves.len());
    for (id, value) in leaves {
        let origin = at(value.span().start);
        let Some(entry) = actions.index(&id) else {
            problems.push(Problem {
                origin,
                kind: ProblemKind::UnknownAction { id },
            });
            continue;
        };
        let texts: Vec<&str> = match value.get_ref() {
            DeValue::String(s) => vec![s.as_ref()],
            DeValue::Array(items) => {
                let texts: Option<Vec<&str>> = items
                    .iter()
                    .map(|item| match item.get_ref() {
                        DeValue::String(s) => Some(s.as_ref()),
                        _ => None,
                    })
                    .collect();
                let Some(texts) = texts else {
                    problems.push(Problem {
                        origin,
                        kind: ProblemKind::BadValue { id },
                    });
                    continue;
                };
                texts
            }
            _ => {
                problems.push(Problem {
                    origin,
                    kind: ProblemKind::BadValue { id },
                });
                continue;
            }
        };
        let mut keys = Vec::with_capacity(texts.len());
        for text in texts {
            match Chord::parse(text) {
                Ok(c) => keys.push(c),
                Err(error) => problems.push(Problem {
                    origin: origin.clone(),
                    kind: ProblemKind::BadChord {
                        id: id.clone(),
                        text: text.to_owned(),
                        error,
                    },
                }),
            }
        }
        overrides.push(Override {
            entry,
            keys,
            origin,
        });
    }
    overrides
}

/// The leaves of nested tables with their dotted ids. Tables deeper than [`KEYMAP_DEPTH_MAX`] become leaves,
/// which then fail as bad values.
fn flatten<'a, 'i>(
    table: &'a DeTable<'i>,
    prefix: &str,
    depth: usize,
    out: &mut Vec<(String, &'a Spanned<DeValue<'i>>)>,
) {
    for (key, value) in table {
        let id = if prefix.is_empty() {
            key.get_ref().to_string()
        } else {
            format!("{prefix}.{}", key.get_ref())
        };
        match value.get_ref() {
            DeValue::Table(inner) if depth < KEYMAP_DEPTH_MAX => {
                flatten(inner, &id, depth + 1, out)
            }
            _ => out.push((id, value)),
        }
        if out.len() > KEYMAP_ENTRIES_MAX {
            return;
        }
    }
}

fn line_of(text: &str, offset: usize) -> usize {
    let end = offset.min(text.len());
    text.as_bytes()[..end]
        .iter()
        .filter(|&&b| b == b'\n')
        .count()
        + 1
}
