//! The pie menu on the device's buttons (SPM-3).
//!
//! - Tap a button (release within [`PieStyle::tap_s`]) to fire its centre slot; the menu never shows.
//! - Hold it and a ring of slots opens at the pointer. Moving out across the outer circle fires the slot under
//!   the pointer at once, and so does a click.
//! - The menu stays open while the button is held, so actions chain: back through the centre (which re-arms it)
//!   for the next one. Releasing closes it without firing anything.
//! - Each button has its own ring; pressing the other one too switches to the third ring, of both.
//! - Every slot shows its icon and name. A free slot says "Free"; a disabled one is drawn muted and, under the
//!   pointer, says why below the circle.
//!
//! The state machine ([`PieState`]) is plain data driven by the caller's time and pointer, so it is tested
//! without a window; [`PieMenu::show`] feeds it from egui and draws.

use std::f32::consts::{PI, TAU};

use egui::epaint::{Mesh, PathShape, PathStroke};
use egui::text::{LayoutJob, TextFormat};
use egui::{
    Color32, FontId, Galley, Id, Order, Painter, Pos2, Rect, Sense, Shape, Vec2, pos2, vec2,
};
use std::sync::Arc;
use ve_theme::Tokens;

use crate::event::Event;
use crate::mapping::{Mapping, Side};

/// Which ring is open: one button's, or both together.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Ring {
    /// The left button's ring.
    Left,
    /// The right button's ring.
    Right,
    /// Both buttons held.
    Both,
}

impl From<Side> for Ring {
    fn from(s: Side) -> Self {
        match s {
            Side::Left => Ring::Left,
            Side::Right => Ring::Right,
        }
    }
}

/// A slot of the menu: a centre half or a slice of a ring.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SlotRef {
    /// The left or right half of the centre.
    Center(Side),
    /// Slice `i` of the open ring, clockwise from the top.
    Ring(usize),
}

/// One action in the menu.
#[derive(Clone, Debug, PartialEq)]
pub struct Slot<A> {
    /// Its name, always shown.
    pub label: String,
    /// The icon: a glyph, e.g. one of [`crate::icons`]; empty for none.
    pub icon: String,
    /// What the app gets back when it fires.
    pub action: A,
    /// Why it can't fire now; `None` when it can.
    pub disabled: Option<String>,
}

impl<A> Slot<A> {
    /// An enabled slot.
    pub fn new(label: impl Into<String>, icon: impl Into<String>, action: A) -> Self {
        Self {
            label: label.into(),
            icon: icon.into(),
            action,
            disabled: None,
        }
    }

    /// Disable it, saying why and what would enable it (shown under the circle while the pointer is on it).
    pub fn disabled(mut self, why: impl Into<String>) -> Self {
        self.disabled = Some(why.into());
        self
    }

    /// Disable it with `why` when `when` holds.
    pub fn disabled_if(self, when: bool, why: impl Into<String>) -> Self {
        if when { self.disabled(why) } else { self }
    }

    fn enabled(&self) -> bool {
        self.disabled.is_none()
    }
}

/// Sizes and timings of the menu. The defaults are the reference menu's.
#[derive(Clone, Debug, PartialEq)]
pub struct PieStyle {
    /// Radius of the ring; crossing it fires the slice under the pointer.
    pub outer_radius: f32,
    /// Radius of the centre.
    pub center_radius: f32,
    /// A release sooner than this (seconds) is a tap: it fires the centre slot, and the menu never shows.
    pub tap_s: f64,
    /// Width a slice's name wraps at.
    pub label_width: f32,
    /// Icon size in a slice, and in a centre half.
    pub icon_size: f32,
    /// Icon size in a centre half.
    pub center_icon_size: f32,
    /// Name size in a slice.
    pub label_size: f32,
    /// Name size in a centre half.
    pub center_label_size: f32,
    /// How long a fired slice flies off, then how long it takes to come back (seconds).
    pub fly_s: (f64, f64),
    /// How far a fired slice flies off.
    pub fly_distance: f32,
    /// Opacity of the slices' fill.
    pub fill_alpha: f32,
}

impl Default for PieStyle {
    fn default() -> Self {
        Self {
            outer_radius: 180.0,
            center_radius: 64.0,
            tap_s: 0.25,
            label_width: 96.0,
            icon_size: 26.0,
            center_icon_size: 20.0,
            label_size: 14.0,
            center_label_size: 12.0,
            fly_s: (0.26, 0.2),
            fly_distance: 50.0,
            fill_alpha: 0.94,
        }
    }
}

