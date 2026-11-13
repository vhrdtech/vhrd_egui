//! The SpaceMouse's settings as one panel every app embeds as is (SPM-5): Calibrate…, the mapping with where each
//! value comes from, Save to ~/.config and Save locally, the unsaved changes and the files found.

use std::path::PathBuf;

use egui::{RichText, Ui};
use ve_theme::UiExt;

use crate::calibrate::{Calibration, CalibrationAction, calibration_ui};
use crate::config::{Config, Saved, display_path, key_values};
use crate::event::{Axes, Event};
use crate::mapping::Mapping;
use crate::settings_ui::mapping_ui_from;

/// Which motions the calibrate step asks for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CalibrateSteps {
    /// Slide, lift, push and the two buttons: a 2D view ([`Calibration::two_d`]).
    #[default]
    TwoD,
    /// All six motions and the two buttons: a view that turns ([`Calibration::full`]).
    Full,
}

/// Which file a save writes (SPM-6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveTarget {
    /// The user's `~/.config/spacemouse.toml`: every app, every folder.
    User,
    /// `.config/spacemouse.toml` in the current folder (the project), made when missing.
    Local,
}

impl SaveTarget {
    /// Both, in the order the buttons show them.
    pub const ALL: [SaveTarget; 2] = [SaveTarget::User, SaveTarget::Local];

    /// The button's label.
    pub fn label(self) -> &'static str {
        match self {
            SaveTarget::User => "Save to ~/.config",
            SaveTarget::Local => "Save locally",
        }
    }
}

/// The settings panel: the [`Config`] read from the files, the mapping in use (with unsaved changes) and the
/// calibrate step's window.
///
/// Per frame: [`SettingsPanel::feed`] the events (the calibrate step records them), use
/// [`SettingsPanel::mapping`] unless [`SettingsPanel::calibrating`], show [`SettingsPanel::ui`] where the app keeps
/// its settings, and [`SettingsPanel::calibration_window`] and [`SettingsPanel::leave_guard`] once per frame.
pub struct SettingsPanel {
    config: Config,
    mapping: Mapping,
    steps: CalibrateSteps,
    calibration: Option<Calibration>,
    note: Option<String>,
    /// The window was asked to close with unsaved changes: the question is open.
    leaving: bool,
}

impl SettingsPanel {
    /// The panel over `config`, its mapping in use.
    pub fn new(config: Config, steps: CalibrateSteps) -> Self {
        let mapping = config.mapping().clone();
        Self {
            config,
            mapping,
            steps,
            calibration: None,
            note: None,
            leaving: false,
        }
    }

    /// The files for the current folder and the user's config folder ([`Config::load`]).
    pub fn load(steps: CalibrateSteps) -> Self {
        Self::new(Config::load(), steps)
    }

    /// The mapping in use, unsaved changes included.
    pub fn mapping(&self) -> &Mapping {
        &self.mapping
    }

    /// Use another mapping (not saved until [`SettingsPanel::save`]).
    pub fn set_mapping(&mut self, mapping: Mapping) {
        if mapping != self.mapping {
            self.mapping = mapping;
            self.note = Some("changed, not saved yet".into());
        }
    }

    /// The files and what they give.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Changed since the files were read or saved.
    pub fn changed(&self) -> bool {
        &self.mapping != self.config.mapping()
    }

    /// The keys changed and not saved, as dotted paths (`speed.pan`, `axes`).
    pub fn unsaved_keys(&self) -> Vec<String> {
        let now = key_values(&self.mapping);
        let read = key_values(self.config.mapping());
        now.iter()
            .filter(|(k, v)| read.get(*k) != Some(*v))
            .map(|(k, _)| k.clone())
            .collect()
    }

    /// The question "save before closing?" is open ([`SettingsPanel::leave_guard`]).
    pub fn leaving(&self) -> bool {
        self.leaving
    }

    /// The calibrate step's window is open: the app should not move anything by the puck, nor open its menus.
    pub fn calibrating(&self) -> bool {
        self.calibration.is_some()
    }

    /// Open the calibrate step, starting from the mapping in use.
    pub fn start_calibration(&mut self) {
        let base = self.mapping.clone();
        self.calibration = Some(match self.steps {
            CalibrateSteps::TwoD => Calibration::two_d(base),
            CalibrateSteps::Full => Calibration::full(base),
        });
    }

    /// The calibrate step, while it is open.
    pub fn calibration_mut(&mut self) -> Option<&mut Calibration> {
        self.calibration.as_mut()
    }

    /// The last thing that happened: saved where, or why not.
    pub fn note(&self) -> Option<&str> {
        self.note.as_deref()
    }

