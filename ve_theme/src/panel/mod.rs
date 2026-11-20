//! The side panel and its rail (CHR-6): the chrome every app's tree, device list or inspector sits in.
//!
//! One model, two widths: a list of groups with rows ([`Group`], [`Row`]) that collapses to its own rail of
//! the groups' icons, their lights and unread counts.
//!
//! - **Header:** the title, the app's own actions, the ⋯ menu (dock left, dock right, float, collapse to
//!   rail, hide, reset width) and the collapse button.
//! - **Filter:** `/` focuses it; matched letters get a `warning.soft` mark, groups keep their heads with
//!   "0 of 2", rows that do not match fold into "Not matching N" (nothing vanishes), the footer says how many
//!   match.
//! - **Rows:** icon, label (cut with an ellipsis, whole in the tooltip), then right-aligned a mono value, a
//!   status light ([`LightStyle`](crate::LightStyle), S29) and a count (an unread count is a `primary` pill). Hover, selected (fill
//!   and a 2 px `focus` bar), keyboard focus (the 1 px inset ring), disabled (muted, the tooltip says why).
//!   ← and → fold and open, ↑ and ↓ move, Enter picks.
//! - **Resize:** the inner edge; on hover a 3 px `focus` line and a grip, the width read out while dragging,
//!   double click resets, Ctrl+Alt+← / → step by 16, under the minimum it snaps to the rail.
//! - **Place** ([`SidePanelState`], S7): docked left or right, floating (a window with a drag head that docks
//!   again when dropped on an edge), collapsed to its rail (Ctrl+B), hidden behind a vertical tab on its edge
//!   (Ctrl+Shift+B).
//! - **Forms** ([`PanelForm`], S6): the list that collapses to its rail (B2, the pick), a rail of panel views
//!   beside the list (B1), the rail only with hover cards (B3).
//! - **Link** ([`RevealLink`]): a click on a row reveals it in the view beside the panel; a selection in the
//!   view reveals it in the tree; hover lights both; [`Follow`] (S28) keeps one in step with the other.
//!
//! Every size and colour comes from the [`Tokens`] of the `Ui`'s theme (`Tokens::chrome` for the density).
//! Fills, bars and the focus ring are whole-pixel rectangles, never stroked hairlines.
//!
//! ```ignore
//! let r = SidePanel::new("tracks", "Tracks").noun("tracks").link(&mut link)
//!     .show(ui, &mut state.tracks, |list| {
//!         list.group(Group::new("dev0 · bench rig"), |g| {
//!             g.row(Row::new(Key::Can0, "can0").icon("∿").value("0x123").light(Tone::Good).selected(sel));
//!         });
//!     });
//! if let Some(k) = r.clicked { selection = k; }
//! ```

mod demo;
mod list;
mod model;
mod paint;
mod parts;
mod rail;
mod reveal;
mod state;

use std::sync::Arc;

use egui::{Frame, Id, Key, KeyboardShortcut, Modifiers, Rect, Ui, UiBuilder, pos2};

pub use demo::{PanelGallery, set_density};
pub use model::{Count, Group, Icon, IconPainter, PanelBuilder, Row, RowKey};
pub use reveal::RevealLink;
pub use state::{Follow, PanelForm, PanelSettings, Shown, Side, SidePanelState};

use crate::Tokens;
use rail::RailItem;

/// A panel view on the views rail (form B1): an icon and a name.
#[derive(Clone, Debug)]
pub struct View {
    icon: Icon,
    name: String,
    unread: u32,
}

impl View {
    /// A view named `name` with `icon`.
    pub fn new(icon: impl Into<Icon>, name: impl Into<String>) -> Self {
        Self {
            icon: icon.into(),
            name: name.into(),
            unread: 0,
        }
    }

    /// An unread count on its rail icon.
    pub fn unread(mut self, n: u32) -> Self {
        self.unread = n;
        self
    }
}

