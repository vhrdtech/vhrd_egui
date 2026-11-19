//! The side panel's edges and limits (CHR-6): resize from every part of the handle in both docks, the width
//! limits, snapping to the rail, the floating window's bounds, the keys that bring a panel back, the app-wide
//! light setting with a panel's override, and the view following the tree.

use egui::{Key, Modifiers, Theme, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use ve_theme::gallery::PanelGallery;
use ve_theme::panel::{Shown, Side, set_density};
use ve_theme::{Density, LightStyle};

use crate::panel::{HEIGHT, LOOKS, WIDTH, harness, settle};

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

/// A drag from the outer third, the middle or the inner third of the handle resizes, docked left and right.
#[test]
fn resize_from_every_third_of_the_handle() {
    for (theme, density) in LOOKS {
        for side in [Side::Left, Side::Right] {
            for third in [-3.0_f32, 0.0, 3.0] {
                let mut g = PanelGallery::default();
                g.tracks.side = side;
                g.states.shown = Shown::Hidden;
                let mut h = harness(theme, density, g);
                let edge = h.get_by_label("Resize Tracks").rect();
                let at = egui::pos2(edge.center().x + third, 300.0);
                let grow = if side == Side::Left { 40.0 } else { -40.0 };
                drag(&mut h, at, grow);
                let w = h.state().tracks.width;
                assert!(
                    w.is_some_and(|w| (w - 304.0).abs() <= 3.0),
                    "{theme:?} {density:?} {side:?} third {third}: width {w:?}"
                );
            }
        }
    }
}

fn no_states() -> PanelGallery {
    let mut g = PanelGallery::default();
    g.states.shown = Shown::Hidden;
    g
}

fn handle(h: &Harness<'_, PanelGallery>) -> egui::Pos2 {
    egui::pos2(h.get_by_label("Resize Tracks").rect().center().x, 300.0)
}

/// Dragged far out, the panel stops at half the room beside it (1184 pt here: 592).
#[test]
fn width_stops_at_half_the_room() {
    let mut h = harness(Theme::Dark, Density::Comfortable, no_states());
    let at = handle(&h);
    drag(&mut h, at, 900.0);
    assert_eq!(h.state().tracks.width, Some(592.0));
}

/// Dragged under the minimum, the panel snaps to its rail and keeps the width it had before the drag.
#[test]
fn drag_under_the_minimum_snaps_to_the_rail() {
    let mut h = harness(Theme::Dark, Density::Comfortable, no_states());
    let at = handle(&h);
    drag(&mut h, at, -150.0);
    assert_eq!(h.state().tracks.shown, Shown::Rail);
    assert_eq!(
        h.state().tracks.width,
        Some(264.0),
        "the width from before the drag"
    );
}

/// Docked right, the handle and its keys are mirrored: a drag left and Ctrl+Alt+← widen it.
#[test]
fn docked_right_the_handle_is_mirrored() {
    let mut g = no_states();
    g.tracks.side = Side::Right;
    let mut h = harness(Theme::Light, Density::Compact, g);
    let at = handle(&h);
    drag(&mut h, at, -40.0);
    assert_eq!(h.state().tracks.width, Some(304.0));
    h.get_by_label("can0").focus();
    settle(&mut h);
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::ALT, Key::ArrowLeft);
    settle(&mut h);
    assert_eq!(h.state().tracks.width, Some(320.0));
}

/// A floating panel stays in the window: a stored place far outside is pulled in, so is one left outside when
/// the window shrinks, and a drag back moves it at once (no dead zone).
#[test]
fn floating_stays_in_the_window() {
    let mut g = no_states();
    g.tracks.floating = true;
    g.tracks.float_at = Some([99_999.0, -99_999.0]);
    let mut h = harness(Theme::Dark, Density::Comfortable, g);
    let at = h.state().tracks.float_at.expect("kept");
    assert!(at[0] <= WIDTH && at[1] >= 0.0, "pulled in: {at:?}");
    let head = h.get_by_label("Tracks drag head").rect();
    assert!(head.max.x <= WIDTH && head.min.y >= 0.0, "{head:?}");

    let from = head.center();
    h.drag_at(from);
    h.step();
    h.hover_at(from + vec2(0.0, 4000.0));
    h.step();
    h.drop_at(from + vec2(0.0, 4000.0));
    settle(&mut h);
    let low = h.state().tracks.float_at.expect("kept")[1];
    assert!(low < HEIGHT, "clamped while dragged: {low}");
    let head = h.get_by_label("Tracks drag head").rect().center();
    h.drag_at(head);
    h.step();
    h.hover_at(head - vec2(0.0, 50.0));
    h.step();
    h.drop_at(head - vec2(0.0, 50.0));
    settle(&mut h);
    let back = h.state().tracks.float_at.expect("kept")[1];
    assert!(
        back < low - 40.0,
        "a drag back moves it at once: {low} to {back}"
    );

    h.set_size(vec2(500.0, 400.0));
    settle(&mut h);
    let at = h.state().tracks.float_at.expect("kept");
    assert!(
        at[0] <= 500.0 && at[1] <= 400.0,
        "pulled in after the window shrank: {at:?}"
    );
}

