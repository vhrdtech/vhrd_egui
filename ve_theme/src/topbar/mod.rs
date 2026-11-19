//! The top bar every vhrd egui app shares (CHR-8, app-chrome.md section A): a layout of slots, left to
//! right: window controls on macOS, the brand (logo and name), the menus, the app's left slot, the command
//! field in the centre (optional), the app's right slot, the build badge, the global actions, window
//! controls on Windows and other stacking desktops.
//!
//! ```ignore
//! let brand = Brand::new("eventfull", &self.build).logo(Logo::Placeholder("ef"));
//! let out = TopBar::new(&brand, &self.settings.top_bar)
//!     .menus(&mut self.actions)
//!     .document("bench-18nov2026.efp", self.dirty)
//!     .globals(&["view.theme", "app.settings"])
//!     .left("View", 180.0, |ui| { ui.selectable_value(&mut view.vertical, false, "Horizontal"); })
//!     .right("Record", 88.0, |ui| { if ui.primary_button("Record", "Start recording").clicked() { rec.start(); } })
//!     .show(ui);
//! for fired in self.actions.take_fired() { self.run(fired.id); } // `app.palette` opens the palette
//! ```
//!
//! Everything the bar runs is an action of the registry (`ve_basics::actions`, CHR-1): a menu entry fires
//! with `Source::Menu`, a global action's button and the field with `Source::Toolbar`, and the field, its
//! search button and the menu's *Command palette* entry fire [`PALETTE_ACTION`] (Ctrl K, registered here),
//! whose keys the field shows. The app runs them as it runs keys; the bar takes no shortcut itself.
//!
//! As the window narrows the bar walks the collapse ladder (A2): every part that folds moves into the menu
//! button's menu, and from 240 points up no two parts overlap and a stretch of `Chrome::drag_min` stays free
//! to move the window. All sizes and colours are [`crate::Tokens`], icons [`crate::icons`]; everything else
//! is painted on the pixel grid.
//!
//! Keyboard: F10 focuses the first menu or the menu button; Arrow Down or Enter opens it with its first entry
//! focused; holding Alt underlines the menus' letters and Alt with the letter opens that menu (folded: the
//! menu button's menu at that menu's entry); the actions sit in the tab order with their keys in their
//! tooltips. Escape and where focus returns after a menu are the keyboard regions' (K8), as are the entries'
//! own mnemonics.

mod gallery;
mod ladder;
mod menus;
mod parts;
mod pixels;
mod settings;
mod text;

pub use gallery::TopBarGallery;
pub use settings::{
    BadgePlace, Desktop, FieldMode, MenuButton, MenuStyle, TopBarSettings, WindowControls,
};

use egui::{Align, Id, Key, Layout, Modifiers, Rect, Response, Sense, Ui, UiBuilder, pos2, vec2};
use ve_basics::actions::{Action, Actions, MenuNode, Refused, Source};
use ve_basics::{AppMenuAction, BuildInfo};

use crate::Tokens;
use ladder::{Plan, Widths};
use parts::Ctx;
use settings::ControlsAt;

/// Accessible label of the command field and of the search button it collapses to.
pub const FIELD_LABEL: &str = "Search or run a command";
/// The command field's placeholder when it shows no document.
pub const FIELD_HINT: &str = FIELD_LABEL;
/// Accessible label of the menu button when it is not the app's name (dots, lines, the word, the mark).
pub const MENU_LABEL: &str = "Menu";
/// The menu entry that opens the command palette, last in the menu button's menu.
pub const PALETTE_ENTRY: &str = "Command palette";
/// The action the field runs: the command palette.
pub const PALETTE_ACTION: &str = "app.palette";

/// The palette's action as declared: Ctrl K, registered with `inventory` for `Actions::from_registry`; an app
/// building its set with `Actions::from_list` adds it.
pub const fn palette_action() -> Action {
    Action::new(PALETTE_ACTION, PALETTE_ENTRY)
        .keys(&["Ctrl+K"])
        .icon("magnifying-glass")
        .help("Search, or run a command by its name")
}

