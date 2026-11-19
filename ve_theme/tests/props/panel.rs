//! The side panel and its rail (CHR-6) driven as a person would, on the gallery's side panel page: filter,
//! fold, resize by keys and by drag, collapse to the rail, dock right, float and back, hide and the edge tab,
//! and a click that reveals a row in the linked view; each in both themes and both densities. Snapshots
//! against frames B1 and B2 of the chrome design (`tests/snapshots/panel_*.png`; after an intended change:
//! `UPDATE_SNAPSHOTS=1 cargo test -p ve_theme --test props panel::`).

use egui::accesskit::Role;
use egui::{Event, Key, Modifiers, MouseWheelUnit, Theme, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use ve_theme::Density;
use ve_theme::gallery::PanelGallery;
use ve_theme::panel::{Follow, PanelForm, Shown, Side, set_density};

pub(crate) const LOOKS: [(Theme, Density); 4] = [
    (Theme::Dark, Density::Comfortable),
    (Theme::Light, Density::Comfortable),
    (Theme::Dark, Density::Compact),
    (Theme::Light, Density::Compact),
];

pub(crate) const WIDTH: f32 = 1200.0;
pub(crate) const HEIGHT: f32 = 640.0;
pub(crate) const WINDOW: f32 = 560.0;

pub(crate) fn harness(
    theme: Theme,
    density: Density,
    gallery: PanelGallery,
) -> Harness<'static, PanelGallery> {
    let mut h = Harness::builder()
        .with_size(vec2(WIDTH, HEIGHT))
        .with_theme(theme)
        .build_ui_state(
            move |ui, g: &mut PanelGallery| g.themed(ui, theme, WINDOW),
            PanelGallery { density, ..gallery },
        );
    ve_theme::setup(&h.ctx);
    set_density(&h.ctx, density);
    settle(&mut h);
    h
}

/// A few frames: enough for a click to land, a popup to open, a scroll to finish.
pub(crate) fn settle(h: &mut Harness<'_, PanelGallery>) {
    for _ in 0..6 {
        h.step();
    }
}

pub(crate) fn each_look(test: impl Fn(Theme, Density)) {
    for (theme, density) in LOOKS {
        test(theme, density);
    }
}

pub(crate) fn shown(h: &Harness<'_, PanelGallery>, label: &str) -> bool {
    h.query_by_label(label).is_some()
}

/// `/` focuses the filter; matches stay with their parents, "Not matching" folds the rest, groups keep their
/// heads with "0 of 2"; Esc clears it.
#[test]
fn filter_marks_keeps_heads_and_folds_the_rest() {
    each_look(|theme, density| {
        let mut h = harness(theme, density, PanelGallery::default());
        h.key_press(Key::Slash);
        settle(&mut h);
        h.get_by_role(Role::TextInput).type_text("ph");
        settle(&mut h);
        assert_eq!(h.state().tracks.filter, "ph");
        assert!(shown(&h, "phase current"));
        assert!(shown(&h, "motor"), "the parent of a match stays");
        assert!(!shown(&h, "vbus"), "a row that does not match folds away");
        h.get_by_label_contains("IMU · own clock, 0 of 2");
        h.get_by_label_contains("Not matching").click();
        settle(&mut h);
        assert!(shown(&h, "vbus"), "nothing vanishes: under Not matching");
        h.get_by_role(Role::TextInput).focus();
        settle(&mut h);
        h.key_press(Key::Escape);
        settle(&mut h);
        assert!(h.state().tracks.filter.is_empty(), "{theme:?} {density:?}");
        assert!(shown(&h, "vbus") && shown(&h, "can0"));
    });
}

/// A group head folds by click; a row folds with ← and opens with → while it has focus, or by its chevron.
#[test]
fn groups_and_rows_fold() {
    each_look(|theme, density| {
        let mut h = harness(theme, density, PanelGallery::default());
        h.get_by_label_contains("dev0 · bench rig").click();
        settle(&mut h);
        assert!(!shown(&h, "can0"));
        h.get_by_label_contains("dev0 · bench rig").click();
        settle(&mut h);
        assert!(shown(&h, "can0"));

        h.get_by_label("motor").focus();
        settle(&mut h);
        h.key_press(Key::ArrowLeft);
        settle(&mut h);
        assert!(!shown(&h, "phase current"), "← folds");
        h.key_press(Key::ArrowRight);
        settle(&mut h);
        assert!(shown(&h, "phase current"), "→ opens");

        assert!(!shown(&h, "rail 5V"), "power starts folded");
        let r = h.get_by_label("power").rect();
        let chevron = egui::pos2(r.min.x + 14.0, r.center().y);
        h.drag_at(chevron);
        h.drop_at(chevron);
        settle(&mut h);
        assert!(shown(&h, "rail 5V"), "the chevron opens it");
        assert_ne!(
            h.state().selected,
            "power",
            "the chevron folds, it does not pick"
        );
    });
}