/// Dropped on the right edge, a floating panel docks right.
#[test]
fn a_drop_on_the_right_edge_docks_right() {
    let mut g = no_states();
    g.tracks.floating = true;
    let mut h = harness(Theme::Dark, Density::Comfortable, g);
    let head = h.get_by_label("Tracks drag head").rect().center();
    h.drag_at(head);
    h.step();
    h.hover_at(egui::pos2(WIDTH - 4.0, head.y));
    h.step();
    h.drop_at(egui::pos2(WIDTH - 4.0, head.y));
    settle(&mut h);
    assert!(!h.state().tracks.floating);
    assert_eq!(h.state().tracks.side, Side::Right);
}

/// Ctrl+B collapses a floating panel to its docked rail; Ctrl+Shift+B hides a panel and brings it back.
#[test]
fn keys_collapse_a_floating_panel_and_bring_a_hidden_one_back() {
    let mut g = no_states();
    g.tracks.floating = true;
    let mut h = harness(Theme::Dark, Density::Comfortable, g);
    h.key_press_modifiers(Modifiers::COMMAND, Key::B);
    settle(&mut h);
    assert_eq!(h.state().tracks.shown, Shown::Rail);
    assert!(!h.state().tracks.floating, "the rail is docked");
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::B);
    settle(&mut h);
    assert_eq!(h.state().tracks.shown, Shown::Hidden);
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::B);
    settle(&mut h);
    assert_eq!(h.state().tracks.shown, Shown::Open);
}

/// S29: the app-wide `Tokens::chrome.lights` draws what a panel's own override draws, and differs from the
/// default.
#[test]
fn app_wide_lights_with_a_panel_override() {
    let render = |app: LightStyle, panel: Option<LightStyle>| {
        let mut g = no_states();
        g.strip = false;
        g.settings.lights = panel;
        let mut h = harness(Theme::Dark, Density::Comfortable, g);
        for theme in [Theme::Dark, Theme::Light] {
            let mut t = (*ve_theme::Tokens::of(&h.ctx, theme)).clone();
            t.chrome.lights = app;
            t.apply(&h.ctx, theme);
        }
        settle(&mut h);
        h.render().expect("renders")
    };
    let app_dot = render(LightStyle::Dot, None);
    let panel_dot = render(LightStyle::Perforated, Some(LightStyle::Dot));
    let default = render(LightStyle::Perforated, None);
    assert!(
        app_dot == panel_dot,
        "app-wide Dot draws as the panel's Dot"
    );
    assert!(app_dot != default, "and unlike the perforated default");
}

/// Switching density keeps the app's status lights.
#[test]
fn density_keeps_the_lights() {
    let mut h = harness(Theme::Dark, Density::Comfortable, no_states());
    for theme in [Theme::Dark, Theme::Light] {
        let mut t = (*ve_theme::Tokens::of(&h.ctx, theme)).clone();
        t.chrome.lights = LightStyle::Dot;
        t.apply(&h.ctx, theme);
    }
    set_density(&h.ctx, Density::Compact);
    settle(&mut h);
    let t = ve_theme::Tokens::of(&h.ctx, Theme::Dark);
    assert_eq!(t.chrome.lights, LightStyle::Dot);
    assert_eq!(t.chrome.row, 24.0, "the sizes did change");
}

/// A row picked in the view is revealed in a tree scrolled away from it: the tree scrolls it into sight.
#[test]
fn a_reveal_scrolls_the_tree() {
    let mut h = Harness::builder()
        .with_size(vec2(WIDTH, 360.0))
        .build_ui_state(
            |ui, g: &mut PanelGallery| g.themed(ui, Theme::Dark, 300.0),
            no_states(),
        );
    ve_theme::setup(&h.ctx);
    settle(&mut h);
    let markers = |h: &Harness<'_, PanelGallery>| h.query_by_label("markers").map(|n| n.rect());
    assert!(
        markers(&h).is_none_or(|r| r.min.y > 360.0),
        "markers starts out of sight"
    );
    h.get_by_label("view: markers").scroll_to_me();
    settle(&mut h);
    h.get_by_label("view: markers").click();
    settle(&mut h);
    let r = markers(&h).expect("laid out once in sight");
    assert!(
        r.max.y <= 360.0 && r.min.y >= 0.0,
        "scrolled into the tree's sight: {r:?}"
    );
}

