use super::*;

fn kinds(actions: &Actions) -> Vec<ProblemKind> {
    actions.problems().map(|p| p.kind.clone()).collect()
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
    assert_eq!(save.default_keys, [chord("Ctrl+S")]);
    assert_eq!(save.enabled, Ok(()));
    assert_eq!(save.checked, None);
    assert_eq!(
        actions.get("view.theme").unwrap().action.icon_name(),
        Some("sun")
    );
    assert!(actions.get("file.nope").is_none());
    assert!(save.action.location().file().ends_with("tests/mod.rs"));
}

#[test]
fn declaration_problems_are_reported_not_panicked() {
    let actions = Actions::from_list([
        SAVE,
        Action::new("Bad Id", "Bad"),
        Action::new("edit.paste", "Paste").keys(&["Ctrl+Banana", "Ctrl+V"]),
        Action::new("edit.store", "Store").keys(&["Ctrl+S"]),
    ]);
    assert_eq!(
        kinds(&actions),
        [
            ProblemKind::BadId {
                id: "Bad Id".into()
            },
            ProblemKind::BadChord {
                id: "edit.paste".into(),
                text: "Ctrl+Banana".into(),
                error: ChordError::UnknownKey("Banana".into())
            },
            ProblemKind::ChordTaken {
                id: "file.save".into(),
                chord: chord("Ctrl+S"),
                holder: "edit.store"
            },
        ]
    );
    let taken = actions.problems().last().unwrap().to_string();
    assert!(
        taken.contains("tests/mod.rs:"),
        "names the declaration: {taken}"
    );
    assert!(taken.ends_with("Ctrl+S for file.save is taken by edit.store; left out (unbind it there first: \"edit.store\" = [])"));
    assert_eq!(actions.ids(), ["edit.paste", "edit.store", "file.save"]);
    assert_eq!(actions.by_chord(chord("Ctrl+V")), Some("edit.paste"));
}

// Review 13: the same id from two places keeps the first by file and line, whatever the link order.
#[test]
fn a_repeated_id_keeps_the_first_declaration_by_place() {
    let first = Action::new("file.save", "Save here");
    let second = Action::new("file.save", "Save there");
    for list in [[first, second], [second, first]] {
        let actions = Actions::from_list(list);
        assert_eq!(
            actions.get("file.save").unwrap().action.title(),
            "Save here"
        );
        let problems: Vec<_> = actions.problems().collect();
        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].origin, Origin::of(&second));
        assert_eq!(
            problems[0].kind,
            ProblemKind::RepeatedId {
                id: "file.save".into(),
                kept: Origin::of(&first)
            }
        );
    }
}

