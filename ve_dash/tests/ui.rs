//! Headless UI tests for the ve_dash widgets through egui_kittest (PLT-3).
//!
//! The widgets paint directly, so the main guard is the wgpu image snapshot;
//! AccessKit queries cover what is a real egui widget (labels, tooltips).
//! After an intended visual change: `UPDATE_SNAPSHOTS=1 cargo test -p ve_dash --test ui`
//! and commit the PNG under `tests/snapshots/`.

use egui::{pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use ve_dash::{Meter, Sparkline, StatTile, Status, StatusLight, Theme, panel};

/// Deterministic wiggly series for the snapshot charts.
fn wave(n: usize, scale: f32) -> Vec<f32> {
    (0..n)
        .map(|i| {
            let x = i as f32;
            ((x * 0.37).sin().abs() + (x * 0.11).cos() * 0.3 + 1.0) * scale
        })
        .collect()
}

#[test]
fn status_light_label_is_accessible() {
    let mut h = Harness::new_ui(|ui| {
        ui.add(StatusLight::new(Status::Good, "connected"));
    });
    h.run();
    // The label is a real egui label, so screen readers (and this test) can find it.
    h.get_by_label("connected");
}

#[test]
fn meter_shows_percent_tooltip_on_hover() {
    let mut h = Harness::builder()
        .with_size(vec2(220.0, 60.0))
        .build_ui(|ui| {
            ui.add(Meter::new(0.63).width(150.0));
        });
    h.run();
    h.hover_at(pos2(60.0, 14.0));
    for _ in 0..8 {
        h.step(); // 0.25 s per step; the tooltip appears after 0.5 s
    }
    assert!(
        h.query_by_label("63 %").is_some(),
        "hovering the meter must reveal the exact percentage"
    );
}

#[test]
fn sparkline_survives_garbage_values() {
    let values = [f32::NAN, 1.0, f32::INFINITY, -2.0, f32::NEG_INFINITY, 3.0];
    let mut h = Harness::new_ui(move |ui| {
        ui.add(Sparkline::new(&values).size(vec2(120.0, 40.0)));
    });
    h.run();
    h.hover_at(pos2(60.0, 20.0)); // crosshair + readout path
    h.run();
}

#[test]
fn sparkline_with_too_few_points_renders_placeholder() {
    let values = [1.0];
    let mut h = Harness::new_ui(move |ui| {
        ui.add(Sparkline::new(&values).size(vec2(120.0, 40.0)));
    });
    h.run();
}

/// Full btop-style composition, pixel-compared against `tests/snapshots/mesh_panel.png`.
#[test]
fn mesh_panel_snapshot() {
    let theme = Theme::dark();
    let rtt = wave(120, 3.0);
    let events = wave(120, 18.0);
    let mut h = Harness::builder()
        .with_size(vec2(460.0, 330.0))
        .build_ui(move |ui| {
            theme.apply(ui.ctx());
            // Cover the whole backbuffer, not just the content area, so the
            // snapshot shows the dashboard on its real window background.
            ui.painter()
                .rect_filled(ui.ctx().viewport_rect(), 0.0, theme.bg);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 28.0;
                ui.add(StatTile::new("nodes online", "3/3").value_color(theme.good));
                ui.add(StatTile::new("worst rtt", "4.2").unit("ms"));
                ui.add(StatTile::new("this pc", "omarchy-m1"));
            });
            ui.add_space(8.0);
            panel(ui, &theme, "gpd-omarchy", |ui| {
                ui.add(StatusLight::new(Status::Good, "connected"));
                ui.add_space(6.0);
                ui.add(Sparkline::new(&rtt).height(42.0).color(theme.accent));
                ui.add_space(6.0);
                ui.add(
                    Sparkline::new(&events)
                        .height(28.0)
                        .color(theme.accent_alt)
                        .range(0.0..=60.0),
                );
                ui.add_space(6.0);
                ui.add(Meter::new(0.63).text("63 %"));
            });
        });
    h.run();
    h.snapshot("mesh_panel");
}