/// Two panels in a narrow window: neither runs off it; each falls back to its rail or its edge tab.
#[test]
fn narrow_windows_never_overflow() {
    for width in [320.0, 480.0, 640.0, 216.0] {
        let mut h = Harness::builder()
            .with_size(vec2(width, 480.0))
            .build_ui_state(
                |ui, g: &mut PanelGallery| g.themed(ui, Theme::Dark, 420.0),
                PanelGallery {
                    strip: false,
                    ..PanelGallery::default()
                },
            );
        ve_theme::setup(&h.ctx);
        settle(&mut h);
        let mut found = 0;
        for label in [
            "Tracks menu",
            "Devices menu",
            "Open Tracks",
            "Open Devices",
            "Show Tracks",
            "Show Devices",
        ] {
            if let Some(n) = h.query_by_label(label) {
                found += 1;
                let r = n.rect();
                assert!(
                    r.max.x <= width + 0.5 && r.min.x >= -0.5,
                    "{width}: {label} at {r:?}"
                );
            }
        }
        assert_eq!(found, 2, "{width}: one control per panel reaches it");
    }
}

fn steps<S>(h: &mut Harness<'_, S>) {
    for _ in 0..6 {
        h.step();
    }
}

fn big_list(rows: u32) -> Harness<'static, (ve_theme::panel::SidePanelState, String)> {
    let mut h = Harness::builder()
        .with_size(vec2(WIDTH, HEIGHT))
        .build_ui_state(
            move |ui, (state, filter): &mut (ve_theme::panel::SidePanelState, String)| {
                state.filter.clone_from(filter);
                ve_theme::panel::SidePanel::new("big", "Rows").show(ui, state, |list| {
                    list.group(ve_theme::panel::Group::new("all"), |g| {
                        for i in 0..rows {
                            g.row(ve_theme::panel::Row::new(i, format!("row {i}")));
                        }
                    });
                });
            },
            (ve_theme::panel::SidePanelState::default(), String::new()),
        );
    ve_theme::setup(&h.ctx);
    steps(&mut h);
    h
}

/// Only the rows in sight are laid out and given to accessibility: 100,000 rows expose as many nodes as 100.
#[test]
fn only_rows_in_sight_are_exposed() {
    let count = |h: &Harness<'_, _>| {
        h.query_all_by_role(egui::accesskit::Role::Label).count()
            + h.query_all_by_role(egui::accesskit::Role::Button).count()
            + h.query_all_by_role(egui::accesskit::Role::ListBoxOption)
                .count()
    };
    let small = count(&big_list(100));
    let large = count(&big_list(100_000));
    assert_eq!(small, large, "nodes do not grow with the rows");
    assert!(big_list(100_000).query_by_label("row 99999").is_none());
}

/// Frame time with 100, 10,000 and 100,000 rows, unfiltered and filtered, the app's closure that builds the
/// rows included: `cargo test -p ve_theme --test props panel_cost -- --ignored --nocapture`.
#[test]
#[ignore = "a measurement, printed"]
#[expect(
    clippy::disallowed_methods,
    reason = "a native measurement; never built for the web"
)]
fn panel_cost() {
    for rows in [100, 10_000, 100_000] {
        for filter in ["", "row 5"] {
            let mut h = big_list(rows);
            h.state_mut().1 = filter.to_owned();
            steps(&mut h);
            let frames = 20;
            let t0 = std::time::Instant::now();
            for _ in 0..frames {
                h.step();
            }
            let ms = t0.elapsed().as_secs_f64() * 1000.0 / f64::from(frames);
            println!("PANEL_COST rows {rows:>6} filter {filter:>5?}: {ms:.2} ms per frame");
        }
        // The app's part alone: building the rows the closure gives.
        let t0 = std::time::Instant::now();
        for _ in 0..20 {
            let mut g = ve_theme::panel::Group::new("all");
            for i in 0..rows {
                g.row(ve_theme::panel::Row::new(i, format!("row {i}")));
            }
            std::hint::black_box(&g);
        }
        let ms = t0.elapsed().as_secs_f64() * 1000.0 / 20.0;
        println!("PANEL_COST rows {rows:>6} building the rows alone: {ms:.2} ms");
    }
}
