//! Free cells and the honeycomb (SETL-28): hexagons laid out cell by cell nest face to face, the pack
//! rests as it was laid out, and a hexagon dropped near the pack glides into a free cell.

// A native-only test; `dump`'s SVG write (behind the `NEST_SVG` env var, for debugging by hand) never
// runs on the web. `ve_settle`'s lib stays wasm-gated (PLT-9).
#![allow(clippy::disallowed_methods)]

use ve_settle::{BodyDesc, BodyId, Params, Poly, Rect, Side, Vec2, World};

fn v(x: f32, y: f32) -> Vec2 {
    Vec2::new(x, y)
}

/// A hexagon frame's outline (flat top and bottom, the faces at 60°), as `ve_settle::frames` makes it.
fn hex_outline(size: Vec2) -> Poly {
    let s = (size.y / (2.0 * 3f32.sqrt())).min(size.x * 0.49) / size.x;
    Poly::new(&[
        v(s, 0.0),
        v(1.0 - s, 0.0),
        v(1.0, 0.5),
        v(1.0 - s, 1.0),
        v(s, 1.0),
        v(0.0, 0.5),
    ])
}

/// The sizes of the sandbox's hexagon frames: three share the width of their flat faces, the big one
/// is wider.
const SIZES: [(f32, f32); 4] = [
    (318.4, 156.0),
    (276.3, 86.0),
    (304.5, 132.0),
    (476.1, 236.0),
];

/// The sandbox's scene: gravity to the top, the bottom open, rigid frames.
fn params() -> Params {
    let mut p = Params {
        home_stiffness: 0.0,
        push_out: true,
        ..Params::default()
    };
    p.max_speed = p.glide_speed;
    p.walls.set(Side::Bottom, false);
    p
}

fn hexagon(w: &mut World, size: Vec2, at: Vec2) -> BodyId {
    w.add(
        BodyDesc::new(at, size)
            .min_size(size)
            .max_size(size)
            .outline(hex_outline(size)),
    )
}

/// `n` hexagons of the sandbox's sizes laid out one by one into the cell a pack takes next.
fn packed(width: f32, n: usize, sizes: &[(f32, f32)]) -> (World, Vec<BodyId>) {
    packed_giving(width, n, sizes, 0.0)
}

/// [`packed`], the tiny ones (86 points tall) free to narrow by `give` points: a frame on auto size
/// (SETL-5) is not rigid across, so it lines up by sliding even in a notch.
fn packed_giving(width: f32, n: usize, sizes: &[(f32, f32)], give: f32) -> (World, Vec<BodyId>) {
    let mut w = World::new(Rect::from_min_size(Vec2::ZERO, v(width, 800.0)), params());
    let mut ids = Vec::new();
    for i in 0..n {
        let (sx, sy) = sizes[i % sizes.len()];
        let size = v(sx, sy);
        let id = hexagon(&mut w, size, v(width / 2.0, 5000.0 + 400.0 * i as f32));
        if sy < 100.0 && give > 0.0 {
            w.set_limits(id, v(sx - give, sy), size);
        }
        let at = w.pack_cell(id, &[]).expect("a cell");
        w.put(id, at, size);
        ids.push(id);
    }
    (w, ids)
}

