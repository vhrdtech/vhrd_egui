//! The SpaceMouse's settings as one panel every app embeds as is (SPM-5): Calibrate…, the mapping with where each
//! value comes from, where to save, and the files found.

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

/// Which file Save and Calibrate write.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SaveTarget {
    /// The user's `~/.config/spacemouse.toml`: every app, every folder.
    #[default]
    User,
    /// The nearest project's `.config/spacemouse.toml`, or a new one in the current folder.
    Project,
}

/// The settings panel: the [`Config`] read from the files, the mapping in use (with unsaved changes) and the
/// calibrate step's window.
///
/// Per frame: [`SettingsPanel::feed`] the events (the calibrate step records them), use
/// [`SettingsPanel::mapping`] unless [`SettingsPanel::calibrating`], show [`SettingsPanel::ui`] where the app keeps
/// its settings and [`SettingsPanel::calibration_window`] once per frame.
pub struct SettingsPanel {
    config: Config,
    mapping: Mapping,
    steps: CalibrateSteps,
    calibration: Option<Calibration>,
    target: SaveTarget,
    note: Option<String>,
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
            target: SaveTarget::User,
            note: None,
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

    /// Where Save writes.
    pub fn target(&self) -> SaveTarget {
        self.target
    }

    /// Choose where Save writes.
    pub fn set_target(&mut self, target: SaveTarget) {
        self.target = target;
    }

    /// The file `target` stands for, or why there is none.
    pub fn target_path(&self, target: SaveTarget) -> Result<PathBuf, &'static str> {
        match target {
            SaveTarget::User => self
                .config
                .user_file()
                .map(PathBuf::from)
                .ok_or("No user config folder: neither XDG_CONFIG_HOME nor HOME is set"),
            SaveTarget::Project => self
                .config
                .project_target()
                .ok_or("No current folder known, so no project file"),
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

    /// Write the mapping in use into the chosen file.
    pub fn save(&mut self) -> Result<Saved, String> {
        let result = self
            .target_path(self.target)
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
            let to = self
                .target_path(self.target)
                .map_or_else(|e| e.to_string(), |p| display_path(&p));
            return (
                "changed".into(),
                format!("Not saved yet; Save writes it to {to}"),
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
                "Measure which way the puck reads when you {what}, and which button is which; saved where Save \
                 writes, for every app"
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

        ui.horizontal(|ui| {
            ui.label("Save to");
            for (target, name) in [
                (SaveTarget::User, "every folder"),
                (SaveTarget::Project, "this project"),
            ] {
                match self.target_path(target) {
                    Ok(path) => {
                        let new = !path.exists();
                        let tip = format!(
                            "{}{}",
                            display_path(&path),
                            if new { " (new file)" } else { "" }
                        );
                        ui.radio_value(&mut self.target, target, name)
                            .on_hover_text(tip);
                    }
                    Err(why) => {
                        ui.add_enabled(false, egui::RadioButton::new(false, name))
                            .on_disabled_hover_text(why);
                    }
                }
            }
        });
        ui.horizontal(|ui| {
            let changed = self.changed();
            let save = ui
                .add_enabled(changed, egui::Button::new("Save"))
                .on_hover_text(
                    self.target_path(self.target)
                        .map_or_else(|e| e.to_string(), |p| display_path(&p)),
                )
                .on_disabled_hover_text("Nothing changed since the files were read");
            if save.clicked() {
                let _ = self.save();
            }
            let reload = ui.button("Reload").on_hover_text(if changed {
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

    /// The calibrate step's window while it is open. Using its mapping makes it the one in use and saves it where
    /// Save writes; returns `true` on that frame.
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
                self.mapping = m;
                let _ = self.save();
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
