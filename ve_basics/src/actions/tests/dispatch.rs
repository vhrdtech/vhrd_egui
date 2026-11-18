use super::*;
use std::cell::Cell;
use std::rc::Rc;
use std::sync::mpsc;
use std::time::Duration;

#[test]
fn key_presses_dispatch_to_handlers_and_the_queue() {
    let mut actions = app();
    let saved = Rc::new(Cell::new(0));
    let s = saved.clone();
    actions.on("file.save", move |fired| {
        assert_eq!(fired, &Fired::new("file.save", Source::Key));
        s.set(s.get() + 1);
    });

    let left = dispatch(
        &mut actions,
        vec![
            press(Key::S, ctrl()),
            press(Key::Y, ctrl()),
            press(Key::Q, ctrl()),
        ],
        Focus::None,
    );
    assert_eq!(saved.get(), 1);
    assert_eq!(actions.take_fired(), [Fired::new("edit.redo", Source::Key)]);
    assert_eq!(
        left,
        [press(Key::Q, ctrl())],
        "bound presses are taken out, the rest stays"
    );
    assert_eq!(actions.take_fired(), []);

    actions.trigger("view.theme", Source::Menu).unwrap();
    assert_eq!(
        actions.take_fired(),
        [Fired::new("view.theme", Source::Menu)]
    );
    assert_eq!(
        actions.trigger("view.nope", Source::Palette),
        Err(Refused::Unknown("view.nope".into()))
    );
}

// Review 1: a handler may call into the egui context it captured; handle_input must not hold egui's lock.
#[test]
fn a_handler_may_call_into_egui_during_handle_input() {
    let (done, finished) = mpsc::channel();
    #[expect(
        clippy::disallowed_methods,
        reason = "native-only test: the thread turns a deadlock into a failure instead of a hang"
    )]
    std::thread::spawn(move || {
        let ctx = egui::Context::default();
        let mut actions = app();
        let c = ctx.clone();
        actions.on("file.save", move |_| {
            c.request_repaint();
            let _ = c.input(|i| i.time);
        });
        ctx.begin_pass(egui::RawInput {
            events: vec![press(Key::S, ctrl())],
            ..Default::default()
        });
        let refused = actions.handle_input(&ctx);
        ctx.end_pass().textures_delta.clear();
        done.send(refused).unwrap();
    });
    // The bound only turns a deadlock into a failure; a working dispatch answers at once.
    let refused = finished
        .recv_timeout(Duration::from_secs(60))
        .expect("handle_input deadlocked");
    assert_eq!(refused, []);
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
    let refused = actions.handle_events(&mut events, Platform::Other, Focus::None);
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
fn a_held_key_repeats_only_actions_that_ask() {
    let mut actions = app();
    let left = dispatch(
        &mut actions,
        vec![
            key_event(Key::Z, ctrl(), true),
            key_event(Key::S, ctrl(), true),
        ],
        Focus::None,
    );
    assert_eq!(left, [], "both are bound, so both are taken");
    assert_eq!(fired_ids(&mut actions), ["edit.undo"]);
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

// Review 4: the platform turns Ctrl+C, X, V into Copy, Cut and Paste events with no key event.
#[test]
fn copy_and_paste_events_fire_their_actions_outside_text_edits() {
    let mut actions = app();
    let left = dispatch(
        &mut actions,
        vec![Event::Copy, Event::Paste("clip".into()), Event::Cut],
        Focus::None,
    );
    assert_eq!(left, [Event::Cut], "nothing is bound to Ctrl+X");
    assert_eq!(
        actions.take_fired(),
        [
            Fired::new("edit.copy", Source::Key),
            Fired {
                id: "edit.paste",
                source: Source::Key,
                pasted: Some("clip".into())
            },
        ]
    );
    let events = vec![Event::Copy, Event::Paste("clip".into())];
    assert_eq!(
        dispatch(&mut actions, events.clone(), Focus::TextEdit),
        events
    );
    assert_eq!(actions.take_fired(), []);
}

// Review 5: punctuation that needs Shift on the layout still matches its chord.
#[test]
fn shifted_symbols_match_their_chords() {
    let mut actions = app();
    // US: Ctrl and Shift and = gives the logical Plus with Shift held.
    // German: Ctrl and Shift and 7 gives the logical Slash with Shift held.
    let left = dispatch(
        &mut actions,
        vec![
            press(Key::Plus, shift(ctrl())),
            press(Key::Slash, shift(ctrl())),
        ],
        Focus::None,
    );
    assert_eq!(left, []);
    assert_eq!(fired_ids(&mut actions), ["view.zoom_in", "view.zoom_in"]);
    // A letter's Shift still counts: Ctrl+Shift+S is not Ctrl+S.
    dispatch(
        &mut actions,
        vec![press(Key::S, shift(ctrl()))],
        Focus::None,
    );
    assert_eq!(fired_ids(&mut actions), ["file.save_as"]);
}

// Review 9: the text a taken press would type is taken too.
#[test]
fn the_text_of_a_taken_press_is_taken_with_it() {
    let mut actions = app();
    let typed = Event::Text("g".into());
    let left = dispatch(
        &mut actions,
        vec![
            press(Key::G, Modifiers::NONE),
            typed.clone(),
            press(Key::Q, Modifiers::NONE),
            typed.clone(),
        ],
        Focus::None,
    );
    assert_eq!(left, [press(Key::Q, Modifiers::NONE), typed]);
    assert_eq!(fired_ids(&mut actions), ["view.grid"]);
}

#[test]
fn scoped_keys_fire_only_while_their_scope_is_active() {
    let mut actions = app();
    dispatch(&mut actions, vec![press(Key::F, ctrl())], Focus::None);
    assert_eq!(fired_ids(&mut actions), ["edit.find"]);
    let left = dispatch(
        &mut actions,
        vec![press(Key::Delete, Modifiers::NONE)],
        Focus::None,
    );
    assert_eq!(left.len(), 1, "tracks is not active");

    actions.set_scopes(&["tracks"]);
    dispatch(
        &mut actions,
        vec![
            press(Key::F, ctrl()),
            press(Key::Delete, Modifiers::NONE),
            press(Key::S, ctrl()),
        ],
        Focus::None,
    );
    assert_eq!(
        fired_ids(&mut actions),
        ["tracks.remove", "tracks.remove", "file.save"],
        "the scope wins, global keys still work"
    );
}

// Re-check D: a paste over the bound is refused, not queued.
#[test]
fn a_paste_over_the_bound_is_refused() {
    let mut actions = app();
    let big = "x".repeat(PASTED_BYTES_MAX + 1);
    let mut events = vec![Event::Paste(big), Event::Paste("ok".into())];
    let refused = actions.handle_events(&mut events, Platform::Other, Focus::None);
    assert_eq!(
        refused,
        [Refused::PasteTooLarge {
            id: "edit.paste",
            size_bytes: PASTED_BYTES_MAX + 1
        }]
    );
    assert_eq!(events, []);
    let fired = actions.take_fired();
    assert_eq!(fired.len(), 1);
    assert_eq!(fired[0].pasted.as_deref(), Some("ok"));
}
