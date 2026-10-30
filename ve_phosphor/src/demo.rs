//! The demo's UI: the scope in a frame, the controls beside it, the output's mode in the status line. The
//! window host and the headless renderer both draw it.

use egui::{RichText, Ui};
use ve_frame::{FrameState, Status, WidgetFrame};
use ve_theme::{Tone, UiExt as _};

use crate::{Output, Phosphor, PhosphorView, Scene, Settings, settings_ui};

/// What the demo is drawn into, for the LED and the status line.
#[derive(Clone, Debug, PartialEq)]
pub struct SurfaceInfo {
    /// The view's target: format and the headroom it shows.
    pub output: Output,
    /// The surface in words: format and color space ("Rgba16Float, extended sRGB").
    pub surface: String,
    /// Why the output is HDR or SDR ("the surface offers no Rgba16Float").
    pub why: String,
    /// The display's headroom now and at most (macOS EDR), when the platform says.
    pub headroom_now: Option<f32>,
    /// The most the display can show (macOS EDR potential), when the platform says.
    pub headroom_max: Option<f32>,
}

impl SurfaceInfo {
    /// The output's mode in a few words: "HDR 7.9x" or "SDR".
    pub fn mode(&self) -> String {
        if self.output.is_hdr() {
            format!("HDR {:.1}x", self.output.headroom)
        } else {
            "SDR".to_owned()
        }
    }

    /// The status line: mode, surface, headroom, reason.
    pub fn line(&self) -> String {
        let mut t = format!("{} · {}", self.mode(), self.surface);
        match (self.headroom_now, self.headroom_max) {
            (Some(n), Some(m)) => {
                t.push_str(&format!(" · display headroom {n:.1}x now, {m:.1}x max"))
            }
            (Some(n), None) => t.push_str(&format!(" · display headroom {n:.1}x")),
            _ => {}
        }
        t.push_str(" · ");
        t.push_str(&self.why);
        t
    }
}

/// What to look at in each scene, for the controls.
fn look_at(scene: Scene) -> &'static str {
    match scene {
        Scene::Clock => {
            "The main trace glows white-hot at the HDR peak; the runt pulse (40 % high), the late falling edge and the narrow spike above the high level stay as faint ghosts for a few seconds. On the SDR side the same ghosts and the trace are squeezed into 0..1: either the trace is flat white or the ghosts vanish."
        }
        Scene::Spectrum => {
            "The carrier peaks at the HDR peak, the noise floor 90 dB below it is a dim green haze, and every few seconds a short frequency-hopping burst leaves a row of faint marks 55 dB under the carrier that fade over the persistence time."
        }
        Scene::Eye => {
            "The eye's rails glow, the crossings are hot; marginal bits draw faint paths into the open eye and crosstalk spikes poke into it: what a mask test catches."
        }
        Scene::Stream => "Sweeps the application pushes in.",
    }
}

/// The demo's state: the phosphor, its settings and the frames around it.
pub struct Demo {
    phosphor: Phosphor,
    /// The settings the controls change.
    pub settings: Settings,
    /// Set the HDR peak to the display's headroom (on an HDR output).
    pub follow_display: bool,
    scope: FrameState,
    controls: FrameState,
}

impl Demo {
    /// The demo drawing `phosphor`, starting from `settings`.
    pub fn new(phosphor: Phosphor, settings: Settings) -> Self {
        Self {
            phosphor,
            settings,
            follow_display: true,
            scope: FrameState::default(),
            controls: FrameState::default(),
        }
    }

    /// The phosphor the demo draws.
    pub fn phosphor(&self) -> &Phosphor {
        &self.phosphor
    }

    /// One frame of `dt` seconds in the window's root `ui`.
    pub fn ui(&mut self, root: &mut Ui, dt: f32, info: &SurfaceInfo) {
        if self.follow_display && info.output.is_hdr() {
            self.settings.peak = info.output.headroom.clamp(1.0, 16.0);
        }
        let stats = self.phosphor.stats();
        egui::Panel::bottom("ve_phosphor_status").show(root, |ui| {
            ui.horizontal(|ui| {
                let tone = if info.output.is_hdr() { Tone::Good } else { Tone::Warn };
                ui.badge(info.mode(), tone, info.why.clone());
                ui.muted_label(info.line());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.muted_label(format!(
                        "{:.0}k wfm/s · {} sweeps/frame · buffer {}×{} px",
                        stats.sweeps_per_s() / 1000.0,
                        stats.sweeps,
                        stats.size[0],
                        stats.size[1]
                    ))
                    .on_hover_text("Waveforms per second drawn in the last frame, sweeps in it, and the hit buffer's size (physical pixels of the view).");
                });
            });
        });
        egui::Panel::left("ve_phosphor_controls")
            .resizable(true)
            .default_size(330.0)
            .show(root, |ui| {
                let mut state = std::mem::take(&mut self.controls);
                WidgetFrame::new("Controls")
                    .about("Scene, tone map and look of the phosphor view. Every control has a tooltip.")
                    .show(ui, &mut state, |ui, _| {
                        egui::ScrollArea::vertical().show(ui, |ui| self.controls_ui(ui, info));
                    });
                self.controls = state;
            });
        egui::CentralPanel::default().show(root, |ui| {
            let (status, why) = led(info);
            let phosphor = self.phosphor.clone();
            let settings = &mut self.settings;
            WidgetFrame::new("Digital phosphor")
                .led(status, why)
                .about(settings.scene.about())
                .info(|ui| {
                    ui.label(format!("{:.0} waveforms/s", stats.sweeps_per_s()));
                    ui.label(format!(
                        "{} sweeps since the last clear",
                        stats.total_sweeps
                    ));
                    ui.label(format!("hit buffer {}×{} px", stats.size[0], stats.size[1]));
                    ui.label(format!("HDR peak {:.1}x SDR white", stats.peak));
                    ui.label(format!("surface: {}", info.surface));
                })
                .scroll_fallback(false)
                .show(ui, &mut self.scope, |ui, _| {
                    let size = ui.available_size().max(egui::vec2(64.0, 64.0));
                    PhosphorView::new(&phosphor).show(ui, settings, dt, info.output, size);
                });
        });
    }

    fn controls_ui(&mut self, ui: &mut Ui, info: &SurfaceInfo) {
        ui.label(RichText::new(look_at(self.settings.scene)).small());
        ui.add_space(6.0);
        let hdr = info.output.is_hdr();
        ui.add_enabled(hdr, egui::Checkbox::new(&mut self.follow_display, "Peak follows the display"))
            .on_hover_text("Set the HDR peak to the display's headroom, re-read twice a second (it changes with the brightness setting).")
            .on_disabled_hover_text("This output is SDR: there is no headroom to follow; the peak slider sets the HDR side's scale.");
        let headroom = info.output.headroom;
        settings_ui(ui, &mut self.settings, headroom);
        ui.horizontal(|ui| {
            if ui
                .button("Clear")
                .on_hover_text("Forget the picture: the persistence starts again from black.")
                .clicked()
            {
                self.phosphor.clear();
            }
            if ui
                .button("Defaults")
                .on_hover_text("Every setting back to its default (keeps the scene).")
                .clicked()
            {
                let scene = self.settings.scene;
                self.settings = Settings {
                    scene,
                    ..Settings::default()
                };
            }
        });
    }
}

fn led(info: &SurfaceInfo) -> (Status, String) {
    if info.output.is_hdr() {
        (
            Status::Good,
            format!("HDR output: {}. {}", info.surface, info.why),
        )
    } else {
        (
            Status::Warn,
            format!("SDR output: {}. {}", info.surface, info.why),
        )
    }
}
