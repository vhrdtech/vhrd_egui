//! The sandbox driven like a person through egui_kittest (SETL-3): real `ve_frame` widgets on the bodies
//! of the world (SETL-7), in its two scenes, rectangles and hexagons. On every frame no two solid bodies
//! overlap and nothing jumps; after each change the world comes to rest. (The sandbox's first scenes of
//! plain boxes and tiles live on as headless worlds in `tests/scenes.rs`.)

#[path = "../examples/sandbox/app.rs"]
#[allow(dead_code)]
mod app;

use std::collections::HashMap;

use app::{Sandbox, Scene};
use egui::{Pos2, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use ve_frame::{FrameAction, Size, SizeMode, SlotId};
use ve_settle::frames::LandAt;
use ve_settle::{Body, BodyId, Poly, Rect};

/// Frames per second the tests run at.
const FPS: f32 = 60.0;
/// Everything has to be at rest this many frames after a change (16 s): a frame on auto may step through
/// its sizes one at a time, each step a change of its own, and a world that goes round in a cycle is
/// cooled to rest 15 s after the last change (`COOL_AFTER` + `COOL_SECS`).
const SETTLE_FRAMES: usize = 960;
/// The farthest a body's edge may move in one frame, points: beyond this it reads as a jump.
const MAX_FRAME_MOVE: f32 = 32.0;
/// The farthest a hexagon's edge may move in one frame, points.
const HEX_FRAME_MOVE: f32 = 40.0;
/// The deepest a hexagon may be found in a neighbour while things move, points (SETL-17: an outline gives
/// way by a glide per step at most, so pressed harder than that it dips in until the row has sorted itself
/// out). Rectangles never overlap, and at rest nothing does.
const OUTLINE_DIP: f32 = 16.0;

type App = Harness<'static, Sandbox>;

/// Steps the app frame by frame and checks the promises on each one.
struct Watch {
    seen: HashMap<BodyId, Rect>,
    /// The deepest a hexagon was found in a neighbour while things moved.
    worst_dip: f32,
}

impl Watch {
    fn look(&mut self, h: &App) {
        let world = &h.state().world;
        let title = |id: BodyId| {
            let frames = &h.state().frames;
            let slot = frames.slot(id).and_then(|s| frames.dock.get(s));
            slot.map(|s| s.widget.title()).unwrap_or_default()
        };
        // As the solver sees them: a hexagon by its outline, a rectangle by its rectangle.
        let shape = |b: &Body| match b.outline {
            Some(_) => b.shape(),
            None => Poly::rect(&b.rect),
        };
        for (i, a) in world.bodies().iter().enumerate() {
            for b in &world.bodies()[i + 1..] {
                if a.lifted || b.lifted {
                    continue;
                }
                if a.outline.is_none() && b.outline.is_none() {
                    let o = a.rect.overlap(&b.rect, 0.0);
                    assert!(
                        o.x.min(o.y) <= 0.01,
                        "two rectangles overlap at step {}: {} {:?}, {} {:?}",
                        world.steps(),
                        title(a.id),
                        a.rect,
                        title(b.id),
                        b.rect
                    );
                    continue;
                }
                let hit = ve_settle::sat(&shape(a), &shape(b), |_| 0.0, ve_settle::Vec2::ZERO);
                let depth = hit.map_or(0.0, |h| h.depth);
                assert!(
                    depth <= OUTLINE_DIP,
                    "a hexagon is {depth} points deep in its neighbour at step {}: {} {:?}, {} {:?}",
                    world.steps(),
                    title(a.id),
                    a.rect,
                    title(b.id),
                    b.rect
                );
                self.worst_dip = self.worst_dip.max(depth);
            }
        }
        let mut now = HashMap::new();
        for body in world.bodies() {
            if let (Some(before), false) = (self.seen.get(&body.id), body.dragged) {
                let moved = (body.rect.min - before.min)
                    .max_abs()
                    .max((body.rect.max - before.max).max_abs());
                // (a hexagon leaving a row that was made too full by hand is the one
                // thing that goes faster: 33 points were seen, SETL-17)
                let limit = match body.outline {
                    Some(_) => HEX_FRAME_MOVE,
                    None => MAX_FRAME_MOVE,
                };
                assert!(
                    moved <= limit,
                    "{} jumped {moved} points in one frame at step {}: {before:?} to {:?}",
                    title(body.id),
                    world.steps(),
                    body.rect
                );
            }
            now.insert(body.id, body.rect);
        }
        self.seen = now;
    }

    fn frame(&mut self, h: &mut App) {
        h.step();
        self.look(h);
    }

    fn frames(&mut self, h: &mut App, n: usize) {
        for _ in 0..n {
            self.frame(h);
        }
    }

    /// Frames until the world rests and stays so for half a second (the layout may still pick a size);
    /// panics if that takes too long.
    fn rest(&mut self, h: &mut App) -> usize {
        self.frames(h, 2);
        let mut calm = 0;
        for n in 2..SETTLE_FRAMES {
            calm = if h.state().world.is_at_rest() {
                calm + 1
            } else {
                0
            };
            if calm >= 30 {
                assert_eq!(h.state().world.max_overlap(), 0.0, "an overlap at rest");
                return n;
            }
            self.frame(h);
        }
        let world = &h.state().world;
        let moving: Vec<String> = world
            .bodies()
            .iter()
            .map(|b| {
                format!(
                    "#{} {:?} want {:?} vel {:?} lifted {} load {:?}",
                    b.id.0,
                    b.rect,
                    b.want,
                    b.vel,
                    b.lifted,
                    b.load()
                )
            })
            .collect();
        panic!(
            "not at rest {SETTLE_FRAMES} frames after the last change (step {}, last move {}): {moving:#?}",
            world.steps(),
            world.last_move()
        );
    }
}

/// The rectangles scene in a window before its first frame; `sideways` turns it to grow to the right.
fn fresh(sideways: bool) -> App {
    fresh_in(Scene::Rectangles, sideways)
}

/// `scene` in a window before its first frame; `sideways` turns it to grow to the right.
fn fresh_in(scene: Scene, sideways: bool) -> App {
    let h = Harness::builder()
        .with_size(vec2(1250.0, 800.0))
        .with_step_dt(1.0 / FPS)
        .build_eframe(move |cc| {
            let mut sandbox = Sandbox::with_scene(cc, scene);
            sandbox.sideways = sideways;
            sandbox.reset();
            sandbox
        });
    // as on a desktop: the glow fades, a window hops, a frame folds over a few frames
    h.ctx.all_styles_mut(|s| s.animation_time = 1.0 / 12.0);
    h
}

/// The rectangles scene in a window, settled.
fn sandbox() -> (App, Watch) {
    sandbox_in(Scene::Rectangles)
}

/// `scene` in a window, settled.
fn sandbox_in(scene: Scene) -> (App, Watch) {
    let mut h = fresh_in(scene, false);
    h.step();
    let mut watch = Watch {
        seen: HashMap::new(),
        worst_dip: 0.0,
    };
    watch.look(&h);
    watch.rest(&mut h);
    (h, watch)
}

/// An empty scene with the demo widgets `kinds` of [`app::mixed`] added in a row.
fn sandbox_of(kinds: &[u32]) -> (App, Watch, Vec<SlotId>) {
    let (mut h, mut watch) = sandbox();
    h.state_mut().clear();
    watch.frame(&mut h);
    let mut slots = Vec::new();
    for (i, kind) in kinds.iter().enumerate() {
        let s = h.state_mut();
        let at = s.arena.min + vec2(200.0 + 420.0 * i as f32, 150.0);
        let demo = app::mixed(*kind);
        let seat = app::seat(&demo);
        let at = ve_settle::Vec2::new(at.x, at.y);
        slots.push(s.frames.add(&mut s.world, demo, seat, at));
    }
    watch.rest(&mut h);
    (h, watch, slots)
}

fn click_at(h: &mut App, watch: &mut Watch, at: Pos2) {
    h.hover_at(at);
    watch.frame(h);
    h.drag_at(at);
    watch.frame(h);
    h.drop_at(at);
    watch.frame(h);
}

fn body_rect(h: &App, slot: SlotId) -> Rect {
    let s = h.state();
    let body = s.frames.body(slot).expect("the widget has a body");
    s.world.body(body).expect("the body is in the world").rect
}

fn frame_of(h: &App, slot: SlotId) -> ve_frame::FrameState {
    h.state().frames.dock.get(slot).expect("the widget").frame
}

/// The first widget of the scene that `pick` accepts.
fn find(h: &App, pick: impl Fn(&app::playground::Demo) -> bool) -> SlotId {
    let slots = &h.state().frames.dock;
    slots
        .iter()
        .find(|s| pick(&s.widget))
        .map(|s| s.id)
        .expect("such a widget")
}

/// Click a frame's size switch.
fn click_switch(h: &mut App, watch: &mut Watch, slot: SlotId) {
    let at = frame_of(h, slot).switch_pos().expect("a size switch");
    click_at(h, watch, at);
}

#[test]
fn each_scene_fills_the_window_runs_past_the_bottom_and_rests() {
    for scene in Scene::ALL {
        let (mut h, mut watch) = sandbox_in(scene);
        let s = h.state();
        assert!(
            s.world.bodies().len() >= 12,
            "{} widgets",
            s.world.bodies().len()
        );
        let lowest = s.world.bodies().iter().map(|b| b.rect.max.y);
        let lowest = lowest.fold(0.0, f32::max);
        assert!(
            lowest > s.arena.max.y + 100.0,
            "runs past the bottom: {lowest}"
        );
        let (mut hexes, mut rects) = (0, 0);
        for slot in s.frames.dock.iter() {
            let body = s.frames.body(slot.id).and_then(|b| s.world.body(b));
            let body = body.expect("every widget of the scene has a body");
            // the frame is drawn on its body: same place, same size
            let drawn = slot.frame.drawn_rect().expect("drawn");
            let on_screen = s.screen_rect(body.rect);
            assert!(
                (drawn.min - on_screen.min).length() < 1.0
                    && (drawn.size() - on_screen.size()).length() < 1.5,
                "{}: drawn {drawn:?}, body {on_screen:?}",
                slot.widget.title()
            );
            // the shape of the frame is the outline of the body
            match (slot.widget.opts.hex, &body.outline) {
                (true, Some(outline)) => {
                    assert_eq!(outline.points().len(), 6);
                    hexes += 1;
                }
                (false, None) => rects += 1,
                (hex, outline) => panic!("hexagon {hex} with outline {outline:?}"),
            }
            assert!(body.tension() < 0.001, "nobody is squeezed at rest");
        }
        match scene {
            Scene::Rectangles => assert!(
                hexes == 0 && rects >= 12,
                "{hexes} hexagons, {rects} rectangles"
            ),
            Scene::Hexagons => assert!(
                rects == 0 && hexes >= 12,
                "{hexes} hexagons, {rects} rectangles"
            ),
        }
        // the frames in view feed the backdrop
        assert!(!s.lit.is_empty() && s.lit.len() <= ve_backdrop::MAX_LIT);
        // at rest nothing moves any more
        let before: Vec<Rect> = s.world.bodies().iter().map(|b| b.rect).collect();
        let steps = s.world.steps();
        watch.frames(&mut h, 60);
        let after: Vec<Rect> = h.state().world.bodies().iter().map(|b| b.rect).collect();
        assert_eq!(before, after);
        assert_eq!(
            h.state().world.steps(),
            steps,
            "a world at rest runs no steps"
        );
    }
}

#[test]
fn the_size_switch_changes_the_body_and_the_neighbours_make_room() {
    let (mut h, mut watch) = sandbox();
    // a rectangle with a size switch that has neighbours below it
    let slot = find(&h, |d| {
        !d.opts.hex && d.opts.sizes && d.title() == "medium LISMFX"
    });
    let hints = app::playground::Scale::Medium.hints();
    let others = |h: &App| -> Vec<(BodyId, Rect)> {
        let world = &h.state().world;
        world.bodies().iter().map(|b| (b.id, b.rect)).collect()
    };
    let auto = body_rect(&h, slot).size();
    let start = others(&h);

    // min: the fixed smallest form, a body just around it
    click_switch(&mut h, &mut watch, slot);
    assert_eq!(frame_of(&h, slot).size, SizeMode::Min);
    watch.rest(&mut h);
    let min = body_rect(&h, slot).size();
    assert_eq!(frame_of(&h, slot).shown_size(), Some(Size::Min));
    assert!(
        min.y < auto.y - 30.0 && min.y < hints.min.y + 100.0,
        "min {min:?}, auto {auto:?}"
    );
    let moved = others(&h);
    let closed_up = start.iter().zip(&moved).filter(|(a, b)| a.1 != b.1).count();
    assert!(closed_up >= 2, "the others close up: {closed_up} moved");

    // nom, then wide: the body grows and the others make room
    click_switch(&mut h, &mut watch, slot);
    watch.rest(&mut h);
    let nom = body_rect(&h, slot).size();
    assert!(nom.y > min.y + 30.0, "nom {nom:?}, min {min:?}");
    click_switch(&mut h, &mut watch, slot);
    assert_eq!(frame_of(&h, slot).size, SizeMode::Wide);
    watch.rest(&mut h);
    let wide = body_rect(&h, slot).size();
    assert!(
        wide.x > nom.x + 100.0 && wide.x >= hints.wide.x,
        "wide {wide:?}, nom {nom:?}"
    );
    // the frame is as large as its body in every form
    let drawn = frame_of(&h, slot).drawn_rect().expect("drawn").size();
    assert!((drawn.x - wide.x).abs() < 1.5 && (drawn.y - wide.y).abs() < 1.5);
}

#[test]
fn a_hexagons_body_and_outline_follow_its_size() {
    let (mut h, mut watch) = sandbox_in(Scene::Hexagons);
    let hex = find(&h, |d| {
        d.opts.hex && d.opts.sizes && d.title().starts_with("medium")
    });
    let before = body_rect(&h, hex).size();
    click_switch(&mut h, &mut watch, hex);
    watch.rest(&mut h);
    let small = body_rect(&h, hex).size();
    assert!(
        // (vertex attraction may stretch it by up to its range, SETL-20)
        small.x < before.x - 25.0 && small.y < before.y - 20.0,
        "hexagon on min {small:?}, before {before:?}"
    );
    let s = h.state();
    let body = s
        .frames
        .body(hex)
        .and_then(|b| s.world.body(b))
        .expect("body");
    assert!(body.shape().is_convex() && body.outline.is_some());
}

#[test]
fn popping_out_and_back_keeps_the_world_calm() {
    let (mut h, mut watch) = sandbox();
    let slot = find(&h, |d| d.opts.menu && d.title() == "medium LISMFX");
    let bodies = h.state().world.bodies().len();
    let home = body_rect(&h, slot);

    // through the menu, as a person does
    let menu = frame_of(&h, slot).menu_rect().expect("a menu").center();
    click_at(&mut h, &mut watch, menu);
    h.get_by_label("Pop out as a window").click();
    watch.frames(&mut h, 3);
    let frame = frame_of(&h, slot);
    assert!(frame.placement.floating);
    // the body left the world; the window opened where the widget sat
    assert_eq!(h.state().frames.body(slot), None);
    assert_eq!(h.state().world.bodies().len(), bodies - 1);
    let window = frame.window_pos.expect("a window position");
    let sat = h.state().screen_rect(home).min;
    assert!((pos2(window[0], window[1]) - sat).length() < 2.0);
    watch.rest(&mut h);
    // it is fed to the backdrop as the floating one, first
    assert!(h.state().lit.first().is_some_and(|l| l.floating));

    // back: the body returns where it sat, the others part for it, the frame glows
    h.state_mut().apply(slot, FrameAction::DockBack);
    watch.frames(&mut h, 3);
    assert!(h.state().frames.body(slot).is_some());
    assert_eq!(h.state().world.bodies().len(), bodies);
    assert!(frame_of(&h, slot).glow() > 0.0, "it glows where it landed");
    let back = body_rect(&h, slot);
    assert!(
        (back.center() - home.center()).max_abs() < 40.0,
        "{back:?} vs {home:?}"
    );
    watch.rest(&mut h);
    assert_eq!(frame_of(&h, slot).glow(), 0.0);
    let rested = body_rect(&h, slot);
    assert!(
        (rested.center().x - home.center().x).abs() < 60.0,
        "{rested:?} vs {home:?}"
    );
}

#[test]
fn a_ghost_keeps_the_place_of_a_widget_that_floats() {
    let (mut h, mut watch) = sandbox_in(Scene::Hexagons);
    h.state_mut().frames.ghosts = true;
    let slot = find(&h, |d| d.opts.hex && d.opts.menu);
    let (bodies, home) = (h.state().world.bodies().len(), body_rect(&h, slot));
    h.state_mut().apply(slot, FrameAction::PopOut);
    watch.rest(&mut h);
    assert!(frame_of(&h, slot).placement.floating && h.state().frames.is_ghost(slot));
    // nothing moved: the body is still there
    assert_eq!(h.state().world.bodies().len(), bodies);
    assert_eq!(body_rect(&h, slot), home);
    // a click on the ghost puts the widget back; the window was over it, so move the window away first
    h.state_mut()
        .frames
        .dock
        .get_mut(slot)
        .expect("the widget")
        .frame
        .window_pos = Some([20.0, 500.0]);
    watch.frames(&mut h, 2);
    let at = h.state().screen_rect(home).center();
    click_at(&mut h, &mut watch, at);
    watch.frames(&mut h, 2);
    assert!(!frame_of(&h, slot).placement.floating);
    assert!(!h.state().frames.is_ghost(slot));
    watch.rest(&mut h);
    let back = body_rect(&h, slot);
    assert!(
        (back.min - home.min).max_abs() < 0.01 && (back.max - home.max).max_abs() < 0.01,
        "{back:?} vs {home:?}"
    );
}

#[test]
fn a_widget_put_back_out_of_view_is_followed_or_lands_where_its_window_is() {
    let (mut h, mut watch) = sandbox();
    // a widget far below the window
    let slot = {
        let s = h.state();
        let lowest = s
            .frames
            .dock
            .iter()
            .filter(|w| w.widget.opts.menu)
            .max_by(|a, b| {
                let y = |w: &ve_frame::Slot<_>| body_rect(&h, w.id).min.y;
                y(a).total_cmp(&y(b))
            });
        lowest.expect("a widget").id
    };
    let home = body_rect(&h, slot);
    assert!(home.min.y > h.state().arena.max.y, "it starts out of view");
    h.state_mut().land = ve_frame::LandMode::Follow;
    h.state_mut().apply(slot, FrameAction::PopOut);
    watch.rest(&mut h);
    assert_eq!(h.state().scroll, 0.0);
    h.state_mut().apply(slot, FrameAction::DockBack);
    watch.rest(&mut h);
    // the view went after it
    let shown = h.state().screen_rect(body_rect(&h, slot));
    assert!(h.state().scroll > 100.0, "scrolled {}", h.state().scroll);
    assert!(h.state().arena.contains(shown.center()), "{shown:?}");

    // the other way: it lands where its window is
    h.state_mut().frames.land_at = LandAt::Window;
    h.state_mut().apply(slot, FrameAction::PopOut);
    watch.frames(&mut h, 3);
    let arena = h.state().arena;
    let to = arena.min + vec2(40.0, 60.0);
    h.state_mut()
        .frames
        .dock
        .get_mut(slot)
        .expect("the widget")
        .frame
        .window_pos = Some([to.x, to.y]);
    watch.rest(&mut h);
    let window = frame_of(&h, slot).drawn_rect().expect("drawn");
    let at = h.state().world_pos(window.center());
    h.state_mut().apply(slot, FrameAction::DockBack);
    watch.frames(&mut h, 3);
    let landed = body_rect(&h, slot).center();
    assert!(
        (landed - at).max_abs() < 60.0,
        "landed {landed:?}, window {at:?}"
    );
    watch.rest(&mut h);
}

#[test]
fn a_folded_frame_is_a_body_as_small_as_its_header() {
    for hex in [false, true] {
        let scene = if hex {
            Scene::Hexagons
        } else {
            Scene::Rectangles
        };
        let (mut h, mut watch) = sandbox_in(scene);
        let slot = find(&h, |d| {
            d.opts.hex == hex && d.title().starts_with("medium LISM")
        });
        let open = body_rect(&h, slot);
        // a double click on the bare header, between the title and the controls
        let header = h.state().screen_rect(open);
        let at = pos2(header.center().x + 10.0, header.min.y + 13.0);
        h.hover_at(at);
        watch.frame(&mut h);
        for _ in 0..2 {
            h.drag_at(at);
            h.drop_at(at);
        }
        watch.frames(&mut h, 2);
        assert!(frame_of(&h, slot).collapsed, "hexagon {hex}: folded");
        watch.rest(&mut h);
        let folded = body_rect(&h, slot);
        assert!(
            folded.size().y < 32.0 && (folded.size().x - open.size().x).abs() < 1.5,
            "hexagon {hex}: folded {folded:?}, open {open:?}"
        );
        if hex {
            // the band that still shows: four corners, wider at the cut
            let s = h.state();
            let body = s
                .frames
                .body(slot)
                .and_then(|b| s.world.body(b))
                .expect("body");
            assert_eq!(body.outline.expect("an outline").points().len(), 4);
        }
        // unfolded it takes the size it had
        h.state_mut()
            .frames
            .dock
            .get_mut(slot)
            .expect("the widget")
            .frame
            .collapsed = false;
        watch.rest(&mut h);
        let again = body_rect(&h, slot).size();
        // (vertex attraction may have lined its side up with a neighbour's before)
        assert!(
            (again.y - open.size().y).abs() < 1.5 && (again.x - open.size().x).abs() < 17.0,
            "{again:?} vs {open:?}"
        );
    }
}

#[test]
fn the_layout_picks_the_size_of_a_frame_on_auto_from_the_room_in_its_row() {
    // one rectangle and one hexagon, alone in a wide row
    let (mut h, mut watch, slots) = sandbox_of(&[0, 2]);
    // off by default (round 4: a busy world never settled)
    h.state_mut().frames.auto_pick = true;
    h.set_size(vec2(1900.0, 800.0));
    watch.rest(&mut h);
    let hints = app::playground::Scale::Medium.hints();
    for slot in &slots {
        // room for the wide form: the layout steps up to it
        assert_eq!(frame_of(&h, *slot).size, SizeMode::Auto);
        assert_eq!(h.state().frames.pick(*slot), Some(Size::Wide));
        assert_eq!(frame_of(&h, *slot).shown_size(), Some(Size::Wide));
        assert!(body_rect(&h, *slot).size().x >= hints.wide.x);
    }
    // the window gets too narrow for both: pressed, they step down and stay there
    h.set_size(vec2(1000.0, 800.0));
    watch.rest(&mut h);
    let picks = |h: &App| -> Vec<Size> {
        let frames = &h.state().frames;
        slots.iter().filter_map(|s| frames.pick(*s)).collect()
    };
    let narrow = picks(&h);
    assert!(narrow.iter().any(|p| *p != Size::Wide), "{narrow:?}");
    let arena = h.state().arena;
    for slot in &slots {
        let r = body_rect(&h, *slot);
        assert!(
            r.min.x >= arena.min.x - 0.5 && r.max.x <= arena.max.x + 0.5,
            "{r:?} in {arena:?}"
        );
    }
    watch.frames(&mut h, 120);
    assert_eq!(picks(&h), narrow, "the picks hold");
    // off: the picks stay as they are, whatever the room
    h.state_mut().frames.auto_pick = false;
    h.set_size(vec2(1600.0, 800.0));
    watch.rest(&mut h);
    assert_eq!(picks(&h), narrow);
}

#[test]
fn dragging_a_frame_moves_its_body_and_the_others_flow_around() {
    let (mut h, mut watch) = sandbox();
    let slot = find(&h, |d| {
        !d.opts.hex && d.opts.footer && d.title() == "medium LISMFX"
    });
    let from_rect = h.state().screen_rect(body_rect(&h, slot));
    // hold it by its footer: no control there
    let from = pos2(from_rect.center().x, from_rect.max.y - 8.0);
    let to = from + vec2(360.0, 260.0);
    h.hover_at(from);
    watch.frame(&mut h);
    h.drag_at(from);
    watch.frame(&mut h);
    for n in 1..=40 {
        h.hover_at(from + (to - from) * (n as f32 / 40.0));
        watch.frame(&mut h);
    }
    let held = h.state().screen_rect(body_rect(&h, slot));
    assert!(
        // (a frame behind: the world steps before the pointer is read)
        (held.center() - (from_rect.center() + (to - from))).length() < 14.0,
        "the body follows the pointer: {held:?}"
    );
    h.drop_at(to);
    watch.rest(&mut h);
    // dropped, it settles near there, under gravity
    let dropped = h.state().screen_rect(body_rect(&h, slot));
    assert!(
        (dropped.center().x - held.center().x).abs() < 200.0,
        "{dropped:?} vs {held:?}"
    );
    // its button still clicks: a control is not a grip
    let clicks = |h: &App| {
        h.state()
            .frames
            .dock
            .get(slot)
            .expect("widget")
            .widget
            .clicks
    };
    assert_eq!(clicks(&h), 0);
    let plus = pos2(dropped.min.x + 14.0, dropped.min.y + 26.0 + 16.0);
    let before = body_rect(&h, slot);
    click_at(&mut h, &mut watch, plus);
    watch.frames(&mut h, 2);
    assert_eq!(clicks(&h), 1, "the + in the left side clicked");
    let after = body_rect(&h, slot);
    assert!(
        (after.center() - before.center()).max_abs() < 1.0,
        "and did not move the body: {before:?} to {after:?}"
    );
}

/// Item 1 of round 2: the scene is measured and laid out before it is shown, so frames open at a real
/// size instead of a guess, and keep it, in every tab (each one a new scene after the last).
#[test]
fn the_scene_opens_at_its_sizes() {
    for tab in Scene::ALL {
        let mut h = fresh(false);
        h.state_mut().scene = tab;
        h.state_mut().reset();
        // the first frame only learns the window's size; the second seeds and shows
        h.step();
        h.step();
        let at_start = |h: &App| -> Vec<(SlotId, Rect, Option<Size>)> {
            let s = h.state();
            let frames = s.frames.dock.iter();
            frames
                .map(|w| (w.id, body_rect(h, w.id), s.frames.pick(w.id)))
                .collect()
        };
        let first = at_start(&h);
        assert!(first.len() >= 12, "{tab:?}: {} frames", first.len());
        let mut watch = Watch {
            seen: HashMap::new(),
            worst_dip: 0.0,
        };
        watch.look(&h);
        let frames = watch.rest(&mut h);
        let rested = at_start(&h);
        let mut farthest = 0.0f32;
        let mut repicked = 0;
        let mut late = Vec::new();
        for ((id, a, pick_a), (_, b, pick_b)) in first.iter().zip(&rested) {
            // (where nesting or a neighbour's width made room afterwards, the layout may still widen
            // a frame or two)
            if pick_a != pick_b {
                repicked += 1;
                let title = h.state().frames.dock.get(*id).map(|w| w.widget.title());
                late.push(format!("{title:?} picked {pick_a:?}, then {pick_b:?}"));
                continue;
            }
            // (vertex attraction may still line each side up with a neighbour's: its range, twice)
            // (and a hexagon's height too, SETL-20)
            let hex = h
                .state()
                .frames
                .dock
                .get(*id)
                .is_some_and(|w| w.widget.opts.hex);
            let tall = if hex { 33.0 } else { 1.5 };
            let same =
                (a.size().y - b.size().y).abs() < tall && (a.size().x - b.size().x).abs() < 33.0;
            if !same {
                let title = h.state().frames.dock.get(*id).map(|w| w.widget.title());
                late.push(format!(
                    "{title:?} opened {:?}, ended {:?}",
                    a.size(),
                    b.size()
                ));
                continue;
            }
            farthest = farthest.max((a.center() - b.center()).max_abs());
        }
        // Every frame opens at the size it keeps. (Until 11 Oct 2026 7 of 21 in the rectangles tab
        // grew after opening: a new scene's widgets got the ids of the last one's and their frames
        // started from what those had learned of their side columns.)
        let changed = late.len();
        println!(
            "{tab:?}: {changed} of {} frames changed size after opening, the farthest moved {farthest}, \
             at rest after {frames} frames",
            first.len()
        );
        assert_eq!(
            changed, 0,
            "{tab:?}: {repicked} repicked; these changed after opening: {late:#?}"
        );
        // Rectangles settle a row's gap down at most; hexagons still pack into each other after
        // opening (laid out as rectangles, SETL-18).
        if tab != Scene::Hexagons {
            assert!(
                farthest <= 80.0,
                "{tab:?}: a frame moved {farthest} points after opening"
            );
        }
        assert!(frames < 400, "{tab:?}: at rest after {frames} frames");
    }
}

/// Parameters anywhere in the ranges of the sandbox's sliders; gravity's side and the walls stay.
fn random_params(seed: &mut u32, base: &ve_settle::Params) -> ve_settle::Params {
    let mut next = || {
        *seed ^= *seed << 13;
        *seed ^= *seed >> 17;
        *seed ^= *seed << 5;
        (*seed % 10_000) as f32 / 10_000.0
    };
    let mut pick = |lo: f32, hi: f32| lo + (hi - lo) * next();
    ve_settle::Params {
        gravity: pick(0.0, 3000.0),
        home_stiffness: pick(0.0, 300.0),
        home_drift: pick(0.0, 5.0),
        damping: pick(0.0, 40.0),
        gap: pick(0.0, 24.0),
        tension: pick(0.0, 1.0),
        restore: pick(0.0, 10.0),
        grow_speed: pick(20.0, 2000.0),
        glide_speed: pick(50.0, 3000.0),
        snap_range: pick(0.0, 80.0),
        snap_stiffness: pick(0.0, 6000.0),
        attract_range: pick(0.0, 80.0),
        attract_strength: pick(0.0, 60.0),
        friction: pick(0.0, 4.0),
        bulge_rest: pick(0.0, 20.0),
        bulge_from: pick(0.0, 20.0),
        bulge_gain: pick(0.0, 5.0),
        bulge_max: pick(0.0, 60.0),
        bulge_speed: pick(10.0, 1000.0),
        grid_pitch: pick(4.0, 96.0),
        grid_stiffness: pick(0.0, 3000.0),
        max_speed: pick(100.0, 6000.0),
        iterations: 1 + pick(0.0, 63.0) as u32,
        dt: 1.0 / pick(30.0, 480.0),
        drag: pick(0.0, 0.5),
        ..*base
    }
}

/// Item 6 of round 2: play with every slider, then press "Default parameters". Whatever the parameters
/// are, no two frames share a middle and nothing is past a hard edge (gravity's side and the two sides
/// that do not scroll); the content the scroll area shows reaches as far as the world; and with the
/// defaults back the scene rests in time with nothing overlapping.
fn any_parameters_then_default_parameters(sideways: bool) {
    let mut h = fresh(sideways);
    h.step();
    let mut watch = Watch {
        seen: HashMap::new(),
        worst_dip: 0.0,
    };
    watch.look(&h);
    watch.rest(&mut h);
    let defaults = h.state().defaults();
    let holds = |h: &App, when: &str| {
        let s = h.state();
        let world = &s.world;
        assert_eq!(
            world.max_escape(),
            0.0,
            "{when}: a frame is past a hard edge"
        );
        assert!(
            world.max_core_overlap() <= 1e-3,
            "{when}: two frames share a middle ({})",
            world.max_core_overlap()
        );
        // the open side grows with the world, and the scroll area's content with it
        let far = |pick: fn(&Rect) -> f32| {
            world
                .bodies()
                .iter()
                .map(|b| pick(&b.rect))
                .fold(0.0, f32::max)
        };
        let (right, bottom) = (far(|r| r.max.x), far(|r| r.max.y));
        assert!(
            right.is_finite() && bottom.is_finite(),
            "{when}: a frame is nowhere"
        );
        if sideways {
            assert!(
                s.content_width() >= right - s.arena.min.x,
                "{when}: content ends before the world"
            );
            assert!(
                bottom <= s.arena.max.y + 0.5,
                "{when}: the bottom is a hard edge"
            );
        } else {
            assert!(
                s.content_height() >= bottom - s.arena.min.y,
                "{when}: content ends before the world"
            );
            assert!(
                right <= s.arena.max.x + 0.5,
                "{when}: the right side is a hard edge"
            );
        }
    };
    let mut seed = 0x2545_f491;
    for round in 0..8 {
        let wild = random_params(&mut seed, &defaults);
        h.state_mut().world.set_params(wild);
        for frame in 0..40 {
            h.step();
            holds(&h, &format!("round {round} frame {frame}"));
        }
    }
    h.get_by_label("Default parameters").click();
    h.step();
    h.step();
    assert_eq!(
        *h.state().world.params(),
        defaults,
        "back at the scene's defaults"
    );
    let mut calm = 0;
    let mut rested = None;
    for frame in 0..SETTLE_FRAMES {
        h.step();
        holds(&h, &format!("after the reset, frame {frame}"));
        calm = if h.state().world.is_at_rest() {
            calm + 1
        } else {
            0
        };
        if calm >= 30 {
            rested = Some(frame);
            break;
        }
    }
    assert!(
        rested.is_some(),
        "not at rest {SETTLE_FRAMES} frames after Default parameters"
    );
    assert_eq!(h.state().world.max_overlap(), 0.0, "an overlap at rest");
    assert!(h.state().world.bodies().iter().all(|b| !b.lifted));
    // the world runs past the window on its open side, and the view scrolls that far
    let s = h.state();
    if sideways {
        assert!(
            s.content_width() > s.arena.width() + 50.0,
            "{}",
            s.content_width()
        );
    } else {
        assert!(
            s.content_height() > s.arena.height() + 50.0,
            "{}",
            s.content_height()
        );
    }
}

#[test]
fn any_parameters_then_default_parameters_in_a_scene_that_scrolls_down() {
    any_parameters_then_default_parameters(false);
}

#[test]
fn any_parameters_then_default_parameters_in_a_scene_that_scrolls_sideways() {
    any_parameters_then_default_parameters(true);
}

/// Items 3, 4 and 5 of round 2: a tab per shape, frame times in the panel, a slider that shows what it
/// does in the world.
#[test]
fn tabs_speed_and_the_effect_of_a_hovered_slider() {
    let (mut h, mut watch) = sandbox();
    for (label, tab, hex) in [
        ("Hexagons", Scene::Hexagons, true),
        ("Rectangles", Scene::Rectangles, false),
    ] {
        h.get_by_label(label).click();
        h.step();
        h.step();
        h.step();
        watch.seen.clear();
        watch.rest(&mut h);
        let s = h.state();
        assert_eq!(s.scene, tab);
        assert!(
            s.frames.dock.len() >= 10,
            "{label}: {} frames",
            s.frames.dock.len()
        );
        assert!(
            s.frames.dock.iter().all(|w| w.widget.opts.hex == hex),
            "{label} only"
        );
    }
    // frame times of the frames just drawn
    let stats = h.state().perf.stats().expect("frames were counted");
    assert!(stats.frames > 30 && stats.fps > 0.0);
    assert!(stats.min <= stats.p50 && stats.p50 <= stats.p95 && stats.p95 <= stats.p99);
    assert!(stats.p99 <= stats.max && stats.solver_p50 <= stats.solver_max);
    assert_eq!(app::percentile(&[1.0, 2.0, 3.0, 4.0], 0.5), 2.0);
    assert_eq!(app::percentile(&[1.0, 2.0, 3.0, 4.0], 0.99), 4.0);
    assert_eq!(app::percentile(&[], 0.5), 0.0);
    // every knob has something to show
    for name in [
        "gravity",
        "home stiffness",
        "damping",
        "gap",
        "tension",
        "snap range",
    ] {
        assert!(app::Effect::of(name).is_some(), "{name}");
    }
    for name in [
        "attract range",
        "friction",
        "bulge gain",
        "grid pitch",
        "glide speed",
        "iterations",
        "rest threshold",
        "rest steps",
    ] {
        assert!(app::Effect::of(name).is_some(), "{name}");
    }
    // each effect draws (kept on with the pin, as after hovering its slider)
    h.state_mut().pin_effect = true;
    use app::Effect as E;
    for effect in [
        E::Gravity,
        E::Home,
        E::Motion,
        E::Gap,
        E::Pressure,
        E::Snap,
        E::Attract,
        E::Friction,
        E::Grid,
        E::Contacts,
        E::Rest,
    ] {
        assert!(!effect.what().is_empty());
        h.state_mut().effect = Some(effect);
        h.step();
        assert_eq!(h.state().effect, Some(effect), "pinned, it stays");
    }
    h.state_mut().pin_effect = false;
    h.step();
    assert_eq!(
        h.state().effect,
        None,
        "without the pin it goes with the pointer"
    );
    // a real hover: the pointer on a slider shows its effect, off it the effect goes (a window tall enough
    // for the whole panel: the sliders are below the fold of the usual one)
    h.set_size(egui::vec2(1250.0, 2400.0));
    h.step();
    h.step();
    for (name, effect) in [
        ("gravity", E::Gravity),
        ("gap", E::Gap),
        ("attract range", E::Attract),
        ("iterations", E::Contacts),
        ("rest steps", E::Rest),
    ] {
        let slider = h.get_by_role_and_label(egui::accesskit::Role::Slider, name);
        assert!(
            slider.rect().max.y < 2400.0,
            "{name} in view: {:?}",
            slider.rect()
        );
        slider.hover();
        h.step();
        h.step();
        assert_eq!(h.state().effect, Some(effect), "hovering {name}");
    }
    let empty = h.state().arena.left_top() + egui::vec2(2.0, 2.0);
    h.hover_at(empty);
    h.step();
    h.step();
    assert_eq!(h.state().effect, None, "the pointer left the sliders");
}

/// Round 4 (user, 12 Oct 2026): one sandbox, its scenes the tabs above the arena. Plain `cargo run`
/// starts the rectangles; it fills the window from the first frame and more as the window grows.
#[test]
fn the_sandbox_starts_with_rectangles_and_fills_the_window_as_it_grows() {
    let mut h: App = Harness::builder()
        .with_size(vec2(400.0, 200.0))
        .with_step_dt(1.0 / FPS)
        .build_eframe(|cc| Sandbox::new(cc));
    h.step();
    assert_eq!(h.state().scene, Scene::Rectangles);
    assert_eq!(h.state().world.bodies().len(), 0, "not seeded while tiny");
    h.set_size(vec2(1250.0, 800.0));
    h.step();
    h.step();
    let n = h.state().count();
    let lowest = |h: &App| {
        let world = &h.state().world;
        world
            .bodies()
            .iter()
            .map(|b| b.rect.max.y)
            .fold(0.0, f32::max)
    };
    assert!(n >= 12, "after sizing up: {n} widgets");
    assert!(lowest(&h) > h.state().arena.max.y, "past the bottom");
    h.set_size(vec2(1900.0, 1100.0));
    h.step();
    h.step();
    let more = h.state().count();
    assert!(more > n, "a bigger window gets more: {more}");
    assert!(h.state().frames.dock.iter().all(|w| !w.widget.opts.hex));
}

/// The wheel scrolls the arena down to the last widgets; the world does not move.
#[test]
fn scrolling_shows_the_bottom_and_moves_nothing() {
    let (mut h, mut watch) = sandbox();
    let arena = h.state().arena;
    let frozen = h.state().world.bodies().to_vec();
    h.hover_at(arena.center());
    h.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: vec2(0.0, -4000.0),
        modifiers: egui::Modifiers::NONE,
        phase: egui::TouchPhase::Move,
    });
    watch.frames(&mut h, 30);
    let s = h.state();
    assert!(s.scroll > 100.0, "scrolled {}", s.scroll);
    assert_eq!(s.world.bodies(), &frozen[..], "scrolling moves nothing");
}

