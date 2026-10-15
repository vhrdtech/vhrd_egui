//! Sending a widget to another machine (FRM-5): the capability an app provides.

use serde::{Deserialize, Serialize};

use crate::size::{HeightMode, SizeMode};
use crate::state::FrameState;

/// A machine a widget can be sent to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Machine {
    /// What [`Remote::send`] gets.
    pub id: String,
    /// What the menu shows.
    pub name: String,
    /// Why nothing can be sent there right now ("offline since 12:40"); the menu entry stays, disabled, with
    /// this as its tooltip.
    pub unavailable: Option<String>,
}

impl Machine {
    /// A reachable machine.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            unavailable: None,
        }
    }

    /// Known, but not reachable now, and why.
    pub fn unavailable(mut self, reason: impl Into<String>) -> Self {
        self.unavailable = Some(reason.into());
        self
    }
}

/// A widget on its way to another machine: what is saved of it, nothing that lives only in this process.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Parcel {
    /// The widget's title, for logs and for the receiving side's "arrived" message.
    pub title: String,
    /// The widget serialized the way the app saves its layout (for `ve_widget` widgets: the typetag JSON with
    /// seed and visual state). Transient state is rebuilt on arrival (`Widget::init`).
    pub widget: String,
    /// The size switches travel along; the placement does not: the widget arrives in the grid.
    pub size: SizeMode,
    /// The height switch (FRM-11); auto from a sender that doesn't have it yet.
    #[serde(default)]
    pub height: HeightMode,
}

impl Parcel {
    /// The frame state the widget arrives with: in the main grid, its switches as they were.
    pub fn frame_state(&self) -> FrameState {
        FrameState {
            height: self.height,
            ..FrameState::sized(self.size)
        }
    }
}

/// The app's transport to other machines. An app that has one hands it to the widget host (`Moves::remote`,
/// `ve_app::ShellOptions::remote`); without it the menu entry stays visible and disabled, with the reason.
///
/// The receiving app turns an arriving [`Parcel`] back into a widget and opens it.
pub trait Remote {
    /// The machines to offer, reachable or not.
    fn machines(&self) -> Vec<Machine>;

    /// Hand `parcel` to `machine` ([`Machine::id`]). Must not block the UI thread: queue it and return. An
    /// `Err` is shown to the user and the widget stays where it is.
    fn send(&mut self, machine: &str, parcel: Parcel) -> Result<(), String>;
}
