//! A world that holds (SETL-38, SETL-40): bodies stay where they are put until the user asks it to tidy up, and a
//! body that does not push its neighbours stops at their edge instead.

use ve_settle::{BodyDesc, BodyId, Params, Rect, Settling, Vec2, World};

fn v(x: f32, y: f32) -> Vec2 {
    Vec2::new(x, y)
}

/// The dash's kind of world: gravity up, no homes, the bottom open.
fn params() -> Params {
    let mut p = Params::default();
    p.walls.bottom = false;
    p.home_stiffness = 0.0;
    p.max_speed = p.glide_speed;
    p.push_out = true;
    p
}

fn world() -> World {
    World::new(Rect::from_min_size(Vec2::ZERO, v(1200.0, 2000.0)), params())
}

fn rect(w: &World, id: BodyId) -> Rect {
    w.body(id).expect("body").rect
}

fn moved(a: Rect, b: Rect) -> f32 {
    (a.min - b.min).max_abs().max((a.max - b.max).max_abs())
}

/// Three panels put at odd places, with room between them and to the top: nothing falls, snaps or lines up.
fn scattered(w: &mut World) -> Vec<BodyId> {
    let ids = [
        (v(200.0, 400.0), v(300.0, 120.0)),
        (v(560.0, 380.0), v(240.0, 160.0)),
        (v(330.0, 700.0), v(260.0, 90.0)),
    ]
    .map(|(c, s)| w.add(BodyDesc::new(c, s).home(None)));
    ids.to_vec()
}

#[test]
fn a_world_holds_by_default_and_nothing_drifts() {
    let mut w = world();
    assert_eq!(w.settling(), Settling::Hold);
    let ids = scattered(&mut w);
    assert!(w.settle(2_000).is_some(), "rests at once");
    let start: Vec<Rect> = ids.iter().map(|&id| rect(&w, id)).collect();
    // a minute of frames, and the solver woken now and then from outside
    for n in 0..3_600 {
        if n % 600 == 0 {
            w.wake();
        }
        w.advance(1.0 / 60.0);
    }
    for (i, &id) in ids.iter().enumerate() {
        let m = moved(start[i], rect(&w, id));
        assert!(m < 0.01, "body {i} drifted {m} without a tidy up");
    }
}

#[test]
fn tidy_up_settles_once_then_holds_again() {
    let mut w = world();
    let ids = scattered(&mut w);
    w.settle(2_000);
    w.settle_once();
    assert_eq!(w.settling(), Settling::Once);
    assert!(w.settle(10_000).is_some(), "the tidy up comes to rest");
    assert_eq!(w.settling(), Settling::Hold, "and holds again");
    // gravity pulled them up: the two top ones under the top wall, the third under them
    let gap = w.params().gap;
    for &id in &ids[..2] {
        assert!(rect(&w, id).min.y < gap + 0.5, "{:?}", rect(&w, id));
    }
    let tidy: Vec<Rect> = ids.iter().map(|&id| rect(&w, id)).collect();
    // a new body put far down stays there: the world holds again
    let late = w.add(BodyDesc::new(v(900.0, 1500.0), v(200.0, 100.0)).home(None));
    w.settle(5_000);
    assert!(rect(&w, late).min.y > 1400.0, "{:?}", rect(&w, late));
    for (i, &id) in ids.iter().enumerate() {
        assert!(moved(tidy[i], rect(&w, id)) < 0.01);
    }
}

#[test]
fn a_world_that_settles_always_does_so_without_being_asked() {
    let mut w = world().with_settling(Settling::Always);
    let ids = scattered(&mut w);
    assert!(w.settle(10_000).is_some());
    assert!(rect(&w, ids[0]).min.y < w.params().gap + 0.5);
    assert_eq!(w.settling(), Settling::Always);
}