/// The menu: two centre halves and three rings (`None` is a free slot). Rings may have any number of slices;
/// the reference has 8.
#[derive(Clone, Debug)]
pub struct PieMenu<A> {
    /// The left centre half: what a tap of the left button fires.
    pub center_left: Option<Slot<A>>,
    /// The right centre half: what a tap of the right button fires.
    pub center_right: Option<Slot<A>>,
    /// The left button's ring.
    pub ring_left: Vec<Option<Slot<A>>>,
    /// The right button's ring.
    pub ring_right: Vec<Option<Slot<A>>>,
    /// The ring of both buttons.
    pub ring_both: Vec<Option<Slot<A>>>,
    /// What a free slot says under the circle, e.g. where to assign it.
    pub free_hint: String,
    /// Sizes and timings.
    pub style: PieStyle,
}

impl<A> Default for PieMenu<A> {
    fn default() -> Self {
        Self {
            center_left: None,
            center_right: None,
            ring_left: Vec::new(),
            ring_right: Vec::new(),
            ring_both: Vec::new(),
            free_hint: "Nothing assigned to this slot yet".into(),
            style: PieStyle::default(),
        }
    }
}

/// A button of the device went down or up, by side.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ButtonEvent {
    /// Which button.
    pub side: Side,
    /// Down or up.
    pub pressed: bool,
}

/// The button events among spacenavd's events, by side (buttons that are neither side are left out).
pub fn buttons(events: &[Event], mapping: &Mapping) -> Vec<ButtonEvent> {
    events
        .iter()
        .filter_map(|e| match *e {
            Event::Button { number, pressed } => mapping
                .side_of(number)
                .map(|side| ButtonEvent { side, pressed }),
            _ => None,
        })
        .collect()
}

/// A slot that fired.
#[derive(Clone, Debug, PartialEq)]
pub struct Fired<A> {
    /// The slot's action.
    pub action: A,
    /// Where the menu opened: where the pointer was on the press, the place the action is about (the
    /// reference menu puts the cursor back there before sending the key).
    pub at: Pos2,
    /// Which slot.
    pub slot: SlotRef,
    /// Which ring was open.
    pub ring: Ring,
    /// Fired by a tap (the menu never showed).
    pub tap: bool,
}

#[derive(Clone, Debug)]
struct Open {
    origin: Pos2,
    ring: Ring,
    opened_at: f64,
    hot: Option<usize>,
    center_side: Option<Side>,
    armed: bool,
    /// Ring slices flying off after they fired: index and start time.
    flights: Vec<(usize, f64)>,
}

/// The menu's state: which buttons are held, whether it is open, where, and what is under the pointer.
/// Transient: keep it beside the menu, never in saved settings.
#[derive(Clone, Debug, Default)]
pub struct PieState {
    held: Vec<Side>,
    open: Option<Open>,
}

impl<A: Clone> PieMenu<A> {
    /// The ring's slices.
    pub fn ring(&self, ring: Ring) -> &[Option<Slot<A>>] {
        match ring {
            Ring::Left => &self.ring_left,
            Ring::Right => &self.ring_right,
            Ring::Both => &self.ring_both,
        }
    }

    /// A centre half.
    pub fn center(&self, side: Side) -> Option<&Slot<A>> {
        match side {
            Side::Left => self.center_left.as_ref(),
            Side::Right => self.center_right.as_ref(),
        }
    }

    /// The slot behind a reference, in the open ring.
    pub fn slot(&self, ring: Ring, slot: SlotRef) -> Option<&Slot<A>> {
        match slot {
            SlotRef::Center(s) => self.center(s),
            SlotRef::Ring(i) => self.ring(ring).get(i).and_then(|s| s.as_ref()),
        }
    }