// Review 11: odd groups are problems, not silently lost or split.
#[test]
fn bad_groups_are_reported_and_cases_join_one_menu() {
    let actions = Actions::from_list([
        Action::new("x.blank", "Blank").group(" "),
        Action::new("x.slash", "Slash").group("/"),
        Action::new("x.double", "Double").group("File//Recent"),
        Action::new("x.trailing", "Trailing").group("File/Recent/"),
        Action::new("x.lower", "Lower").group("file"),
        Action::new("a.open", "Open").group("File"),
    ]);
    let bad: Vec<(String, String)> = actions
        .problems()
        .filter_map(|p| match &p.kind {
            ProblemKind::BadGroup { id, why, .. } => Some((id.clone(), why.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(
        bad,
        [
            (
                "x.blank".into(),
                "an empty menu name; the action is in no menu".into()
            ),
            (
                "x.double".into(),
                "an empty menu name; the action is in no menu".into()
            ),
            (
                "x.lower".into(),
                "spelled \"File\" elsewhere; put there".into()
            ),
            (
                "x.slash".into(),
                "an empty menu name; the action is in no menu".into()
            ),
            (
                "x.trailing".into(),
                "an empty menu name; the action is in no menu".into()
            ),
        ]
    );
    let menus = actions.menu_tree();
    assert_eq!(menus.len(), 1);
    assert_eq!(menus[0].name, "File");
    assert_eq!(
        menus[0].items,
        [MenuItem::Action("x.lower"), MenuItem::Action("a.open")],
        "same order, then by title"
    );
}

#[test]
fn mnemonics_must_be_in_the_title_and_unique_in_their_menu() {
    let actions = Actions::from_list([
        SAVE,
        Action::new("file.share", "Share")
            .group("File")
            .mnemonic('s'),
        Action::new("file.open", "Open").group("File").mnemonic('x'),
        Action::new("edit.select", "Select")
            .group("Edit")
            .mnemonic('S'),
    ]);
    assert_eq!(
        kinds(&actions),
        [
            ProblemKind::BadMnemonic {
                id: "file.open".into(),
                letter: 'x',
                holder: None
            },
            ProblemKind::BadMnemonic {
                id: "file.share".into(),
                letter: 's',
                holder: Some("file.save")
            },
        ]
    );
    assert_eq!(
        actions.get("file.save").unwrap().action.mnemonic_letter(),
        Some('S')
    );
}

#[test]
fn a_destructive_action_never_takes_enter_and_comes_last() {
    let actions = Actions::from_list([
        Action::new("file.wipe", "Wipe…")
            .group("File")
            .destructive()
            .keys(&["Enter", "Ctrl+Shift+Delete"]),
        Action::new("file.open", "Open").group("File").order(5),
    ]);
    assert_eq!(
        kinds(&actions),
        [ProblemKind::DestructiveEnter {
            id: "file.wipe".into()
        }]
    );
    assert_eq!(
        actions.get("file.wipe").unwrap().keys,
        [chord("Ctrl+Shift+Delete")]
    );
    assert_eq!(
        actions.menu_tree()[0].items,
        [MenuItem::Action("file.open"), MenuItem::Action("file.wipe")]
    );
    assert!(actions.get("file.wipe").unwrap().action.is_destructive());
}

#[test]
fn a_toggle_keeps_its_checked_state() {
    let mut actions = app();
    assert_eq!(actions.get("view.grid").unwrap().checked, Some(false));
    actions.set_checked("view.grid", true);
    assert_eq!(actions.get("view.grid").unwrap().checked, Some(true));
    assert!(actions.get("view.grid").unwrap().action.is_toggle());
}

// Gap: chords a text edit keeps are bound, with a warning, not a problem.
#[test]
fn chords_a_text_edit_keeps_are_warned_about() {
    let actions = app();
    let warned: Vec<_> = actions
        .warnings()
        .map(|w| match &w.kind {
            ProblemKind::TextEditKeeps { id, chord } => (id.clone(), chord.text()),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(
        warned,
        [
            ("edit.copy".into(), "Ctrl+C".into()),
            ("edit.paste".into(), "Ctrl+V".into()),
            ("edit.redo".into(), "Ctrl+Shift+Z".into()),
            ("edit.redo".into(), "Ctrl+Y".into()),
            ("edit.undo".into(), "Ctrl+Z".into()),
        ]
    );
    assert_eq!(actions.by_chord(chord("Ctrl+C")), Some("edit.copy"));
}

#[test]
fn scoped_chords_may_repeat_global_ones() {
    let actions = app();
    assert_eq!(actions.by_chord(chord("Ctrl+F")), Some("edit.find"));
    assert_eq!(
        actions.by_chord_in("tracks", chord("Ctrl+F")),
        Some("tracks.remove")
    );
    assert_eq!(
        actions.by_chord_in("tracks", chord("Ctrl+S")),
        None,
        "no fallback to global here"
    );
    let clash = Actions::from_list([
        Action::new("tracks.a", "A")
            .scope("tracks")
            .keys(&["Delete"]),
        Action::new("tracks.b", "B")
            .scope("tracks")
            .keys(&["Delete"]),
    ]);
    assert_eq!(
        kinds(&clash),
        [ProblemKind::ChordTaken {
            id: "tracks.b".into(),
            chord: chord("Delete"),
            holder: "tracks.a"
        }]
    );
}

// Re-check E: every holder of a chord across scopes, for a settings window.
#[test]
fn bound_to_lists_every_scope() {
    let mut actions = app();
    let mut held = actions.bound_to(chord("Ctrl+F"));
    held.sort_unstable();
    assert_eq!(held, [("", "edit.find"), ("tracks", "tracks.remove")]);
    assert_eq!(actions.bound_to(chord("Ctrl+Q")), []);
    // A scope no action has is ignored.
    actions.set_scopes(&["nowhere", "tracks"]);
    assert_eq!(actions.scopes, ["tracks"]);
}

// Re-check E: a global Alt+letter would shadow the menu that letter opens.
#[test]
fn alt_letters_of_top_level_menus_are_refused() {
    let actions = Actions::from_list([
        Action::new("file.open", "Open").group("File"),
        Action::new("edit.fill", "Fill").keys(&["Alt+F", "Alt+Shift+F", "Alt+G"]),
        Action::new("tracks.fold", "Fold")
            .scope("tracks")
            .keys(&["Alt+F"]),
    ]);
    assert_eq!(Actions::menu_mnemonic("File"), Some('F'));
    assert_eq!(
        kinds(&actions),
        [ProblemKind::MenuMnemonicTaken {
            id: "edit.fill".into(),
            chord: chord("Alt+F"),
            menu: "File".into()
        }]
    );
    assert_eq!(
        actions.get("edit.fill").unwrap().keys,
        [chord("Alt+Shift+F"), chord("Alt+G")]
    );
    assert_eq!(
        actions.by_chord_in("tracks", chord("Alt+F")),
        Some("tracks.fold")
    );
}
