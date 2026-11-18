//! The keymap file's format, its limits and the problems found in it or in the declarations.
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
//! Every problem is reported with the file and the line, and binding goes on without the bad entry; never a
//! panic.

use super::{Chord, ChordError};
use std::fmt;
use std::ops::Range;
use toml::Spanned;
use toml::de::{DeTable, DeValue};

/// Where a binding or a problem comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Origin {
    /// An action's declaration in the code.
    Declared {
        /// The source file of the `Action::new` call.
        file: &'static str,
        /// Its line.
        line: u32,
    },
    /// A line of a keymap file.
    File {
        /// The file's name as the app passed it.
        file: String,
        /// The line, from 1.
        line: usize,
    },
    /// [`super::Actions::rebind`] at run time (a settings window).
    Rebind,
}

impl Origin {
    pub(super) fn of(action: &super::Action) -> Self {
        Self::Declared {
            file: action.location.file(),
            line: action.location.line(),
        }
    }
}

impl fmt::Display for Origin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Declared { file, line } => write!(f, "{file}:{line}"),
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
    /// Two declarations, or two keymap entries, with one id; the first is kept, the other ignored.
    RepeatedId {
        /// The id.
        id: String,
        /// Where the kept one is.
        kept: Origin,
    },
    /// A menu group with an empty part (`" "`, `"File//Recent"`), or a part spelled with other case than the same
    /// menu elsewhere (`file` against `File`).
    BadGroup {
        /// The action's id.
        id: String,
        /// The group as declared.
        group: String,
        /// What is wrong with it, and what is done instead.
        why: String,
    },
    /// A mnemonic that is not a letter of the title, or that another entry of the same menu has.
    BadMnemonic {
        /// The action's id.
        id: String,
        /// The letter.
        letter: char,
        /// The entry of the same menu that has it, or `None` when it is not in the title.
        holder: Option<&'static str>,
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
    /// A chord another action of the same scope holds already; the one claiming it later goes without it.
    ChordTaken {
        /// The action that wanted it.
        id: String,
        /// The chord.
        chord: Chord,
        /// The action holding it.
        holder: &'static str,
    },
    /// Enter bound to a destructive action, which never takes Enter; left out.
    DestructiveEnter {
        /// The action's id.
        id: String,
    },
    /// A chord a focused text edit keeps (Ctrl+C, Ctrl+Z, ...): it reaches the action only while no text edit
    /// has focus. A warning: [`super::Actions::warnings`].
    TextEditKeeps {
        /// The action's id.
        id: String,
        /// The chord.
        chord: Chord,
    },
    /// A keymap entry for an id no action has. Kept in the file when it is saved.
    UnknownAction {
        /// The id as written.
        id: String,
    },
    /// A keymap value that is neither a chord nor a list of chords, or a list longer than
    /// [`KEYMAP_CHORDS_PER_ACTION_MAX`]; the action keeps its keys.
    BadValue {
        /// The id.
        id: String,
    },
    /// The file is not valid TOML, or longer than [`KEYMAP_BYTES_MAX`] or [`KEYMAP_ENTRIES_MAX`] entries: nothing
    /// of it applies and saving over it is refused.
    Unreadable {
        /// What is wrong.
        message: String,
    },
    /// More problems than [`KEYMAP_PROBLEMS_MAX`]; this many more are not listed.
    Truncated {
        /// How many.
        more: usize,
    },
}

/// A problem in the declarations, the keymap file or a rebind, with where it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problem {
    /// Where.
    pub origin: Origin,
    /// What.
    pub kind: ProblemKind,
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: ", self.origin)?;
        match &self.kind {
            ProblemKind::BadId { id } => write!(
                f,
                "\"{id}\" is no action id (lower case, digits, '.', '_', '-'; at most 96)"
            ),
            ProblemKind::RepeatedId { id, kept } => {
                write!(f, "\"{id}\" again; ignored, the one at {kept} is kept")
            }
            ProblemKind::BadGroup { id, group, why } => {
                write!(f, "group \"{group}\" of {id}: {why}")
            }
            ProblemKind::BadMnemonic {
                id,
                letter,
                holder: Some(holder),
            } => write!(
                f,
                "mnemonic '{letter}' of {id} is taken by {holder} in the same menu"
            ),
            ProblemKind::BadMnemonic {
                id,
                letter,
                holder: None,
            } => {
                write!(
                    f,
                    "mnemonic '{letter}' of {id} is not a letter of its title"
                )
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
            ProblemKind::DestructiveEnter { id } => {
                write!(f, "{id} is destructive and never takes Enter; left out")
            }
            ProblemKind::TextEditKeeps { id, chord } => write!(
                f,
                "{chord} for {id} reaches it only while no text field has focus: a text field keeps it"
            ),
            ProblemKind::UnknownAction { id } => {
                write!(f, "no action \"{id}\"; entry ignored, kept in the file")
            }
            ProblemKind::BadValue { id } => write!(
                f,
                "{id}: expected a chord (\"Ctrl+S\") or a list of at most {KEYMAP_CHORDS_PER_ACTION_MAX}; entry ignored"
            ),
            ProblemKind::Unreadable { message } => {
                write!(f, "{message}; nothing applied, saving over it is refused")
            }
            // spirit-level: allow(ui-more-count) the rest is not kept: the cap bounds memory on a hostile file
            ProblemKind::Truncated { more } => write!(f, "{more} more problems not listed"),
        }
    }
}