    /// Feed the device's buttons and egui's pointer to `state`, draw the menu while it is open, and return
    /// what fired this frame. Call it once per frame, after the app's panels (it draws on top of them).
    ///
    /// A press while the window has no focus or the pointer is outside it is ignored, as the reference ignores
    /// presses outside the editor.
    pub fn show(
        &self,
        ctx: &egui::Context,
        state: &mut PieState,
        buttons: &[ButtonEvent],
    ) -> Vec<Fired<A>> {
        let (now, pointer, focused, clicked) = ctx.input(|i| {
            (
                i.time,
                i.pointer.latest_pos(),
                i.focused,
                i.pointer.any_pressed(),
            )
        });
        let mut fired = Vec::new();
        for b in buttons {
            if b.pressed {
                state.press(b.side, pointer.filter(|_| focused), now);
            } else {
                fired.extend(state.release(b.side, now, self));
            }
        }
        if state.is_open() {
            if let Some(p) = pointer {
                fired.extend(state.pointer(p, now, self));
            }
            if clicked {
                fired.extend(state.click(now, self));
            }
        }
        if let Some(open) = &mut state.open {
            let (fly, back) = self.style.fly_s;
            open.flights.retain(|(_, t)| now - t < fly + back);
            let tokens = Tokens::of(ctx, ctx.theme());
            egui::Area::new(Id::new("ve_spacemouse_pie"))
                .order(Order::Foreground)
                .fixed_pos(Pos2::ZERO)
                .show(ctx, |ui| {
                    // The whole window: clicks go to the menu, not to what is under it.
                    ui.allocate_rect(ctx.content_rect(), Sense::click_and_drag());
                    if now - open.opened_at >= self.style.tap_s {
                        let marks = self.paint(ui.painter(), open, now, &tokens);
                        // Every name and the hint as accessible labels, for screen readers and kittests.
                        for (k, (rect, text, enabled)) in marks.into_iter().enumerate() {
                            let r = ui.interact(
                                rect,
                                Id::new("ve_spacemouse_pie_label").with(k),
                                Sense::hover(),
                            );
                            r.widget_info(|| {
                                egui::WidgetInfo::labeled(egui::WidgetType::Label, enabled, &text)
                            });
                        }
                    }
                });
            ctx.request_repaint();
        }
        fired
    }

