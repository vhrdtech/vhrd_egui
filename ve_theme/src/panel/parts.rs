//! The panel's frame parts: header and its menu, filter field, footer, resize handle, the edge tab of a
//! hidden panel, the floating window.

use std::f32::consts::FRAC_PI_2;
use std::sync::Arc;

use egui::epaint::TextShape;
use egui::{
    Align, Area, Button, CursorIcon, Frame, Id, Key, LayerId, Layout, Margin, Modifiers, Order,
    Rect, RichText, ScrollArea, Sense, TextEdit, Theme, Ui, UiBuilder, WidgetInfo, WidgetType,
    pos2, vec2,
};

use super::list::{self, ListCx};
use super::model::{Group, Query, RowKey};
use super::paint::{
    self, bar_button, fill, galley_at, key_chip, line, mono_font, snap_pos, text_font,
};
use super::state::{Shown, Side, SidePanelState};
use super::{PanelResponse, SidePanel};
use crate::Tokens;

/// The narrowest and widest a docked panel may be in `bounds`: the token minimum, and
/// `Chrome::side_panel_max_share` of the room (half of it).
pub(crate) fn limits(t: &Tokens, bounds: Rect) -> (f32, f32) {
    let min = t.chrome.side_panel_min;
    let max = (bounds.width() * t.chrome.side_panel_max_share).floor();
    (min, if max.is_finite() { max.max(min) } else { min })
}

/// The panel's width now: the person's, else the token default, within [`limits`], on a whole point.
pub(crate) fn width(t: &Tokens, state: &SidePanelState, bounds: Rect) -> f32 {
    let (min, max) = limits(t, bounds);
    state
        .set_width()
        .unwrap_or(t.chrome.side_panel_default)
        .clamp(min, max)
        .round()
}

/// The 1 px `line` on a docked panel's inner edge.
pub(crate) fn edge_line(ui: &Ui, t: &Tokens, full: Rect, side: Side) {
    let w = t.stroke.thin;
    let r = match side {
        Side::Left => Rect::from_min_max(pos2(full.right() - w, full.top()), full.max),
        Side::Right => Rect::from_min_max(full.min, pos2(full.left() + w, full.bottom())),
    };
    fill(ui.painter(), r, 0, t.colors.line);
}

