//! The side panel from the keyboard (CHR-6): where the focus goes when the panel changes form, the Tab order
//! of its header, a disabled row as a Tab stop, the resize handle as a slider, ↑ ↓ and Enter on rows, a hidden
//! rail coming back as a rail, a floating panel's resize, a grab off the edge, and a stored `NaN` width.

use egui::accesskit::Role;
use egui::{Key, Modifiers, Theme, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{By, NodeT, Queryable};
use ve_theme::Density;
use ve_theme::gallery::PanelGallery;
use ve_theme::panel::{Shown, Side};

use crate::panel::{LOOKS, each_look, harness, settle};

/// The label of the widget with the keyboard focus.
fn focused(h: &Harness<'_, PanelGallery>) -> Option<String> {
    h.query_all(By::new().predicate(|n| n.is_focused()))
        .next()
        .and_then(|n| n.accesskit_node().label())
}

fn press(h: &mut Harness<'_, PanelGallery>, mods: Modifiers, key: Key) {
    h.key_press_modifiers(mods, key);
    settle(h);
}

fn one_panel() -> PanelGallery {
    let mut g = PanelGallery::default();
    g.states.shown = Shown::Hidden;
    g
}

/// Ctrl+Shift+B from the rail hides the panel; pressed again, or the edge tab, brings back the rail.
#[test]
fn a_hidden_rail_comes_back_as_a_rail() {
    each_look(|theme, density| {
        let mut h = harness(theme, density, PanelGallery::default());
        press(&mut h, Modifiers::COMMAND, Key::B);
        assert_eq!(h.state().tracks.shown, Shown::Rail);
        press(&mut h, Modifiers::COMMAND | Modifiers::SHIFT, Key::B);
        assert_eq!(h.state().tracks.shown, Shown::Hidden);
        press(&mut h, Modifiers::COMMAND | Modifiers::SHIFT, Key::B);
        assert_eq!(h.state().tracks.shown, Shown::Rail, "{theme:?} {density:?}");
        press(&mut h, Modifiers::COMMAND | Modifiers::SHIFT, Key::B);
        h.get_by_label("Show Tracks").click();
        settle(&mut h);
        assert_eq!(h.state().tracks.shown, Shown::Rail, "the edge tab too");
    });
}

/// With the focus in the panel, Ctrl+B moves it to the rail's open button and back to the filter; hiding
/// moves it to the edge tab; the filter's clear button gives it back to the field.
#[test]
fn the_focus_follows_the_panel() {
    each_look(|theme, density| {
        let mut h = harness(theme, density, one_panel());
        press(&mut h, Modifiers::NONE, Key::Slash);
        assert_eq!(focused(&h).as_deref(), Some("Filter tracks"));
        press(&mut h, Modifiers::COMMAND, Key::B);
        assert_eq!(focused(&h).as_deref(), Some("Open Tracks"), "{theme:?} {density:?}");
        press(&mut h, Modifiers::COMMAND, Key::B);
        assert_eq!(focused(&h).as_deref(), Some("Filter tracks"));
        press(&mut h, Modifiers::COMMAND | Modifiers::SHIFT, Key::B);
        assert_eq!(focused(&h).as_deref(), Some("Show Tracks"));
        press(&mut h, Modifiers::NONE, Key::Enter);
        assert_eq!(h.state().tracks.shown, Shown::Open);
        assert_eq!(focused(&h).as_deref(), Some("Filter tracks"));

        h.get_by_label("Filter tracks").type_text("ph");
        settle(&mut h);
        h.get_by_label("Clear the filter").click();
        settle(&mut h);
        assert!(h.state().tracks.filter.is_empty());
        assert_eq!(focused(&h).as_deref(), Some("Filter tracks"), "after clear");
    });
}

/// Ctrl+B with the focus outside the panel leaves the focus where it is.
#[test]
fn ctrl_b_never_takes_the_focus_from_the_work() {
    let mut h = harness(Theme::Dark, Density::Comfortable, one_panel());
    press(&mut h, Modifiers::COMMAND, Key::B);
    assert_eq!(h.state().tracks.shown, Shown::Rail);
    assert_eq!(focused(&h), None);
}

/// The labels Tab reaches, in order, from the start of the page.
fn tab_order(h: &mut Harness<'_, PanelGallery>, presses: usize) -> Vec<String> {
    let mut seen = Vec::new();
    for _ in 0..presses {
        press(h, Modifiers::NONE, Key::Tab);
        if let Some(l) = focused(h) {
            seen.push(l);
        }
    }
    seen
}

/// Tab goes through the header left to right (the app's action, ⋯, collapse), reaches a disabled row, whose
/// reason then shows, and the resize handle is a slider holding the width.
#[test]
fn tab_reaches_the_header_in_screen_order_and_disabled_rows() {
    let mut h = harness(Theme::Dark, Density::Comfortable, one_panel());
    let order = tab_order(&mut h, 60);
    let at = |label: &str| {
        order
            .iter()
            .position(|l| l == label)
            .unwrap_or_else(|| panic!("{label} not reached: {order:?}"))
    };
    assert!(at("+") < at("Tracks menu"), "{order:?}");
    assert!(at("Tracks menu") < at("Collapse Tracks"), "{order:?}");
    at("disabled: offline");

    let mut h = harness(Theme::Dark, Density::Comfortable, one_panel());
    while focused(&h).as_deref() != Some("disabled: offline") {
        press(&mut h, Modifiers::NONE, Key::Tab);
    }
    h.get_by_label_contains("Plug it in or press Rescan (F5)");

    let handle = h.get_by_label("Resize Tracks");
    assert_eq!(handle.accesskit_node().role(), Role::Slider);
    assert_eq!(handle.accesskit_node().numeric_value(), Some(264.0));
}

/// ← → on the focused handle step the width by 16.
#[test]
fn arrows_on_the_focused_handle_resize() {
    let mut h = harness(Theme::Dark, Density::Comfortable, one_panel());
    let mut n = 0;
    while focused(&h).as_deref() != Some("Resize Tracks") {
        press(&mut h, Modifiers::NONE, Key::Tab);
        n += 1;
        assert!(n < 80, "the handle is a Tab stop");
    }
    press(&mut h, Modifiers::NONE, Key::ArrowRight);
    assert_eq!(h.state().tracks.width, Some(280.0));
    press(&mut h, Modifiers::NONE, Key::ArrowLeft);
    press(&mut h, Modifiers::NONE, Key::ArrowLeft);
    assert_eq!(h.state().tracks.width, Some(248.0));
    assert_eq!(focused(&h).as_deref(), Some("Resize Tracks"), "the arrows keep the focus");
}

/// ↓ and ↑ move the focus between rows; Enter picks the focused row.
#[test]
fn arrows_move_between_rows_and_enter_picks() {
    each_look(|theme, density| {
        let mut h = harness(theme, density, one_panel());
        h.get_by_label("can0").focus();
        settle(&mut h);
        assert_eq!(focused(&h).as_deref(), Some("can0"));
        press(&mut h, Modifiers::NONE, Key::ArrowDown);
        let below = focused(&h).expect("a row below has the focus");
        assert_ne!(below, "can0", "{theme:?} {density:?}");
        press(&mut h, Modifiers::NONE, Key::ArrowUp);
        assert_eq!(focused(&h).as_deref(), Some("can0"));
        press(&mut h, Modifiers::NONE, Key::ArrowDown);
        press(&mut h, Modifiers::NONE, Key::Enter);
        assert_eq!(h.state().selected, below, "Enter picks the focused row");
    });
}

fn drag(h: &mut Harness<'_, PanelGallery>, from: egui::Pos2, by: f32) {
    h.hover_at(from);
    h.step();
    h.drag_at(from);
    h.step();
    h.hover_at(from + vec2(by, 0.0));
    h.step();
    h.drop_at(from + vec2(by, 0.0));
    settle(h);
}

/// A grab a few points off the edge moves the edge by exactly the drag.
#[test]
fn a_grab_off_the_edge_keeps_its_distance() {
    for (theme, density) in LOOKS {
        for off in [-3.0_f32, 3.0] {
            let mut h = harness(theme, density, one_panel());
            let edge = h.get_by_label("Resize Tracks").rect().center().x;
            drag(&mut h, egui::pos2(edge + off, 300.0), 40.0);
            assert_eq!(h.state().tracks.width, Some(304.0), "{theme:?} {density:?} {off}");
        }
    }
}

/// A floating panel resizes by its right edge, and stays where it is.
#[test]
fn a_floating_panel_resizes() {
    for side in [Side::Left, Side::Right] {
        let mut g = one_panel();
        g.tracks.floating = true;
        g.tracks.side = side;
        let mut h = harness(Theme::Light, Density::Comfortable, g);
        let left = h.get_by_label("Tracks drag head").rect().left();
        let edge = h.get_by_label("Resize Tracks").rect().center();
        drag(&mut h, edge, 48.0);
        assert_eq!(h.state().tracks.width, Some(312.0), "{side:?}");
        let head = h.get_by_label("Tracks drag head").rect();
        assert_eq!(head.left(), left, "{side:?}: the left edge stays");
        drag(&mut h, h.get_by_label("Resize Tracks").rect().center(), -400.0);
        assert!(h.state().tracks.floating, "no snap to the rail while floating");
        assert_eq!(h.state().tracks.shown, Shown::Open);
    }
}

/// A stored `NaN` width draws the default width.
#[test]
fn a_nan_width_draws_the_default() {
    let mut g = one_panel();
    g.tracks.width = Some(f32::NAN);
    let h = harness(Theme::Dark, Density::Comfortable, g);
    let edge = h.get_by_label("Resize Tracks").rect().center().x;
    assert!((edge - 264.0).abs() <= 1.0, "edge at {edge}");
}
