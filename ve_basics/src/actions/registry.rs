//! The registry: declarations checked once, their state each frame, firing.

use super::doc::KeymapDoc;
use super::keymap::{Origin, Problem, ProblemKind};
use super::{Action, ActionState, Chord, Fired, Refused, Source};
use std::borrow::Cow;
use std::collections::{HashMap, VecDeque};
use std::fmt;

/// Most fired actions kept for [`Actions::take_fired`]; older ones are dropped and counted.
pub const FIRED_MAX: usize = 64;

/// Top-level menus in this order unless [`Actions::menu_order`] says otherwise; others follow by name.
pub const DEFAULT_MENU_ORDER: &[&str] = &["File", "Edit", "View", "Go", "Tools", "Window", "Help"];

/// Longest id accepted.
const ID_CHARS_MAX: usize = 96;

type Handler = Box<dyn FnMut(&Fired)>;

/// Chords that replace an action's declared ones: from the keymap file or a rebind. `seq` orders the claims.
pub(super) struct Override {
    pub(super) seq: u64,
    pub(super) keys: Vec<Chord>,
    pub(super) origin: Origin,
}

pub(super) struct Entry {
    pub(super) action: Action,
    /// The menu path, cleaned: no empty parts, each part spelled as the first action naming that menu.
    pub(super) menu: Vec<String>,
    pub(super) defaults: Vec<Chord>,
    pub(super) keys: Vec<Chord>,
    pub(super) over: Option<Override>,
    enabled: Result<(), Cow<'static, str>>,
    checked: bool,
    handler: Option<Handler>,
}

/// The app's actions: declarations, bound keys, enabled and checked state, handlers, and what fired.
///
/// Not `Send`: it holds handlers and lives on the UI thread.
pub struct Actions {
    /// Sorted by id.
    pub(super) entries: Vec<Entry>,
    by_id: HashMap<&'static str, usize>,
    /// The entries holding each chord, at most one per scope.
    pub(super) by_chord: HashMap<Chord, Vec<usize>>,
    /// Problems of the declarations: ids, groups, mnemonics, default chords.
    pub(super) declared_problems: Vec<Problem>,
    /// Problems of the keymap file's text: unknown ids, bad values and chords, repeated ids.
    pub(super) keymap_problems: Vec<Problem>,
    /// Problems of binding: chords claimed twice, Enter on a destructive action.
    pub(super) bind_problems: Vec<Problem>,
    /// Chords bound that a focused text edit keeps.
    pub(super) warnings: Vec<Problem>,
    pub(super) doc: KeymapDoc,
    pub(super) next_seq: u64,
    /// The active scopes, as the declarations spell them: setting them each frame allocates nothing.
    pub(super) scopes: Vec<&'static str>,
    fired: VecDeque<Fired>,
    fired_dropped: usize,
    pub(super) menu_order: Vec<String>,
}

impl fmt::Debug for Actions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Actions")
            .field("ids", &self.ids())
            .field("problems", &self.problems().count())
            .field("fired", &self.fired)
            .finish_non_exhaustive()
    }
}

impl Actions {
    /// Every action registered with `inventory::submit!` in the crates linked into the app.
    pub fn from_registry() -> Self {
        Self::from_list(inventory::iter::<Action>.into_iter().copied())
    }

    /// The given actions only (tests, or an app composing its own set). Problems in them are kept in
    /// [`Actions::problems`], never a panic: a bad id (left out), the same id twice (the declaration first by
    /// file and line is kept, so the pick does not depend on link order), a bad group or mnemonic, a default
    /// chord that does not parse or is taken.
    pub fn from_list(actions: impl IntoIterator<Item = Action>) -> Self {
        let mut list: Vec<Action> = actions.into_iter().collect();
        list.sort_by_key(|a| {
            (
                a.id,
                a.location.file(),
                a.location.line(),
                a.location.column(),
            )
        });
        let mut problems = Vec::new();
        let mut entries: Vec<Entry> = Vec::with_capacity(list.len());
        for action in list {
            if !valid_id(action.id) {
                problems.push(Problem {
                    origin: Origin::of(&action),
                    kind: ProblemKind::BadId {
                        id: action.id.to_owned(),
                    },
                });
                continue;
            }
            if let Some(kept) = entries.last().filter(|e| e.action.id == action.id) {
                problems.push(Problem {
                    origin: Origin::of(&action),
                    kind: ProblemKind::RepeatedId {
                        id: action.id.to_owned(),
                        kept: Origin::of(&kept.action),
                    },
                });
                continue;
            }
            let mut defaults = Vec::with_capacity(action.keys.len());
            for text in action.keys {
                match Chord::parse(text) {
                    Ok(chord) => defaults.push(chord),
                    Err(error) => problems.push(Problem {
                        origin: Origin::of(&action),
                        kind: ProblemKind::BadChord {
                            id: action.id.to_owned(),
                            text: (*text).to_owned(),
                            error,
                        },
                    }),
                }
            }
            entries.push(Entry {
                action,
                menu: Vec::new(),
                defaults,
                keys: Vec::new(),
                over: None,
                enabled: Ok(()),
                checked: false,
                handler: None,
            });
        }
        clean_groups(&mut entries, &mut problems);
        check_mnemonics(&entries, &mut problems);
        let by_id = entries
            .iter()
            .enumerate()
            .map(|(i, e)| (e.action.id, i))
            .collect();
        let mut actions = Self {
            entries,
            by_id,
            by_chord: HashMap::new(),
            declared_problems: problems,
            keymap_problems: Vec::new(),
            bind_problems: Vec::new(),
            warnings: Vec::new(),
            doc: KeymapDoc::default(),
            next_seq: 0,
            scopes: Vec::new(),
            fired: VecDeque::new(),
            fired_dropped: 0,
            menu_order: DEFAULT_MENU_ORDER.iter().map(|&s| s.to_owned()).collect(),
        };
        actions.bind();
        actions
    }

