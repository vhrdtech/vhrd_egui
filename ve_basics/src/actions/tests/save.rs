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
    actions.reset_keys("view.theme");
    actions.rebind("file.save", vec![chord("Ctrl+W")]).unwrap();
    actions.reset_keys("edit.undo");
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
    actions.reset_keys("file.save");
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
        refused.problem.kind,
        ProblemKind::Unreadable { .. }
    ));
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
