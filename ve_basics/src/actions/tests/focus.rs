use super::*;
use egui::{Id, Ui};

/// Draw `ui` for two frames (focus settles), then a frame whose input is `events`: dispatch at its start, as
/// an app does. Returns the events left for the widgets.
fn frame_after(
    actions: &mut Actions,
    ui: impl Fn(&mut Ui),
    events: Vec<Event>,
    popup: bool,
) -> Vec<Event> {
    let ctx = egui::Context::default();
    for _ in 0..2 {
        ctx.run_ui(egui::RawInput::default(), |u| ui(u))
            .textures_delta
            .clear();
    }
    ctx.begin_pass(egui::RawInput {
        events,
        ..Default::default()
    });
    if popup {
        egui::Popup::open_id(&ctx, Id::new("menu"));
    }
    assert_eq!(actions.handle_input(&ctx), []);
    let left = ctx.input(|i| i.events.clone());
    ctx.end_pass().textures_delta.clear();
    left
}

fn nothing(_: &mut Ui) {}

fn focused_button(ui: &mut Ui) {
    ui.button("x").request_focus();
}

fn focused_slider(ui: &mut Ui) {
    let mut v = 0.5;
    ui.add(egui::Slider::new(&mut v, 0.0..=1.0).show_value(false))
        .request_focus();
}

fn focused_text(ui: &mut Ui) {
    let mut text = String::from("abc");
    ui.text_edit_singleline(&mut text).request_focus();
}

#[test]
fn without_focus_every_bound_key_fires() {
    let mut actions = app();
    let left = frame_after(
        &mut actions,
        nothing,
        vec![
            press(Key::Space, Modifiers::NONE),
            press(Key::G, Modifiers::NONE),
            press(Key::S, ctrl()),
        ],
        false,
    );
    assert_eq!(left, []);
    assert_eq!(
        fired_ids(&mut actions),
        ["view.play", "view.grid", "file.save"]
    );
}

// Review 2: a focused button keeps only its own keys, not every plain chord.
#[test]
fn a_focused_button_keeps_space_but_not_other_keys() {
    let mut actions = app();
    let space = press(Key::Space, Modifiers::NONE);
    let left = frame_after(
        &mut actions,
        focused_button,
        vec![
            space.clone(),
            press(Key::P, Modifiers::NONE),
            press(Key::G, shift(Modifiers::NONE)),
        ],
        false,
    );
    assert_eq!(
        left,
        [space, press(Key::G, shift(Modifiers::NONE))],
        "Shift+G is bound to nothing"
    );
    assert_eq!(fired_ids(&mut actions), ["view.play"]);
}

// Gap: Escape and the arrows stay with a focused slider and an open popup or menu.
#[test]
fn a_focused_slider_and_an_open_popup_keep_escape_and_the_arrows() {
    let mut actions = Actions::from_list([
        Action::new("view.close", "Close").keys(&["Escape"]),
        Action::new("view.next", "Next").keys(&["Right"]),
        Action::new("view.grid", "Grid").keys(&["G"]),
    ]);
    let keys = || {
        vec![
            press(Key::Escape, Modifiers::NONE),
            press(Key::ArrowRight, Modifiers::NONE),
            press(Key::G, Modifiers::NONE),
        ]
    };
    // egui itself drops a widget's focus on Escape before the frame starts, so the slider keeps the arrows.
    let left = frame_after(&mut actions, focused_slider, keys()[1..].to_vec(), false);
    assert_eq!(left, keys()[1..2]);
    assert_eq!(fired_ids(&mut actions), ["view.grid"]);

    let left = frame_after(&mut actions, nothing, keys(), true);
    assert_eq!(left, keys()[..2]);
    assert_eq!(fired_ids(&mut actions), ["view.grid"]);

    frame_after(&mut actions, nothing, keys(), false);
    assert_eq!(
        fired_ids(&mut actions),
        ["view.close", "view.next", "view.grid"]
    );
}

// Review 2 and 3: a focused text edit keeps typing and its editing chords; commands still fire.
#[test]
fn a_focused_text_edit_keeps_typing_and_editing_chords() {
    let mut actions = app();
    let kept = vec![
        press(Key::Slash, Modifiers::NONE),
        press(Key::G, Modifiers::NONE),
        press(Key::Z, ctrl()),
        press(Key::Y, ctrl()),
        Event::Copy,
        Event::Paste("x".into()),
    ];
    let mut events = kept.clone();
    events.extend([
        press(Key::F, ctrl()),
        press(Key::F6, Modifiers::NONE),
        press(Key::S, ctrl()),
    ]);
    let left = frame_after(&mut actions, focused_text, events, false);
    assert_eq!(left, kept);
    assert_eq!(
        fired_ids(&mut actions),
        ["edit.find", "focus.next_region", "file.save"]
    );
}

// Review 9: on macOS Option types characters in a text edit; elsewhere Alt chords stay commands.
#[test]
fn option_chords_type_in_a_mac_text_edit() {
    let mut actions = app();
    let alt_s = vec![press(Key::S, alt()), Event::Text("ß".into())];

    let mut events = alt_s.clone();
    actions.handle_events(&mut events, Platform::Mac, Focus::TextEdit);
    assert_eq!(
        events, alt_s,
        "the key and its text stay with the text edit"
    );
    assert_eq!(actions.take_fired(), []);

    let mut events = alt_s.clone();
    actions.handle_events(&mut events, Platform::Mac, Focus::None);
    assert_eq!(events, [], "taken, and the ß it would type with it");
    assert_eq!(fired_ids(&mut actions), ["view.split"]);

    let mut events = vec![press(Key::S, alt())];
    actions.handle_events(&mut events, Platform::Other, Focus::TextEdit);
    assert_eq!(events, []);
    assert_eq!(fired_ids(&mut actions), ["view.split"]);
}
