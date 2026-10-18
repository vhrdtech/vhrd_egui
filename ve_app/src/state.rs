//! What the shell persists: the widgets with their frame state, the tile trees, the side panel and which
//! windows are open.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ve_frame::{Dock, SlotId};
use ve_widget::Widget;

use crate::ShellOptions;

/// Storage key of the shell state in eframe storage. JSON, not eframe's RON: typetag's internally tagged
/// widgets need `deserialize_any`, which serde_json handles fully.
const KEY: &str = "ve_app_shell";

/// Bump when [`State`] changes shape in a way old saves can't be read into.
/// 2: widgets live in a dock with their frame state, the trees name them (FRM-7).
const SHELL_VERSION: u32 = 2;

/// A tile tree; its panes name widgets of [`State::widgets`].
pub(crate) type Tree = egui_tiles::Tree<SlotId>;

/// Every open widget with its frame state (size switch, placement).
pub(crate) type Widgets = Dock<Box<dyn Widget>>;

#[derive(Serialize, Deserialize)]
pub(crate) struct State {
    /// `SHELL_VERSION` and the app's `layout_version` this was saved with.
    pub(crate) version: (u32, u32),
    pub(crate) widgets: Widgets,
    /// The tiles of the main window.
    pub(crate) tree: Tree,
    /// The tiles of each extra OS window, by its `ViewportSlot` number.
    #[serde(default)]
    pub(crate) extra: BTreeMap<u32, Tree>,
    #[serde(default)]
    pub(crate) side_panel_open: bool,
    #[serde(default)]
    pub(crate) windows: OpenWindows,
}

/// Which of the shell's windows are open.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct OpenWindows {
    pub(crate) settings: bool,
    pub(crate) debug: bool,
}

impl State {
    pub(crate) fn new(options: &ShellOptions) -> Self {
        let (widgets, tree) = default_layout(options);
        Self {
            version: (SHELL_VERSION, options.layout_version),
            widgets,
            tree,
            extra: BTreeMap::new(),
            side_panel_open: options.side_panel.is_some(),
            windows: OpenWindows::default(),
        }
    }

    /// The saved state when there is one from this layout version, else a fresh one.
    pub(crate) fn load(storage: Option<&dyn eframe::Storage>, options: &ShellOptions) -> Self {
        let Some(json) = storage.and_then(|s| s.get_string(KEY)) else {
            return Self::new(options);
        };
        match Self::from_json(&json, options) {
            Ok(state) => state,
            Err(reason) => {
                log::info!("ve_app: starting with the default layout, {reason}");
                Self::new(options)
            }
        }
    }

    pub(crate) fn from_json(json: &str, options: &ShellOptions) -> Result<Self, String> {
        let state: Self =
            serde_json::from_str(json).map_err(|e| format!("saved layout unreadable: {e}"))?;
        let expected = (SHELL_VERSION, options.layout_version);
        if state.version != expected {
            return Err(format!(
                "saved layout is version {:?}, this build uses {expected:?}",
                state.version
            ));
        }
        Ok(state)
    }

    pub(crate) fn to_json(&self) -> Result<String, String> {
        serde_json::to_string(self).map_err(|e| e.to_string())
    }

    pub(crate) fn save(&self, storage: &mut dyn eframe::Storage) {
        match self.to_json() {
            Ok(json) => storage.set_string(KEY, json),
            Err(e) => log::warn!("ve_app: layout not saved: {e}"),
        }
    }
}

/// The app's default widgets as tabs of the main window (or nothing).
pub(crate) fn default_layout(options: &ShellOptions) -> (Widgets, Tree) {
    let mut widgets = Dock::new();
    let ids = (options.default_widgets)()
        .into_iter()
        .map(|w| widgets.add(w))
        .collect();
    (widgets, egui_tiles::Tree::new_tabs("ve_app_tiles", ids))
}