    /// Draw the open menu; the rectangle, text and enabled state of every name and of the hint.
    fn paint(
        &self,
        painter: &Painter,
        open: &Open,
        now: f64,
        t: &Tokens,
    ) -> Vec<(Rect, String, bool)> {
        let mut marks = Vec::new();
        let s = &self.style;
        let o = open.origin;
        let fill = |c: Color32| c.gamma_multiply(s.fill_alpha);
        let (accent, bg, fg, muted) = (
            t.colors.accent,
            t.colors.surface,
            t.colors.text,
            t.colors.text_muted,
        );
        let on_accent = t.on(accent);
        let ring = self.ring(open.ring);
        let n = ring.len();
        if n > 0 {
            let step = TAU / n as f32;
            let label_r = (s.outer_radius + s.center_radius) / 2.0;
            for (i, slot) in ring.iter().enumerate() {
                let middle = -PI / 2.0 + i as f32 * step;
                let dir = vec2(middle.cos(), middle.sin());
                let (kick, back) = open
                    .flights
                    .iter()
                    .find(|(k, _)| *k == i)
                    .map(|(_, start)| flight(now - start, s.fly_s))
                    .unwrap_or((0.0, 1.0));
                let alpha = (1.0 - kick) * back;
                let c = o + dir * s.fly_distance * kick;
                let lit = open.hot == Some(i) && slot.is_some();
                let enabled = slot.as_ref().is_some_and(|s| s.enabled());
                let color = if lit && enabled {
                    accent
                } else if lit {
                    t.colors.line_strong
                } else {
                    bg
                };
                let gap_out = 0.6f32.to_radians();
                let gap_in = 1.5f32.to_radians();
                sector(
                    painter,
                    c,
                    (
                        s.center_radius + 6.0,
                        middle - step / 2.0 + gap_in,
                        middle + step / 2.0 - gap_in,
                    ),
                    (
                        s.outer_radius,
                        middle - step / 2.0 + gap_out,
                        middle + step / 2.0 - gap_out,
                    ),
                    fill(color).gamma_multiply(alpha),
                );
                let (icon_c, text_c) = match slot {
                    Some(sl) if !sl.enabled() => (muted, muted),
                    Some(_) if lit => (on_accent, on_accent),
                    Some(_) => (accent, fg),
                    None => (muted, muted),
                };
                let rect = label(
                    painter,
                    c + dir * label_r,
                    slot.as_ref(),
                    (s.icon_size, s.label_size, s.label_width),
                    (icon_c.gamma_multiply(alpha), text_c.gamma_multiply(alpha)),
                );
                marks.push((
                    rect,
                    slot.as_ref().map_or("Free".into(), |s| s.label.clone()),
                    enabled,
                ));
            }
        }
        for side in [Side::Left, Side::Right] {
            let item = self.center(side);
            let d = if side == Side::Left { -1.0 } else { 1.0 };
            let lit = open.hot.is_none() && open.center_side == Some(side);
            let enabled = item.is_some_and(|i| i.enabled());
            let color = match (lit, item.is_some(), enabled) {
                (true, true, true) => accent,
                (true, true, false) => t.colors.line_strong,
                _ => bg,
            };
            let (a0, a1) = if side == Side::Left {
                (PI / 2.0, 1.5 * PI)
            } else {
                (-PI / 2.0, PI / 2.0)
            };
            sector(
                painter,
                o + vec2(d * 1.5, 0.0),
                (0.0, a0, a1),
                (s.center_radius, a0, a1),
                fill(color),
            );
            let (icon_c, text_c) = match item {
                Some(i) if !i.enabled() => (muted, muted),
                Some(_) if lit => (on_accent, on_accent),
                Some(_) => (accent, fg),
                None => (muted, muted),
            };
            let rect = label(
                painter,
                o + vec2(d * s.center_radius / 2.0, 0.0),
                item,
                (
                    s.center_icon_size,
                    s.center_label_size,
                    s.center_radius - 8.0,
                ),
                (icon_c, text_c),
            );
            marks.push((
                rect,
                item.map_or("Free".into(), |s| s.label.clone()),
                enabled,
            ));
        }
        // What the slot under the pointer can't do, and why.
        let hint = match open.hot {
            Some(i) => match ring.get(i) {
                Some(Some(sl)) => sl.disabled.clone(),
                _ => Some(self.free_hint.clone()),
            },
            None => open.center_side.and_then(|side| match self.center(side) {
                Some(sl) => sl.disabled.clone(),
                None => Some(self.free_hint.clone()),
            }),
        };
        if let Some(hint) = hint {
            let g = painter.layout(
                hint.clone(),
                FontId::proportional(12.0),
                fg,
                2.0 * s.outer_radius + 120.0,
            );
            let size = g.size() + vec2(20.0, 10.0);
            let rect =
                Rect::from_min_size(pos2(o.x - size.x / 2.0, o.y + s.outer_radius + 10.0), size);
            painter.rect_filled(rect, 6.0, fill(bg));
            painter.galley(rect.min + vec2(10.0, 5.0), g, fg);
            marks.push((rect, hint, true));
        }
        marks
    }
}

impl PieState {
    /// Whether the menu is open (possibly not shown yet: a press counts as a tap until [`PieStyle::tap_s`]).
    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    /// Where the menu is, while open.
    pub fn origin(&self) -> Option<Pos2> {
        self.open.as_ref().map(|o| o.origin)
    }

    /// The open ring.
    pub fn ring(&self) -> Option<Ring> {
        self.open.as_ref().map(|o| o.ring)
    }

    /// The ring slice under the pointer, `None` in the centre or when closed.
    pub fn hot(&self) -> Option<usize> {
        self.open.as_ref().and_then(|o| o.hot)
    }

    /// The centre half under the pointer (the pressed button's own until the pointer moves; none for the ring
    /// of both).
    pub fn center_side(&self) -> Option<Side> {
        self.open.as_ref().and_then(|o| o.center_side)
    }

    /// A button went down at `now`, the pointer at `pointer` (`None`: not over the window, the press is ignored).
    pub fn press(&mut self, side: Side, pointer: Option<Pos2>, now: f64) {
        if self.held.contains(&side) {
            return;
        }
        match (&mut self.open, pointer) {
            (None, Some(origin)) => {
                self.held.push(side);
                self.open = Some(Open {
                    origin,
                    ring: side.into(),
                    opened_at: now,
                    hot: None,
                    center_side: Some(side),
                    armed: true,
                    flights: Vec::new(),
                });
            }
            (None, None) => log::debug!(
                "ve_spacemouse: {} button pressed outside the window, ignored",
                side.name()
            ),
            (Some(open), _) => {
                // A second button while the first is held switches to the ring of both, in the same place.
                self.held.push(side);
                open.ring = Ring::Both;
                open.opened_at = now;
                open.hot = None;
                open.center_side = None;
                open.armed = true;
            }
        }
    }