/// Add widget and a double click on empty arena add a frame; Remove selected takes the one clicked.
#[test]
fn adding_and_removing_widgets() {
    let (mut h, mut watch) = sandbox();
    let n = h.state().world.bodies().len();
    h.get_by_label("Add widget").click();
    watch.seen.clear();
    watch.rest(&mut h);
    assert_eq!(h.state().world.bodies().len(), n + 1);
    assert!(h.state().world.bodies().iter().all(|b| !b.lifted));

    // a click on a frame selects it, and the button removes it
    let slot = find(&h, |d| d.opts.footer && d.title() == "medium LISMFX");
    let rect = h.state().screen_rect(body_rect(&h, slot));
    click_at(&mut h, &mut watch, pos2(rect.center().x, rect.max.y - 8.0));
    let body = h.state().frames.body(slot);
    assert!(body.is_some() && h.state().selected == body);
    h.get_by_label("Remove selected").click();
    watch.rest(&mut h);
    assert_eq!(h.state().world.bodies().len(), n);
    assert!(h.state().frames.dock.get(slot).is_none());
    assert_eq!(h.state().selected, None);

    // an empty arena takes a double click
    h.get_by_label("Clear scene").click();
    h.step();
    watch.seen.clear();
    assert!(h.state().world.bodies().is_empty());
    let spot = h.state().arena.center();
    h.hover_at(spot);
    // (half a second after the button: else egui counts a triple click)
    for _ in 0..40 {
        h.step();
    }
    for _ in 0..2 {
        h.drag_at(spot);
        h.step();
        h.drop_at(spot);
        h.step();
    }
    watch.rest(&mut h);
    assert_eq!(h.state().world.bodies().len(), 1);
    // and Reset seeds the scene again
    h.get_by_label("Reset").click();
    h.step();
    h.step();
    watch.seen.clear();
    watch.rest(&mut h);
    assert!(h.state().world.bodies().len() >= 12);
}

