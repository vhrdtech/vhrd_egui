//! The menus: the menu row's titles, their popups, the menu button's menu (every menu as a submenu, the
//! folded slot items, global actions and badge, the command palette) and the entries, all from the action
//! registry.
//!
//! Keyboard: Enter or Arrow Down on a focused title or menu button opens its menu with the first entry
//! focused; the arrows walk the entries (egui's focus moves); Enter on a submenu entry opens it with its first
//! entry focused; Enter runs an entry. Arrow Right and Left across submenus, Escape and where focus goes after
//! a menu closes are the keyboard regions' (piece K8).

use egui::containers::menu::{MenuState, SubMenu};
use egui::{
    Color32, Frame, Id, Key, Modifiers, Popup, PopupCloseBehavior, Rect, Response, Sense, Shape,
    Ui, WidgetInfo, WidgetType, pos2, vec2,
};
use ve_basics::actions::{Actions, MenuItem, MenuNode, Source};

use super::parts::{Ctx, Look, app_menu_of, palette_keys};
use super::{Brand, PALETTE_ACTION, PALETTE_ENTRY, Problem, SlotItem, TopBarOutput};
use crate::icons::{Icon, ph};

/// What the menu button's menu holds besides the menus.
pub(crate) struct Folded<'f, 's> {
    pub left: &'f mut [SlotItem<'s>],
    pub right: &'f mut [SlotItem<'s>],
    /// Global actions folded into the menu, by id.
    pub globals: &'f [&'f str],
    /// The badge folded: an entry with the app menu.
    pub badge: Option<&'f Brand<'f>>,
    /// The breadcrumb folded: the document's name.
    pub document: Option<&'f str>,
}

/// Where a menu's wish to focus an entry is kept: the label to focus, empty for the first entry.
fn focus_key(owner: Id) -> Id {
    owner.with("ve_theme::topbar::focus")
}

/// Asks the menu `owner` to focus its entry `label` (the first when empty) once it is drawn.
pub(crate) fn want_focus(ctx: &egui::Context, owner: Id, label: &str) {
    ctx.data_mut(|d| d.insert_temp(focus_key(owner), label.to_owned()));
}

/// The menu row from `x`: a groove, then a title per menu. Returns the x after it.
pub(crate) fn row(
    ui: &mut Ui,
    cx: &Ctx,
    x: f32,
    menus: &[MenuNode],
    mut actions: Option<&mut Actions>,
    out: &mut TopBarOutput,
) -> f32 {
    let t = &cx.tokens;
    let inset = t.space.l;
    let mut p = cx.pixels();
    p.vline(
        x + t.space.s,
        cx.rect.top() + inset,
        cx.rect.bottom() - inset,
        t.colors.line,
    );
    ui.painter().add(p.finish());
    let mut x = x + cx.groove_width();
    let alt = ui.input(|i| i.modifiers.alt && !i.modifiers.command && !i.modifiers.shift);
    let open = menus
        .iter()
        .position(|m| Popup::is_id_open(ui.ctx(), cx.menu_popup_id(&m.name)));
    for (i, m) in menus.iter().enumerate() {
        let w = cx.menu_title_width(ui, &m.name);
        let rect = cx.button_rect(x, w);
        let popup = cx.menu_popup_id(&m.name);
        let r = ui.interact(rect, cx.menu_title_id(&m.name), Sense::click());
        r.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, &m.name));
        if open.is_some_and(|o| o != i) && r.hovered() {
            Popup::open_id(ui.ctx(), popup);
        }
        open_by_keys(ui, &r, popup);
        let is_open = Popup::is_id_open(ui.ctx(), popup);
        cx.plate(ui, &r, rect, Look::of(&r, is_open));
        let letter = alt.then(|| Actions::menu_mnemonic(&m.name)).flatten();
        title_text(ui, cx, rect, &m.name, letter, t.colors.text);
        menu_popup(&r, cx).show(|ui| {
            body(ui, cx, |ui| {
                entries(ui, cx, popup, &m.items, actions.as_deref_mut(), out);
            });
        });
        x += w;
    }
    x
}