/// Header, filter, list and footer in `ui`'s whole rectangle. `float_bounds` is the window's rectangle when
/// the panel floats (its header then drags it).
#[expect(
    clippy::too_many_arguments,
    reason = "the panel's parts, each needed once"
)]
pub(crate) fn body<K: RowKey>(
    panel: &mut SidePanel<'_, K>,
    ui: &mut Ui,
    t: &Arc<Tokens>,
    state: &mut SidePanelState,
    groups: &[Group<K>],
    out: &mut PanelResponse<K>,
    reveal: Option<K>,
    float_bounds: Option<Rect>,
) {
    let c = t.chrome;
    let rect = ui.max_rect();
    let head = Rect::from_min_size(rect.min, vec2(rect.width(), c.panel_header));
    header(panel, ui, t, state, head, float_bounds);
    if let Some(add) = panel.custom.take() {
        let body = Rect::from_min_max(pos2(rect.left(), head.bottom()), rect.max);
        let mut child = ui.new_child(
            UiBuilder::new()
                .max_rect(body)
                .layout(Layout::top_down(Align::Min)),
        );
        child.set_clip_rect(body.intersect(ui.clip_rect()));
        // Framed in the panel's ground, so the scroll fade blends toward it.
        Frame::new().fill(t.colors.surface).show(&mut child, |ui| {
            ui.set_min_size(ui.available_size());
            ScrollArea::vertical()
                .id_salt(panel.id.with("body"))
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    Frame::new()
                        .inner_margin(Margin::symmetric(t.space.l as i8, t.space.s as i8))
                        .show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            add(ui);
                        });
                });
        });
        return;
    }
    let mut top = head.bottom();
    if panel.filter {
        let r = Rect::from_min_size(
            pos2(rect.left() + t.space.m, top),
            vec2(rect.width() - t.space.m * 2.0, c.text_field),
        );
        filter_field(panel, ui, t, state, r);
        top = r.bottom() + t.space.s;
    }
    let foot = Rect::from_min_max(pos2(rect.left(), rect.bottom() - c.row), rect.max);
    let list_rect = Rect::from_min_max(pos2(rect.left(), top), pos2(rect.right(), foot.top()));
    let typed = state.filter.trim().to_owned();
    let query = Query::new(&typed);
    let flat = list::flatten(groups, &query, state);
    let (total, matched, cut) = (flat.total, flat.matched, flat.cut);

    let scroll_group = ui.data_mut(|d| d.remove_temp::<usize>(panel.id.with("scroll group")));
    let follow = follow_target(ui, panel);
    let mut row_menu = panel.row_menu.take();
    let lights = panel.settings.lights.unwrap_or(t.chrome.lights);
    let mut cx = ListCx {
        t: t.clone(),
        id: panel.id,
        query,
        lights,
        state,
        link: panel.link.as_deref_mut(),
        reveal,
        scroll_group,
        row_menu: row_menu
            .as_deref_mut()
            .map(|m| m as &mut dyn FnMut(&mut Ui, &K)),
        out,
        top: None,
        follow,
    };
    let mut list_ui = ui.new_child(
        UiBuilder::new()
            .max_rect(list_rect)
            .layout(Layout::top_down(Align::Min)),
    );
    list_ui.set_clip_rect(list_rect.intersect(ui.clip_rect()));
    // Framed in the panel's ground: egui's scroll fade blends toward the fill of the frame under the list.
    Frame::new()
        .fill(t.colors.surface)
        .show(&mut list_ui, |ui| {
            ui.set_min_size(ui.available_size());
            list::draw(ui, &mut cx, &flat);
        });
    let top_row = cx.top.take();
    if let Some(link) = panel.link.as_deref_mut() {
        link.set_tree_top(top_row);
    }
    let mut text = if typed.is_empty() {
        panel
            .footer
            .clone()
            .unwrap_or_else(|| format!("{total} {}", panel.noun))
    } else {
        format!(
            "{matched} of {total} {} match \u{201c}{typed}\u{201d}",
            panel.noun
        )
    };
    if cut > 0 {
        text.push_str(&format!(" \u{b7} {cut} not shown"));
    }
    footer(ui, t, foot, &text);
}

/// The row the tree should scroll to while it follows the view: the view's top row, once per change.
fn follow_target<K: RowKey>(ui: &Ui, panel: &SidePanel<'_, K>) -> Option<K> {
    let link = panel.link.as_deref()?;
    let k = link.tree_should_show()?.clone();
    let id = panel.id.with("followed");
    let h = Id::new(&k).value();
    if ui.data(|d| d.get_temp::<u64>(id)) == Some(h) {
        return None;
    }
    ui.data_mut(|d| d.insert_temp(id, h));
    Some(k)
}