/// Pause stops the world; Step runs one fixed step; Resume lets it settle.
#[test]
fn pause_stops_the_world_and_step_runs_one_step() {
    let (mut h, mut watch) = sandbox();
    h.get_by_label("Pause").click();
    watch.frame(&mut h);
    h.get_by_label("Add widget").click();
    watch.frames(&mut h, 2);
    let steps = h.state().world.steps();
    watch.frames(&mut h, 10);
    assert_eq!(h.state().world.steps(), steps, "paused: no steps");
    for n in 1..=3 {
        h.get_by_label("Step").click();
        watch.frame(&mut h);
        assert_eq!(h.state().world.steps(), steps + n);
    }
    h.get_by_label("Resume").click();
    watch.rest(&mut h);
    assert!(h.state().world.steps() > steps + 3);
}

/// A narrower and a wider window: every frame stays between the side walls and the world rests.
#[test]
fn resizing_the_window_keeps_everything_between_the_walls() {
    let (mut h, mut watch) = sandbox();
    for size in [vec2(1100.0, 760.0), vec2(1700.0, 900.0)] {
        h.set_size(size);
        watch.rest(&mut h);
        let arena = h.state().arena;
        for b in h.state().world.bodies() {
            assert!(
                b.rect.min.x >= arena.min.x - 0.5 && b.rect.max.x <= arena.max.x + 0.5,
                "{:?} outside {arena:?}",
                b.rect
            );
            assert!(!b.lifted);
        }
    }
}

