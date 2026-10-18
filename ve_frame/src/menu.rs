//! The frame's ☰ menu: where the widget can go.

use egui::{Button, Ui};

use crate::remote::{Machine, Remote};
use crate::state::{FrameAction, Placement, ViewportSlot, ViewportTarget};

/// Why sending to another machine is off when the app gave no [`Remote`].
pub const NO_REMOTE: &str =
    "Not available: this app has no connection to other machines to send widgets over";

/// What a frame's menu can offer, from whoever hosts the widget ([`crate::Dock::moves`]). Entries that can't
/// be used stay in the menu, disabled, and say why.
#[derive(Clone, Debug, Default)]
pub struct Moves {
    /// Every OS window open now, the main one included.
    pub viewports: Vec<ViewportSlot>,
    /// The machines the app can send to; `None` when it has no [`Remote`].
    pub machines: Option<Vec<Machine>>,
    /// Whether the menu has *Close*: `None` leaves it out, `Some(Err(why))` shows it disabled with the reason.
    pub close: Option<Result<(), String>>,
}

impl Moves {
    /// Moves between the given OS windows, no other machines, no *Close*.
    pub fn new(viewports: Vec<ViewportSlot>) -> Self {
        Self {
            viewports,
            machines: None,
            close: None,
        }
    }

    /// Offer the machines of the app's transport, if it has one.
    pub fn remote(mut self, remote: Option<&dyn Remote>) -> Self {
        self.machines = remote.map(|r| r.machines());
        self
    }

    /// Add *Close*; `Err(why)` shows it disabled with the reason (a busy widget).
    pub fn close(mut self, allowed: Result<(), String>) -> Self {
        self.close = Some(allowed);
        self
    }
}

/// The entries of a frame in a physics layout ([`crate::WidgetFrame::layout_menu`]): "Push neighbours" flips
/// `push`, "Tidy up" is a submenu of the four ways (FRM-18).
pub(crate) fn layout_ui(ui: &mut Ui, push: &mut bool) -> Option<FrameAction> {
    ui.checkbox(push, "Push neighbours").on_hover_text(
        "On: dragged onto other widgets, dropped there or growing, this one pushes them out of its way. \
         Off: it stops at their edge and nothing else moves",
    );
    let mut picked = None;
    ui.menu_button("Tidy up", |ui| {
        picked = tidy_ui(ui);
    })
    .response
    .on_hover_text("Settle the layout once toward a side, then everything stays put again");
    picked.map(FrameAction::Tidy)
}

/// The four ways to tidy a physics layout up (FRM-18), each an arrow and its word with a tooltip; returns the
/// one picked and closes the menu. For a host's own "Tidy up" menus (a chip, the page's background).
pub fn tidy_ui(ui: &mut Ui) -> Option<crate::Toward> {
    let mut picked = None;
    for toward in crate::Toward::ALL {
        let word = toward.label();
        let r = ui
            .button(format!("{} {word}", toward.arrow()))
            .on_hover_text(format!(
                "Settle the layout once toward the {}: every widget slides {word} against what lies before \
                 it, then everything stays put again",
                match toward {
                    crate::Toward::Up => "top",
                    crate::Toward::Down => "bottom",
                    _ => word,
                }
            ));
        if r.clicked() {
            picked = Some(toward);
            ui.close();
        }
    }
    picked
}

/// The menu's entries; returns what was picked.
pub(crate) fn ui(ui: &mut Ui, moves: &Moves, placement: Placement) -> Option<FrameAction> {
    let mut action = None;
    if placement.floating {
        if ui
            .button("Back into the grid")
            .on_hover_text("Put the widget back among the others of this window")
            .clicked()
        {
            action = Some(FrameAction::DockBack);
        }
    } else if ui
        .button("Pop out as a window")
        .on_hover_text("Take the widget out of the grid: it floats over this window and can be dragged anywhere in it")
        .clicked()
    {
        action = Some(FrameAction::PopOut);
    }

    ui.menu_button("Send to OS window", |ui| {
        if ui
            .button("New window")
            .on_hover_text(
                "Open another window of this app with the widget in it, e.g. for a second monitor",
            )
            .clicked()
        {
            action = Some(FrameAction::SendToViewport(ViewportTarget::New));
        }
        for &slot in &moves.viewports {
            let here = slot == placement.viewport;
            let r = ui.add_enabled(!here, Button::new(slot.label()));
            let r = r
                .on_hover_text(format!("Move the widget into the grid of {}", slot.label()))
                .on_disabled_hover_text("The widget is in this window already");
            if r.clicked() {
                action = Some(FrameAction::SendToViewport(ViewportTarget::Existing(slot)));
            }
        }
    })
    .response
    .on_hover_text("Move the widget to another window of this app");

    match &moves.machines {
        None => {
            ui.add_enabled(false, Button::new("Send to another machine"))
                .on_disabled_hover_text(NO_REMOTE);
        }
        Some(machines) if machines.is_empty() => {
            ui.add_enabled(false, Button::new("Send to another machine"))
                .on_disabled_hover_text("Not available: no other machine is known right now");
        }
        Some(machines) => {
            ui.menu_button("Send to another machine", |ui| {
                for m in machines {
                    let r = ui.add_enabled(m.unavailable.is_none(), Button::new(&m.name));
                    let r = r.on_hover_text(format!(
                        "Close the widget here and open it on {} with its settings",
                        m.name
                    ));
                    let r = match &m.unavailable {
                        Some(why) => r.on_disabled_hover_text(format!("Not available: {why}")),
                        None => r,
                    };
                    if r.clicked() {
                        action = Some(FrameAction::SendToMachine(m.id.clone()));
                    }
                }
            })
            .response
            .on_hover_text("Move the widget to the same app on another machine");
        }
    }

    if let Some(close) = &moves.close {
        ui.separator();
        let r = ui.add_enabled(close.is_ok(), Button::new("Close"));
        let r = r.on_hover_text("Close the widget; its settings are lost");
        let r = match close {
            Err(why) => r.on_disabled_hover_text(format!("Can't be closed now: {why}")),
            Ok(()) => r,
        };
        if r.clicked() {
            action = Some(FrameAction::Close);
        }
    }
    if action.is_some() {
        ui.close();
    }
    action
}
