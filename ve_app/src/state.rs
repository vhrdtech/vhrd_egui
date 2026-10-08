//! What the shell persists: the tile tree, the side panel and which windows are open.

use serde::{Deserialize, Serialize};
use ve_widget::Widget;

use crate::ShellOptions;

/// Storage key of the shell state in eframe storage. JSON, not eframe's RON: typetag's internally tagged
/// widgets need `deserialize_any`, which serde_json handles fully.
const KEY: &str = "ve_app_shell";

/// Bump when [`State`] changes shape in a way old saves can't be read into.
const SHELL_VERSION: u32 = 1;

pub(crate) type Tree = egui_tiles::Tree<Box<dyn Widget>>;

#[derive(Serialize, Deserialize)]
pub(crate) struct State {
    /// `SHELL_VERSION` and the app's `layout_version` this was saved with.
    pub(crate) version: (u32, u32),
    pub(crate) tree: Tree,
    #[serde(default)]
    pub(crate) side_panel_open: bool,
    #[serde(default)]
    pub(crate) windows: OpenWindows,
}

/// Which of the shell's windows are open.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct OpenWindows {
    pub(crate) about: bool,
    pub(crate) settings: bool,
    pub(crate) debug: bool,
}

impl State {
    pub(crate) fn new(options: &ShellOptions) -> Self {
        Self {
            version: (SHELL_VERSION, options.layout_version),
            tree: default_tree(options),
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

/// A tree with the app's default widgets as tabs (or an empty one).
pub(crate) fn default_tree(options: &ShellOptions) -> Tree {
    egui_tiles::Tree::new_tabs("ve_app_tiles", (options.default_widgets)())
}
