//! The solver on its own: forces, contacts, squeeze, rest, determinism (SETL-1).

use ve_settle::{Axis, BodyDesc, BodyId, Params, Rect, Side, Vec2, World};

fn v(x: f32, y: f32) -> Vec2 {
    Vec2::new(x, y)
}

fn world(w: f32, h: f32, params: Params) -> World {
    World::new(Rect::from_min_size(Vec2::ZERO, v(w, h)), params)
}

/// No gravity and no homes unless a test turns them on.
fn calm() -> Params {
    Params {
        gravity_side: None,
        home_stiffness: 0.0,
        ..Params::default()
    }
}

fn rect(w: &World, id: BodyId) -> Rect {
    w.body(id).expect("body exists").rect
}

/// Step until at rest, checking on every step that solid bodies never
/// overlap and nothing moves farther than `max_move` in one step.
fn settle_checked(w: &mut World, max_steps: u32, max_move: f32) -> u32 {
    for n in 0..max_steps {
        if !w.step() {
            return n;
        }
        assert_eq!(w.max_overlap(), 0.0, "overlap at step {n}");
        assert!(
            w.last_move() <= max_move,
            "a body jumped {} points at step {n}",
            w.last_move()
        );
    }
    panic!("still moving after {max_steps} steps");
}

/// The farthest anything may move in one step with these parameters: at
/// the speed limit, plus being pushed out of the way.
fn glide(p: &Params) -> f32 {
    (p.max_speed + p.glide_speed.max(p.grow_speed)) * p.dt
}

fn assert_size(w: &World, id: BodyId, want: Vec2) {
    let size = rect(w, id).size();
    assert!(
        (size - want).max_abs() < 1e-3,
        "size {size:?}, expected {want:?}"
    );
}

#[test]
fn gravity_pulls_to_each_side_and_walls_hold() {
    for side in Side::ALL {
        let p = Params {
            gravity_side: Some(side),
            home_stiffness: 0.0,
            ..Params::default()
        };
        let mut w = world(400.0, 300.0, p);
        let id = w.add(BodyDesc::new(v(200.0, 150.0), v(80.0, 60.0)));
        settle_checked(&mut w, 2_000, glide(&p));
        let r = rect(&w, id);
        let (edge, wall) = match side {
            Side::Top => (r.min.y, p.gap),
            Side::Bottom => (r.max.y, 300.0 - p.gap),
            Side::Left => (r.min.x, p.gap),
            Side::Right => (r.max.x, 400.0 - p.gap),
        };
        assert!(
            (edge - wall).abs() < 0.5,
            "{side:?}: edge {edge}, wall {wall}"
        );
        assert_size(&w, id, v(80.0, 60.0));
    }
}

#[test]
fn bodies_stack_in_order_with_the_gap_between_them() {
    let p = Params {
        home_stiffness: 0.0,
        ..Params::default()
    };
    let mut w = world(300.0, 900.0, p);
    let ids: Vec<_> = (0..6)
        .map(|i| {
            w.add(BodyDesc::new(
                v(150.0, 80.0 + 130.0 * i as f32),
                v(200.0, 70.0),
            ))
        })
        .collect();
    settle_checked(&mut w, 4_000, glide(&p));
    let mut y = 0.0;
    for id in ids {
        let r = rect(&w, id);
        assert!(
            (r.min.y - (y + p.gap)).abs() < 0.5,
            "top {} after {y}",
            r.min.y
        );
        y = r.max.y;
    }
    assert!(w.max_gap_violation() < 0.1);
}

#[test]
fn home_spring_brings_a_body_back_to_its_spot() {
    let p = Params {
        gravity_side: None,
        ..Params::default()
    };
    let mut w = world(600.0, 400.0, p);
    let id = w.add(BodyDesc::new(v(100.0, 100.0), v(60.0, 40.0)).home(Some(v(400.0, 250.0))));
    settle_checked(&mut w, 4_000, glide(&p));
    let c = rect(&w, id).center();
    assert!((c - v(400.0, 250.0)).length() < 1.0, "rests at {c:?}");
}

