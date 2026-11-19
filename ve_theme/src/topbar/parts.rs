//! The bar's parts, each placed at an x and drawn on the pixel grid: logo, wordmark, badge, menu button,
//! breadcrumb, command field, classic title, search button, global actions, window controls. Icons are
//! [`crate::icons`]'s; sizes and colours are [`Tokens`].

use std::sync::Arc;

use egui::{
    Color32, CursorIcon, FontId, Galley, Id, Modifiers, Rect, Response, Sense, Ui, WidgetInfo,
    WidgetType, pos2, vec2,
};
use ve_basics::actions::{Actions, Chord, Source};
use ve_basics::{AppMenu, AppMenuAction, BuildInfo};

use super::pixels::Pixels;
use super::text::{fit_end, fit_middle};
use super::{
    Brand, FIELD_HINT, FIELD_LABEL, Logo, MENU_LABEL, MenuButton, PALETTE_ACTION, Problem,
};
use crate::Tokens;
use crate::icons::{Icon, IconImage, ph};

/// The separator between the badge's parts.
const BADGE_SEPARATOR: &str = " · ";

/// What every part needs: tokens, the bar's rectangle, its id, the pixel size.
pub(crate) struct Ctx {
    pub tokens: Arc<Tokens>,
    pub rect: Rect,
    pub id: Id,
    pub ppp: f32,
}

/// Whether a button can be pressed, and whether its toggle is on.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Pressable {
    /// Cannot run now; its tooltip says why.
    Disabled,
    Ready,
    /// A toggle that is on.
    On,
}

/// How a pressable part looks now.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Look {
    Rest,
    Hover,
    Pressed,
}

impl Look {
    pub(crate) fn of(r: &Response, open: bool) -> Self {
        if open || r.is_pointer_button_down_on() {
            Self::Pressed
        } else if r.hovered() {
            Self::Hover
        } else {
            Self::Rest
        }
    }
}

/// A chord as key chips, in the platform's order and words: `Ctrl`, `Shift`, `K`; `⇧`, `⌘`, `K` on macOS.
pub(crate) fn chord_keys(ctx: &egui::Context, chord: Chord) -> Vec<String> {
    let mac = ctx.os().is_mac();
    let order = if mac {
        [
            (chord.alt, Modifiers::ALT),
            (chord.shift, Modifiers::SHIFT),
            (chord.command, Modifiers::MAC_CMD),
        ]
    } else {
        [
            (chord.command, Modifiers::CTRL),
            (chord.alt, Modifiers::ALT),
            (chord.shift, Modifiers::SHIFT),
        ]
    };
    let mut keys: Vec<String> = order
        .into_iter()
        .filter(|(on, _)| *on)
        .map(|(_, m)| ctx.format_modifiers(m))
        .collect();
    keys.push(chord.key.symbol_or_name().to_owned());
    keys
}

impl Ctx {
    pub(crate) fn new(ui: &Ui, tokens: &Arc<Tokens>, rect: Rect, id: Id) -> Self {
        Self {
            tokens: tokens.clone(),
            rect,
            id,
            ppp: ui.ctx().pixels_per_point(),
        }
    }

    pub(crate) fn pixels(&self) -> Pixels {
        Pixels::new(self.ppp)
    }

    /// The chrome's text font.
    pub(crate) fn font(&self) -> FontId {
        FontId::proportional(self.tokens.chrome.text_size)
    }

    /// The badge's and key chips' monospace font.
    pub(crate) fn mono(&self) -> FontId {
        FontId::monospace(self.tokens.type_scale.caption.size)
    }

    pub(crate) fn menu_title_id(&self, name: &str) -> Id {
        self.id.with(("menu", name))
    }

    pub(crate) fn menu_popup_id(&self, name: &str) -> Id {
        self.menu_title_id(name).with("popup")
    }

    pub(crate) fn menu_button_id(&self) -> Id {
        self.id.with("menu-button")
    }

    /// A button's rectangle at `x` of `w` points, `chrome.bar_button` high, centred in the bar.
    pub(crate) fn button_rect(&self, x: f32, w: f32) -> Rect {
        let h = self.tokens.chrome.bar_button;
        Rect::from_min_size(pos2(x, self.rect.center().y - h / 2.0), vec2(w, h))
    }

