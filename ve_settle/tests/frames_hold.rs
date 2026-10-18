//! Frames in a world that holds (SETL-38, SETL-39, SETL-40): a frame does not push its neighbours unless its ☰ says so,
//! "Tidy up" settles the world once, and the handle in a frame's corner resizes it: the frame keeps the size it is
//! let go at (SETL-42).

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
    /// How tall each panel's content is.
    heights: Vec<f32>,
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
            heights: vec![60.0; 3],
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        let view = View {
            offset: ui.max_rect().min.to_vec2(),
            clip: ui.clip_rect(),
        };
        self.offset = view.offset;
        let heights = self.heights.clone();
        let mut draw = |ui: &mut egui::Ui, slot: &mut Slot<usize>| -> Option<FrameAction> {
            let i = slot.widget;
            let h = heights[i];
            WidgetFrame::new(format!("panel {i}"))
                .sizes(hints())
                .layout_menu(true)
                .show(ui, &mut slot.frame, |ui, _| {
                    ui.allocate_space(vec2(ui.available_width(), h));
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
    h.state_mut()
        .frames
        .apply(id, FrameAction::Tidy(ve_frame::Toward::Up));
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

/// SETL-42 (user, P2605#18oct-1407b: "the handle snaps to other modes and hardly lets anything be resized"): a
/// frame keeps exactly the size it was let go at, its switches on auto pick the forms that fit it, its body
/// scrolls where its content wants more, and a neighbour that is not pushed stays.
#[test]
fn the_resize_handle_keeps_the_size_it_was_let_go_at() {
    use ve_frame::Size;
    let mut h = host();
    until_still(&mut h);
    let o = h.state().offset;
    let others = |h: &App| [1, 2].map(|i| h.state().rect(i));
    let before = others(&h);
    // narrower than its nom form, and 40 taller: that size, the min form shows
    let r = h.state().rect(0);
    let from = grip(&h, 0);
    let to = pos2(r.min.x + 130.0, r.max.y + 40.0) + o;
    mouse_drag(&mut h, from, to);
    until_still(&mut h);
    for _ in 0..300 {
        h.step();
    }
    let f = *h.state().frame(0);
    assert_eq!((f.size, f.height), (SizeMode::Auto, HeightMode::Auto));
    let (w, t) = (
        f.grid_width.expect("a width of its own"),
        f.grid_height.expect("a height of its own"),
    );
    let now = h.state().rect(0);
    assert!(
        (now.size().x - w).abs() < 0.5 && (now.size().y - t).abs() < 0.5,
        "the body is the size it was given: {now:?} {w} × {t}"
    );
    // (the corner follows the pointer from where it took the handle)
    let by = to - from;
    assert!(
        (w - (r.size().x + by.x)).abs() < 1.0,
        "no snap to a form's width: {w}"
    );
    assert!((t - (r.size().y + by.y)).abs() < 1.0, "{t} for {r:?}");
    assert_eq!(f.shown_size(), Some(Size::Min), "auto shows what fits");
    assert!(
        (now.min - r.min).max_abs() < 0.5,
        "its top left corner stays: {r:?} {now:?}"
    );
    for (i, (a, b)) in before.iter().zip(others(&h)).enumerate() {
        assert!(moved(*a, b) < 0.5, "panel {} moved: {a:?} {b:?}", i + 1);
    }
    // its content wants far more: the size stays, the body scrolls
    h.state_mut().heights[0] = 400.0;
    for _ in 0..120 {
        h.step();
    }
    until_still(&mut h);
    let f = *h.state().frame(0);
    assert!(
        moved(now, h.state().rect(0)) < 0.5,
        "it grew: {:?}",
        h.state().rect(0)
    );
    assert!(f.overflow_scroll().y, "its body scrolls");
    // a fixed form picked on its switch keeps the size too
    {
        let host = h.state_mut();
        let id = host.slots[0];
        host.frames.dock.get_mut(id).expect("slot").frame.size = SizeMode::Wide;
    }
    for _ in 0..120 {
        h.step();
    }
    until_still(&mut h);
    assert!(
        moved(now, h.state().rect(0)) < 0.5,
        "a fixed form changed the size: {:?}",
        h.state().rect(0)
    );
    // panel 1 pulled far right: it does not push, so it stops at panel 2, which stays
    let r2 = h.state().rect(2);
    let from = grip(&h, 1);
    mouse_drag(&mut h, from, pos2(r2.max.x, from.y - o.y) + o);
    until_still(&mut h);
    assert!(moved(r2, h.state().rect(2)) < 0.5, "panel 2 was pushed");
    assert!(
        h.state().rect(1).max.x <= r2.min.x,
        "{:?} {r2:?}",
        h.state().rect(1)
    );
    assert!(
        h.state().resized.len() >= 2,
        "reported: {:?}",
        h.state().resized
    );
    // a double click gives it back to auto and its own size
    let at = grip(&h, 1);
    h.hover_at(at);
    h.step();
    for _ in 0..2 {
        h.drag_at(at);
        h.step();
        h.drop_at(at);
        h.step();
    }
    until_still(&mut h);
    let f = h.state().frame(1);
    assert_eq!(
        (f.size, f.grid_width, f.grid_height),
        (SizeMode::Auto, None, None)
    );
}

#[test]
fn a_frame_that_cannot_grow_scrolls_and_its_neighbour_stays() {
    let mut h = host();
    until_still(&mut h);
    // panel 1 right under panel 0
    let (r0, id1) = (h.state().rect(0), h.state().slots[1]);
    let size = h.state().rect(1).size();
    let gap = h.state().world.params().gap;
    let at = Vec2::new(r0.min.x + size.x / 2.0, r0.max.y + gap + 4.0 + size.y / 2.0);
    {
        let host = h.state_mut();
        host.frames.place(&mut host.world, id1, at);
    }
    until_still(&mut h);
    let under = h.state().rect(1);
    // panel 0's content wants 150 more
    h.state_mut().heights[0] = 210.0;
    for _ in 0..120 {
        h.step();
    }
    until_still(&mut h);
    assert!(
        moved(under, h.state().rect(1)) < 0.5,
        "pushed: {:?}",
        h.state().rect(1)
    );
    let r = h.state().rect(0);
    assert!(r.max.y <= under.min.y - gap + 0.5, "in it: {r:?} {under:?}");
    let f = h.state().frame(0);
    assert!(f.layout_height().is_some(), "capped by the layout");
    let drawn = f.drawn_rect().expect("drawn");
    assert!(
        drawn.height() <= r.size().y + 0.5,
        "drawn as tall as its body: {drawn:?} {r:?}"
    );
    assert!(f.overflow_scroll().y, "its body scrolls");
    // moved out of the way, it grows to its content
    let far = Vec2::new(at.x, at.y + 400.0);
    {
        let host = h.state_mut();
        host.frames.place(&mut host.world, id1, far);
    }
    for _ in 0..120 {
        h.step();
    }
    until_still(&mut h);
    assert!(h.state().frame(0).layout_height().is_none(), "room again");
    assert!(
        h.state().rect(0).size().y > r.size().y + 100.0,
        "{:?}",
        h.state().rect(0)
    );
}
