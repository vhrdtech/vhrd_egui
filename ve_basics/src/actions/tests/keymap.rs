use super::*;

fn file_line(line: usize) -> Origin {
    Origin::File {
        file: "k.toml".into(),
        line,
    }
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
        "k.toml",
    );
    assert_eq!(problems, []);
    assert_eq!(actions.by_chord(chord("Ctrl+W")), Some("file.save"));
    assert_eq!(actions.by_chord(chord("Ctrl+S")), None);
    assert_eq!(actions.by_chord(chord("Ctrl+F")), None);
    // Undo and redo swap keys: neither claim is taken by the other's old binding.
    assert_eq!(actions.by_chord(chord("Ctrl+Y")), Some("edit.undo"));
    assert_eq!(actions.by_chord(chord("Ctrl+Z")), Some("edit.redo"));
    assert_eq!(actions.by_chord(chord("Ctrl+Shift+Z")), None);
    assert_eq!(
        actions.get("edit.redo").unwrap().default_keys,
        [chord("Ctrl+Shift+Z"), chord("Ctrl+Y")]
    );

    // A new keymap replaces the old one, it does not add to it.
    assert_eq!(actions.apply_keymap("", "k.toml"), []);
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
    assert_eq!(actions.by_chord(chord("Ctrl+S")), Some("file.save"));
    assert_eq!(actions.get("edit.find").unwrap().keys, []);
    assert_eq!(
        actions.by_chord(chord("Ctrl+Shift+O")),
        Some("file.open"),
        "the rest applied"
    );
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
            "k.toml:3: no action \"file.burn\"; entry ignored, kept in the file",
            "k.toml:4: edit.undo: expected a chord (\"Ctrl+S\") or a list of at most 16; entry ignored",
        ]
    );
    assert_eq!(actions.get("file.save").unwrap().keys, [chord("Ctrl+W")]);
    assert_eq!(actions.get("edit.undo").unwrap().keys, [chord("Ctrl+Z")]);
}

// Review 6: a typo does not unbind the action; only an empty list does.
#[test]
fn an_entry_whose_chords_all_fail_keeps_the_declared_ones() {
    let mut actions = app();
    let problems = actions.apply_keymap("\"file.save\" = \"Ctrl+Sx\"\n", "k.toml");
    assert_eq!(problems.len(), 1);
    assert!(matches!(problems[0].kind, ProblemKind::BadChord { .. }));
    assert_eq!(actions.get("file.save").unwrap().keys, [chord("Ctrl+S")]);
}

// Review 8: the same id twice is a problem naming both lines; the first applies.
#[test]
fn the_same_id_twice_in_a_keymap_names_both_lines() {
    let mut actions = app();
    let problems = actions.apply_keymap(
        "\"file.save\" = \"Ctrl+W\"\n[file]\nsave = \"Ctrl+Q\"\n",
        "k.toml",
    );
    assert_eq!(
        problems,
        [Problem {
            origin: file_line(3),
            kind: ProblemKind::RepeatedId {
                id: "file.save".into(),
                kept: file_line(1)
            }
        }]
    );
    assert_eq!(actions.get("file.save").unwrap().keys, [chord("Ctrl+W")]);
}

#[test]
fn a_keymap_that_cannot_be_read_changes_nothing() {
    let mut actions = app();
    actions.apply_keymap("\"file.save\" = \"Ctrl+W\"\n", "k.toml");
    let problems = actions.apply_keymap("\"file.save\" = \"Ctrl+Q\"\n\"edit.undo\" = \n", "k.toml");
    assert_eq!(problems.len(), 1);
    assert!(
        matches!(
            &problems[0],
            Problem {
                origin: Origin::File { line: 2, .. },
                kind: ProblemKind::Unreadable { .. }
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
    assert!(matches!(
        actions.apply_keymap(&huge, "k.toml")[0].kind,
        ProblemKind::Unreadable { .. }
    ));
    let many: String = (0..=KEYMAP_ENTRIES_MAX)
        .map(|i| format!("\"x{i}\"=1\n"))
        .collect();
    assert!(matches!(
        actions.apply_keymap(&many, "k.toml")[0].kind,
        ProblemKind::Unreadable { .. }
    ));
}

// Review 10: problems are bounded, line numbers come from a table, one list is bounded.
#[test]
fn hostile_keymaps_give_bounded_problems() {
    let mut actions = app();
    let unknown: String = (0..KEYMAP_ENTRIES_MAX)
        .map(|i| format!("\"zz.{i:05}\" = \"A\"\n"))
        .collect();
    let problems = actions.apply_keymap(&unknown, "k.toml");
    assert_eq!(problems.len(), KEYMAP_PROBLEMS_MAX + 1);
    assert_eq!(
        problems[KEYMAP_PROBLEMS_MAX - 1].origin,
        file_line(KEYMAP_PROBLEMS_MAX)
    );
    assert_eq!(
        problems[KEYMAP_PROBLEMS_MAX].kind,
        ProblemKind::Truncated {
            more: KEYMAP_ENTRIES_MAX - KEYMAP_PROBLEMS_MAX
        }
    );

    let long = format!("\"edit.find\" = [{}]\n", vec!["\"A\""; 60_000].join(","));
    let problems = actions.apply_keymap(&long, "k.toml");
    assert_eq!(
        problems,
        [Problem {
            origin: file_line(1),
            kind: ProblemKind::BadValue {
                id: "edit.find".into()
            }
        }]
    );
    assert_eq!(
        actions.get("edit.find").unwrap().keys,
        [chord("Ctrl+F"), chord("/")]
    );

    let crlf = "# a\r\n# b\r\n\"zz\" = \"A\"\r\n";
    assert_eq!(actions.apply_keymap(crlf, "k.toml")[0].origin, file_line(3));
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

// A rebind of an earlier-bound action must not take a chord a later rebind holds.
#[test]
fn a_rebind_never_takes_a_chord_from_another_override() {
    let mut actions = app();
    actions.rebind("edit.find", vec![chord("Ctrl+G")]).unwrap();
    actions.rebind("view.theme", vec![chord("Ctrl+H")]).unwrap();
    let refused = actions
        .rebind("edit.find", vec![chord("Ctrl+H")])
        .unwrap_err();
    assert!(
        matches!(
            refused.kind,
            ProblemKind::ChordTaken {
                holder: "view.theme",
                ..
            }
        ),
        "{refused:?}"
    );
    assert_eq!(actions.by_chord(chord("Ctrl+H")), Some("view.theme"));
    assert_eq!(actions.by_chord(chord("Ctrl+G")), Some("edit.find"));
}

// Review 15: rebinds replace, they do not pile up.
#[test]
fn many_rebinds_keep_one_entry() {
    let mut actions = app();
    for i in 0..3000 {
        let key = if i % 2 == 0 { "Ctrl+G" } else { "Ctrl+H" };
        actions.rebind("edit.find", vec![chord(key)]).unwrap();
    }
    assert_eq!(
        actions.keymap_text().unwrap(),
        "\"edit.find\" = [\"Ctrl+H\"]\n"
    );
}
