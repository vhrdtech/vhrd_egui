//! Binding chords to actions: declared defaults, the keymap file, rebinds; saving the keymap.

use super::doc::{KeymapBroken, KeymapDoc};
use super::keymap::{self, Lines, Origin, Parsed, Problem, ProblemKind};
use super::registry::Override;
use super::{Actions, Chord, KEYMAP_CHORDS_PER_ACTION_MAX};
use egui::Key;
use std::collections::HashMap;

impl Actions {
    /// Apply a keymap file's text: its entries replace the declared chords of the actions they name; a previous
    /// keymap's entries and rebinds are dropped. `file` names it in problems. Returns this keymap's problems
    /// (also in [`Actions::problems`]); whatever is fine in the file applies:
    /// - an unknown id, a value that is no chord list, a repeated id: the entry is ignored;
    /// - a chord that does not parse or is taken: left out, the entry's other chords apply; an entry whose
    ///   chords all fail keeps the declared ones (only `[]` unbinds);
    /// - a file that is not TOML or too large: nothing applies, and [`Actions::keymap_text`] refuses to save
    ///   over it.
    pub fn apply_keymap(&mut self, text: &str, file: &str) -> Vec<Problem> {
        for e in &mut self.entries {
            e.over = None;
        }
        self.next_seq = 0;
        let lines = Lines::new(text);
        let mut problems = Vec::new();
        match keymap::parse(text, file, &lines) {
            Parsed::Unreadable(problem) => {
                self.doc = KeymapDoc::broken(text, problem.clone());
                problems.push(problem);
            }
            Parsed::Entries(raw) => {
                self.doc = KeymapDoc::loaded(text, &raw);
                let mut seen: HashMap<&str, usize> = HashMap::new();
                for r in &raw {
                    let origin = Origin::File {
                        file: file.to_owned(),
                        line: r.line,
                    };
                    let problem = |kind| Problem {
                        origin: origin.clone(),
                        kind,
                    };
                    if let Some(&first) = seen.get(r.id.as_str()) {
                        problems.push(problem(ProblemKind::RepeatedId {
                            id: r.id.clone(),
                            kept: Origin::File {
                                file: file.to_owned(),
                                line: first,
                            },
                        }));
                        continue;
                    }
                    seen.insert(&r.id, r.line);
                    let Some(i) = self.index(&r.id) else {
                        problems.push(problem(ProblemKind::UnknownAction { id: r.id.clone() }));
                        continue;
                    };
                    let Some(texts) = &r.chords else {
                        problems.push(problem(ProblemKind::BadValue { id: r.id.clone() }));
                        continue;
                    };
                    let mut keys = Vec::with_capacity(texts.len());
                    for text in texts {
                        match Chord::parse(text) {
                            Ok(c) => keys.push(c),
                            Err(error) => problems.push(problem(ProblemKind::BadChord {
                                id: r.id.clone(),
                                text: text.clone(),
                                error,
                            })),
                        }
                    }
                    if keys.is_empty() && !texts.is_empty() {
                        continue;
                    }
                    self.entries[i].over = Some(Override {
                        seq: self.next_seq,
                        keys,
                        origin,
                    });
                    self.next_seq += 1;
                }
            }
        }
        self.keymap_problems.clone_from(&problems);
        keymap::cap(&mut self.keymap_problems);
        self.bind();
        problems.extend(
            self.bind_problems
                .iter()
                .filter(|p| matches!(p.origin, Origin::File { .. }))
                .cloned(),
        );
        keymap::cap(&mut problems);
        problems
    }

