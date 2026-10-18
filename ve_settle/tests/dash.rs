//! What a live dashboard needs of `ve_settle::frames` (SETL-29): a page that scrolls, panels in rows laid
//! out before they are shown, heights that change while it runs, hints that follow the window, and touch.

use egui::{Event, Pos2, ScrollArea, Sense, TouchDeviceId, TouchId, TouchPhase, pos2, vec2};
use egui_kittest::Harness;
use ve_frame::{FrameAction, Size, SizeHints, Slot, SlotId, WidgetFrame};
use ve_settle::Settling;
use ve_settle::frames::{Frames, Hands, LayOut, Seat, Shown, View};
use ve_settle::{Params, Rect, Vec2, World};

/// The page: as wide as the window, taller than it, so it scrolls.
const PAGE: egui::Vec2 = vec2(700.0, 1600.0);
const WINDOW: egui::Vec2 = vec2(700.0, 500.0);

fn hints(width: f32) -> SizeHints {
    SizeHints::new([80.0, 30.0], [width, 60.0], [2.0 * width, 60.0])
}

/// A dashboard host: panels `0..n` whose content is `heights[i]` tall, in a world on a page that scrolls.
struct Dash {
    world: World,
    frames: Frames<usize>,
    hints: Vec<SizeHints>,
    heights: Vec<f32>,
    slots: Vec<SlotId>,
    layout: LayOut,
    /// Rows from the first frame's [`Frames::lay_out`].
    rows: Vec<Vec<SlotId>>,
    laid: bool,
    shown: Shown,
    scroll: f32,
}

impl Dash {
    fn new(n: usize, layout: LayOut) -> Self {
        let mut p = Params::default();
        // as the sandbox's scenes: gravity packs the panels up, the bottom is open, no homes
        p.walls.bottom = false;
        p.home_stiffness = 0.0;
        p.max_speed = p.glide_speed;
        p.push_out = true;
        let bounds = Rect::from_min_size(Vec2::ZERO, Vec2::new(PAGE.x, PAGE.y));
        let mut world = World::new(bounds, p).with_settling(Settling::Always);
        let mut frames = Frames::new();
        let h = hints(200.0);
        let slots = (0..n)
            .map(|i| {
                let seat = Seat::new(format!("panel {i}")).hints(h);
                let id = frames.add(&mut world, i, seat, Vec2::new(350.0, 60.0));
                // these scenes are about what a panel pushes (SETL-38: off by default)
                if let Some(slot) = frames.dock.get_mut(id) {
                    slot.frame.push = true;
                }
                id
            })
            .collect();
        Self {
            world,
            frames,
            hints: vec![h; n],
            heights: vec![60.0; n],
            slots,
            layout,
            rows: Vec::new(),
            laid: false,
            shown: Shown::default(),
            scroll: 0.0,
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        let out = ScrollArea::vertical().id_salt("page").show(ui, |ui| {
            let (page, _) = ui.allocate_exact_size(PAGE, Sense::hover());
            let view = View {
                offset: page.min.to_vec2(),
                clip: ui.clip_rect(),
            };
            let (hints, heights) = (&self.hints, &self.heights);
            let mut draw =
                |ui: &mut egui::Ui, slot: &mut Slot<usize>| draw(ui, slot, hints, heights);
            if !self.laid {
                self.laid = true;
                for &id in &self.slots {
                    self.frames.measure(ui, id, &mut draw);
                }
                self.rows = self
                    .frames
                    .lay_out(&mut self.world, &self.slots, self.layout);
                assert!(
                    self.frames.snap(&mut self.world),
                    "the laid out world rests"
                );
            }
            let dt = ui.input(|i| i.stable_dt).min(0.1);
            self.world.advance(dt);
            self.shown = self.frames.show(ui, &mut self.world, view, &mut draw);
        });
        self.scroll = out.state.offset.y;
    }

    fn rect(&self, i: usize) -> Rect {
        let body = self.frames.body(self.slots[i]).expect("a body");
        self.world.body(body).expect("in the world").rect
    }

    fn rects(&self) -> Vec<Rect> {
        (0..self.slots.len()).map(|i| self.rect(i)).collect()
    }
}

fn draw(
    ui: &mut egui::Ui,
    slot: &mut Slot<usize>,
    hints: &[SizeHints],
    heights: &[f32],
) -> Option<FrameAction> {
    let i = slot.widget;
    WidgetFrame::new(format!("panel {i}"))
        .sizes(hints[i])
        .show(ui, &mut slot.frame, |ui, _| {
            ui.allocate_space(vec2(ui.available_width(), heights[i]));
        })
        .action
}

type App = Harness<'static, Dash>;

fn dash(n: usize, layout: LayOut) -> App {
    let mut h = Harness::builder()
        .with_size(WINDOW)
        .with_step_dt(1.0 / 60.0)
        .build_ui_state(|ui, dash: &mut Dash| dash.ui(ui), Dash::new(n, layout));
    h.step();
    h
}

/// Frames until nothing is under way ([`Shown::moving`] false); panics past `limit`.
fn until_still(h: &mut App, limit: usize) -> usize {
    for n in 0..limit {
        if !h.state().shown.moving {
            return n;
        }
        h.step();
    }
    panic!("still moving after {limit} frames");
}

fn moved(a: &Rect, b: &Rect) -> f32 {
    (a.min - b.min).max_abs().max((a.max - b.max).max_abs())
}

/// The center of panel `i` on the screen.
fn on_screen(h: &App, i: usize) -> Pos2 {
    let r = h.state().rect(i);
    let c = r.center();
    pos2(c.x, c.y + 8.0 - h.state().scroll)
}

fn touch(h: &mut App, phase: TouchPhase, at: Pos2) {
    h.event(Event::Touch {
        device_id: TouchDeviceId(1),
        id: TouchId(1),
        phase,
        pos: at,
        force: None,
    });
    // as egui-winit does for the first finger: the pointer follows it
    match phase {
        TouchPhase::Start => {
            h.event(Event::PointerMoved(at));
            h.event(Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            });
        }
        TouchPhase::Move => h.event(Event::PointerMoved(at)),
        TouchPhase::End | TouchPhase::Cancel => {
            h.event(Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            });
            h.event(Event::PointerGone);
        }
    }
    h.step();
}

