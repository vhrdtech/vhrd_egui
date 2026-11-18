use super::*;
use egui::{Key, Modifiers};
use std::cell::Cell;
use std::rc::Rc;

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
    .help("Write the document to its file");
const SAVE_AS: Action = Action::new("file.save_as", "Save as…")
    .group("File")
    .order(21)
    .keys(&["Ctrl+Shift+S"]);
const OPEN: Action = Action::new("file.open", "Open…")
    .group("File")
    .order(10)
    .keys(&["Ctrl+O"]);
const RECENT: Action = Action::new("file.recent.clear", "Clear recent").group("File/Recent");
const UNDO: Action = Action::new("edit.undo", "Undo")
    .group("Edit")
    .keys(&["Ctrl+Z"])
    .repeats(true);
const REDO: Action = Action::new("edit.redo", "Redo")
    .group("Edit")
    .order(1)
    .keys(&["Ctrl+Shift+Z", "Ctrl+Y"]);
const FIND: Action = Action::new("edit.find", "Find")
    .group("Edit")
    .keys(&["Ctrl+F", "/"]);
const NEXT_REGION: Action = Action::new("focus.next_region", "Next region").keys(&["F6"]);
const THEME: Action = Action::new("view.theme", "Switch theme")
    .group("View")
    .icon("sun");

fn app() -> Actions {
    let actions = Actions::from_list([
        SAVE,
        SAVE_AS,
        OPEN,
        RECENT,
        UNDO,
        REDO,
        FIND,
        NEXT_REGION,
        THEME,
    ]);
    assert_eq!(
        actions.problems().count(),
        0,
        "{:?}",
        actions.problems().collect::<Vec<_>>()
    );
    actions
}

fn chord(text: &str) -> Chord {
    Chord::parse(text).unwrap()
}

fn press(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}

