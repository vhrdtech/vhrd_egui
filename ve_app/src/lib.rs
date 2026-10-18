//! The application shell every vhrd egui app is built on: menu bar, a tiled area of
//! [`ve_widget::Widget`]s, the About / Settings / Debug windows, a status bar and a layout that survives
//! restarts.
//!
//! ```ignore
//! fn main() -> eframe::Result {
//!     let cx = ve_widget::context::Context::new(Box::new(MyAppState::default()));
//!     eframe::run_native("My app", Default::default(), Box::new(|cc| {
//!         let options = ve_app::ShellOptions::new("My app", ve_basics::build_info!())
//!             .default_widgets(|| vec![Box::new(MyWidget::default())]);
//!         Ok(Box::new(ve_app::Shell::new(cc, cx, options)))
//!     }))
//! }
//! ```
//!
//! [`Shell::new`] turns on the VHRD theme (`ve_theme::setup`) and non-selectable labels
//! (`ve_basics::setup_labels`), restores the saved layout and calls [`Widget::init`] on every widget.
//! Widgets register themselves with `inventory::submit!(WidgetInfo { .. })` and show up in
//! *View → Open widget* (and the ➕ of every tab bar), grouped by [`ve_widget::WidgetInfo::group_path`] (WID-3).
//!
//! - Menus (WID-8): *File* (Settings, Quit), *View* (Open widget, side panel, reset layout), *Windows*
//!   (Debug, center), *Help* (About, reset UI memory); theme switch on the right.
//! - Windows (WID-8): About (app name, description, build info), Settings (theme plus the app's own
//!   [`ShellOptions::settings`]), Debug (tile tree, tab bar settings, egui inspection).
//! - Quit (WID-9): when a widget says it is not closeable ([`Widget::is_closeable`]), quitting asks first and
//!   names the busy widgets.
//! - Frames (FRM-7): every widget sits in a `ve_frame::WidgetFrame`. Its ☰ menu pops the widget out as a
//!   floating window and back into the tiles, sends it to another OS window (which has tiles of its own), or to
//!   another machine when the app gives a transport ([`ShellOptions::remote`], [`Shell::receive`]). What
//!   [`Widget::frame_spec`] returns adds an LED, an ⓘ and the size switch.
//! - [`Repainter`]: a cheap handle background tasks use to wake the UI.
//! - [`prelude`]: what a widget implementation needs in one `use`.
//!
//! Persistence: with [`ShellOptions::persist`] (default on) the tile tree, side panel and open windows go
//! to eframe storage. A saved layout is dropped (with a log line) when it no longer deserializes — a widget
//! type was renamed or removed — or when [`ShellOptions::layout_version`] changed.

mod close_dialog;
pub mod prelude;
mod repainter;
mod state;
mod tiles;
mod widget_menu;
mod windows;

pub use repainter::Repainter;
pub use widget_menu::registered_widgets;

use egui::{CentralPanel, MenuBar, Panel, Ui};
use ve_basics::BuildInfo;
use ve_frame::{Applied, FrameAction, FrameState, Parcel, Remote, SlotId, ViewportSlot};
use ve_widget::Widget;
use ve_widget::context::Context;

use crate::close_dialog::CloseDialog;
use crate::state::State;
use crate::tiles::{Behavior, Pass};

/// Draws a part of the shell the app fills in (side panel, app settings).
pub type UiFn = Box<dyn FnMut(&mut Ui, &Context)>;

/// What the shell needs to know about the app. Build with [`ShellOptions::new`] and the setters.
pub struct ShellOptions {
    /// Shown in the About window, the quit dialog and the status bar.
    pub app_name: String,
    /// One or two sentences for the About window: what the app is for.
    pub description: String,
    /// The app's build info, `ve_basics::build_info!()` in the app crate.
    pub build_info: BuildInfo,
    /// Save and restore the layout through eframe storage (needs a persistence-enabled eframe app id).
    pub persist: bool,
    /// Bump when the default layout or the meaning of saved widgets changes: saved layouts with another
    /// number are dropped at startup.
    pub layout_version: u32,
    /// The widgets of a fresh layout (first start, *View → Reset layout*), opened as tabs.
    pub default_widgets: fn() -> Vec<Box<dyn Widget>>,
    /// Which registered widgets the open-widget menus offer, e.g. by [`ve_widget::WidgetInfo::tags`].
    pub widget_filter: fn(&ve_widget::WidgetInfo) -> bool,
    /// Contents of the collapsible left side panel; no panel (and no toggle) when `None`.
    pub side_panel: Option<UiFn>,
    /// The app's part of the Settings window, below the theme switch.
    pub settings: Option<UiFn>,
    /// The app's transport to other machines: with it the widgets' menus offer *Send to another machine*;
    /// without it the entry is disabled and says why. The receiving side calls [`Shell::receive`].
    pub remote: Option<Box<dyn Remote>>,
}