ve_basics::actions::inventory::submit! { palette_action() }

/// The app the bar belongs to: its name, build and logo.
pub struct Brand<'a> {
    name: &'a str,
    info: &'a BuildInfo,
    logo: Logo<'a>,
    description: &'a str,
}

impl<'a> Brand<'a> {
    /// The name people know the app by ("Mesh Dash", "eventfull") and its build (`ve_basics::build_info!`).
    pub fn new(name: &'a str, info: &'a BuildInfo) -> Self {
        Self {
            name,
            info,
            logo: Logo::None,
            description: "",
        }
    }

    /// The logo in the logo slot; [`Logo::None`] starts the wordmark at the bar's edge.
    pub fn logo(mut self, logo: Logo<'a>) -> Self {
        self.logo = logo;
        self
    }

    /// One or two sentences on what the app is for, shown in the app menu the badge opens.
    pub fn description(mut self, description: &'a str) -> Self {
        self.description = description;
        self
    }
}

/// The logo slot: any square mark at `Tokens::chrome.logo_mark`, with clear space on every side.
pub enum Logo<'a> {
    /// No logo: the wordmark starts at the bar's edge, and stays (cut if need be) when the bar narrows.
    None,
    /// One mark for both themes (a mark with its own dark tile); a texture (`ve_basics::load_icon`), or any
    /// source an installed egui loader takes.
    Image(egui::ImageSource<'a>),
    /// A mark for the light and one for the dark theme.
    Themed {
        /// Drawn in the light theme.
        light: egui::ImageSource<'a>,
        /// Drawn in the dark theme.
        dark: egui::ImageSource<'a>,
    },
    /// No logo yet: a dashed square with a monogram (`ef`), clearly a placeholder.
    Placeholder(&'a str),
}

/// An item the app puts in the left or right slot: drawn by its closure in a rectangle of its width, the
/// bar's height; folded into the menu button's menu, under its label, when the bar is too narrow.
struct SlotItem<'a> {
    label: &'a str,
    width: f32,
    add: Box<dyn FnMut(&mut Ui) + 'a>,
}

/// The open document the command field shows.
#[derive(Clone, Copy, Debug)]
struct Document<'a> {
    name: &'a str,
    path: &'a str,
    dirty: bool,
}

/// The top bar, built each frame and shown with [`TopBar::show`].
pub struct TopBar<'a> {
    brand: &'a Brand<'a>,
    settings: TopBarSettings,
    desktop: Option<Desktop>,
    actions: Option<&'a mut Actions>,
    document: Option<Document<'a>>,
    globals: &'a [&'a str],
    left: Vec<SlotItem<'a>>,
    right: Vec<SlotItem<'a>>,
}

/// Something wrong in what the app gave the bar; the bar still draws, with a `crit` box where an icon is
/// missing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Problem {
    /// No action has this id: a global action, or [`PALETTE_ACTION`] when the field ran it.
    UnknownAction(String),
    /// An action shown with an icon names none of the icon sets (or none at all, for a global action).
    UnknownIcon {
        /// The action's id.
        action: &'static str,
        /// The icon name it gives.
        icon: Option<&'static str>,
    },
}

/// What happened in the bar this frame.
#[derive(Debug)]
pub struct TopBarOutput {
    /// The whole bar.
    pub response: Response,
    /// The app menu (on the badge, or its entry in the menu) confirmed an action.
    pub app_menu: Option<AppMenuAction>,
    /// Actions picked but refused (disabled).
    pub refused: Vec<Refused>,
    /// What is wrong in the bar's input; empty in a healthy app, a test can assert so.
    pub problems: Vec<Problem>,
    /// What the collapse ladder folded.
    pub folded: Folded,
}