/// A finger drags from `from` up by `up` points, in steps.
fn swipe_up(h: &mut App, from: Pos2, up: f32) {
    touch(h, TouchPhase::Start, from);
    for k in 1..=10 {
        touch(h, TouchPhase::Move, from - vec2(0.0, up * k as f32 / 10.0));
    }
    touch(h, TouchPhase::End, from - vec2(0.0, up));
    for _ in 0..30 {
        h.step();
    }
}

fn mouse_drag(h: &mut App, from: Pos2, by: egui::Vec2) {
    h.hover_at(from);
    h.step();
    h.drag_at(from);
    h.step();
    for k in 1..=10 {
        h.hover_at(from + by * (k as f32 / 10.0));
        h.step();
    }
    h.drop_at(from + by);
    h.step();
}

/// (a) With [`Hands::MouseOnly`] a finger dragged over a panel scrolls the page and the panel stays; the
/// mouse still moves it. With [`Hands::All`] the finger moves it too, with [`Hands::Off`] nothing does.
#[test]
fn a_touch_drag_over_a_panel_scrolls_the_page_unless_hands_say_otherwise() {
    let mut h = dash(12, LayOut::default());
    until_still(&mut h, 600);
    h.state_mut().frames.hands = Hands::MouseOnly;
    let before = h.state().rects();
    let at = on_screen(&h, 4);
    swipe_up(&mut h, at, 200.0);
    let scrolled = h.state().scroll;
    assert!(scrolled > 100.0, "the page scrolled {scrolled}");
    let after = h.state().rects();
    for (i, (a, b)) in before.iter().zip(&after).enumerate() {
        assert!(
            moved(a, b) < 0.5,
            "panel {i} moved under a finger: {a:?} to {b:?}"
        );
    }
    // the mouse still moves a panel (after it moved, the pointer is a mouse again): one in view
    let seen = (0..12).find(|&i| (40.0..400.0).contains(&on_screen(&h, i).y));
    let ys: Vec<f32> = (0..12).map(|i| on_screen(&h, i).y).collect();
    let seen =
        seen.unwrap_or_else(|| panic!("a panel in view: {ys:?}, scroll {}", h.state().scroll));
    let at = on_screen(&h, seen);
    mouse_drag(&mut h, at, vec2(0.0, 150.0));
    let dragged = moved(&before[seen], &h.state().rect(seen));
    assert!(dragged > 50.0, "the mouse moved the panel {dragged}");

    // all hands: the finger takes the panel along, the page stays
    let mut h = dash(12, LayOut::default());
    until_still(&mut h, 600);
    let at = on_screen(&h, 4);
    swipe_up(&mut h, at, 120.0);
    assert!(
        h.state().scroll < 1.0,
        "the page scrolled {}",
        h.state().scroll
    );
    assert!(h.state().world.bodies().iter().all(|b| !b.dragged));

    // off: a kiosk, the mouse moves nothing, and a click is still a click
    let mut h = dash(12, LayOut::default());
    until_still(&mut h, 600);
    h.state_mut().frames.hands = Hands::Off;
    let before = h.state().rects();
    let at = on_screen(&h, 4);
    mouse_drag(&mut h, at, vec2(0.0, 150.0));
    for _ in 0..30 {
        h.step();
    }
    let after = h.state().rects();
    for (i, (a, b)) in before.iter().zip(&after).enumerate() {
        assert!(
            moved(a, b) < 0.5,
            "panel {i} moved with hands off: {a:?} to {b:?}"
        );
    }
}