/// Title, the app's actions, the menu, collapse (docked) or dock and hide (floating).
fn header<K: RowKey>(
    panel: &mut SidePanel<'_, K>,
    ui: &mut Ui,
    t: &Tokens,
    state: &mut SidePanelState,
    rect: Rect,
    float_bounds: Option<Rect>,
) {
    if let Some(bounds) = float_bounds {
        drag_head(ui, t, panel.id, &panel.title, state, rect, bounds);
    }
    let floating = float_bounds.is_some();
    let size = t.chrome.bar_button;
    let gap = t.space.xs;
    // The panel's own buttons: ⋯ and collapse docked, ⋯, dock and hide floating.
    let fixed = if floating { 3.0 } else { 2.0 };
    let fixed_w = fixed * size + (fixed - 1.0) * gap;
    let buttons = rect.shrink2(vec2(t.space.s, 0.0));
    // Everything is added left to right, in the order Tab reaches it; the app's actions end where the panel's
    // buttons start, so their width is the one they took last frame (a change redoes the frame).
    let width_id = panel.id.with("actions width");
    let actions_w = ui.data(|d| d.get_temp::<f32>(width_id)).unwrap_or(0.0);
    let p = ui.painter();
    let g = line(
        p,
        &panel.title,
        text_font(t),
        t.colors.title,
        rect.width() - t.space.l - t.space.s - fixed_w - actions_w - gap * 2.0,
    );
    galley_at(
        p,
        g,
        rect.left() + t.space.l,
        rect.center().y,
        t.colors.title,
    );
    let fixed_left = buttons.right() - fixed_w;
    if let Some(actions) = panel.actions.take() {
        let end = fixed_left - gap;
        let mut bar = ui.new_child(
            UiBuilder::new()
                .max_rect(Rect::from_min_max(
                    pos2(end - actions_w, buttons.top()),
                    pos2(end, buttons.bottom()),
                ))
                .layout(Layout::left_to_right(Align::Center)),
        );
        bar.spacing_mut().item_spacing.x = gap;
        actions(&mut bar);
        let took = bar.min_rect().width().max(0.0);
        if (took - actions_w).abs() > 0.5 {
            ui.data_mut(|d| d.insert_temp(width_id, took));
            ui.ctx().request_discard("the side panel's actions changed width");
        }
    }
    let mut bar = ui.new_child(
        UiBuilder::new()
            .max_rect(Rect::from_min_max(
                pos2(fixed_left, buttons.top()),
                buttons.max,
            ))
            .layout(Layout::left_to_right(Align::Center)),
    );
    bar.spacing_mut().item_spacing.x = gap;
    let m = bar_button(
        &mut bar,
        t,
        "⋯",
        &format!("{} menu", panel.title),
        "Dock, float, collapse, hide, reset width",
    );
    egui::Popup::menu(&m).show(|ui| menu(ui, t, panel, state));
    if floating {
        if bar_button(
            &mut bar,
            t,
            "◫",
            &format!("Dock {}", panel.title),
            "Dock it again on its edge",
        )
        .clicked()
        {
            state.floating = false;
        }
        let hide = bar_button(
            &mut bar,
            t,
            "×",
            &format!("Hide {}", panel.title),
            "Hide (Ctrl+Shift+B); its tab on the edge brings it back",
        );
        if hide.clicked() {
            state.hide();
            if hide.has_focus() {
                super::focus_next(ui, panel.id, super::FocusNext::EdgeTab);
            }
        }
    } else {
        let glyph = match state.side {
            Side::Left => "«",
            Side::Right => "»",
        };
        let collapse = bar_button(
            &mut bar,
            t,
            glyph,
            &format!("Collapse {}", panel.title),
            "Collapse to its rail (Ctrl+B)",
        );
        if collapse.clicked() {
            state.shown = Shown::Rail;
            if collapse.has_focus() {
                super::focus_next(ui, panel.id, super::FocusNext::RailFoot);
            }
        }
    }
}

/// The floating panel's head: drag to move; dropped near a window edge, it docks there.
fn drag_head(
    ui: &Ui,
    t: &Tokens,
    id: Id,
    title: &str,
    state: &mut SidePanelState,
    rect: Rect,
    bounds: Rect,
) {
    let r = ui
        .interact(rect, id.with("drag head"), Sense::drag())
        .on_hover_cursor(CursorIcon::Grab);
    let name = format!("{title} drag head");
    r.widget_info(|| WidgetInfo::labeled(WidgetType::Other, true, &name));
    if r.dragged() {
        let at = state.float_pos().unwrap_or(rect.min) + r.drag_delta();
        state.float_at = Some([at.x.round(), at.y.round()]);
    }
    if r.drag_stopped()
        && let Some(p) = r.interact_pointer_pos()
    {
        let reach = t.chrome.rail_width;
        if p.x < bounds.left() + reach {
            state.side = Side::Left;
            state.floating = false;
        } else if p.x > bounds.right() - reach {
            state.side = Side::Right;
            state.floating = false;
        }
    }
}