/// Ctrl+Alt+→ / ← step the width by 16 while focus is in the panel; under the minimum it snaps to the rail.
/// A drag on the edge resizes; a double click resets.
#[test]
fn resize_by_keys_and_drag() {
    each_look(|theme, density| {
        let mut h = harness(theme, density, PanelGallery::default());
        let keys = Modifiers::COMMAND | Modifiers::ALT;
        h.get_by_label("can0").focus();
        settle(&mut h);
        h.key_press_modifiers(keys, Key::ArrowRight);
        settle(&mut h);
        assert_eq!(h.state().tracks.width, Some(280.0));
        h.key_press_modifiers(keys, Key::ArrowLeft);
        h.key_press_modifiers(keys, Key::ArrowLeft);
        settle(&mut h);
        assert_eq!(h.state().tracks.width, Some(248.0));

        let edge = h.get_by_label("Resize Tracks").rect();
        let at = egui::pos2(edge.center().x, 300.0);
        h.drag_at(at);
        h.step();
        h.hover_at(at + vec2(56.0, 0.0));
        h.step();
        h.drop_at(at + vec2(56.0, 0.0));
        settle(&mut h);
        assert_eq!(h.state().tracks.width, Some(304.0));

        double_click_resets(theme, density);

        h.state_mut().tracks.width = Some(208.0);
        settle(&mut h);
        h.get_by_label("can0").focus();
        settle(&mut h);
        h.key_press_modifiers(keys, Key::ArrowLeft);
        settle(&mut h);
        assert_eq!(
            h.state().tracks.shown,
            Shown::Rail,
            "under the minimum it snaps to the rail"
        );
    });
}

/// A double click on the edge resets the width: two clicks 0.1 s apart, each press and release a frame
/// apart (egui decides click or drag at the release).
fn double_click_resets(theme: Theme, density: Density) {
    let mut h = Harness::builder()
        .with_size(vec2(WIDTH, HEIGHT))
        .with_theme(theme)
        .with_step_dt(1.0 / 40.0)
        .build_ui_state(
            move |ui, g: &mut PanelGallery| g.themed(ui, theme, WINDOW),
            PanelGallery::default(),
        );
    ve_theme::setup(&h.ctx);
    set_density(&h.ctx, density);
    h.state_mut().tracks.width = Some(304.0);
    settle(&mut h);
    let at = h.get_by_label("Resize Tracks").rect().center();
    h.hover_at(at);
    h.step();
    for _ in 0..2 {
        h.drag_at(at);
        h.step();
        h.drop_at(at);
        h.step();
    }
    settle(&mut h);
    assert_eq!(h.state().tracks.width, None, "a double click resets");
}

/// Ctrl+B and the « button collapse to the rail; a rail item opens the panel at its group; » opens it.
#[test]
fn collapse_to_rail_and_back() {
    each_look(|theme, density| {
        let mut h = harness(theme, density, PanelGallery::default());
        h.key_press_modifiers(Modifiers::COMMAND, Key::B);
        settle(&mut h);
        assert_eq!(h.state().tracks.shown, Shown::Rail);
        assert!(!shown(&h, "can0"));
        h.get_by_label("Project").click();
        settle(&mut h);
        assert_eq!(h.state().tracks.shown, Shown::Open);
        assert!(shown(&h, "notes"));

        h.get_by_label("Collapse Tracks").click();
        settle(&mut h);
        assert_eq!(h.state().tracks.shown, Shown::Rail);
        h.get_by_label("Open Tracks").click();
        settle(&mut h);
        assert_eq!(h.state().tracks.shown, Shown::Open);
    });
}

pub(crate) fn menu(h: &mut Harness<'_, PanelGallery>, entry: &str) {
    h.get_by_label("Tracks menu").click();
    settle(h);
    h.get_by_label_contains(entry).click();
    settle(h);
}

