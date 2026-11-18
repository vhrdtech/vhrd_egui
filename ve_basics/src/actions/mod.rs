//! One registry of an app's actions that menus, the command palette, context menus, toolbar buttons and key
//! bindings all read, so nothing is declared twice (CHR-1).
//!
//! An [`Action`] is declared once, in the module that owns it, and registers itself through `inventory`:
//!
//! ```
//! use ve_basics::actions::{Action, inventory};
//!
//! inventory::submit! {
//!     Action::new("file.save", "Save").group("File").order(20).keys(&["Ctrl+S"])
//!         .help("Write the document to its file")
//! }
//! ```
//!
//! The app builds [`Actions`] from the registry once at start, applies the user's keymap file
//! ([`Actions::apply_keymap`]), and then each frame: says which actions can run now and why the others cannot
//! ([`Actions::set_enabled`]), dispatches key presses ([`Actions::handle_input`]) and runs what was fired
//! ([`Actions::take_fired`], or a handler set with [`Actions::on`]). Menus read [`Actions::menu_tree`], the
//! palette [`Actions::find`].

mod chord;
mod find;
mod keymap;
#[cfg(test)]
mod tests;

/// Re-exported so an app declares actions without its own `inventory` dependency.
pub use inventory;

pub use chord::{Chord, ChordError, Platform};
pub use find::{Found, MenuItem, MenuNode};
pub use keymap::{KEYMAP_BYTES_MAX, KEYMAP_ENTRIES_MAX, Origin, Problem, ProblemKind};

use egui::{Context, Event};
use std::borrow::Cow;
use std::collections::{HashMap, VecDeque};
use std::fmt;

/// An action as declared: id, words, place in the menus, default keys. Built with `const` methods so it can go
/// into `inventory::submit!`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Action {
    id: &'static str,
    title: &'static str,
    group: &'static str,
    icon: Option<&'static str>,
    keys: &'static [&'static str],
    order: i32,
    help: &'static str,
    repeats: bool,
}

inventory::collect!(Action);

impl Action {
    /// An action with its stable id (`file.save`: lower case, digits, `.`, `_`, `-`) and the title menus and the
    /// palette show (`Save`). No group, no keys, order 0.
    pub const fn new(id: &'static str, title: &'static str) -> Self {
        Self {
            id,
            title,
            group: "",
            icon: None,
            keys: &[],
            order: 0,
            help: "",
            repeats: false,
        }
    }

    /// The menu it sits in, a path for submenus: `File`, `View/Panels`. Without a group the action is in the
    /// palette and on keys, not in the menus.
    pub const fn group(mut self, group: &'static str) -> Self {
        self.group = group;
        self
    }

    /// The icon slot: an icon's name, drawn by the app's icon set.
    pub const fn icon(mut self, icon: &'static str) -> Self {
        self.icon = Some(icon);
        self
    }

    /// The default key chords, as [`Chord::parse`] reads them: `&["Ctrl+S"]`, `&["Ctrl+Shift+Z", "Ctrl+Y"]`.
    pub const fn keys(mut self, keys: &'static [&'static str]) -> Self {
        self.keys = keys;
        self
    }

    /// Place within its menu, lower first; ties go by title.
    pub const fn order(mut self, order: i32) -> Self {
        self.order = order;
        self
    }

    /// One line saying what it does, for tooltips and the palette.
    pub const fn help(mut self, help: &'static str) -> Self {
        self.help = help;
        self
    }

    /// Fire again on key repeat while the chord is held (undo, zoom steps). Off by default: a held key fires once.
    pub const fn repeats(mut self, repeats: bool) -> Self {
        self.repeats = repeats;
        self
    }

    /// The stable id.
    pub const fn id(&self) -> &'static str {
        self.id
    }

    /// The title.
    pub const fn title(&self) -> &'static str {
        self.title
    }

    /// The menu path, empty when in no menu.
    pub const fn menu_group(&self) -> &'static str {
        self.group
    }

    /// The icon's name, if any.
    pub const fn icon_name(&self) -> Option<&'static str> {
        self.icon
    }

    /// The default chords as declared.
    pub const fn default_key_texts(&self) -> &'static [&'static str] {
        self.keys
    }

    /// Place within its menu.
    pub const fn menu_order(&self) -> i32 {
        self.order
    }

    /// The help line, empty when none.
    pub const fn help_text(&self) -> &'static str {
        self.help
    }
}