    /// Lays `text` out in `font` and `color`, no wrapping.
    pub(crate) fn galley(&self, ui: &Ui, text: &str, font: FontId, color: Color32) -> Arc<Galley> {
        ui.painter().layout_no_wrap(text.to_owned(), font, color)
    }

    pub(crate) fn text_width(&self, ui: &Ui, text: &str, font: FontId) -> f32 {
        self.galley(ui, text, font, Color32::WHITE).size().x
    }

    /// Paints a galley with its left edge at `x`, centred on `cy`, on whole pixels.
    pub(crate) fn paint_galley(&self, ui: &Ui, x: f32, cy: f32, galley: Arc<Galley>) -> Rect {
        let p = self.pixels().snap_pos(pos2(x, cy - galley.size().y / 2.0));
        let r = Rect::from_min_size(p, galley.size());
        ui.painter().galley(p, galley, Color32::WHITE);
        r
    }

    /// An icon of `side` points centred in `rect`, its ink in `color`.
    pub(crate) fn icon(&self, ui: &Ui, icon: Icon, rect: Rect, side: f32, color: Color32) {
        IconImage::new(icon)
            .size(side)
            .ink(color)
            .paint(ui.painter(), &self.tokens, rect);
    }

    /// The bar's ground and its bottom line.
    pub(crate) fn ground(&self, ui: &Ui) {
        let c = &self.tokens.colors;
        let mut p = self.pixels();
        p.rect(self.rect, c.surface);
        p.hline_bottom(self.rect, c.line);
        ui.painter().add(p.finish());
    }

    /// The fill under a pressable part and its focus ring (one pixel, inside).
    pub(crate) fn plate(&self, ui: &Ui, r: &Response, rect: Rect, look: Look) {
        let c = &self.tokens.colors;
        let radius = f32::from(self.tokens.radius.m);
        let mut p = self.pixels();
        match look {
            Look::Rest => {}
            Look::Hover => p.rounded(rect, radius, c.hover),
            Look::Pressed => p.rounded(rect, radius, c.line),
        }
        if r.has_focus() {
            p.outline(rect, radius, c.focus);
        }
        ui.painter().add(p.finish());
    }

    /// The box a missing icon leaves: a one pixel `crit` outline, never nothing.
    fn missing_icon(&self, ui: &Ui, rect: Rect) {
        let side = self.tokens.chrome.icon;
        let mut p = self.pixels();
        p.outline(
            Rect::from_center_size(rect.center(), vec2(side, side)),
            0.0,
            self.tokens.colors.crit,
        );
        ui.painter().add(p.finish());
    }

    // ---------------------------------------------------------------------------------------------------
    // Widths

    pub(crate) fn wordmark_font(&self, ui: &Ui) -> FontId {
        crate::brand_font(ui.ctx(), self.tokens.chrome.wordmark_size)
    }

    pub(crate) fn wordmark_width(&self, ui: &Ui, name: &str) -> f32 {
        self.text_width(ui, name, self.wordmark_font(ui))
    }

    /// The chevron after a name or word that opens a menu, with its gap.
    pub(crate) fn chevron_width(&self) -> f32 {
        self.tokens.chrome.icon_small + self.tokens.space.s
    }

    pub(crate) fn groove_width(&self) -> f32 {
        self.tokens.space.m + self.tokens.stroke.thin
    }

    pub(crate) fn menu_title_width(&self, ui: &Ui, name: &str) -> f32 {
        self.text_width(ui, name, self.font()) + 2.0 * self.tokens.space.m
    }

    pub(crate) fn word_button_width(&self, ui: &Ui) -> f32 {
        self.text_width(ui, MENU_LABEL, self.font())
            + 2.0 * self.tokens.space.m
            + self.chevron_width()
    }

    pub(crate) fn badge_width(&self, ui: &Ui, text: &str, debug: bool) -> f32 {
        let pad = if debug {
            self.tokens.space.m
        } else {
            self.tokens.space.s
        };
        self.text_width(ui, text, self.mono()) + 2.0 * pad
    }

    /// A square bar button with its gap.
    pub(crate) fn square_width(&self) -> f32 {
        self.tokens.chrome.bar_button + self.tokens.space.s
    }

    // ---------------------------------------------------------------------------------------------------
    // Brand

