//! Who owns the widgets and carries out their moves (FRM-4): between the grid, egui windows and OS windows.

use egui::{Id, Ui};
use serde::{Deserialize, Serialize};

use crate::menu::Moves;
use crate::remote::{Parcel, Remote};
use crate::state::{FrameAction, FrameState, Placement, ViewportSlot, ViewportTarget};

/// Names a widget in a [`Dock`]; stable while it lives there, whatever its placement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SlotId(pub u64);

/// A widget with its frame state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Slot<W> {
    pub id: SlotId,
    pub widget: W,
    pub frame: FrameState,
}

/// What [`Dock::apply`] did, or what is left for the host to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Applied {
    /// The widget changed its placement.
    Moved,
    /// The user wants it on this machine: pack it and call [`Dock::send`].
    Send(String),
    /// The user wants it closed: [`Dock::remove`] it (the host may ask first).
    Close,
    /// No widget with that id.
    Gone,
}

/// Owns widgets of type `W` with their [`FrameState`] and moves them: into an egui `Window` and back, to
/// another OS window, off to another machine. It lays out nothing in the grid: the host asks which widgets
/// sit in the grid of a window ([`Dock::ids`]) and arranges those itself (rows, tiles, a physics layout).
///
/// Per frame, a host:
/// 1. draws the grid of the main window from `ids(ViewportSlot::MAIN, false)`, each widget in a
///    [`crate::WidgetFrame`], collecting [`crate::FrameResponse::action`];
/// 2. calls [`Dock::show_windows`] for the floating ones and [`Dock::show_viewports`] for the other OS
///    windows (drawing each one's grid the same way);
/// 3. hands every collected action to [`Dock::apply`].
///
/// A move inside the process keeps the widget value as it is, connections and caches included. The dock is
/// serializable when `W` is: save it with the layout, and widgets come back where they were.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Dock<W> {
    slots: Vec<Slot<W>>,
    next_id: u64,
}

impl<W> Default for Dock<W> {
    fn default() -> Self {
        Self {
            slots: Vec::new(),
            next_id: 0,
        }
    }
}

impl<W> Dock<W> {
    /// An empty dock.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a widget to the grid of the main window.
    pub fn add(&mut self, widget: W) -> SlotId {
        self.add_with(widget, FrameState::default())
    }

    /// Add a widget with a given state (a restored one, one that arrived in a [`Parcel`]).
    pub fn add_with(&mut self, widget: W, frame: FrameState) -> SlotId {
        let id = SlotId(self.next_id);
        self.next_id += 1;
        self.slots.push(Slot { id, widget, frame });
        id
    }

    /// Take a widget out.
    pub fn remove(&mut self, id: SlotId) -> Option<Slot<W>> {
        let at = self.slots.iter().position(|s| s.id == id)?;
        Some(self.slots.remove(at))
    }

    pub fn get(&self, id: SlotId) -> Option<&Slot<W>> {
        self.slots.iter().find(|s| s.id == id)
    }

    pub fn get_mut(&mut self, id: SlotId) -> Option<&mut Slot<W>> {
        self.slots.iter_mut().find(|s| s.id == id)
    }

    /// Every widget, in the order they were added.
    pub fn iter(&self) -> impl Iterator<Item = &Slot<W>> {
        self.slots.iter()
    }