/// The debug overlay's layers are switches, off at the start.
#[test]
fn the_overlay_layers_switch() {
    let (mut h, mut watch) = sandbox();
    // tall enough for the whole control panel
    h.set_size(vec2(1250.0, 1600.0));
    watch.rest(&mut h);
    assert!(!h.state().overlay.shapes && !h.state().overlay.pulls);
    h.get_by_label("Collision shapes").click();
    watch.frame(&mut h);
    h.get_by_label("Vertex pulls").click();
    watch.frame(&mut h);
    assert!(h.state().overlay.shapes && h.state().overlay.pulls);
}

/// Round 4, item 3 (user: "vertex attraction on hexagons is too weak, even at strength 60"): in the
/// Hexagons scene, at rest, every pull of the overlay is done (locked), as in the Rectangles scene. Before,
/// 64 of 86 never locked: hexagons of slightly different widths stacked in a column were centred on each
/// other (both ends 2 points off, for ever), and their bounding boxes were pulled though a rigid hexagon
/// cannot follow its box.
#[test]
fn the_pulls_of_the_hexagons_scene_are_done_at_rest() {
    let (h, _watch) = sandbox_in(Scene::Hexagons);
    let world = &h.state().world;
    let pulls = world.pulls();
    assert!(pulls.len() >= 10, "faces lie against each other: {pulls:?}");
    let open: Vec<_> = pulls.iter().filter(|p| !p.locked).collect();
    assert!(open.is_empty(), "pulls not done at rest: {open:?}");
}