/// How many faces of `id` lie against a face of another body: antiparallel, the gap apart within
/// `slack`, side by side for at least a quarter of the shorter.
fn faces_against(w: &World, id: BodyId, slack: f32) -> usize {
    let gap = w.params().gap;
    let me = w.body(id).expect("body").shape();
    let faces = |p: &Poly| {
        let pts = p.points().to_vec();
        let mid = p.centroid();
        (0..pts.len())
            .map(|i| {
                let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
                let len = (b - a).length();
                let u = (b - a) * (1.0 / len);
                let mut n = v(-u.y, u.x);
                if (a - mid).dot(n) < 0.0 {
                    n = -n;
                }
                (a, b, u, n, len)
            })
            .collect::<Vec<_>>()
    };
    let mut count = 0;
    for (a0, a1, u, n, la) in faces(&me) {
        let against = w.bodies().iter().filter(|o| o.id != id).any(|o| {
            faces(&o.shape()).into_iter().any(|(b0, b1, ub, nb, lb)| {
                if u.dot(ub) > -0.98 || n.dot(nb) > -0.98 {
                    return false;
                }
                let dist = (b0 - a0).dot(n);
                let (s0, s1) = (a0.dot(u), a1.dot(u));
                let (t0, t1) = (b0.dot(u), b1.dot(u));
                let overlap = s0.max(s1).min(t0.max(t1)) - s0.min(s1).max(t0.min(t1));
                (dist - gap).abs() <= slack && overlap >= 0.25 * la.min(lb)
            })
        });
        if against {
            count += 1;
        }
    }
    count
}

/// Hexagons of one size laid out cell by cell are a honeycomb: every one after the first lies face to
/// face with another, most with two or more, the first row at the wall.
#[test]
fn hexagons_of_one_size_pack_as_a_honeycomb() {
    let (w, ids) = packed(1200.0, 12, &SIZES[..1]);
    dump(&w, "one");
    for &id in &ids[1..] {
        assert!(faces_against(&w, id, 0.5) >= 1, "#{} alone", id.0);
    }
    let paired = ids
        .iter()
        .filter(|&&id| faces_against(&w, id, 0.5) >= 2)
        .count();
    assert!(paired >= 8, "{paired} of 12 with two faces against others");
    assert_eq!(w.max_overlap(), 0.0);
}

fn dump(w: &World, name: &str) {
    let Ok(dir) = std::env::var("NEST_SVG") else {
        return;
    };
    let mut s = String::from(
        "<svg xmlns='http://www.w3.org/2000/svg' viewBox='-20 -20 1640 1640' width='820' height='820'><rect x='-20' y='-20' width='1640' height='1640' fill='#111'/>",
    );
    for b in w.bodies() {
        let pts: Vec<String> = b
            .shape()
            .points()
            .iter()
            .map(|p| format!("{},{}", p.x, p.y))
            .collect();
        s += &format!(
            "<polygon points='{}' fill='#234' stroke='#8af'/><text x='{}' y='{}' fill='#fff' font-size='24'>{}</text>",
            pts.join(" "),
            b.rect.center().x,
            b.rect.center().y,
            b.id.0
        );
    }
    s += "</svg>";
    let _ = std::fs::write(format!("{dir}/{name}.svg"), s);
}

/// The sandbox's four sizes laid out cell by cell: every one after the first lies face to face with
/// another, and the pack, let go, rests where it was laid out (gravity presses each into the faces it
/// lies on, which hold it: SETL-15), give or take a slide along a face that lines a vertex up.
#[test]
fn hexagons_of_the_sandboxs_sizes_pack_and_rest_as_laid_out() {
    let (mut w, ids) = packed(1200.0, 21, &SIZES);
    dump(&w, "mixed");
    for &id in &ids[1..] {
        assert!(faces_against(&w, id, 0.5) >= 1, "#{} alone", id.0);
    }
    let laid: Vec<Vec2> = ids
        .iter()
        .map(|&id| w.body(id).unwrap().rect.center())
        .collect();
    for n in 0..8_000 {
        if !w.step() {
            break;
        }
        assert!(w.max_overlap() < 0.01, "overlap at step {n}");
    }
    dump(&w, "mixed_rest");
    assert!(w.is_at_rest());
    for (&id, at) in ids.iter().zip(&laid) {
        let moved = (w.body(id).unwrap().rect.center() - *at).length();
        // (vertex attraction may slide one along a face to line a vertex up: SETL-20)
        assert!(moved <= w.params().attract_range, "#{} moved {moved}", id.0);
        if id != ids[0] {
            assert!(faces_against(&w, id, 1.0) >= 1, "#{} alone at rest", id.0);
        }
    }
}

