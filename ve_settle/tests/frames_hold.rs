//! Frames in a world that holds (SETL-38, SETL-39, SETL-40): a frame does not push its neighbours unless its ☰ says so,
//! "Tidy up" settles the world once, and the handle in a frame's corner resizes it: the width snaps to its min /
//! nom / wide form, the height is free.

use egui::{Pos2, pos2, vec2};
use egui_kittest::Harness;
use ve_frame::{FrameAction, HeightMode, SizeHints, SizeMode, Slot, SlotId, WidgetFrame};
use ve_settle::frames::{Frames, RESIZE_GRIP, Seat, Shown, View};
use ve_settle::{Params, Rect, Settling, Vec2, World};

const WINDOW: egui::Vec2 = vec2(1300.0, 700.0);

fn hints() -> SizeHints {
    SizeHints::new([100.0, 40.0], [200.0, 60.0], [400.0, 60.0])
}

/// Three panels side by side with room around them, in a world with gravity up (when it settles).
struct Host {
    world: World,
    frames: Frames<usize>,
    slots: Vec<SlotId>,
    laid: bool,
    shown: Shown,
    resized: Vec<ve_settle::frames::Resized>,
    /// Where the world's origin is on the screen.
    offset: egui::Vec2,
}

impl Host {
    fn new() -> Self {
        let mut p = Params::default();
        p.walls.bottom = false;
        p.home_stiffness = 0.0;
        let bounds = Rect::from_min_size(Vec2::ZERO, Vec2::new(WINDOW.x, WINDOW.y));
        let mut world = World::new(bounds, p);
        let mut frames = Frames::new();
        let slots = (0..3)
            .map(|i| {
                let seat = Seat::new(format!("panel {i}")).hints(hints());
                frames.add(&mut world, i, seat, Vec2::new(150.0, 300.0))
            })
            .collect();
        Self {
            world,
            frames,
            slots,
            laid: false,
            shown: Shown::default(),
            resized: Vec::new(),
            offset: egui::Vec2::ZERO,
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        let view = View {
            offset: ui.max_rect().min.to_vec2(),
            clip: ui.clip_rect(),
        };
        self.offset = view.offset;
        let mut draw = |ui: &mut egui::Ui, slot: &mut Slot<usize>| -> Option<FrameAction> {
            let i = slot.widget;
            WidgetFrame::new(format!("panel {i}"))
                .sizes(hints())
                .layout_menu(true)
                .show(ui, &mut slot.frame, |ui, _| {
                    ui.allocate_space(vec2(ui.available_width(), 60.0));
                })
                .action
        };
        if !self.laid {
            self.laid = true;
            // apart, low on the page: room to grow and room for gravity to take them up
            for (n, &id) in self.slots.iter().enumerate() {
                self.frames.measure(ui, id, &mut draw);
                self.frames.set_pick(id, ve_frame::Size::Nom);
                let at = Vec2::new(150.0 + 320.0 * n as f32, 300.0 + 40.0 * n as f32);
                self.frames.place(&mut self.world, id, at);
            }
            self.frames.snap(&mut self.world);
        }
        self.world.advance(ui.input(|i| i.stable_dt).min(0.1));
        self.shown = self.frames.show(ui, &mut self.world, view, &mut draw);
        self.resized.extend(self.shown.resized.iter().copied());
        for (id, action) in std::mem::take(&mut self.shown.picked) {
            self.frames.apply(id, action);
        }
    }

    fn rect(&self, i: usize) -> Rect {
        let body = self.frames.body(self.slots[i]).expect("a body");
        self.world.body(body).expect("in the world").rect
    }

    fn frame(&self, i: usize) -> &ve_frame::FrameState {
        &self.frames.dock.get(self.slots[i]).expect("slot").frame
    }
}

type App = Harness<'static, Host>;

fn host() -> App {
    let mut h = Harness::builder()
        .with_size(WINDOW)
        .with_step_dt(1.0 / 60.0)
        .build_ui_state(|ui, host: &mut Host| host.ui(ui), Host::new());
    h.step();
    h
}

fn until_still(h: &mut App) {
    for _ in 0..2_000 {
        if !h.state().shown.moving {
            return;
        }
        h.step();
    }
    panic!("still moving");
}

fn moved(a: Rect, b: Rect) -> f32 {
    (a.min - b.min).max_abs().max((a.max - b.max).max_abs())
}

/// The middle of the resize handle of panel `i`.
fn grip(h: &App, i: usize) -> Pos2 {
    let r = h.state().rect(i);
    pos2(r.max.x - RESIZE_GRIP / 2.0, r.max.y - RESIZE_GRIP / 2.0) + h.state().offset
}

fn mouse_drag(h: &mut App, from: Pos2, to: Pos2) {
    h.hover_at(from);
    h.step();
    h.drag_at(from);
    h.step();
    for k in 1..=10 {
        h.hover_at(from + (to - from) * (k as f32 / 10.0));
        h.step();
    }
    h.drop_at(to);
    h.step();
}

#[test]
fn frames_hold_where_they_are_and_push_nothing_by_default() {
    let mut h = host();
    until_still(&mut h);
    let start: Vec<Rect> = (0..3).map(|i| h.state().rect(i)).collect();
    for _ in 0..300 {
        h.step();
    }
    for (i, r) in start.iter().enumerate() {
        assert!(moved(*r, h.state().rect(i)) < 0.01, "panel {i} drifted");
        assert!(!h.state().frame(i).push, "panel {i} pushes by default");
    }
    assert_eq!(h.state().world.settling(), Settling::Hold);
    // panel 0 dragged onto panel 1: panel 1 stays, panel 0 stops at its edge
    let (r1, o) = (h.state().rect(1), h.state().offset);
    let from = pos2(h.state().rect(0).center().x, h.state().rect(0).min.y + 40.0) + o;
    mouse_drag(
        &mut h,
        from,
        pos2(r1.center().x - 40.0, from.y + 10.0) + vec2(o.x, 0.0),
    );
    until_still(&mut h);
    assert!(moved(r1, h.state().rect(1)) < 0.5, "panel 1 was pushed");
    let (a, b) = (h.state().rect(0), h.state().rect(1));
    let o = a.overlap(&b, 0.0);
    assert!(o.x.min(o.y) <= 0.0, "they overlap: {a:?} {b:?}");
}

#[test]
fn tidy_up_from_a_frame_menu_settles_once() {
    let mut h = host();
    until_still(&mut h);
    let id = h.state().slots[2];
    h.state_mut().frames.apply(id, FrameAction::Tidy);
    h.step();
    until_still(&mut h);
    assert_eq!(h.state().world.settling(), Settling::Hold, "holds again");
    let gap = h.state().world.params().gap;
    for i in 0..3 {
        assert!(
            h.state().rect(i).min.y < gap + 0.5,
            "panel {i} went up: {:?}",
            h.state().rect(i)
        );
    }
}

#[test]
fn the_resize_handle_snaps_the_width_and_frees_the_height() {
    let mut h = host();
    until_still(&mut h);
    assert_eq!(h.state().frame(0).size, SizeMode::Auto);
    // narrower than nom: it snaps to min
    let r = h.state().rect(0);
    let from = grip(&h, 0);
    let to = pos2(r.min.x + h.state().offset.x + 120.0, from.y);
    mouse_drag(&mut h, from, to);
    until_still(&mut h);
    assert_eq!(h.state().frame(0).size, SizeMode::Min);
    assert!(
        h.state().rect(0).size().x < 160.0,
        "{:?}",
        h.state().rect(0)
    );
    // panel 1 pulled far right: it does not push, so it stops at panel 2, and nom is the nearest that fits
    let (r, r2, o) = (h.state().rect(1), h.state().rect(2), h.state().offset);
    let from = grip(&h, 1);
    mouse_drag(&mut h, from, pos2(r2.max.x, from.y - o.y) + o);
    until_still(&mut h);
    assert_eq!(h.state().frame(1).size, SizeMode::Nom, "stopped at panel 2");
    assert!(moved(r2, h.state().rect(2)) < 0.5, "panel 2 was pushed");
    assert!(
        h.state().rect(1).max.x <= r2.min.x,
        "{:?} {r2:?}",
        h.state().rect(1)
    );
    let _ = r;
    // panel 2 has the room to its right: wide, and taller
    let r = h.state().rect(2);
    let from = grip(&h, 2);
    mouse_drag(&mut h, from, pos2(r.min.x + 420.0, r.max.y + 120.0) + o);
    until_still(&mut h);
    let f = h.state().frame(2);
    assert_eq!(f.size, SizeMode::Wide, "snapped to wide");
    assert_eq!(f.height, HeightMode::Auto);
    let tall = f.grid_height.expect("a height of its own");
    assert!(
        (tall - (r.size().y + 120.0)).abs() < 2.0,
        "{tall} for {r:?}"
    );
    assert!(
        (h.state().rect(2).size().y - tall).abs() < 1.0,
        "{:?}",
        h.state().rect(2)
    );
    assert!(
        h.state().resized.len() >= 3,
        "reported: {:?}",
        h.state().resized
    );
    // a double click gives it back to auto, as tall as its content
    let at = grip(&h, 2);
    h.hover_at(at);
    h.step();
    for _ in 0..2 {
        h.drag_at(at);
        h.step();
        h.drop_at(at);
        h.step();
    }
    until_still(&mut h);
    let f = h.state().frame(2);
    assert_eq!((f.size, f.grid_height), (SizeMode::Auto, None));
}