/// What the person did in the panel this frame.
#[derive(Clone, Debug)]
pub struct PanelResponse<K> {
    /// A row picked (click, or Enter on a focused row).
    pub clicked: Option<K>,
    /// A row double-clicked.
    pub double_clicked: Option<K>,
    /// The row under the pointer.
    pub hovered: Option<K>,
    /// A rail item clicked in form B3 (the group's index); the other forms open the panel at the group.
    pub rail_clicked: Option<usize>,
    /// How much of the panel shows after this frame.
    pub shown: Shown,
    /// Whether it floats.
    pub floating: bool,
    /// The panel's rectangle, the edge tab's while hidden.
    pub rect: Rect,
}

impl<K> PanelResponse<K> {
    fn new(state: &SidePanelState) -> Self {
        Self {
            clicked: None,
            double_clicked: None,
            hovered: None,
            rail_clicked: None,
            shown: state.shown,
            floating: state.floating,
            rect: Rect::NOTHING,
        }
    }
}

type Actions<'a> = Box<dyn FnOnce(&mut Ui) + 'a>;
type RowMenu<'a, K> = Box<dyn FnMut(&mut Ui, &K) + 'a>;

/// A side panel: build it each frame, fill it from a closure, keep its [`SidePanelState`] in the app's visual
/// state. See the [module docs](self).
pub struct SidePanel<'a, K> {
    id: Id,
    title: String,
    noun: String,
    filter_hint: Option<String>,
    footer: Option<String>,
    filter: bool,
    settings: PanelSettings,
    views: Vec<View>,
    actions: Option<Actions<'a>>,
    link: Option<&'a mut RevealLink<K>>,
    row_menu: Option<RowMenu<'a, K>>,
    icon: Option<Icon>,
    custom: Option<Actions<'a>>,
    collapse_key: Option<KeyboardShortcut>,
    hide_key: Option<KeyboardShortcut>,
}

impl<'a, K: RowKey> SidePanel<'a, K> {
    /// A panel keyed by `id_salt` (unique per window), titled `title`.
    pub fn new(id_salt: impl std::hash::Hash + std::fmt::Debug, title: impl Into<String>) -> Self {
        Self {
            id: Id::new(("ve_theme::SidePanel", id_salt)),
            title: title.into(),
            noun: "rows".to_owned(),
            filter_hint: None,
            footer: None,
            filter: true,
            settings: PanelSettings::default(),
            views: Vec::new(),
            actions: None,
            link: None,
            row_menu: None,
            icon: None,
            custom: None,
            collapse_key: Some(KeyboardShortcut::new(Modifiers::COMMAND, Key::B)),
            hide_key: Some(KeyboardShortcut::new(
                Modifiers::COMMAND | Modifiers::SHIFT,
                Key::B,
            )),
        }
    }

    /// What the rows are, plural, for the footer and the filter's hint ("tracks").
    pub fn noun(mut self, noun: impl Into<String>) -> Self {
        self.noun = noun.into();
        self
    }

    /// The filter field's hint (default "Filter" and the noun).
    pub fn filter_hint(mut self, hint: impl Into<String>) -> Self {
        self.filter_hint = Some(hint.into());
        self
    }

    /// The footer's text while not filtering ("14 tracks · 3 runs · 2 groups"; default the row count).
    pub fn footer(mut self, text: impl Into<String>) -> Self {
        self.footer = Some(text.into());
        self
    }

    /// Show the filter field (default on).
    pub fn filter(mut self, on: bool) -> Self {
        self.filter = on;
        self
    }

    /// Form, follow and lights (S6, S28, S29).
    pub fn settings(mut self, settings: PanelSettings) -> Self {
        self.settings = settings;
        self
    }

    /// The panel views on the views rail (form B1); the closure reads the picked one with
    /// [`PanelBuilder::view`].
    pub fn views(mut self, views: impl IntoIterator<Item = View>) -> Self {
        self.views = views.into_iter().collect();
        self
    }

