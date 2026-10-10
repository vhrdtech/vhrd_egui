//! The sandbox driven like a person through egui_kittest: add a widget,
//! drag one, resize the window, and on every frame no two solid bodies
//! overlap, nothing jumps, and it all comes to rest in time (SETL-3).

#[path = "../examples/sandbox/app.rs"]
#[allow(dead_code)]
mod app;

use std::collections::HashMap;

use app::{Sandbox, Scene};
use egui::{Pos2, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use ve_settle::{BodyId, Rect};

/// Frames per second the tests run at.
const FPS: f32 = 60.0;
/// Everything has to be at rest this many frames after the last change (5 s).
const SETTLE_FRAMES: usize = 300;
/// The farthest a body's edge may move in one frame, points: beyond this
/// it reads as a jump, not a glide.
const MAX_FRAME_MOVE: f32 = 32.0;

type App = Harness<'static, Sandbox>;

/// Steps the app frame by frame and checks the promises on each one.
struct Watch {
    seen: HashMap<BodyId, Rect>,
    /// The largest move of a body in one frame so far.
    worst_move: f32,
}

impl Watch {
    fn new(h: &App) -> Self {
        let mut watch = Self {
            seen: HashMap::new(),
            worst_move: 0.0,
        };
        watch.look(h);
        watch
    }

    fn look(&mut self, h: &App) {
        let world = &h.state().world;
        assert_eq!(world.max_overlap(), 0.0, "two solid bodies overlap");
        let mut now = HashMap::new();
        for body in world.bodies() {
            // The held body goes where the pointer goes.
            if let (Some(before), false) = (self.seen.get(&body.id), body.dragged) {
                let moved = (body.rect.min - before.min)
                    .max_abs()
                    .max((body.rect.max - before.max).max_abs());
                assert!(
                    moved <= MAX_FRAME_MOVE,
                    "body {:?} jumped {moved} points in one frame",
                    body.id
                );
                self.worst_move = self.worst_move.max(moved);
            }
            now.insert(body.id, body.rect);
        }
        self.seen = now;
    }

    /// One frame.
    fn frame(&mut self, h: &mut App) {
        h.step();
        self.look(h);
    }

    /// Frames until the world rests; panics if that takes too long.
    fn rest(&mut self, h: &mut App) -> usize {
        self.rest_within(h, SETTLE_FRAMES)
    }

    /// Frames until the world rests; panics after `limit`.
    fn rest_within(&mut self, h: &mut App, limit: usize) -> usize {
        // The frame after an input applies it; only then is "at rest" news.
        self.frame(h);
        self.frame(h);
        for n in 2..limit {
            if h.state().world.is_at_rest() {
                return n;
            }
            self.frame(h);
        }
        panic!("not at rest {limit} frames after the last change");
    }
}

/// The sandbox's few-widgets scene in a window, its widgets settled.
fn sandbox() -> (App, Watch) {
    sandbox_with(Scene::Few)
}

fn sandbox_with(scene: Scene) -> (App, Watch) {
    let mut h = Harness::builder()
        .with_size(vec2(1600.0, 1000.0))
        .with_step_dt(1.0 / FPS)
        .build_eframe(move |cc| Sandbox::with_scene(cc, scene));
    h.step();
    let mut watch = Watch::new(&h);
    // The full scene drops 30 widgets at once, packed tighter than they
    // fit; the field gives way row by row and rests in about 6.5 s
    // (SETL-11).
    let limit = match scene {
        Scene::Few => SETTLE_FRAMES,
        Scene::Full | Scene::Shapes | Scene::Tight => SETTLE_FRAMES * 2,
    };
    watch.rest_within(&mut h, limit);
    (h, watch)
}

fn click_button(h: &mut App, watch: &mut Watch, label: &str) {
    h.get_by_label(label).click();
    watch.frame(h);
}

fn center(h: &App, id: BodyId) -> Pos2 {
    let c = h.state().world.body(id).expect("body exists").rect.center();
    pos2(c.x, c.y)
}

/// Press, move in small steps like a hand does, release.
fn drag(h: &mut App, watch: &mut Watch, from: Pos2, to: Pos2, frames: usize) {
    h.hover_at(from);
    watch.frame(h);
    h.drag_at(from);
    watch.frame(h);
    for n in 1..=frames {
        h.hover_at(from + (to - from) * (n as f32 / frames as f32));
        watch.frame(h);
    }
    h.drop_at(to);
    watch.frame(h);
}

fn click_at(h: &mut App, watch: &mut Watch, at: Pos2) {
    h.hover_at(at);
    watch.frame(h);
    h.drag_at(at);
    watch.frame(h);
    h.drop_at(at);
    watch.frame(h);
}

fn assert_inside_arena(h: &App) {
    let arena = h.state().arena;
    for body in h.state().world.bodies() {
        let r = body.rect;
        assert!(
            r.min.x >= arena.min.x - 0.5
                && r.min.y >= arena.min.y - 0.5
                && r.max.x <= arena.max.x + 0.5
                && r.max.y <= arena.max.y + 0.5,
            "body {:?} at {r:?} is outside the arena {arena:?}",
            body.id
        );
    }
}

#[test]
fn the_first_widgets_settle_and_then_nothing_moves() {
    let (mut h, mut watch) = sandbox();
    let world = &h.state().world;
    assert_eq!(world.bodies().len(), 5);
    assert!(world.bodies().iter().all(|b| !b.lifted));
    assert_inside_arena(&h);

    let (frozen, steps) = (world.bodies().to_vec(), world.steps());
    for _ in 0..30 {
        watch.frame(&mut h);
    }
    assert_eq!(
        h.state().world.bodies(),
        &frozen[..],
        "at rest nothing moves"
    );
    assert_eq!(
        h.state().world.steps(),
        steps,
        "at rest nothing is computed"
    );
}

#[test]
fn adding_widgets_makes_room_for_them() {
    let (mut h, mut watch) = sandbox();
    for n in 0..4 {
        click_button(&mut h, &mut watch, "Add widget");
        let frames = watch.rest(&mut h);
        assert!(frames > 2, "widget {n}: the world had something to do");
        assert_eq!(h.state().world.bodies().len(), 6 + n);
        assert!(h.state().world.bodies().iter().all(|b| !b.lifted));
        assert_inside_arena(&h);
    }
    // Double-clicking empty arena adds one under the pointer.
    let arena = h.state().arena;
    let spot = arena.right_bottom() - vec2(30.0, 30.0);
    click_at(&mut h, &mut watch, spot);
    click_at(&mut h, &mut watch, spot);
    watch.rest(&mut h);
    assert_eq!(h.state().world.bodies().len(), 10);
}

#[test]
fn dragging_a_widget_through_the_others() {
    let (mut h, mut watch) = sandbox();
    let (first, last) = {
        let bodies = h.state().world.bodies();
        (bodies[0].id, bodies[4].id)
    };
    let (from, to) = (center(&h, last), center(&h, first));
    let others_before: Vec<_> = h.state().world.bodies()[1..4]
        .iter()
        .map(|b| b.rect)
        .collect();

    // Onto the first widget and on to the arena's top left corner.
    drag(&mut h, &mut watch, from, to, 40);
    assert!(h.state().world.body(last).is_some_and(|b| !b.dragged));
    watch.rest(&mut h);

    let world = &h.state().world;
    assert!(world.bodies().iter().all(|b| !b.lifted && !b.dragged));
    assert!(world.max_gap_violation() < 0.5);
    assert_inside_arena(&h);
    assert_eq!(
        h.state().selected,
        Some(last),
        "the dragged widget is selected"
    );
    // It was dropped on the first widget's place: that is where it now is, near enough.
    let dropped = center(&h, last);
    assert!(
        (dropped - to).length() < 160.0,
        "dropped at {to:?}, rests at {dropped:?}"
    );
    // Widgets that were not in the way are where they were.
    let stayed = h.state().world.bodies()[1..4]
        .iter()
        .zip(&others_before)
        .filter(|(b, before)| (b.rect.min - before.min).max_abs() < 2.0)
        .count();
    assert!(stayed >= 2, "only {stayed} of 3 bystanders stayed put");
}

#[test]
fn resizing_the_window_keeps_everything_inside() {
    let (mut h, mut watch) = sandbox();
    for size in [
        vec2(1300.0, 820.0),
        vec2(1150.0, 760.0),
        vec2(1800.0, 1100.0),
    ] {
        h.set_size(size);
        watch.rest(&mut h);
        assert_inside_arena(&h);
        assert!(h.state().world.bodies().iter().all(|b| !b.lifted));
    }
}

#[test]
fn growing_and_shrinking_content_pushes_and_lets_go() {
    let (mut h, mut watch) = sandbox();
    let id = h.state().world.bodies()[0].id;
    let at = center(&h, id);
    click_at(&mut h, &mut watch, at);
    assert_eq!(h.state().selected, Some(id));
    watch.rest(&mut h);
    let before = h.state().world.body(id).unwrap().rect.size();

    for _ in 0..8 {
        click_button(&mut h, &mut watch, "Grow");
    }
    watch.rest(&mut h);
    let grown = h.state().world.body(id).unwrap().rect.size();
    assert!(grown.y > before.y + 60.0, "{before:?} → {grown:?}");

    for _ in 0..8 {
        click_button(&mut h, &mut watch, "Shrink");
    }
    watch.rest(&mut h);
    let back = h.state().world.body(id).unwrap().rect.size();
    assert!((back - before).max_abs() < 1.0, "{before:?} → {back:?}");
}

#[test]
fn the_corner_handle_sets_a_size_by_hand() {
    let (mut h, mut watch) = sandbox();
    let id = h.state().world.bodies()[1].id;
    let rect = h.state().world.body(id).unwrap().rect;
    let corner = pos2(rect.max.x - 5.0, rect.max.y - 5.0);
    drag(&mut h, &mut watch, corner, corner + vec2(70.0, 50.0), 20);
    watch.rest(&mut h);
    let size = h.state().world.body(id).unwrap().rect.size();
    assert!(
        (size.x - (rect.size().x + 70.0)).abs() < 8.0
            && (size.y - (rect.size().y + 50.0)).abs() < 8.0,
        "{:?} → {size:?}",
        rect.size()
    );
    click_button(&mut h, &mut watch, "Auto size");
    watch.rest(&mut h);
    let size = h.state().world.body(id).unwrap().rect.size();
    assert!(
        (size - rect.size()).max_abs() < 1.0,
        "back to the content's size: {size:?}"
    );
}

#[test]
fn removing_the_selected_widget() {
    let (mut h, mut watch) = sandbox();
    let id = h.state().world.bodies()[2].id;
    let at = center(&h, id);
    click_at(&mut h, &mut watch, at);
    click_button(&mut h, &mut watch, "Remove selected");
    watch.rest(&mut h);
    assert_eq!(h.state().world.bodies().len(), 4);
    assert!(h.state().world.body(id).is_none());
    assert_eq!(h.state().selected, None);
}

#[test]
fn pause_stops_the_world_and_step_runs_one_step() {
    let (mut h, mut watch) = sandbox();
    click_button(&mut h, &mut watch, "Pause");
    click_button(&mut h, &mut watch, "Add widget");
    let steps = h.state().world.steps();
    for _ in 0..10 {
        watch.frame(&mut h);
    }
    assert_eq!(h.state().world.steps(), steps, "paused: no steps");
    for n in 1..=3 {
        click_button(&mut h, &mut watch, "Step");
        assert_eq!(h.state().world.steps(), steps + n);
    }
    click_button(&mut h, &mut watch, "Resume");
    watch.rest(&mut h);
    assert!(h.state().world.steps() > steps + 3);
}

#[test]
fn content_changing_by_itself_never_overlaps_or_jumps() {
    let (mut h, mut watch) = sandbox();
    let amounts = |h: &App| h.state().items.iter().map(|i| i.amount).collect::<Vec<_>>();
    let (steps, before) = (h.state().world.steps(), amounts(&h));
    click_button(&mut h, &mut watch, "Content changes by itself");
    for _ in 0..900 {
        watch.frame(&mut h);
    }
    assert_ne!(amounts(&h), before, "content changed");
    assert!(
        h.state().world.steps() > steps + 100,
        "the world kept working"
    );
    click_button(&mut h, &mut watch, "Content changes by itself");
    watch.rest(&mut h);
    println!("worst move in one frame: {:.2} points", watch.worst_move);
}

#[test]
fn a_widget_dropped_near_another_ones_corner_snaps_to_it() {
    let (mut h, mut watch) = sandbox();
    // Two widgets only, so nothing else is in the way.
    let ids: Vec<_> = h.state().world.bodies().iter().map(|b| b.id).collect();
    for id in &ids[2..] {
        let at = center(&h, *id);
        click_at(&mut h, &mut watch, at);
        click_button(&mut h, &mut watch, "Remove selected");
    }
    watch.rest(&mut h);
    let (fixed, moved) = (ids[0], ids[1]);
    let gap = h.state().world.params().gap;

    // Drop it to the right of the other one, its top left corner 9 points
    // to the right of and 2 above where it would line up. (The two differ
    // in height, so it is the top corners that are nearest.)
    let (a, b) = {
        let world = &h.state().world;
        (
            world.body(fixed).unwrap().rect,
            world.body(moved).unwrap().rect,
        )
    };
    let from = center(&h, moved);
    let to = pos2(
        a.max.x + gap + 9.0 + b.size().x / 2.0,
        a.min.y - 2.0 + b.size().y / 2.0,
    );
    drag(&mut h, &mut watch, from, to, 40);
    watch.rest(&mut h);

    let world = &h.state().world;
    let (a, b) = (
        world.body(fixed).unwrap().rect,
        world.body(moved).unwrap().rect,
    );
    assert!(
        (a.min.y - b.min.y).abs() < 1.0,
        "tops line up: {} and {}",
        a.min.y,
        b.min.y
    );
    assert!(
        (b.min.x - a.max.x - gap).abs() < 1.0,
        "the gap is between them: {}",
        b.min.x - a.max.x
    );
    assert!(
        world
            .snaps()
            .iter()
            .any(|s| s.body == moved && s.grip > 0.9),
        "the overlay shows the corner held: {:?}",
        world.snaps()
    );

    // Snap range 0 (the slider's left end) lets go: gravity and the home
    // spot pull it out of line again.
    let mut params = *world.params();
    params.snap_range = 0.0;
    h.state_mut().world.set_params(params);
    watch.rest(&mut h);
    let world = &h.state().world;
    let (a, b) = (
        world.body(fixed).unwrap().rect,
        world.body(moved).unwrap().rect,
    );
    assert!(world.snaps().is_empty());
    assert!(
        (a.min.y - b.min.y).abs() > 3.0,
        "no longer held: {} and {}",
        a.min.y,
        b.min.y
    );
}

#[test]
fn a_row_overfilled_by_the_window_sends_one_widget_to_the_next_row() {
    let (mut h, mut watch) = sandbox();
    // Start from four widgets of one size in a row under the top wall.
    let ids: Vec<_> = h.state().world.bodies().iter().map(|b| b.id).collect();
    for id in ids {
        let at = center(&h, id);
        click_at(&mut h, &mut watch, at);
        click_button(&mut h, &mut watch, "Remove selected");
    }
    let arena = h.state().arena;
    let size = vec2(280.0, 180.0);
    for i in 0..4 {
        let at = arena.min + vec2(20.0 + size.x / 2.0 + i as f32 * (size.x + 30.0), 120.0);
        let id = h.state_mut().add_widget(Some(at));
        let item = h.state_mut().items.iter_mut().find(|item| item.id == id);
        item.expect("just added").user_size = Some(size);
    }
    watch.rest(&mut h);
    let world = &h.state().world;
    let top = world.bodies()[0].rect.min.y;
    assert!(
        world
            .bodies()
            .iter()
            .all(|b| (b.rect.min.y - top).abs() < 6.0),
        "one row: {:?}",
        world.bodies().iter().map(|b| b.rect).collect::<Vec<_>>()
    );

    // Narrow the window until only three fit; watch the pillow at work.
    let gap = world.params().gap;
    let three = 3.0 * size.x + 4.0 * gap;
    let window = h.ctx.content_rect().size();
    h.set_size(vec2(window.x - (arena.width() - three) + 30.0, window.y));
    let mut yielded = false;
    for _ in 0..SETTLE_FRAMES {
        watch.frame(&mut h);
        yielded |= h.state().world.bodies().iter().any(|b| b.yielding);
        if h.state().world.is_at_rest() {
            break;
        }
    }
    watch.rest(&mut h);
    assert!(yielded, "one widget gave way");
    assert_inside_arena(&h);
    let world = &h.state().world;
    let below = world
        .bodies()
        .iter()
        .filter(|b| b.rect.min.y > top + size.y)
        .count();
    assert_eq!(below, 1, "exactly one moved down: {:#?}", world.bodies());
    for b in world.bodies() {
        assert!(b.tension() < 0.01, "not crushed: {b:?}");
        assert_eq!(b.bulge, ve_settle::Vec2::ZERO, "calm again: {b:?}");
    }
    // The overlay layer for it is there to switch.
    h.get_by_label("Collision shapes").click();
    watch.frame(&mut h);
    assert!(!h.state().overlay.shapes);
}

#[test]
fn widgets_of_nearly_the_same_height_end_with_one_bottom_line() {
    let (mut h, mut watch) = sandbox();
    // Two widgets side by side under the top wall, 5 points apart in height.
    let ids: Vec<_> = h.state().world.bodies().iter().map(|b| b.id).collect();
    for id in ids {
        let at = center(&h, id);
        click_at(&mut h, &mut watch, at);
        click_button(&mut h, &mut watch, "Remove selected");
    }
    let arena = h.state().arena;
    let gap = h.state().world.params().gap;
    let mut pair = Vec::new();
    for (i, height) in [180.0, 185.0].into_iter().enumerate() {
        let size = vec2(280.0, height);
        let at = arena.min
            + vec2(
                gap + size.x / 2.0 + i as f32 * (size.x + gap),
                gap + height / 2.0,
            );
        let id = h.state_mut().add_widget(Some(at));
        let item = h.state_mut().items.iter_mut().find(|item| item.id == id);
        item.expect("just added").user_size = Some(size);
        pair.push(id);
    }
    watch.rest(&mut h);
    let rects = |h: &App| {
        let world = &h.state().world;
        [
            world.body(pair[0]).unwrap().rect,
            world.body(pair[1]).unwrap().rect,
        ]
    };
    let [a, b] = rects(&h);
    assert!((a.min.y - b.min.y).abs() < 0.1, "one row: {a:?} {b:?}");
    assert!(
        (a.max.y - b.max.y).abs() < 0.1,
        "bottoms flush: {a:?} {b:?}"
    );
    assert!(
        (a.size().y - 182.5).abs() < 1.0,
        "they met in the middle: {a:?}"
    );
    assert!(
        !h.state().world.pulls().is_empty(),
        "the overlay has pulls to show"
    );

    // 60 points apart they are left as they are.
    let item = h
        .state_mut()
        .items
        .iter_mut()
        .find(|item| item.id == pair[1]);
    item.expect("there").user_size = Some(vec2(280.0, 240.0));
    watch.rest(&mut h);
    let [a, b] = rects(&h);
    assert!((a.size().y - 180.0).abs() < 0.1, "{a:?}");
    assert!((b.size().y - 240.0).abs() < 0.1, "{b:?}");

    // Range 0 (the slider's left end) turns it off, and the layer can be switched.
    let item = h
        .state_mut()
        .items
        .iter_mut()
        .find(|item| item.id == pair[1]);
    item.expect("there").user_size = Some(vec2(280.0, 185.0));
    let mut params = *h.state().world.params();
    params.attract_range = 0.0;
    h.state_mut().world.set_params(params);
    watch.rest(&mut h);
    let [a, b] = rects(&h);
    assert!((b.max.y - a.max.y - 5.0).abs() < 0.1, "{a:?} {b:?}");
    h.get_by_label("Vertex pulls").click();
    watch.frame(&mut h);
    assert!(!h.state().overlay.pulls);
}

#[test]
fn the_shapes_scene_settles_and_an_outline_can_be_edited() {
    let (mut h, mut watch) = sandbox_with(Scene::Shapes);
    // Hexagons, half hexagons, triangles and panels came to rest without
    // overlap (the watch checks every frame, by outline).
    let world = &h.state().world;
    let tiles = world
        .bodies()
        .iter()
        .filter(|b| b.outline.is_some())
        .count();
    assert!(tiles >= 12 && tiles < world.bodies().len(), "{tiles} tiles");
    assert!(
        world
            .bodies()
            .iter()
            .all(|b| !b.lifted && b.shape().is_convex())
    );
    assert_inside_arena(&h);

    // The buttons add more.
    let before = world.bodies().len();
    for label in ["Add hexagon", "Add half hexagon", "Add triangle"] {
        click_button(&mut h, &mut watch, label);
    }
    watch.rest(&mut h);
    assert_eq!(h.state().world.bodies().len(), before + 3);

    // Select a hexagon and drag the + of its top edge down, into it: a
    // vertex is put on the edge, and stays there (it would dent the outline).
    let hex = h.state().world.bodies()[0].id;
    let at = center(&h, hex);
    click_at(&mut h, &mut watch, at);
    assert_eq!(h.state().selected, Some(hex));
    let plus = h.state().plus_pos(hex, 0).expect("the top edge has a +");
    drag(&mut h, &mut watch, plus, plus + vec2(0.0, 40.0), 10);
    let outline = h.state().world.body(hex).unwrap().outline.expect("outline");
    assert_eq!(outline.points().len(), 7, "{outline:?}");
    assert!(
        outline.points()[1].y.abs() < 1e-3,
        "on the top edge: {outline:?}"
    );
    assert!(outline.is_convex());

    // Its right tip (now vertex 3) dragged inward moves; dragged far out
    // it stops at the rectangle.
    let tip = h.state().vertex_pos(hex, 3).expect("the tip");
    drag(&mut h, &mut watch, tip, tip + vec2(-30.0, 0.0), 10);
    let outline = h.state().world.body(hex).unwrap().outline.unwrap();
    assert!(
        (outline.points()[3].x - 0.83).abs() < 0.05,
        "moved in by 30 of 180: {outline:?}"
    );
    let tip = h.state().vertex_pos(hex, 3).expect("the tip");
    drag(&mut h, &mut watch, tip, tip + vec2(200.0, 10.0), 10);
    let outline = h.state().world.body(hex).unwrap().outline.unwrap();
    assert!(outline.points()[3].x <= 1.0 + 1e-3, "{outline:?}");
    assert!(outline.is_convex(), "{outline:?}");
    watch.rest(&mut h);

    // A plain panel gets an outline with its first new vertex.
    let panel = h
        .state()
        .world
        .bodies()
        .iter()
        .find(|b| b.outline.is_none())
        .expect("a panel")
        .id;
    let at = center(&h, panel);
    click_at(&mut h, &mut watch, at);
    let plus = h
        .state()
        .plus_pos(panel, 1)
        .expect("the right edge has a +");
    click_at(&mut h, &mut watch, plus);
    let outline = h
        .state()
        .world
        .body(panel)
        .unwrap()
        .outline
        .expect("outline");
    assert_eq!(outline.points().len(), 5);
    watch.rest(&mut h);
}

/// `frames` more frames in which nothing at all moves: the world sleeps.
fn assert_asleep(h: &mut App, watch: &mut Watch, frames: usize) {
    let frozen = h.state().world.bodies().to_vec();
    let steps = h.state().world.steps();
    for _ in 0..frames {
        watch.frame(h);
    }
    assert!(h.state().world.is_at_rest());
    assert_eq!(h.state().world.steps(), steps, "no step runs at rest");
    assert_eq!(h.state().world.bodies(), &frozen[..], "exactly zero motion");
}

#[test]
fn every_captured_pair_in_the_full_scene_ends_lined_up_and_the_field_sleeps() {
    let (mut h, mut watch) = sandbox_with(Scene::Full);
    let world = &h.state().world;
    assert!(world.is_at_rest());
    // Pulls there are (the field is full of near-equal neighbours), and
    // every one of them is closed: locked, less than half a point left.
    assert!(world.pulls().len() > 20, "{} pulls", world.pulls().len());
    for pull in world.pulls() {
        let left = (pull.to - pull.from).max_abs();
        assert!(left < 0.5 && pull.locked, "open by {left}: {pull:?}");
    }
    let stretched = world
        .bodies()
        .iter()
        .filter(|b| b.stretch_lo.max_abs().max(b.stretch_hi.max_abs()) > 0.5)
        .count();
    assert!(stretched >= 5, "{stretched} widgets were resized");
    assert_asleep(&mut h, &mut watch, 120);
}

#[test]
fn the_mixed_scenes_come_to_full_rest_and_clear_empties_the_arena() {
    for scene in [Scene::Shapes, Scene::Tight] {
        let (mut h, mut watch) = sandbox_with(scene);
        assert_asleep(&mut h, &mut watch, 120);
        // A tile dropped on the pack (SETL-14): nothing jumps (the watch
        // checks every frame), and it all sleeps again.
        for label in [
            "Add half hexagon",
            "Add hexagon",
            "Add triangle",
            "Add half hexagon",
        ] {
            click_button(&mut h, &mut watch, label);
            for _ in 0..20 {
                watch.frame(&mut h);
            }
        }
        watch.rest_within(&mut h, SETTLE_FRAMES * 2);
        assert_asleep(&mut h, &mut watch, 120);

        // Clear scene: an empty arena, the parameters kept; widgets can
        // be added by hand, and Reset seeds the scene again.
        let params = *h.state().world.params();
        h.get_by_label("Clear scene").click();
        h.step();
        // (A new world: its ids start over, so the watch does too.)
        let mut watch = Watch::new(&h);
        assert!(h.state().world.bodies().is_empty() && h.state().items.is_empty());
        assert_eq!(*h.state().world.params(), params);
        for _ in 0..30 {
            watch.frame(&mut h);
        }
        assert!(
            h.state().world.bodies().is_empty(),
            "nothing is seeded again"
        );
        click_button(&mut h, &mut watch, "Add hexagon");
        click_button(&mut h, &mut watch, "Add widget");
        watch.rest(&mut h);
        assert_eq!(h.state().world.bodies().len(), 2);
        h.get_by_label("Reset").click();
        h.step();
        let mut watch = Watch::new(&h);
        watch.rest_within(&mut h, SETTLE_FRAMES * 2);
        assert!(h.state().world.bodies().len() > 10);
    }
}

#[test]
fn the_default_scene_fills_the_width_and_runs_past_the_bottom() {
    let h: App = Harness::builder().build_eframe(|cc| Sandbox::new(cc));
    assert_eq!(
        h.state().scene,
        Scene::Full,
        "the full scene is the default"
    );
    drop(h);

    let (mut h, mut watch) = sandbox_with(Scene::Full);
    let arena = h.state().arena;
    let world = &h.state().world;
    assert!(
        world.bodies().len() >= 15,
        "{} widgets",
        world.bodies().len()
    );
    assert!(world.bodies().iter().all(|b| !b.lifted));
    assert!(!world.params().walls.bottom, "the bottom is open");
    // The packed field gives way row by row: nobody stays squeezed (SETL-11).
    for b in world.bodies() {
        assert!(
            b.tension() < 0.01,
            "squeezed {:.0} %: {b:?}",
            b.tension() * 100.0
        );
    }
    // Past the bottom of the window, and across the whole width.
    let lowest = world
        .bodies()
        .iter()
        .map(|b| b.rect.max.y)
        .fold(0.0, f32::max);
    assert!(
        lowest > arena.max.y + 100.0,
        "lowest {lowest}, window {arena:?}"
    );
    let right = world
        .bodies()
        .iter()
        .map(|b| b.rect.max.x)
        .fold(0.0, f32::max);
    assert!(
        right > arena.max.x - 120.0,
        "rightmost {right}, window {arena:?}"
    );
    for b in world.bodies() {
        assert!(
            b.rect.min.x >= arena.min.x - 0.5
                && b.rect.max.x <= arena.max.x + 0.5
                && b.rect.min.y >= arena.min.y - 0.5,
            "inside the side and top walls: {:?}",
            b.rect
        );
    }

    // Scrolling down shows the bottom; the world does not move.
    let frozen = h.state().world.bodies().to_vec();
    h.hover_at(arena.center());
    h.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: vec2(0.0, -4000.0),
        modifiers: egui::Modifiers::NONE,
        phase: egui::TouchPhase::Move,
    });
    for _ in 0..30 {
        watch.frame(&mut h);
    }
    let state = h.state();
    assert!(state.scroll > 100.0, "scrolled {}", state.scroll);
    assert!(
        (state.scroll - state.max_scroll()).abs() < 1.0,
        "to the end"
    );
    assert_eq!(state.world.bodies(), &frozen[..], "scrolling moves nothing");

    // A widget down there can be grabbed where it is drawn.
    let low = state
        .world
        .bodies()
        .iter()
        .max_by(|a, b| a.rect.max.y.total_cmp(&b.rect.max.y))
        .expect("bodies")
        .id;
    let at = state.screen_pos(state.world.body(low).unwrap().rect.center());
    assert!(arena.contains(at), "on screen after scrolling: {at:?}");
    click_at(&mut h, &mut watch, at);
    assert_eq!(h.state().selected, Some(low));
}