/// What the collapse ladder folded this frame; each folded part is behind the menu button.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Folded {
    /// The menu row is behind the menu button.
    pub menu_row: bool,
    /// The badge shows the version only; the rest is in its tooltip.
    pub badge: bool,
    /// The command field is a search button.
    pub field: bool,
    /// Left and right slot items folded into the menu.
    pub left: usize,
    /// See `left`.
    pub right: usize,
    /// The breadcrumb (style B) folded; the document's name heads the menu.
    pub breadcrumb: bool,
    /// The wordmark is gone; the logo carries the name and the menu.
    pub wordmark: bool,
    /// The search button folded into the menu's *Command palette* entry.
    pub search: bool,
    /// Global actions folded into the menu, counted from the left.
    pub globals: usize,
    /// The badge is an entry of the menu.
    pub badge_in_menu: bool,
    /// The name is cut to fit.
    pub name_cut: bool,
}

impl<'a> TopBar<'a> {
    /// A bar for `brand` with the user's `settings`.
    pub fn new(brand: &'a Brand<'a>, settings: &TopBarSettings) -> Self {
        Self {
            brand,
            settings: *settings,
            desktop: None,
            actions: None,
            document: None,
            globals: &[],
            left: Vec::new(),
            right: Vec::new(),
        }
    }

    /// The desktop for [`WindowControls::Auto`]; [`Desktop::detect`], once per context, when not set.
    pub fn desktop(mut self, desktop: Desktop) -> Self {
        self.desktop = Some(desktop);
        self
    }

    /// The menus: the registry's menu tree (`Actions::menu_tree`); a pick fires its action.
    pub fn menus(mut self, actions: &'a mut Actions) -> Self {
        self.actions = Some(actions);
        self
    }

    /// The open document: its name and whether it has unsaved changes.
    pub fn document(mut self, name: &'a str, dirty: bool) -> Self {
        self.document = Some(Document {
            name,
            path: "",
            dirty,
        });
        self
    }

    /// The open document's folder or full path, shown whole when its name is hovered.
    pub fn document_path(mut self, path: &'a str) -> Self {
        if let Some(d) = &mut self.document {
            d.path = path;
        }
        self
    }

