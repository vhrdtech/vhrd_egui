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

use egui::epaint::FontsView;
use egui::epaint::{Mesh, PathShape, PathStroke};
use egui::text::{LayoutJob, TextFormat};
use egui::{
    Color32, FontId, Galley, Id, Order, Painter, Pos2, Rect, Sense, Shape, Vec2, pos2, vec2,
};
use std::hash::{Hash, Hasher};
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
    /// How many lines a name may wrap to; a name that needs more grows the ring instead.
    pub max_lines: usize,
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
            outer_radius: 150.0,
            center_radius: 52.0,
            tap_s: 0.25,
            label_width: 96.0,
            max_lines: 2,
            icon_size: 20.0,
            center_icon_size: 16.0,
            label_size: 12.0,
            center_label_size: 11.0,
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
    /// The centre and outer radii the menu is drawn and hit-tested with, once [`PieMenu::show`] fitted the
    /// names (larger than the style's when a name needs the room).
    radii: Option<(f32, f32)>,
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
        let fit = state.ring().map(|ring| self.fit(ctx, ring));
        if let (Some(open), Some(fit)) = (&mut state.open, &fit) {
            open.radii = Some((fit.center_radius, fit.outer_radius));
        }
        if state.is_open() {
            if let Some(p) = pointer {
                fired.extend(state.pointer(p, now, self));
            }
            if clicked {
                fired.extend(state.click(now, self));
            }
        }
        if let (Some(open), Some(fit)) = (&mut state.open, &fit) {
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
                        let marks = self.paint(ui.painter(), open, now, &tokens, fit);
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

    /// Draw the open menu; the rectangle, text and enabled state of every name and of the hint. The fills go
    /// first and the names over all of them, so no slice covers a neighbour's name.
    fn paint(
        &self,
        painter: &Painter,
        open: &Open,
        now: f64,
        t: &Tokens,
        fit: &Fit,
    ) -> Vec<(Rect, String, bool)> {
        let mut marks = Vec::new();
        let s = &self.style;
        let (center_radius, outer_radius) = (fit.center_radius, fit.outer_radius);
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
        let colors = |slot: Option<&Slot<A>>, lit: bool| match slot {
            Some(sl) if !sl.enabled() => (muted, muted),
            Some(_) if lit => (on_accent, on_accent),
            Some(_) => (accent, fg),
            None => (muted, muted),
        };
        let mut names = Vec::new();
        if n > 0 {
            let step = TAU / n as f32;
            let label_r = (outer_radius + center_radius + 6.0) / 2.0;
            for (i, slot) in ring.iter().enumerate() {
                let middle = slice_middle(i, n);
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
                        center_radius + 6.0,
                        middle - step / 2.0 + gap_in,
                        middle + step / 2.0 - gap_in,
                    ),
                    (
                        outer_radius,
                        middle - step / 2.0 + gap_out,
                        middle + step / 2.0 - gap_out,
                    ),
                    fill(color).gamma_multiply(alpha),
                );
                let (icon_c, text_c) = colors(slot.as_ref(), lit);
                let (size, width, r) =
                    fit.ring
                        .get(i)
                        .copied()
                        .unwrap_or((s.label_size, s.label_width, label_r));
                names.push((
                    c + dir * r,
                    slot.as_ref(),
                    (s.icon_size, size, width),
                    (icon_c.gamma_multiply(alpha), text_c.gamma_multiply(alpha)),
                    enabled,
                ));
            }
        }
        for (k, side) in [Side::Left, Side::Right].into_iter().enumerate() {
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
                (center_radius, a0, a1),
                fill(color),
            );
            let (size, width) = fit.center[k];
            names.push((
                o + vec2(d * center_radius / 2.0, 0.0),
                item,
                (s.center_icon_size, size, width),
                colors(item, lit),
                enabled,
            ));
        }
        for (c, slot, sizes, colors, enabled) in names {
            let rect = label(painter, c, slot, sizes, colors);
            marks.push((
                rect,
                slot.map_or("Free".into(), |s| s.label.clone()),
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
                2.0 * outer_radius + 120.0,
            );
            let size = g.size() + vec2(20.0, 10.0);
            let rect =
                Rect::from_min_size(pos2(o.x - size.x / 2.0, o.y + outer_radius + 10.0), size);
            painter.rect_filled(rect, 6.0, fill(bg));
            painter.galley(rect.min + vec2(10.0, 5.0), g, fg);
            marks.push((rect, hint, true));
        }
        marks
    }

    /// Where the names of `ring` and the centre fit, cached in egui's memory until a name or the style changes.
    fn fit(&self, ctx: &egui::Context, ring: Ring) -> Fit {
        let mut h = std::hash::DefaultHasher::new();
        let mut name = |slot: Option<&Slot<A>>| {
            slot.map(|s| (&s.label, &s.icon)).hash(&mut h);
        };
        name(self.center_left.as_ref());
        name(self.center_right.as_ref());
        for slot in self.ring(ring) {
            name(slot.as_ref());
        }
        let s = &self.style;
        [
            s.outer_radius,
            s.center_radius,
            s.label_width,
            s.icon_size,
            s.center_icon_size,
            s.label_size,
            s.center_label_size,
        ]
        .map(f32::to_bits)
        .hash(&mut h);
        s.max_lines.hash(&mut h);
        let key = Id::new("ve_spacemouse_pie_fit").with(h.finish());
        if let Some(fit) = ctx.data(|d| d.get_temp::<Fit>(key)) {
            return fit;
        }
        let fit = ctx.fonts_mut(|fonts| self.fit_with(fonts, ring));
        ctx.data_mut(|d| d.insert_temp(key, fit.clone()));
        fit
    }

    /// Fit every name in its slot: the largest text size (down to [`MIN_SIZE`]) at which every name of the ring
    /// fits in [`PieStyle::max_lines`] rows (the centre's halves, narrow and tall, one more), with a wrap width
    /// that breaks no word; a slice's name may move along it to where it fits. Where even the smallest size does
    /// not fit, the centre and then the ring grow (up to twice the style's), so every name shows in full.
    fn fit_with(&self, fonts: &mut FontsView<'_>, ring: Ring) -> Fit {
        let s = &self.style;
        let mut center_radius = s.center_radius;
        let center = loop {
            let fits = sizes(s.center_label_size).find_map(|size| {
                let [l, r] = [Side::Left, Side::Right].map(|side| {
                    let d = if side == Side::Left { -1.0 } else { 1.0 };
                    fit_width(
                        fonts,
                        self.center(side),
                        (
                            s.center_icon_size,
                            size,
                            center_radius - 8.0,
                            s.max_lines + 1,
                        ),
                        pos2(d * center_radius / 2.0, 0.0),
                        |rects| inside_half(rects, center_radius, d),
                    )
                });
                Some([(size, l?), (size, r?)])
            });
            if let Some(fits) = fits {
                break fits;
            }
            if center_radius >= s.center_radius * MAX_GROWTH {
                let last = sizes(s.center_label_size)
                    .last()
                    .unwrap_or(s.center_label_size);
                break [(last, center_radius - 8.0); 2];
            }
            center_radius += 4.0;
        };
        let slots = self.ring(ring);
        let n = slots.len();
        let ring_width = s.outer_radius - s.center_radius;
        let mut outer_radius = center_radius + ring_width;
        let ring = loop {
            let inner = center_radius + 6.0;
            let middle_r = (outer_radius + inner) / 2.0;
            let chord = 2.0 * outer_radius * (PI / n.max(1) as f32).min(PI / 2.0).sin();
            let max_width = s.label_width.max(chord);
            // Where a name may sit along its slice: the middle first, then out (where the slice is wider) and in.
            let band = (outer_radius - inner) / 2.0;
            let radii = [0.0, 0.2, -0.2, 0.4, -0.4].map(|k| middle_r + k * band);
            let fits = sizes(s.label_size).find_map(|size| {
                slots
                    .iter()
                    .enumerate()
                    .map(|(i, slot)| {
                        let middle = slice_middle(i, n);
                        radii.iter().find_map(|&r| {
                            let width = fit_width(
                                fonts,
                                slot.as_ref(),
                                (s.icon_size, size, max_width, s.max_lines),
                                pos2(middle.cos(), middle.sin()) * r,
                                |rects| inside_slice(rects, (inner, outer_radius), middle, n),
                            )?;
                            Some((size, width, r))
                        })
                    })
                    .collect::<Option<Vec<_>>>()
            });
            if let Some(fits) = fits {
                break fits;
            }
            if outer_radius - center_radius >= ring_width * MAX_GROWTH {
                let last = sizes(s.label_size).last().unwrap_or(s.label_size);
                break vec![(last, max_width, middle_r); n];
            }
            outer_radius += 4.0;
        };
        Fit {
            center_radius,
            outer_radius,
            ring,
            center,
        }
    }
}

