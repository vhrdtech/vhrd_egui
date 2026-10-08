//! Steady widths: a piece of UI that remembers the widest it has been and
//! keeps that room, so live values changing width don't shove their
//! neighbours back and forth every frame.

use egui::{AsIdSalt, Id, InnerResponse, Ui};

/// Lay out `add_contents` at least as wide as it has ever been under `id_salt`
/// (scoped to `ui.id()`): it grows at once, never shrinks, and the spare room
/// sits on the side the layout grows toward (right of a left-to-right row,
/// left of a right-to-left one). Remembered in egui temp memory for the session.
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
    let inner = ui.scope(|ui| {
        ui.set_min_width(widest);
        add_contents(ui)
    });
    remember(ui, id, inner.response.rect.width());
    inner
}

/// The widest width stored under `id`, 0 when none yet.
pub(crate) fn remembered(ui: &Ui, id: Id) -> f32 {
    ui.data(|d| d.get_temp::<f32>(id)).unwrap_or(0.0)
}

/// Store `width` under `id` when it is wider than what is there.
pub(crate) fn remember(ui: &Ui, id: Id, width: f32) {
    if width > remembered(ui, id) {
        ui.data_mut(|d| d.insert_temp(id, width));
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
