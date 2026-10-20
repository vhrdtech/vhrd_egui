//! Headless UI tests for ve_basics (BAS-1..3).

use egui::{pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use std::cell::Cell;
use std::rc::Rc;

use ve_basics::{BuildInfo, build_info_label, copyable_label, hover_link, setup_labels};

fn info(debug: bool) -> BuildInfo {
    BuildInfo {
        name: "demo",
        version: "1.2.3",
        git_sha: "abc1234-dirty",
        build_time: "8 Oct 2026 10:00 UTC",
        debug,
    }
}

#[test]
fn build_info_macro_captures_the_callers_crate() {
    let i = ve_basics::build_info!();
    assert_eq!(i.name, "ve_basics");
    assert_eq!(i.debug, cfg!(debug_assertions));
    assert!(!i.git_sha.is_empty());
}

#[test]
fn build_info_label_shows_marker_and_spells_out_on_hover() {
    let mut h = Harness::builder()
        .with_size(vec2(300.0, 100.0))
        .build_ui(|ui| {
            build_info_label(ui, &info(true));
        });
    h.run();
    h.get_by_label("1.2.3 · abc1234-dirty · debug");
    h.hover_at(pos2(30.0, 10.0));
    for _ in 0..8 {
        h.step();
    }
    let tip = h.get_by_label_contains("built: 8 Oct 2026 10:00 UTC");
    assert!(format!("{tip:?}").contains("debug (unoptimized"));
}

#[test]
fn release_marker() {
    assert_eq!(info(false).short(), "1.2.3 · abc1234-dirty · release");
}

#[test]
fn labels_not_selectable_after_setup_but_copyable_is() {
    let mut h = Harness::new_ui(|ui| {
        setup_labels(ui.ctx());
        let plain = ui.label("plain");
        let copy = copyable_label(ui, "/etc/hosts");
        assert!(!plain.sense.contains(egui::Sense::click_and_drag()));
        assert!(copy.sense.interactive());
    });
    h.run();
    assert!(!h.ctx.global_style().interaction.selectable_labels);
}

#[test]
fn hover_highlights_every_item_with_the_key_and_clears() {
    let lit = Rc::new(Cell::new((false, false, false)));
    let mut h = Harness::builder().with_size(vec2(300.0, 100.0)).build_ui({
        let lit = lit.clone();
        move |ui| {
            ui.horizontal(|ui| {
                let a = ui.button("a");
                let b = ui.button("b");
                let c = ui.button("c");
                lit.set((
                    hover_link(ui, "k", &a),
                    hover_link(ui, "k", &b),
                    hover_link(ui, "other", &c),
                ));
            });
        }
    });
    h.run();
    assert_eq!(lit.get(), (false, false, false));
    let a = h.get_by_label("a").rect();
    h.hover_at(a.center());
    h.run();
    assert_eq!(
        lit.get(),
        (true, true, false),
        "b follows a, c has another key"
    );
    h.hover_at(pos2(290.0, 90.0));
    h.run();
    assert_eq!(
        lit.get(),
        (false, false, false),
        "highlight clears after leaving"
    );
}

/// BAS-4: the badge opens the menu with the build in full; the reset asks first, Cancel keeps everything,
/// Reset reports the action once.
#[test]
fn app_menu_opens_from_the_badge_and_confirms_the_reset() {
    use ve_basics::{AppMenu, AppMenuAction, BADGE_HOVER};
    let got = Rc::new(Cell::new(0u32));
    let mut h = Harness::builder().with_size(vec2(500.0, 400.0)).build_ui({
        let got = got.clone();
        move |ui| {
            let i = info(false);
            let menu = AppMenu::new(&i)
                .title("Demo app")
                .description("Shows the menu")
                .resets("the layout");
            if menu.badge(ui).action == Some(AppMenuAction::ResetStorage) {
                got.set(got.get() + 1);
            }
        }
    });
    h.run();
    assert!(h.query_by_label("Reset storage to defaults…").is_none());
    h.get_by_label(BADGE_HOVER).click();
    h.run();
    h.get_by_label("Demo app");
    h.get_by_label("Shows the menu");
    h.get_by_label("abc1234-dirty");
    h.get_by_label("8 Oct 2026 10:00 UTC");
    h.get_by_label("release");
    h.get_by_label("Reset storage to defaults…").click();
    h.run();
    assert!(h.query_by_label("Demo app").is_none(), "the menu closes");
    h.get_by_label_contains("forgets the layout");
    h.get_by_label("Cancel").click();
    h.run();
    assert!(h.query_by_label_contains("forgets the layout").is_none());
    assert_eq!(got.get(), 0, "Cancel resets nothing");

    h.get_by_label(BADGE_HOVER).click();
    h.run();
    h.get_by_label("Reset storage to defaults…").click();
    h.run();
    h.get_by_label("Reset").click();
    h.run();
    assert_eq!(got.get(), 1);
    assert!(h.query_by_label_contains("forgets the layout").is_none());
    h.run();
    assert_eq!(got.get(), 1, "reported once");
}

#[test]
fn app_menu_snapshot() {
    use ve_basics::{AppMenu, BADGE_HOVER};
    let mut h = Harness::builder()
        .with_size(vec2(360.0, 260.0))
        .build_ui(|ui| {
            let i = info(true);
            AppMenu::new(&i)
                .title("Demo app")
                .description("Shows the menu")
                .badge(ui);
        });
    h.run();
    h.get_by_label(BADGE_HOVER).click();
    h.run();
    h.snapshot("app_menu");
}

#[test]
fn reset_egui_memory_keeps_the_style() {
    let ctx = egui::Context::default();
    ctx.style_mut_of(egui::Theme::Dark, |s| s.spacing.item_spacing.x = 42.0);
    ctx.data_mut(|d| d.insert_persisted(egui::Id::new("k"), 7u32));
    ve_basics::reset_egui_memory(&ctx);
    assert_eq!(
        ctx.data_mut(|d| d.get_persisted::<u32>(egui::Id::new("k"))),
        None
    );
    assert_eq!(ctx.style_of(egui::Theme::Dark).spacing.item_spacing.x, 42.0);
}

/// BAS-5: the rest of a cut list sits under a fold: collapsed at first (header only, labelled with its
/// count), every item after a click, folded again after another; the open state persists in egui memory.
#[test]
fn fold_rest_starts_folded_and_opens_to_every_item() {
    use ve_basics::fold_rest;
    let mut h = Harness::builder()
        .with_size(vec2(300.0, 200.0))
        .build_ui(|ui| {
            ui.label("busy-1");
            let (header, _) = fold_rest(ui, "rest", "3 more subagents, not busy", |ui| {
                for name in ["idle-a", "idle-b", "idle-c"] {
                    ui.label(name);
                }
            });
            header.on_hover_text("idle-a, idle-b, idle-c");
        });
    h.run();
    let header = h.get_by_label("3 more subagents, not busy");
    assert_eq!(
        header.accesskit_node().role(),
        egui::accesskit::Role::Button,
        "a clickable header"
    );
    for name in ["idle-a", "idle-b", "idle-c"] {
        assert!(h.query_by_label(name).is_none(), "{name} folded at first");
    }

    h.get_by_label("3 more subagents, not busy").click();
    h.run();
    for name in ["idle-a", "idle-b", "idle-c"] {
        h.get_by_label(name);
    }
    h.get_by_label("3 more subagents, not busy");

    h.get_by_label("3 more subagents, not busy").click();
    h.run();
    for name in ["idle-a", "idle-b", "idle-c"] {
        assert!(h.query_by_label(name).is_none(), "{name} folded again");
    }
}

/// BAS-5: the header and the body drawn apart (the caller's row layout), `default_open` and `is_open`.
#[test]
fn fold_rest_split_header_and_default_open() {
    use ve_basics::FoldRest;
    let open = Rc::new(Cell::new(false));
    let mut h = Harness::builder().with_size(vec2(300.0, 200.0)).build_ui({
        let open = open.clone();
        move |ui| {
            let mut fold = FoldRest::new(ui, "split").default_open(ui, true);
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                fold.header(ui, "2 more");
            });
            open.set(fold.is_open());
            fold.body(ui, |ui| ui.label("shown"));
        }
    });
    h.run();
    assert!(open.get());
    h.get_by_label("shown");
    h.get_by_label("2 more").click();
    h.run();
    assert!(!open.get());
    assert!(h.query_by_label("shown").is_none());
}

