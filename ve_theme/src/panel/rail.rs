//! The rail: a column of icons, one per group (the panel collapsed, or form B3) or per panel view (form B1),
//! each with its light, its unread count and a hover card.

use egui::{
    CursorIcon, Id, Rect, Response, RichText, Sense, Ui, Vec2, WidgetInfo, WidgetType, pos2, vec2,
};

use super::model::Icon;
use super::paint::{self, fill, focus_ring, key_chip};
use super::state::Side;
use crate::{LightStyle, Tokens, Tone};

/// One rail item.
pub(crate) struct RailItem<'a> {
    pub icon: Option<&'a Icon>,
    pub name: &'a str,
    pub summary: Option<&'a str>,
    pub key: Option<&'a str>,
    pub light: Option<Tone>,
    pub unread: u32,
    pub active: bool,
}

/// Draw `items` top down in `ui` (a rail-wide column); returns the index clicked.
pub(crate) fn items(
    ui: &mut Ui,
    t: &Tokens,
    id: Id,
    lights: LightStyle,
    side: Side,
    items: &[RailItem<'_>],
) -> Option<usize> {
    let button = t.chrome.bar_button;
    let mut clicked = None;
    ui.add_space(t.space.s);
    for (i, item) in items.iter().enumerate() {
        let row = ui
            .allocate_space(vec2(ui.available_width(), button + t.space.s))
            .1;
        let rect = Rect::from_center_size(
            pos2(row.center().x, row.top() + button * 0.5),
            Vec2::splat(button),
        );
        let r = ui.interact(rect, id.with(("rail", i)), Sense::click());
        paint_item(ui, t, (lights, side), item, (row, rect), &r);
        let name = item.name.to_owned();
        r.widget_info(|| WidgetInfo::selected(WidgetType::Button, true, item.active, &name));
        let r = r
            .on_hover_cursor(CursorIcon::PointingHand)
            .on_hover_ui(|ui| card(ui, t, item));
        if r.clicked() {
            clicked = Some(i);
        }
    }
    clicked
}

/// One item: its ground, the active bar on the window's edge of the rail, the icon, then its unread pill or
/// light in the top corner. `place` is the rail's lights and side, `rects` the item's row and button.
fn paint_item(
    ui: &Ui,
    t: &Tokens,
    place: (LightStyle, Side),
    item: &RailItem<'_>,
    rects: (Rect, Rect),
    r: &Response,
) {
    let (lights, side) = place;
    let (row, rect) = rects;
    if !ui.is_rect_visible(row) {
        return;
    }
    let p = ui.painter();
    let ground = if item.active {
        t.colors.selection
    } else if r.hovered() {
        t.colors.hover
    } else {
        t.colors.surface
    };
    if item.active || r.hovered() {
        fill(p, rect, t.radius.m, ground);
    }
    if item.active {
        let x = match side {
            Side::Left => row.left(),
            Side::Right => row.right() - t.stroke.thick,
        };
        paint::left_bar(
            p,
            Rect::from_min_max(pos2(x, rect.top()), pos2(x + t.stroke.thick, rect.bottom())),
            t.stroke.thick,
            t.colors.focus,
        );
    }
    let ink = if item.active || r.hovered() {
        t.colors.text
    } else {
        t.colors.text_muted
    };
    match item.icon {
        Some(icon) => paint::icon(p, t, icon, rect.center(), t.space.xl, ink),
        None => {
            let letter: String = item.name.chars().take(1).collect();
            paint::text_in(p, rect, &letter, paint::text_font(t), ink);
        }
    }
    if item.unread > 0 {
        // Inside the rail's top: the pill's top meets the button's.
        let cy = rect.top() + t.space.xl * 0.5;
        paint::pill(p, t, rect.right() + t.space.s, cy, item.unread);
    } else if let Some(tone) = item.light {
        let s = lights.side(t, p.pixels_per_point());
        lights.paint(
            p,
            t,
            tone,
            pos2(rect.right() - s * 0.5, rect.top() + s * 0.5),
        );
    }
    if r.has_focus() {
        focus_ring(p, t, rect);
    }
}

/// The hover card: the item's name and its key, then its summary.
fn card(ui: &mut Ui, t: &Tokens, item: &RailItem<'_>) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(item.name).strong().color(t.colors.text));
        if let Some(key) = item.key {
            ui.add_space(t.space.m);
            key_chip(ui, t, key);
        }
    });
    if let Some(s) = item.summary {
        ui.label(RichText::new(s).color(t.colors.text_muted));
    }
}

/// A square button at the rail's foot (expand); returns its response.
pub(crate) fn foot(ui: &mut Ui, t: &Tokens, glyph: &str, name: &str, tip: &str) -> Response {
    let side = t.chrome.bar_button;
    let full = ui.max_rect();
    let rect = Rect::from_center_size(
        pos2(full.center().x, full.bottom() - t.space.s - side * 0.5),
        Vec2::splat(side),
    );
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
    paint::bar_button(&mut child, t, glyph, name, tip)
}