    /// The logo at `x` (clear space included in its slot); a button opening the menu when `button`.
    pub(crate) fn logo(
        &self,
        ui: &mut Ui,
        x: f32,
        logo: &Logo<'_>,
        name: &str,
        button: bool,
    ) -> Response {
        let t = &self.tokens;
        let c = &t.colors;
        let side = t.chrome.logo_mark;
        let rect = self.pixels().snap(Rect::from_min_size(
            pos2(x, self.rect.center().y - side / 2.0),
            vec2(side, side),
        ));
        let (sense, id, kind, label) = if button {
            (
                Sense::click(),
                self.menu_button_id(),
                WidgetType::Button,
                format!("{name}: {MENU_LABEL}"),
            )
        } else {
            (
                Sense::hover(),
                self.id.with("logo"),
                WidgetType::Image,
                format!("{name} logo"),
            )
        };
        let r = ui.interact(rect, id, sense);
        r.widget_info(|| WidgetInfo::labeled(kind, true, &label));
        let source = match logo {
            Logo::Image(s) => Some(s.clone()),
            Logo::Themed { light, dark } => Some(if ui.visuals().dark_mode {
                dark.clone()
            } else {
                light.clone()
            }),
            Logo::Placeholder(_) | Logo::None => None,
        };
        if let Some(source) = source {
            egui::Image::new(source)
                .fit_to_exact_size(rect.size())
                .paint_at(ui, rect);
        } else if let Logo::Placeholder(mono) = logo {
            let mut p = self.pixels();
            p.rounded(rect, f32::from(t.radius.l), c.surface_raised);
            p.dashed(rect, t.space.s, t.space.xs, c.line_strong);
            ui.painter().add(p.finish());
            let font = crate::number_font(ui.ctx(), t.type_scale.caption.size);
            let g = self.galley(ui, mono, font, c.text_muted);
            self.paint_galley(ui, rect.center().x - g.size().x / 2.0, rect.center().y, g);
        }
        if !button {
            return r.on_hover_text(name);
        }
        // The chevron sits on a disc astride the mark's lower right corner, in its clear space.
        let s = t.chrome.icon_small;
        let disc = Rect::from_center_size(rect.right_bottom(), vec2(s, s));
        let mut p = self.pixels();
        p.rounded(disc, s / 2.0, c.surface_raised);
        p.outline(disc, s / 2.0, c.line_strong);
        if r.has_focus() {
            p.outline(rect, f32::from(t.radius.m), c.focus);
        }
        ui.painter().add(p.finish());
        self.icon(ui, ph::CARET_DOWN.into(), disc, s, c.text);
        r.on_hover_text(format!(
            "{name}: the menus, the folded items and the command palette"
        ))
        .on_hover_cursor(CursorIcon::PointingHand)
    }