    /// A button went up at `now`. Only a tap (released before the menu showed, pointer still in the centre)
    /// fires: the centre slot of that button. Once the menu shows, a release closes it.
    pub fn release<A: Clone>(
        &mut self,
        side: Side,
        now: f64,
        menu: &PieMenu<A>,
    ) -> Option<Fired<A>> {
        let at = self.held.iter().position(|s| *s == side)?;
        self.held.remove(at);
        if !self.held.is_empty() {
            return None;
        }
        let open = self.open.take()?;
        let tap = now - open.opened_at < menu.style.tap_s && open.hot.is_none();
        if open.ring == Ring::Both {
            return None;
        }
        let slot = SlotRef::Center(side);
        match menu.center(side) {
            Some(item) if tap && item.enabled() => Some(Fired {
                action: item.action.clone(),
                at: open.origin,
                slot,
                ring: open.ring,
                tap: true,
            }),
            _ => None,
        }
    }

    /// The pointer is at `pos`: light the slot under it, and fire the slice if it just crossed the outer circle.
    pub fn pointer<A: Clone>(
        &mut self,
        pos: Pos2,
        now: f64,
        menu: &PieMenu<A>,
    ) -> Option<Fired<A>> {
        let s = &menu.style;
        let open = self.open.as_mut()?;
        let d: Vec2 = pos - open.origin;
        let distance = d.length();
        let n = menu.ring(open.ring).len();
        if distance <= s.center_radius || n == 0 {
            open.hot = None;
            open.armed = true;
            if d.x != 0.0 && open.ring != Ring::Both {
                open.center_side = Some(if d.x < 0.0 { Side::Left } else { Side::Right });
            }
            return None;
        }
        // Clockwise from the top, with slice 0 centred on the top.
        let step = TAU / n as f32;
        let angle = d.x.atan2(-d.y) + TAU + step / 2.0;
        let hot = (angle / step).floor() as usize % n;
        open.hot = Some(hot);
        if open.armed && distance >= s.outer_radius {
            return self.fire(SlotRef::Ring(hot), now, menu);
        }
        None
    }

    /// A click while open fires the slot under the pointer; on a free or disabled slot it closes the menu,
    /// which also frees the window if a button release ever got lost.
    pub fn click<A: Clone>(&mut self, now: f64, menu: &PieMenu<A>) -> Option<Fired<A>> {
        let open = self.open.as_ref()?;
        let slot = match open.hot {
            Some(i) => SlotRef::Ring(i),
            None => SlotRef::Center(open.center_side?),
        };
        let fired = self.fire(slot, now, menu);
        if fired.is_none() {
            self.open = None;
            self.held.clear();
        }
        fired
    }

    fn fire<A: Clone>(&mut self, slot: SlotRef, now: f64, menu: &PieMenu<A>) -> Option<Fired<A>> {
        let open = self.open.as_mut()?;
        let item = menu.slot(open.ring, slot).filter(|i| i.enabled())?;
        // The next one needs the pointer back in the centre first.
        open.armed = false;
        if let SlotRef::Ring(i) = slot {
            open.flights.retain(|(k, _)| *k != i);
            open.flights.push((i, now));
        }
        Some(Fired {
            action: item.action.clone(),
            at: open.origin,
            slot,
            ring: open.ring,
            tap: false,
        })
    }
}

/// A fired slice's flight at `t` seconds: how far out (0 to 1) and how far back in (0 to 1).
fn flight(t: f64, (fly, back): (f64, f64)) -> (f32, f32) {
    if t < fly {
        let x = (t / fly) as f32;
        (1.0 - (1.0 - x).powi(3), 1.0)
    } else {
        let x = ((t - fly) / back).min(1.0) as f32;
        (0.0, x * x)
    }
}