/// (a) A finger that moves a panel with [`Hands::All`] really moves it.
#[test]
fn with_all_hands_a_finger_moves_a_panel() {
    let mut h = dash(12, LayOut::default());
    until_still(&mut h, 600);
    let before = h.state().rect(4);
    let from = on_screen(&h, 4);
    touch(&mut h, TouchPhase::Start, from);
    for k in 1..=10 {
        touch(&mut h, TouchPhase::Move, from + vec2(0.0, 15.0 * k as f32));
    }
    let held = h.state().rect(4);
    assert!(
        moved(&before, &held) > 50.0,
        "the finger moved it {}",
        moved(&before, &held)
    );
    touch(&mut h, TouchPhase::End, from + vec2(0.0, 150.0));
    assert!(
        h.state().scroll < 1.0,
        "the page scrolled {}",
        h.state().scroll
    );
}

/// (b) New hints after a widget was added: the same ones change nothing and the world stays at rest; new
/// ones wake it, the body takes them and it rests again.
#[test]
fn new_hints_resize_a_panel_and_the_same_ones_wake_nothing() {
    let mut h = dash(6, LayOut::default());
    until_still(&mut h, 600);
    let steps = h.state().world.steps();
    let s = h.state_mut();
    let (id, same) = (s.slots[1], s.hints[1]);
    assert!(!s.frames.set_hints(id, same));
    assert!(!s.frames.set_title(id, "panel 1"));
    for _ in 0..30 {
        h.step();
        assert!(!h.state().shown.moving, "the same hints woke it");
    }
    assert_eq!(h.state().world.steps(), steps, "the world stepped");

    let before = h.state().rect(1);
    let s = h.state_mut();
    s.hints[1] = hints(150.0);
    assert!(s.frames.set_hints(id, hints(150.0)));
    assert!(s.frames.set_title(id, "narrow"));
    assert_eq!(s.frames.seat(id).map(|s| s.title.as_str()), Some("narrow"));
    h.step();
    assert!(h.state().shown.moving, "new hints wake the world");
    until_still(&mut h, 120);
    let after = h.state().rect(1);
    let narrower = before.size().x - after.size().x;
    assert!((narrower - 50.0).abs() < 2.0, "{before:?} to {after:?}");
}