#[test]
fn gravity_and_home_balance() {
    // At rest the spring's pull equals gravity: gravity / stiffness away from home.
    let p = Params {
        gravity: 300.0,
        home_stiffness: 30.0,
        ..Params::default()
    };
    let mut w = world(600.0, 400.0, p);
    let id = w.add(BodyDesc::new(v(300.0, 200.0), v(60.0, 40.0)));
    settle_checked(&mut w, 4_000, glide(&p));
    let c = rect(&w, id).center();
    assert!(
        (c.y - 190.0).abs() < 1.0 && (c.x - 300.0).abs() < 0.1,
        "{c:?}"
    );
}

#[test]
fn a_neighbour_that_grows_and_shrinks_leaves_a_body_where_it_was() {
    let p = Params {
        gravity_side: None,
        ..Params::default()
    };
    let mut w = world(600.0, 600.0, p);
    let grower = w.add(BodyDesc::new(v(300.0, 100.0), v(200.0, 80.0)));
    let other = w.add(BodyDesc::new(v(300.0, 200.0), v(200.0, 80.0)));
    settle_checked(&mut w, 4_000, glide(&p));
    let before = rect(&w, other);

    w.set_want(grower, v(200.0, 240.0));
    settle_checked(&mut w, 4_000, glide(&p));
    let pushed = rect(&w, other);
    assert!(
        pushed.min.y > before.min.y + 30.0,
        "pushed down: {pushed:?}"
    );
    assert!((rect(&w, grower).size().y - 240.0).abs() < 0.5);

    w.set_want(grower, v(200.0, 80.0));
    settle_checked(&mut w, 4_000, glide(&p));
    let after = rect(&w, other);
    assert!(
        (after.min - before.min).length() < 1.0,
        "{before:?} → {after:?}"
    );
}

#[test]
fn size_stays_within_min_and_max() {
    let mut w = world(600.0, 600.0, calm());
    let id = w.add(
        BodyDesc::new(v(300.0, 300.0), v(100.0, 100.0))
            .min_size(v(50.0, 50.0))
            .max_size(v(150.0, 120.0)),
    );
    w.set_want(id, v(500.0, 500.0));
    w.settle(4_000).expect("settles");
    assert_size(&w, id, v(150.0, 120.0));
    w.set_want(id, v(10.0, 10.0));
    w.settle(4_000).expect("settles");
    assert_size(&w, id, v(50.0, 50.0));
}

/// Three bodies wanting 3 × 120 in a 300 high box.
fn tight_stack(tension: f32) -> (World, Vec<BodyId>) {
    let p = Params {
        home_stiffness: 0.0,
        tension,
        ..Params::default()
    };
    let mut w = world(300.0, 300.0, p);
    let ids = (0..3)
        .map(|i| {
            w.add(
                BodyDesc::new(v(150.0, 60.0 + 100.0 * i as f32), v(200.0, 120.0))
                    .min_size(v(200.0, 60.0)),
            )
        })
        .collect();
    (w, ids)
}

#[test]
fn bodies_without_room_are_squeezed_but_not_below_their_minimum() {
    let (mut w, ids) = tight_stack(0.5);
    w.settle(20_000).expect("settles");
    assert_eq!(w.max_overlap(), 0.0);
    let mut total = 0.0;
    for id in &ids {
        let b = w.body(*id).unwrap();
        let h = b.rect.size().y;
        assert!((60.0 - 1e-3..119.0).contains(&h), "height {h}");
        assert!(b.tension() > 0.01, "the overlay has a tension to show");
        assert!(
            (b.rect.size().x - 200.0).abs() < 1e-3,
            "only the pressed axis gives"
        );
        total += h;
    }
    // Squeezing took back most of the 84 points that did not fit; what
    // restore keeps giving back sticks out past the walls.
    let room = 300.0 - 4.0 * 6.0;
    assert!(total < room + 30.0, "total {total}, room {room}");
    // More room: the squeeze goes away.
    w.set_bounds(Rect::from_min_size(Vec2::ZERO, v(300.0, 600.0)));
    w.settle(20_000).expect("settles");
    for id in &ids {
        assert!((rect(&w, *id).size().y - 120.0).abs() < 0.5);
    }
}