/// Longest keymap file read.
pub const KEYMAP_BYTES_MAX: usize = 256 * 1024;
/// Most entries a keymap file may have.
pub const KEYMAP_ENTRIES_MAX: usize = 4096;
/// Most chords one action may have.
pub const KEYMAP_CHORDS_PER_ACTION_MAX: usize = 16;
/// Most problems listed at once; the rest are counted in one [`ProblemKind::Truncated`].
pub const KEYMAP_PROBLEMS_MAX: usize = 256;
/// Deepest table nesting followed in a keymap file.
const KEYMAP_DEPTH_MAX: usize = 8;

/// Cut a list of problems to [`KEYMAP_PROBLEMS_MAX`], counting the rest.
pub(super) fn cap(problems: &mut Vec<Problem>) {
    if problems.len() > KEYMAP_PROBLEMS_MAX {
        let more = problems.len() - KEYMAP_PROBLEMS_MAX;
        let origin = problems[KEYMAP_PROBLEMS_MAX].origin.clone();
        problems.truncate(KEYMAP_PROBLEMS_MAX);
        problems.push(Problem {
            origin,
            kind: ProblemKind::Truncated { more },
        });
    }
}

/// Byte offsets of line starts, to turn an offset into a line number without rescanning.
pub(super) struct Lines(Vec<usize>);

impl Lines {
    pub(super) fn new(text: &str) -> Self {
        let mut starts = vec![0];
        starts.extend(
            text.bytes()
                .enumerate()
                .filter(|&(_, b)| b == b'\n')
                .map(|(i, _)| i + 1),
        );
        Self(starts)
    }

    /// The line, from 1, of a byte offset.
    pub(super) fn line_of(&self, offset: usize) -> usize {
        self.0.partition_point(|&start| start <= offset)
    }
}

/// One entry of the file, as written.
pub(super) struct RawEntry {
    pub(super) id: String,
    /// The chord texts, or `None` when the value is no chord list.
    pub(super) chords: Option<Vec<String>>,
    pub(super) line: usize,
    pub(super) key_span: Range<usize>,
    pub(super) value_span: Range<usize>,
}

/// The file read: its entries in file order, or why it cannot be read at all.
pub(super) enum Parsed {
    Entries(Vec<RawEntry>),
    Unreadable(Problem),
}

pub(super) fn parse(text: &str, file: &str, lines: &Lines) -> Parsed {
    let unreadable = |offset: usize, message: String| {
        Parsed::Unreadable(Problem {
            origin: Origin::File {
                file: file.to_owned(),
                line: lines.line_of(offset),
            },
            kind: ProblemKind::Unreadable { message },
        })
    };
    if text.len() > KEYMAP_BYTES_MAX {
        return unreadable(0, format!("longer than {KEYMAP_BYTES_MAX} bytes"));
    }
    let table = match DeTable::parse(text) {
        Ok(t) => t,
        Err(e) => {
            return unreadable(
                e.span().map_or(0, |s| s.start),
                format!("not valid TOML: {}", e.message()),
            );
        }
    };
    let mut leaves = Vec::new();
    flatten(table.get_ref(), "", 0, &mut leaves);
    if leaves.len() > KEYMAP_ENTRIES_MAX {
        return unreadable(0, format!("more than {KEYMAP_ENTRIES_MAX} entries"));
    }
    leaves.sort_by_key(|(_, _, v)| v.span().start);
    let entries = leaves
        .into_iter()
        .map(|(id, key_span, value)| RawEntry {
            id,
            chords: chord_texts(value.get_ref()),
            line: lines.line_of(key_span.start),
            key_span,
            value_span: value.span(),
        })
        .collect();
    Parsed::Entries(entries)
}

fn chord_texts(value: &DeValue<'_>) -> Option<Vec<String>> {
    match value {
        DeValue::String(s) => Some(vec![s.to_string()]),
        DeValue::Array(items) if items.len() <= KEYMAP_CHORDS_PER_ACTION_MAX => items
            .iter()
            .map(|item| match item.get_ref() {
                DeValue::String(s) => Some(s.to_string()),
                _ => None,
            })
            .collect(),
        _ => None,
    }
}

type Leaf<'a, 'i> = (String, Range<usize>, &'a Spanned<DeValue<'i>>);

/// The leaves of nested tables with their dotted ids and key spans. Tables deeper than [`KEYMAP_DEPTH_MAX`]
/// become leaves, which then fail as bad values.
fn flatten<'a, 'i>(
    table: &'a DeTable<'i>,
    prefix: &str,
    depth: usize,
    out: &mut Vec<Leaf<'a, 'i>>,
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
            _ => out.push((id, key.span(), value)),
        }
        if out.len() > KEYMAP_ENTRIES_MAX {
            return;
        }
    }
}
