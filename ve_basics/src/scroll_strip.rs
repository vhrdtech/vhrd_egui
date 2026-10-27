//! A row wider than its room scrolls in place, with an arrow on each side that says so (BAS-8).

use egui::{Button, Id, RichText, ScrollArea, Ui, Vec2};
use std::{fmt::Debug, hash::Hash};

/// The words of the arrows' tooltips: what is on that side, or why the arrow can't be clicked. English by
/// default; an app with translations passes its own ([`ScrollStrip::texts`]).
#[derive(Clone, Debug)]
pub struct StripTexts {
    /// The left arrow while there is more to the left.
    pub left: String,
    /// The right arrow while there is more to the right.
    pub right: String,
    /// Either arrow at its end.
    pub at_start: String,
    pub at_end: String,
    /// Both arrows while the whole row fits.
    pub fits: String,
}

impl Default for StripTexts {
    fn default() -> Self {
        Self {
            left: "Scroll left: there is more before what is shown (or scroll the row itself)"
                .into(),
            right: "Scroll right: there is more after what is shown (or scroll the row itself)"
                .into(),
            at_start: "At the start: nothing more to the left".into(),
            at_end: "At the end: nothing more to the right".into(),
            fits: "The whole row is shown: nothing to scroll".into(),
        }
    }
}

/// What the strip measured last frame: where it is scrolled to, how wide its row and its room are.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Measure {
    offset: f32,
    content: f32,
    view: f32,
}

/// A horizontal row (chips, refs, a chain of links) that keeps its height and scrolls sideways when it is wider
/// than the room, instead of wrapping or cutting it to "+N": every item stays reachable in place. An arrow on
/// each side shows that it scrolls and moves it by most of a view; an arrow with nothing more on its side is
/// disabled and says why (at the start, at the end, all of it fits), never hidden.
///
/// ```ignore
/// ScrollStrip::new(("chain", &milestone.name)).show(ui, |ui| {
///     for r in &milestone.critical_path {
///         ui.link(r);
///     }
/// });
/// ```
///
/// The strip takes the width left in the row it sits in; the scroll position lives in egui's memory under
/// `id_salt` for this run.
pub struct ScrollStrip {
    id_salt: Id,
    texts: StripTexts,
}

impl ScrollStrip {
    /// A strip; `id_salt` must be unique within the parent `ui` (the row's own key).
    pub fn new(id_salt: impl Hash + Debug) -> Self {
        Self {
            id_salt: Id::new(id_salt),
            texts: StripTexts::default(),
        }
    }

    /// The arrows' tooltips in the app's own words.
    pub fn texts(mut self, texts: StripTexts) -> Self {
        self.texts = texts;
        self
    }

    /// Draw the arrows and the row `add` lays out from left to right; what `add` returns.
    pub fn show<R>(self, ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
        let id = ui.make_persistent_id(self.id_salt);
        let m: Measure = ui.data(|d| d.get_temp(id)).unwrap_or_default();
        let fits = m.content <= m.view + 0.5;
        let at_start = m.offset <= 0.5;
        let at_end = m.offset + m.view >= m.content - 0.5;
        let texts = &self.texts;
        let reason = |end: bool, end_text: &str| -> String {
            if fits {
                texts.fits.clone()
            } else if end {
                end_text.to_owned()
            } else {
                String::new()
            }
        };
        ui.horizontal(|ui| {
            let arrow = |ui: &mut Ui, glyph: &str, on: bool, hover: &str, off: String| {
                ui.add_enabled(on, Button::new(RichText::new(glyph).small()).frame(false))
                    .on_hover_text(hover)
                    .on_disabled_hover_text(off)
            };
            let left = arrow(
                ui,
                "⏴",
                !fits && !at_start,
                &texts.left,
                reason(at_start, &texts.at_start),
            );
            // the right arrow's room is kept before the row takes the rest
            let glyph = ui.fonts_mut(|f| {
                f.layout_no_wrap(
                    "⏵".into(),
                    egui::TextStyle::Small.resolve(ui.style()),
                    egui::Color32::WHITE,
                )
                .size()
                .x
            });
            let spacing = ui.spacing().item_spacing.x;
            let room =
                (ui.available_width() - glyph - ui.spacing().button_padding.x * 2.0 - spacing)
                    .max(0.0);
            let out = ScrollArea::horizontal()
                .id_salt(id.with("area"))
                .max_width(room)
                // egui's floor (64) would push the right arrow out of a narrow room: the strip fits it
                .min_scrolled_width(room)
                .auto_shrink([true, true])
                .show(ui, |ui| ui.horizontal(add).inner);
            let right = arrow(
                ui,
                "⏵",
                !fits && !at_end,
                &texts.right,
                reason(at_end, &texts.at_end),
            );
            let now = Measure {
                offset: out.state.offset.x,
                content: out.content_size.x,
                view: out.inner_rect.width(),
            };
            // most of a view at a click, so one item of the last view stays in sight
            let step = (now.view * 0.8).max(1.0);
            let end = (now.content - now.view).max(0.0);
            let target = if left.clicked() {
                Some((now.offset - step).max(0.0))
            } else if right.clicked() {
                Some((now.offset + step).min(end))
            } else {
                None
            };
            if let Some(x) = target {
                let mut state = out.state;
                state.offset = Vec2::new(x, state.offset.y);
                state.store(ui.ctx(), out.id);
            }
            if now != m || target.is_some() {
                ui.data_mut(|d| d.insert_temp(id, now));
                ui.ctx().request_repaint();
            }
            out.inner
        })
        .inner
    }
}
