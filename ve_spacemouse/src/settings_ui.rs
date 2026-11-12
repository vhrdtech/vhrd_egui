//! The mapping as a settings page, and the live axes as bars (SPM-2).

use egui::{Sense, Ui, vec2};
use ve_theme::UiExt;

use crate::event::{AXIS_NAMES, Axes};
use crate::mapping::{AxisMap, Mapping, Output};

/// One bar per raw axis: grows right for positive, left for negative, full at `full_scale`.
pub fn axes_bars(ui: &mut Ui, axes: &Axes, full_scale: f32) {
    let t = ui.tokens();
    egui::Grid::new(ui.id().with("ve_spacemouse_axes")).show(ui, |ui| {
        for (k, name) in AXIS_NAMES.iter().enumerate() {
            ui.label(*name);
            signed_bar(ui, axes[k] as f32 / full_scale.max(1.0), &t).on_hover_text(format!(
                "raw {name}: {} (a full push is {full_scale:.0})",
                axes[k]
            ));
            ui.end_row();
        }
    });
}

/// A bar from the middle, -1 to 1.
fn signed_bar(ui: &mut Ui, v: f32, t: &ve_theme::Tokens) -> egui::Response {
    let h = ui.text_style_height(&egui::TextStyle::Body) * 0.7;
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.spacing().slider_width, h), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, t.radius.s, t.colors.surface_raised);
    let mid = rect.center().x;
    let end = mid + v.clamp(-1.0, 1.0) * rect.width() / 2.0;
    let bar = egui::Rect::from_x_y_ranges(mid.min(end)..=mid.max(end), rect.y_range());
    p.rect_filled(bar, 0.0, t.colors.accent);
    p.vline(
        mid,
        rect.y_range(),
        egui::Stroke::new(t.stroke.thin, t.colors.line_strong),
    );
    resp
}

/// Edit a mapping by hand: the raw axis and sign per output, dead zone, full scale, speeds and buttons.
/// `live` (the puck now) adds a bar per output showing what it gives. Returns whether something changed.
pub fn mapping_ui(ui: &mut Ui, mapping: &mut Mapping, live: Option<&Axes>) -> bool {
    let t = ui.tokens();
    let before = mapping.clone();
    egui::Grid::new(ui.id().with("ve_spacemouse_mapping"))
        .striped(true)
        .show(ui, |ui| {
            ui.label("Output");
            ui.label("Axis").on_hover_text(
                "The raw spacenavd axis that drives it; the calibrate step measures this",
            );
            ui.label("Reverse")
                .on_hover_text("Turn the direction around");
            if live.is_some() {
                ui.label("Now")
                    .on_hover_text("What the puck gives this output now, dead zone taken out");
            }
            ui.end_row();
            for o in Output::ALL {
                let map: &mut AxisMap = &mut mapping.axes[o.index()];
                ui.label(o.name());
                egui::ComboBox::from_id_salt(("ve_spacemouse_axis", o))
                    .selected_text(match map.raw {
                        Some(r) => AXIS_NAMES[r],
                        None => "off",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut map.raw, None, "off")
                            .on_hover_text("The puck does not drive this");
                        for (k, name) in AXIS_NAMES.iter().enumerate() {
                            ui.selectable_value(&mut map.raw, Some(k), *name);
                        }
                    })
                    .response
                    .on_hover_text(format!("The raw axis that makes {}", o.effect()));
                ui.add_enabled(
                    map.raw.is_some(),
                    egui::Checkbox::without_text(&mut map.invert),
                )
                .on_hover_text("Reverse this direction")
                .on_disabled_hover_text("Off: pick an axis first");
                if let Some(live) = live {
                    let level = mapping.level(o, live);
                    signed_bar(ui, level, &t).on_hover_text(format!(
                        "{} at {:.0} %",
                        o.name(),
                        level * 100.0
                    ));
                }
                ui.end_row();
            }
        });
    ui.add_space(t.space.s);
    egui::Grid::new(ui.id().with("ve_spacemouse_tuning")).show(ui, |ui| {
        ui.label("Dead zone");
        ui.add(
            egui::Slider::new(&mut mapping.dead_zone, 0.0..=0.5)
                .custom_formatter(|v, _| format!("{:.0} %", v * 100.0)),
        )
        .on_hover_text(
            "The part of the travel that does nothing, so a resting hand does not drift the view",
        );
        ui.end_row();
        ui.label("Full push");
        ui.add(
            egui::DragValue::new(&mut mapping.full_scale)
                .range(10.0..=2000.0)
                .speed(2.0),
        )
        .on_hover_text(
            "The raw value that gives full speed; the calibrate step sets it from your pushes",
        );
        ui.end_row();
        ui.label("Pan speed");
        ui.add(
            egui::Slider::new(&mut mapping.speed.pan, 50.0..=5000.0)
                .logarithmic(true)
                .suffix(" pt/s"),
        )
        .on_hover_text("Points per second at a full push");
        ui.end_row();
        ui.label("Zoom speed");
        ui.add(egui::Slider::new(&mut mapping.speed.zoom, 0.1..=5.0).logarithmic(true))
            .on_hover_text("Zoom steps per second at a full push: 1 is ×2.7 a second");
        ui.end_row();
        ui.label("Rotate speed");
        ui.add(
            egui::Slider::new(&mut mapping.speed.rotate, 0.1..=10.0)
                .logarithmic(true)
                .suffix(" rad/s"),
        )
        .on_hover_text("Radians per second at a full push");
        ui.end_row();
        ui.label("Left button");
        ui.add(egui::DragValue::new(&mut mapping.buttons.left).range(0..=31))
            .on_hover_text("spacenavd's number of the left button; the calibrate step finds it");
        ui.end_row();
        ui.label("Right button");
        ui.add(egui::DragValue::new(&mut mapping.buttons.right).range(0..=31))
            .on_hover_text("spacenavd's number of the right button; the calibrate step finds it");
        ui.end_row();
    });
    *mapping != before
}