/// An annular sector around `c`: inner (radius, from, to) and outer (radius, from, to), angles in radians
/// clockwise from +x (egui's y points down). An inner radius of 0 makes a pie slice.
fn sector(
    painter: &Painter,
    c: Pos2,
    inner: (f32, f32, f32),
    outer: (f32, f32, f32),
    color: Color32,
) {
    if color.a() == 0 {
        return;
    }
    let segments = (((outer.2 - outer.1).abs() / 3f32.to_radians()).ceil() as usize).max(2);
    let at = |r: f32, a: f32| c + vec2(a.cos(), a.sin()) * r;
    let mut mesh = Mesh::default();
    let mut outline = Vec::with_capacity(2 * segments + 2);
    for k in 0..=segments {
        let f = k as f32 / segments as f32;
        let po = at(outer.0, outer.1 + (outer.2 - outer.1) * f);
        let pi = at(inner.0, inner.1 + (inner.2 - inner.1) * f);
        mesh.colored_vertex(po, color);
        mesh.colored_vertex(pi, color);
        outline.push(po);
        if k > 0 {
            let b = (2 * k) as u32;
            mesh.add_triangle(b - 2, b - 1, b);
            mesh.add_triangle(b - 1, b + 1, b);
        }
    }
    for k in (0..=segments).rev() {
        let f = k as f32 / segments as f32;
        outline.push(at(inner.0, inner.1 + (inner.2 - inner.1) * f));
    }
    painter.add(Shape::mesh(mesh));
    // A hairline in the same colour smooths the edges (the mesh itself is not anti-aliased).
    painter.add(PathShape::closed_line(outline, PathStroke::new(1.0, color)));
}

