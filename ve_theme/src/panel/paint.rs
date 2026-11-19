//! Small painters every part of the panel shares: pixel-aligned fills, the focus ring, icons, pills.

use egui::emath::GuiRounding as _;
use egui::text::{LayoutJob, TextWrapping};
use egui::{
    Align2, Color32, CornerRadius, FontId, Galley, Painter, Pos2, Rect, Response, Sense, Ui, Vec2,
    WidgetInfo, WidgetType, pos2, vec2,
};
use std::sync::Arc;

use super::model::Icon;
use crate::Tokens;

/// `rect` with every edge on a whole pixel.
pub(crate) fn snap(painter: &Painter, rect: Rect) -> Rect {
    rect.round_to_pixels(painter.pixels_per_point())
}

/// `pos` on a whole pixel.
pub(crate) fn snap_pos(painter: &Painter, pos: Pos2) -> Pos2 {
    pos.round_to_pixels(painter.pixels_per_point())
}

/// A fill with every edge on a whole pixel, its corners rounded to `radius` points.
pub(crate) fn fill(painter: &Painter, rect: Rect, radius: u8, color: Color32) {
    if color != Color32::TRANSPARENT {
        painter.rect_filled(snap(painter, rect), CornerRadius::same(radius), color);
    }
}

/// The 1 px inset focus ring (S22, the pick): four whole-pixel bars inside `rect`, never a stroked outline.
pub(crate) fn focus_ring(painter: &Painter, t: &Tokens, rect: Rect) {
    let r = snap(painter, rect);
    let w = t.stroke.thin;
    let c = t.colors.focus;
    for bar in [
        Rect::from_min_max(r.min, pos2(r.max.x, r.min.y + w)),
        Rect::from_min_max(pos2(r.min.x, r.max.y - w), r.max),
        Rect::from_min_max(pos2(r.min.x, r.min.y + w), pos2(r.min.x + w, r.max.y - w)),
        Rect::from_min_max(pos2(r.max.x - w, r.min.y + w), pos2(r.max.x, r.max.y - w)),
    ] {
        fill(painter, bar, 0, c);
    }
}

/// A vertical bar `width` wide on the left of `rect` (the selected row's mark, the active rail item's).
pub(crate) fn left_bar(painter: &Painter, rect: Rect, width: f32, color: Color32) {
    let r = snap(painter, rect);
    fill(
        painter,
        Rect::from_min_max(r.min, pos2(r.min.x + width, r.max.y)),
        0,
        color,
    );
}

/// The chrome's text font at `t.chrome.text_size`.
pub(crate) fn text_font(t: &Tokens) -> FontId {
    FontId::proportional(t.chrome.text_size)
}

/// Mono for values and counts: the caption size.
pub(crate) fn mono_font(t: &Tokens) -> FontId {
    FontId::monospace(t.type_scale.caption.size)
}

/// One line of `text`, cut with an ellipsis past `max_width`.
pub(crate) fn line(
    painter: &Painter,
    text: &str,
    font: FontId,
    color: Color32,
    max_width: f32,
) -> Arc<Galley> {
    let mut job = LayoutJob::simple_singleline(text.to_owned(), font, color);
    job.wrap = TextWrapping::truncate_at_width(max_width.max(0.0));
    painter.layout_job(job)
}

/// Draw `galley` with its left edge at `x`, centred on `cy`, its corner on a whole pixel.
pub(crate) fn galley_at(painter: &Painter, galley: Arc<Galley>, x: f32, cy: f32, color: Color32) {
    let pos = snap_pos(painter, pos2(x, cy - galley.size().y * 0.5));
    painter.galley(pos, galley, color);
}

/// Draw `icon` in a square of side `side` centred on `center`.
pub(crate) fn icon(
    painter: &Painter,
    t: &Tokens,
    icon: &Icon,
    center: Pos2,
    side: f32,
    color: Color32,
) {
    match icon {
        Icon::Glyph(g) => {
            let galley =
                painter.layout_no_wrap(g.to_string(), FontId::monospace(t.chrome.text_size), color);
            let pos = snap_pos(painter, center - galley.size() * 0.5);
            painter.galley(pos, galley, color);
        }
        Icon::Paint(p) => {
            let rect = snap(painter, Rect::from_center_size(center, Vec2::splat(side)));
            p(painter, rect, color);
        }
    }
}

/// A count pill: `primary` fill, the number in mono, at least as wide as tall; returns its width.
pub(crate) fn pill(painter: &Painter, t: &Tokens, right: f32, cy: f32, n: u32) -> f32 {
    let ink = t.on(t.colors.primary);
    let g = painter.layout_no_wrap(n.to_string(), mono_font(t), ink);
    let h = t.space.xl;
    let w = (g.size().x + t.space.s * 2.0).max(h);
    let rect = snap(
        painter,
        Rect::from_min_size(pos2(right - w, cy - h * 0.5), vec2(w, h)),
    );
    painter.rect_filled(rect, CornerRadius::same((h * 0.5) as u8), t.colors.primary);
    let pos = snap_pos(painter, rect.center() - g.size() * 0.5);
    painter.galley(pos, g, ink);
    w
}

/// A square bar button with a glyph: hover fill, its tooltip, a name for screen readers and tests.
pub(crate) fn bar_button(
    ui: &mut Ui,
    t: &Tokens,
    glyph: &str,
    name: &str,
    tooltip: &str,
) -> Response {
    let side = t.chrome.bar_button;
    let (rect, r) = ui.allocate_exact_size(Vec2::splat(side), Sense::click());
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        if r.hovered() || r.is_pointer_button_down_on() {
            fill(p, rect, t.radius.m, t.colors.hover);
        }
        let color = if r.hovered() {
            t.colors.text
        } else {
            t.colors.text_muted
        };
        let g = p.layout_no_wrap(
            glyph.to_owned(),
            FontId::monospace(t.chrome.text_size),
            color,
        );
        let pos = snap_pos(p, rect.center() - g.size() * 0.5);
        p.galley(pos, g, color);
        if r.has_focus() {
            focus_ring(p, t, rect);
        }
    }
    r.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, name));
    r.on_hover_text(tooltip)
}

/// A key chip: the chord in mono on a hairline box.
pub(crate) fn key_chip(ui: &mut Ui, t: &Tokens, text: &str) {
    let g = ui
        .painter()
        .layout_no_wrap(text.to_owned(), mono_font(t), t.colors.text_muted);
    let size = g.size() + vec2(t.space.s * 2.0, t.space.xs * 2.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let p = ui.painter();
    let r = snap(p, rect);
    fill(p, r, t.radius.s, t.colors.line);
    fill(
        p,
        r.shrink(t.stroke.thin),
        t.radius.s,
        t.colors.surface_raised,
    );
    p.galley(
        snap_pos(p, r.center() - g.size() * 0.5),
        g,
        t.colors.text_muted,
    );
}

/// Text centred in `rect`, rounded to whole pixels.
pub(crate) fn text_in(painter: &Painter, rect: Rect, text: &str, font: FontId, color: Color32) {
    let g = painter.layout_no_wrap(text.to_owned(), font, color);
    let pos = Align2::CENTER_CENTER
        .align_size_within_rect(g.size(), rect)
        .min;
    painter.galley(snap_pos(painter, pos), g, color);
}
