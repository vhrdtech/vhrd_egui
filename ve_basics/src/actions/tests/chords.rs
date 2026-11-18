use super::*;

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
    assert_eq!(
        Chord::parse(&"Ctrl+".repeat(200_000)),
        Err(ChordError::TooLong)
    );
}

#[test]
fn every_key_with_every_modifier_reads_back_from_its_text() {
    for &key in Key::ALL {
        for m in 0..8 {
            let c = Chord {
                command: m & 1 != 0,
                alt: m & 2 != 0,
                shift: m & 4 != 0,
                key,
            };
            assert_eq!(Chord::parse(&c.text()), Ok(c), "{}", c.text());
        }
    }
}

// Review 12: key names in any case, short names, a bare plus.
#[test]
fn key_names_are_read_in_any_case() {
    assert_eq!(chord("ctrl+enter"), chord("Ctrl+Enter"));
    assert_eq!(chord("space"), Chord::key(Key::Space));
    assert_eq!(chord("ESC"), Chord::key(Key::Escape));
    assert_eq!(chord("pageup"), Chord::key(Key::PageUp));
    assert_eq!(chord("ctrl+up"), chord("Ctrl+ArrowUp"));
    assert_eq!(chord("pgdn"), Chord::key(Key::PageDown));
    assert_eq!(chord("f6"), Chord::key(Key::F6));
    assert_eq!(chord("+"), Chord::key(Key::Plus));
    assert_eq!(chord("Shift++"), chord("Shift+Plus"));
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
fn chords_know_who_keeps_them() {
    assert!(chord("/").is_typing());
    assert!(!chord("F5").is_typing());
    assert!(!chord("Ctrl+A").is_typing());
    assert!(chord("Ctrl+Z").is_text_editing());
    assert!(chord("Ctrl+Shift+Z").is_text_editing());
    assert!(chord("Alt+Left").is_text_editing());
    assert!(!chord("Ctrl+S").is_text_editing());
    assert!(chord("Space").is_navigation());
    assert!(chord("Shift+Tab").is_navigation());
    assert!(!chord("Ctrl+Enter").is_navigation());
    assert!(chord("Shift+Plus").is_symbol());
    assert!(!chord("Shift+A").is_symbol());
}

#[test]
fn a_chord_labels_itself_for_egui() {
    let ctx = egui::Context::default();
    assert_eq!(chord("Ctrl+Shift+S").label(&ctx), "Ctrl+Shift+S");
}