    /// Top-level menus in this order; menus not named follow by name.
    pub fn menu_order(mut self, names: &[&str]) -> Self {
        self.menu_order = names.iter().map(|&s| s.to_owned()).collect();
        self
    }

    /// Every id, sorted. A test compares it with the expected list so an action added or lost shows.
    pub fn ids(&self) -> Vec<&'static str> {
        self.entries.iter().map(|e| e.action.id).collect()
    }

    /// Every action as it stands now, sorted by id.
    pub fn iter(&self) -> impl Iterator<Item = ActionState<'_>> {
        self.entries.iter().map(Entry::state)
    }

    /// One action as it stands now.
    pub fn get(&self, id: &str) -> Option<ActionState<'_>> {
        self.index(id).map(|i| self.entries[i].state())
    }

    /// The global action a chord is bound to.
    pub fn by_chord(&self, chord: Chord) -> Option<&'static str> {
        self.by_chord_in("", chord)
    }

    /// The action a chord is bound to in one scope (`""` for global), without falling back to global.
    pub fn by_chord_in(&self, scope: &str, chord: Chord) -> Option<&'static str> {
        self.holder(scope, chord).map(|i| self.entries[i].action.id)
    }

    /// Every action bound to `chord`, in any scope, as (scope, id), `""` for global: a settings window shows
    /// where a chord is used and what it would clash with.
    pub fn bound_to(&self, chord: Chord) -> Vec<(&'static str, &'static str)> {
        self.by_chord.get(&chord).map_or_else(Vec::new, |held| {
            held.iter()
                .map(|&i| (self.entries[i].action.scope, self.entries[i].action.id))
                .collect()
        })
    }

    /// Everything wrong with the declarations, the keymap file and binding, in that order, at most
    /// [`super::KEYMAP_PROBLEMS_MAX`] of the file's. Empty in a healthy app; a test can assert so.
    pub fn problems(&self) -> impl Iterator<Item = &Problem> {
        self.declared_problems
            .iter()
            .chain(&self.keymap_problems)
            .chain(&self.bind_problems)
    }

    /// Bindings that work, with a catch worth showing in a settings window: chords a focused text edit keeps
    /// ([`ProblemKind::TextEditKeeps`]).
    pub fn warnings(&self) -> impl Iterator<Item = &Problem> {
        self.warnings.iter()
    }

    /// The keyboard scopes active now, innermost first (`["tracks"]` while the tracks panel has focus). An
    /// action with a scope fires on its keys only while its scope is named here; a scope no action has is
    /// ignored. Meant for every frame: it reuses its storage.
    pub fn set_scopes(&mut self, scopes: &[&str]) {
        self.scopes.clear();
        for &scope in scopes {
            if let Some(e) = self
                .entries
                .iter()
                .find(|e| !scope.is_empty() && e.action.scope == scope)
            {
                self.scopes.push(e.action.scope);
            }
        }
    }

    /// Whether the action can run now: `Ok(())`, or `Err(reason)` saying why not and what enables it ("Nothing to
    /// redo: the last step was undone already"). Menus show the reason under a disabled entry; a key press on
    /// it returns [`Refused::Disabled`]. Set it each frame, or when the state changes.
    ///
    /// An unknown id is a mistake in the app's code: debug builds assert, release builds ignore it.
    pub fn set_enabled(&mut self, id: &str, enabled: Result<(), Cow<'static, str>>) {
        let found = self.index(id);
        debug_assert!(found.is_some(), "set_enabled: no action \"{id}\"");
        if let Some(i) = found {
            self.entries[i].enabled = enabled;
        }
    }

    /// A toggle's state, shown as a check mark.
    ///
    /// An unknown id or an action that is no toggle is a mistake in the app's code: debug builds assert,
    /// release builds ignore it.
    pub fn set_checked(&mut self, id: &str, on: bool) {
        let found = self.index(id).filter(|&i| self.entries[i].action.toggle);
        debug_assert!(found.is_some(), "set_checked: no toggle \"{id}\"");
        if let Some(i) = found {
            self.entries[i].checked = on;
        }
    }

    /// Run `handler` whenever the action fires, instead of queueing it for [`Actions::take_fired`]. It runs
    /// outside egui's locks, so it may call into an `egui::Context` it captured.
    ///
    /// An unknown id is a mistake in the app's code: debug builds assert, release builds ignore it.
    pub fn on(&mut self, id: &str, handler: impl FnMut(&Fired) + 'static) {
        let found = self.index(id);
        debug_assert!(found.is_some(), "on: no action \"{id}\"");
        if let Some(i) = found {
            self.entries[i].handler = Some(Box::new(handler));
        }
    }

    /// Fire an action: run its handler or queue it. Refused when unknown or disabled.
    pub fn trigger(&mut self, id: &str, source: Source) -> Result<(), Refused> {
        let i = self
            .index(id)
            .ok_or_else(|| Refused::Unknown(id.to_owned()))?;
        self.fire(i, source, None)
    }

    /// The actions fired since the last call that have no handler, oldest first.
    pub fn take_fired(&mut self) -> Vec<Fired> {
        self.fired.drain(..).collect()
    }

    /// How many fired actions were dropped because nobody took them while more than [`FIRED_MAX`] queued.
    pub fn fired_dropped(&self) -> usize {
        self.fired_dropped
    }

    /// The entry holding `chord` in `scope`.
    pub(super) fn holder(&self, scope: &str, chord: Chord) -> Option<usize> {
        self.by_chord
            .get(&chord)?
            .iter()
            .copied()
            .find(|&i| self.entries[i].action.scope == scope)
    }

    pub(super) fn index(&self, id: &str) -> Option<usize> {
        self.by_id.get(id).copied()
    }

    pub(super) fn fire(
        &mut self,
        i: usize,
        source: Source,
        pasted: Option<String>,
    ) -> Result<(), Refused> {
        let entry = &mut self.entries[i];
        if let Err(reason) = &entry.enabled {
            return Err(Refused::Disabled {
                id: entry.action.id,
                reason: reason.clone(),
                source,
            });
        }
        let fired = Fired {
            id: entry.action.id,
            source,
            pasted,
        };
        if let Some(handler) = &mut entry.handler {
            handler(&fired);
            return Ok(());
        }
        if self.fired.len() == FIRED_MAX {
            self.fired.pop_front();
            self.fired_dropped += 1;
        }
        self.fired.push_back(fired);
        Ok(())
    }
}

