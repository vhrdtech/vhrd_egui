//! Key presses to actions: who keeps a press, matching it, firing outside egui's locks.

use super::{Actions, Chord, Platform, Refused, Source};
use egui::{Context, Event, Key};

/// What has the keyboard, which decides the presses left to it (see the module docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    /// Nothing: every bound chord goes to its action.
    None,
    /// A widget that is no text edit (a button, a slider), or an open popup or menu: it keeps Enter, Space,
    /// Escape, Tab and the arrows.
    Widget,
    /// A text edit: it keeps typing, its editing chords, Copy, Cut and Paste, and on macOS Option chords.
    TextEdit,
}

impl Focus {
    /// What has the keyboard in `ctx` now.
    pub fn of(ctx: &Context) -> Self {
        if ctx.text_edit_focused() {
            Self::TextEdit
        } else if ctx.memory(|m| m.focused().is_some()) || egui::Popup::is_any_open(ctx) {
            Self::Widget
        } else {
            Self::None
        }
    }

    /// Whether this focus keeps the chord from the actions.
    pub fn keeps(self, chord: Chord, platform: Platform) -> bool {
        match self {
            Self::None => false,
            Self::Widget => chord.is_navigation(),
            Self::TextEdit => {
                chord.is_navigation()
                    || chord.is_typing()
                    || chord.is_text_editing()
                    || (platform == Platform::Mac && chord.alt && !chord.command)
            }
        }
    }
}

/// A press taken for an action, fired after the input is released.
struct Press {
    entry: usize,
    pasted: Option<String>,
}

impl Actions {
    /// Dispatch this frame's key presses: each press of a bound chord, and the platform's Copy, Cut and Paste
    /// when no text edit has focus, is taken out of the input (so widgets do not see it, nor the text it would
    /// type) and fires its action. What the focus keeps stays (see [`Focus`]). Call it at the start of the
    /// frame, before any widget reads keys. Handlers run after egui's input is released. Returns the presses
    /// refused because their action is disabled, for the app to say why.
    pub fn handle_input(&mut self, ctx: &Context) -> Vec<Refused> {
        let platform = Platform::of(ctx);
        let focus = Focus::of(ctx);
        let presses = ctx.input_mut(|i| self.take_presses(&mut i.events, platform, focus));
        self.fire_presses(presses)
    }

    /// [`Actions::handle_input`] on a list of events.
    pub fn handle_events(
        &mut self,
        events: &mut Vec<Event>,
        platform: Platform,
        focus: Focus,
    ) -> Vec<Refused> {
        let presses = self.take_presses(events, platform, focus);
        self.fire_presses(presses)
    }

    fn fire_presses(&mut self, presses: Vec<Press>) -> Vec<Refused> {
        presses
            .into_iter()
            .filter_map(|p| self.fire(p.entry, Source::Key, p.pasted).err())
            .collect()
    }

    fn take_presses(
        &self,
        events: &mut Vec<Event>,
        platform: Platform,
        focus: Focus,
    ) -> Vec<Press> {
        let mut presses = Vec::new();
        let mut drop_text = false;
        events.retain(|event| {
            let after_taken_press = std::mem::take(&mut drop_text);
            let (chord, pasted, pressed, repeat) = match event {
                Event::Text(_) => return !after_taken_press,
                Event::Copy => (Chord::command(Key::C), None, true, false),
                Event::Cut => (Chord::command(Key::X), None, true, false),
                Event::Paste(text) => (Chord::command(Key::V), Some(text.clone()), true, false),
                Event::Key {
                    key,
                    pressed,
                    repeat,
                    modifiers,
                    ..
                } => match Chord::of_press(*key, *modifiers, platform) {
                    Some(chord) => (chord, None, *pressed, *repeat),
                    None => return true,
                },
                _ => return true,
            };
            let clipboard = !matches!(event, Event::Key { .. });
            if (clipboard && focus == Focus::TextEdit) || focus.keeps(chord, platform) {
                return true;
            }
            let Some(entry) = self.lookup(chord) else {
                return true;
            };
            if pressed && (!repeat || self.entries[entry].action.repeats) {
                presses.push(Press { entry, pasted });
            }
            drop_text = pressed && !clipboard;
            false
        });
        presses
    }

    /// The action a press of `chord` reaches: the innermost active scope first, then global; punctuation also
    /// without the Shift its layout needed to type it.
    fn lookup(&self, chord: Chord) -> Option<usize> {
        let find = |chord: Chord| {
            self.scopes
                .iter()
                .map(String::as_str)
                .chain([""])
                .find_map(|scope| self.holder(scope, chord))
        };
        find(chord).or_else(|| {
            (chord.shift && chord.is_symbol())
                .then(|| {
                    find(Chord {
                        shift: false,
                        ..chord
                    })
                })
                .flatten()
        })
    }
}