/// The ⋯ menu docks right (the panel moves to the right edge), floats it as a window, and a drop of its head on
/// the left edge docks it there again.
#[test]
fn dock_right_float_and_back() {
    each_look(|theme, density| {
        let mut h = harness(theme, density, PanelGallery::default());
        menu(&mut h, "Dock right");
        assert_eq!(h.state().tracks.side, Side::Right);
        assert!(h.get_by_label("can0").rect().min.x > WIDTH * 0.5);

        menu(&mut h, "Float");
        assert!(h.state().tracks.floating);
        let head = h.get_by_label("Tracks drag head").rect();
        let at = head.center();
        h.drag_at(at);
        h.step();
        h.hover_at(egui::pos2(300.0, at.y));
        h.step();
        let moved = h.state().tracks.float_at;
        assert!(moved.is_some(), "dragging the head moves the window");
        h.hover_at(egui::pos2(4.0, at.y));
        h.step();
        h.drop_at(egui::pos2(4.0, at.y));
        settle(&mut h);
        assert!(
            !h.state().tracks.floating,
            "dropped on the left edge, it docks"
        );
        assert_eq!(h.state().tracks.side, Side::Left);
        assert!(h.get_by_label("can0").rect().max.x < WIDTH * 0.5);
    });
}

/// Ctrl+Shift+B hides the panel behind a vertical tab on its edge; the tab brings it back.
#[test]
fn hide_and_the_edge_tab() {
    each_look(|theme, density| {
        let mut h = harness(theme, density, PanelGallery::default());
        h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::B);
        settle(&mut h);
        assert_eq!(h.state().tracks.shown, Shown::Hidden);
        assert!(!shown(&h, "can0"));
        h.get_by_label("Show Tracks").click();
        settle(&mut h);
        assert_eq!(h.state().tracks.shown, Shown::Open);
        assert!(shown(&h, "can0"));
        menu(&mut h, "Hide");
        assert_eq!(h.state().tracks.shown, Shown::Hidden);
    });
}

/// A click on a tree row reveals it in the view: scrolled into sight and flashing. A click in the view
/// reveals the row in the tree, unfolding its group. Hover lights both.
#[test]
fn a_click_reveals_the_row_in_the_linked_view() {
    each_look(|theme, density| {
        let mut h = harness(theme, density, PanelGallery::default());
        assert!(
            h.get_by_label("view: markers").rect().min.y > HEIGHT,
            "markers starts below the view's fold"
        );
        h.get_by_label("markers").click();
        h.step();
        h.step();
        assert_eq!(h.state().selected, "markers");
        let ctx = h.ctx.clone();
        let now = ctx.input(|i| i.time);
        assert!(
            h.state().link.flash_at(&"markers", now, 0.8) > 0.0,
            "it flashes"
        );
        settle(&mut h);
        let r = h.get_by_label("view: markers").rect();
        assert!(r.max.y < HEIGHT, "scrolled into the view: {r:?}");

        h.get_by_label_contains("IMU · own clock").click();
        settle(&mut h);
        assert!(!shown(&h, "gyro"));
        h.get_by_label("view: gyro").scroll_to_me();
        settle(&mut h);
        h.get_by_label("view: gyro").click();
        settle(&mut h);
        assert!(shown(&h, "gyro"), "revealed in the tree: its group unfolds");
        assert_eq!(h.state().selected, "gyro");

        h.get_by_label("can0").hover();
        h.step();
        h.step();
        assert_eq!(
            h.state().link.hovered(&ctx),
            Some(&"can0"),
            "hover reaches the view"
        );
    });
}

/// The tree follows the view (S28): the view scrolled to its end, the tree keeps the view's top row in sight.
#[test]
fn tree_follows_the_view() {
    let mut g = PanelGallery::default();
    g.settings.follow = Follow::TreeFollowsView;
    let mut h = harness(Theme::Dark, Density::Comfortable, g);
    let view = h.get_by_label("view: can0").rect();
    h.hover_at(egui::pos2(view.center().x, 300.0));
    h.event(Event::MouseWheel {
        unit: MouseWheelUnit::Point,
        delta: vec2(0.0, -2000.0),
        modifiers: Modifiers::NONE,
        phase: egui::TouchPhase::Move,
    });
    settle(&mut h);
    let r = h.get_by_label("notes").rect();
    assert!(
        r.max.y < HEIGHT && r.min.y > 0.0,
        "the tree scrolled with the view: {r:?}"
    );
}