impl ShellOptions {
    /// Options with a name and build info; persistence on, no default widgets, every widget in the menus.
    pub fn new(app_name: impl Into<String>, build_info: BuildInfo) -> Self {
        Self {
            app_name: app_name.into(),
            description: String::new(),
            build_info,
            persist: true,
            layout_version: 0,
            default_widgets: Vec::new,
            widget_filter: |_| true,
            side_panel: None,
            settings: None,
            remote: None,
        }
    }

    /// See [`Self::description`].
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// See [`Self::persist`].
    pub fn persist(mut self, persist: bool) -> Self {
        self.persist = persist;
        self
    }

    /// See [`Self::layout_version`].
    pub fn layout_version(mut self, version: u32) -> Self {
        self.layout_version = version;
        self
    }

    /// See [`Self::default_widgets`].
    pub fn default_widgets(mut self, widgets: fn() -> Vec<Box<dyn Widget>>) -> Self {
        self.default_widgets = widgets;
        self
    }

    /// See [`Self::widget_filter`].
    pub fn widget_filter(mut self, filter: fn(&ve_widget::WidgetInfo) -> bool) -> Self {
        self.widget_filter = filter;
        self
    }

    /// See [`Self::side_panel`].
    pub fn side_panel(mut self, ui: impl FnMut(&mut Ui, &Context) + 'static) -> Self {
        self.side_panel = Some(Box::new(ui));
        self
    }

    /// See [`Self::settings`].
    pub fn settings(mut self, ui: impl FnMut(&mut Ui, &Context) + 'static) -> Self {
        self.settings = Some(Box::new(ui));
        self
    }

    /// See [`Self::remote`].
    pub fn remote(mut self, remote: impl Remote + 'static) -> Self {
        self.remote = Some(Box::new(remote));
        self
    }
}

/// The app shell. Implements [`eframe::App`]; apps with more around it call [`Shell::ui`] themselves.
pub struct Shell {
    cx: Context,
    options: ShellOptions,
    state: State,
    behavior: Behavior,
    close: CloseDialog,
    repainter: Repainter,
    /// How the last send to another machine went, for the status bar.
    notice: String,
}

impl Shell {
    /// Set up theme and labels, restore the saved layout (if [`ShellOptions::persist`]) and init every widget.
    pub fn new(cc: &eframe::CreationContext<'_>, cx: Context, options: ShellOptions) -> Self {
        ve_theme::setup(&cc.egui_ctx);
        ve_basics::setup_labels(&cc.egui_ctx);
        let storage = if options.persist { cc.storage } else { None };
        let state = State::load(storage, &options);
        let mut shell = Self {
            behavior: Behavior::new(cx.clone(), options.widget_filter),
            cx,
            options,
            state,
            close: CloseDialog::default(),
            repainter: Repainter::new(cc.egui_ctx.clone()),
            notice: String::new(),
        };
        shell.init_widgets();
        shell
    }

    /// A handle background tasks use to request a repaint.
    pub fn repainter(&self) -> Repainter {
        self.repainter.clone()
    }

    /// The shared context handed to every widget.
    pub fn context(&self) -> &Context {
        &self.cx
    }

    /// Open `widget` as a new tab next to the others and make it the active one; calls [`Widget::init`].
    pub fn open_widget(&mut self, mut widget: Box<dyn Widget>) {
        widget.init(&self.cx);
        let id = self.state.widgets.add(widget);
        tiles::open(&mut self.state.tree, id, None);
    }

    /// Open a widget that another machine sent ([`ShellOptions::remote`] on that side): it is rebuilt from
    /// what was saved of it, [`Widget::init`] runs, and it shows up as a new tab with its size switch as it
    /// was. `Err` when this build doesn't know the widget type or can't read it.
    pub fn receive(&mut self, parcel: Parcel) -> Result<(), String> {
        let mut widget: Box<dyn Widget> = serde_json::from_str(&parcel.widget)
            .map_err(|e| format!("{}: unreadable widget, {e}", parcel.title))?;
        widget.init(&self.cx);
        self.state.widgets.add_with(widget, parcel.frame_state());
        Ok(())
    }

    /// Every open widget, in the order they were opened: tiles, floating windows and other OS windows alike.
    pub fn widgets(&self) -> impl Iterator<Item = &dyn Widget> {
        self.state.widgets.iter().map(|s| s.widget.as_ref())
    }

