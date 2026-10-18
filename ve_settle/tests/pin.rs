//! Pinned bodies (SETL-27): frozen in place; the others rest against them as against a wall.

use ve_settle::Settling;
use ve_settle::{BodyDesc, BodyId, Params, Rect, Vec2, World};

fn v(x: f32, y: f32) -> Vec2 {
    Vec2::new(x, y)
}

fn world(p: Params) -> World {
    World::new(Rect::from_min_size(Vec2::ZERO, v(1200.0, 800.0)), p).with_settling(Settling::Always)
}

fn rect(w: &World, id: BodyId) -> Rect {
    w.body(id).expect("body").rect
}

#[test]
fn a_pinned_body_hangs_where_it_was_pinned_and_holds_what_rests_on_it() {
    // gravity up: a pinned panel in the middle, one let go under it rests against it
    let p = Params {
        home_stiffness: 0.0,
        ..Params::default()
    };
    let mut w = world(p);
    let pinned = w.add(BodyDesc::new(v(400.0, 400.0), v(300.0, 120.0)).home(None));
    w.step();
    w.set_pinned(pinned, true);
    let at = rect(&w, pinned);
    let under = w.add(BodyDesc::new(v(400.0, 600.0), v(200.0, 100.0)).home(None));
    assert!(w.settle(6_000).is_some());
    assert_eq!(rect(&w, pinned), at, "the pinned one did not move");
    let u = rect(&w, under);
    assert!(
        (u.min.y - (at.max.y + p.gap)).abs() < 0.05,
        "rests against it: {u:?} {at:?}"
    );
    // unpinned, both rise to the top wall
    w.set_pinned(pinned, false);
    assert!(w.settle(6_000).is_some());
    assert!((rect(&w, pinned).min.y - p.gap).abs() < 0.05);
}

#[test]
fn a_body_dropped_on_a_pinned_one_moves_away_and_the_pinned_one_stays() {
    let p = Params {
        gravity_side: None,
        home_stiffness: 0.0,
        ..Params::default()
    };
    let mut w = world(p);
    let pinned = w.add(BodyDesc::new(v(400.0, 400.0), v(300.0, 120.0)).home(None));
    w.step();
    w.set_pinned(pinned, true);
    let at = rect(&w, pinned);
    // added right over it (lifted): it glides off
    let other = w.add(BodyDesc::new(v(450.0, 420.0), v(200.0, 100.0)).home(None));
    assert!(w.settle(6_000).is_some());
    assert_eq!(rect(&w, pinned), at);
    assert_eq!(w.max_overlap(), 0.0);
    // the gap kept
    let o = rect(&w, other).overlap(&at, p.gap);
    assert!(o.x.min(o.y) <= 0.05, "{o:?}");
}

#[test]
fn a_pinned_body_is_not_grabbed_and_belongs_to_no_group() {
    let p = Params {
        gravity_side: None,
        home_stiffness: 0.0,
        ..Params::default()
    };
    let mut w = world(p);
    let a = w.add(BodyDesc::new(v(400.0, 400.0), v(200.0, 100.0)).home(None));
    let b = w.add(BodyDesc::new(v(400.0 + 206.0, 400.0), v(200.0, 100.0)).home(None));
    assert!(w.settle(6_000).is_some());
    assert_eq!(w.cluster(a).len(), 2);
    w.set_pinned(b, true);
    assert_eq!(w.cluster(a), vec![a], "the pinned one stays behind");
    assert_eq!(w.cluster(b), vec![b]);
    w.grab(b);
    assert!(!w.body(b).unwrap().dragged, "not grabbed while pinned");
}