/// Where a fired action came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// A key chord.
    Key,
    /// The menu row or a menu button.
    Menu,
    /// The command palette.
    Palette,
    /// A context menu.
    Context,
    /// A toolbar button.
    Toolbar,
    /// The app's own code.
    Code,
}

/// An action that ran without a handler: the app carries it out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fired {
    /// The action's id.
    pub id: &'static str,
    /// What fired it.
    pub source: Source,
}

/// Why an action did not run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refused {
    /// No action has this id.
    Unknown(String),
    /// The action cannot run now; the reason says why and what enables it.
    Disabled {
        /// The action's id.
        id: &'static str,
        /// The reason given to [`Actions::set_enabled`].
        reason: Cow<'static, str>,
        /// What tried to fire it.
        source: Source,
    },
}

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown(id) => write!(f, "no action \"{id}\""),
            Self::Disabled { id, reason, .. } => write!(f, "{id}: {reason}"),
        }
    }
}

impl std::error::Error for Refused {}

/// One action as it stands now: its declaration, its bound chords and whether it can run.
#[derive(Clone, Copy, Debug)]
pub struct ActionState<'a> {
    /// The declaration.
    pub action: &'a Action,
    /// The chords bound now: the defaults or the keymap's, minus refused ones. The first is the one to show.
    pub keys: &'a [Chord],
    /// `Err(reason)` when it cannot run now.
    pub enabled: Result<(), &'a str>,
}

/// Most fired actions kept for [`Actions::take_fired`]; older ones are dropped and counted.
pub const FIRED_MAX: usize = 64;

type Handler = Box<dyn FnMut()>;

struct Entry {
    action: Action,
    defaults: Vec<Chord>,
    keys: Vec<Chord>,
    enabled: Result<(), Cow<'static, str>>,
    handler: Option<Handler>,
}

/// The app's actions: declarations, bound keys, enabled state, handlers, and what fired.
pub struct Actions {
    /// Sorted by id.
    entries: Vec<Entry>,
    by_id: HashMap<&'static str, usize>,
    by_chord: HashMap<Chord, usize>,
    /// Problems of the declarations themselves: bad ids, repeated ids, bad default chords.
    declared_problems: Vec<Problem>,
    /// Chord overrides in the order they apply: the keymap file's, then runtime rebinds.
    overrides: Vec<keymap::Override>,
    /// Problems of the keymap file's text: syntax, unknown ids, bad values and chords.
    keymap_problems: Vec<Problem>,
    /// Problems of binding: chords claimed twice.
    bind_problems: Vec<Problem>,
    fired: VecDeque<Fired>,
    fired_dropped: usize,
    menu_order: Vec<String>,
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

/// Top-level menus in this order unless [`Actions::menu_order`] says otherwise; others follow by name.
pub const DEFAULT_MENU_ORDER: &[&str] = &["File", "Edit", "View", "Go", "Tools", "Window", "Help"];

impl Actions {
    /// Every action registered with `inventory::submit!` in the crates linked into the app.
    pub fn from_registry() -> Self {
        Self::from_list(inventory::iter::<Action>.into_iter().copied())
    }

