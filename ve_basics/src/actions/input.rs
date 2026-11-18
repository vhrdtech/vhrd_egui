//! Key presses to actions: who keeps a press, matching it, firing outside egui's locks.

use super::{Actions, Chord, Platform, Refused, Source};
use egui::{Context, Event, Id, Key, RawInput};

/// Largest pasted text an action receives; a larger paste is refused with [`Refused::PasteTooLarge`].
pub const PASTED_BYTES_MAX: usize = 1024 * 1024;

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
    /// What has the keyboard in `ctx` now. Inside a frame this is after egui handled the frame's input, which
    /// already dropped a focus on Escape; [`Focus::before_input`] is what had it when the keys were pressed.
    pub fn of(ctx: &Context) -> Self {
        if ctx.text_edit_focused() {
            Self::TextEdit
        } else if ctx.memory(|m| m.focused().is_some()) || egui::Popup::is_any_open(ctx) {
            Self::Widget
        } else {
            Self::None
        }
    }

    /// What had the keyboard when this frame's keys were pressed: recorded just before egui handled the
    /// input, once [`Actions::attach`] has installed the recorder. `None` before that.
    pub fn before_input(ctx: &Context) -> Option<Self> {
        ctx.data(|d| d.get_temp::<Self>(focus_id()))
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

fn focus_id() -> Id {
    Id::new("ve_basics::actions::focus_before_input")
}

/// Records [`Focus::of`] before egui handles each frame's input, where Escape and Tab still find the focus
/// they act on.
struct FocusRecorder;

impl egui::Plugin for FocusRecorder {
    fn debug_name(&self) -> &'static str {
        "ve_basics::actions focus"
    }

    fn input_hook(&mut self, ctx: &Context, _input: &mut RawInput) {
        let focus = Focus::of(ctx);
        ctx.data_mut(|d| d.insert_temp(focus_id(), focus));
    }
}

/// A press taken for an action, fired after the input is released.
struct Press {
    entry: usize,
    pasted: Option<String>,
}

impl Actions {
    /// Install the focus recorder on `ctx` (once; again is harmless), so key presses go by the focus that held
    /// when they were pressed. Call it at start; [`Actions::handle_input`] calls it too, so only the first
    /// frame goes by the focus after egui's own handling.
    pub fn attach(&self, ctx: &Context) {
        ctx.add_plugin(FocusRecorder);
    }

    /// Dispatch this frame's key presses: each press of a bound chord, and the platform's Copy, Cut and Paste
    /// when no text edit has focus, is taken out of the input (so widgets do not see it, nor the text it would
    /// type) and fires its action. What the focus keeps stays (see [`Focus`]). Call it at the start of the
    /// frame, before any widget reads keys. Handlers run after egui's input is released. Returns the presses
    /// refused: their action disabled, or a paste over [`PASTED_BYTES_MAX`].
    pub fn handle_input(&mut self, ctx: &Context) -> Vec<Refused> {
        let platform = Platform::of(ctx);
        let focus = Focus::before_input(ctx).unwrap_or_else(|| {
            self.attach(ctx);
            Focus::of(ctx)
        });
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

    fn fire_presses(&mut self, presses: Vec<Result<Press, Refused>>) -> Vec<Refused> {
        presses
            .into_iter()
            .filter_map(|p| match p {
                Ok(p) => self.fire(p.entry, Source::Key, p.pasted).err(),
                Err(refused) => Some(refused),
            })
            .collect()
    }

    fn take_presses(
        &self,
        events: &mut Vec<Event>,
        platform: Platform,
        focus: Focus,
    ) -> Vec<Result<Press, Refused>> {
        let mut presses = Vec::new();
        let mut kept = Vec::with_capacity(events.len());
        let mut drop_text = false;
        for event in events.drain(..) {
            let after_taken_press = std::mem::take(&mut drop_text);
            if matches!(event, Event::Text(_)) && after_taken_press {
                continue;
            }
            let Some((chord, pressed, repeat)) = press_of(&event, platform) else {
                kept.push(event);
                continue;
            };
            let clipboard = !matches!(event, Event::Key { .. });
            let entry = if (clipboard && focus == Focus::TextEdit) || focus.keeps(chord, platform) {
                None
            } else {
                self.lookup(chord)
            };
            let Some(entry) = entry else {
                kept.push(event);
                continue;
            };
            drop_text = pressed && !clipboard;
            if !pressed || (repeat && !self.entries[entry].action.repeats) {
                continue;
            }
            let pasted = match event {
                Event::Paste(text) if text.len() > PASTED_BYTES_MAX => {
                    presses.push(Err(Refused::PasteTooLarge {
                        id: self.entries[entry].action.id,
                        size_bytes: text.len(),
                    }));
                    continue;
                }
                Event::Paste(text) => Some(text),
                _ => None,
            };
            presses.push(Ok(Press { entry, pasted }));
        }
        *events = kept;
        presses
    }

    /// The action a press of `chord` reaches: the innermost active scope first, then global; punctuation also
    /// without the Shift its layout needed to type it.
    fn lookup(&self, chord: Chord) -> Option<usize> {
        let find = |chord: Chord| {
            self.scopes
                .iter()
                .copied()
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

/// The chord an event presses, with whether it is pressed and a repeat: a key, or the platform's Copy, Cut and
/// Paste as Ctrl+C, X and V. `None` for other events and for keys no chord uses.
fn press_of(event: &Event, platform: Platform) -> Option<(Chord, bool, bool)> {
    match event {
        Event::Copy => Some((Chord::command(Key::C), true, false)),
        Event::Cut => Some((Chord::command(Key::X), true, false)),
        Event::Paste(_) => Some((Chord::command(Key::V), true, false)),
        Event::Key {
            key,
            pressed,
            repeat,
            modifiers,
            ..
        } => Chord::of_press(*key, *modifiers, platform).map(|chord| (chord, *pressed, *repeat)),
        _ => None,
    }
}
