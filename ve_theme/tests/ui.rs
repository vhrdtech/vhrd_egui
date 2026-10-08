//! Headless UI tests for ve_theme through egui_kittest (THM-5).
//!
//! The gallery renders through wgpu and is pixel-compared against `tests/snapshots/gallery_dark.png` and
//! `gallery_light.png`; AccessKit queries check that the helpers are labelled and carry their tooltips.
//! After an intended visual change: `UPDATE_SNAPSHOTS=1 cargo test -p ve_theme --test ui` and commit the PNGs.

use egui::{Theme, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use ve_theme::gallery::Gallery;
use ve_theme::{Tone, UiExt};

fn gallery_snapshot(theme: Theme, name: &str) {
    let mut gallery = Gallery::default();
    let mut h = Harness::builder()
        .with_size(vec2(520.0, 900.0))
        .with_theme(theme)
        .build_ui(move |ui| {
            // Cover the whole backbuffer so the snapshot shows the theme's real background.
            let bg = ve_theme::Tokens::of(ui.ctx(), theme).colors.bg;
            ui.painter().rect_filled(ui.ctx().viewport_rect(), 0.0, bg);
            gallery.themed(ui, theme);
        });
    ve_theme::setup(&h.ctx);
    h.fit_contents();
    h.run();
    h.snapshot(name);
}

#[test]
fn gallery_dark_snapshot() {
    gallery_snapshot(Theme::Dark, "gallery_dark");
}

#[test]
fn gallery_light_snapshot() {
    gallery_snapshot(Theme::Light, "gallery_light");
}

/// `setup` installs styles egui switches between with the system theme.
#[test]
fn setup_follows_theme() {
    let mut h = Harness::builder().with_theme(Theme::Light).build_ui(|ui| {
        ui.label("x");
    });
    ve_theme::setup(&h.ctx);
    h.run();
    let light = ve_theme::Tokens::light();
    assert_eq!(h.ctx.global_style().visuals.panel_fill, light.colors.bg);
    h.ctx.set_theme(Theme::Dark);
    h.run();
    assert_eq!(
        h.ctx.global_style().visuals.panel_fill,
        ve_theme::Tokens::dark().colors.bg
    );
}

fn hover_tooltip(h: &mut Harness<'_>, label: &str, tooltip: &str) {
    h.get_by_label(label).hover();
    for _ in 0..8 {
        h.step(); // tooltips show after a short delay
    }
    assert!(
        h.query_by_label(tooltip).is_some(),
        "hovering {label:?} must show {tooltip:?}"
    );
}

#[test]
fn helpers_are_accessible_and_have_tooltips() {
    let cases = [
        ("Save", "the main action"),
        ("Delete", "deletes the node"),
        ("online", "node answers pings"),
        ("Signals", "signal list"),
    ];
    for (label, tooltip) in cases {
        let mut h = Harness::builder()
            .with_size(vec2(400.0, 200.0))
            .build_ui(|ui| {
                ui.primary_button("Save", "the main action");
                ui.danger_button("Delete", "deletes the node");
                ui.badge("online", Tone::Good, "node answers pings");
                ui.section_header("Signals", "signal list");
            });
        ve_theme::setup(&h.ctx);
        h.run();
        hover_tooltip(&mut h, label, tooltip);
    }
}

#[test]
fn buttons_are_buttons_and_click() {
    let clicked = std::rc::Rc::new(std::cell::Cell::new(false));
    let mut h = Harness::new_ui({
        let clicked = clicked.clone();
        move |ui| {
            if ui.primary_button("Save", "the main action").clicked() {
                clicked.set(true);
            }
        }
    });
    ve_theme::setup(&h.ctx);
    h.run();
    let node = h.get_by_label("Save");
    assert_eq!(node.accesskit_node().role(), egui::accesskit::Role::Button);
    node.click();
    h.run();
    assert!(clicked.get());
}

#[test]
fn panel_title_bar_returns_right_side_and_title() {
    let mut h = Harness::new_ui(|ui| {
        let r = ui.panel_title_bar("Nodes", "all mesh nodes", |ui| {
            ui.button("Add").on_hover_text("add a node")
        });
        assert!(r.response.rect.width() > 0.0);
    });
    ve_theme::setup(&h.ctx);
    h.run();
    h.get_by_label("Nodes");
    h.get_by_label("Add");
}

/// The number font has tabular digits: every digit is as wide as every other.
#[test]
fn number_font_digits_are_tabular() {
    let widths = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let out = widths.clone();
    let mut h = Harness::new_ui(move |ui| {
        let mut w = out.borrow_mut();
        w.clear();
        for text in ["1111", "8080", "0123"] {
            let font = ve_theme::number_font(ui.ctx(), 13.0);
            w.push(
                ui.painter()
                    .layout_no_wrap(text.into(), font, egui::Color32::WHITE)
                    .size()
                    .x,
            );
        }
    });
    ve_theme::setup(&h.ctx);
    h.run();
    h.run();
    let w = widths.borrow();
    assert!(w[0] > 0.0);
    assert!(
        (w[0] - w[1]).abs() < 0.01 && (w[0] - w[2]).abs() < 0.01,
        "{w:?}"
    );
}
