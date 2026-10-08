//! Widgets as egui_tiles panes (WID-3).

use egui::{Frame, Id, Response, Ui, UiKind, WidgetText};
use egui_tiles::{SimplificationOptions, Tile, TileId, Tiles, UiResponse};
use ve_widget::context::Context;
use ve_widget::{Widget, WidgetInfo};

use crate::state::Tree;

/// The egui_tiles behavior: draws widgets, their tab titles and the ➕ open-widget menu of each tab bar.
pub(crate) struct Behavior {
    cx: Context,
    pub(crate) simplification: SimplificationOptions,
    pub(crate) tab_bar_height: f32,
    pub(crate) gap_width: f32,
    pub(crate) widget_filter: fn(&WidgetInfo) -> bool,
    /// Picked in a menu this frame: open it, in that tabs container or next to the others.
    pub(crate) open_request: Option<(&'static WidgetInfo, Option<TileId>)>,
}

impl Behavior {
    pub(crate) fn new(cx: Context, widget_filter: fn(&WidgetInfo) -> bool) -> Self {
        Self {
            cx,
            simplification: SimplificationOptions {
                prune_empty_tabs: true,
                prune_empty_containers: true,
                prune_single_child_tabs: false,
                prune_single_child_containers: false,
                all_panes_must_have_tabs: true,
                join_nested_linear_containers: false,
                ..Default::default()
            },
            tab_bar_height: 24.0,
            gap_width: 2.0,
            widget_filter,
            open_request: None,
        }
    }

    /// Tab bar and simplification settings, for the Debug window.
    pub(crate) fn settings_ui(&mut self, ui: &mut Ui) {
        egui::Grid::new("ve_app_tiles_settings")
            .num_columns(2)
            .show(ui, |ui| {
                ui.label("All panes have tabs")
                    .on_hover_text("Every widget sits in a tab container, even when it is alone");
                ui.checkbox(&mut self.simplification.all_panes_must_have_tabs, "");
                ui.end_row();

                ui.label("Join nested splits").on_hover_text(
                    "Merge a horizontal split inside a horizontal split (same for vertical)",
                );
                ui.checkbox(&mut self.simplification.join_nested_linear_containers, "");
                ui.end_row();

                ui.label("Tab bar height")
                    .on_hover_text("Height of the bar with the tab titles, in points");
                ui.add(
                    egui::DragValue::new(&mut self.tab_bar_height)
                        .range(12.0..=64.0)
                        .suffix(" pt"),
                );
                ui.end_row();

                ui.label("Gap width")
                    .on_hover_text("Space between split tiles, in points");
                ui.add(
                    egui::DragValue::new(&mut self.gap_width)
                        .range(0.0..=20.0)
                        .suffix(" pt"),
                );
                ui.end_row();
            });
    }
}

impl egui_tiles::Behavior<Box<dyn Widget>> for Behavior {
    fn pane_ui(
        &mut self,
        ui: &mut Ui,
        tile_id: TileId,
        widget: &mut Box<dyn Widget>,
    ) -> UiResponse {
        let id = Id::new("ve_app_widget").with(tile_id);
        Frame::NONE
            .inner_margin(4.0)
            .show(ui, |ui| widget.ui(ui, &self.cx, id));
        UiResponse::None
    }

    fn tab_title_for_pane(&mut self, widget: &Box<dyn Widget>) -> WidgetText {
        widget.widget_text()
    }

    fn on_tab_button(
        &mut self,
        tiles: &mut Tiles<Box<dyn Widget>>,
        tile_id: TileId,
        response: Response,
    ) -> Response {
        let busy = matches!(tiles.get(tile_id), Some(Tile::Pane(w)) if !w.is_closeable());
        let text = if busy {
            "Drag to move or split. Busy: can't be closed right now."
        } else {
            "Drag to move it or onto an edge to split the view"
        };
        response.on_hover_text(text)
    }

    fn is_tab_closable(&self, tiles: &Tiles<Box<dyn Widget>>, tile_id: TileId) -> bool {
        match tiles.get(tile_id) {
            Some(Tile::Pane(w)) => w.is_closeable(),
            Some(Tile::Container(_)) => false,
            None => true,
        }
    }

    fn top_bar_right_ui(
        &mut self,
        _tiles: &Tiles<Box<dyn Widget>>,
        ui: &mut Ui,
        tile_id: TileId,
        _tabs: &egui_tiles::Tabs,
        _scroll_offset: &mut f32,
    ) {
        ui.add_space(4.0);
        let filter = self.widget_filter;
        let r = ui.menu_button("➕", |ui| {
            if let Some(info) = crate::widget_menu::ui(ui, filter) {
                self.open_request = Some((info, Some(tile_id)));
                ui.close_kind(UiKind::Menu);
            }
        });
        r.response.on_hover_text("Open a widget as a new tab here");
    }

    fn tab_bar_height(&self, _style: &egui::Style) -> f32 {
        self.tab_bar_height
    }

    fn gap_width(&self, _style: &egui::Style) -> f32 {
        self.gap_width
    }

    fn simplification_options(&self) -> SimplificationOptions {
        self.simplification
    }
}

/// Insert `widget` as a tab of `parent` (a tabs container) or next to the root's tabs, and activate it.
pub(crate) fn open(tree: &mut Tree, widget: Box<dyn Widget>, parent: Option<TileId>) {
    let pane = tree.tiles.insert_pane(widget);
    let target = parent.or(tree.root());
    match target.and_then(|id| tree.tiles.get_mut(id)) {
        Some(Tile::Container(egui_tiles::Container::Tabs(tabs))) => {
            tabs.add_child(pane);
            tabs.set_active(pane);
        }
        Some(Tile::Container(container)) => container.add_child(pane),
        // Empty tree (everything closed) or a lone pane as root: start a new tabs container.
        Some(Tile::Pane(_)) | None => {
            let mut children = vec![pane];
            if let Some(old_root) = tree.root() {
                children.insert(0, old_root);
            }
            let tabs = tree.tiles.insert_tab_tile(children);
            if let Some(Tile::Container(egui_tiles::Container::Tabs(t))) = tree.tiles.get_mut(tabs)
            {
                t.set_active(pane);
            }
            tree.root = Some(tabs);
        }
    }
}