/// Arrow Down on a focused title or button opens its menu, and so does a keyboard click; either focuses
/// the menu's first entry. Keys are taken only while the button has focus.
fn open_by_keys(ui: &Ui, r: &Response, popup: Id) {
    if !r.has_focus() {
        return;
    }
    let down = ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown));
    let key_click = r.clicked() && !r.clicked_by(egui::PointerButton::Primary);
    if down {
        Popup::open_id(ui.ctx(), popup);
    }
    if down || key_click {
        want_focus(ui.ctx(), popup, "");
    }
}

/// The menu button's menu on `anchor`: every menu as a submenu, then what the bar folded, then the
/// command palette.
pub(crate) fn name_menu(
    ui: &mut Ui,
    cx: &Ctx,
    anchor: &Response,
    menus: &[MenuNode],
    mut actions: Option<&mut Actions>,
    folded: Folded<'_, '_>,
    out: &mut TopBarOutput,
) {
    let popup = Popup::default_response_id(anchor);
    open_by_keys(ui, anchor, popup);
    let Folded {
        left,
        right,
        globals,
        badge,
        document,
    } = folded;
    let palette = palette_keys(ui.ctx(), actions.as_deref());
    menu_popup(anchor, cx).show(|ui| {
        body(ui, cx, |ui| {
            if let Some(doc) = document {
                caption(ui, cx, doc);
            }
            for m in menus {
                let r = entry(ui, cx, popup, &Entry::submenu(&m.name));
                let sub = submenu_of(&r);
                SubMenu::new().show(ui, &r, |ui| {
                    body(ui, cx, |ui| {
                        entries(ui, cx, sub, &m.items, actions.as_deref_mut(), out);
                    });
                });
            }
            if !globals.is_empty() {
                separator(ui, cx);
                let items: Vec<MenuItem> = globals
                    .iter()
                    .filter_map(|&g| global_item(g, actions.as_deref(), out))
                    .collect();
                entries(ui, cx, popup, &items, actions.as_deref_mut(), out);
            }
            if let Some(brand) = badge {
                separator(ui, cx);
                let r = entry(ui, cx, popup, &Entry::submenu(&about(brand)));
                SubMenu::new().show(ui, &r, |ui| {
                    body(ui, cx, |ui| app_menu_of(brand).contents(ui));
                });
            }
            for item in left.iter_mut().chain(right.iter_mut()) {
                separator(ui, cx);
                caption(ui, cx, item.label);
                ui.horizontal(|ui| {
                    ui.add_space(cx.tokens.space.m);
                    (item.add)(ui);
                });
            }
            separator(ui, cx);
            let e = Entry {
                label: PALETTE_ENTRY.to_owned(),
                keys: palette,
                icon: Some(ph::MAGNIFYING_GLASS.into()),
                ..Entry::default()
            };
            if entry(ui, cx, popup, &e).clicked() {
                fire(actions, PALETTE_ACTION, out);
                ui.close();
            }
        });
    });
}

/// The label of the folded badge's entry: the app's name and version.
pub(crate) fn about(brand: &Brand<'_>) -> String {
    format!("About {} {}", brand.name, brand.info.version)
}

/// A folded global action as a menu item; an unknown id is a [`Problem`].
fn global_item(id: &str, actions: Option<&Actions>, out: &mut TopBarOutput) -> Option<MenuItem> {
    let found = actions
        .and_then(|a| a.get(id))
        .map(|s| MenuItem::Action(s.action.id()));
    if found.is_none() {
        out.problems.push(Problem::UnknownAction(id.to_owned()));
    }
    found
}

