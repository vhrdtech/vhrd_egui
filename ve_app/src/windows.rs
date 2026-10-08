//! The shell's own windows: About, Settings, Debug (WID-8).

use egui::{Ui, Window};
use egui_tiles::Behavior as _;
use ve_theme::UiExt as _;
use ve_widget::Widget;
use ve_widget::context::Context;

use crate::ShellOptions;
use crate::state::State;
use crate::tiles::Behavior;

/// Draw whichever windows are open.
pub(crate) fn show(
    ctx: &egui::Context,
    state: &mut State,
    behavior: &mut Behavior,
    options: &mut ShellOptions,
    cx: &Context,
) {
    let open = &mut state.windows;
    Window::new("About")
        .open(&mut open.about)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| about(ui, options));
    Window::new("Settings")
        .open(&mut open.settings)
        .collapsible(true)
        .scroll([false, true])
        .show(ctx, |ui| settings(ui, options, cx));
    Window::new("Debug")
        .open(&mut open.debug)
        .collapsible(true)
        .scroll([true, true])
        .show(ctx, |ui| debug(ui, &mut state.tree, behavior));
}

fn about(ui: &mut Ui, options: &ShellOptions) {
    ui.heading(&options.app_name);
    if !options.description.is_empty() {
        ui.label(&options.description);
    }
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label("Build")
            .on_hover_text("Version · git commit · build profile");
        let r = ve_basics::build_info_label(ui, &options.build_info);
        ve_basics::hover_link(ui, "ve_app::build_info", &r);
    });
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.label("Built with ");
        ui.hyperlink_to("egui", "https://github.com/emilk/egui")
            .on_hover_text("The immediate-mode GUI library (opens the browser)");
        ui.label(" and vhrd_egui.");
    });
}

fn settings(ui: &mut Ui, options: &mut ShellOptions, cx: &Context) {
    ui.section_header("Appearance", "How the app looks");
    ui.horizontal(|ui| {
        ui.label("Theme")
            .on_hover_text("Follow the system, or force dark or light");
        egui::widgets::global_theme_preference_buttons(ui);
    });
    if let Some(app_settings) = &mut options.settings {
        ui.add_space(8.0);
        ui.section_header(&options.app_name, "Settings of this app");
        app_settings(ui, cx);
    }
}

fn debug(ui: &mut Ui, tree: &mut crate::state::Tree, behavior: &mut Behavior) {
    ui.collapsing("Tab bars", |ui| behavior.settings_ui(ui));
    ui.collapsing("Active tiles", |ui| {
        for tile_id in tree.active_tiles() {
            let name = behavior.tab_title_for_tile(&tree.tiles, tile_id);
            ui.label(format!("{} - {tile_id:?}", name.text()));
        }
    });
    ui.collapsing("Tile tree", |ui| {
        if let Some(root) = tree.root() {
            tree_ui(ui, behavior, &mut tree.tiles, root);
        }
    });
    ui.collapsing("egui", |ui| ui.ctx().clone().inspection_ui(ui));
}

/// One tile with its visibility and container kind, then its children.
fn tree_ui(
    ui: &mut Ui,
    behavior: &mut Behavior,
    tiles: &mut egui_tiles::Tiles<Box<dyn Widget>>,
    tile_id: egui_tiles::TileId,
) {
    // The title before the tile is taken out below.
    let text = format!(
        "{} - {tile_id:?}",
        behavior.tab_title_for_tile(tiles, tile_id).text()
    );
    // Take the tile out to borrow the rest of `tiles` for the children.
    let Some(mut tile) = tiles.remove(tile_id) else {
        log::debug!("ve_app: missing tile {tile_id:?}");
        return;
    };
    egui::collapsing_header::CollapsingState::load_with_default_open(
        ui.ctx(),
        ui.id().with((tile_id, "tree")),
        true,
    )
    .show_header(ui, |ui| {
        ui.label(text);
        let mut visible = tiles.is_visible(tile_id);
        ui.checkbox(&mut visible, "Visible")
            .on_hover_text("Hide the tile without closing it");
        tiles.set_visible(tile_id, visible);
    })
    .body(|ui| {
        if let egui_tiles::Tile::Container(container) = &mut tile {
            let mut kind = container.kind();
            egui::ComboBox::from_label("Kind")
                .selected_text(format!("{kind:?}"))
                .show_ui(ui, |ui| {
                    for k in egui_tiles::ContainerKind::ALL {
                        ui.selectable_value(&mut kind, k, format!("{k:?}"));
                    }
                })
                .response
                .on_hover_text("Tabs, horizontal or vertical split, or grid");
            if kind != container.kind() {
                container.set_kind(kind);
            }
            for child in container.children_vec() {
                tree_ui(ui, behavior, tiles, child);
            }
        }
    });
    tiles.insert(tile_id, tile);
}