/// A hexagon dropped near a free cell of the honeycomb glides into it (a glide a step, the others stay
/// where they are) and lies face to face there; dropped farther than `nest_range` from any cell it is let go as
/// usual.
#[test]
fn a_hexagon_dropped_near_the_pack_glides_into_a_free_cell() {
    let (mut w, ids) = packed(1200.0, 10, &SIZES[..1]);
    while w.step() {}
    let before: Vec<Vec2> = ids
        .iter()
        .map(|&id| w.body(id).unwrap().rect.center())
        .collect();
    let size = v(SIZES[0].0, SIZES[0].1);
    let id = hexagon(&mut w, size, v(600.0, 3000.0));
    let cell = w.pack_cell(id, &[]).expect("a free cell");
    // straight out of the notch, away from gravity: lying in nothing (a held body pushes what it is
    // dragged into, SETL-25)
    let dropped = cell + v(0.0, 40.0);
    w.grab(id);
    w.drag_to(id, dropped);
    w.step();
    let at = w.release_to_cell(id, false).expect("near a cell");
    // (the pack settled by a point here and there: the notch is where it is now)
    assert!(
        (at - cell).length() < 2.0,
        "the nearest cell: {at:?}, {cell:?}"
    );
    let mut steps = 0;
    let mut last = w.body(id).unwrap().rect.center();
    while w.step() {
        steps += 1;
        assert!(steps < 4000, "rests");
        let now = w.body(id).unwrap().rect.center();
        let moved = (now - last).length();
        last = now;
        assert!(
            moved <= w.params().glide_speed * w.params().dt + 0.01,
            "a glide a step, not {moved}"
        );
    }
    let b = w.body(id).unwrap();
    assert!(
        (b.rect.center() - at).length() < 1.0,
        "in the cell: {:?}",
        b.rect
    );
    assert!(!b.dragged && !b.lifted && b.nesting().is_none());
    assert!(faces_against(&w, id, 1.0) >= 2);
    for (&o, at) in ids.iter().zip(&before) {
        let moved = (w.body(o).unwrap().rect.center() - *at).length();
        assert!(moved < 0.5, "#{} moved {moved}", o.0);
    }

    // far from any cell: an ordinary drop
    let far = hexagon(&mut w, size, v(600.0, 3000.0));
    w.grab(far);
    w.drag_to(far, v(600.0, 2500.0));
    w.step();
    assert_eq!(w.release_to_cell(far, false), None);
    assert!(!w.body(far).unwrap().dragged);
}

/// Without gravity a hexagon let go 30 points off a neighbour's face stays there; let go into a cell it
/// glides the 30 points and lies face to face with it, its end vertices on the neighbour's.
#[test]
fn without_gravity_only_the_cell_brings_a_dropped_hexagon_to_the_face() {
    let size = v(SIZES[0].0, SIZES[0].1);
    for into_cell in [false, true] {
        let p = Params {
            gravity_side: None,
            home_stiffness: 0.0,
            ..Params::default()
        };
        let mut w = World::new(Rect::from_min_size(Vec2::ZERO, v(1200.0, 900.0)), p);
        hexagon(&mut w, size, v(400.0, 300.0));
        while w.step() {}
        let b = hexagon(&mut w, size, v(900.0, 700.0));
        while w.step() {}
        // the cell on the lower right face of `a`, then 30 points farther out along that face's normal
        w.grab(b);
        let cell = w
            .free_cells(b)
            .into_iter()
            .filter(|c| c.x > 500.0 && c.y > 300.0 && c.y < 500.0)
            .min_by(|c, d| {
                (c.y - 378.0)
                    .abs()
                    .partial_cmp(&(d.y - 378.0).abs())
                    .unwrap()
            })
            .expect("a cell beside it");
        let out = v(3f32.sqrt() / 2.0, 0.5);
        w.drag_to(b, cell + out * 30.0);
        w.step();
        let at = if into_cell {
            w.release_to_cell(b, false)
        } else {
            w.release(b, false);
            None
        };
        let mut n = 0;
        while w.step() {
            n += 1;
            assert!(n < 4000);
        }
        let center = w.body(b).unwrap().rect.center();
        if into_cell {
            let at = at.expect("near a cell");
            assert!((center - at).length() < 1.0, "{center:?} in {at:?}");
            assert!(faces_against(&w, b, 1.0) >= 1);
        } else {
            assert!(
                (center - (cell + out * 30.0)).length() < 1.0,
                "stays: {center:?}"
            );
            assert_eq!(faces_against(&w, b, 1.0), 0);
        }
    }
}