/// The ⋯ menu: dock left, dock right, float, collapse to rail, hide, reset width.
fn menu<K>(ui: &mut Ui, t: &Tokens, panel: &SidePanel<'_, K>, state: &mut SidePanelState) {
    ui.set_min_width(t.chrome.side_panel_min);
    ui.label(
        RichText::new(format!("{} panel", panel.title).to_uppercase())
            .font(t.type_scale.caption.font_id())
            .color(t.colors.text_muted),
    );
    let left = !state.floating && state.side == Side::Left;
    let right = !state.floating && state.side == Side::Right;
    if ui.add(Button::selectable(left, "Dock left")).clicked() {
        state.side = Side::Left;
        state.floating = false;
    }
    if ui.add(Button::selectable(right, "Dock right")).clicked() {
        state.side = Side::Right;
        state.floating = false;
    }
    if ui
        .add(Button::selectable(state.floating, "Float"))
        .clicked()
    {
        state.floating = true;
    }
    let key = |k: Option<egui::KeyboardShortcut>| {
        k.map(|k| ui.ctx().format_shortcut(&k)).unwrap_or_default()
    };
    let collapse = key(panel.collapse_key);
    let hide = key(panel.hide_key);
    if ui
        .add(Button::new("Collapse to rail").shortcut_text(collapse))
        .clicked()
    {
        state.shown = Shown::Rail;
        state.floating = false;
    }
    if ui.add(Button::new("Hide").shortcut_text(hide)).clicked() {
        state.hide();
    }
    ui.separator();
    let reset = ui.add_enabled(state.set_width().is_some(), Button::new("Reset width"));
    if reset.clicked() {
        state.width = None;
    }
    reset.on_disabled_hover_text("Already at the default width");
}

/// The filter field: search glyph, the text, then the `/` chip while empty or a clear button.
fn filter_field<K>(
    panel: &SidePanel<'_, K>,
    ui: &mut Ui,
    t: &Tokens,
    state: &mut SidePanelState,
    rect: Rect,
) {
    let te_id = panel.id.with("filter");
    let focused = ui.memory(|m| m.has_focus(te_id));
    let theme = if ui.visuals().dark_mode {
        Theme::Dark
    } else {
        Theme::Light
    };
    {
        let p = ui.painter();
        fill(
            p,
            rect,
            t.radius.m,
            if focused {
                t.colors.focus
            } else {
                t.colors.line
            },
        );
        fill(
            p,
            rect.shrink(t.stroke.thin),
            t.radius.m,
            crate::props::input_fill(t, theme),
        );
        let icon = Rect::from_min_size(rect.min, vec2(t.space.xxl, rect.height()));
        paint::text_in(p, icon, "🔍", text_font(t), t.colors.text_muted);
    }
    let end = rect.right() - t.space.s;
    let chip_w = t.space.xl + t.space.s;
    let text_rect = Rect::from_min_max(
        pos2(rect.left() + t.space.xxl, rect.top()),
        pos2(end - chip_w, rect.bottom()),
    );
    let hint = panel
        .filter_hint
        .clone()
        .unwrap_or_else(|| format!("Filter {}", panel.noun));
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(text_rect)
            .layout(Layout::left_to_right(Align::Center)),
    );
    let r = child.add_sized(
        text_rect.size(),
        TextEdit::singleline(&mut state.filter)
            .id(te_id)
            .frame(Frame::NONE)
            .hint_text(RichText::new(hint).color(t.colors.text_muted))
            .font(text_font(t))
            .margin(Margin::ZERO)
            .vertical_align(Align::Center),
    );
    if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Escape)) {
        state.filter.clear();
    }
    if super::take_focus(ui, panel.id, super::FocusNext::Filter) {
        r.request_focus();
    }
    if !ui.ctx().egui_wants_keyboard_input()
        && ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Slash))
    {
        r.request_focus();
    }
    let chip = Rect::from_min_max(pos2(end - chip_w, rect.top()), pos2(end, rect.bottom()));
    let mut chip_ui = ui.new_child(
        UiBuilder::new()
            .max_rect(chip)
            .layout(Layout::right_to_left(Align::Center)),
    );
    if state.filter.is_empty() {
        key_chip(&mut chip_ui, t, "/");
    } else {
        let x = chip_ui
            .add(Button::new(RichText::new("×").font(mono_font(t))).frame(false))
            .on_hover_text("Clear the filter (Esc)");
        x.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, "Clear the filter"));
        if x.clicked() {
            state.filter.clear();
            // The button goes with the text: the focus goes back to the field.
            r.request_focus();
        }
    }
}

/// The footer: a `line` on top, a muted caption.
fn footer(ui: &Ui, t: &Tokens, rect: Rect, text: &str) {
    let p = ui.painter();
    fill(
        p,
        Rect::from_min_max(rect.min, pos2(rect.right(), rect.top() + t.stroke.thin)),
        0,
        t.colors.line,
    );
    let g = line(
        p,
        text,
        t.type_scale.caption.font_id(),
        t.colors.text_muted,
        rect.width() - t.space.l * 2.0,
    );
    galley_at(
        p,
        g,
        rect.left() + t.space.l,
        rect.center().y,
        t.colors.text_muted,
    );
}