/// A slot's icon over its name, centred on `c`. A free slot says "Free" in italics.
fn label<A>(
    painter: &Painter,
    c: Pos2,
    slot: Option<&Slot<A>>,
    (icon_size, text_size, width): (f32, f32, f32),
    (icon_color, text_color): (Color32, Color32),
) -> Rect {
    let icon: Option<Arc<Galley>> = slot.filter(|s| !s.icon.is_empty()).map(|s| {
        painter.layout_no_wrap(s.icon.clone(), FontId::proportional(icon_size), icon_color)
    });
    let mut job = LayoutJob::default();
    let (text, italics) = match slot {
        Some(s) => (s.label.as_str(), false),
        None => ("Free", true),
    };
    job.append(
        text,
        0.0,
        TextFormat {
            font_id: FontId::proportional(text_size),
            color: text_color,
            italics,
            ..Default::default()
        },
    );
    job.wrap.max_width = width;
    job.halign = egui::Align::Center;
    let text = painter.fonts_mut(|f| f.layout_job(job));
    let gap = 2.0;
    let h = icon.as_ref().map_or(0.0, |g| g.size().y + gap) + text.size().y;
    let top = c.y - h / 2.0;
    let w = icon.as_ref().map_or(0.0, |g| g.size().x).max(text.size().x);
    let mut y = top;
    if let Some(g) = icon {
        let w = g.size().x;
        let hh = g.size().y;
        painter.galley(pos2(c.x - w / 2.0, y), g, icon_color);
        y += hh + gap;
    }
    // A centred job lays out around x = 0.
    painter.galley(pos2(c.x, y), text, text_color);
    Rect::from_center_size(pos2(c.x, top + h / 2.0), vec2(w, h))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, PartialEq)]
    enum A {
        Highlight,
        Route,
        Copy,
        Save,
        Paste,
    }

    fn menu() -> PieMenu<A> {
        let mut ring_right = vec![None; 8];
        ring_right[0] = Some(Slot::new("Copy", "", A::Copy));
        ring_right[4] = Some(Slot::new("Paste", "", A::Paste).disabled("Nothing copied yet"));
        let mut ring_both = vec![None; 8];
        ring_both[0] = Some(Slot::new("Save", "", A::Save));
        PieMenu {
            center_left: Some(Slot::new("Highlight net", "", A::Highlight)),
            center_right: Some(Slot::new("Route track", "", A::Route)),
            ring_right,
            ring_both,
            ..Default::default()
        }
    }

    const O: Pos2 = pos2(400.0, 300.0);

    #[test]
    fn a_tap_fires_the_centre_of_that_button() {
        let m = menu();
        let mut s = PieState::default();
        s.press(Side::Right, Some(O), 1.0);
        let f = s.release(Side::Right, 1.1, &m).expect("a tap fires");
        assert_eq!((f.action, f.at, f.tap), (A::Route, O, true));
        assert!(!s.is_open());
    }

    #[test]
    fn a_hold_then_release_cancels() {
        let m = menu();
        let mut s = PieState::default();
        s.press(Side::Left, Some(O), 1.0);
        assert_eq!(s.release(Side::Left, 1.5, &m), None);
        assert!(!s.is_open());
    }

    #[test]
    fn crossing_the_outer_circle_fires_then_rearms_in_the_centre() {
        let m = menu();
        let mut s = PieState::default();
        s.press(Side::Right, Some(O), 1.0);
        // up: slice 0
        assert_eq!(s.pointer(O + vec2(0.0, -100.0), 1.3, &m), None);
        assert_eq!(s.hot(), Some(0));
        let f = s
            .pointer(O + vec2(0.0, -190.0), 1.4, &m)
            .expect("fires on crossing");
        assert_eq!(
            (f.action, f.slot, f.tap),
            (A::Copy, SlotRef::Ring(0), false)
        );
        // not again until back through the centre
        assert_eq!(s.pointer(O + vec2(0.0, -200.0), 1.5, &m), None);
        s.pointer(O, 1.6, &m);
        assert!(s.pointer(O + vec2(5.0, -190.0), 1.7, &m).is_some());
        assert!(s.is_open(), "stays open while held");
    }

    #[test]
    fn slices_go_clockwise_from_the_top() {
        let m = menu();
        let mut s = PieState::default();
        s.press(Side::Right, Some(O), 0.0);
        for (d, i) in [
            (vec2(0.0, -100.0), 0),
            (vec2(100.0, 0.0), 2),
            (vec2(0.0, 100.0), 4),
            (vec2(-100.0, 0.0), 6),
        ] {
            s.pointer(O + d, 0.5, &m);
            assert_eq!(s.hot(), Some(i), "{d:?}");
        }
    }

    #[test]
    fn disabled_and_free_slots_do_not_fire_and_a_click_on_them_closes() {
        let m = menu();
        let mut s = PieState::default();
        s.press(Side::Right, Some(O), 0.0);
        assert_eq!(
            s.pointer(O + vec2(0.0, 200.0), 0.5, &m),
            None,
            "paste is disabled"
        );
        assert_eq!(s.hot(), Some(4));
        s.pointer(O + vec2(150.0, 0.0), 0.6, &m);
        assert_eq!(s.click(0.7, &m), None, "a free slot");
        assert!(!s.is_open());
    }

    #[test]
    fn a_click_fires_the_slot_under_the_pointer() {
        let m = menu();
        let mut s = PieState::default();
        s.press(Side::Right, Some(O), 0.0);
        s.pointer(O + vec2(-20.0, 0.0), 0.5, &m);
        assert_eq!(s.center_side(), Some(Side::Left));
        assert_eq!(s.click(0.6, &m).map(|f| f.action), Some(A::Highlight));
        assert!(s.is_open());
    }

    #[test]
    fn both_buttons_open_the_third_ring() {
        let m = menu();
        let mut s = PieState::default();
        s.press(Side::Left, Some(O), 0.0);
        s.press(Side::Right, Some(O + vec2(30.0, 0.0)), 0.5);
        assert_eq!((s.ring(), s.origin()), (Some(Ring::Both), Some(O)));
        assert_eq!(
            s.pointer(O + vec2(0.0, -190.0), 0.7, &m).map(|f| f.action),
            Some(A::Save)
        );
        assert_eq!(s.release(Side::Left, 0.8, &m), None);
        assert!(s.is_open(), "one still held");
        assert_eq!(s.release(Side::Right, 0.9, &m), None);
        assert!(!s.is_open());
    }

    #[test]
    fn a_press_outside_the_window_is_ignored() {
        let m = menu();
        let mut s = PieState::default();
        s.press(Side::Left, None, 0.0);
        assert!(!s.is_open());
        assert_eq!(s.release(Side::Left, 0.1, &m), None);
    }

    #[test]
    fn button_events_by_side() {
        let mapping = Mapping::default();
        let ev = [
            Event::button(1, true),
            Event::motion([1; 6]),
            Event::button(5, true),
            Event::button(0, false),
        ];
        assert_eq!(
            buttons(&ev, &mapping),
            vec![
                ButtonEvent {
                    side: Side::Right,
                    pressed: true
                },
                ButtonEvent {
                    side: Side::Left,
                    pressed: false
                }
            ]
        );
    }
}