    /// The panel's own actions in its header, right of the title, laid out right to left.
    pub fn actions(mut self, add: impl FnOnce(&mut Ui) + 'a) -> Self {
        self.actions = Some(Box::new(add));
        self
    }

    /// Link the tree to a view: a click reveals the row there, a reveal from there shows it here.
    pub fn link(mut self, link: &'a mut RevealLink<K>) -> Self {
        self.link = Some(link);
        self
    }

    /// A context menu on every row (right click), given the row's key.
    pub fn row_menu(mut self, menu: impl FnMut(&mut Ui, &K) + 'a) -> Self {
        self.row_menu = Some(Box::new(menu));
        self
    }

    /// The keys that collapse to the rail and hide (default Ctrl+B, Ctrl+Shift+B; `None` for a second panel).
    pub fn keys(
        mut self,
        collapse: Option<KeyboardShortcut>,
        hide: Option<KeyboardShortcut>,
    ) -> Self {
        self.collapse_key = collapse;
        self.hide_key = hide;
        self
    }

    /// The panel's icon on its rail when it holds free content ([`Self::show_ui`]).
    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Draw the panel with free content under its header instead of rows: an inspector of property parts,
    /// an app's own controls. It scrolls; collapsed, its rail holds one item, the panel's [`Self::icon`].
    pub fn show_ui(
        mut self,
        ui: &mut Ui,
        state: &mut SidePanelState,
        add: impl FnOnce(&mut Ui) + 'a,
    ) -> PanelResponse<K> {
        self.custom = Some(Box::new(add));
        self.show(ui, state, |_| {})
    }

    /// Draw the panel at its place in `ui` (the window's root `Ui`, or the part of it the panel may take),
    /// filled by `add`.
    pub fn show(
        mut self,
        ui: &mut Ui,
        state: &mut SidePanelState,
        add: impl FnOnce(&mut PanelBuilder<K>),
    ) -> PanelResponse<K> {
        let t = Tokens::of_ui(ui);
        state.view = state.view.min(self.views.len().saturating_sub(1));
        let mut b = PanelBuilder::new(state.view);
        add(&mut b);
        let groups = b.groups;
        self.handle_keys(ui, state);
        let mut reveal = None;
        if let Some(link) = self.link.as_deref_mut()
            && let Some(k) = link.take_for_tree_at(ui.input(|i| i.time))
        {
            unfold_to(state, &groups, &k);
            if state.shown != Shown::Open && self.settings.form != PanelForm::RailOnly {
                state.shown = Shown::Open;
            }
            reveal = Some(k);
        }
        let bounds = ui.available_rect_before_wrap();
        let mut out = PanelResponse::new(state);
        let shown = fit_shown(&t, state, bounds);
        out.rect = match (self.settings.form, shown) {
            (_, Shown::Hidden) => parts::edge_tab(ui, &t, self.id, &self.title, state, bounds),
            (PanelForm::RailOnly, _) | (_, Shown::Rail) => {
                self.docked_rail(ui, &t, state, &groups, &mut out)
            }
            (_, Shown::Open) if state.floating => {
                parts::floating(self, ui, &t, state, &groups, &mut out, reveal, bounds)
            }
            (_, Shown::Open) => self.docked(ui, &t, state, &groups, &mut out, reveal, bounds),
        };
        out.shown = state.shown;
        out.floating = state.floating;
        out
    }

    fn handle_keys(&self, ui: &Ui, state: &mut SidePanelState) {
        if let Some(k) = self.hide_key
            && ui.input_mut(|i| i.consume_shortcut(&k))
        {
            state.shown = if state.shown == Shown::Hidden {
                Shown::Open
            } else {
                Shown::Hidden
            };
        }
        if let Some(k) = self.collapse_key
            && ui.input_mut(|i| i.consume_shortcut(&k))
        {
            state.shown = if state.shown == Shown::Open {
                state.floating = false;
                Shown::Rail
            } else {
                Shown::Open
            };
        }
    }

