//! The property-panel widgets (THM-17) driven as a person would: drag a number, type one, cancel with Esc, pick
//! a segment, a disabled segment that says why, a toggle chip, a list row, a sub-pane folding; and the props
//! page of the gallery pixel-compared in dark and light (`tests/snapshots/props_dark.png`, `props_light.png`;
//! after an intended change: `UPDATE_SNAPSHOTS=1 cargo test -p ve_theme --test props`).

use egui::accesskit::Toggled;
use egui::{Theme, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use ve_theme::Tone;
use ve_theme::gallery::PropsGallery;
use ve_theme::props::*;

fn props_snapshot(theme: Theme, name: &str) {
    let mut gallery = PropsGallery::default();
    let mut h = Harness::builder()
        .with_size(vec2(380.0, 1000.0))
        .with_theme(theme)
        .build_ui(move |ui| {
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
fn props_dark_snapshot() {
    props_snapshot(Theme::Dark, "props_dark");
}

#[test]
fn props_light_snapshot() {
    props_snapshot(Theme::Light, "props_light");
}

fn harness<S: 'static>(
    state: S,
    add: impl FnMut(&mut egui::Ui, &mut S) + 'static,
) -> Harness<'static, S> {
    let mut add = add;
    let mut h = Harness::builder()
        .with_size(vec2(360.0, 400.0))
        .build_ui_state(move |ui, s: &mut S| add(ui, s), state);
    ve_theme::setup(&h.ctx);
    h.run();
    h
}

/// Dragging sideways changes the number by `speed` per point, kept to its decimals and clamped to its range
#[test]
fn scalar_field_drags() {
    let mut h = harness(5.0_f64, |ui, v| {
        ui.add(
            ScalarField::new(v)
                .speed(0.5)
                .range(0.0..=20.0)
                .decimals(1)
                .unit("mm"),
        );
    });
    let field = h.get_by_role(egui::accesskit::Role::Slider);
    let at = field.rect().center();
    h.drag_at(at);
    h.run();
    h.hover_at(at + vec2(10.0, 0.0));
    h.run();
    h.drop_at(at + vec2(10.0, 0.0));
    h.run();
    assert!((*h.state() - 10.0).abs() < 1e-9, "{}", h.state());
    // Far past the end: clamped
    let at = h.get_by_role(egui::accesskit::Role::Slider).rect().center();
    h.drag_at(at);
    h.run();
    h.hover_at(at + vec2(200.0, 0.0));
    h.run();
    h.drop_at(at + vec2(200.0, 0.0));
    h.run();
    assert_eq!(*h.state(), 20.0);
}

/// A click opens the number for typing; Enter sets it (a comma is a decimal point), Esc keeps the old value
#[test]
fn scalar_field_types_and_cancels() {
    let mut h = harness(1.0_f32, |ui, v| {
        ui.add(ScalarField::new(v).decimals(2));
    });
    h.get_by_role(egui::accesskit::Role::Slider).click();
    h.run();
    h.run();
    h.get_by_role(egui::accesskit::Role::TextInput)
        .type_text("2,5");
    h.run();
    h.key_press(egui::Key::Enter);
    h.run();
    h.run();
    assert_eq!(*h.state(), 2.5);
    h.get_by_role(egui::accesskit::Role::Slider).click();
    h.run();
    h.run();
    h.get_by_role(egui::accesskit::Role::TextInput)
        .type_text("99");
    h.run();
    h.key_press(egui::Key::Escape);
    h.run();
    h.run();
    assert_eq!(*h.state(), 2.5);
    assert!(h.query_by_role(egui::accesskit::Role::TextInput).is_none());
}

/// A disabled field doesn't change and says why on hover
#[test]
fn scalar_field_disabled_says_why() {
    let mut h = harness(3.0_f64, |ui, v| {
        ui.add(ScalarField::new(v).disabled_because(Some("open a file first")));
    });
    let field = h.get_by_role(egui::accesskit::Role::Slider);
    assert!(field.accesskit_node().is_disabled());
    let at = field.rect().center();
    h.drag_at(at);
    h.run();
    h.drop_at(at + vec2(30.0, 0.0));
    h.run();
    assert_eq!(*h.state(), 3.0);
    h.hover_at(at);
    for _ in 0..8 {
        h.step();
    }
    h.get_by_label("open a file first");
}

/// Each axis of a Vec field changes its own value
#[test]
fn vec_field_axes() {
    let mut h = harness([1.0_f64, 2.0, 3.0], |ui, v| {
        let [x, y, z] = v;
        ui.add(VecField::new([x, y, z]).speed(0.25).decimals(0));
    });
    let sliders: Vec<_> = h
        .get_all_by_role(egui::accesskit::Role::Slider)
        .map(|n| n.rect().center())
        .collect();
    assert_eq!(sliders.len(), 3);
    h.get_by_label("Y");
    let at = sliders[1];
    h.drag_at(at);
    h.run();
    h.hover_at(at + vec2(20.0, 0.0));
    h.run();
    h.drop_at(at + vec2(20.0, 0.0));
    h.run();
    assert_eq!(*h.state(), [1.0, 7.0, 3.0]);
}

/// A segment picks its value; a disabled one doesn't, and says why
#[test]
fn segmented_picks_and_disabled_says_why() {
    let mut h = harness(0_u8, |ui, v| {
        Segmented::new(v)
            .item(0, "Normal", "plain")
            .item(1, "Multiply", "dark where both are")
            .item_if(2, "Screen", "", Some("needs the wgpu renderer"))
            .show(ui);
    });
    h.get_by_label("Multiply").click();
    h.run();
    assert_eq!(*h.state(), 1);
    assert_eq!(
        h.get_by_label("Multiply").accesskit_node().toggled(),
        Some(Toggled::True)
    );
    let screen = h.get_by_label("Screen");
    assert!(screen.accesskit_node().is_disabled());
    screen.click();
    h.run();
    assert_eq!(*h.state(), 1);
    h.get_by_label("Screen").hover();
    for _ in 0..8 {
        h.step();
    }
    h.get_by_label("needs the wgpu renderer");
}

#[test]
fn toggle_chip_toggles() {
    let mut h = harness(false, |ui, v| {
        ui.add(ToggleChip::new(v, "Blink").tooltip("3 times a second"));
    });
    h.get_by_label("Blink").click();
    h.run();
    assert!(*h.state());
    assert_eq!(
        h.get_by_label("Blink").accesskit_node().toggled(),
        Some(Toggled::True)
    );
}

/// A row is found by its text and picked by a click; the lead word is not part of its label
#[test]
fn list_rows_pick() {
    let mut h = harness(None::<usize>, |ui, picked| {
        ListView::new("t").show(ui, |list| {
            for (i, name) in ["Alpha", "Beta", "Gamma"].into_iter().enumerate() {
                if list
                    .row(
                        ListRow::new(name)
                            .lead("ok", Tone::Good)
                            .selected(*picked == Some(i)),
                    )
                    .clicked()
                {
                    *picked = Some(i);
                }
            }
        });
    });
    h.get_by_label("Beta").click();
    h.run();
    assert_eq!(*h.state(), Some(1));
    assert_eq!(
        h.get_by_label("Beta").accesskit_node().toggled(),
        Some(Toggled::True)
    );
}

/// A sub-pane folds on a click of its header and unfolds again; the summary stays on the header
#[test]
fn sub_pane_folds() {
    let mut h = harness((), |ui, _| {
        SubPane::new("View")
            .summary("Copper · Normal")
            .show(ui, |ui| {
                ui.label("inside");
            });
    });
    h.get_by_label("inside");
    h.get_by_label("View").click();
    for _ in 0..20 {
        h.step();
    }
    assert!(h.query_by_label("inside").is_none());
    h.get_by_label("View").click();
    for _ in 0..20 {
        h.step();
    }
    h.get_by_label("inside");
}