/// (c) Laid out before it is shown: rows in reading order, what a row leaves over to a frame with a wide
/// form, and from the first frame on nothing changes size or moves.
#[test]
fn a_dash_laid_out_before_it_is_shown_opens_at_rest() {
    let h = dash(4, LayOut::default());
    let s = h.state();
    let rows: Vec<usize> = s.rows.iter().map(Vec::len).collect();
    assert_eq!(rows, [3, 1], "rows of {:?}", s.rects());
    // the second row has room for the wide form of its one frame
    assert_eq!(s.frames.pick(s.slots[3]), Some(Size::Wide));
    assert_eq!(s.frames.pick(s.slots[0]), Some(Size::Nom));
    // reading order: left to right, then the next row under it
    let r = s.rects();
    assert!(r[0].max.x < r[1].min.x && r[1].max.x < r[2].min.x);
    assert!(r[3].min.y > r[0].max.y);
    let first = r;
    let mut h = h;
    for _ in 0..120 {
        h.step();
    }
    for (i, (a, b)) in first.iter().zip(&h.state().rects()).enumerate() {
        assert!(
            moved(a, b) < 1.0,
            "panel {i} changed after opening: {a:?} to {b:?}"
        );
    }
    assert!(!h.state().shown.moving);

    // without widening, every frame keeps its normal size
    let h = dash(
        4,
        LayOut {
            widen: false,
            ..LayOut::default()
        },
    );
    assert_eq!(h.state().frames.pick(h.state().slots[3]), Some(Size::Nom));

    // columns: down, then the next one to the right
    let h = dash(
        4,
        LayOut {
            columns: true,
            ..LayOut::default()
        },
    );
    assert_eq!(
        h.state().rows.len(),
        1,
        "one column fits 4 in the page's height"
    );
}

/// (d) A live panel whose height changes by 20 points once a second for 30 s: each time the world wakes,
/// moves only what is under the changed panel, and rests within a second; nothing above it or beside it
/// moves.
#[test]
fn a_live_panel_moves_only_what_is_under_it_and_rests() {
    let mut h = dash(9, LayOut::default());
    until_still(&mut h, 600);
    assert_eq!(
        h.state().rows.iter().map(Vec::len).collect::<Vec<_>>(),
        [3, 3, 3]
    );
    let live = 4;
    let still = [0, 1, 2, 3, 5];
    let start = h.state().rects();
    let mut slowest = 0;
    let (mut stray, mut followed) = (0.0f32, 0.0f32);
    for second in 0..30 {
        h.state_mut().heights[live] = if second % 2 == 0 { 80.0 } else { 60.0 };
        let mut frames = 0;
        for f in 0..60 {
            h.step();
            for &i in &still {
                let m = moved(&start[i], &h.state().rect(i));
                assert!(
                    m < 0.5,
                    "second {second}: panel {i} moved {m} when panel {live} changed"
                );
            }
            for i in [6, 8] {
                stray = stray.max(moved(&start[i], &h.state().rect(i)));
            }
            followed = followed.max(moved(&start[7], &h.state().rect(7)));
            if frames == 0 && f > 0 && !h.state().shown.moving {
                frames = f;
            }
        }
        assert!(frames > 0, "second {second}: still moving after a second");
        slowest = slowest.max(frames);
        assert!(!h.state().shown.moving);
    }
    println!("rests within {slowest} frames; the panels beside the one under it moved {stray}");
    assert!(
        stray < 0.5,
        "the panels beside the one under it moved {stray}"
    );
    // the one under the live panel follows it (pushed down, it runs on up to 4 points past and comes back)
    assert!(
        (19.0..26.0).contains(&followed),
        "the one under it moved {followed}"
    );
    let under = h.state().rect(7);
    assert!(
        moved(&start[7], &under) < 0.5,
        "back where it was: {under:?}"
    );
}
