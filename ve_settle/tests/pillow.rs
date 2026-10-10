//! Polygon contacts and the pillow: bodies pressed too hard in a row or
//! column slide off each other instead of being crushed (SETL-9).

use ve_settle::{BodyDesc, Params, Poly, Rect, Vec2, World, sat};

fn v(x: f32, y: f32) -> Vec2 {
    Vec2::new(x, y)
}

fn world(w: f32, h: f32, params: Params) -> World {
    World::new(Rect::from_min_size(Vec2::ZERO, v(w, h)), params)
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

/// The farthest anything may move in one step with these parameters.
fn glide(p: &Params) -> f32 {
    (p.max_speed + p.glide_speed.max(p.grow_speed)) * p.dt
}

fn close(a: Vec2, b: Vec2) -> bool {
    (a - b).max_abs() < 1e-3
}

#[test]
fn a_pillow_without_bulge_is_the_rectangle() {
    let r = Rect::from_min_size(v(10.0, 20.0), v(100.0, 50.0));
    let p = Poly::pillow(&r, Vec2::ZERO);
    assert_eq!(p.points(), Poly::rect(&r).points());
    assert_eq!(p.points().len(), 4);

    let p = Poly::pillow(&r, v(4.0, 2.0));
    assert_eq!(p.points().len(), 8);
    // The middle of each edge stands out by its bulge, the corners stay.
    let pts = p.points();
    assert!(pts.iter().any(|&q| close(q, v(60.0, 18.0))), "top");
    assert!(pts.iter().any(|&q| close(q, v(114.0, 45.0))), "right");
    assert!(pts.iter().any(|&q| close(q, v(60.0, 72.0))), "bottom");
    assert!(pts.iter().any(|&q| close(q, v(6.0, 45.0))), "left");
    assert!(pts.iter().any(|&q| close(q, r.max)), "corner");
}

#[test]
fn sat_on_rectangles_takes_the_axis_of_least_overlap() {
    let a = Poly::rect(&Rect::from_min_size(v(0.0, 0.0), v(100.0, 50.0)));
    let b = Poly::rect(&Rect::from_min_size(v(90.0, 10.0), v(100.0, 50.0)));
    let hit = sat(&a, &b, |_| 0.0, Vec2::ZERO).expect("they overlap");
    assert!(close(hit.normal, v(1.0, 0.0)), "{hit:?}");
    assert!((hit.depth - 10.0).abs() < 1e-3);
    // The gap counts as overlap.
    let hit = sat(&a, &b, |_| 6.0, Vec2::ZERO).unwrap();
    assert!((hit.depth - 16.0).abs() < 1e-3);
    // Apart by more than the gap: no contact.
    let far = b.translate(v(30.0, 0.0));
    assert!(sat(&a, &far, |_| 6.0, Vec2::ZERO).is_none());
}

#[test]
fn sat_works_on_hexagons() {
    // Two flat-topped hexagons side by side touch along a slanted edge
    // when one is lower: the normal is that edge's, 30° off the axis.
    let a = Poly::hexagon(v(0.0, 0.0), 50.0);
    let width = 50.0 * 3f32.sqrt();
    let b = Poly::hexagon(v(75.0, width / 2.0 - 2.0), 50.0);
    let hit = sat(&a, &b, |_| 0.0, Vec2::ZERO).expect("they touch");
    let slanted = v(30f32.to_radians().cos(), 30f32.to_radians().sin());
    assert!(close(hit.normal, slanted), "{hit:?}");
    assert!(hit.depth > 0.0 && hit.depth < 3.0, "{hit:?}");
    // Moving `b` along the normal by the depth parts them.
    let parted = b.translate(hit.normal * (hit.depth + 0.01));
    assert!(sat(&a, &parted, |_| 0.0, Vec2::ZERO).is_none());
}

#[test]
fn two_tips_tied_go_the_preferred_way_or_split() {
    let bulge = v(10.0, 0.0);
    let a = Poly::pillow(&Rect::from_min_size(v(0.0, 0.0), v(100.0, 100.0)), bulge);
    let b = Poly::pillow(&Rect::from_min_size(v(115.0, 0.0), v(100.0, 100.0)), bulge);
    // Tip to tip, 5 points in each other: the up and the down slope tie.
    let down = sat(&a, &b, |_| 0.0, v(0.0, 1.0)).unwrap();
    assert!(down.normal.x > 0.9 && down.normal.y > 0.1, "{down:?}");
    let up = sat(&a, &b, |_| 0.0, v(0.0, -1.0)).unwrap();
    assert!(up.normal.x > 0.9 && up.normal.y < -0.1, "{up:?}");
    let split = sat(&a, &b, |_| 0.0, Vec2::ZERO).unwrap();
    assert!(close(split.normal, v(1.0, 0.0)), "{split:?}");
}

/// Four panels in a row that fills a 1000 wide world, gravity up; then
/// the walls move in to 800, where only three fit.
fn overfilled_row(params: Params, min_width: f32) -> World {
    let mut w = world(1000.0, 800.0, params);
    for i in 0..4 {
        w.add(
            BodyDesc::new(v(127.0 + 236.0 * i as f32, 90.0), v(230.0, 150.0))
                .min_size(v(min_width, 150.0)),
        );
    }
    w.settle(20_000).expect("the full row settles");
    assert!(w.bodies().iter().all(|b| b.rect.min.y < 7.0), "one row");
    w.set_bounds(Rect::from_min_size(Vec2::ZERO, v(800.0, 800.0)));
    settle_checked(&mut w, 20_000, glide(&params));
    w
}

fn assert_one_moved_to_the_next_row(w: &World) {
    let below: Vec<_> = w.bodies().iter().filter(|b| b.rect.min.y > 150.0).collect();
    assert_eq!(below.len(), 1, "exactly one leaves: {:#?}", w.bodies());
    for b in w.bodies() {
        assert!(b.tension() < 0.01, "not crushed: {b:?}");
        assert!(b.rect.max.x <= 794.5, "inside the walls: {b:?}");
        if b.id != below[0].id {
            assert!(b.rect.min.y < 6.5, "the others stay in the row: {b:?}");
        }
    }
}

#[test]
fn a_row_overfilled_by_one_widget_sends_one_to_the_next_row() {
    // Squeezable panels with home springs: the sandbox's setting.
    let w = overfilled_row(Params::default(), 150.0);
    assert_one_moved_to_the_next_row(&w);
}

#[test]
fn rigid_panels_in_an_overfilled_row_slide_off_too() {
    let p = Params {
        tension: 0.0,
        home_stiffness: 0.0,
        ..Params::default()
    };
    let w = overfilled_row(p, 230.0);
    assert_one_moved_to_the_next_row(&w);
}

#[test]
fn without_bulge_an_overfilled_row_is_squeezed_as_before() {
    let p = Params {
        bulge_gain: 0.0,
        ..Params::default()
    };
    let w = overfilled_row(p, 150.0);
    assert!(w.bodies().iter().all(|b| b.rect.min.y < 6.5));
    assert!(w.bodies().iter().all(|b| b.tension() > 0.05));
}

#[test]
fn an_overfilled_column_spills_to_the_side() {
    let p = Params::default();
    let mut w = world(600.0, 500.0, p);
    for i in 0..3 {
        w.add(
            BodyDesc::new(v(131.0, 81.0 + 156.0 * i as f32), v(250.0, 150.0))
                .min_size(v(250.0, 80.0)),
        );
    }
    w.settle(20_000).expect("the full column settles");
    w.set_bounds(Rect::from_min_size(Vec2::ZERO, v(600.0, 400.0)));
    settle_checked(&mut w, 20_000, glide(&p));
    let beside = w.bodies().iter().filter(|b| b.rect.min.x > 250.0).count();
    assert_eq!(beside, 1, "{:#?}", w.bodies());
    assert!(w.bodies().iter().all(|b| b.tension() < 0.01));
}

#[test]
fn a_calm_row_stays_put_and_lined_up() {
    // Three panels with room to spare: no pressure, so no bulge, and the
    // row rests exactly where it would as plain rectangles.
    let run = |bulge_gain| {
        let p = Params {
            bulge_gain,
            home_stiffness: 0.0,
            ..Params::default()
        };
        let mut w = world(1000.0, 600.0, p);
        for i in 0..3 {
            w.add(BodyDesc::new(
                v(150.0 + 260.0 * i as f32, 100.0 + 7.0 * i as f32),
                v(240.0, 140.0 + 20.0 * i as f32),
            ));
        }
        settle_checked(&mut w, 20_000, glide(&p));
        w
    };
    let (pillow, plain) = (run(Params::default().bulge_gain), run(0.0));
    for (a, b) in pillow.bodies().iter().zip(plain.bodies()) {
        assert_eq!(a.bulge, Vec2::ZERO, "calm bodies do not bulge: {a:?}");
        assert!((a.rect.min - b.rect.min).max_abs() < 0.01, "{a:?} vs {b:?}");
        assert!((a.rect.min.y - 6.0).abs() < 0.5, "under the top wall");
    }
    let mut w = pillow;
    let frozen = w.bodies().to_vec();
    for _ in 0..300 {
        assert!(!w.step(), "a calm row does nothing");
    }
    assert_eq!(w.bodies(), &frozen[..]);
}

#[test]
fn a_rigid_pile_spills_sideways_and_stays_inside_the_walls() {
    // SETL-6: eight rigid panels added on a diagonal used to part into one
    // column twice the height of the world.
    let p = Params {
        tension: 0.0,
        ..Params::default()
    };
    let mut w = world(900.0, 600.0, p);
    for i in 0..8 {
        let f = i as f32;
        w.add(BodyDesc::new(
            v(300.0 + 10.0 * f, 200.0 + 10.0 * f),
            v(200.0, 120.0),
        ));
    }
    w.settle(20_000).expect("settles");
    assert_eq!(w.max_overlap(), 0.0);
    for b in w.bodies() {
        let r = b.rect;
        assert!(
            r.min.x >= 5.5 && r.min.y >= 5.5 && r.max.x <= 894.5 && r.max.y <= 594.5,
            "inside the walls: {b:?}"
        );
    }
}

/// Rows of the bodies by their tops: a new row starts where a top is more
/// than half a row below the first top of the row before.
fn rows_of(w: &World, half_row: f32) -> Vec<usize> {
    let mut tops: Vec<(f32, usize)> = w
        .bodies()
        .iter()
        .enumerate()
        .map(|(i, b)| (b.rect.min.y, i))
        .collect();
    tops.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut row_of = vec![0; tops.len()];
    let (mut row, mut first) = (0, tops[0].0);
    for (top, i) in tops {
        if top > first + half_row {
            row += 1;
            first = top;
        }
        row_of[i] = row;
    }
    row_of
}

#[test]
fn a_packed_field_gives_way_row_by_row_without_crushing() {
    // SETL-11: six rows of five where four and a half fit, the bottom
    // open, as a full dash. Every row overflows; the rows after each
    // overflow shift on together, so nobody stays squeezed and nobody
    // dives past the panels that started below it.
    let mut p = Params::default();
    p.walls.bottom = false;
    let (width, size) = (4.5 * 306.0 + 6.0, v(300.0, 180.0));
    let mut w = world(width, 900.0, p);
    let dx = width / 5.0;
    for i in 0..30 {
        let (row, col) = (i / 5, i % 5);
        let at = v(dx * (col as f32 + 0.5), 6.0 + 186.0 * (row as f32 + 0.5));
        w.add(BodyDesc::new(at, size).min_size(v(200.0, 120.0)));
    }
    settle_checked(&mut w, 3_000, glide(&p));
    for b in w.bodies() {
        assert!(b.tension() < 0.01, "not squeezed: {b:?}");
        assert!(b.rect.min.x >= 5.5 && b.rect.max.x <= width - 5.5, "{b:?}");
    }
    // A panel that started in a row above another ends at most one row
    // below it.
    let row = rows_of(&w, 90.0);
    for a in 0..30 {
        for b in 0..30 {
            if a / 5 < b / 5 {
                assert!(
                    row[a] <= row[b] + 1,
                    "#{a} passed #{b} by {} rows",
                    row[a] - row[b]
                );
            }
        }
    }
}