    /// Where each open widget sits and what its size switch is set to, in the order of [`Self::widgets`].
    pub fn frames(&self) -> impl Iterator<Item = &FrameState> {
        self.state.widgets.iter().map(|s| &s.frame)
    }

    /// Draw the whole shell into the root `ui`: menu bar, status bar, side panel, windows, tiles, floating
    /// widgets, the other OS windows, quit dialog.
    pub fn ui(&mut self, ui: &mut Ui) {
        for slot in self.state.widgets.iter_mut() {
            slot.widget.logic(&self.cx);
        }

        Panel::top("ve_app_menu_bar").show(ui, |ui| {
            MenuBar::new().ui(ui, |ui| self.menu_bar(ui));
        });
        Panel::bottom("ve_app_status_bar").show(ui, |ui| self.status_bar(ui));

        if let Some(side_panel) = &mut self.options.side_panel {
            let cx = &self.cx;
            Panel::left("ve_app_side_panel")
                .resizable(true)
                .show_collapsible(ui, &mut self.state.side_panel_open, |ui| side_panel(ui, cx));
        }

        windows::show(
            ui.ctx(),
            &mut self.state,
            &mut self.behavior,
            &mut self.options,
            &self.cx,
        );

        let moves = self.state.widgets.moves(self.options.remote.as_deref());
        let mut picked = Vec::new();
        let State {
            widgets,
            tree,
            extra,
            ..
        } = &mut self.state;
        let (behavior, cx) = (&mut self.behavior, &self.cx);
        tiles::sync(tree, widgets, ViewportSlot::MAIN);
        CentralPanel::default().show(ui, |ui| {
            if tree.is_empty() {
                empty_hint(ui);
            } else {
                let mut pass = Pass {
                    behavior: &mut *behavior,
                    widgets: &mut *widgets,
                    moves: &moves,
                    picked: &mut picked,
                    viewport: ViewportSlot::MAIN,
                };
                tree.ui(&mut pass, ui);
            }
            let floating = widgets.show_windows(ui, ViewportSlot::MAIN, |ui, slot| {
                tiles::framed(ui, slot, cx, &moves, false)
            });
            picked.extend(floating);
        });

        // The other OS windows: each has tiles of its own and lives while a widget is in it.
        let open = widgets.viewports();
        extra.retain(|n, _| open.contains(&ViewportSlot(*n)));
        widgets.show_viewports(ui.ctx(), &self.options.app_name, |widgets, ui, viewport| {
            let tree = extra
                .entry(viewport.0)
                .or_insert_with(|| egui_tiles::Tree::empty(format!("ve_app_tiles_{}", viewport.0)));
            tiles::sync(tree, widgets, viewport);
            if !tree.is_empty() {
                let mut pass = Pass {
                    behavior: &mut *behavior,
                    widgets: &mut *widgets,
                    moves: &moves,
                    picked: &mut picked,
                    viewport,
                };
                tree.ui(&mut pass, ui);
            }
            let floating = widgets.show_windows(ui, viewport, |ui, slot| {
                tiles::framed(ui, slot, cx, &moves, false)
            });
            picked.extend(floating);
        });

        for (id, action) in picked {
            self.apply(id, action);
        }
        if let Some((info, parent, viewport)) = self.behavior.open_request.take() {
            let mut widget = (info.spawn_fn)();
            widget.init(&self.cx);
            let mut frame = FrameState::default();
            frame.placement.viewport = viewport;
            let id = self.state.widgets.add_with(widget, frame);
            let tree = match viewport.is_main() {
                true => Some(&mut self.state.tree),
                false => self.state.extra.get_mut(&viewport.0),
            };
            if let Some(tree) = tree {
                tiles::open(tree, id, parent);
            }
        }

        let busy = busy_widgets(&self.state);
        self.close.ui(ui.ctx(), &self.options.app_name, busy);
    }

    /// Carry out what a widget's menu picked.
    fn apply(&mut self, id: SlotId, action: FrameAction) {
        match self.state.widgets.apply(id, action) {
            Applied::Moved | Applied::Gone | Applied::Tidy => {}
            Applied::Close => {
                let closeable = self.state.widgets.get(id).map(|s| s.widget.is_closeable());
                if closeable == Some(true) {
                    self.state.widgets.remove(id);
                }
            }
            Applied::Send(machine) => {
                let Some(remote) = self.options.remote.as_deref_mut() else {
                    return;
                };
                let sent = self.state.widgets.send(id, &machine, remote, |w| {
                    let json = serde_json::to_string(w).map_err(|e| e.to_string())?;
                    Ok((w.widget_text().text().to_owned(), json))
                });
                self.notice = match sent {
                    Ok(()) => format!("Widget sent to {machine}"),
                    Err(e) => format!("Widget not sent to {machine}: {e}"),
                };
            }
        }
    }