/// Where the names fit: the radii the menu is drawn and hit-tested with, each slice's name's text size, wrap
/// width and distance from the centre, and the centre's left and right halves' text size and wrap width.
#[derive(Clone, Debug, PartialEq)]
struct Fit {
    center_radius: f32,
    outer_radius: f32,
    ring: Vec<(f32, f32, f32)>,
    center: [(f32, f32); 2],
}

/// Room kept between a name and the edges of its slot.
const MARGIN: f32 = 4.0;
/// The smallest text size a name is set in before the menu grows.
const MIN_SIZE: f32 = 10.0;
/// How far the centre and the ring may grow for the names to fit, as a factor of the style's.
const MAX_GROWTH: f32 = 2.0;

/// The middle of slice `i` of `n`, in radians clockwise from +x; slice 0 is centred on the top.
fn slice_middle(i: usize, n: usize) -> f32 {
    -PI / 2.0 + i as f32 * TAU / n as f32
}

/// The text sizes a name may take: `max` down to [`MIN_SIZE`], a point at a time.
fn sizes(max: f32) -> impl Iterator<Item = f32> {
    let min = MIN_SIZE.min(max);
    (0..)
        .map(move |k| max - k as f32)
        .take_while(move |s| *s >= min)
}

/// A wrap width at which `inside` takes the name's block (icon over text, as row rectangles) centred on
/// `at` in at most `max_lines` rows: from `max_width` down to the longest word's, so no word breaks.
fn fit_width<A>(
    fonts: &mut FontsView<'_>,
    slot: Option<&Slot<A>>,
    (icon_size, size, max_width, max_lines): (f32, f32, f32, usize),
    at: Pos2,
    inside: impl Fn(&[Rect]) -> bool,
) -> Option<f32> {
    let word = name_text(slot)
        .0
        .split_whitespace()
        .map(|w| {
            fonts
                .layout_no_wrap(w.into(), FontId::proportional(size), Color32::WHITE)
                .size()
                .x
        })
        .fold(0.0, f32::max)
        + 1.0;
    let widest = max_width.max(word);
    (0..=4).rev().find_map(|k| {
        let width = word + (widest - word) * k as f32 / 4.0;
        let (icon, text) = name_galleys(
            fonts,
            slot,
            (icon_size, size, width),
            (Color32::WHITE, Color32::WHITE),
        );
        (text.rows.len() <= max_lines && inside(&block_rects(icon.as_deref(), &text, at)))
            .then_some(width)
    })
}