/// BAS-5: the fold folded and open, one picture each.
#[test]
fn fold_rest_snapshots() {
    use ve_basics::fold_rest;
    let body = |ui: &mut egui::Ui| {
        ui.label("busy-1 · working");
        fold_rest(ui, "rest", "3 more subagents, not busy", |ui| {
            for name in ["idle-a · done", "idle-b · done", "idle-c · failed"] {
                ui.label(name);
            }
        });
        ui.label("next row");
    };
    let mut h = Harness::builder()
        .with_size(vec2(260.0, 120.0))
        .build_ui(body);
    h.run();
    h.snapshot("fold_rest_folded");
    h.get_by_label("3 more subagents, not busy").click();
    h.run();
    h.snapshot("fold_rest_open");
}

/// BAS-5: in a grid the header stands alone and the caller draws the rest while `is_open`; the header
/// stores the state itself, so it holds without `body`.
#[test]
fn fold_rest_header_alone_in_a_grid() {
    use ve_basics::FoldRest;
    let mut h = Harness::builder()
        .with_size(vec2(300.0, 200.0))
        .build_ui(|ui| {
            egui::Grid::new("g").show(ui, |ui| {
                ui.label("row-1");
                ui.end_row();
                let mut fold = FoldRest::new(ui, "grid-rest");
                fold.header(ui, "2 more rows");
                ui.end_row();
                if fold.is_open() {
                    ui.label("row-2");
                    ui.end_row();
                    ui.label("row-3");
                    ui.end_row();
                }
            });
        });
    h.run();
    assert!(h.query_by_label("row-2").is_none());
    h.get_by_label("2 more rows").click();
    h.run();
    h.get_by_label("row-2");
    h.get_by_label("row-3");
    h.run();
    h.get_by_label("row-3");
}