    /// Bind other chords to one action at run time (a settings window); [`Actions::keymap_text`] then saves
    /// them. Refused, with nothing changed, when it would leave any chord unbound: one held by another action
    /// of its scope, Enter on a destructive action, more than [`KEYMAP_CHORDS_PER_ACTION_MAX`].
    pub fn rebind(&mut self, id: &str, keys: Vec<Chord>) -> Result<(), Problem> {
        let refused = |kind| Problem {
            origin: Origin::Rebind,
            kind,
        };
        let i = self
            .index(id)
            .ok_or_else(|| refused(ProblemKind::UnknownAction { id: id.to_owned() }))?;
        if keys.len() > KEYMAP_CHORDS_PER_ACTION_MAX {
            return Err(refused(ProblemKind::BadValue { id: id.to_owned() }));
        }
        let before = self.bind_problems.clone();
        let old = self.entries[i].over.take();
        // The newest claim comes last, so a chord held elsewhere refuses this rebind, not the holder.
        let seq = self.next_seq;
        self.next_seq += 1;
        self.entries[i].over = Some(Override {
            seq,
            keys: keys.clone(),
            origin: Origin::Rebind,
        });
        self.bind();
        if let Some(problem) = self
            .bind_problems
            .iter()
            .find(|p| !before.contains(p))
            .cloned()
        {
            self.entries[i].over = old;
            self.bind();
            return Err(problem);
        }
        self.doc.set(id, keys.iter().map(Chord::text).collect());
        Ok(())
    }

    /// Back to the declared chords for one action; its keymap entries go when the keymap is saved.
    pub fn reset_keys(&mut self, id: &str) {
        if let Some(i) = self.index(id) {
            self.entries[i].over = None;
            self.doc.remove(id);
            self.bind();
        }
    }

    /// The keymap file to save: the file last applied, with rebinds and resets patched in where they stand.
    /// Comments, order, entries for unknown ids and refused chords stay as the user wrote them; new entries go
    /// before the first table header. Saving is safe whenever this returns `Ok`; it is refused while the file
    /// last applied could not be read, since writing over it would lose it.
    pub fn keymap_text(&self) -> Result<String, KeymapBroken> {
        self.doc.text(|id| {
            self.index(id)
                .map(|i| self.entries[i].defaults.iter().map(Chord::text).collect())
                .unwrap_or_default()
        })
    }

    /// Bind every chord again: declared chords of actions without an override first (by id), then the
    /// overrides in order. A chord already held in its scope stays with its holder and the later claim is a
    /// problem.
    pub(super) fn bind(&mut self) {
        self.by_chord.clear();
        for e in &mut self.entries {
            e.keys.clear();
        }
        let mut claims: Vec<(usize, Chord, Origin)> = Vec::new();
        for (i, e) in self.entries.iter().enumerate() {
            if e.over.is_none() {
                claims.extend(e.defaults.iter().map(|&c| (i, c, Origin::of(&e.action))));
            }
        }
        let mut overridden: Vec<(u64, usize)> = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(i, e)| e.over.as_ref().map(|o| (o.seq, i)))
            .collect();
        overridden.sort_unstable();
        for (_, i) in overridden {
            if let Some(o) = &self.entries[i].over {
                claims.extend(o.keys.iter().map(|&c| (i, c, o.origin.clone())));
            }
        }
        let mut problems = Vec::new();
        let mut warnings = Vec::new();
        for (i, chord, origin) in claims {
            let action = self.entries[i].action;
            let id = action.id.to_owned();
            if action.destructive && chord.key == Key::Enter && !chord.command && !chord.alt {
                problems.push(Problem {
                    origin,
                    kind: ProblemKind::DestructiveEnter { id },
                });
                continue;
            }
            if let Some(holder) = self.holder(action.scope, chord) {
                problems.push(Problem {
                    origin,
                    kind: ProblemKind::ChordTaken {
                        id,
                        chord,
                        holder: self.entries[holder].action.id,
                    },
                });
                continue;
            }
            self.by_chord.entry(chord).or_default().push(i);
            self.entries[i].keys.push(chord);
            if chord.is_text_editing() {
                warnings.push(Problem {
                    origin,
                    kind: ProblemKind::TextEditKeeps { id, chord },
                });
            }
        }
        keymap::cap(&mut problems);
        keymap::cap(&mut warnings);
        self.bind_problems = problems;
        self.warnings = warnings;
    }
}
