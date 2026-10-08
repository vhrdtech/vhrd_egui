//! Quit, and ask first when a widget is busy (WID-9).

use egui::{Id, Modal, ViewportCommand};
use ve_theme::UiExt as _;

/// Quit handling: closes right away unless a widget says it is not closeable; then a modal names the busy
/// widgets and asks.
#[derive(Default)]
pub(crate) struct CloseDialog {
    /// Busy widgets shown in the open dialog.
    asking: Option<Vec<String>>,
    /// The user said quit anyway: let the next close request through.
    confirmed: bool,
}

impl CloseDialog {
    /// Quit from the menu: close, or ask when `busy` is not empty.
    pub(crate) fn request(&mut self, ctx: &egui::Context, busy: Vec<String>) {
        if busy.is_empty() {
            self.confirmed = true;
            ctx.send_viewport_cmd(ViewportCommand::Close);
        } else {
            self.asking = Some(busy);
        }
    }

    /// Catch the window's close button and draw the dialog when it is open.
    pub(crate) fn ui(&mut self, ctx: &egui::Context, app_name: &str, busy: Vec<String>) {
        if ctx.input(|i| i.viewport().close_requested()) && !self.confirmed && !busy.is_empty() {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.asking = Some(busy);
        }
        let Some(names) = &self.asking else {
            return;
        };
        let mut answer = None;
        let modal = Modal::new(Id::new("ve_app_close_dialog")).show(ctx, |ui| {
            ui.set_max_width(320.0);
            ui.heading(format!("Quit {app_name}?"));
            ui.add_space(8.0);
            ui.label("These widgets are busy and may lose work:");
            for name in names {
                ui.label(format!("• {name}"));
            }
            ui.add_space(12.0);
            egui::Sides::new().show(
                ui,
                |_| {},
                |ui| {
                    if ui
                        .button("Cancel")
                        .on_hover_text("Keep the app open (Esc)")
                        .clicked()
                    {
                        answer = Some(false);
                    }
                    if ui
                        .danger_button(
                            "Quit anyway",
                            "Close now; the busy widgets stop where they are",
                        )
                        .clicked()
                    {
                        answer = Some(true);
                    }
                },
            );
        });
        if modal.should_close() && answer.is_none() {
            answer = Some(false);
        }
        match answer {
            Some(true) => {
                self.asking = None;
                self.confirmed = true;
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
            Some(false) => self.asking = None,
            None => {}
        }
    }
}