/// The icon's and each text row's rectangle of a block centred on `at`.
fn block_rects(icon: Option<&Galley>, text: &Galley, at: Pos2) -> Vec<Rect> {
    let size = block_size(icon, text);
    let mut y = at.y - size.y / 2.0;
    let mut rects = Vec::with_capacity(text.rows.len() + 1);
    if let Some(g) = icon {
        rects.push(Rect::from_min_size(
            pos2(at.x - g.size().x / 2.0, y),
            g.size(),
        ));
        y += g.size().y + ICON_GAP;
    }
    // A centred job lays out around x = 0.
    rects.extend(text.rows.iter().map(|r| r.rect().translate(vec2(at.x, y))));
    rects
}

/// Whether every rectangle (relative to the menu's centre) lies inside the slice centred on `middle` of a
/// ring of `n` between the radii `(inner, outer)`, with [`MARGIN`] to spare.
fn inside_slice(rects: &[Rect], (inner, outer): (f32, f32), middle: f32, n: usize) -> bool {
    // The inward normals of the slice's two edges (angles grow clockwise).
    let half = PI / n.max(1) as f32;
    let (a0, a1) = (middle - half, middle + half);
    let (n0, n1) = (vec2(-a0.sin(), a0.cos()), vec2(a1.sin(), -a1.cos()));
    rects.iter().all(|r| {
        let corners = [
            r.left_top(),
            r.right_top(),
            r.left_bottom(),
            r.right_bottom(),
        ];
        let nearest = pos2(
            0.0f32.clamp(r.min.x, r.max.x),
            0.0f32.clamp(r.min.y, r.max.y),
        );
        corners
            .iter()
            .all(|q| q.to_vec2().length() <= outer - MARGIN)
            && nearest.to_vec2().length() >= inner + MARGIN
            && (n < 2
                || corners
                    .iter()
                    .all(|q| q.to_vec2().dot(n0) >= MARGIN && q.to_vec2().dot(n1) >= MARGIN))
    })
}

