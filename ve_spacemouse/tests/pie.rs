//! The pie menu in a headless egui (SPM-3): the demo's menu driven the way a person would, with the device's
//! buttons and the mouse. Time is the harness's own (0.05 s a step), so nothing depends on the host.
//!
//! After an intended visual change: `UPDATE_SNAPSHOTS=1 cargo test -p ve_spacemouse --test pie` and commit the
//! PNGs under `tests/snapshots/`.

#[path = "../examples/demo/menu.rs"]
#[allow(dead_code)]
mod menu;

use egui::{Pos2, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use menu::{Action, Can};
use ve_spacemouse::{ButtonEvent, Fired, PieMenu, PieState, Side, Slot, icons};

const O: Pos2 = pos2(450.0, 320.0);

/// Down and left of the menu's centre, on slice 5 of the right ring: a free slot.
fn below() -> Pos2 {
    O + vec2(-1.0, 1.0).normalized() * 120.0
}

#[derive(Default)]
struct App {
    can: Can,
    pie: PieState,
    pending: Vec<ButtonEvent>,
    fired: Vec<Fired<Action>>,
    clicks_below: u32,
    fonts: bool,
    /// Show [`long_menu`] instead of the demo's.
    long: bool,
}

fn harness() -> Harness<'static, App> {
    Harness::builder()
        .with_size(vec2(900.0, 700.0))
        .with_step_dt(0.05)
        .build_ui_state(
            |ui, app: &mut App| {
                if !app.fonts {
                    ve_theme::setup(ui.ctx());
                    ve_spacemouse::install_icons(ui.ctx());
                    app.fonts = true;
                }
                egui::CentralPanel::default().show(ui, |ui| {
                    // An app button right where the menu opens: the menu must keep clicks from it.
                    let r = ui.put(
                        egui::Rect::from_center_size(below(), vec2(80.0, 24.0)),
                        egui::Button::new("Below"),
                    );
                    if r.clicked() {
                        app.clicks_below += 1;
                    }
                });
                let pending = std::mem::take(&mut app.pending);
                let menu = if app.long {
                    long_menu()
                } else {
                    menu::menu(app.can)
                };
                app.fired
                    .extend(menu.show(ui.ctx(), &mut app.pie, &pending));
            },
            App::default(),
        )
}

/// Names as long as better_erc's (a name, then its key), ten slices on the left ring.
fn long_menu() -> PieMenu<Action> {
    use Action::*;
    let s = |label: &str, a| Some(Slot::new(label, icons::RULER, a));
    PieMenu {
        center_left: Some(Slot::new("Ruler · R", icons::RULER, AddDot)),
        center_right: Some(Slot::new("Move footprint · Tab", icons::TRASH, DeleteDot)),
        ring_left: vec![
            s("Full width · W", Copy),
            s("Zoom on a ruler end · Z", Paste),
            s("Type the length · L", Cut),
            s("The dimension's number · N", Measure),
            s("Back to Move · Esc", ZoomToFit),
            s("Stop measuring the PDF · Shift+M", Recenter),
            s("Select the drawing · S", Snap),
            s("Next page · PgDn", Highlight),
            s("Invert the page · I", Route),
            s("Previous page · PgUp", Flip),
        ],
        ..Default::default()
    }
}

fn button(h: &mut Harness<'_, App>, side: Side, pressed: bool) {
    h.state_mut().pending.push(ButtonEvent { side, pressed });
    h.step();
}

fn actions(h: &Harness<'_, App>) -> Vec<(Action, bool)> {
    h.state().fired.iter().map(|f| (f.action, f.tap)).collect()
}

#[test]
fn a_tap_fires_the_centre_slot_where_the_pointer_is() {
    let mut h = harness();
    h.hover_at(O);
    h.step();
    button(&mut h, Side::Left, true);
    button(&mut h, Side::Left, false);
    assert_eq!(actions(&h), vec![(Action::AddDot, true)]);
    assert_eq!(h.state().fired[0].at, O);
    assert!(!h.state().pie.is_open());
}

#[test]
fn hold_flick_out_and_chain_two_actions() {
    let mut h = harness();
    h.hover_at(O);
    h.step();
    button(&mut h, Side::Right, true);
    h.run_steps(10);
    // Up and out across the outer circle: slice 0, Highlight.
    for y in [60.0, 120.0, 170.0, 200.0] {
        h.hover_at(O - vec2(0.0, y));
        h.step();
    }
    // Back through the centre, then right and down: slice 2, Flip.
    for p in [O, O + vec2(100.0, 0.0), O + vec2(200.0, 0.0)] {
        h.hover_at(p);
        h.step();
    }
    assert_eq!(
        actions(&h),
        vec![(Action::Highlight, false), (Action::Flip, false)]
    );
    assert!(h.state().pie.is_open(), "open while held");
    button(&mut h, Side::Right, false);
    assert!(!h.state().pie.is_open());
    assert_eq!(h.state().fired.len(), 2, "the release fires nothing");
}