/// The sandbox's sizes, `dx` points wider each (a frame's header is as wide as what it holds; it has
/// changed width more than once: FRM-11, FRM-14).
fn widened(dx: f32) -> Vec<(f32, f32)> {
    SIZES.iter().map(|&(x, y)| (x + dx, y)).collect()
}

/// The sandbox's sizes in the order its Hexagons scene repeats them, the three of one flat-face width
/// `dx` points wider each (the big one's width is its own).
fn scene(dx: f32) -> Vec<(f32, f32)> {
    let w = widened(dx);
    vec![w[0], w[1], w[0], w[2], SIZES[3]]
}

/// The extra widths the sweeps below go through, points.
const WIDER: [f32; 9] = [-40.0, -23.0, -17.0, -6.0, 0.0, 6.0, 17.0, 23.0, 40.0];

/// Whatever the frames' widths, a pack laid out cell by cell rests as a honeycomb: each one lies face
/// to face with another. A pack never takes a cell gravity does not hold (on a face across from
/// gravity's side): the hexagon floated off it and rested tip to face against what it met (SETL-33).
#[test]
fn a_pack_rests_face_to_face_whatever_the_widths() {
    for dx in WIDER {
        for width in [900.0, 1200.0] {
            let what = format!("{dx} points wider in {width}");
            let (mut w, ids) = packed(width, 21, &widened(dx));
            let mut n = 0;
            while w.step() {
                n += 1;
                assert!(n < 8_000, "{what}: rests");
                assert!(w.max_overlap() < 0.01, "{what}: overlap at step {n}");
            }
            for &id in &ids[1..] {
                assert!(faces_against(&w, id, 1.0) >= 1, "{what}: #{} alone", id.0);
            }
        }
    }
}

/// Whatever the frames' widths, lifting any hexagon out of the pack sends none of the others away from
/// gravity's side with speed: those it held up rise into the room, others may line a vertex up along the
/// face they rest on (a slide, no speed). A slide along a face that lines a vertex up took one off another
/// face it hung from once room opened under it (SETL-34), or ran it across the face of a neighbour
/// (SETL-36); the contacts pushed that one back with a seat's speed, and the pack under it on with it:
/// the hole the lifted one left closed.
#[test]
fn lifting_a_hexagon_out_of_the_pack_pushes_nothing_away_whatever_the_widths() {
    for (dx, width) in WIDER.into_iter().flat_map(|dx| [(dx, 900.0), (dx, 1200.0)]) {
        let (mut w, ids) = packed_giving(width, 21, &scene(dx), 40.0);
        while w.step() {}
        for &lift in &ids {
            let mut world = w.clone();
            world.grab(lift);
            world.drag_to(lift, v(600.0, 20_000.0));
            for step in 0..300 {
                world.step();
                for b in world.bodies().iter().filter(|b| b.id != lift) {
                    // gravity is to the top: away from it is down
                    assert!(
                        b.vel.y < SINK_SPEED,
                        "{dx} points wider in {width}, #{} lifted: #{} sinks at {} points/s at step {step}",
                        lift.0,
                        b.id.0,
                        b.vel.y
                    );
                }
            }
        }
    }
}

/// Faster than this away from gravity's side (points/s) a body is pushed, not settling.
const SINK_SPEED: f32 = 10.0;
