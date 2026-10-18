//! A world that holds moves nothing by itself (SETL-41): a drop leaves the dropped body where it was let go (or
//! puts it out of what it landed on, alone) and every other body where it is, a resize moves no neighbour, and
//! nothing coasts. Over many steps, in the dash's kind of world (gravity up, the bottom open, panels packed a gap
//! apart).

use ve_settle::{BodyDesc, BodyId, Params, Rect, Settling, Side, Vec2, World};

fn v(x: f32, y: f32) -> Vec2 {
    Vec2::new(x, y)
}

/// The dash's world (`tpm_mesh_dash` `app::layout::params`).
fn params() -> Params {
    let d = Params::default();
    let mut p = d;
    p.gravity_side = Some(Side::Top);
    p.walls.bottom = false;
    p.home_stiffness = 0.0;
    p.max_speed = d.glide_speed;
    p.push_out = true;
    p.attract_range = 0.0;
    p.snap_range = 0.0;
    p
}

fn rect(w: &World, id: BodyId) -> Rect {
    w.body(id).expect("body").rect
}

fn moved(a: Rect, b: Rect) -> f32 {
    (a.min - b.min).max_abs().max((a.max - b.max).max_abs())
}

/// A page: two rows of panels a gap apart under the top wall, a full-width one under them, a row of three
/// under that; none pushes (the dash's default). Laid out and at rest.
fn page() -> (World, Vec<BodyId>) {
    let mut w = World::new(
        Rect::from_min_size(v(-6.0, -6.0), v(1212.0, 100_000.0)),
        params(),
    );
    let gap = w.params().gap;
    let mut ids = Vec::new();
    let mut put = |w: &mut World, x: f32, y: f32, sx: f32, sy: f32| {
        let id = w.add(
            BodyDesc::new(v(x + sx / 2.0, y + sy / 2.0), v(sx, sy))
                .home(None)
                .pushes(false),
        );
        w.put(id, v(x + sx / 2.0, y + sy / 2.0), v(sx, sy));
        ids.push(id);
    };
    put(&mut w, 0.0, 0.0, 597.0, 200.0);
    put(&mut w, 597.0 + gap, 0.0, 597.0, 320.0);
    put(&mut w, 0.0, 200.0 + gap, 597.0, 260.0);
    let y = 466.0 + gap;
    put(&mut w, 0.0, y, 1200.0, 150.0);
    let y = y + 150.0 + gap;
    for k in 0..3 {
        put(
            &mut w,
            k as f32 * (300.0 + gap),
            y,
            300.0,
            240.0 + 30.0 * k as f32,
        );
    }
    assert!(w.settle(5_000).is_some(), "the page rests");
    (w, ids)
}

/// Grab `id`, drag it by `by` over a second, let go with the speed of the last steps.
fn drag(w: &mut World, id: BodyId, by: Vec2) {
    let from = rect(w, id).center();
    w.grab(id);
    for k in 1..=60 {
        let t = k as f32 / 60.0;
        w.drag_to(id, from + by * t);
        w.step();
    }
    w.release(id, true);
}

/// Steps a long while (ten seconds of the solver), waking the world now and then from outside.
fn run(w: &mut World) {
    for n in 0..1_200 {
        if n % 200 == 0 {
            w.wake();
        }
        w.step();
    }
}

fn others_stayed(w: &World, ids: &[BodyId], skip: BodyId, before: &[Rect]) {
    for (i, &id) in ids.iter().enumerate() {
        if id == skip {
            continue;
        }
        let m = moved(before[i], rect(w, id));
        assert!(
            m < 0.01,
            "body {i} moved {m}: {:?} → {:?}",
            before[i],
            rect(w, id)
        );
    }
}

#[test]
fn a_drop_into_free_room_moves_nothing_else_and_stays_where_it_was_let_go() {
    let (mut w, ids) = page();
    assert_eq!(w.settling(), Settling::Hold);
    let before: Vec<Rect> = ids.iter().map(|&id| rect(&w, id)).collect();
    // the last of the row of three, into the free room right of it
    let mover = ids[6];
    drag(&mut w, mover, v(250.0, 40.0));
    let let_go = rect(&w, mover);
    run(&mut w);
    others_stayed(&w, &ids, mover, &before);
    let m = moved(let_go, rect(&w, mover));
    assert!(m < 0.01, "it coasted {m} after the drop");
}

#[test]
fn a_drop_onto_a_neighbour_moves_only_the_dropped_one() {
    let (mut w, ids) = page();
    let before: Vec<Rect> = ids.iter().map(|&id| rect(&w, id)).collect();
    // the middle of the row of three, half onto its left neighbour
    let mover = ids[5];
    drag(&mut w, mover, v(-150.0, 20.0));
    run(&mut w);
    others_stayed(&w, &ids, mover, &before);
    assert!(
        w.max_overlap() < 0.5,
        "it does not stay in it: {}",
        w.max_overlap()
    );
}

#[test]
fn a_drop_onto_the_full_width_one_moves_only_the_dropped_one() {
    let (mut w, ids) = page();
    let before: Vec<Rect> = ids.iter().map(|&id| rect(&w, id)).collect();
    // the first of the row of three, up onto the full-width one: it finds no room and goes back
    let mover = ids[4];
    drag(&mut w, mover, v(40.0, -120.0));
    run(&mut w);
    others_stayed(&w, &ids, mover, &before);
    assert!(w.max_overlap() < 0.5, "{}", w.max_overlap());
}

#[test]
fn a_resize_moves_no_neighbour() {
    let (mut w, ids) = page();
    let before: Vec<Rect> = ids.iter().map(|&id| rect(&w, id)).collect();
    // the top left one is given a new size (a resize handle let go): wider and taller than its room
    let id = ids[0];
    let r = rect(&w, id);
    let size = r.size() + v(80.0, 90.0);
    w.set_limits(id, size, size);
    w.put(id, r.min + size * 0.5, size);
    run(&mut w);
    let mut before = before;
    before[0] = rect(&w, id);
    others_stayed(&w, &ids, id, &before);
}

#[test]
fn after_a_body_that_pushes_was_dropped_nothing_goes_on_moving() {
    let (mut w, ids) = page();
    let mover = ids[6];
    w.set_pushes(mover, true);
    drag(&mut w, mover, v(-160.0, 0.0));
    assert!(w.settle(10_000).is_some(), "it comes to rest");
    assert!(w.max_overlap() < 0.5, "{}", w.max_overlap());
    // once at rest nothing goes on moving
    let now: Vec<Rect> = ids.iter().map(|&id| rect(&w, id)).collect();
    run(&mut w);
    others_stayed(&w, &ids, BodyId(u64::MAX), &now);
}