fn ctrl() -> Modifiers {
    Modifiers {
        ctrl: true,
        command: true,
        ..Modifiers::NONE
    }
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

#[test]
fn chords_parse_print_and_refuse_with_reasons() {
    let c = chord("ctrl+shift+s");
    assert_eq!(
        c,
        Chord {
            command: true,
            alt: false,
            shift: true,
            key: Key::S
        }
    );
    assert_eq!(c.text(), "Ctrl+Shift+S");
    assert_eq!(chord(&c.text()), c);
    assert_eq!(
        chord("Cmd+K"),
        chord("Ctrl+K"),
        "Cmd and Ctrl are the one command key"
    );
    assert_eq!(chord("Option+Enter"), chord("Alt+Return"));
    assert_eq!(chord("Ctrl++").key, Key::Plus);
    assert_eq!(chord("Ctrl+,"), chord("Ctrl+Comma"));
    assert_eq!(chord("F6"), Chord::key(Key::F6));
    assert_eq!(Chord::parse(" "), Err(ChordError::Empty));
    assert_eq!(
        Chord::parse("Hyper+S"),
        Err(ChordError::UnknownModifier("Hyper".into()))
    );
    assert_eq!(
        Chord::parse("Ctrl+Banana"),
        Err(ChordError::UnknownKey("Banana".into()))
    );
    assert_eq!(
        Chord::parse("Ctrl+Cmd+S"),
        Err(ChordError::RepeatedModifier("Cmd".into()))
    );
    assert!(chord("/").is_typing());
    assert!(!chord("F5").is_typing());
    assert!(!chord("Ctrl+A").is_typing());
}

#[test]
fn a_press_makes_the_platform_chord() {
    let mac_cmd = Modifiers {
        mac_cmd: true,
        command: true,
        ..Modifiers::NONE
    };
    assert_eq!(
        Chord::of_press(Key::S, mac_cmd, Platform::Mac),
        Some(chord("Ctrl+S"))
    );
    assert_eq!(
        Chord::of_press(Key::S, ctrl(), Platform::Other),
        Some(chord("Ctrl+S"))
    );
    // The physical Ctrl key on a Mac is no chord's.
    let mac_ctrl = Modifiers {
        ctrl: true,
        ..Modifiers::NONE
    };
    assert_eq!(Chord::of_press(Key::S, mac_ctrl, Platform::Mac), None);
}

#[test]
fn actions_register_and_bind_their_default_keys() {
    let actions = app();
    assert_eq!(actions.by_chord(chord("Ctrl+S")), Some("file.save"));
    assert_eq!(actions.by_chord(chord("Ctrl+Y")), Some("edit.redo"));
    assert_eq!(actions.by_chord(chord("Ctrl+Shift+Z")), Some("edit.redo"));
    let save = actions.get("file.save").unwrap();
    assert_eq!(save.action.title(), "Save");
    assert_eq!(save.action.help_text(), "Write the document to its file");
    assert_eq!(save.keys, [chord("Ctrl+S")]);
    assert_eq!(save.enabled, Ok(()));
    assert_eq!(
        actions.get("view.theme").unwrap().action.icon_name(),
        Some("sun")
    );
    assert!(actions.get("file.nope").is_none());
}

#[test]
fn declaration_problems_are_reported_not_panicked() {
    let actions = Actions::from_list([
        SAVE,
        Action::new("file.save", "Save again"),
        Action::new("Bad Id", "Bad"),
        Action::new("edit.paste", "Paste").keys(&["Ctrl+Banana", "Ctrl+V"]),
        Action::new("edit.store", "Store").keys(&["Ctrl+S"]),
    ]);
    let problems: Vec<String> = actions.problems().map(ToString::to_string).collect();
    assert_eq!(
        problems,
        [
            "declared actions: \"Bad Id\" is no action id (lower case, digits, '.', '_', '-'; at most 96)",
            "declared actions: \"Ctrl+Banana\" for edit.paste: unknown key \"Banana\"; left out",
            "declared actions: \"file.save\" is declared twice; the first is kept",
            "declared actions: Ctrl+S for file.save is taken by edit.store; left out (unbind it there first: \"edit.store\" = [])",
        ]
    );
    assert_eq!(actions.ids(), ["edit.paste", "edit.store", "file.save"]);
    assert_eq!(actions.by_chord(chord("Ctrl+V")), Some("edit.paste"));
}

#[test]
fn a_keymap_file_rebinds_unbinds_and_swaps() {
    let mut actions = app();
    let problems = actions.apply_keymap(
        "# mine\n\
         \"file.save\" = \"Ctrl+W\"\n\
         \"edit.find\" = []\n\
         [edit]\n\
         undo = \"Ctrl+Y\"\n\
         redo = [\"Ctrl+Z\"]\n",
        "keymap.toml",
    );
    assert_eq!(problems, []);
    assert_eq!(actions.by_chord(chord("Ctrl+W")), Some("file.save"));
    assert_eq!(actions.by_chord(chord("Ctrl+S")), None);
    assert_eq!(actions.by_chord(chord("Ctrl+F")), None);
    // Undo and redo swap keys: neither claim is taken by the other's old binding.
    assert_eq!(actions.by_chord(chord("Ctrl+Y")), Some("edit.undo"));
    assert_eq!(actions.by_chord(chord("Ctrl+Z")), Some("edit.redo"));
    assert_eq!(actions.by_chord(chord("Ctrl+Shift+Z")), None);

    // The keymap the bindings make reads back to the same bindings.
    let text = actions.keymap_text();
    assert_eq!(
        text,
        "\"edit.find\" = []\n\"edit.redo\" = [\"Ctrl+Z\"]\n\"edit.undo\" = [\"Ctrl+Y\"]\n\"file.save\" = [\"Ctrl+W\"]\n"
    );
    let mut again = app();
    assert_eq!(again.apply_keymap(&text, "saved.toml"), []);
    for a in actions.iter() {
        assert_eq!(again.get(a.action.id()).unwrap().keys, a.keys);
    }

    // A new keymap replaces the old one, it does not add to it.
    assert_eq!(actions.apply_keymap("", "keymap.toml"), []);
    assert_eq!(actions.by_chord(chord("Ctrl+S")), Some("file.save"));
    assert_eq!(actions.by_chord(chord("Ctrl+W")), None);
}

#[test]
fn a_keymap_refuses_a_taken_key_with_file_and_line() {
    let mut actions = app();
    let problems = actions.apply_keymap(
        "\"file.open\" = \"Ctrl+Shift+O\"\n\
         \"edit.find\" = \"Ctrl+S\"\n",
        "keymap.toml",
    );
    let text: Vec<String> = problems.iter().map(ToString::to_string).collect();
    assert_eq!(
        text,
        [
            "keymap.toml:2: Ctrl+S for edit.find is taken by file.save; left out (unbind it there first: \"file.save\" = [])"
        ]
    );
    assert_eq!(
        problems[0].origin,
        Origin::File {
            file: "keymap.toml".into(),
            line: 2
        }
    );
    assert_eq!(actions.by_chord(chord("Ctrl+S")), Some("file.save"));
    assert_eq!(actions.get("edit.find").unwrap().keys, []);
    // The rest of the file applied.
    assert_eq!(actions.by_chord(chord("Ctrl+Shift+O")), Some("file.open"));
    assert_eq!(actions.problems().count(), 1);
}

#[test]
fn a_keymap_reports_unknown_ids_bad_chords_and_values() {
    let mut actions = app();
    let problems = actions.apply_keymap(
        "\"file.save\" = [\"Ctrl+Banana\", \"Ctrl+W\"]\n\
         \n\
         \"file.burn\" = \"Ctrl+B\"\n\
         \"edit.undo\" = 3\n",
        "k.toml",
    );
    let text: Vec<String> = problems.iter().map(ToString::to_string).collect();
    assert_eq!(
        text,
        [
            "k.toml:1: \"Ctrl+Banana\" for file.save: unknown key \"Banana\"; left out",
            "k.toml:3: no action \"file.burn\"; entry ignored",
            "k.toml:4: edit.undo: expected a chord (\"Ctrl+S\") or a list of chords; entry ignored",
        ]
    );
    assert_eq!(actions.get("file.save").unwrap().keys, [chord("Ctrl+W")]);
    assert_eq!(actions.get("edit.undo").unwrap().keys, [chord("Ctrl+Z")]);
}

#[test]
fn a_keymap_that_is_not_toml_changes_nothing() {
    let mut actions = app();
    actions.apply_keymap("\"file.save\" = \"Ctrl+W\"\n", "k.toml");
    let problems = actions.apply_keymap("\"file.save\" = \"Ctrl+Q\"\n\"edit.undo\" = \n", "k.toml");
    assert_eq!(problems.len(), 1);
    assert!(
        matches!(
            &problems[0],
            Problem {
                origin: Origin::File { line: 2, .. },
                kind: ProblemKind::Syntax { .. }
            }
        ),
        "{problems:?}"
    );
    assert_eq!(
        actions.by_chord(chord("Ctrl+S")),
        Some("file.save"),
        "back to the defaults"
    );
    assert_eq!(actions.by_chord(chord("Ctrl+Q")), None);

    let huge = "#".repeat(KEYMAP_BYTES_MAX + 1);
    let problems = actions.apply_keymap(&huge, "k.toml");
    assert_eq!(problems[0].kind, ProblemKind::TooLarge);
}

#[test]
fn a_rebind_at_run_time_is_refused_when_taken() {
    let mut actions = app();
    let refused = actions
        .rebind("edit.find", vec![chord("Ctrl+S")])
        .unwrap_err();
    assert_eq!(
        refused.to_string(),
        "rebind: Ctrl+S for edit.find is taken by file.save; left out (unbind it there first: \"file.save\" = [])"
    );
    assert_eq!(
        actions.get("edit.find").unwrap().keys,
        [chord("Ctrl+F"), chord("/")],
        "unchanged"
    );
    assert_eq!(actions.problems().count(), 0);

    actions
        .rebind("edit.find", vec![chord("Ctrl+Shift+F")])
        .unwrap();
    assert_eq!(actions.by_chord(chord("Ctrl+Shift+F")), Some("edit.find"));
    actions.reset_keys("edit.find");
    assert_eq!(actions.by_chord(chord("Ctrl+F")), Some("edit.find"));
    assert_eq!(actions.by_chord(chord("Ctrl+Shift+F")), None);
}

#[test]
fn key_presses_dispatch_to_handlers_and_the_queue() {
    let mut actions = app();
    let saved = Rc::new(Cell::new(0));
    let s = saved.clone();
    actions.on("file.save", move || s.set(s.get() + 1));

    let a = Event::Text("a".into());
    let mut events = vec![
        press(Key::S, ctrl()),
        a.clone(),
        press(Key::Y, ctrl()),
        press(Key::Q, ctrl()),
    ];
    let refused = actions.handle_events(&mut events, Platform::Other, false);
    assert_eq!(refused, []);
    assert_eq!(saved.get(), 1);
    assert_eq!(
        actions.take_fired(),
        [Fired {
            id: "edit.redo",
            source: Source::Key
        }]
    );
    // Bound presses are taken out; the rest stays for the widgets.
    assert_eq!(events, [a, press(Key::Q, ctrl())]);
    assert_eq!(actions.take_fired(), []);

    actions.trigger("view.theme", Source::Menu).unwrap();
    assert_eq!(
        actions.take_fired(),
        [Fired {
            id: "view.theme",
            source: Source::Menu
        }]
    );
    assert_eq!(
        actions.trigger("view.nope", Source::Palette),
        Err(Refused::Unknown("view.nope".into()))
    );
}

#[test]
fn typing_chords_stay_with_a_focused_text_field() {
    let mut actions = app();
    let slash = press(Key::Slash, Modifiers::NONE);
    let mut events = vec![
        slash.clone(),
        press(Key::F, ctrl()),
        press(Key::F6, Modifiers::NONE),
    ];
    actions.handle_events(&mut events, Platform::Other, true);
    assert_eq!(events, std::slice::from_ref(&slash));
    let ids: Vec<_> = actions.take_fired().into_iter().map(|f| f.id).collect();
    assert_eq!(ids, ["edit.find", "focus.next_region"]);

    let mut events = vec![slash];
    actions.handle_events(&mut events, Platform::Other, false);
    assert_eq!(events, []);
    assert_eq!(actions.take_fired()[0].id, "edit.find");
}

#[test]
fn a_held_key_repeats_only_actions_that_ask() {
    let mut actions = app();
    let repeat = |key| Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: true,
        modifiers: ctrl(),
    };
    let mut events = vec![repeat(Key::Z), repeat(Key::S)];
    actions.handle_events(&mut events, Platform::Other, false);
    assert_eq!(events, [], "both are bound, so both are taken");
    let ids: Vec<_> = actions.take_fired().into_iter().map(|f| f.id).collect();
    assert_eq!(ids, ["edit.undo"]);
}