/// Fires `id` from a menu; a refusal is kept in `out`.
fn fire(actions: Option<&mut Actions>, id: &str, out: &mut TopBarOutput) {
    match actions {
        Some(a) => {
            if let Err(e) = a.trigger(id, Source::Menu) {
                out.refused.push(e);
            }
        }
        None => out.problems.push(Problem::UnknownAction(id.to_owned())),
    }
}

/// The id a submenu opened from entry `r` has.
fn submenu_of(r: &Response) -> Id {
    SubMenu::id_from_widget_id(r.id)
}

/// A menu popup under `anchor`, framed by [`body`].
fn menu_popup<'a>(anchor: &Response, cx: &Ctx) -> Popup<'a> {
    Popup::menu(anchor)
        .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
        .frame(Frame::NONE.inner_margin(cx.tokens.space.s))
}

/// A menu's contents on a crisp ground: a whole-pixel shadow, `surface_raised` with a one pixel `line`,
/// under whatever `add` draws. Submenus opened inside get the same frame.
fn body(ui: &mut Ui, cx: &Ctx, add: impl FnOnce(&mut Ui)) {
    let t = &cx.tokens;
    let painter = ui.ctx().layer_painter(ui.layer_id());
    let ground = painter.add(Shape::Noop);
    {
        let s = ui.style_mut();
        // Submenus frame themselves with `Frame::menu` of this style: it draws nothing; `body` draws.
        s.visuals.window_fill = Color32::TRANSPARENT;
        s.visuals.window_stroke = egui::Stroke::NONE;
        s.visuals.popup_shadow = egui::Shadow::NONE;
        s.visuals.menu_corner_radius = egui::CornerRadius::same(t.radius.m);
        s.spacing.menu_margin = egui::Margin::same(t.space.s as i8);
        s.spacing.item_spacing = vec2(t.space.s, 0.0);
    }
    ui.set_min_width(t.chrome.menu_width_min);
    add(ui);
    let width = ui.min_rect().width();
    ui.data_mut(|d| d.insert_temp(width_key(ui), width));
    let rect = ui.min_rect().expand(t.space.s);
    let radius = f32::from(t.radius.m);
    let mut p = cx.pixels();
    p.shadow(rect, radius, t.shadow.popup);
    p.rounded(rect, radius, t.colors.surface_raised);
    p.outline(rect, radius, t.colors.line);
    painter.set(ground, p.finish());
}

/// Where a menu keeps its width: the widest entry of the last frame, so every row spans the menu.
fn width_key(ui: &Ui) -> Id {
    ui.id().with("ve_theme::topbar::menu_width")
}

/// The menu's width as its widest entry left it last frame; at least its minimum.
fn menu_width(ui: &Ui) -> f32 {
    ui.data(|d| d.get_temp::<f32>(width_key(ui)))
        .unwrap_or(0.0)
        .max(ui.min_rect().width())
}

/// One entry as drawn.
#[derive(Default)]
struct Entry {
    label: String,
    keys: Vec<String>,
    icon: Option<Icon>,
    /// Why it cannot run now.
    disabled: Option<String>,
    checked: Option<bool>,
    destructive: bool,
    submenu: bool,
    help: String,
}

impl Entry {
    fn submenu(name: &str) -> Self {
        Self {
            label: name.to_owned(),
            submenu: true,
            ..Self::default()
        }
    }
}

/// A menu's entries, in order; picking one fires its action and closes the menu.
fn entries(
    ui: &mut Ui,
    cx: &Ctx,
    owner: Id,
    items: &[MenuItem],
    mut actions: Option<&mut Actions>,
    out: &mut TopBarOutput,
) {
    for item in items {
        match item {
            MenuItem::Menu(node) => {
                let r = entry(ui, cx, owner, &Entry::submenu(&node.name));
                let sub = submenu_of(&r);
                SubMenu::new().show(ui, &r, |ui| {
                    body(ui, cx, |ui| {
                        entries(ui, cx, sub, &node.items, actions.as_deref_mut(), out);
                    });
                });
            }
            MenuItem::Action(id) => {
                let Some(acts) = actions.as_deref_mut() else {
                    continue;
                };
                let Some(e) = action_entry(ui, acts, id, out) else {
                    continue;
                };
                if entry(ui, cx, owner, &e).clicked() {
                    fire(Some(acts), id, out);
                    ui.close();
                }
            }
        }
    }
}