    /// The global actions at the right end, by action id (`view.theme`, `app.settings`): a square button each
    /// with the action's icon (`crate::icons::Icon::named`); one without a known icon is a [`Problem`].
    pub fn globals(mut self, ids: &'a [&'a str]) -> Self {
        self.globals = ids;
        self
    }

    /// An item in the left slot, after the menus: `label` names it in the menu when it folds, `width` is the
    /// room it takes in the bar, `add` draws it (left to right, centred vertically).
    pub fn left(mut self, label: &'a str, width: f32, add: impl FnMut(&mut Ui) + 'a) -> Self {
        self.left.push(SlotItem {
            label,
            width,
            add: Box::new(add),
        });
        self
    }

    /// An item in the right slot, before the badge, as [`TopBar::left`]; drawn right to left, so buttons stack
    /// from the right. Items come in the order given, left to right; the leftmost folds first.
    pub fn right(mut self, label: &'a str, width: f32, add: impl FnMut(&mut Ui) + 'a) -> Self {
        self.right.push(SlotItem {
            label,
            width,
            add: Box::new(add),
        });
        self
    }

    /// Draws the bar across `ui`'s available width, at `Tokens::chrome.top_bar` height.
    pub fn show(mut self, ui: &mut Ui) -> TopBarOutput {
        let tokens = Tokens::of_ui(ui);
        let height = tokens.chrome.top_bar;
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
        let id = ui.make_persistent_id("ve_theme::topbar");
        let desktop = self.desktop.unwrap_or_else(|| desktop_of(ui.ctx()));
        let controls = self.settings.window_controls.place(desktop);
        let system = self.settings.window_controls.system_decorations(desktop);
        let cx = Ctx::new(ui, &tokens, rect, id);
        sync_decorations(ui, id, system);

        // The bar behind everything: its ground, its bottom line, and the window's grip.
        let response = ui.interact(rect, id.with("bar"), Sense::click_and_drag());
        cx.ground(ui);
        if !system {
            if response.drag_started() {
                ui.send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
            if response.double_clicked() {
                let max = ui.input(|i| i.viewport().maximized.unwrap_or(false));
                ui.send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
            }
        }

        let menus = self
            .actions
            .as_deref()
            .map(Actions::menu_tree)
            .unwrap_or_default();
        let widths = self.measure(ui, &cx, &menus, controls, !system);
        let plan = ladder::plan(&widths);
        let style = self.settings.menu_style;
        let has_field = widths.field_full > 0.0;
        let mut out = TopBarOutput {
            response,
            app_menu: None,
            refused: Vec::new(),
            problems: Vec::new(),
            folded: Folded {
                menu_row: !plan.menu_row && style != MenuStyle::Button,
                badge: !plan.badge_full,
                field: has_field && plan.field <= 0.0,
                left: self.left.len() - plan.left_shown,
                right: self.right.len() - plan.right_shown,
                breadcrumb: widths.breadcrumb > 0.0 && !plan.breadcrumb,
                wordmark: !plan.wordmark,
                search: widths.search_button > 0.0 && plan.field <= 0.0 && !plan.search_button,
                globals: self.globals.len() - plan.globals_shown,
                badge_in_menu: !plan.badge,
                name_cut: plan.wordmark && plan.wordmark_width < widths.wordmark,
            },
        };
        keyboard(ui, &cx, &menus, &plan);
        self.lay_out(ui, &cx, &menus, &plan, &widths, controls, &mut out);
        out
    }

    /// Widths of every part at full size.
    fn measure(
        &self,
        ui: &Ui,
        cx: &Ctx,
        menus: &[MenuNode],
        controls: ControlsAt,
        drags: bool,
    ) -> Widths {
        let t = &cx.tokens;
        let c = &t.chrome;
        let style = self.settings.menu_style;
        let has_logo = !matches!(self.brand.logo, Logo::None);
        let has_field = style != MenuStyle::Classic && self.settings.field != FieldMode::None;
        let name_button = self.settings.menu_button == MenuButton::Name
            || (self.settings.menu_button == MenuButton::Mark && !has_logo);
        // The name carries a chevron only while it is the menu button; measured with it, so the ladder never
        // flips back and forth across the threshold.
        let chevron = if name_button {
            cx.chevron_width() + t.space.m
        } else {
            t.space.m
        };
        let first: String = self.brand.name.chars().take(1).chain(['…']).collect();
        let menu_row = if style == MenuStyle::Button || menus.is_empty() {
            0.0
        } else {
            cx.groove_width()
                + menus
                    .iter()
                    .map(|m| cx.menu_title_width(ui, &m.name))
                    .sum::<f32>()
        };
        let menu_button = match self.settings.menu_button {
            MenuButton::Name | MenuButton::Mark => 0.0,
            MenuButton::Word => cx.word_button_width(ui) + t.space.s,
            MenuButton::Dots | MenuButton::Lines => cx.square_width(),
        };
        let menu_button_min = match self.settings.menu_button {
            MenuButton::Word => cx.square_width(),
            _ => menu_button,
        };
        let ctl = 3.0 * c.window_control;
        Widths {
            bar: cx.rect.width(),
            edge: t.space.l,
            gap: t.space.m,
            drag: if drags { c.drag_min } else { 0.0 },
            logo: if has_logo {
                c.logo_mark + t.space.m
            } else {
                0.0
            },
            wordmark: cx.wordmark_width(ui, self.brand.name) + chevron,
            wordmark_min: cx.wordmark_width(ui, &first) + chevron,
            menu_row,
            menu_button,
            menu_button_min,
            breadcrumb: if style == MenuStyle::Button && self.document.is_some() {
                c.breadcrumb
            } else {
                0.0
            },
            badge_full: cx.badge_width(ui, &self.brand.info.short(), self.brand.info.debug),
            badge_version: cx.badge_width(ui, self.brand.info.version, self.brand.info.debug),
            field_full: if has_field { c.field_width } else { 0.0 },
            field_mid: if has_field { c.field_width_mid } else { 0.0 },
            field_min: if has_field { c.field_width_min } else { 0.0 },
            search_button: if has_field || style == MenuStyle::Classic {
                cx.square_width()
            } else {
                0.0
            },
            left: self.left.iter().map(|s| s.width).collect(),
            right: self.right.iter().map(|s| s.width).collect(),
            globals: vec![cx.square_width(); self.globals.len()],
            controls_right: if controls == ControlsAt::Right {
                ctl
            } else {
                0.0
            },
            controls_left: if controls == ControlsAt::Left {
                ctl
            } else {
                0.0
            },
            wordmark_may_go: has_logo,
        }
    }

    /// Places and draws every part along the plan.
    #[expect(
        clippy::too_many_arguments,
        reason = "the layout pass reads every input of the bar"
    )]
    fn lay_out(
        &mut self,
        ui: &mut Ui,
        cx: &Ctx,
        menus: &[MenuNode],
        plan: &Plan,
        w: &Widths,
        controls: ControlsAt,
        out: &mut TopBarOutput,
    ) {
        let (left_end, anchor) = self.lay_out_left(ui, cx, menus, plan, w, controls, out);
        let rx = self.lay_out_right(ui, cx, plan, w, controls, out);
        self.lay_out_centre(ui, cx, plan, w, left_end, rx, out);
        if let Some(anchor) = anchor {
            let shown_globals = self.globals.len() - plan.globals_shown;
            let folded_doc = (self.settings.menu_style == MenuStyle::Button && !plan.breadcrumb)
                .then_some(self.document)
                .flatten()
                .map(|d| d.name);
            let shown_right = self.right.len() - plan.right_shown;
            let folded = menus::Folded {
                left: &mut self.left[plan.left_shown..],
                right: &mut self.right[..shown_right],
                globals: &self.globals[..shown_globals],
                badge: (!plan.badge).then_some(self.brand),
                document: folded_doc,
            };
            menus::name_menu(
                ui,
                cx,
                &anchor,
                menus,
                self.actions.as_deref_mut(),
                folded,
                out,
            );
        }
        if !plan.badge {
            out.app_menu = parts::app_menu_of(self.brand).dialog(ui.ctx());
        }
    }

    /// The left end: controls on macOS, logo, name, badge by the name, menu button or row, breadcrumb, left
    /// slot. Returns where it ends and the menu button's response, if the menus are behind one.
    #[expect(
        clippy::too_many_arguments,
        reason = "the layout pass reads every input of the bar"
    )]
    fn lay_out_left(
        &mut self,
        ui: &mut Ui,
        cx: &Ctx,
        menus: &[MenuNode],
        plan: &Plan,
        w: &Widths,
        controls: ControlsAt,
        out: &mut TopBarOutput,
    ) -> (f32, Option<Response>) {
        let t = &cx.tokens;
        let rect = cx.rect;
        let mut x = rect.left();
        if controls == ControlsAt::Left {
            x = cx.window_controls(ui, x, true);
        }
        x += w.edge;
        let has_logo = !matches!(self.brand.logo, Logo::None);
        let collapsed = !plan.menu_row;
        let kind = self.settings.menu_button;
        // Who carries the menu when the row is folded (or style B has no row).
        let mark_is_button = collapsed && has_logo && (kind == MenuButton::Mark || !plan.wordmark);
        let name_is_button = collapsed
            && plan.wordmark
            && (kind == MenuButton::Name || (kind == MenuButton::Mark && !has_logo));
        let mut anchor: Option<Response> = None;
        if has_logo {
            let r = cx.logo(ui, x, &self.brand.logo, self.brand.name, mark_is_button);
            if mark_is_button {
                anchor = Some(r);
            }
            x += w.logo;
        }
        if plan.wordmark {
            let width = plan.wordmark_width - t.space.m;
            let r = cx.wordmark(ui, x, self.brand.name, width, name_is_button);
            if name_is_button {
                anchor = Some(r);
            }
            x += plan.wordmark_width;
        }
        if self.settings.badge == BadgePlace::ByName && plan.badge {
            let r = cx.badge(ui, x, self.brand.info, plan.badge_full);
            x = r.rect.right() + w.gap;
            out.app_menu = cx.app_menu(&r, self.brand);
        }
        if collapsed && anchor.is_none() {
            let shown = if plan.menu_button_small {
                MenuButton::Lines
            } else {
                kind
            };
            let r = cx.menu_button(ui, x, shown);
            x = r.rect.right() + t.space.s;
            anchor = Some(r);
        }
        if plan.menu_row {
            x = menus::row(ui, cx, x, menus, self.actions.as_deref_mut(), out);
        }
        if plan.breadcrumb
            && let Some(doc) = self.document
        {
            x = cx.breadcrumb(ui, x, doc.name, doc.path, doc.dirty);
        }
        for item in &mut self.left[..plan.left_shown] {
            let r =
                Rect::from_min_size(pos2(x + w.gap, rect.top()), vec2(item.width, rect.height()));
            slot(ui, r, Layout::left_to_right(Align::Center), &mut item.add);
            x = r.right();
        }
        (x, anchor)
    }

    /// The right end, from the edge inwards: controls, global actions, badge, right slot, search button.
    /// Returns where it starts.
    fn lay_out_right(
        &mut self,
        ui: &mut Ui,
        cx: &Ctx,
        plan: &Plan,
        w: &Widths,
        controls: ControlsAt,
        out: &mut TopBarOutput,
    ) -> f32 {
        let t = &cx.tokens;
        let rect = cx.rect;
        let mut rx = rect.right();
        if controls == ControlsAt::Right {
            rx = cx.window_controls(ui, rx, false);
        }
        rx -= w.edge - t.space.s;
        let shown = &self.globals[self.globals.len() - plan.globals_shown..];
        for &gid in shown.iter().rev() {
            rx -= cx.square_width();
            match self.actions.as_deref_mut() {
                Some(actions) => cx.global(ui, rx, gid, actions, out),
                None => out.problems.push(Problem::UnknownAction(gid.to_owned())),
            }
        }
        if self.settings.badge == BadgePlace::Right && plan.badge {
            let bw = if plan.badge_full {
                w.badge_full
            } else {
                w.badge_version
            };
            rx -= bw + w.gap;
            let r = cx.badge(ui, rx, self.brand.info, plan.badge_full);
            out.app_menu = cx.app_menu(&r, self.brand);
        }
        let shown_right = self.right.len() - plan.right_shown;
        for item in self.right[shown_right..].iter_mut().rev() {
            rx -= item.width + w.gap;
            let r = Rect::from_min_size(pos2(rx, rect.top()), vec2(item.width, rect.height()));
            slot(ui, r, Layout::right_to_left(Align::Center), &mut item.add);
        }
        if plan.search_button {
            rx -= cx.square_width();
            let keys = parts::palette_keys(ui.ctx(), self.actions.as_deref());
            if cx.search_button(ui, rx, &keys).clicked() {
                self.palette(out);
            }
        }
        rx
    }

    /// The centre between `left_end` and `rx`: the field, or style A's title. The field is centred on the
    /// window unless that leaves no stretch of `drag` points free on either side; then it moves left.
    #[expect(
        clippy::too_many_arguments,
        reason = "the layout pass reads every input of the bar"
    )]
    fn lay_out_centre(
        &mut self,
        ui: &mut Ui,
        cx: &Ctx,
        plan: &Plan,
        w: &Widths,
        left_end: f32,
        rx: f32,
        out: &mut TopBarOutput,
    ) {
        let style = self.settings.menu_style;
        if plan.field > 0.0 {
            let lo = left_end + w.gap;
            let hi = rx - w.gap - plan.field;
            let mut fx = (cx.rect.center().x - plan.field / 2.0).clamp(lo, hi.max(lo));
            let free = |fx: f32| (fx - lo).max(hi - fx);
            if free(fx) < w.drag {
                fx = lo;
            }
            let doc = (self.settings.field == FieldMode::Document
                && style == MenuStyle::RowAndField)
                .then_some(self.document)
                .flatten()
                .map(|d| (d.name, d.path, d.dirty));
            let keys = parts::palette_keys(ui.ctx(), self.actions.as_deref());
            if cx.field(ui, fx, plan.field, doc, &keys).clicked() {
                self.palette(out);
            }
        } else if style == MenuStyle::Classic
            && let Some(doc) = self.document
        {
            // The title keeps the drag stretch free on its right.
            let x1 = rx - w.gap - w.drag;
            cx.title(ui, left_end + w.gap, x1, doc.name, doc.path, doc.dirty);
        }
    }

    /// Runs the palette's action from the field or the search button.
    fn palette(&mut self, out: &mut TopBarOutput) {
        match self.actions.as_deref_mut() {
            Some(a) => {
                if let Err(e) = a.trigger(PALETTE_ACTION, Source::Toolbar) {
                    match e {
                        Refused::Unknown(id) => out.problems.push(Problem::UnknownAction(id)),
                        other => out.refused.push(other),
                    }
                }
            }
            None => out
                .problems
                .push(Problem::UnknownAction(PALETTE_ACTION.to_owned())),
        }
    }
}

