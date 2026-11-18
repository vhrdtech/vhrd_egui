//! Key chords: parsed from text, matched against key events, labelled for the platform.
//!
//! The one place that knows Ctrl against Cmd: a chord says `Ctrl` for the platform's command key, which is
//! Ctrl on Linux, Windows and the web, and ⌘ on macOS. The keymap file and the defaults stay portable.

use egui::{Context, Key, KeyboardShortcut, Modifiers};
use std::fmt;

/// Which command key the platform uses; decides how a key event matches a [`Chord`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    /// macOS (and iOS): the command key is ⌘, the physical Ctrl key is a modifier no chord uses.
    Mac,
    /// Linux, Windows, the web and the rest: the command key is Ctrl.
    Other,
}

impl Platform {
    /// The platform egui runs on.
    pub fn of(ctx: &Context) -> Self {
        if ctx.os().is_mac() {
            Self::Mac
        } else {
            Self::Other
        }
    }
}

/// One key with its modifiers, as an action is bound to it: `Ctrl+Shift+S`, `F6`, `Alt+Enter`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Chord {
    /// The platform's command key: Ctrl, or ⌘ on macOS.
    pub command: bool,
    /// Alt, or ⌥ on macOS.
    pub alt: bool,
    /// Shift.
    pub shift: bool,
    /// The logical key.
    pub key: Key,
}

/// Why a chord's text did not parse.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChordError {
    /// Nothing but blanks.
    Empty,
    /// A part before the key that is no modifier.
    UnknownModifier(String),
    /// The last part names no key egui knows.
    UnknownKey(String),
    /// A modifier written twice.
    RepeatedModifier(String),
}

impl fmt::Display for ChordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "empty chord"),
            Self::UnknownModifier(m) => {
                write!(
                    f,
                    "unknown modifier \"{m}\" (Ctrl, Cmd, Alt, Option, Shift)"
                )
            }
            Self::UnknownKey(k) => write!(f, "unknown key \"{k}\""),
            Self::RepeatedModifier(m) => write!(f, "modifier \"{m}\" written twice"),
        }
    }
}

impl std::error::Error for ChordError {}

/// Longest chord text read; anything longer is not a chord.
const CHORD_CHARS_MAX: usize = 64;

impl Chord {
    /// A key without modifiers.
    pub const fn key(key: Key) -> Self {
        Self {
            command: false,
            alt: false,
            shift: false,
            key,
        }
    }

    /// Parse `Ctrl+Shift+S`: modifiers then the key, joined by `+`, any case for modifiers. `Ctrl`, `Cmd`,
    /// `Command` and `Mod` all mean the platform's command key; `Alt`, `Option` and `Opt` mean Alt; the key is
    /// any name egui knows (`S`, `F6`, `Comma`, `,`, `Enter`, `Up`), a single letter in either case.
    pub fn parse(text: &str) -> Result<Self, ChordError> {
        let text = text.trim();
        if text.is_empty() {
            return Err(ChordError::Empty);
        }
        if text.chars().count() > CHORD_CHARS_MAX {
            return Err(ChordError::UnknownKey(
                text.chars().take(CHORD_CHARS_MAX).collect(),
            ));
        }
        // `Ctrl++` binds the plus key: the last `+` is then the key, not a separator.
        let (mods, key) = match text.strip_suffix("++") {
            Some(rest) => (rest, "+"),
            None => match text.rsplit_once('+') {
                Some((mods, key)) => (mods, key),
                None => ("", text),
            },
        };
        let mut chord = Self::key(parse_key(key.trim())?);
        for part in mods.split('+').map(str::trim).filter(|p| !p.is_empty()) {
            let flag = match part.to_ascii_lowercase().as_str() {
                "ctrl" | "control" | "cmd" | "command" | "mod" => &mut chord.command,
                "alt" | "option" | "opt" => &mut chord.alt,
                "shift" => &mut chord.shift,
                _ => return Err(ChordError::UnknownModifier(part.to_owned())),
            };
            if *flag {
                return Err(ChordError::RepeatedModifier(part.to_owned()));
            }
            *flag = true;
        }
        Ok(chord)
    }

    /// The portable text the keymap file uses: `Ctrl+Shift+S`. [`Chord::parse`] reads it back.
    pub fn text(&self) -> String {
        let mut s = String::new();
        for (on, name) in [
            (self.command, "Ctrl"),
            (self.alt, "Alt"),
            (self.shift, "Shift"),
        ] {
            if on {
                s.push_str(name);
                s.push('+');
            }
        }
        s.push_str(self.key.name());
        s
    }

    /// The label a menu, a tooltip or the palette shows: `Ctrl+Shift+S`, or `⇧⌘S` on macOS where the font has
    /// the symbols (egui's own formatting).
    pub fn label(&self, ctx: &Context) -> String {
        ctx.format_shortcut(&self.shortcut())
    }

    /// The same chord as an egui shortcut, for egui's own buttons (`Button::shortcut_text`).
    pub fn shortcut(&self) -> KeyboardShortcut {
        let mut modifiers = if self.command {
            Modifiers::COMMAND
        } else {
            Modifiers::NONE
        };
        modifiers.alt = self.alt;
        modifiers.shift = self.shift;
        KeyboardShortcut::new(modifiers, self.key)
    }

    /// The chord a key press makes on `platform`, or `None` when it holds a modifier no chord uses (the
    /// physical Ctrl key on macOS).
    pub fn of_press(key: Key, modifiers: Modifiers, platform: Platform) -> Option<Self> {
        let command = match platform {
            Platform::Mac if modifiers.ctrl => return None,
            Platform::Mac => modifiers.mac_cmd || modifiers.command,
            Platform::Other => modifiers.ctrl || modifiers.command,
        };
        Some(Self {
            command,
            alt: modifiers.alt,
            shift: modifiers.shift,
            key,
        })
    }

    /// Whether the chord is typing, not a command: no command key and no Alt, and not a function key. Such
    /// chords (`/`, `Delete`, `Shift+A`) are left to a focused text field.
    pub fn is_typing(&self) -> bool {
        !self.command && !self.alt && !is_function_key(self.key)
    }
}

impl fmt::Display for Chord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text())
    }
}

fn parse_key(name: &str) -> Result<Key, ChordError> {
    if name.is_empty() {
        return Err(ChordError::UnknownKey(String::new()));
    }
    Key::from_name(name)
        .or_else(|| Key::from_name(&name.to_ascii_uppercase()))
        .ok_or_else(|| ChordError::UnknownKey(name.to_owned()))
}

fn is_function_key(key: Key) -> bool {
    key.name()
        .strip_prefix('F')
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}