/// Drag the frame `slot` by its footer by `by`, with `modifiers` held, let go and wait for rest.
fn drag_by(
    h: &mut App,
    watch: &mut Watch,
    slot: SlotId,
    by: egui::Vec2,
    modifiers: egui::Modifiers,
) {
    let rect = h.state().screen_rect(body_rect(h, slot));
    let from = pos2(rect.center().x, rect.max.y - 8.0);
    h.event(egui::Event::ModifiersChanged(modifiers));
    h.hover_at(from);
    watch.frame(h);
    h.drag_at(from);
    watch.frame(h);
    for n in 1..=30 {
        h.hover_at(from + by * (n as f32 / 30.0));
        watch.frame(h);
    }
    h.drop_at(from + by);
    watch.frame(h);
    h.event(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
    watch.rest(h);
}

/// Round 4, item 4 (SETL-22): two frames stuck together away from the walls (no gravity) move together when
/// one is dragged; with Alt held the one dragged comes away alone.
#[test]
fn dragging_one_of_a_group_moves_the_group_and_alt_takes_it_alone() {
    let (mut h, mut watch, slots) = sandbox_of(&[0, 3]);
    let s = h.state_mut();
    let mut p = *s.world.params();
    p.gravity_side = None;
    s.world.set_params(p);
    // side by side in the middle, the gap apart
    let (a, b) = (slots[0], slots[1]);
    let (ba, bb) = (s.frames.body(a).unwrap(), s.frames.body(b).unwrap());
    let (ra, rb) = (
        s.world.body(ba).unwrap().rect,
        s.world.body(bb).unwrap().rect,
    );
    let corner = s.world.bounds().min + ve_settle::Vec2::new(150.0, 250.0);
    s.world.put(ba, corner + ra.size() * 0.5, ra.size());
    let at = corner + ve_settle::Vec2::new(ra.size().x + p.gap, 0.0) + rb.size() * 0.5;
    s.world.put(bb, at, rb.size());
    watch.seen.clear();
    watch.rest(&mut h);
    assert_eq!(h.state().world.cluster(ba).len(), 2, "stuck together");

    let before = (body_rect(&h, a), body_rect(&h, b));
    drag_by(
        &mut h,
        &mut watch,
        a,
        vec2(120.0, 160.0),
        egui::Modifiers::NONE,
    );
    let after = (body_rect(&h, a), body_rect(&h, b));
    let moved = |from: Rect, to: Rect| to.center() - from.center();
    let (ma, mb) = (moved(before.0, after.0), moved(before.1, after.1));
    assert!(ma.length() > 100.0, "the dragged one moved: {ma:?}");
    assert!(
        (ma - mb).length() < 2.0,
        "the other came along: {ma:?} {mb:?}"
    );

    let before = (body_rect(&h, a), body_rect(&h, b));
    drag_by(
        &mut h,
        &mut watch,
        a,
        vec2(0.0, 400.0),
        egui::Modifiers::ALT,
    );
    let after = (body_rect(&h, a), body_rect(&h, b));
    let (ma, mb) = (moved(before.0, after.0), moved(before.1, after.1));
    assert!(ma.length() > 300.0, "the dragged one moved: {ma:?}");
    assert!(mb.length() < 2.0, "the other stayed: {mb:?}");
}

/// User, 12 Oct 2026 15:35: a widget dropped near the arena's edge or past it jumped away from the edge
/// instead of settling. Dragged down through the stack and dropped near the open bottom, it pushed the frames
/// under it out, and its corner snapped to theirs and took on their speed: it ran off after them, 30 points
/// and more away from the top where gravity has it. Now a corner does not snap to a body pushed aside faster
/// than settling goes, and the drag (`Params::drag`) stops the pushed ones sooner.
#[test]
fn a_frame_dropped_near_the_bottom_stays_where_it_is_dropped() {
    let (mut h, mut watch) = sandbox();
    let slot = find(&h, |d| {
        !d.opts.hex && d.opts.footer && d.title() == "medium LISMFX"
    });
    let r = h.state().screen_rect(body_rect(&h, slot));
    let from = pos2(r.center().x, r.max.y - 8.0);
    let by = vec2(0.0, 560.0);
    h.hover_at(from);
    watch.frame(&mut h);
    h.drag_at(from);
    watch.frame(&mut h);
    for n in 1..=30 {
        h.hover_at(from + by * (n as f32 / 30.0));
        watch.frame(&mut h);
    }
    h.drop_at(from + by);
    // the frame of the drop: the body takes the pointer's last step
    watch.frame(&mut h);
    let world = &h.state().world;
    let at: Vec<f32> = world.bodies().iter().map(|b| b.rect.min.y).collect();
    let mine = h.state().frames.body(slot).expect("a body");
    let mine = world
        .bodies()
        .iter()
        .position(|b| b.id == mine)
        .expect("in the world");
    // how far each runs on down, away from gravity's side
    let mut ran = vec![0.0f32; at.len()];
    for _ in 0..300 {
        watch.frame(&mut h);
        for (i, b) in h.state().world.bodies().iter().enumerate() {
            ran[i] = ran[i].max(b.rect.min.y - at[i]);
        }
    }
    assert!(ran[mine] < 3.0, "the dropped one ran on {}", ran[mine]);
    let most = ran.iter().copied().fold(0.0, f32::max);
    // pushed out of its way they glide on 48 points (89 with drag 0)
    assert!(most < 60.0, "a pushed one ran on {most}: {ran:?}");
}