/// A disabled row is muted and says why; a long label is cut and whole on hover.
#[test]
fn disabled_rows_say_why() {
    let mut h = harness(Theme::Dark, Density::Comfortable, PanelGallery::default());
    let row = h.get_by_label("disabled: offline");
    assert!(row.accesskit_node().is_disabled());
    row.hover();
    for _ in 0..8 {
        h.step();
    }
    h.get_by_label_contains("Plug it in or press Rescan (F5)");
}

/// Form B3: the rail only; a rail item's hover card names it with its summary and key, a click is reported.
#[test]
fn rail_only_has_hover_cards() {
    let mut g = PanelGallery::default();
    g.settings.form = PanelForm::RailOnly;
    let mut h = harness(Theme::Light, Density::Compact, g);
    assert!(!shown(&h, "can0"));
    h.get_by_label("IMU · own clock").hover();
    for _ in 0..8 {
        h.step();
    }
    h.get_by_label("2 sensors");
    h.get_all_by_label("IMU · own clock")
        .next()
        .expect("the rail item")
        .click();
    settle(&mut h);
    assert!(h.state().said.contains("Rail item 2"), "{}", h.state().said);
}

/// Form B1: a rail of panel views beside the list; Bookmarks shows only its group.
#[test]
fn views_rail_switches_the_list() {
    let mut g = PanelGallery::default();
    g.settings.form = PanelForm::RailAndList;
    let mut h = harness(Theme::Dark, Density::Comfortable, g);
    assert!(shown(&h, "can0"));
    h.get_by_label("Bookmarks").click();
    settle(&mut h);
    assert_eq!(h.state().tracks.view, 2);
    assert!(!shown(&h, "can0") && shown(&h, "notes"));
}

// ---------------------------------------------------------------------------------------------------------
// Snapshots

/// The gallery page as shown, settings strip included.
fn bare() -> PanelGallery {
    PanelGallery::default()
}

fn snap(
    h: &mut Harness<'_, PanelGallery>,
    name: &str,
    results: &mut egui_kittest::SnapshotResults,
) {
    h.snapshot(name);
    results.extend(h.take_snapshot_results());
}

/// B2: the tracks tree with its selection, the row states panel with a hovered and a focused row, in both
/// themes and both densities.
#[test]
fn b2_snapshots() {
    let mut results = egui_kittest::SnapshotResults::new();
    for (theme, density) in LOOKS {
        let mut h = harness(theme, density, bare());
        h.get_by_label("focused (keyboard)").focus();
        h.get_by_label("hovered").hover();
        settle(&mut h);
        let t = if theme == Theme::Dark {
            "dark"
        } else {
            "light"
        };
        let d = if density == Density::Compact {
            "compact"
        } else {
            "comfortable"
        };
        snap(&mut h, &format!("panel_b2_{t}_{d}"), &mut results);
    }
}

/// B2 "Filter typed" and "Resize handle" (while dragging), and B1 (views rail and list) and B3 (rail only),
/// dark and light.
#[test]
fn b1_b2_state_snapshots() {
    let mut results = egui_kittest::SnapshotResults::new();
    for (theme, t) in [(Theme::Dark, "dark"), (Theme::Light, "light")] {
        let mut h = harness(theme, Density::Comfortable, bare());
        h.key_press(Key::Slash);
        settle(&mut h);
        h.get_by_role(Role::TextInput).type_text("ph");
        settle(&mut h);
        snap(&mut h, &format!("panel_filter_{t}"), &mut results);

        let mut h = harness(theme, Density::Comfortable, bare());
        let edge = h.get_by_label("Resize Tracks").rect();
        let at = egui::pos2(edge.center().x, 300.0);
        h.drag_at(at);
        h.step();
        h.hover_at(at + vec2(48.0, 0.0));
        settle(&mut h);
        snap(&mut h, &format!("panel_resize_{t}"), &mut results);

        let mut g = bare();
        g.settings.form = PanelForm::RailAndList;
        let mut h = harness(theme, Density::Comfortable, g);
        snap(&mut h, &format!("panel_b1_{t}"), &mut results);

        let mut g = bare();
        g.settings.form = PanelForm::RailOnly;
        let mut h = harness(theme, Density::Comfortable, g);
        h.get_by_label("dev0 · bench rig").hover();
        for _ in 0..8 {
            h.step();
        }
        snap(&mut h, &format!("panel_b3_{t}"), &mut results);
    }
}

