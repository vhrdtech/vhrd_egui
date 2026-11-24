//! The bar: the running version and its menu of kept releases, or a newer installed release with Restart.

use egui::{Button, Id, Response, RichText, Ui};
use ve_theme::Tokens;

use crate::model::{Release, Releases, Running, Tone, View};
use crate::relaunch::Relaunch;

/// The Restart button's text.
pub const RESTART: &str = "Restart";
/// A kept release's switch button.
pub const SWITCH: &str = "Switch to this one";
/// The confirm step's buttons: the safe one stands out.
const KEEP_RUNNING: &str = "Keep running";
const RESTART_ANYWAY: &str = "Restart anyway";

/// The update bar of a native app, for its title or status bar (CHR-9).
///
/// The running version, whose menu lists the kept releases with [`SWITCH`]; once the installed release is not the
/// running one, "New version installed: VERSION (commit, age)" and [`RESTART`]. Restart plans a [`Relaunch`] and
/// closes the window; while [`UpdateBar::busy`] says why, it asks first.
///
/// ```no_run
/// # fn f(ui: &mut egui::Ui, releases: &dyn ve_update::Releases, relaunch: &ve_update::Relaunch) {
/// ve_update::UpdateBar::new(releases, relaunch)
///     .busy(Some("A card waits for your answer: it comes back after the restart."))
///     .show(ui);
/// # }
/// ```
pub struct UpdateBar<'a> {
    releases: &'a dyn Releases,
    relaunch: &'a Relaunch,
    busy: Option<&'a str>,
    now_unix: Option<i64>,
    id: Id,
}

impl<'a> UpdateBar<'a> {
    /// The bar over `releases`; Restart plans into `relaunch`.
    pub fn new(releases: &'a dyn Releases, relaunch: &'a Relaunch) -> Self {
        Self {
            releases,
            relaunch,
            busy: None,
            now_unix: None,
            id: Id::new("ve_update_bar"),
        }
    }

    /// Why a restart now would interrupt something: Restart then asks first, showing this.
    pub fn busy(mut self, why: Option<&'a str>) -> Self {
        self.busy = why;
        self
    }

    /// The time ages are counted to, in Unix seconds; the system clock by default.
    pub fn now_unix(mut self, now: i64) -> Self {
        self.now_unix = Some(now);
        self
    }

    /// Another id, for a second bar in one window.
    pub fn id_salt(mut self, salt: impl std::hash::Hash + std::fmt::Debug) -> Self {
        self.id = Id::new(("ve_update_bar", salt));
        self
    }

    /// Draws the bar.
    pub fn show(self, ui: &mut Ui) -> Response {
        let view = self.releases.view();
        let now = self.now_unix.unwrap_or_else(now_unix);
        ui.horizontal_wrapped(|ui| {
            if ui.data(|d| d.get_temp::<bool>(self.id)).unwrap_or(false) {
                self.confirm(ui);
                return;
            }
            self.versions(ui, &view, now);
            if let Some(newer) = view.newer() {
                self.newer(ui, &view, newer, now);
            }
            if let Some(w) = &view.working {
                ui.spinner();
                ui.label(RichText::new(w).weak());
            }
            if let Some(p) = &view.problem {
                ui.colored_label(Tokens::of_ui(ui).colors.crit, p);
            }
        })
        .response
    }

    /// The running version as a menu button; the menu lists every kept release.
    fn versions(&self, ui: &mut Ui, view: &View, now: i64) {
        let running = match &view.running {
            Running::Release(id) => view.release(id).map_or_else(|| id.clone(), Release::label),
            Running::Other(what) => what.clone(),
        };
        let r = ui.menu_button(RichText::new(&running).weak(), |ui| {
            menu(ui, self.releases, view, now);
        });
        r.response.on_hover_text(format!(
            "Runs {running}. Click for the kept releases: switch to one to go back or forward."
        ));
    }

