use super::*;

// Review 7: saving keeps what the user wrote: comments, order, unknown ids, refused chords.
#[test]
fn saving_patches_the_file_in_place() {
    let mut actions = app();
    let file = "# my keys\n\
                \"plugin.gone\" = \"Ctrl+G\" # a plugin not linked now\n\
                \"edit.find\" = [\"Ctrl+S\", \"Ctrl+J\"] # Ctrl+S is taken\n\
                \"file.open\" = \"Ctrl+Shift+O\"\n\
                \"view.theme\" = \"Ctrl+T\" # dark at night\n\
                \n\
                [edit]\n\
                undo = \"Ctrl+U\"\n";
    let problems = actions.apply_keymap(file, "k.toml");
    assert_eq!(problems.len(), 2, "{problems:?}");
    assert_eq!(
        actions.keymap_text().unwrap(),
        file,
        "nothing changed, nothing moves"
    );

    actions.rebind("file.open", vec![chord("Ctrl+P")]).unwrap();
    actions.reset_keys("view.theme").unwrap();
    actions.rebind("file.save", vec![chord("Ctrl+W")]).unwrap();
    actions.reset_keys("edit.undo").unwrap();
    assert_eq!(
        actions.keymap_text().unwrap(),
        "# my keys\n\
         \"plugin.gone\" = \"Ctrl+G\" # a plugin not linked now\n\
         \"edit.find\" = [\"Ctrl+S\", \"Ctrl+J\"] # Ctrl+S is taken\n\
         \"file.open\" = [\"Ctrl+P\"]\n\
          # dark at night\n\
         \n\
         \"file.save\" = [\"Ctrl+W\"]\n\
         [edit]\n"
    );
}

#[test]
fn saving_writes_defaults_where_an_entry_shares_its_line() {
    let mut actions = app();
    actions.apply_keymap(
        "file = { save = \"Ctrl+W\", open = \"Ctrl+P\" }\n",
        "k.toml",
    );
    actions.reset_keys("file.save").unwrap();
    let text = actions.keymap_text().unwrap();
    assert_eq!(text, "file = { save = [\"Ctrl+S\"], open = \"Ctrl+P\" }\n");
    let mut again = app();
    assert_eq!(again.apply_keymap(&text, "k.toml"), []);
    assert_eq!(again.get("file.save").unwrap().keys, [chord("Ctrl+S")]);
}

#[test]
fn saving_over_a_file_that_cannot_be_read_is_refused() {
    let mut actions = app();
    actions.apply_keymap("\"file.save\" = \"Ctrl+W\n", "k.toml");
    let refused = actions.keymap_text().unwrap_err();
    assert!(matches!(
        &refused,
        KeymapSaveError::Unreadable(Problem {
            kind: ProblemKind::Unreadable { .. },
            ..
        })
    ));
    assert_eq!(
        actions.save_keymap("\"file.save\" = \"Ctrl+W\n"),
        Err(refused.clone())
    );
    assert!(
        refused
            .to_string()
            .starts_with("keymap not saved, the file has an error to fix first: k.toml:1:")
    );
    actions.apply_keymap("", "k.toml");
    assert_eq!(actions.keymap_text().unwrap(), "");
}

/// Deterministic xorshift.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

fn many(n: usize) -> Actions {
    Actions::from_list(
        (0..n).map(|i| Action::new(leak(format!("a.act{i}")), leak(format!("Action {i}")))),
    )
}

// Random rebinds, saved and loaded into a fresh registry, bind the same and save the same text.
#[test]
fn saved_rebinds_load_back_the_same() {
    let mut rng = Rng(0x1234_5678_9abc_def1);
    for _ in 0..300 {
        let mut a = many(20);
        for _ in 0..rng.next() % 30 {
            let id = format!("a.act{}", rng.next() % 20);
            let keys: Vec<Chord> = (0..rng.next() % 3)
                .map(|_| {
                    let key =
                        Key::ALL[usize::try_from(rng.next() % Key::ALL.len() as u64).unwrap()];
                    let m = rng.next();
                    Chord {
                        command: m & 1 != 0,
                        alt: m & 2 != 0,
                        shift: m & 4 != 0,
                        key,
                    }
                })
                .collect();
            let _refused_when_taken = a.rebind(&id, keys);
        }
        let text = a.keymap_text().unwrap();
        let mut b = many(20);
        assert_eq!(b.apply_keymap(&text, "saved.toml"), [], "{text}");
        for (x, y) in a.iter().zip(b.iter()) {
            assert_eq!(x.keys, y.keys, "{text}");
        }
        assert_eq!(b.keymap_text().unwrap(), text);
    }
}

// Re-check C: saving goes onto the file as it is on disk now.
#[test]
fn saving_keeps_outside_edits_and_refuses_a_collision() {
    let loaded = "\"file.save\" = \"Ctrl+W\"\n\"file.open\" = \"Ctrl+P\"\n";
    let mut actions = app();
    assert_eq!(actions.apply_keymap(loaded, "k.toml"), []);
    actions
        .rebind("file.save", vec![chord("Ctrl+Shift+W")])
        .unwrap();

    // The user edited another entry and added a comment meanwhile: both stay.
    let disk = "# mine\n\"file.save\" = \"Ctrl+W\"\n\"file.open\" = \"Ctrl+Shift+P\"\n";
    let saved = actions.save_keymap(disk).unwrap();
    assert_eq!(
        saved,
        "# mine\n\"file.save\" = [\"Ctrl+Shift+W\"]\n\"file.open\" = \"Ctrl+Shift+P\"\n"
    );
    // The result counts as loaded: saving again onto it changes nothing.
    assert_eq!(actions.save_keymap(&saved).unwrap(), saved);

    // The user changed the very entry the app edits: refused, nothing overwritten.
    actions
        .rebind("file.save", vec![chord("Ctrl+Alt+W")])
        .unwrap();
    let disk = saved.replace("Ctrl+Shift+W", "F2");
    assert_eq!(
        actions.save_keymap(&disk),
        Err(KeymapSaveError::Changed {
            id: "file.save".into(),
            line: Some(2)
        })
    );
    let refused = actions.save_keymap("\"x\" = ").unwrap_err();
    assert!(
        matches!(refused, KeymapSaveError::Unreadable(_)),
        "{refused:?}"
    );

    // No file on disk yet: the edits make one.
    let mut fresh = app();
    fresh.rebind("view.theme", vec![chord("Ctrl+T")]).unwrap();
    assert_eq!(
        fresh.save_keymap("").unwrap(),
        "\"view.theme\" = [\"Ctrl+T\"]\n"
    );
}

// Re-check E: a reset removes every entry of the id, the ignored repeat too, so none applies on the next load.
#[test]
fn a_reset_removes_every_entry_of_its_id() {
    let mut actions = app();
    let problems = actions.apply_keymap(
        "\"file.save\" = \"Ctrl+W\"\n[file]\nsave = \"Ctrl+Q\" # repeat\n",
        "k.toml",
    );
    assert_eq!(problems.len(), 1);
    actions.reset_keys("file.save").unwrap();
    assert_eq!(actions.keymap_text().unwrap(), "[file]\n# repeat\n");
}