    /// The wordmark at `x`, cut to `width`; with a chevron and a click opening the menu when `button`.
    pub(crate) fn wordmark(
        &self,
        ui: &mut Ui,
        x: f32,
        name: &str,
        width: f32,
        button: bool,
    ) -> Response {
        let t = &self.tokens;
        let font = self.wordmark_font(ui);
        let room = width - if button { self.chevron_width() } else { 0.0 };
        let shown = fit_end(name, room, |s| self.text_width(ui, s, font.clone()));
        let g = self.galley(ui, &shown, font, t.colors.red);
        if !button {
            let tr = self.paint_galley(ui, x, self.rect.center().y, g);
            let r = ui.interact(tr, self.id.with("wordmark"), Sense::hover());
            r.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, name));
            return if shown == name {
                r
            } else {
                r.on_hover_text(name)
            };
        }
        let pad = t.space.s;
        let w = g.size().x + self.chevron_width() + 2.0 * pad;
        let rect = self.button_rect(x - pad, w);
        let r = ui.interact(rect, self.menu_button_id(), Sense::click());
        r.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, name));
        let open = egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(&r));
        self.plate(ui, &r, rect, Look::of(&r, open));
        let s = t.chrome.icon_small;
        let chevron = Rect::from_min_size(
            pos2(x + g.size().x + t.space.s, self.rect.center().y - s / 2.0),
            vec2(s, s),
        );
        self.paint_galley(ui, x, self.rect.center().y, g);
        self.icon(ui, ph::CARET_DOWN.into(), chevron, s, t.colors.text_muted);
        r.on_hover_text(format!(
            "{name}: the menus, the folded items and the command palette"
        ))
        .on_hover_cursor(CursorIcon::PointingHand)
    }

    /// The menu button the row collapses into (when the name or the mark is not it).
    pub(crate) fn menu_button(&self, ui: &mut Ui, x: f32, kind: MenuButton) -> Response {
        let t = &self.tokens;
        let c = &t.colors;
        let word = kind == MenuButton::Word;
        let w = if word {
            self.word_button_width(ui)
        } else {
            t.chrome.bar_button
        };
        let rect = self.button_rect(x, w);
        let r = ui.interact(rect, self.menu_button_id(), Sense::click());
        r.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, MENU_LABEL));
        let open = egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(&r));
        self.plate(ui, &r, rect, Look::of(&r, open));
        match kind {
            MenuButton::Word => {
                let g = self.galley(ui, MENU_LABEL, self.font(), c.text);
                self.paint_galley(ui, rect.left() + t.space.m, rect.center().y, g);
                let s = t.chrome.icon_small;
                let chevron = Rect::from_min_size(
                    pos2(rect.right() - t.space.m - s, rect.center().y - s / 2.0),
                    vec2(s, s),
                );
                self.icon(ui, ph::CARET_DOWN.into(), chevron, s, c.text_muted);
            }
            MenuButton::Dots => self.icon(ui, ph::DOTS_NINE.into(), rect, t.chrome.icon, c.text),
            _ => self.icon(ui, ph::LIST.into(), rect, t.chrome.icon, c.text),
        }
        r.on_hover_text("The menus, the folded items and the command palette")
    }

    /// The build badge at `x`: a debug build as an orange pill, a release one as quiet mono text. The whole
    /// build when `full`, else the version only; the details on hover.
    pub(crate) fn badge(&self, ui: &mut Ui, x: f32, info: &BuildInfo, full: bool) -> Response {
        let t = &self.tokens;
        let c = &t.colors;
        let text = if full {
            [info.version, info.git_sha, info.profile()].join(BADGE_SEPARATOR)
        } else {
            info.version.to_owned()
        };
        let w = self.badge_width(ui, &text, info.debug);
        let h = t.type_scale.caption.line_height + t.space.s;
        let rect = self.pixels().snap(Rect::from_min_size(
            pos2(x, self.rect.center().y - h / 2.0),
            vec2(w, h),
        ));
        let r = ui.interact(rect, self.id.with("badge"), Sense::click());
        let (font, color) = if info.debug {
            (
                crate::number_font(ui.ctx(), t.type_scale.caption.size),
                c.debug.text,
            )
        } else {
            (self.mono(), c.text_muted)
        };
        let radius = if info.debug {
            h / 2.0
        } else {
            f32::from(t.radius.m)
        };
        let mut p = self.pixels();
        if info.debug {
            p.rounded(rect, radius, c.debug.fill);
            p.outline(rect, radius, c.debug.line);
        } else if r.hovered() {
            p.rounded(rect, radius, c.hover);
        }
        if r.has_focus() {
            p.outline(rect, radius, c.focus);
        }
        ui.painter().add(p.finish());
        let g = self.galley(ui, &text, font, color);
        self.paint_galley(ui, rect.center().x - g.size().x / 2.0, rect.center().y, g);
        r
    }

    /// The app menu on the badge; the action confirmed this frame.
    pub(crate) fn app_menu(&self, badge: &Response, brand: &Brand<'_>) -> Option<AppMenuAction> {
        app_menu_of(brand).attach(badge)
    }

    // ---------------------------------------------------------------------------------------------------
    // Document

    /// The breadcrumb after the name (style B): "/ name" and the unsaved dot. Returns the next x.
    pub(crate) fn breadcrumb(
        &self,
        ui: &mut Ui,
        x: f32,
        name: &str,
        path: &str,
        dirty: bool,
    ) -> f32 {
        let t = &self.tokens;
        let c = &t.colors;
        let cy = self.rect.center().y;
        let slash = self.galley(ui, "/", self.font(), c.line_strong);
        let sr = self.paint_galley(ui, x + t.space.s, cy, slash);
        let end = x + t.chrome.breadcrumb;
        let dot = if dirty {
            t.chrome.dirty_dot + t.space.m
        } else {
            0.0
        };
        let room = end - sr.right() - t.space.m - dot;
        let tr = self.document_name(ui, sr.right() + t.space.m, cy, room, name, dirty);
        let r = ui.interact(tr, self.id.with("document"), Sense::hover());
        let label = document_label(name, dirty);
        r.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, &label));
        r.on_hover_text(document_whole(name, path, dirty));
        end
    }

    /// The document's name cut to `room` and its unsaved dot; the rectangle both take.
    fn document_name(&self, ui: &Ui, x: f32, cy: f32, room: f32, name: &str, dirty: bool) -> Rect {
        let t = &self.tokens;
        let font = self.font();
        let cut = fit_middle(name, room, |s| self.text_width(ui, s, font.clone()));
        let g = self.galley(ui, &cut, font, t.colors.text);
        let mut tr = self.paint_galley(ui, x, cy, g);
        if dirty {
            let d = t.chrome.dirty_dot;
            let dot = Rect::from_min_size(pos2(tr.right() + t.space.m, cy - d / 2.0), vec2(d, d));
            let mut p = self.pixels();
            p.rounded(dot, d / 2.0, t.colors.text_muted);
            ui.painter().add(p.finish());
            tr.max.x = dot.right();
        }
        tr
    }

    /// The command field at `x`, `w` wide: a search icon, the document or the hint, the palette's key chips.
    /// The document's whole name and folder show on hover.
    pub(crate) fn field(
        &self,
        ui: &mut Ui,
        x: f32,
        w: f32,
        doc: Option<(&str, &str, bool)>,
        keys: &[String],
    ) -> Response {
        let t = &self.tokens;
        let c = &t.colors;
        let h = t.chrome.text_field;
        let rect = self.pixels().snap(Rect::from_min_size(
            pos2(x, self.rect.center().y - h / 2.0),
            vec2(w, h),
        ));
        let r = ui.interact(rect, self.id.with("field"), Sense::click());
        r.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, FIELD_LABEL));
        let radius = f32::from(t.radius.m);
        let mut p = self.pixels();
        p.rounded(rect, radius, c.bg);
        let edge = if r.has_focus() {
            c.focus
        } else if r.hovered() {
            c.line_strong
        } else {
            c.line
        };
        p.outline(rect, radius, edge);
        ui.painter().add(p.finish());
        let s = t.chrome.icon_small;
        let glass = Rect::from_min_size(
            pos2(rect.left() + t.space.m, rect.center().y - s / 2.0),
            vec2(s, s),
        );
        self.icon(ui, ph::MAGNIFYING_GLASS.into(), glass, s, c.text_muted);
        let keys: Vec<&str> = keys.iter().map(String::as_str).collect();
        let chip_w = if keys.is_empty() {
            0.0
        } else {
            self.chips(ui, rect.right() - t.space.s, rect.center().y, &keys, true) + t.space.m
        };
        let tx = glass.right() + t.space.m;
        let room = rect.right() - t.space.s - chip_w - tx;
        let tip = palette_tip(&keys);
        match doc {
            Some((name, path, dirty)) => {
                let dot = if dirty {
                    t.chrome.dirty_dot + t.space.m
                } else {
                    0.0
                };
                self.document_name(ui, tx, rect.center().y, room - dot, name, dirty);
                r.on_hover_text(format!("{}\n\n{tip}", document_whole(name, path, dirty)))
                    .on_hover_cursor(CursorIcon::Text)
            }
            None => {
                let hint = fit_middle(FIELD_HINT, room, |s| self.text_width(ui, s, self.font()));
                let g = self.galley(ui, &hint, self.font(), c.text_muted);
                self.paint_galley(ui, tx, rect.center().y, g);
                r.on_hover_text(tip).on_hover_cursor(CursorIcon::Text)
            }
        }
    }

    /// The document's name centred in the bar between `x0` and `x1` (style A).
    pub(crate) fn title(&self, ui: &mut Ui, x0: f32, x1: f32, name: &str, path: &str, dirty: bool) {
        let t = &self.tokens;
        let dot = if dirty {
            t.chrome.dirty_dot + t.space.m
        } else {
            0.0
        };
        let room = (x1 - x0 - dot).min(t.chrome.field_width);
        if room <= t.chrome.bar_button {
            return;
        }
        let w = self.text_width(ui, name, self.font()).min(room) + dot;
        let x = (self.rect.center().x - w / 2.0).clamp(x0, (x1 - w).max(x0));
        let tr = self.document_name(ui, x, self.rect.center().y, room, name, dirty);
        let r = ui.interact(tr, self.id.with("document"), Sense::hover());
        let label = document_label(name, dirty);
        r.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, &label));
        r.on_hover_text(document_whole(name, path, dirty));
    }

    /// Key chips ending at `right`, centred on `cy`; returns their width. Painted only when `paint`.
    pub(crate) fn chips(&self, ui: &Ui, right: f32, cy: f32, keys: &[&str], paint: bool) -> f32 {
        let t = &self.tokens;
        let c = &t.colors;
        let gap = t.space.xs;
        let pad = t.space.s;
        let h = t.type_scale.caption.line_height + t.space.xs;
        let galleys: Vec<_> = keys
            .iter()
            .map(|k| self.galley(ui, k, self.mono(), c.text_muted))
            .collect();
        let total: f32 = galleys.iter().map(|g| g.size().x + 2.0 * pad).sum::<f32>()
            + gap * (keys.len().saturating_sub(1)) as f32;
        if !paint {
            return total;
        }
        let mut x = right - total;
        let mut p = self.pixels();
        let mut texts = Vec::new();
        for g in galleys {
            let w = g.size().x + 2.0 * pad;
            let r = p.snap(Rect::from_min_size(pos2(x, cy - h / 2.0), vec2(w, h)));
            p.rounded(r, f32::from(t.radius.s), c.surface_raised);
            p.outline(r, f32::from(t.radius.s), c.line);
            texts.push((r, g));
            x += w + gap;
        }
        ui.painter().add(p.finish());
        for (r, g) in texts {
            self.paint_galley(ui, r.center().x - g.size().x / 2.0, r.center().y, g);
        }
        total
    }

    // ---------------------------------------------------------------------------------------------------
    // Buttons

    /// A square bar button with an icon (a `crit` box when `icon` is `None`); `label` for accessibility.
    fn icon_button(
        &self,
        ui: &mut Ui,
        x: f32,
        id: Id,
        icon: Option<Icon>,
        state: Pressable,
        label: &str,
    ) -> Response {
        let t = &self.tokens;
        let c = &t.colors;
        let enabled = state != Pressable::Disabled;
        let rect = self.button_rect(x, t.chrome.bar_button);
        let sense = if enabled {
            Sense::click()
        } else {
            Sense::hover()
        };
        let r = ui.interact(rect, id, sense);
        r.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, label));
        let look = if state == Pressable::On {
            Look::Pressed
        } else {
            Look::of(&r, false)
        };
        let ink = if !enabled {
            c.off
        } else if look == Look::Rest {
            c.text_muted
        } else {
            c.text
        };
        self.plate(ui, &r, rect, look);
        match icon {
            Some(icon) => self.icon(ui, icon, rect, t.chrome.icon, ink),
            None => self.missing_icon(ui, rect),
        }
        r
    }

    /// The search button the field collapses into.
    pub(crate) fn search_button(&self, ui: &mut Ui, x: f32, keys: &[String]) -> Response {
        let keys: Vec<&str> = keys.iter().map(String::as_str).collect();
        self.icon_button(
            ui,
            x,
            self.id.with("search"),
            Some(ph::MAGNIFYING_GLASS.into()),
            Pressable::Ready,
            FIELD_LABEL,
        )
        .on_hover_text(palette_tip(&keys))
    }

    /// A global action's button at `x`, with its icon; a missing or unknown icon is a [`Problem`].
    pub(crate) fn global(
        &self,
        ui: &mut Ui,
        x: f32,
        id: &str,
        actions: &mut Actions,
        out: &mut super::TopBarOutput,
    ) {
        let Some(state) = actions.get(id) else {
            out.problems.push(Problem::UnknownAction(id.to_owned()));
            return;
        };
        let a = state.action;
        let title = a.title();
        let enabled = state.enabled.is_ok();
        let mut tip = title.to_owned();
        if let Some(k) = state.keys.first() {
            tip.push_str(&format!(" ({})", k.label(ui.ctx())));
        }
        if !a.help_text().is_empty() {
            tip.push('\n');
            tip.push_str(a.help_text());
        }
        if let Err(why) = state.enabled {
            tip.push('\n');
            tip.push_str(why);
        }
        let pressable = if !enabled {
            Pressable::Disabled
        } else if state.checked == Some(true) {
            Pressable::On
        } else {
            Pressable::Ready
        };
        let icon = a.icon_name().and_then(Icon::named);
        if icon.is_none() {
            out.problems.push(Problem::UnknownIcon {
                action: a.id(),
                icon: a.icon_name(),
            });
        }
        let r = self
            .icon_button(ui, x, self.id.with(("global", id)), icon, pressable, title)
            .on_hover_text(tip);
        if r.clicked()
            && let Err(e) = actions.trigger(id, Source::Toolbar)
        {
            out.refused.push(e);
        }
    }

    /// The window controls from `x`: at the left end close, minimise, maximise (x is their left edge); at the
    /// right end minimise, maximise, close (x is their right edge). Returns the other edge.
    pub(crate) fn window_controls(&self, ui: &mut Ui, x: f32, left: bool) -> f32 {
        let t = &self.tokens;
        let c = &t.colors;
        let w = t.chrome.window_control;
        let maximised = ui.input(|i| i.viewport().maximized.unwrap_or(false));
        let max = if maximised {
            (ph::COPY, "Restore")
        } else {
            (ph::SQUARE, "Maximise")
        };
        let order = if left {
            [(ph::X, "Close"), (ph::MINUS, "Minimise"), max]
        } else {
            [(ph::MINUS, "Minimise"), max, (ph::X, "Close")]
        };
        let x0 = if left { x } else { x - 3.0 * w };
        let mut p = self.pixels();
        let inset = t.space.l;
        let rule = if left { x0 + 3.0 * w } else { x0 };
        p.vline(
            rule,
            self.rect.top() + inset,
            self.rect.bottom() - inset,
            c.line,
        );
        let px = p.px();
        let mut icons = Vec::new();
        for (i, (glyph, label)) in order.into_iter().enumerate() {
            let rect = Rect::from_min_max(
                pos2(x0 + i as f32 * w, self.rect.top()),
                pos2(x0 + (i + 1) as f32 * w, self.rect.bottom() - px),
            );
            let r = ui.interact(rect, self.id.with(("control", label)), Sense::click());
            r.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, label));
            let close = label == "Close";
            let hot = r.hovered() || r.has_focus();
            let ink = if close && hot {
                p.rect(rect, c.crit);
                t.on(c.crit)
            } else {
                if hot {
                    p.rect(rect, c.hover);
                }
                c.text
            };
            if r.has_focus() {
                p.outline(rect, 0.0, c.focus);
            }
            icons.push((glyph, rect, ink));
            let r = r.on_hover_text(label);
            if r.clicked() {
                let cmd = match label {
                    "Close" => egui::ViewportCommand::Close,
                    "Minimise" => egui::ViewportCommand::Minimized(true),
                    _ => egui::ViewportCommand::Maximized(!maximised),
                };
                ui.send_viewport_cmd(cmd);
            }
        }
        ui.painter().add(p.finish());
        for (glyph, rect, ink) in icons {
            self.icon(ui, glyph.into(), rect, t.chrome.icon, ink);
        }
        if left { x0 + 3.0 * w } else { x0 }
    }
}