#[test]
fn a_disabled_action_reports_its_reason() {
    let mut actions = app();
    let why = "Nothing to redo: the last step was undone already";
    actions.set_enabled("edit.redo", Err(why.into()));
    assert_eq!(actions.get("edit.redo").unwrap().enabled, Err(why));

    let refused = actions.trigger("edit.redo", Source::Menu).unwrap_err();
    assert_eq!(refused.to_string(), format!("edit.redo: {why}"));

    let mut events = vec![press(Key::Y, ctrl())];
    let refused = actions.handle_events(&mut events, Platform::Other, false);
    assert_eq!(
        refused,
        [Refused::Disabled {
            id: "edit.redo",
            reason: why.into(),
            source: Source::Key
        }]
    );
    assert_eq!(
        events,
        [],
        "the press is still the action's, not a widget's"
    );
    assert_eq!(actions.take_fired(), []);

    actions.set_enabled("edit.redo", Ok(()));
    actions.trigger("edit.redo", Source::Menu).unwrap();
    assert_eq!(actions.take_fired().len(), 1);
}

#[test]
fn the_fired_queue_is_bounded_and_counts_what_it_drops() {
    let mut actions = app();
    for _ in 0..FIRED_MAX + 3 {
        actions.trigger("view.theme", Source::Code).unwrap();
    }
    assert_eq!(actions.take_fired().len(), FIRED_MAX);
    assert_eq!(actions.fired_dropped(), 3);
}