#[test]
fn tension_zero_is_rigid() {
    let (mut w, ids) = tight_stack(0.0);
    w.settle(20_000).expect("settles");
    assert_eq!(w.max_overlap(), 0.0, "bodies win over walls");
    for id in ids {
        assert_size(&w, id, v(200.0, 120.0));
    }
}

#[test]
fn more_tension_squeezes_more() {
    let total = |tension| {
        let (mut w, ids) = tight_stack(tension);
        w.settle(20_000).expect("settles");
        ids.iter().map(|id| rect(&w, *id).size().y).sum::<f32>()
    };
    let (soft, firm) = (total(0.9), total(0.1));
    assert!(soft < firm, "tension 0.9 → {soft}, 0.1 → {firm}");
}

#[test]
fn at_rest_nothing_moves_until_something_changes() {
    let mut w = world(400.0, 400.0, Params::default());
    let a = w.add(BodyDesc::new(v(100.0, 100.0), v(80.0, 60.0)));
    w.add(BodyDesc::new(v(120.0, 180.0), v(80.0, 60.0)));
    w.settle(4_000).expect("settles");
    assert!(w.is_at_rest());
    let (frozen, steps) = (w.bodies().to_vec(), w.steps());
    for _ in 0..100 {
        assert!(!w.step());
        assert_eq!(w.advance(1.0 / 60.0), 0);
    }
    assert_eq!(w.bodies(), &frozen[..]);
    assert_eq!(w.steps(), steps);

    // Telling it what it already knows does not wake it.
    w.set_want(a, v(80.0, 60.0));
    w.set_params(Params::default());
    w.set_bounds(w.bounds());
    assert!(w.is_at_rest());
    // A real change does.
    w.set_want(a, v(80.0, 90.0));
    assert!(!w.is_at_rest());
    assert!(w.step());
}

/// A scripted session touching every kind of change.
fn script() -> World {
    let mut w = world(500.0, 400.0, Params::default());
    let mut ids = Vec::new();
    for i in 0..7 {
        let f = i as f32;
        ids.push(
            w.add(
                BodyDesc::new(
                    v(60.0 + 61.0 * f, 50.0 + 37.0 * f),
                    v(70.0 + 9.0 * f, 40.0 + 13.0 * f),
                )
                .min_size(v(40.0, 30.0)),
            ),
        );
    }
    for n in 0..1_500u32 {
        match n {
            100 => w.set_want(ids[2], v(200.0, 160.0)),
            300 => w.grab(ids[4]),
            301..=420 => w.drag_to(
                ids[4],
                v(100.0 + n as f32 - 300.0, 300.0 - (n - 300) as f32),
            ),
            421 => w.release(ids[4], true),
            600 => {
                w.remove(ids[1]);
            }
            700 => w.set_bounds(Rect::from_min_size(Vec2::ZERO, v(380.0, 330.0))),
            900 => w.set_params(Params {
                gravity_side: Some(Side::Left),
                ..Params::default()
            }),
            _ => {}
        }
        if n % 3 == 0 {
            w.advance(1.0 / 60.0);
        } else {
            w.step();
        }
    }
    w
}

#[test]
fn the_same_calls_give_the_same_world_bit_for_bit() {
    let (a, b) = (script(), script());
    assert_eq!(a.bodies(), b.bodies());
    assert_eq!(a.steps(), b.steps());
    assert_eq!(a.contacts(), b.contacts());
}

#[test]
fn a_body_added_on_top_of_others_makes_room_without_a_jump() {
    let p = Params::default();
    let mut w = world(500.0, 500.0, p);
    for i in 0..4 {
        w.add(BodyDesc::new(
            v(100.0 + 90.0 * i as f32, 100.0),
            v(80.0, 80.0),
        ));
    }
    settle_checked(&mut w, 4_000, glide(&p));
    let new = w.add(BodyDesc::new(v(190.0, 100.0), v(120.0, 90.0)));
    assert!(w.body(new).unwrap().lifted, "starts lifted");
    settle_checked(&mut w, 8_000, glide(&p));
    assert!(w.bodies().iter().all(|b| !b.lifted), "everything landed");
    assert!(w.max_gap_violation() < 0.1);
}