    /// The given actions only (tests, or an app composing its own set). Problems in them (a bad or repeated id,
    /// a default chord that does not parse or is taken) are kept in [`Actions::problems`], never a panic.
    pub fn from_list(actions: impl IntoIterator<Item = Action>) -> Self {
        let mut list: Vec<Action> = actions.into_iter().collect();
        list.sort_by_key(|a| a.id);
        let mut declared_problems = Vec::new();
        let mut entries: Vec<Entry> = Vec::with_capacity(list.len());
        for action in list {
            if !valid_id(action.id) {
                declared_problems.push(Problem::declared(ProblemKind::BadId {
                    id: action.id.to_owned(),
                }));
                continue;
            }
            if entries.last().is_some_and(|e| e.action.id == action.id) {
                declared_problems.push(Problem::declared(ProblemKind::RepeatedId {
                    id: action.id.to_owned(),
                }));
                continue;
            }
            let mut defaults = Vec::with_capacity(action.keys.len());
            for text in action.keys {
                match Chord::parse(text) {
                    Ok(chord) => defaults.push(chord),
                    Err(error) => {
                        declared_problems.push(Problem::declared(ProblemKind::BadChord {
                            id: action.id.to_owned(),
                            text: (*text).to_owned(),
                            error,
                        }))
                    }
                }
            }
            entries.push(Entry {
                action,
                defaults,
                keys: Vec::new(),
                enabled: Ok(()),
                handler: None,
            });
        }
        let by_id = entries
            .iter()
            .enumerate()
            .map(|(i, e)| (e.action.id, i))
            .collect();
        let mut actions = Self {
            entries,
            by_id,
            by_chord: HashMap::new(),
            declared_problems,
            overrides: Vec::new(),
            keymap_problems: Vec::new(),
            bind_problems: Vec::new(),
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

    /// The action a chord is bound to.
    pub fn by_chord(&self, chord: Chord) -> Option<&'static str> {
        self.by_chord
            .get(&chord)
            .map(|&i| self.entries[i].action.id)
    }

    /// Everything wrong with the declarations, the keymap file and runtime rebinds, in that order. Empty in a
    /// healthy app; a test can assert so.
    pub fn problems(&self) -> impl Iterator<Item = &Problem> {
        self.declared_problems
            .iter()
            .chain(&self.keymap_problems)
            .chain(&self.bind_problems)
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

    /// Run `handler` whenever the action fires, instead of queueing it for [`Actions::take_fired`]. The handler
    /// captures what it needs (an `egui::Context` clone, a channel).
    ///
    /// An unknown id is a mistake in the app's code: debug builds assert, release builds ignore it.
    pub fn on(&mut self, id: &str, handler: impl FnMut() + 'static) {
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
        self.fire(i, source)
    }

    /// The actions fired since the last call that have no handler, oldest first.
    pub fn take_fired(&mut self) -> Vec<Fired> {
        self.fired.drain(..).collect()
    }

    /// How many fired actions were dropped because nobody took them while more than [`FIRED_MAX`] queued.
    pub fn fired_dropped(&self) -> usize {
        self.fired_dropped
    }

    /// Dispatch this frame's key presses: each press of a bound chord is taken out of the input (so widgets do
    /// not see it too) and fires its action. Call it at the start of the frame, before any widget reads keys.
    /// While a text field has focus, typing chords ([`Chord::is_typing`]) are left to it. Returns the presses
    /// refused because their action is disabled, for the app to say why.
    pub fn handle_input(&mut self, ctx: &Context) -> Vec<Refused> {
        let platform = Platform::of(ctx);
        let text_focus = ctx.egui_wants_keyboard_input();
        ctx.input_mut(|i| self.handle_events(&mut i.events, platform, text_focus))
    }

    /// [`Actions::handle_input`] on a list of events: the presses of bound chords are removed and fired.
    pub fn handle_events(
        &mut self,
        events: &mut Vec<Event>,
        platform: Platform,
        text_focus: bool,
    ) -> Vec<Refused> {
        let mut refused = Vec::new();
        events.retain(|event| {
            let Event::Key {
                key,
                pressed,
                repeat,
                modifiers,
                ..
            } = *event
            else {
                return true;
            };
            let Some(chord) = Chord::of_press(key, modifiers, platform) else {
                return true;
            };
            if text_focus && chord.is_typing() {
                return true;
            }
            let Some(&i) = self.by_chord.get(&chord) else {
                return true;
            };
            if pressed
                && (!repeat || self.entries[i].action.repeats)
                && let Err(r) = self.fire(i, Source::Key)
            {
                refused.push(r);
            }
            false
        });
        refused
    }

    fn index(&self, id: &str) -> Option<usize> {
        self.by_id.get(id).copied()
    }

    fn fire(&mut self, i: usize, source: Source) -> Result<(), Refused> {
        let entry = &mut self.entries[i];
        if let Err(reason) = &entry.enabled {
            return Err(Refused::Disabled {
                id: entry.action.id,
                reason: reason.clone(),
                source,
            });
        }
        if let Some(handler) = &mut entry.handler {
            handler();
            return Ok(());
        }
        if self.fired.len() == FIRED_MAX {
            self.fired.pop_front();
            self.fired_dropped += 1;
        }
        self.fired.push_back(Fired {
            id: entry.action.id,
            source,
        });
        Ok(())
    }
}

impl Entry {
    fn state(&self) -> ActionState<'_> {
        ActionState {
            action: &self.action,
            keys: &self.keys,
            enabled: self.enabled.as_ref().map(|_| ()).map_err(|r| r.as_ref()),
        }
    }
}

/// Longest id accepted.
const ID_CHARS_MAX: usize = 96;

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= ID_CHARS_MAX
        && id.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-')
        })
}
