//! The egui side: the view as a widget (a paint callback plus the divider and labels) and the settings form.

use egui::{Align2, CursorIcon, FontId, Rect, Response, Sense, Stroke, Ui, Vec2, pos2};

use crate::gpu::{Callback, Frame, Output, Phosphor};
use crate::settings::{Mode, Palette, Scene, Settings};

/// The phosphor view in an egui `wgpu` paint callback.
///
/// ```no_run
/// # fn f(ui: &mut egui::Ui, phosphor: &ve_phosphor::Phosphor, settings: &mut ve_phosphor::Settings) {
/// let output = ve_phosphor::Output::sdr(egui_wgpu::wgpu::TextureFormat::Bgra8Unorm);
/// let dt = ui.input(|i| i.stable_dt);
/// ve_phosphor::PhosphorView::new(phosphor).show(ui, settings, dt, output, ui.available_size());
/// # }
/// ```
pub struct PhosphorView<'a> {
    phosphor: &'a Phosphor,
}

impl<'a> PhosphorView<'a> {
    /// A view of `phosphor`.
    pub fn new(phosphor: &'a Phosphor) -> Self {
        Self { phosphor }
    }

    /// Draw a frame of `dt` seconds into a rectangle of `size`. In the split mode the divider can be dragged,
    /// which moves `settings.split`. Asks for the next frame: the picture lives.
    pub fn show(
        self,
        ui: &mut Ui,
        settings: &mut Settings,
        dt: f32,
        output: Output,
        size: Vec2,
    ) -> Response {
        let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
        let split_x = rect.left() + settings.split * rect.width();
        if settings.mode == Mode::Split {
            let grab = Rect::from_x_y_ranges(split_x - 6.0..=split_x + 6.0, rect.y_range());
            let near = response.hover_pos().is_some_and(|p| grab.contains(p));
            if near || response.dragged() {
                ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
            }
            if response.dragged()
                && let Some(p) = response.interact_pointer_pos()
            {
                settings.split = ((p.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
            }
        }
        let frame = Frame {
            settings: *settings,
            dt,
            output,
        };
        ui.painter().add(egui_wgpu::Callback::new_paint_callback(
            rect,
            Callback {
                phosphor: self.phosphor.clone(),
                frame,
                rect,
            },
        ));
        self.labels(ui, rect, settings, output);
        ui.ctx().request_repaint();
        response.on_hover_text(hover_text(settings, output))
    }

    fn labels(&self, ui: &Ui, rect: Rect, s: &Settings, output: Output) {
        let painter = ui.painter_at(rect);
        let text = ui.visuals().weak_text_color();
        let font = FontId::monospace(11.0);
        let pad = 6.0;
        let peak = if output.is_hdr() {
            format!("HDR peak {:.1}x", s.peak)
        } else if s.fit_to_sdr {
            format!(
                "HDR peak {:.1}x, shown at 1/{:.0} (SDR screen)",
                s.peak, s.peak
            )
        } else {
            format!("HDR peak {:.1}x, clipped at SDR white", s.peak)
        };
        let split_x = rect.left() + s.split * rect.width();
        match s.mode {
            Mode::Hdr => {
                painter.text(
                    rect.left_top() + Vec2::splat(pad),
                    Align2::LEFT_TOP,
                    peak,
                    font.clone(),
                    text,
                );
            }
            Mode::Sdr => {
                painter.text(
                    rect.right_top() + Vec2::new(-pad, pad),
                    Align2::RIGHT_TOP,
                    "SDR",
                    font.clone(),
                    text,
                );
            }
            Mode::Split => {
                painter.line_segment(
                    [pos2(split_x, rect.top()), pos2(split_x, rect.bottom())],
                    Stroke::new(1.0, ui.visuals().widgets.noninteractive.fg_stroke.color),
                );
                painter.text(
                    pos2(split_x - pad, rect.top() + pad),
                    Align2::RIGHT_TOP,
                    peak,
                    font.clone(),
                    text,
                );
                painter.text(
                    pos2(split_x + pad, rect.top() + pad),
                    Align2::LEFT_TOP,
                    "SDR, clamped at white",
                    font.clone(),
                    text,
                );
            }
        }
        if s.scene == Scene::Spectrum {
            for div in 0..=8 {
                let y = rect.top() + rect.height() * div as f32 / 8.0;
                let db = -15 * div;
                let anchor = if div == 8 {
                    Align2::LEFT_BOTTOM
                } else {
                    Align2::LEFT_TOP
                };
                painter.text(
                    pos2(rect.left() + pad, y + if div == 8 { -2.0 } else { 2.0 }),
                    anchor,
                    format!("{db} dBm"),
                    font.clone(),
                    text,
                );
            }
        }
    }
}

fn hover_text(s: &Settings, output: Output) -> String {
    let mut t = format!(
        "{}\n\nBrightness is log(hits): a pixel every sweep goes through glows at the peak, a single hit is a faint ghost. ",
        s.scene.about()
    );
    if s.mode == Mode::Split {
        t.push_str("Left of the divider: HDR tone map; right: classic SDR, clamped at SDR white. Drag the divider to compare. ");
    }
    if !output.is_hdr() {
        t.push_str("This output can't show more than SDR white.");
    }
    t
}

fn palette_combo(ui: &mut Ui, id: &str, value: &mut Palette) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(value.label())
        .show_ui(ui, |ui| {
            for p in Palette::ALL {
                ui.selectable_value(value, p, p.label())
                    .on_hover_text(p.about());
            }
        });
}

/// The settings form: scene, mode, rates, tone map and look, a tooltip on every control. `headroom` is what the
/// output shows (1.0 on SDR), for the hints.
pub fn settings_ui(ui: &mut Ui, s: &mut Settings, headroom: f32) {
    egui::Grid::new("ve_phosphor_settings")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            ui.label("Scene").on_hover_text("What the scope shows.");
            egui::ComboBox::from_id_salt("ve_phosphor_scene")
                .selected_text(s.scene.label())
                .show_ui(ui, |ui| {
                    for sc in Scene::ALL {
                        ui.selectable_value(&mut s.scene, sc, sc.label()).on_hover_text(sc.about());
                    }
                });
            ui.end_row();

            ui.label("Mode").on_hover_text("HDR tone map, classic SDR, or both side by side with a divider.");
            ui.horizontal(|ui| {
                for m in Mode::ALL {
                    ui.selectable_value(&mut s.mode, m, m.label());
                }
            });
            ui.end_row();

            ui.label("Waveforms/s").on_hover_text("Sweeps drawn per second, all on the GPU. The rare events have per-sweep rates, so more sweeps show them more often.");
            ui.add(egui::Slider::new(&mut s.sweeps_per_s, 1_000.0..=3_000_000.0).logarithmic(true).integer());
            ui.end_row();

            ui.label("Persistence").on_hover_text("Time constant of the fade, seconds: a hit dims to 1/e in this time. Infinite keeps every hit.");
            ui.horizontal(|ui| {
                ui.add_enabled(!s.infinite, egui::Slider::new(&mut s.persistence_s, 0.005..=60.0).logarithmic(true).suffix(" s"))
                    .on_disabled_hover_text("Infinite persistence is on");
                ui.checkbox(&mut s.infinite, "∞").on_hover_text("Infinite persistence: nothing fades.");
            });
            ui.end_row();

            ui.label("Intensity").on_hover_text("Exposure: multiplies the hit counts before the log scale, so the rare hits come up.");
            ui.add(egui::Slider::new(&mut s.intensity, 0.1..=1000.0).logarithmic(true));
            ui.end_row();

            ui.label("Ghost").on_hover_text("Brightness of a single hit, × SDR white (linear light).");
            ui.add(egui::Slider::new(&mut s.ghost, 0.002..=0.3).logarithmic(true));
            ui.end_row();

            let hint = if headroom > 1.0 {
                format!("Peak of the HDR side, × SDR white. This display shows up to {headroom:.1}x now.")
            } else {
                "Peak of the HDR side, × SDR white. This output stops at SDR white (1.0x).".to_owned()
            };
            ui.label("HDR peak").on_hover_text(hint.clone());
            ui.add(egui::Slider::new(&mut s.peak, 1.0..=16.0).suffix("x")).on_hover_text(hint);
            ui.end_row();

            ui.label("HDR palette").on_hover_text("Colours of the HDR side.");
            palette_combo(ui, "ve_phosphor_hdr_palette", &mut s.hdr_palette);
            ui.end_row();

            ui.label("SDR palette").on_hover_text("Colours of the SDR side.");
            palette_combo(ui, "ve_phosphor_sdr_palette", &mut s.sdr_palette);
            ui.end_row();

            ui.label("Bloom").on_hover_text("Soft glow around the light above the threshold, HDR side only. 0 = off.");
            ui.add(egui::Slider::new(&mut s.bloom, 0.0..=2.0));
            ui.end_row();

            ui.label("Bloom radius").on_hover_text("Radius of the glow, physical pixels.");
            ui.add_enabled(s.bloom > 0.0, egui::Slider::new(&mut s.bloom_radius, 1.0..=Settings::MAX_BLOOM_RADIUS).suffix(" px"))
                .on_disabled_hover_text("Bloom is off (0)");
            ui.end_row();

            ui.label("Graticule").on_hover_text("Brightness of the grid, × SDR white. Kept dim: a static bright grid burns into an OLED.");
            ui.add(egui::Slider::new(&mut s.graticule, 0.0..=0.3));
            ui.end_row();

            ui.label("Age span").on_hover_text("Age palette: seconds from fresh (white-cyan) to old (violet).");
            ui.add(egui::Slider::new(&mut s.age_span_s, 0.01..=30.0).logarithmic(true).suffix(" s"));
            ui.end_row();

            ui.label("Fit on SDR").on_hover_text("On a screen that stops at SDR white: draw the HDR side at 1/peak so its gradation shows (darker), instead of clipping it.");
            ui.add_enabled(headroom <= 1.0, egui::Checkbox::without_text(&mut s.fit_to_sdr))
                .on_disabled_hover_text("This output shows HDR: nothing to fit");
            ui.end_row();
        });

    ui.collapsing("Signal", |ui| {
        let sig = &mut s.signal;
        egui::Grid::new("ve_phosphor_signal").num_columns(2).show(ui, |ui| {
            let rate = |ui: &mut Ui, label: &str, tip: &str, v: &mut f32| {
                ui.label(label).on_hover_text(tip);
                ui.add(egui::Slider::new(v, 0.0..=1.0e-2).logarithmic(true).custom_formatter(|v, _| {
                    if v > 0.0 { format!("1 in {:.0}", 1.0 / v) } else { "never".to_owned() }
                }));
                ui.end_row();
            };
            rate(ui, "Runt pulses", "Clock: a pulse that only reaches 40 % of the swing. Eye: a marginal bit that stops inside the eye. Per sweep.", &mut sig.runt_rate);
            rate(ui, "Spikes", "A narrow glitch (clock: above the high level, eye: into the eye). Per sweep.", &mut sig.spike_rate);
            rate(ui, "Late edges", "Clock: the falling edge after the trigger comes late. Per sweep.", &mut sig.late_rate);
            ui.label("Noise").on_hover_text("σ of the vertical noise, divisions.");
            ui.add(egui::Slider::new(&mut sig.noise, 0.0..=0.5));
            ui.end_row();
            ui.label("Jitter").on_hover_text("σ of the edge jitter, fraction of a period (unit interval in the eye).");
            ui.add(egui::Slider::new(&mut sig.jitter, 0.0..=0.1));
            ui.end_row();
            ui.label("Burst every").on_hover_text("Spectrum: seconds between frequency-hopping bursts.");
            ui.add(egui::Slider::new(&mut sig.burst_period_s, 0.2..=20.0).suffix(" s"));
            ui.end_row();
        });
    });
}