#[test]
fn the_full_scene_is_full_from_the_first_frame_and_after_the_window_grows() {
    let full = |h: &App| {
        let (arena, world) = (h.state().arena, &h.state().world);
        let lowest = world
            .bodies()
            .iter()
            .map(|b| b.rect.max.y)
            .fold(0.0, f32::max);
        let right = world
            .bodies()
            .iter()
            .map(|b| b.rect.max.x)
            .fold(0.0, f32::max);
        (
            lowest > arena.max.y,
            right > arena.max.x - 300.0,
            world.bodies().len(),
        )
    };
    // The first frame already shows the screen filled, running past the bottom.
    let mut h: App = Harness::builder()
        .with_size(vec2(1600.0, 1000.0))
        .with_step_dt(1.0 / FPS)
        .build_eframe(|cc| Sandbox::new(cc));
    h.step();
    let (past_bottom, across, n) = full(&h);
    assert!(past_bottom && across && n >= 20, "frame 1: {n} widgets");

    // A window that starts tiny and is then sized up (a tiling window
    // manager does that) ends up just as full.
    let mut h: App = Harness::builder()
        .with_size(vec2(400.0, 200.0))
        .with_step_dt(1.0 / FPS)
        .build_eframe(|cc| Sandbox::new(cc));
    h.step();
    assert_eq!(h.state().world.bodies().len(), 0, "not seeded while tiny");
    h.set_size(vec2(1600.0, 1000.0));
    h.step();
    h.step();
    let (past_bottom, across, n) = full(&h);
    assert!(
        past_bottom && across && n >= 20,
        "after sizing up: {n} widgets"
    );
    h.set_size(vec2(2400.0, 1400.0));
    h.step();
    h.step();
    let (past_bottom, across, more) = full(&h);
    assert!(
        past_bottom && across && more > n,
        "a bigger window gets more: {more}"
    );
}