#[test]
fn handle_input_takes_presses_out_of_an_egui_frame() {
    let ctx = egui::Context::default();
    let mut actions = app();
    let input = egui::RawInput {
        events: vec![press(Key::S, ctrl()), press(Key::A, Modifiers::NONE)],
        ..Default::default()
    };
    ctx.begin_pass(input);
    assert_eq!(actions.handle_input(&ctx), []);
    let left = ctx.input(|i| i.events.clone());
    ctx.end_pass().textures_delta.clear();
    assert_eq!(actions.take_fired()[0].id, "file.save");
    assert_eq!(left, [press(Key::A, Modifiers::NONE)]);
    assert_eq!(chord("Ctrl+Shift+S").label(&ctx), "Ctrl+Shift+S");
}

#[test]
fn find_matches_titles_and_ids_best_first_and_bounded() {
    let mut actions = app();
    actions.set_enabled("file.save", Err("No document open".into()));

    let found = actions.find("save", 10);
    let ids: Vec<_> = found.iter().map(|f| f.id).collect();
    assert_eq!(
        ids,
        ["file.save", "file.save_as"],
        "disabled actions are found too"
    );
    assert_eq!(found[0].title_hits, [0, 1, 2, 3]);

    let found = actions.find("save as", 10);
    assert_eq!(found[0].id, "file.save_as");
    assert_eq!(found[0].title_hits, [0, 1, 2, 3, 5, 6]);

    // Word starts beat letters scattered in the middle.
    let found = actions.find("nr", 10);
    assert_eq!(found[0].id, "focus.next_region");

    // The id is searched too, a little below the title.
    let found = actions.find("theme", 10);
    assert_eq!(found[0].id, "view.theme");
    let found = actions.find("view.th", 10);
    assert_eq!(found[0].id, "view.theme");
    assert_eq!(found[0].title_hits, []);

    assert_eq!(actions.find("zzz", 10), []);
    assert_eq!(actions.find("e", 2).len(), 2);
    assert_eq!(actions.find("", 100).len(), actions.ids().len());
    assert_eq!(actions.find(&"s".repeat(10_000), 10), []);
}

#[test]
fn the_menu_tree_follows_groups_and_order() {
    let actions = app();
    let menus = actions.menu_tree();
    let names: Vec<_> = menus.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(
        names,
        ["File", "Edit", "View"],
        "the default menu order; groupless actions are in none"
    );
    assert_eq!(
        menus[0].items,
        [
            MenuItem::Menu(MenuNode {
                name: "Recent".into(),
                items: vec![MenuItem::Action("file.recent.clear")]
            }),
            MenuItem::Action("file.open"),
            MenuItem::Action("file.save"),
            MenuItem::Action("file.save_as"),
        ]
    );
    assert_eq!(
        menus[1].items,
        [
            MenuItem::Action("edit.find"),
            MenuItem::Action("edit.undo"),
            MenuItem::Action("edit.redo"),
        ]
    );
    let menus = app().menu_order(&["View"]).menu_tree();
    let names: Vec<_> = menus.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(names, ["View", "Edit", "File"]);
}
