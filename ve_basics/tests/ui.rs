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