    /// The installed release that does not run yet, and Restart.
    fn newer(&self, ui: &mut Ui, view: &View, newer: &Release, now: i64) {
        let tokens = Tokens::of_ui(ui);
        let ramp = tokens.colors.status.info;
        let age = newer
            .installed_unix
            .map(|t| format!(", {}", ve_basics::ago(t, now)))
            .unwrap_or_default();
        egui::Frame::new()
            .fill(ramp.soft)
            .stroke(egui::Stroke::new(tokens.stroke.thin, ramp.line))
            .corner_radius(tokens.radius.m)
            .inner_margin(egui::Margin::from(egui::vec2(
                tokens.space.m,
                tokens.space.xs,
            )))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new(format!("New version installed: {}{age}", newer.label()))
                            .color(ramp.solid),
                    );
                    let blocked = match (&view.working, self.releases.program()) {
                        (Some(w), _) => Some(format!("Not now: {w}.")),
                        (None, Err(why)) => Some(format!("Can't restart: {why}")),
                        (None, Ok(_)) => None,
                    };
                    let b = ui.add_enabled(blocked.is_none(), Button::new(RESTART));
                    let b = match &blocked {
                        Some(why) => b.on_disabled_hover_text(why),
                        None => b.on_hover_text(format!(
                            "Closes this window and starts {} with the same arguments.",
                            newer.label()
                        )),
                    };
                    if b.clicked() {
                        if self.busy.is_some() {
                            ui.data_mut(|d| d.insert_temp(self.id, true));
                        } else {
                            self.restart(ui);
                        }
                    }
                });
            });
    }

    /// Restart while busy: what it interrupts, and the choice; keeping it running is the prominent one.
    fn confirm(&self, ui: &mut Ui) {
        let tokens = Tokens::of_ui(ui);
        let ramp = tokens.colors.status.warning;
        egui::Frame::new()
            .fill(ramp.soft)
            .stroke(egui::Stroke::new(tokens.stroke.thin, ramp.line))
            .corner_radius(tokens.radius.m)
            .inner_margin(egui::Margin::from(egui::vec2(
                tokens.space.m,
                tokens.space.xs,
            )))
            .show(ui, |ui| {
                // the question wraps above its answers, so a narrow window never cuts the buttons off
                ui.vertical(|ui| {
                    let why = self.busy.unwrap_or("The app is busy.");
                    ui.add(
                        egui::Label::new(
                            RichText::new(format!("{why} Restart anyway?")).color(ramp.solid),
                        )
                        .wrap(),
                    );
                    ui.horizontal(|ui| {
                        if ui
                            .add(Button::new(RichText::new(KEEP_RUNNING).strong()))
                            .clicked()
                        {
                            ui.data_mut(|d| d.remove::<bool>(self.id));
                        }
                        if ui.button(RESTART_ANYWAY).clicked() {
                            ui.data_mut(|d| d.remove::<bool>(self.id));
                            self.restart(ui);
                        }
                    });
                });
            });
    }

    fn restart(&self, ui: &Ui) {
        if let Ok(program) = self.releases.program() {
            self.relaunch.request(ui.ctx(), program);
        }
    }
}

/// The menu of kept releases, newest first: each with its label, age and note, and [`SWITCH`].
fn menu(ui: &mut Ui, releases: &dyn Releases, view: &View, now: i64) {
    let tokens = Tokens::of_ui(ui);
    ui.set_min_width(320.0);
    if view.releases.is_empty() {
        ui.label(RichText::new("No kept releases: this build was not installed.").weak());
    }
    for r in &view.releases {
        let running = matches!(&view.running, Running::Release(id) if *id == r.id);
        let installed = view.installed.as_deref() == Some(r.id.as_str());
        ui.horizontal(|ui| {
            let mut line = r.label();
            if let Some(t) = r.installed_unix {
                line.push_str(&format!(" · {}", ve_basics::ago(t, now)));
            }
            ui.label(RichText::new(line).strong());
            for (on, word) in [(running, "runs"), (installed, "installed")] {
                if on {
                    ui.label(RichText::new(word).color(tokens.colors.accent));
                }
            }
        });
        if let Some(note) = &r.note {
            let color = match r.tone {
                Tone::Good => tokens.colors.good,
                Tone::Bad => tokens.colors.crit,
                Tone::Neutral => tokens.colors.text_muted,
            };
            ui.colored_label(color, note);
        }
        let why_not = if installed {
            Err(if running {
                "Installed and running already.".to_owned()
            } else {
                format!("Installed already: {RESTART} starts it.")
            })
        } else if let Some(w) = &view.working {
            Err(format!("Not now: {w}."))
        } else {
            r.switch.clone()
        };
        let hover = match &why_not {
            Err(why) => why.clone(),
            Ok(()) => format!(
                "Installs {} again in place of the installed release; {RESTART} then runs it.",
                r.label()
            ),
        };
        let clicked = ui
            .push_id(&r.id, |ui| {
                ui.add_enabled(why_not.is_ok(), Button::new(SWITCH))
                    .on_disabled_hover_text(&hover)
                    .on_hover_text(&hover)
                    .clicked()
            })
            .inner;
        if clicked && why_not.is_ok() {
            releases.switch(&r.id);
            ui.close();
        }
        ui.separator();
    }
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}