#[test]
fn a_disabled_slot_says_why_and_does_not_fire() {
    let mut h = harness();
    h.hover_at(O);
    h.step();
    button(&mut h, Side::Left, true);
    h.run_steps(10);
    // Paste is slice 1 (up right), disabled: nothing copied.
    let dir = vec2(1.0, -1.0).normalized();
    for d in [90.0, 150.0, 200.0] {
        h.hover_at(O + dir * d);
        h.step();
    }
    assert_eq!(h.state().pie.hot(), Some(1));
    assert!(actions(&h).is_empty());
    h.get_by_label_contains("Nothing copied yet");
}

#[test]
fn both_buttons_open_the_third_ring() {
    let mut h = harness();
    h.state_mut().can.save = true;
    h.hover_at(O);
    h.step();
    button(&mut h, Side::Left, true);
    button(&mut h, Side::Right, true);
    h.run_steps(10);
    for y in [100.0, 200.0] {
        h.hover_at(O - vec2(0.0, y));
        h.step();
    }
    assert_eq!(actions(&h), vec![(Action::Save, false)]);
}

#[test]
fn the_open_menu_keeps_clicks_from_the_app_and_a_click_on_a_free_slot_closes_it() {
    let mut h = harness();
    // Without the menu the button below takes the click.
    h.get_by_label("Below").click();
    h.run();
    assert_eq!(h.state().clicks_below, 1);

    h.hover_at(O);
    h.step();
    button(&mut h, Side::Right, true);
    h.run_steps(10);
    // Slice 5 (down left) of the right ring is free; the "Below" button sits under it.
    let below = below();
    h.hover_at(below);
    h.step();
    assert_eq!(h.state().pie.hot(), Some(5));
    h.drag_at(below);
    h.step();
    h.drop_at(below);
    h.step();
    assert_eq!(h.state().clicks_below, 1, "the menu took the click");
    assert!(
        !h.state().pie.is_open(),
        "a click on a free slot closes the menu"
    );
    assert!(actions(&h).is_empty());
}

#[test]
fn a_press_while_the_window_has_no_focus_is_ignored() {
    let mut h = harness();
    h.hover_at(O);
    h.step();
    h.input_mut().focused = false;
    button(&mut h, Side::Left, true);
    assert!(!h.state().pie.is_open());
}

#[test]
fn the_open_menu_looks_right() {
    let mut h = harness();
    h.hover_at(O);
    h.step();
    button(&mut h, Side::Right, true);
    h.run_steps(10);
    h.hover_at(O - vec2(0.0, 120.0));
    h.step();
    h.snapshot("pie_right_ring");
}

#[test]
fn long_names_show_in_full() {
    let mut h = harness();
    h.state_mut().long = true;
    h.hover_at(O);
    h.step();
    button(&mut h, Side::Left, true);
    h.run_steps(10);
    h.hover_at(O + vec2(0.0, 30.0));
    h.step();
    let (center, outer) = h.state().pie.radii().expect("open");
    // Compact: the ring grows only a little past the style's for names this long (it took 237 before SPM-8).
    assert!(outer <= 190.0, "the ring grew to {outer}");
    let menu = long_menu();
    let far = |r: egui::Rect| {
        [
            r.left_top(),
            r.right_top(),
            r.left_bottom(),
            r.right_bottom(),
        ]
        .map(|q| (q - O).length())
        .into_iter()
        .fold(0.0, f32::max)
    };
    let near = |r: egui::Rect| (O.clamp(r.min, r.max) - O).length();
    let mut rects = Vec::new();
    for slot in menu.ring_left.iter().flatten() {
        let r = h.get_by_label(&slot.label).rect();
        // An icon over at most two rows.
        assert!(
            r.height() <= 56.0,
            "{:?} takes more than two rows: {r:?}",
            slot.label
        );
        assert!(
            near(r) >= center && far(r) <= outer,
            "{:?} leaves the ring: {r:?}",
            slot.label
        );
        rects.push((slot.label.clone(), r));
    }
    // The centre's names are rounder than their boxes (shorter rows top and bottom): their edges' middles.
    let edges = |r: egui::Rect| {
        [
            r.center_top(),
            r.center_bottom(),
            r.left_center(),
            r.right_center(),
        ]
        .map(|q| (q - O).length())
        .into_iter()
        .fold(0.0, f32::max)
    };
    for slot in [&menu.center_left, &menu.center_right]
        .into_iter()
        .flatten()
    {
        let r = h.get_by_label(&slot.label).rect();
        assert!(
            edges(r) <= center,
            "{:?} leaves the centre: {r:?}",
            slot.label
        );
        rects.push((slot.label.clone(), r));
    }
    for (i, (a, ra)) in rects.iter().enumerate() {
        for (b, rb) in &rects[i + 1..] {
            assert!(!ra.intersects(*rb), "{a:?} overlaps {b:?}");
        }
    }
    h.snapshot("pie_long_names");
}