/// Whether every rectangle (relative to the menu's centre) lies inside the centre half of `radius` on side `d`
/// (-1 left, 1 right), with [`MARGIN`] to spare.
fn inside_half(rects: &[Rect], radius: f32, d: f32) -> bool {
    rects.iter().all(|r| {
        [
            r.left_top(),
            r.right_top(),
            r.left_bottom(),
            r.right_bottom(),
        ]
        .iter()
        .all(|q| q.to_vec2().length() <= radius - MARGIN && d * q.x >= MARGIN)
    })
}

/// A slot's name and whether it is set in italics: a free slot says "Free".
fn name_text<A>(slot: Option<&Slot<A>>) -> (&str, bool) {
    match slot {
        Some(s) => (s.label.as_str(), false),
        None => ("Free", true),
    }
}

/// A slot's icon and its name wrapped at `width`, centred.
fn name_galleys<A>(
    fonts: &mut FontsView<'_>,
    slot: Option<&Slot<A>>,
    (icon_size, text_size, width): (f32, f32, f32),
    (icon_color, text_color): (Color32, Color32),
) -> (Option<Arc<Galley>>, Arc<Galley>) {
    let icon = slot
        .filter(|s| !s.icon.is_empty())
        .map(|s| fonts.layout_no_wrap(s.icon.clone(), FontId::proportional(icon_size), icon_color));
    let (text, italics) = name_text(slot);
    let mut job = LayoutJob::default();
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
    (icon, fonts.layout_job(job))
}

/// The gap between a slot's icon and its name.
const ICON_GAP: f32 = 2.0;

/// The size of a slot's block: icon over name.
fn block_size(icon: Option<&Galley>, text: &Galley) -> Vec2 {
    let h = icon.map_or(0.0, |g| g.size().y + ICON_GAP) + text.size().y;
    let w = icon.map_or(0.0, |g| g.size().x).max(text.size().x);
    vec2(w, h)
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

    /// The centre and outer radii the open menu is drawn with: the style's, or larger when a name needs the
    /// room. `None` when closed.
    pub fn radii(&self) -> Option<(f32, f32)> {
        self.open.as_ref().and_then(|o| o.radii)
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
                    radii: None,
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
        let open = self.open.as_mut()?;
        let (center_radius, outer_radius) = open
            .radii
            .unwrap_or((menu.style.center_radius, menu.style.outer_radius));
        let d: Vec2 = pos - open.origin;
        let distance = d.length();
        let n = menu.ring(open.ring).len();
        if distance <= center_radius || n == 0 {
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
        if open.armed && distance >= outer_radius {
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
    sizes: (f32, f32, f32),
    (icon_color, text_color): (Color32, Color32),
) -> Rect {
    let (icon, text) =
        painter.fonts_mut(|f| name_galleys(f, slot, sizes, (icon_color, text_color)));
    let size = block_size(icon.as_deref(), &text);
    let top = c.y - size.y / 2.0;
    let mut y = top;
    if let Some(g) = icon {
        let hh = g.size().y;
        painter.galley(pos2(c.x - g.size().x / 2.0, y), g, icon_color);
        y += hh + ICON_GAP;
    }
    // A centred job lays out around x = 0.
    painter.galley(pos2(c.x, y), text, text_color);
    Rect::from_center_size(pos2(c.x, top + size.y / 2.0), size)
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