/// A panel dragged over a neighbour and let go there: one that does not push floats over it and comes to rest at
/// its edge, the neighbour stays; one that pushes moves the neighbour.
fn drop_onto(pushes: bool) -> (Rect, Rect, Rect, Rect) {
    let mut w = world();
    let still = w.add(BodyDesc::new(v(400.0, 300.0), v(300.0, 200.0)).home(None));
    let mover = w.add(
        BodyDesc::new(v(900.0, 300.0), v(200.0, 120.0))
            .home(None)
            .pushes(pushes),
    );
    w.settle(2_000);
    let before = rect(&w, still);
    w.grab(mover);
    // over it, a little right of its middle
    for k in 1..=60 {
        let t = k as f32 / 60.0;
        w.drag_to(mover, v(900.0 - 430.0 * t, 300.0));
        w.step();
    }
    let held = rect(&w, still);
    w.release(mover, true);
    assert!(w.settle(10_000).is_some(), "rests after the drop");
    (before, held, rect(&w, still), rect(&w, mover))
}

#[test]
fn a_body_that_does_not_push_floats_over_its_neighbour_and_stops_at_its_edge() {
    let (before, held, after, mover) = drop_onto(false);
    assert!(
        moved(before, held) < 0.01,
        "held over it: {before:?} {held:?}"
    );
    assert!(
        moved(before, after) < 0.01,
        "dropped on it: {before:?} {after:?}"
    );
    let gap = 6.0;
    let apart = mover.min.x >= after.max.x + gap - 0.5
        || mover.max.x <= after.min.x - gap + 0.5
        || mover.min.y >= after.max.y + gap - 0.5
        || mover.max.y <= after.min.y - gap + 0.5;
    assert!(apart, "not in it: {mover:?} {after:?}");
    // it went out the short way, down, and rests by its edge
    assert!(
        (mover.min.y - (after.max.y + gap)).abs() < 1.0,
        "at its edge: {mover:?} {after:?}"
    );
}

#[test]
fn a_body_that_pushes_moves_its_neighbour_out_of_the_way() {
    let (before, held, _, _) = drop_onto(true);
    assert!(
        moved(before, held) > 20.0,
        "pushed while held over it: {before:?} {held:?}"
    );
}

#[test]
fn a_body_that_does_not_push_grows_no_further_than_its_neighbour() {
    let mut w = world();
    let gap = w.params().gap;
    let above = w.add(
        BodyDesc::new(v(300.0, 100.0), v(300.0, 100.0))
            .home(None)
            .pushes(false),
    );
    let top = 150.0 + gap;
    let under = w.add(BodyDesc::new(v(300.0, top + 50.0), v(300.0, 100.0)).home(None));
    w.settle(2_000);
    let start = rect(&w, under);
    // its content wants 80 points more
    w.set_limits(above, v(300.0, 180.0), v(300.0, 180.0));
    w.set_want(above, v(300.0, 180.0));
    w.settle(10_000);
    assert!(
        moved(start, rect(&w, under)) < 0.01,
        "the one under it stayed: {:?}",
        rect(&w, under)
    );
    assert!(
        w.max_overlap() < 0.5,
        "and they do not overlap: {}",
        w.max_overlap()
    );
}

#[test]
fn a_body_that_does_not_push_and_finds_no_room_goes_back() {
    // dropped in a gap between two columns too narrow for it: it does not stay there, it goes back
    let mut w = world();
    // two columns from the top down to 1200, 112 points between them; the mover is 200 wide: no way out within
    // what it glides in half a second
    let left = w.add(BodyDesc::new(v(150.0, 606.0), v(300.0, 1200.0)).home(None));
    let right = w.add(BodyDesc::new(v(300.0 + 112.0 + 150.0, 606.0), v(300.0, 1200.0)).home(None));
    let mover = w.add(
        BodyDesc::new(v(900.0, 1500.0), v(200.0, 380.0))
            .home(None)
            .pushes(false),
    );
    w.settle(2_000);
    let (l, r, from) = (rect(&w, left), rect(&w, right), rect(&w, mover));
    w.grab(mover);
    for k in 1..=60 {
        let t = k as f32 / 60.0;
        let to = from.center() + (v(356.0, 600.0) - from.center()) * t;
        w.drag_to(mover, to);
        w.step();
    }
    w.release(mover, true);
    assert!(w.settle(20_000).is_some(), "rests");
    assert!(moved(l, rect(&w, left)) < 0.01 && moved(r, rect(&w, right)) < 0.01);
    assert!(
        moved(from, rect(&w, mover)) < 0.5,
        "back where it was: {:?}",
        rect(&w, mover)
    );
}