/// S29 dot lights (`Tokens::chrome.lights` app-wide, overridden by the tracks panel): round dots.
#[test]
fn dot_lights_snapshot() {
    let mut results = egui_kittest::SnapshotResults::new();
    let mut g = bare();
    g.settings.lights = Some(ve_theme::LightStyle::Dot);
    let mut h = harness(Theme::Light, Density::Comfortable, g);
    snap(&mut h, "panel_dot_light", &mut results);
}

/// B3: the tracks panel floating over the work, the devices panel hidden behind its edge tab.
#[test]
fn float_and_hidden_snapshot() {
    let mut results = egui_kittest::SnapshotResults::new();
    let mut g = bare();
    g.tracks.floating = true;
    g.states.shown = Shown::Hidden;
    let mut h = harness(Theme::Dark, Density::Comfortable, g);
    snap(&mut h, "panel_float_hidden_dark", &mut results);
}

/// The floating panel and its shadow in light at 1x and 2x, and B2 dark at 2x: corners, pills, dots and the
/// soft shadow at both scales.
#[test]
fn scale_snapshots() {
    let mut results = egui_kittest::SnapshotResults::new();
    for (ppp, s) in [(1.0, "1x"), (2.0, "2x")] {
        let mut g = bare();
        g.tracks.floating = true;
        g.states.shown = Shown::Hidden;
        g.settings.lights = Some(ve_theme::LightStyle::Dot);
        let mut h = harness_at(Theme::Light, ppp, g);
        snap(&mut h, &format!("panel_float_light_{s}"), &mut results);
    }
    let mut h = harness_at(Theme::Dark, 2.0, bare());
    snap(&mut h, "panel_b2_dark_2x", &mut results);
}

fn harness_at(theme: Theme, ppp: f32, g: PanelGallery) -> Harness<'static, PanelGallery> {
    let mut h = Harness::builder()
        .with_size(vec2(WIDTH, HEIGHT))
        .with_pixels_per_point(ppp)
        .with_theme(theme)
        .build_ui_state(
            move |ui, g: &mut PanelGallery| g.themed(ui, theme, WINDOW),
            g,
        );
    ve_theme::setup(&h.ctx);
    settle(&mut h);
    h
}

fn short(theme: Theme, g: PanelGallery) -> Harness<'static, PanelGallery> {
    let mut h = Harness::builder()
        .with_size(vec2(WIDTH, 380.0))
        .with_theme(theme)
        .build_ui_state(
            move |ui, g: &mut PanelGallery| g.themed(ui, theme, 320.0),
            g,
        );
    ve_theme::setup(&h.ctx);
    settle(&mut h);
    h
}

/// The tree scrolled to its middle: egui's scroll fade at the list's top and bottom, toward the panel's own
/// ground, docked and floating, dark and light.
#[test]
fn fade_snapshots() {
    let mut results = egui_kittest::SnapshotResults::new();
    for (theme, t) in [(Theme::Dark, "dark"), (Theme::Light, "light")] {
        for floating in [false, true] {
            let mut g = bare();
            g.tracks.floating = floating;
            g.states.shown = Shown::Hidden;
            let mut h = short(theme, g);
            let at = h.get_by_label("can0").rect().center();
            h.hover_at(at);
            h.event(Event::MouseWheel {
                unit: MouseWheelUnit::Point,
                delta: vec2(0.0, -120.0),
                modifiers: Modifiers::NONE,
                phase: egui::TouchPhase::Move,
            });
            settle(&mut h);
            h.hover_at(egui::pos2(WIDTH - 4.0, 4.0));
            settle(&mut h);
            let place = if floating { "float" } else { "docked" };
            snap(&mut h, &format!("panel_fade_{place}_{t}"), &mut results);
        }
    }
}

/// A row revealed from the view flashes in the tree: the picture one kittest step (its fixed clock) after the
/// click, the flash partway through its fade.
#[test]
fn flash_snapshot() {
    let mut results = egui_kittest::SnapshotResults::new();
    for (theme, t) in [(Theme::Dark, "dark"), (Theme::Light, "light")] {
        let mut h = harness(theme, Density::Comfortable, bare());
        h.get_by_label("view: vbus").click();
        // The click lands, the tree takes the reveal on the next frame, one more step ages the flash.
        h.step();
        h.step();
        h.step();
        let ctx = h.ctx.clone();
        let now = ctx.input(|i| i.time);
        let f = h.state().link.flash_at(&"vbus", now, 0.8);
        assert!(f > 0.0 && f < 1.0, "partway through the flash: {f}");
        snap(&mut h, &format!("panel_flash_{t}"), &mut results);
    }
}
