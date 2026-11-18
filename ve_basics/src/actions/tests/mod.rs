//! Tests of the action registry; each module covers one part.

mod chords;
mod declare;
mod dispatch;
mod focus;
mod keymap;
mod lookup;
mod save;

use super::*;
use egui::{Event, Key, Modifiers};

// The registry of this test binary: what `from_registry` must collect, no more, no less.
inventory::submit! {
    Action::new("test.registered.save", "Save").group("File").keys(&["Ctrl+S"])
}
inventory::submit! {
    Action::new("test.registered.palette", "Command palette").keys(&["Ctrl+K"])
}

const SAVE: Action = Action::new("file.save", "Save")
    .group("File")
    .order(20)
    .keys(&["Ctrl+S"])
    .mnemonic('S')
    .help("Write the document to its file");
const SAVE_AS: Action = Action::new("file.save_as", "Save as…")
    .group("File")
    .order(21)
    .keys(&["Ctrl+Shift+S"])
    .mnemonic('a');
const OPEN: Action = Action::new("file.open", "Open…")
    .group("File")
    .order(10)
    .keys(&["Ctrl+O"]);
const RECENT: Action = Action::new("file.recent.clear", "Clear recent").group("File/Recent");
const DELETE: Action = Action::new("file.delete", "Delete…")
    .group("File")
    .destructive();
const UNDO: Action = Action::new("edit.undo", "Undo")
    .group("Edit")
    .keys(&["Ctrl+Z"])
    .repeats();
const REDO: Action = Action::new("edit.redo", "Redo")
    .group("Edit")
    .order(1)
    .keys(&["Ctrl+Shift+Z", "Ctrl+Y"]);
const FIND: Action = Action::new("edit.find", "Find")
    .group("Edit")
    .keys(&["Ctrl+F", "/"]);
const COPY: Action = Action::new("edit.copy", "Copy")
    .group("Edit")
    .keys(&["Ctrl+C"]);
const PASTE: Action = Action::new("edit.paste", "Paste")
    .group("Edit")
    .keys(&["Ctrl+V"]);
const NEXT_REGION: Action = Action::new("focus.next_region", "Next region").keys(&["F6"]);
const THEME: Action = Action::new("view.theme", "Switch theme")
    .group("View")
    .icon("sun");
const GRID: Action = Action::new("view.grid", "Show grid")
    .group("View")
    .toggle()
    .keys(&["G"]);
const PLAY: Action = Action::new("view.play", "Play")
    .group("View")
    .keys(&["Space", "P"]);
const ZOOM_IN: Action = Action::new("view.zoom_in", "Zoom in")
    .group("View")
    .keys(&["Ctrl+Plus", "Ctrl+/"]);
const ALT_S: Action = Action::new("view.split", "Split")
    .group("View")
    .keys(&["Alt+S"]);
const TRACK_DELETE: Action = Action::new("tracks.remove", "Remove track")
    .scope("tracks")
    .keys(&["Delete", "Ctrl+F"]);

const ALL: [Action; 18] = [
    SAVE,
    SAVE_AS,
    OPEN,
    RECENT,
    DELETE,
    UNDO,
    REDO,
    FIND,
    COPY,
    PASTE,
    NEXT_REGION,
    THEME,
    GRID,
    PLAY,
    ZOOM_IN,
    ALT_S,
    TRACK_DELETE,
    Action::new("help.about", "About").group("Help"),
];

fn app() -> Actions {
    let actions = Actions::from_list(ALL);
    let problems: Vec<String> = actions.problems().map(ToString::to_string).collect();
    assert_eq!(problems, Vec::<String>::new());
    actions
}

fn chord(text: &str) -> Chord {
    Chord::parse(text).unwrap()
}

fn key_event(key: Key, modifiers: Modifiers, repeat: bool) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat,
        modifiers,
    }
}

fn press(key: Key, modifiers: Modifiers) -> Event {
    key_event(key, modifiers, false)
}

fn ctrl() -> Modifiers {
    Modifiers {
        ctrl: true,
        command: true,
        ..Modifiers::NONE
    }
}

fn alt() -> Modifiers {
    Modifiers {
        alt: true,
        ..Modifiers::NONE
    }
}

fn shift(m: Modifiers) -> Modifiers {
    Modifiers { shift: true, ..m }
}

fn fired_ids(actions: &mut Actions) -> Vec<&'static str> {
    actions.take_fired().into_iter().map(|f| f.id).collect()
}

/// Feed `events` to `actions` on Linux with `focus`; the events left for the widgets.
fn dispatch(actions: &mut Actions, events: Vec<Event>, focus: Focus) -> Vec<Event> {
    let mut events = events;
    let refused = actions.handle_events(&mut events, Platform::Other, focus);
    assert_eq!(refused, []);
    events
}

#[test]
fn the_registered_set_is_the_expected_list() {
    let actions = Actions::from_registry();
    assert_eq!(
        actions.ids(),
        ["test.registered.palette", "test.registered.save"]
    );
    assert_eq!(actions.problems().count(), 0);
}