    /// The file `target` stands for, or why there is none.
    pub fn target_path(&self, target: SaveTarget) -> Result<PathBuf, &'static str> {
        match target {
            SaveTarget::User => self
                .config
                .user_file()
                .map(PathBuf::from)
                .ok_or("No user config folder: neither XDG_CONFIG_HOME nor HOME is set"),
            SaveTarget::Local => self
                .config
                .new_project_file()
                .ok_or("No current folder known, so no local file"),
        }
    }

    /// Hand spacenavd's events of this frame to the calibrate step while it is open.
    pub fn feed(&mut self, events: &[Event], now: f64) {
        if let Some(cal) = &mut self.calibration {
            for e in events {
                cal.feed(e, now);
            }
        }
    }

    /// What saving into `target` would write: the file and the keys, one line for a tooltip.
    pub fn plan_line(&self, target: SaveTarget) -> String {
        match self.target_path(target) {
            Err(why) => why.to_string(),
            Ok(path) => {
                let keys = self.config.plan(&self.mapping, &path);
                let what = if keys.is_empty() {
                    "no keys: everything as the farther files and the defaults give it".to_string()
                } else {
                    keys.join(", ")
                };
                let new = if path.exists() { "" } else { " (new file)" };
                format!(
                    "Writes {}{new}, which then holds: {what}",
                    display_path(&path)
                )
            }
        }
    }

    /// Write the mapping in use into `target`'s file.
    pub fn save(&mut self, target: SaveTarget) -> Result<Saved, String> {
        let result = self
            .target_path(target)
            .map_err(|e| format!("not saved: {e}"))
            .and_then(|path| self.config.save(&self.mapping, &path));
        self.note = Some(match &result {
            Ok(s) => s.line(),
            Err(e) => e.clone(),
        });
        if let Ok(s) = &result {
            log::info!("ve_spacemouse: {}", s.line());
        }
        result
    }

    /// Read the files again; unsaved changes are dropped.
    pub fn reload(&mut self) {
        self.config.reload();
        self.mapping = self.config.mapping().clone();
        self.note = Some("read the files again".into());
    }

    /// Where a key's value comes from: a short name and a tooltip.
    fn from(&self, key: &str) -> (String, String) {
        let now = key_values(&self.mapping);
        let read = key_values(self.config.mapping());
        if now.get(key) != read.get(key) {
            return (
                "changed".into(),
                "Not saved yet: Save to ~/.config or Save locally keeps it".into(),
            );
        }
        match self.config.source(key) {
            Some(l) if l.user => ("user".into(), display_path(&l.path)),
            Some(l) => (project_name(&l.path), display_path(&l.path)),
            None => (
                "default".into(),
                "No file sets it: ve_spacemouse's default".into(),
            ),
        }
    }

    /// The panel. `live` (the puck now) adds a bar per output. Returns whether the mapping changed.
    pub fn ui(&mut self, ui: &mut Ui, live: Option<&Axes>) -> bool {
        let t = ui.tokens();
        let calibrating = self.calibrating();
        let what = match self.steps {
            CalibrateSteps::TwoD => "slide it, lift it and push it away",
            CalibrateSteps::Full => "slide, lift, push, tilt, rock and twist it",
        };
        if ui
            .add_enabled(!calibrating, egui::Button::new("Calibrate…"))
            .on_hover_text(format!(
                "Measure which way the puck reads when you {what}, and which button is which; then save it with \
                 Save to ~/.config (every app, every folder) or Save locally (this folder)"
            ))
            .on_disabled_hover_text("The calibration window is open")
            .clicked()
        {
            self.start_calibration();
        }
        ui.add_space(t.space.s);

        let mut mapping = self.mapping.clone();
        let changed = {
            let from = |k: &str| self.from(k);
            mapping_ui_from(ui, &mut mapping, live, Some(&from))
        };
        if changed {
            self.set_mapping(mapping);
        }
        ui.add_space(t.space.s);

        self.unsaved_ui(ui);
        ui.horizontal(|ui| {
            self.save_buttons(ui);
            let reload = ui.button("Reload").on_hover_text(if self.changed() {
                "Read the files again; the changes not saved are dropped"
            } else {
                "Read the files again, for changes made by another app or by hand"
            });
            if reload.clicked() {
                self.reload();
            }
        });
        if let Some(n) = &self.note {
            ui.label(RichText::new(n).small());
        }
        for p in self.config.problems() {
            ui.colored_label(t.colors.warn, RichText::new(p).small());
        }
        self.files_ui(ui);
        changed
    }

    /// The changed keys, in the warning colour, while they are not saved.
    fn unsaved_ui(&self, ui: &mut Ui) {
        let keys = self.unsaved_keys();
        if keys.is_empty() {
            return;
        }
        let t = ui.tokens();
        ui.colored_label(t.colors.warn, format!("⚠ Unsaved: {}", keys.join(", ")))
            .on_hover_text("In use now, lost when the app closes unless saved; closing asks");
    }

    /// "Save to ~/.config" and "Save locally", each off with the reason when there is nothing to save or no file.
    /// Returns whether one saved.
    fn save_buttons(&mut self, ui: &mut Ui) -> bool {
        let changed = self.changed();
        let mut saved = false;
        for target in SaveTarget::ALL {
            let why = match self.target_path(target) {
                Err(why) => Some(why.to_string()),
                Ok(_) if !changed => Some("Nothing changed since the files were read".to_string()),
                Ok(_) => None,
            };
            let b = ui
                .add_enabled(why.is_none(), egui::Button::new(target.label()))
                .on_hover_text(self.plan_line(target));
            let b = match &why {
                Some(why) => b.on_disabled_hover_text(why),
                None => b,
            };
            if b.clicked() {
                saved |= self.save(target).is_ok();
            }
        }
        saved
    }

    /// Once per frame: when the window is asked to close while the mapping has unsaved changes, the close is held
    /// and a question asks to save to ~/.config, save locally, close without saving, or stay.
    pub fn leave_guard(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.viewport().close_requested()) && self.changed() && !self.leaving {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.leaving = true;
        }
        if !self.leaving {
            return;
        }
        let mut close = false;
        let mut stay = false;
        egui::Modal::new(egui::Id::new("ve_spacemouse_leave")).show(ctx, |ui| {
            ui.heading("Save the SpaceMouse's mapping?");
            ui.label(format!(
                "Changed and not saved: {}.",
                self.unsaved_keys().join(", ")
            ));
            let s = ui.tokens().space.s;
            ui.add_space(s);
            ui.horizontal(|ui| {
                if self.save_buttons(ui) {
                    close = true;
                }
                if ui
                    .danger_button("Close without saving", "The changes are lost")
                    .clicked()
                {
                    close = true;
                }
                if ui
                    .button("Stay")
                    .on_hover_text("Keep the app open")
                    .clicked()
                {
                    stay = true;
                }
            });
            if let Some(n) = &self.note {
                ui.label(RichText::new(n).small());
            }
        });
        if close {
            self.leaving = false;
            if self.changed() {
                // Close without saving: the mapping read from the files is the one in use until the window goes.
                self.mapping = self.config.mapping().clone();
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        } else if stay {
            self.leaving = false;
        }
    }

    /// The files found with the keys each sets; the places looked at with nothing there under a fold of their own.
    fn files_ui(&self, ui: &mut Ui) {
        let layers = self.config.layers();
        let missing: Vec<_> = self
            .config
            .candidates()
            .iter()
            .filter(|c| !layers.iter().any(|l| l.path == c.path))
            .collect();
        egui::CollapsingHeader::new(format!(
            "Files: {} found, {} places with none",
            layers.len(),
            missing.len()
        ))
        .id_salt("ve_spacemouse_files")
        .show(ui, |ui| {
            ui.muted_label(
                "Nearest first; a key in a nearer file wins. The user's file serves every folder.",
            );
            for l in layers {
                let keys = l.keys();
                let keys = if keys.is_empty() {
                    "no settings".to_string()
                } else {
                    keys.join(", ")
                };
                ui.label(RichText::new(display_path(&l.path)).monospace())
                    .on_hover_text(if l.user {
                        "The user's file"
                    } else {
                        "A project's file"
                    });
                ui.indent("keys", |ui| ui.muted_label(keys));
            }
            egui::CollapsingHeader::new(format!(
                "{} places looked at, nothing there",
                missing.len()
            ))
            .id_salt("ve_spacemouse_missing")
            .show(ui, |ui| {
                for c in missing {
                    ui.label(RichText::new(display_path(&c.path)).monospace().small());
                }
            });
        });
    }

    /// The calibrate step's window while it is open. Using its mapping makes it the one in use, shown as unsaved
    /// until Save to ~/.config or Save locally; returns `true` on that frame.
    pub fn calibration_window(&mut self, ctx: &egui::Context) -> bool {
        let Some(cal) = &mut self.calibration else {
            return false;
        };
        let mut open = true;
        let mut action = CalibrationAction::None;
        egui::Window::new("Calibrate the SpaceMouse")
            .id(egui::Id::new("ve_spacemouse_calibration_window"))
            .open(&mut open)
            .default_width(460.0)
            .show(ctx, |ui| action = calibration_ui(ui, cal));
        match action {
            CalibrationAction::Apply(m) => {
                self.calibration = None;
                self.set_mapping(m);
                true
            }
            CalibrationAction::Cancel => {
                self.calibration = None;
                false
            }
            CalibrationAction::None => {
                if !open {
                    self.calibration = None;
                }
                false
            }
        }
    }
}

/// "board/.config" for `…/board/.config/spacemouse.toml`.
fn project_name(path: &std::path::Path) -> String {
    path.parent()
        .and_then(|d| d.parent())
        .and_then(|d| d.file_name())
        .map_or_else(
            || "/.config".into(),
            |n| format!("{}/.config", n.to_string_lossy()),
        )
}