/// The resize handle on a docked panel's inner edge, or a floating panel's right edge. Drag, Ctrl+Alt+← / →
/// with focus in the panel (← → alone on the handle), double click to reset; docked, under the minimum it
/// snaps to the rail.
#[expect(
    clippy::too_many_arguments,
    reason = "the panel's parts, each needed once"
)]
pub(crate) fn resize(
    ui: &Ui,
    t: &Tokens,
    id: Id,
    title: &str,
    state: &mut SidePanelState,
    rect: Rect,
    rail_w: f32,
    bounds: Rect,
) {
    // The edge that moves: the inner one docked, the right one floating (it grows to the right like a panel
    // docked left).
    let floating = state.floating;
    let side = if floating { Side::Left } else { state.side };
    let edge = match side {
        Side::Left => rect.right(),
        Side::Right => rect.left(),
    };
    let hit = Rect::from_min_max(
        pos2(edge - t.space.s, rect.top()),
        pos2(edge + t.space.s, rect.bottom()),
    );
    let rid = id.with("resize");
    // The handle reaches past the panel's edge over the work beside it, whose widgets are registered later in
    // the same layer and would win the press there: it lives in an area of its own above them.
    let r = Area::new(rid.with("area"))
        .order(if floating {
            Order::Foreground
        } else {
            Order::Middle
        })
        .fixed_pos(hit.min)
        .show(ui.ctx(), |ui| {
            ui.allocate_rect(hit, Sense::click_and_drag())
        })
        .inner
        .on_hover_cursor(CursorIcon::ResizeColumn)
        .on_hover_text("Drag to resize; double click resets; ← → step");
    let (min, max) = limits(t, bounds);
    let current = width(t, state, bounds);
    let name = format!("Resize {title}");
    r.widget_info(|| WidgetInfo::slider(true, f64::from(current), &name));
    // Where on the handle the drag began, from the edge: the edge keeps that distance to the pointer.
    let grab_id = rid.with("grab");
    if r.drag_started() {
        if floating && state.float_at.is_none() {
            // Pinned where it is, so only its right edge moves.
            state.float_at = Some([rect.left(), rect.top()]);
        }
        ui.data_mut(|d| d.insert_temp(rid, current));
        let grab = r.interact_pointer_pos().map_or(0.0, |p| p.x - edge);
        ui.data_mut(|d| d.insert_temp(grab_id, grab));
    }
    // The width before the drag: only a real drag (not a click) resizes.
    let before = ui.data(|d| d.get_temp::<f32>(rid));
    let grab = ui.data(|d| d.get_temp::<f32>(grab_id)).unwrap_or(0.0);
    if r.drag_stopped() {
        ui.data_mut(|d| d.remove_temp::<f32>(rid));
        ui.data_mut(|d| d.remove_temp::<f32>(grab_id));
    }
    // The list's rows reach under the edge and win egui's click hit test there, so the double click is read
    // from the pointer, as the drag is.
    let double = r.contains_pointer()
        && ui.input(|i| {
            i.pointer
                .button_double_clicked(egui::PointerButton::Primary)
        });
    if double {
        state.width = None;
    } else if (r.dragged() || r.drag_stopped())
        && before.is_some()
        && let Some(p) = r.interact_pointer_pos()
    {
        let x = p.x - grab;
        let raw = match side {
            Side::Left => x - rect.left() - rail_w,
            Side::Right => rect.right() - x - rail_w,
        }
        .round();
        if r.drag_stopped() && raw < min && !floating {
            state.shown = Shown::Rail;
            state.width = before;
        } else {
            state.width = Some(raw.clamp(min, max));
            let text = if raw < min && !floating {
                "rail".to_owned()
            } else {
                format!("{} px", raw.clamp(min, max))
            };
            // On the panel's own footer row by the edge, never over the work.
            let y = rect.bottom() - t.chrome.row * 0.5;
            readout(ui, t, rid, pos2(edge, y), side, text);
        }
    }
    let focus_inside = r.has_focus()
        || ui
            .memory(|m| m.focused())
            .and_then(|f| ui.ctx().read_response(f))
            .is_some_and(|f| rect.contains_rect(f.rect));
    if r.has_focus() {
        // ← → resize while the handle has the focus, instead of moving the focus.
        ui.memory_mut(|m| {
            m.set_focus_lock_filter(
                r.id,
                egui::EventFilter {
                    horizontal_arrows: true,
                    ..Default::default()
                },
            );
        });
    }
    if focus_inside {
        let mods = if r.has_focus() {
            Modifiers::NONE
        } else {
            Modifiers::COMMAND | Modifiers::ALT
        };
        let (grow, shrink) = match side {
            Side::Left => (Key::ArrowRight, Key::ArrowLeft),
            Side::Right => (Key::ArrowLeft, Key::ArrowRight),
        };
        let step = t.space.xl;
        let pressed = |key| {
            ui.input_mut(|i| {
                i.consume_key(mods, key)
                    || (mods == Modifiers::NONE
                        && i.consume_key(Modifiers::COMMAND | Modifiers::ALT, key))
            })
        };
        if pressed(grow) {
            state.width = Some((current + step).min(max));
        }
        if pressed(shrink) {
            if current - step < min && !floating {
                state.shown = Shown::Rail;
            } else {
                state.width = Some((current - step).max(min));
            }
        }
    }
    if r.hovered() || r.dragged() || r.has_focus() {
        handle(ui, t, rect, edge, side);
    }
}

