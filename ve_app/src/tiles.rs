//! Widgets as egui_tiles panes (WID-3), each in its frame (FRM-7).

use egui::{Id, Response, Ui, UiKind, WidgetText};
use egui_tiles::{SimplificationOptions, Tile, TileId, Tiles, UiResponse};
use ve_frame::{FrameAction, Moves, Slot, SlotId, ViewportSlot, WidgetFrame};
use ve_widget::context::Context;
use ve_widget::{Widget, WidgetInfo};

use crate::state::{Tree, Widgets};

/// The tile settings that outlive a frame, and what was picked in an open-widget menu.
pub(crate) struct Behavior {
    pub(crate) cx: Context,
    pub(crate) simplification: SimplificationOptions,
    pub(crate) tab_bar_height: f32,
    pub(crate) gap_width: f32,
    pub(crate) widget_filter: fn(&WidgetInfo) -> bool,
    /// Picked in a menu this frame: open it in that OS window, in that tabs container or next to the others.
    pub(crate) open_request: Option<(&'static WidgetInfo, Option<TileId>, ViewportSlot)>,
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

/// The egui_tiles behavior of one frame for one OS window: draws the widgets in their frames, their tab titles
/// and the ➕ open-widget menu of each tab bar, and collects what the frames' menus picked.
pub(crate) struct Pass<'a> {
    pub(crate) behavior: &'a mut Behavior,
    pub(crate) widgets: &'a mut Widgets,
    pub(crate) moves: &'a Moves,
    pub(crate) picked: &'a mut Vec<(SlotId, FrameAction)>,
    pub(crate) viewport: ViewportSlot,
}

impl Pass<'_> {
    fn busy(&self, tiles: &Tiles<SlotId>, tile_id: TileId) -> bool {
        match tiles.get(tile_id) {
            Some(Tile::Pane(id)) => self
                .widgets
                .get(*id)
                .is_some_and(|s| !s.widget.is_closeable()),
            _ => false,
        }
    }
}

impl egui_tiles::Behavior<SlotId> for Pass<'_> {
    fn pane_ui(&mut self, ui: &mut Ui, _tile_id: TileId, id: &mut SlotId) -> UiResponse {
        if let Some(slot) = self.widgets.get_mut(*id) {
            let action = framed(ui, slot, &self.behavior.cx, self.moves, true);
            self.picked.extend(action.map(|a| (*id, a)));
        }
        UiResponse::None
    }

    fn tab_title_for_pane(&mut self, id: &SlotId) -> WidgetText {
        match self.widgets.get(*id) {
            Some(slot) => slot.widget.widget_text(),
            None => "closed".into(),
        }
    }

    fn on_tab_button(
        &mut self,
        tiles: &mut Tiles<SlotId>,
        tile_id: TileId,
        response: Response,
    ) -> Response {
        let text = if self.busy(tiles, tile_id) {
            "Drag to move or split. Busy: can't be closed right now."
        } else {
            "Drag to move it or onto an edge to split the view"
        };
        response.on_hover_text(text)
    }

    fn is_tab_closable(&self, tiles: &Tiles<SlotId>, tile_id: TileId) -> bool {
        match tiles.get(tile_id) {
            Some(Tile::Pane(_)) => !self.busy(tiles, tile_id),
            Some(Tile::Container(_)) => false,
            None => true,
        }
    }

    fn on_tab_close(&mut self, tiles: &mut Tiles<SlotId>, tile_id: TileId) -> bool {
        if let Some(Tile::Pane(id)) = tiles.get(tile_id) {
            self.widgets.remove(*id);
        }
        true
    }

    fn top_bar_right_ui(
        &mut self,
        _tiles: &Tiles<SlotId>,
        ui: &mut Ui,
        tile_id: TileId,
        _tabs: &egui_tiles::Tabs,
        _scroll_offset: &mut f32,
    ) {
        ui.add_space(4.0);
        let filter = self.behavior.widget_filter;
        let r = ui.menu_button("➕", |ui| {
            if let Some(info) = crate::widget_menu::ui(ui, filter) {
                self.behavior.open_request = Some((info, Some(tile_id), self.viewport));
                ui.close_kind(UiKind::Menu);
            }
        });
        r.response.on_hover_text("Open a widget as a new tab here");
    }

    fn tab_bar_height(&self, _style: &egui::Style) -> f32 {
        self.behavior.tab_bar_height
    }

    fn gap_width(&self, _style: &egui::Style) -> f32 {
        self.behavior.gap_width
    }

    fn simplification_options(&self) -> SimplificationOptions {
        self.behavior.simplification
    }
}

/// Draw a widget in its frame: what [`Widget::frame_spec`] gives plus the menu. In a tile the frame fills the
/// pane without a border of its own and leaves the title to the tab; floating, it is the whole window.
pub(crate) fn framed(
    ui: &mut Ui,
    slot: &mut Slot<Box<dyn Widget>>,
    cx: &Context,
    moves: &Moves,
    in_tile: bool,
) -> Option<FrameAction> {
    let Slot { id, widget, frame } = slot;
    let spec = widget.frame_spec();
    let closeable = if widget.is_closeable() {
        Ok(())
    } else {
        Err("the widget is busy".to_owned())
    };
    let moves = moves.clone().close(closeable);
    let widget_id = Id::new("ve_app_widget").with(id.0);
    let mut f = WidgetFrame::new(widget.widget_text().text())
        .id_salt(("ve_app", id.0))
        .moves(&moves)
        .fill(in_tile)
        .bare(in_tile)
        .title_shown(!in_tile)
        .about(spec.about);
    if let Some((status, why)) = spec.led {
        f = f.led(status, why);
    }
    if let Some(sizes) = spec.sizes {
        f = f.sizes(sizes);
    }
    if !spec.stats.is_empty() {
        let stats = spec.stats;
        f = f.info(move |ui| {
            egui::Grid::new("ve_app_widget_stats")
                .num_columns(2)
                .show(ui, |ui| {
                    for (label, value) in &stats {
                        ui.weak(label);
                        ui.label(value);
                        ui.end_row();
                    }
                });
        });
    }
    f.show(ui, frame, |ui, size| {
        widget.ui_sized(ui, cx, widget_id, size)
    })
    .action
}

/// Make `tree` show exactly the widgets docked in `viewport`: tiles of widgets that left (popped out, sent
/// away, closed) go, widgets that arrived get a tab.
pub(crate) fn sync(tree: &mut Tree, widgets: &Widgets, viewport: ViewportSlot) {
    let want = widgets.ids(viewport, false);
    let have: Vec<(TileId, SlotId)> = tree
        .tiles
        .iter()
        .filter_map(|(tile_id, tile)| match tile {
            Tile::Pane(id) => Some((*tile_id, *id)),
            Tile::Container(_) => None,
        })
        .collect();
    for (tile_id, id) in &have {
        if !want.contains(id) {
            tree.remove_recursively(*tile_id);
        }
    }
    for id in want {
        if !have.iter().any(|(_, h)| *h == id) {
            open(tree, id, None);
        }
    }
}

/// Insert `widget` as a tab of `parent` (a tabs container) or next to the root's tabs, and activate it.
pub(crate) fn open(tree: &mut Tree, widget: SlotId, parent: Option<TileId>) {
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