/// The app menu for `brand`.
pub(crate) fn app_menu_of<'b>(brand: &'b Brand<'b>) -> AppMenu<'b> {
    AppMenu::new(brand.info)
        .title(brand.name)
        .description(brand.description)
}

/// The palette action's keys, from the action registry; empty when it has none.
pub(crate) fn palette_keys(ctx: &egui::Context, actions: Option<&Actions>) -> Vec<String> {
    actions
        .and_then(|a| a.get(PALETTE_ACTION))
        .and_then(|s| s.keys.first().copied())
        .map(|k| chord_keys(ctx, k))
        .unwrap_or_default()
}

/// The field's and search button's tooltip, naming the palette's keys.
fn palette_tip(keys: &[&str]) -> String {
    if keys.is_empty() {
        "Search, or run a command by its name".to_owned()
    } else {
        format!("Search, or run a command by its name ({})", keys.join(" "))
    }
}

/// The document as an accessibility label.
fn document_label(name: &str, dirty: bool) -> String {
    if dirty {
        format!("{name}, unsaved")
    } else {
        name.to_owned()
    }
}

/// The document's whole name, folder and state, for its tooltip.
fn document_whole(name: &str, path: &str, dirty: bool) -> String {
    let mut s = name.to_owned();
    if !path.is_empty() {
        s.push('\n');
        s.push_str(path);
    }
    if dirty {
        s.push_str("\nUnsaved changes");
    }
    s
}
