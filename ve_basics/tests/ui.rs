//! Headless UI tests for ve_basics (BAS-1..3).

use egui::{pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
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