/// An action as a menu entry: title ("…" on a destructive one), its first key, its icon, why it is off. An
/// icon name no icon set has is a [`Problem`].
fn action_entry(ui: &Ui, actions: &Actions, id: &str, out: &mut TopBarOutput) -> Option<Entry> {
    let s = actions.get(id)?;
    let a = s.action;
    let mut label = a.title().to_owned();
    if a.is_destructive() && !label.ends_with('…') {
        label.push('…');
    }
    let icon = a.icon_name().and_then(Icon::named);
    if let (Some(name), None) = (a.icon_name(), icon) {
        out.problems.push(Problem::UnknownIcon {
            action: a.id(),
            icon: Some(name),
        });
    }
    Some(Entry {
        label,
        keys: s
            .keys
            .first()
            .map(|&k| super::parts::chord_keys(ui.ctx(), k))
            .unwrap_or_default(),
        icon,
        disabled: s.enabled.err().map(str::to_owned),
        checked: s.checked,
        destructive: a.is_destructive(),
        submenu: false,
        help: a.help_text().to_owned(),
    })
}

/// Draws one entry row (and its reason line when disabled) of the menu `owner`; the row's response.
fn entry(ui: &mut Ui, cx: &Ctx, owner: Id, e: &Entry) -> Response {
    let t = &cx.tokens;
    let c = &t.colors;
    let h = t.chrome.row;
    let gs = t.chrome.icon;
    let icon_col = gs + t.space.m;
    let label_w = cx.text_width(ui, &e.label, cx.font());
    let keys: Vec<&str> = e.keys.iter().map(String::as_str).collect();
    let chips = if keys.is_empty() {
        0.0
    } else {
        cx.chips(ui, 0.0, 0.0, &keys, false)
    };
    let tail = if e.submenu {
        t.chrome.icon_small
    } else {
        chips
    };
    let want = t.space.m + icon_col + label_w + t.space.xxl + tail + t.space.m;
    let w = want.max(menu_width(ui));
    let enabled = e.disabled.is_none();
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
    let r = ui.interact(rect, ui.id().with(("entry", &e.label)), sense);
    r.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, &e.label));
    take_focus(ui, owner, &r, &e.label, enabled);
    if e.submenu && r.clicked() && !r.clicked_by(egui::PointerButton::Primary) {
        want_focus(ui.ctx(), SubMenu::id_from_widget_id(r.id), "");
    }
    let open = e.submenu && sub_open(ui, &r);
    let look = if enabled && (open || r.hovered() || r.has_focus()) {
        Look::Hover
    } else {
        Look::Rest
    };
    let ink = if !enabled {
        c.off
    } else if e.destructive {
        c.crit
    } else {
        c.text
    };
    let cy = rect.center().y;
    cx.plate(ui, &r, rect, look);
    let icon_rect = Rect::from_min_size(pos2(rect.left() + t.space.m, cy - gs / 2.0), vec2(gs, gs));
    if e.checked == Some(true) {
        cx.icon(ui, ph::CHECK.into(), icon_rect, gs, ink);
    } else if let Some(icon) = e.icon {
        cx.icon(ui, icon, icon_rect, gs, c.text_muted);
    }
    if e.submenu {
        let s = t.chrome.icon_small;
        let chevron =
            Rect::from_min_size(pos2(rect.right() - t.space.m - s, cy - s / 2.0), vec2(s, s));
        cx.icon(ui, ph::CARET_RIGHT.into(), chevron, s, c.text_muted);
    }
    let g = cx.galley(ui, &e.label, cx.font(), ink);
    cx.paint_galley(ui, icon_rect.right() + t.space.m, cy, g);
    if !keys.is_empty() {
        cx.chips(ui, rect.right() - t.space.m, cy, &keys, true);
    }
    if let Some(why) = &e.disabled {
        let font = egui::FontId::proportional(t.type_scale.caption.size);
        let g = ui.painter().layout(
            why.clone(),
            font,
            c.text_muted,
            w - icon_col - 2.0 * t.space.m,
        );
        let (cr, cresp) = ui.allocate_exact_size(vec2(w, g.size().y + t.space.s), Sense::hover());
        cresp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, why));
        ui.painter().galley(
            cx.pixels()
                .snap_pos(pos2(cr.left() + t.space.m + icon_col, cr.top())),
            g,
            c.text_muted,
        );
        return r.on_hover_text(why);
    }
    if e.help.is_empty() {
        r
    } else {
        r.on_hover_text(&e.help)
    }
}