    fn init_widgets(&mut self) {
        for slot in self.state.widgets.iter_mut() {
            slot.widget.init(&self.cx);
        }
    }

    fn reset_layout(&mut self) {
        let (widgets, tree) = state::default_layout(&self.options);
        self.state.widgets = widgets;
        self.state.tree = tree;
        self.state.extra.clear();
        self.init_widgets();
    }

    fn menu_bar(&mut self, ui: &mut Ui) {
        if self.options.side_panel.is_some() {
            ui.toggle_value(&mut self.state.side_panel_open, "☰")
                .on_hover_text("Show or hide the side panel");
            ui.separator();
        }
        ui.menu_button("File", |ui| {
            ui.toggle_value(&mut self.state.windows.settings, "Settings…")
                .on_hover_text("Open the settings window: theme and app settings");
            if !cfg!(target_arch = "wasm32") {
                ui.separator();
                if ui
                    .button("Quit")
                    .on_hover_text(format!(
                        "Close {}; asks first when a widget is busy",
                        self.options.app_name
                    ))
                    .clicked()
                {
                    self.close.request(ui.ctx(), busy_widgets(&self.state));
                    ui.close();
                }
            }
        });
        ui.menu_button("View", |ui| {
            ui.menu_button("Open widget", |ui| {
                if let Some(info) = widget_menu::ui(ui, self.options.widget_filter) {
                    self.behavior.open_request = Some((info, None, ViewportSlot::MAIN));
                }
            })
            .response
            .on_hover_text("Open a new widget as a tab in the main area");
            if self.options.side_panel.is_some() {
                ui.toggle_value(&mut self.state.side_panel_open, "Side panel")
                    .on_hover_text("Show or hide the side panel on the left");
            }
            ui.separator();
            if ui
                .button("Reset layout")
                .on_hover_text(
                    "Close every widget and open the default ones again; widget settings are lost",
                )
                .clicked()
            {
                self.reset_layout();
                ui.close();
            }
        });
        ui.menu_button("Windows", |ui| {
            ui.toggle_value(&mut self.state.windows.debug, "Debug")
                .on_hover_text("Tile tree, tab bar settings and egui internals, for developers");
            if ui
                .button("Center on screen")
                .on_hover_text("Move the app window to the middle of the screen")
                .clicked()
            {
                if let Some(cmd) = egui::ViewportCommand::center_on_screen(ui.ctx()) {
                    ui.ctx().send_viewport_cmd(cmd);
                }
                ui.close();
            }
        });
        ui.menu_button("Help", |ui| {
            ui.toggle_value(&mut self.state.windows.about, "About")
                .on_hover_text(format!(
                    "What {} is, its version and build",
                    self.options.app_name
                ));
            ui.separator();
            if ui
                .button("Reset UI memory")
                .on_hover_text(
                    "Forget window positions, scroll offsets and collapsed sections; the layout stays",
                )
                .clicked()
            {
                ui.ctx().memory_mut(|m| *m = Default::default());
                ui.close();
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            egui::widgets::global_theme_preference_switch(ui);
        });
    }

    fn status_bar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label(&self.options.app_name)
                .on_hover_text(&self.options.description);
            if !self.notice.is_empty() {
                ui.separator();
                ui.weak(&self.notice)
                    .on_hover_text("How the last send of a widget to another machine went");
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let r = ve_basics::build_info_label(ui, &self.options.build_info);
                ve_basics::hover_link(ui, "ve_app::build_info", &r);
            });
        });
    }
}

impl eframe::App for Shell {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        Shell::ui(self, ui);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        if self.options.persist {
            self.state.save(storage);
        }
    }

    fn persist_egui_memory(&self) -> bool {
        self.options.persist
    }

    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        visuals.panel_fill.to_normalized_gamma_f32()
    }
}

/// Titles of the open widgets that say they must not be closed now.
fn busy_widgets(state: &State) -> Vec<String> {
    state
        .widgets
        .iter()
        .filter(|s| !s.widget.is_closeable())
        .map(|s| s.widget.widget_text().text().to_owned())
        .collect()
}

fn empty_hint(ui: &mut Ui) {
    ui.centered_and_justified(|ui| {
        ui.weak("No widgets open. View → Open widget adds one.");
    });
}