impl Entry {
    fn state(&self) -> ActionState<'_> {
        ActionState {
            action: &self.action,
            keys: &self.keys,
            default_keys: &self.defaults,
            enabled: self.enabled.as_ref().map(|_| ()).map_err(|r| r.as_ref()),
            checked: self.action.toggle.then_some(self.checked),
        }
    }
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= ID_CHARS_MAX
        && id.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-')
        })
}

/// Split each group into its menu path. A path with an empty part puts the action in no menu; a part spelled
/// with other case than the same menu met first (by id) takes that spelling. Both are problems.
fn clean_groups(entries: &mut [Entry], problems: &mut Vec<Problem>) {
    let mut spelled: HashMap<String, String> = HashMap::new();
    for e in entries.iter_mut() {
        let group = e.action.group;
        if group.is_empty() {
            continue;
        }
        let bad = |why: &str| Problem {
            origin: Origin::of(&e.action),
            kind: ProblemKind::BadGroup {
                id: e.action.id.to_owned(),
                group: group.to_owned(),
                why: why.to_owned(),
            },
        };
        let parts: Vec<&str> = group.split('/').map(str::trim).collect();
        if parts.iter().any(|p| p.is_empty()) {
            problems.push(bad("an empty menu name; the action is in no menu"));
            continue;
        }
        let mut path = Vec::with_capacity(parts.len());
        let mut respelled = false;
        for part in parts {
            let key = format!("{}/{}", path.join("/"), part).to_lowercase();
            let first = spelled.entry(key).or_insert_with(|| part.to_owned());
            respelled |= first != part;
            path.push(first.clone());
        }
        if respelled {
            problems.push(bad(&format!(
                "spelled \"{}\" elsewhere; put there",
                path.join("/")
            )));
        }
        e.menu = path;
    }
}

/// A mnemonic must be a letter of the title and unique within its menu.
fn check_mnemonics(entries: &[Entry], problems: &mut Vec<Problem>) {
    let mut taken: HashMap<(&[String], char), &'static str> = HashMap::new();
    for e in entries {
        let Some(letter) = e.action.mnemonic else {
            continue;
        };
        let folded = letter.to_lowercase().next().unwrap_or(letter);
        let holder = if e
            .action
            .title
            .chars()
            .any(|c| c.to_lowercase().next() == Some(folded))
        {
            match taken.get(&(e.menu.as_slice(), folded)) {
                Some(&holder) => Some(Some(holder)),
                None => {
                    taken.insert((e.menu.as_slice(), folded), e.action.id);
                    None
                }
            }
        } else {
            Some(None)
        };
        if let Some(holder) = holder {
            problems.push(Problem {
                origin: Origin::of(&e.action),
                kind: ProblemKind::BadMnemonic {
                    id: e.action.id.to_owned(),
                    letter,
                    holder,
                },
            });
        }
    }
}