    /// The panel docked to its edge: views rail (B1), header, filter, list, footer; the resize handle.
    #[expect(
        clippy::too_many_arguments,
        reason = "the panel's parts, each needed once"
    )]
    fn docked(
        &mut self,
        ui: &mut Ui,
        t: &Arc<Tokens>,
        state: &mut SidePanelState,
        groups: &[Group<K>],
        out: &mut PanelResponse<K>,
        reveal: Option<K>,
        bounds: Rect,
    ) -> Rect {
        let c = t.chrome;
        let width = parts::width(t, state, bounds);
        let views = self.settings.form == PanelForm::RailAndList && !self.views.is_empty();
        let rail_w = if views { c.rail_width } else { 0.0 };
        let side = state.side;
        let rect = egui_panel(side, self.id)
            .resizable(false)
            .show_separator_line(false)
            .exact_size(width + rail_w)
            .frame(Frame::NONE)
            .show(ui, |ui| {
                let full = ui.max_rect();
                paint::fill(ui.painter(), full, 0, t.colors.surface);
                let (rail_rect, list_rect) = split(full, rail_w, side);
                if views {
                    let mut rail_ui = ui.new_child(UiBuilder::new().max_rect(rail_rect));
                    self.views_rail(&mut rail_ui, t, state);
                }
                let mut list_ui = ui.new_child(UiBuilder::new().max_rect(list_rect));
                parts::body(self, &mut list_ui, t, state, groups, out, reveal, None);
                // After the body: its ground frame would cover the line.
                parts::edge_line(ui, t, full, side);
                full
            })
            .inner;
        parts::resize(ui, t, self.id, &self.title, state, rect, rail_w, bounds);
        rect
    }

    fn views_rail(&self, ui: &mut Ui, t: &Tokens, state: &mut SidePanelState) {
        let items: Vec<RailItem<'_>> = self
            .views
            .iter()
            .enumerate()
            .map(|(i, v)| RailItem {
                icon: Some(&v.icon),
                name: &v.name,
                summary: None,
                key: None,
                light: None,
                unread: v.unread,
                active: i == state.view,
            })
            .collect();
        if let Some(i) = rail::items(
            ui,
            t,
            self.id.with("views"),
            self.settings.lights.unwrap_or(t.chrome.lights),
            state.side,
            &items,
        ) {
            state.view = i;
            state.shown = Shown::Open;
        }
    }

    /// The panel collapsed to its rail, or form B3.
    fn docked_rail(
        &self,
        ui: &mut Ui,
        t: &Arc<Tokens>,
        state: &mut SidePanelState,
        groups: &[Group<K>],
        out: &mut PanelResponse<K>,
    ) -> Rect {
        let side = state.side;
        let form = self.settings.form;
        egui_panel(side, self.id.with("rail"))
            .resizable(false)
            .show_separator_line(false)
            .exact_size(t.chrome.rail_width)
            .frame(Frame::NONE)
            .show(ui, |ui| {
                let full = ui.max_rect();
                paint::fill(ui.painter(), full, 0, t.colors.surface);
                parts::edge_line(ui, t, full, side);
                if form == PanelForm::RailAndList && !self.views.is_empty() {
                    self.views_rail(ui, t, state);
                } else if self.custom.is_some() {
                    let own = [RailItem {
                        icon: self.icon.as_ref(),
                        name: &self.title,
                        summary: None,
                        key: None,
                        light: None,
                        unread: 0,
                        active: false,
                    }];
                    if rail::items(
                        ui,
                        t,
                        self.id.with("own"),
                        self.settings.lights.unwrap_or(t.chrome.lights),
                        side,
                        &own,
                    )
                    .is_some()
                    {
                        if form == PanelForm::RailOnly {
                            out.rail_clicked = Some(0);
                        } else {
                            state.shown = Shown::Open;
                        }
                    }
                } else {
                    let items: Vec<RailItem<'_>> = groups
                        .iter()
                        .map(|g| RailItem {
                            icon: g.rail_icon(),
                            name: g.title.as_deref().unwrap_or(&self.title),
                            summary: g.summary.as_deref(),
                            key: g.key_hint.as_deref(),
                            light: g.rail_light(),
                            unread: g.unread(),
                            active: g.rows.iter().any(any_selected),
                        })
                        .collect();
                    if let Some(i) = rail::items(
                        ui,
                        t,
                        self.id.with("groups"),
                        self.settings.lights.unwrap_or(t.chrome.lights),
                        side,
                        &items,
                    ) {
                        if form == PanelForm::RailOnly {
                            out.rail_clicked = Some(i);
                        } else {
                            state.shown = Shown::Open;
                            if let Some(g) = groups.get(i) {
                                state.set_open(g.fold_key(i), true);
                            }
                            ui.data_mut(|d| d.insert_temp(self.id.with("scroll group"), i));
                        }
                    }
                }
                if form != PanelForm::RailOnly {
                    let name = format!("Open {}", self.title);
                    if rail::foot(ui, t, expand_glyph(side), &name, "Open the panel (Ctrl+B)")
                        .clicked()
                    {
                        state.shown = Shown::Open;
                    }
                }
                full
            })
            .inner
    }
}