#[test]
fn dragging_through_neighbours_moves_them_aside_without_overlap_or_jumps() {
    let p = Params::default();
    let mut w = world(500.0, 500.0, p);
    let ids: Vec<_> = (0..4)
        .map(|i| {
            w.add(BodyDesc::new(
                v(250.0, 60.0 + 100.0 * i as f32),
                v(200.0, 80.0),
            ))
        })
        .collect();
    settle_checked(&mut w, 4_000, glide(&p));

    let start = rect(&w, ids[3]).center();
    w.grab(ids[3]);
    for n in 0..=240 {
        // Up through the whole stack in two seconds.
        w.drag_to(ids[3], v(start.x, start.y - 330.0 * n as f32 / 240.0));
        assert!(w.step());
        assert_eq!(
            w.max_overlap(),
            0.0,
            "solid bodies overlap at drag step {n}"
        );
        assert!(
            w.last_move() <= glide(&p),
            "jump of {} at {n}",
            w.last_move()
        );
        assert_eq!(
            rect(&w, ids[3]).center().x,
            start.x,
            "the held body is where it is held"
        );
    }
    w.release(ids[3], true);
    settle_checked(&mut w, 8_000, glide(&p));
    assert!(w.bodies().iter().all(|b| !b.lifted && !b.dragged));
    assert!(w.max_gap_violation() < 0.1);
    // Dropped near the top, it now lives there.
    let home = w.body(ids[3]).unwrap().home.expect("rehomed");
    assert!(home.y < 80.0, "home {home:?}");
}

#[test]
fn walls_that_move_in_make_bodies_glide_not_jump() {
    let p = Params {
        gravity_side: Some(Side::Right),
        home_stiffness: 0.0,
        ..Params::default()
    };
    let mut w = world(800.0, 300.0, p);
    for i in 0..3 {
        w.add(BodyDesc::new(
            v(700.0, 60.0 + 90.0 * i as f32),
            v(100.0, 70.0),
        ));
    }
    settle_checked(&mut w, 4_000, glide(&p));
    w.set_bounds(Rect::from_min_size(Vec2::ZERO, v(500.0, 300.0)));
    settle_checked(&mut w, 4_000, glide(&p));
    for b in w.bodies() {
        assert!(
            (b.rect.max[Axis::X] - (500.0 - p.gap)).abs() < 0.5,
            "{:?}",
            b.rect
        );
    }
}

#[test]
fn an_open_side_lets_bodies_through() {
    let mut p = Params {
        gravity_side: Some(Side::Bottom),
        home_stiffness: 0.0,
        ..Params::default()
    };
    p.walls.set(Side::Bottom, false);
    let mut w = world(300.0, 300.0, p);
    let id = w.add(BodyDesc::new(v(150.0, 150.0), v(80.0, 60.0)));
    for _ in 0..600 {
        w.step();
    }
    assert!(
        rect(&w, id).min.y > 300.0,
        "fell out through the open bottom"
    );
}

#[test]
fn drifting_homes_follow_a_body_that_was_pushed_away_for_long() {
    let p = Params {
        gravity_side: None,
        home_drift: 2.0,
        ..Params::default()
    };
    let mut w = world(600.0, 600.0, p);
    let grower = w.add(BodyDesc::new(v(300.0, 100.0), v(200.0, 80.0)));
    let other = w.add(BodyDesc::new(v(300.0, 200.0), v(200.0, 80.0)));
    w.set_want(grower, v(200.0, 240.0));
    for _ in 0..2_400 {
        w.step();
    }
    let b = w.body(other).unwrap();
    let home = b.home.expect("has a home");
    assert!(
        (home - b.rect.center()).length() < 20.0,
        "home {home:?} near {:?}",
        b.rect.center()
    );
    assert!(
        home.y > 230.0,
        "the home moved down with the body: {home:?}"
    );
}
