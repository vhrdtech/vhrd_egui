//! One registry of an app's actions that menus, the command palette, context menus, toolbar buttons and key
//! bindings all read, so nothing is declared twice (CHR-1).
//!
//! An [`Action`] is declared once, in the module that owns it, and registers itself through `inventory`:
//!
//! ```
//! use ve_basics::actions::{Action, inventory};
//!
//! inventory::submit! {
//!     Action::new("file.save", "Save").group("File").order(20).keys(&["Ctrl+S"]).mnemonic('S')
//!         .help("Write the document to its file")
//! }
//! ```
//!
//! The app builds [`Actions`] from the registry once at start and applies the user's keymap file
//! ([`Actions::apply_keymap`]). Then each frame it says which actions can run now and why the others cannot
//! ([`Actions::set_enabled`]), which toggles are on ([`Actions::set_checked`]) and which keyboard scopes are
//! active ([`Actions::set_scopes`]), dispatches key presses ([`Actions::handle_input`]) and carries out what
//! fired ([`Actions::take_fired`], or a handler set with [`Actions::on`]). Menus read [`Actions::menu_tree`],
//! the palette [`Actions::find`].
//!
//! Who gets a key press:
//! - A focused text edit keeps typing chords ([`Chord::is_typing`]), its editing chords
//!   ([`Chord::is_text_editing`]: Ctrl with A, Z, Y, C, X, V, arrows, Backspace, Delete, Home, End), Copy, Cut
//!   and Paste, and on macOS every Option chord, which types characters there.
//! - A focused widget (a button, a slider), an open popup or menu keeps Enter, Space, Escape, Tab and the
//!   arrows without Ctrl or Alt ([`Chord::is_navigation`]). egui itself drops a widget's focus on Escape
//!   before the frame starts, so Escape then reaches the actions; an open popup still keeps it.
//! - Everything else goes to the action bound in the innermost active scope, else to the global one.
//!
//! A chord is one key with modifiers: sequences (`Ctrl+K Ctrl+S`) are not supported. `Actions` holds handlers
//! (`Box<dyn FnMut>`) and is not `Send`: it lives on the UI thread with the app.

mod bind;
mod chord;
mod doc;
mod find;
mod input;
mod keymap;
mod registry;
#[cfg(test)]
mod tests;

/// Re-exported so an app declares actions without its own `inventory` dependency.
pub use inventory;

pub use chord::{CHORD_CHARS_MAX, Chord, ChordError, Platform};
pub use doc::KeymapBroken;
pub use find::{Found, MenuItem, MenuNode, QUERY_CHARS_MAX};
pub use input::Focus;
pub use keymap::{
    KEYMAP_BYTES_MAX, KEYMAP_CHORDS_PER_ACTION_MAX, KEYMAP_ENTRIES_MAX, KEYMAP_PROBLEMS_MAX,
    Origin, Problem, ProblemKind,
};
pub use registry::{Actions, DEFAULT_MENU_ORDER, FIRED_MAX};

use std::borrow::Cow;
use std::fmt;
use std::panic::Location;

/// An action as declared: id, words, place in the menus, default keys. Built with `const` methods so it can go
/// into `inventory::submit!`; [`Action::new`] records where it is declared, for problems.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Action {
    id: &'static str,
    title: &'static str,
    group: &'static str,
    icon: Option<&'static str>,
    keys: &'static [&'static str],
    order: i32,
    help: &'static str,
    scope: &'static str,
    mnemonic: Option<char>,
    repeats: bool,
    toggle: bool,
    destructive: bool,
    location: &'static Location<'static>,
}

inventory::collect!(Action);

impl Action {
    /// An action with its stable id (`file.save`: lower case, digits, `.`, `_`, `-`) and the title menus and the
    /// palette show (`Save`). No group, no keys, order 0, global scope.
    #[track_caller]
    pub const fn new(id: &'static str, title: &'static str) -> Self {
        Self {
            id,
            title,
            group: "",
            icon: None,
            keys: &[],
            order: 0,
            help: "",
            scope: "",
            mnemonic: None,
            repeats: false,
            toggle: false,
            destructive: false,
            location: Location::caller(),
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

    /// The keyboard scope its keys work in: a region or panel (`tracks`), active while [`Actions::set_scopes`]
    /// names it. Its chords may repeat those of other scopes; inside the scope they win over global ones.
    pub const fn scope(mut self, scope: &'static str) -> Self {
        self.scope = scope;
        self
    }

    /// The letter underlined in its menu entry, typed to pick it while the menu is open (Alt mnemonics). It
    /// must be a letter of the title and unique in its menu.
    pub const fn mnemonic(mut self, letter: char) -> Self {
        self.mnemonic = Some(letter);
        self
    }

    /// Fire again on key repeat while the chord is held (undo, zoom steps). Without it a held key fires once.
    pub const fn repeats(mut self) -> Self {
        self.repeats = true;
        self
    }

    /// A toggle: it has an on or off state ([`Actions::set_checked`]) that menus show as a check mark.
    pub const fn toggle(mut self) -> Self {
        self.toggle = true;
        self
    }

    /// Destroys something: menus put it last, in the danger colour, ending in "…", and it never takes Enter.
    pub const fn destructive(mut self) -> Self {
        self.destructive = true;
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

    /// The menu path as declared, empty when in no menu.
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

    /// The keyboard scope, empty for global.
    pub const fn key_scope(&self) -> &'static str {
        self.scope
    }

    /// The menu mnemonic letter, if any.
    pub const fn mnemonic_letter(&self) -> Option<char> {
        self.mnemonic
    }

    /// Whether a held key fires it again.
    pub const fn is_repeating(&self) -> bool {
        self.repeats
    }

    /// Whether it is a toggle.
    pub const fn is_toggle(&self) -> bool {
        self.toggle
    }

    /// Whether it destroys something.
    pub const fn is_destructive(&self) -> bool {
        self.destructive
    }

    /// Where it is declared.
    pub const fn location(&self) -> &'static Location<'static> {
        self.location
    }
}

/// Where a fired action came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// A key chord, or Copy, Cut or Paste from the platform.
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

/// An action that fired: the handler gets it, or the app takes it with [`Actions::take_fired`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fired {
    /// The action's id.
    pub id: &'static str,
    /// What fired it.
    pub source: Source,
    /// The clipboard text when the platform's Paste fired it.
    pub pasted: Option<String>,
}

impl Fired {
    /// An action fired without pasted text.
    pub fn new(id: &'static str, source: Source) -> Self {
        Self {
            id,
            source,
            pasted: None,
        }
    }
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

/// One action as it stands now: its declaration, its chords and whether it can run.
#[derive(Clone, Copy, Debug)]
pub struct ActionState<'a> {
    /// The declaration.
    pub action: &'a Action,
    /// The chords bound now: the defaults or the keymap's, minus refused ones. The first is the one to show.
    pub keys: &'a [Chord],
    /// The declared chords that parsed, for a settings window's "reset" and "default" column.
    pub default_keys: &'a [Chord],
    /// `Err(reason)` when it cannot run now.
    pub enabled: Result<(), &'a str>,
    /// On or off for a toggle, `None` for other actions.
    pub checked: Option<bool>,
}