/// What of a docked panel fits the room `bounds` leaves, without changing what the person chose: open needs
/// its minimum width and as much again for the work; else the rail, when the rail and that much work fit;
/// else only its edge tab. A floating panel is kept inside the window instead.
fn fit_shown(t: &Tokens, state: &SidePanelState, bounds: Rect) -> Shown {
    let room = bounds.width();
    let work = t.chrome.side_panel_min;
    if state.floating && state.shown == Shown::Open {
        return Shown::Open;
    }
    match state.shown {
        Shown::Open if room >= t.chrome.side_panel_min + work => Shown::Open,
        Shown::Open | Shown::Rail if room >= t.chrome.rail_width + work => Shown::Rail,
        _ => Shown::Hidden,
    }
}

fn egui_panel(side: Side, id: Id) -> egui::Panel {
    match side {
        Side::Left => egui::Panel::left(id),
        Side::Right => egui::Panel::right(id),
    }
}

/// The glyph of the button that opens a rail on `side`.
fn expand_glyph(side: Side) -> &'static str {
    match side {
        Side::Left => "»",
        Side::Right => "«",
    }
}

/// The rail part (outer edge) and the list part of a docked panel's rectangle.
fn split(full: Rect, rail_w: f32, side: Side) -> (Rect, Rect) {
    match side {
        Side::Left => (
            Rect::from_min_max(full.min, pos2(full.left() + rail_w, full.bottom())),
            Rect::from_min_max(pos2(full.left() + rail_w, full.top()), full.max),
        ),
        Side::Right => (
            Rect::from_min_max(pos2(full.right() - rail_w, full.top()), full.max),
            Rect::from_min_max(full.min, pos2(full.right() - rail_w, full.bottom())),
        ),
    }
}

fn any_selected<K>(row: &Row<K>) -> bool {
    row.selected || row.children.iter().any(any_selected)
}

/// Open the group and every parent row of `key`, so revealing it shows it.
fn unfold_to<K: RowKey>(state: &mut SidePanelState, groups: &[Group<K>], key: &K) {
    fn path<K: RowKey>(rows: &[Row<K>], key: &K, out: &mut Vec<u64>) -> bool {
        for r in rows {
            if &r.key == key {
                return true;
            }
            out.push(r.fold_key());
            if path(&r.children, key, out) {
                return true;
            }
            out.pop();
        }
        false
    }
    for (i, g) in groups.iter().enumerate() {
        let mut keys = Vec::new();
        if path(&g.rows, key, &mut keys) {
            state.set_open(g.fold_key(i), true);
            for k in keys {
                state.set_open(k, true);
            }
            state.filter.clear();
            return;
        }
    }
}