/// Gives `r` the focus when its menu `owner` asked for this entry, or for its first one.
fn take_focus(ui: &Ui, owner: Id, r: &Response, label: &str, enabled: bool) {
    if !enabled {
        return;
    }
    let key = focus_key(owner);
    let wanted = ui.data(|d| d.get_temp::<String>(key));
    if wanted.is_some_and(|w| w.is_empty() || w == label) {
        ui.data_mut(|d| d.remove::<String>(key));
        r.request_focus();
    }
}

/// Whether the submenu of the entry `r` is open now.
fn sub_open(ui: &Ui, r: &Response) -> bool {
    let id = SubMenu::id_from_widget_id(r.id);
    MenuState::from_ui(ui, |state, _| state.open_item == Some(id))
}

/// A one pixel rule across the menu.
fn separator(ui: &mut Ui, cx: &Ctx) {
    let t = &cx.tokens;
    let (rect, _) = ui.allocate_exact_size(
        vec2(menu_width(ui), t.space.m + t.stroke.thin),
        Sense::hover(),
    );
    let mut p = cx.pixels();
    let y = rect.center().y;
    p.rect(
        Rect::from_min_max(pos2(rect.left(), y), pos2(rect.right(), y + p.px())),
        t.colors.line,
    );
    ui.painter().add(p.finish());
}

/// A small caption naming what is under it.
fn caption(ui: &mut Ui, cx: &Ctx, text: &str) {
    let t = &cx.tokens;
    let font = egui::FontId::proportional(t.type_scale.caption.size);
    let g = cx.galley(ui, text, font, t.colors.text_muted);
    let (rect, r) =
        ui.allocate_exact_size(vec2(menu_width(ui), g.size().y + t.space.s), Sense::hover());
    r.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, text));
    cx.paint_galley(ui, rect.left() + t.space.m, rect.center().y, g);
}

/// A menu title's text in `rect`, its mnemonic `letter` underlined with a one pixel rule.
fn title_text(ui: &Ui, cx: &Ctx, rect: Rect, name: &str, letter: Option<char>, color: Color32) {
    let t = &cx.tokens;
    let g = cx.galley(ui, name, cx.font(), color);
    let x = rect.left() + t.space.m;
    let tr = cx.paint_galley(ui, x, rect.center().y, g);
    let Some(letter) = letter else {
        return;
    };
    let Some(at) = name
        .char_indices()
        .find(|(_, ch)| ch.to_ascii_uppercase() == letter)
        .map(|(i, _)| i)
    else {
        return;
    };
    let before = cx.text_width(ui, &name[..at], cx.font());
    let end = at + name[at..].chars().next().map_or(0, char::len_utf8);
    let upto = cx.text_width(ui, &name[..end], cx.font());
    let mut p = cx.pixels();
    let y = tr.bottom() - t.space.xs;
    p.rect(
        Rect::from_min_max(
            pos2(tr.left() + before, y),
            pos2(tr.left() + upto, y + p.px()),
        ),
        color,
    );
    ui.painter().add(p.finish());
}
