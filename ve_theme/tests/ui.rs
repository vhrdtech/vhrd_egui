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

/// Every emoji tpm shows for a task or project status, a board or a log kind draws the Noto Emoji glyph, not a
/// box or egui's tag-like pictogram (THM-14).
#[test]
fn tpm_status_emoji_snapshot() {
    let emoji = [
        "✅", "🚧", "🧪", "🐛", "⬜", "📋", "💡", "⛔", "⏳", "🔍", "❌", // task_status
        "🔬", "🛠️", "🏗️", "🚀", "🔧", "⏸️", "🗄️", // project_status
        "🏭", "📐", // board_status
        "💻", "⚖️", "ℹ️", "📝", "👍", "🆕", "🗣️", // log_kinds
    ];
    let mut h = Harness::builder()
        .with_size(vec2(360.0, 200.0))
        .with_theme(Theme::Dark)
        .build_ui(move |ui| {
            let bg = ve_theme::Tokens::of(ui.ctx(), Theme::Dark).colors.bg;
            ui.painter().rect_filled(ui.ctx().viewport_rect(), 0.0, bg);
            ui.horizontal_wrapped(|ui| {
                for e in emoji {
                    ui.label(egui::RichText::new(e).size(28.0));
                }
            });
        });
    ve_theme::setup(&h.ctx);
    h.fit_contents();
    h.run();
    h.snapshot("tpm_status_emoji");
}

/// Every tpm status tinted by its tone (THM-15): the same glyphs as `tpm_status_emoji_snapshot`, now in
/// colour. `status_icon_with` adds nothing egui's text painting didn't already do: the glyph takes the
/// colour it is drawn with, the same as any other character (no colour-glyph font support to lose).
#[test]
fn tpm_status_color_snapshot() {
    let mut h = Harness::builder()
        .with_size(vec2(420.0, 420.0))
        .with_theme(Theme::Dark)
        .build_ui(move |ui| {
            let bg = ve_theme::Tokens::of(ui.ctx(), Theme::Dark).colors.bg;
            ui.painter().rect_filled(ui.ctx().viewport_rect(), 0.0, bg);
            egui::Grid::new("status_colors")
                .spacing(vec2(12.0, 6.0))
                .show(ui, |ui| {
                    for s in ve_theme::STATUS_GLYPHS {
                        ve_theme::status_icon_with(
                            ui,
                            s.glyph,
                            s.tone,
                            format!("{} ({:?})", s.name, s.tone),
                        );
                        ui.label(s.name);
                        ui.end_row();
                    }
                });
        });
    ve_theme::setup(&h.ctx);
    h.fit_contents();
    h.run();
    h.snapshot("tpm_status_color");
}

/// `status_icon` looks a tpm status up by name; a name outside `STATUS_GLYPHS` still shows the bare word
/// (neutral tone) rather than disappearing, and every icon carries its tooltip.
#[test]
fn status_icon_shows_tooltip_and_falls_back_for_unknown_names() {
    let mut h = Harness::builder()
        .with_size(vec2(300.0, 120.0))
        .build_ui(|ui| {
            ve_theme::status_icon(ui, "done", "finished, believed correct");
            ve_theme::status_icon(ui, "not-a-tpm-status", "an app's own status");
        });
    ve_theme::setup(&h.ctx);
    h.run();
    hover_tooltip(&mut h, "✅", "finished, believed correct");
    h.get_by_label("not-a-tpm-status");
}

/// Every status glyph stands out from the background it is drawn on, in both themes (THM-16): the neutral
/// ones (📋 planned, 💡 idea, ⬜ stub: most of a task list) were painted in `line_strong`, a hairline colour,
/// and read as no colour at all. Checked as a person would see it, on the colour the frame painted.
#[test]
fn status_glyphs_stand_out_in_both_themes() {
    for theme in [Theme::Dark, Theme::Light] {
        let mut h = Harness::builder()
            .with_size(vec2(200.0, 80.0))
            .with_theme(theme)
            .build_ui(|ui| {
                ve_theme::status_icon(ui, "planned", "planned");
                ve_theme::status_icon(ui, "done", "done");
            });
        ve_theme::setup(&h.ctx);
        h.run();
        let t = ve_theme::Tokens::of(&h.ctx, theme);
        let painted: Vec<(char, egui::Color32)> = h
            .output()
            .shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::Text(s) => Some((
                    s.galley.job.text.chars().next()?,
                    s.galley.job.sections[0].format.color,
                )),
                _ => None,
            })
            .collect();
        let planned = painted
            .iter()
            .find(|(c, _)| *c == '📋')
            .expect("📋 painted")
            .1;
        assert_eq!(planned, Tone::Neutral.ink(&t));
        assert_eq!(
            planned, t.colors.text_muted,
            "neutral ink is the muted text"
        );
        assert!(
            ve_theme::contrast::contrast(planned, t.colors.bg) >= 3.0,
            "{theme:?}: planned at {:.1}:1",
            ve_theme::contrast::contrast(planned, t.colors.bg)
        );
        let done = painted
            .iter()
            .find(|(c, _)| *c == '✅')
            .expect("✅ painted")
            .1;
        assert_eq!(done, Tone::Good.color(&t));
    }
}