    /// Every widget, mutable, in the order they were added.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Slot<W>> {
        self.slots.iter_mut()
    }

    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// The widgets of one OS window: those in its grid (`floating` false) or those floating over it.
    pub fn ids(&self, viewport: ViewportSlot, floating: bool) -> Vec<SlotId> {
        let place = Placement { viewport, floating };
        self.slots
            .iter()
            .filter(|s| s.frame.placement == place)
            .map(|s| s.id)
            .collect()
    }

    /// Every OS window: the main one, then each extra one that holds a widget. An extra window exists as
    /// long as something is in it.
    pub fn viewports(&self) -> Vec<ViewportSlot> {
        let mut all = vec![ViewportSlot::MAIN];
        all.extend(self.slots.iter().map(|s| s.frame.placement.viewport));
        all.sort();
        all.dedup();
        all
    }

    /// What the frames' menus can offer right now: the open windows, and the machines of `remote` if the app
    /// has one.
    pub fn moves(&self, remote: Option<&dyn Remote>) -> Moves {
        Moves::new(self.viewports()).remote(remote)
    }

    /// Carry out a menu pick. Moves happen here; sending and closing come back for the host to do.
    pub fn apply(&mut self, id: SlotId, action: FrameAction) -> Applied {
        let fresh = ViewportSlot(self.viewports().last().map_or(0, |v| v.0) + 1);
        let Some(slot) = self.get_mut(id) else {
            return Applied::Gone;
        };
        let place = &mut slot.frame.placement;
        match action {
            FrameAction::PopOut => place.floating = true,
            FrameAction::DockBack => place.floating = false,
            FrameAction::SendToViewport(target) => {
                *place = Placement {
                    viewport: match target {
                        ViewportTarget::New => fresh,
                        ViewportTarget::Existing(v) => v,
                    },
                    floating: false,
                };
            }
            FrameAction::SendToMachine(machine) => return Applied::Send(machine),
            FrameAction::Close => return Applied::Close,
        }
        slot.frame.land();
        Applied::Moved
    }

    /// Send a widget to `machine`: `pack` gives its title and its serialized form, `remote` takes the
    /// [`Parcel`]. Only when that worked is the widget removed here; on `Err` it stays and the reason comes
    /// back for the user.
    pub fn send(
        &mut self,
        id: SlotId,
        machine: &str,
        remote: &mut dyn Remote,
        pack: impl FnOnce(&W) -> Result<(String, String), String>,
    ) -> Result<(), String> {
        let slot = self.get(id).ok_or("the widget is gone")?;
        let (title, widget) = pack(&slot.widget)?;
        let parcel = Parcel {
            title,
            widget,
            size: slot.frame.size,
        };
        remote.send(machine, parcel)?;
        self.remove(id);
        Ok(())
    }

    /// Put every widget of an extra OS window back into the main grid (the window was closed).
    pub fn close_viewport(&mut self, viewport: ViewportSlot) {
        for slot in &mut self.slots {
            if slot.frame.placement.viewport == viewport {
                slot.frame.placement = Placement::default();
            }
        }
    }

    /// The floating widgets of `viewport`, each as an egui `Window` without chrome of its own: the frame
    /// `add` draws is the window, and dragging its header or body moves it. Its right edge, bottom edge and
    /// corner resize it (a rectangle; a hexagon keeps its shape): the size is kept in
    /// [`FrameState::window_size`] and never goes below what the frame needs. Returns the menu picks, for
    /// [`Dock::apply`].
    pub fn show_windows(
        &mut self,
        ui: &mut Ui,
        viewport: ViewportSlot,
        mut add: impl FnMut(&mut Ui, &mut Slot<W>) -> Option<FrameAction>,
    ) -> Vec<(SlotId, FrameAction)> {
        let mut picked = Vec::new();
        let origin = ui.max_rect().min;
        for (n, id) in self.ids(viewport, true).into_iter().enumerate() {
            let Some(slot) = self.get_mut(id) else {
                continue;
            };
            let step = 28.0 * (n % 8) as f32;
            let window_id = Id::new(("ve_frame::window", id.0));
            let action = egui::Window::new("")
                .id(window_id)
                .title_bar(false)
                .resizable(false)
                .frame(egui::Frame::NONE)
                .default_pos(origin + egui::vec2(48.0 + step, 48.0 + step))
                .show(ui.ctx(), |ui| {
                    let resizable = slot.frame.is_resizable();
                    let wanted = slot.frame.window_size.filter(|_| resizable);
                    if let Some([w, h]) = wanted {
                        ui.set_min_size(egui::vec2(w, h));
                        ui.set_max_size(egui::vec2(w, h));
                    }
                    let action = add(ui, slot);
                    let drawn = ui.min_rect();
                    if resizable {
                        if let Some(size) = resize_handles(ui, drawn, window_id) {
                            slot.frame.window_size = Some(size.into());
                        } else if wanted.is_some() {
                            // the frame needed more than was asked: that is the size now
                            slot.frame.window_size = Some(drawn.size().into());
                        }
                    }
                    action
                })
                .and_then(|r| r.inner)
                .flatten();
            if let Some(action) = action {
                picked.push((id, action));
            }
        }
        picked
    }

    /// The extra OS windows (egui viewports), each titled `app_title · Window N`. `add` draws one window's
    /// content: its grid and, through [`Dock::show_windows`], its floating widgets. Where the backend has no
    /// multi-viewport (web, tests) egui shows them as windows inside the main one. Closing an OS window puts
    /// its widgets back into the main grid.
    pub fn show_viewports(
        &mut self,
        ctx: &egui::Context,
        app_title: &str,
        mut add: impl FnMut(&mut Self, &mut Ui, ViewportSlot),
    ) {
        for slot in self.viewports() {
            if slot.is_main() {
                continue;
            }
            let builder = egui::ViewportBuilder::default()
                .with_title(format!("{app_title} · {}", slot.label()))
                .with_inner_size([720.0, 480.0]);
            let closed = ctx.show_viewport_immediate(slot.viewport_id(), builder, |ui, class| {
                if class == egui::ViewportClass::EmbeddedWindow {
                    add(self, ui, slot);
                    false
                } else {
                    egui::CentralPanel::default().show(ui, |ui| add(self, ui, slot));
                    ui.ctx().input(|i| i.viewport().close_requested())
                }
            });
            if closed {
                self.close_viewport(slot);
            }
        }
    }
}