/// The handle's look: a line `stroke.thick + stroke.thin` wide in `focus` inside the edge, and a grip.
fn handle(ui: &Ui, t: &Tokens, rect: Rect, edge: f32, side: Side) {
    let w = t.stroke.thick + t.stroke.thin;
    let x0 = match side {
        Side::Left => edge - w,
        Side::Right => edge,
    };
    let p = ui.ctx().layer_painter(LayerId::new(
        Order::Foreground,
        Id::new("ve_theme::panel handle"),
    ));
    fill(
        &p,
        Rect::from_min_max(pos2(x0, rect.top()), pos2(x0 + w, rect.bottom())),
        0,
        t.colors.focus,
    );
    let grip = Rect::from_center_size(
        pos2(x0 + w * 0.5, rect.center().y),
        vec2(t.space.s + t.stroke.thick, t.space.xxl),
    );
    fill(&p, grip, t.radius.s, t.colors.focus);
}

/// The width while dragging, in a small box inside the panel against its edge.
fn readout(ui: &Ui, t: &Tokens, id: Id, at: egui::Pos2, side: Side, text: String) {
    let p = ui.ctx().layer_painter(LayerId::new(Order::Tooltip, id));
    let g = p.layout_no_wrap(text, mono_font(t), t.colors.text);
    let size = g.size() + vec2(t.space.m * 2.0, t.space.s * 2.0);
    let x = match side {
        Side::Left => at.x - t.space.m - size.x,
        Side::Right => at.x + t.space.m,
    };
    let rect = Rect::from_min_size(pos2(x, at.y - size.y * 0.5), size);
    fill(&p, rect, t.radius.m, t.colors.line_strong);
    fill(
        &p,
        rect.shrink(t.stroke.thin),
        t.radius.m,
        t.colors.surface_raised,
    );
    paint::text_in(&p, rect, g.text(), mono_font(t), t.colors.text);
}

