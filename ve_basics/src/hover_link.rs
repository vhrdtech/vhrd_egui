//! Cross-reference highlight: hover one item, see every related item light up.

use egui::{Id, Response, Stroke, StrokeKind, Ui};
use std::{fmt::Debug, hash::Hash};

/// Register `response` under `key`. While it is hovered, every item registered under the same key (this
/// widget included) is highlighted from the next frame on; returns whether this item is highlighted now.
///
/// ```ignore
/// let r = ui.label("eth0");
/// hover_link(ui, "iface-eth0", &r);   // elsewhere: the rx chart and the status light of eth0
/// ```
///
/// Keys are global to the egui context. The hovered key lives in ctx memory (with the hovering item) and is
/// dropped when that item stops being hovered; nothing repaints while the pointer rests.
pub fn hover_link(ui: &Ui, key: impl Hash + Debug, response: &Response) -> bool {
    let key = Id::new(key);
    let slot = Id::new("ve_basics::hover_link");
    let current = ui.data(|d| d.get_temp::<Option<(Id, Id)>>(slot)).flatten(); // (key, hovering item)
    let active = current.is_some_and(|(k, _)| k == key);
    if response.hovered() {
        if current != Some((key, response.id)) {
            ui.data_mut(|d| d.insert_temp(slot, Some((key, response.id))));
            ui.ctx().request_repaint(); // the others light up next frame
        }
    } else if current.is_some_and(|(_, owner)| owner == response.id) {
        ui.data_mut(|d| d.insert_temp(slot, None::<(Id, Id)>));
        ui.ctx().request_repaint(); // the others go dark next frame
    }
    if active {
        let color = ui.visuals().selection.stroke.color;
        ui.painter().rect_stroke(
            response.rect.expand(2.0),
            3.0,
            Stroke::new(1.5, color),
            StrokeKind::Outside,
        );
    }
    active
}