/// F10 focuses the first menu or the menu button; Alt with a menu's letter opens that menu, or, folded, the
/// menu button's menu at that menu's entry. Keys are taken only when they do that.
fn keyboard(ui: &mut Ui, cx: &Ctx, menus: &[MenuNode], plan: &Plan) {
    if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F10)) {
        let first = if plan.menu_row {
            menus.first().map(|m| cx.menu_title_id(&m.name))
        } else {
            Some(cx.menu_button_id())
        };
        if let Some(first) = first {
            ui.memory_mut(|m| m.request_focus(first));
        }
    }
    for m in menus {
        let Some(letter) = Actions::menu_mnemonic(&m.name) else {
            continue;
        };
        let Some(key) = Key::from_name(&letter.to_string()) else {
            continue;
        };
        if !ui.input_mut(|i| i.consume_key(Modifiers::ALT, key)) {
            continue;
        }
        if plan.menu_row {
            let popup = cx.menu_popup_id(&m.name);
            egui::Popup::open_id(ui.ctx(), popup);
            ui.memory_mut(|mem| mem.request_focus(cx.menu_title_id(&m.name)));
            menus::want_focus(ui.ctx(), popup, "");
        } else {
            let popup = cx.menu_button_id().with("popup");
            egui::Popup::open_id(ui.ctx(), popup);
            menus::want_focus(ui.ctx(), popup, &m.name);
        }
    }
}

/// The desktop this process runs on, detected once per context.
fn desktop_of(ctx: &egui::Context) -> Desktop {
    let key = Id::new("ve_theme::topbar::desktop");
    if let Some(d) = ctx.data(|d| d.get_temp::<Desktop>(key)) {
        return d;
    }
    let d = Desktop::detect();
    ctx.data_mut(|data| data.insert_temp(key, d));
    d
}

/// Runs a slot item's closure in its rectangle.
fn slot(ui: &mut Ui, rect: Rect, layout: Layout, add: &mut Box<dyn FnMut(&mut Ui) + '_>) {
    ui.scope_builder(UiBuilder::new().max_rect(rect).layout(layout), |ui| add(ui));
}

/// Tells the window whether the system draws its decorations, once per change of the setting.
fn sync_decorations(ui: &Ui, id: Id, system: bool) {
    let key = id.with("decorations");
    let last = ui.data(|d| d.get_temp::<bool>(key));
    if last != Some(system) {
        ui.data_mut(|d| d.insert_temp(key, system));
        ui.send_viewport_cmd(egui::ViewportCommand::Decorations(system));
    }
}