/// Width of the strips along the right and bottom edge that resize a floating frame, and the corner's size.
const RESIZE_EDGE: f32 = 5.0;
const RESIZE_CORNER: f32 = 12.0;
/// The least a floating frame can be dragged to; the frame's own needs come on top.
const RESIZE_LEAST: egui::Vec2 = egui::vec2(60.0, 40.0);

/// The resize handles of a floating frame drawn in `rect`: the right edge, the bottom edge and the corner
/// (with a small grip). Returns the new size while one is dragged.
fn resize_handles(ui: &mut Ui, rect: egui::Rect, id: Id) -> Option<egui::Vec2> {
    use egui::{CursorIcon, Rect, Sense, pos2, vec2};
    let corner = Rect::from_min_max(rect.max - vec2(RESIZE_CORNER, RESIZE_CORNER), rect.max);
    let right = Rect::from_min_max(
        pos2(rect.right() - RESIZE_EDGE, rect.top() + RESIZE_CORNER),
        pos2(rect.right(), corner.top()),
    );
    let bottom = Rect::from_min_max(
        pos2(rect.left() + RESIZE_CORNER, rect.bottom() - RESIZE_EDGE),
        pos2(corner.left(), rect.bottom()),
    );
    let handles = [
        (
            right,
            "right",
            vec2(1.0, 0.0),
            CursorIcon::ResizeHorizontal,
            "width",
        ),
        (
            bottom,
            "bottom",
            vec2(0.0, 1.0),
            CursorIcon::ResizeVertical,
            "height",
        ),
        (
            corner,
            "corner",
            vec2(1.0, 1.0),
            CursorIcon::ResizeNwSe,
            "size",
        ),
    ];
    let mut delta = egui::Vec2::ZERO;
    let mut dragged = false;
    let mut lit = false;
    for (handle, salt, axes, cursor, what) in handles {
        let r = ui.interact(handle, id.with(("resize", salt)), Sense::drag());
        r.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Other, true, format!("resize {salt}"))
        });
        if r.dragged() {
            dragged = true;
            delta += r.drag_delta() * axes;
        }
        lit |= r.hovered() || r.dragged();
        r.on_hover_cursor(cursor)
            .on_hover_text(format!("Drag to change the window's {what}"));
    }
    // the grip: two short diagonal strokes in the corner
    let color = if lit {
        ui.visuals().text_color()
    } else {
        ui.visuals().weak_text_color()
    };
    let stroke = egui::Stroke::new(1.0, color);
    for inset in [4.0, 8.0] {
        let a = pos2(rect.right() - inset, rect.bottom() - 3.0);
        let b = pos2(rect.right() - 3.0, rect.bottom() - inset);
        ui.painter().line_segment([a, b], stroke);
    }
    dragged.then(|| (rect.size() + delta).max(RESIZE_LEAST))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::remote::Machine;

    struct Wire {
        up: bool,
        got: Vec<(String, Parcel)>,
    }

    impl Remote for Wire {
        fn machines(&self) -> Vec<Machine> {
            vec![Machine::new("m2", "bench PC")]
        }

        fn send(&mut self, machine: &str, parcel: Parcel) -> Result<(), String> {
            if !self.up {
                return Err("link down".into());
            }
            self.got.push((machine.to_owned(), parcel));
            Ok(())
        }
    }

    #[test]
    fn pop_out_and_back_keeps_the_widget() {
        let mut dock = Dock::new();
        let a = dock.add("a");
        let b = dock.add("b");
        assert_eq!(dock.ids(ViewportSlot::MAIN, false), [a, b]);
        assert_eq!(dock.apply(a, FrameAction::PopOut), Applied::Moved);
        assert_eq!(dock.ids(ViewportSlot::MAIN, false), [b]);
        assert_eq!(dock.ids(ViewportSlot::MAIN, true), [a]);
        dock.apply(a, FrameAction::DockBack);
        assert_eq!(dock.ids(ViewportSlot::MAIN, false), [a, b]);
        assert_eq!(dock.get(a).map(|s| s.widget), Some("a"));
    }

    #[test]
    fn windows_exist_while_they_hold_a_widget() {
        let mut dock = Dock::new();
        let a = dock.add("a");
        let b = dock.add("b");
        dock.apply(a, FrameAction::PopOut);
        dock.apply(a, FrameAction::SendToViewport(ViewportTarget::New));
        let second = ViewportSlot(1);
        assert_eq!(dock.viewports(), [ViewportSlot::MAIN, second]);
        // it arrives in that window's grid, and can float there again
        assert_eq!(dock.ids(second, false), [a]);
        dock.apply(a, FrameAction::PopOut);
        assert_eq!(dock.ids(second, true), [a]);
        dock.apply(b, FrameAction::SendToViewport(ViewportTarget::New));
        assert_eq!(dock.viewports().len(), 3);
        dock.apply(
            b,
            FrameAction::SendToViewport(ViewportTarget::Existing(second)),
        );
        assert_eq!(dock.viewports(), [ViewportSlot::MAIN, second]);
        dock.close_viewport(second);
        assert_eq!(dock.viewports(), [ViewportSlot::MAIN]);
        assert_eq!(dock.ids(ViewportSlot::MAIN, false), [a, b]);
    }

    #[test]
    fn a_sent_widget_leaves_only_when_the_send_worked() {
        let mut dock = Dock::new();
        let a = dock.add(7_u32);
        dock.get_mut(a).unwrap().frame.size = crate::SizeMode::Wide;
        let mut wire = Wire {
            up: false,
            got: Vec::new(),
        };
        let action = FrameAction::SendToMachine("m2".into());
        assert_eq!(dock.apply(a, action), Applied::Send("m2".into()));
        let pack = |w: &u32| Ok(("seven".to_owned(), w.to_string()));
        assert_eq!(dock.send(a, "m2", &mut wire, pack), Err("link down".into()));
        assert_eq!(dock.len(), 1, "a failed send keeps the widget");
        wire.up = true;
        assert_eq!(dock.send(a, "m2", &mut wire, pack), Ok(()));
        assert!(dock.is_empty());
        let (machine, parcel) = &wire.got[0];
        assert_eq!(machine, "m2");
        assert_eq!(
            (parcel.title.as_str(), parcel.widget.as_str(), parcel.size),
            ("seven", "7", crate::SizeMode::Wide)
        );
        assert_eq!(dock.apply(a, FrameAction::PopOut), Applied::Gone);
    }

    #[test]
    fn menus_offer_the_remote_only_when_there_is_one() {
        let dock: Dock<u8> = Dock::new();
        assert!(dock.moves(None).machines.is_none());
        let wire = Wire {
            up: true,
            got: Vec::new(),
        };
        let moves = dock.moves(Some(&wire));
        assert_eq!(moves.machines.map(|m| m.len()), Some(1));
        assert_eq!(moves.viewports, [ViewportSlot::MAIN]);
    }
}
