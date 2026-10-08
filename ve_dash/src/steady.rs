//! Steady widths: a piece of UI that remembers the widest it has been and
//! keeps that room, so live values changing width don't shove their
//! neighbours back and forth every frame.

use egui::{AsIdSalt, Id, InnerResponse, Ui};

/// Lay out `add_contents` at least as wide as it has been under `id_salt`
/// (scoped to `ui.id()`): it grows at once and the spare room sits on the side the layout grows toward (right
/// of a left-to-right row, left of a right-to-left one). It does not shrink with every shorter value, only
/// compacts now and then: after [`COMPACT_AFTER`] seconds without growth it glides down to the widest the
/// content needed in that time, and never while the content's width is still changing. Remembered in egui
/// temp memory for the session.
///
/// ```ignore
/// steady_width(ui, "rx", |ui| ui.label(format!("rx {rate}")));
/// ```
pub fn steady_width<R>(
    ui: &mut Ui,
    id_salt: impl AsIdSalt,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<R> {
    let id = ui.id().with("ve_dash::steady_width").with(id_salt);
    let widest = remembered(ui, id);
    // The inner scope measures what the content needs, the outer one adds the learned room.
    let mut need = 0.0;
    let inner = ui.scope(|ui| {
        ui.set_min_width(widest);
        let content = ui.scope(add_contents);
        need = content.response.rect.width();
        content.inner
    });
    remember(ui, id, need);
    InnerResponse::new(inner.inner, inner.response)
}

/// Seconds without growth after which a steady width tries to compact.
pub const COMPACT_AFTER: f64 = 180.0;
/// A content width that changed within this many seconds is "still changing": no compaction yet.
const SETTLE: f64 = 5.0;
/// Seconds the shrinking glide takes.
const GLIDE: f32 = 0.4;

/// What is learned about one steady piece.
#[derive(Clone, Copy, Default)]
struct Learned {
    /// The width handed out (follows `target`: at once up, gliding down).
    shown: f32,
    /// The widest the content needed since `epoch`.
    peak: f32,
    /// The content's width in the last frame, and when it last differed noticeably.
    last: f32,
    changed_at: f64,
    /// When the width last grew or compacted: the compaction clock.
    epoch: f64,
}

impl Learned {
    /// One frame with the content needing `need` at time `now`; returns the width to hand out next.
    fn step(&mut self, need: f32, now: f64) -> f32 {
        if (need - self.last).abs() > 0.25 {
            self.changed_at = now;
        }
        self.last = need;
        self.peak = self.peak.max(need);
        if need > self.shown + 0.25 {
            // Growth is immediate and restarts the clock.
            self.shown = need;
            self.peak = 0.0; // the growth itself is the old news the next compaction should look past
            self.epoch = now;
        } else if now - self.epoch >= COMPACT_AFTER && now - self.changed_at >= SETTLE {
            // Compact toward what the content needed in this whole time; try again after another round.
            if self.peak < self.shown - 0.5 {
                self.shown = self.peak;
            }
            self.epoch = now;
            self.peak = need;
        }
        self.shown
    }
}

/// Where the generation of the learned widths is kept.
fn generation_id() -> Id {
    Id::new("ve_dash::steady_generation")
}

/// The current generation: bumped by [`reset_steady`], mixed into every key so older widths are not found.
fn generation(ui: &Ui) -> u32 {
    ui.data(|d| d.get_temp::<u32>(generation_id())).unwrap_or(0)
}

/// Forget every learned width (steady widths, columns, chips): they settle again on what is drawn next. Call
/// it when the text changes wholesale, e.g. when the language switches, so the widths follow the new words
/// instead of keeping the widest the old ones needed.
///
/// ```ignore
/// i18n::toggle();
/// ve_dash::reset_steady(ui.ctx());
/// ```
pub fn reset_steady(ctx: &egui::Context) {
    ctx.data_mut(|d| {
        let g = d.get_temp::<u32>(generation_id()).unwrap_or(0);
        d.insert_temp(generation_id(), g.wrapping_add(1));
    });
    ctx.request_repaint();
}

fn learned_id(ui: &Ui, id: Id) -> Id {
    id.with(generation(ui)).with("learned")
}

/// The width to keep for `id`, 0 when none yet. A compaction glides down to the new width, so neighbours
/// move smoothly; growth is immediate.
pub(crate) fn remembered(ui: &Ui, id: Id) -> f32 {
    let key = learned_id(ui, id);
    match ui.data(|d| d.get_temp::<Learned>(key)) {
        Some(l) => ui
            .ctx()
            .animate_value_with_time(key.with("glide"), l.shown, GLIDE),
        None => 0.0,
    }
}

/// Report what the piece under `id` needs this frame (`width`, its natural width). A growth asks egui for
/// another pass at once, so the pieces laid out earlier in this frame get the new width too instead of lagging
/// a frame.
pub(crate) fn remember(ui: &Ui, id: Id, width: f32) {
    let key = learned_id(ui, id);
    let now = ui.input(|i| i.time);
    let mut l = ui.data(|d| d.get_temp::<Learned>(key)).unwrap_or_default();
    let before = l.shown;
    l.step(width, now);
    ui.data_mut(|d| d.insert_temp(key, l));
    if l.shown > before + 0.25 {
        ui.ctx()
            .animate_value_with_time(key.with("glide"), l.shown, 0.0);
        ui.ctx().request_discard("ve_dash steady width grew");
    }
}

/// Measured-width variant of [`steady_width`] for layouts that need the width *before* drawing (to decide
/// what fits): records `measured` under `id_salt` and returns the widest ever recorded under it.
///
/// ```ignore
/// let w = steady_of(ui, "rx", ui.painter().layout_no_wrap(text, font, color).size().x);
/// ```
pub fn steady_of(ui: &Ui, id_salt: impl AsIdSalt, measured: f32) -> f32 {
    let id = ui.id().with("ve_dash::steady_of").with(id_salt);
    remember(ui, id, measured);
    remembered(ui, id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grows_at_once_and_keeps_until_compaction_is_due() {
        let mut l = Learned::default();
        assert_eq!(l.step(100.0, 0.0), 100.0);
        assert_eq!(l.step(40.0, 1.0), 100.0, "a shorter value keeps the room");
        assert_eq!(l.step(40.0, COMPACT_AFTER - 1.0), 100.0, "not yet due");
    }

    #[test]
    fn compacts_to_the_recent_peak_after_a_quiet_spell() {
        let mut l = Learned::default();
        l.step(100.0, 0.0);
        l.step(60.0, 10.0);
        l.step(50.0, 100.0);
        l.step(50.0, COMPACT_AFTER + 10.0);
        let w = l.step(50.0, COMPACT_AFTER + 20.0);
        assert_eq!(
            w, 60.0,
            "toward the widest since growth, not the current one"
        );
        // Another quiet round now settles on what is needed now.
        l.step(50.0, 2.0 * COMPACT_AFTER + 30.0);
        assert_eq!(l.step(50.0, 2.0 * COMPACT_AFTER + 40.0), 50.0);
    }

    #[test]
    fn never_compacts_while_the_content_is_still_changing() {
        let mut l = Learned::default();
        l.step(100.0, 0.0);
        let t = COMPACT_AFTER + 1.0;
        l.step(30.0, t - 1.0);
        // The width changed a second ago: wait.
        assert_eq!(l.step(31.0, t), 100.0);
        // Settled for a while: now it compacts.
        assert!(l.step(31.0, t + SETTLE + 1.0) < 100.0);
    }

    #[test]
    fn growth_after_compaction_is_immediate() {
        let mut l = Learned::default();
        l.step(100.0, 0.0);
        l.step(30.0, 10.0);
        l.step(30.0, COMPACT_AFTER + 20.0);
        assert!(l.step(30.0, COMPACT_AFTER + 30.0) < 100.0);
        assert_eq!(l.step(80.0, COMPACT_AFTER + 31.0), 80.0);
    }
}