/// How far body `id` could still slide toward `side` before it meets another body (across from it) or the wall
/// there, less the gap; `None` where nothing stops it that way.
fn slack(w: &World, id: BodyId, side: ve_settle::Side, wall: Option<f32>) -> Option<f32> {
    let gap = w.params().gap;
    let r = rect(w, id);
    let axis_x = matches!(side, ve_settle::Side::Left | ve_settle::Side::Right);
    let lo = side == ve_settle::Side::Left || side == ve_settle::Side::Top;
    let across = |o: &Rect| {
        if axis_x {
            o.max.y > r.min.y + 0.5 && o.min.y < r.max.y - 0.5
        } else {
            o.max.x > r.min.x + 0.5 && o.min.x < r.max.x - 0.5
        }
    };
    let (near, far) = if axis_x {
        (r.min.x, r.max.x)
    } else {
        (r.min.y, r.max.y)
    };
    let gaps = w
        .bodies()
        .iter()
        .filter(|b| b.id != id && across(&b.rect))
        .filter_map(|b| {
            let (omin, omax) = if axis_x {
                (b.rect.min.x, b.rect.max.x)
            } else {
                (b.rect.min.y, b.rect.max.y)
            };
            if lo && omax <= near + 0.5 {
                Some(near - omax - gap)
            } else if !lo && omin >= far - 0.5 {
                Some(omin - far - gap)
            } else {
                None
            }
        });
    let wall = wall.map(|at| if lo { near - at - gap } else { at - far - gap });
    gaps.chain(wall).reduce(f32::min)
}

/// P2605#18oct-1409 (SETL-43): a tidy up toward each side packs every body that way (against the wall, or
/// against what lies before it), then the world holds again and nothing moves; the dash's world is open at the
/// bottom, so down packs against the lowest body's edge.
#[test]
fn a_tidy_up_toward_each_side_packs_that_way_and_holds() {
    use ve_settle::Side;
    for side in [Side::Top, Side::Left, Side::Bottom, Side::Right] {
        let mut w = world();
        let ids = scattered(&mut w);
        w.settle(2_000);
        let bounds = w.bounds();
        let lowest = ids.iter().map(|&id| rect(&w, id).max.y).fold(0.0, f32::max);
        w.settle_once_toward(side);
        assert_eq!(w.settling(), Settling::Once);
        assert!(
            w.settle(20_000).is_some(),
            "{side:?}: the tidy up comes to rest"
        );
        assert_eq!(w.settling(), Settling::Hold, "{side:?}: and holds again");
        assert!(w.max_overlap() < 0.5, "{side:?}: {}", w.max_overlap());
        let wall = match side {
            Side::Top => Some(bounds.min.y),
            Side::Left => Some(bounds.min.x),
            Side::Right => Some(bounds.max.x),
            // the page is open at the bottom: the lowest body's edge stands in for a wall
            Side::Bottom => Some(lowest + w.params().gap),
        };
        for &id in &ids {
            let s = slack(&w, id, side, wall).expect("something stops it");
            assert!(
                s < 1.0,
                "{side:?}: {:?} could still go {s} that way",
                rect(&w, id)
            );
        }
        // it holds: nothing moves afterwards
        let tidy: Vec<Rect> = ids.iter().map(|&id| rect(&w, id)).collect();
        for n in 0..1_200 {
            if n % 200 == 0 {
                w.wake();
            }
            w.step();
        }
        for (i, &id) in ids.iter().enumerate() {
            let m = moved(tidy[i], rect(&w, id));
            assert!(m < 0.01, "{side:?}: body {i} moved {m} after the tidy up");
        }
    }
}