/// A hidden panel's vertical tab on its edge; a click brings the panel back.
pub(crate) fn edge_tab(
    ui: &Ui,
    t: &Tokens,
    id: Id,
    title: &str,
    state: &mut SidePanelState,
    bounds: Rect,
) -> Rect {
    let font = t.type_scale.caption.font_id();
    let g = ui
        .painter()
        .layout_no_wrap(title.to_owned(), font, t.colors.text_muted);
    let size = vec2(t.space.xl + t.space.s, g.size().x + t.space.m * 2.0);
    let x = match state.side {
        Side::Left => bounds.left(),
        Side::Right => bounds.right() - size.x,
    };
    let min = pos2(x, bounds.top() + t.chrome.panel_header).round();
    Area::new(id.with("edge tab"))
        .order(Order::Middle)
        .fixed_pos(min)
        .show(ui.ctx(), |ui| {
            let (rect, r) = ui.allocate_exact_size(size, Sense::click());
            let p = ui.painter();
            let hot = r.hovered() || r.has_focus();
            fill(
                p,
                rect,
                t.radius.m,
                if hot {
                    t.colors.focus
                } else {
                    t.colors.line_strong
                },
            );
            fill(
                p,
                rect.shrink(t.stroke.thin),
                t.radius.m,
                if hot {
                    t.colors.hover
                } else {
                    t.colors.surface
                },
            );
            let pos = snap_pos(
                p,
                pos2(
                    rect.center().x - g.size().y * 0.5,
                    rect.bottom() - t.space.m,
                ),
            );
            p.add(TextShape::new(pos, g.clone(), t.colors.text_muted).with_angle(-FRAC_PI_2));
            let name = format!("Show {title}");
            r.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, &name));
            if super::take_focus(ui, id, super::FocusNext::EdgeTab) {
                r.request_focus();
            }
            let focused = r.has_focus();
            if r.on_hover_text(format!("Show the {title} panel (Ctrl+Shift+B)"))
                .clicked()
            {
                state.unhide();
                if focused {
                    let next = if state.shown == Shown::Rail {
                        super::FocusNext::RailFoot
                    } else {
                        super::FocusNext::Filter
                    };
                    super::focus_next(ui, id, next);
                }
            }
            rect
        })
        .inner
}

/// `pos` moved so a window of `size` there stays inside `bounds` (its top left wins when it cannot fit).
pub(crate) fn clamp_float(pos: egui::Pos2, size: egui::Vec2, bounds: Rect) -> egui::Pos2 {
    let x = pos.x.min(bounds.right() - size.x).max(bounds.left());
    let y = pos.y.min(bounds.bottom() - size.y).max(bounds.top());
    if x.is_finite() && y.is_finite() {
        pos2(x, y).round()
    } else {
        bounds.min
    }
}

/// The panel as a window of its own: a raised frame with the window shadow, its header the drag head.
#[expect(
    clippy::too_many_arguments,
    reason = "the panel's parts, each needed once"
)]
pub(crate) fn floating<K: RowKey>(
    mut panel: SidePanel<'_, K>,
    ui: &Ui,
    t: &Arc<Tokens>,
    state: &mut SidePanelState,
    groups: &[Group<K>],
    out: &mut PanelResponse<K>,
    reveal: Option<K>,
    bounds: Rect,
) -> Rect {
    let w = width(t, state, bounds);
    // A margin of `space.xxl` above and below, never taller than the window.
    let h = (bounds.height() - t.space.xxl * 2.0).max(0.0).round();
    let default = match state.side {
        Side::Left => pos2(bounds.left() + t.space.xxl, bounds.top() + t.space.xxl),
        Side::Right => pos2(bounds.right() - t.space.xxl - w, bounds.top() + t.space.xxl),
    };
    // Kept inside the window, also after the window shrinks, so a drag back moves it at once.
    let pos = clamp_float(state.float_pos().unwrap_or(default), vec2(w, h), bounds);
    if state.float_at.is_some() {
        state.float_at = Some([pos.x, pos.y]);
    }
    let rect = Area::new(panel.id.with("float"))
        .order(Order::Middle)
        .fixed_pos(pos)
        .constrain_to(bounds)
        .show(ui.ctx(), |ui| {
            Frame::new()
                .corner_radius(t.radius.l)
                .shadow(t.shadow.window)
                .show(ui, |ui| {
                    let (rect, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
                    fill(ui.painter(), rect, t.radius.l, t.colors.line_strong);
                    fill(
                        ui.painter(),
                        rect.shrink(t.stroke.thin),
                        t.radius.l,
                        t.colors.surface,
                    );
                    let mut child =
                        ui.new_child(UiBuilder::new().max_rect(rect.shrink(t.stroke.thin)));
                    body(
                        &mut panel,
                        &mut child,
                        t,
                        state,
                        groups,
                        out,
                        reveal,
                        Some(bounds),
                    );
                    rect
                })
                .inner
        })
        .inner;
    resize(ui, t, panel.id, &panel.title, state, rect, 0.0, bounds);
    rect
}
