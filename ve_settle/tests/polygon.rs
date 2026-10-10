//! Convex polygon bodies (SETL-4): outlines, their contacts, editing a
//! vertex with the outline kept convex, and vertex attraction on them.

use ve_settle::{BodyDesc, BodyId, Params, Poly, Rect, Side, Vec2, World};

fn v(x: f32, y: f32) -> Vec2 {
    Vec2::new(x, y)
}

fn world(w: f32, h: f32, params: Params) -> World {
    World::new(Rect::from_min_size(Vec2::ZERO, v(w, h)), params)
}

fn unit() -> Rect {
    Rect::from_min_size(Vec2::ZERO, v(1.0, 1.0))
}

fn close(a: Vec2, b: Vec2) -> bool {
    (a - b).max_abs() < 1e-3
}

/// Step to rest; on the way solid bodies never overlap.
fn settle(w: &mut World) {
    for n in 0..8_000 {
        if !w.step() {
            return;
        }
        assert!(
            w.max_overlap() < 0.01,
            "overlap {} at step {n}",
            w.max_overlap()
        );
    }
    panic!("not at rest");
}

fn hexagon(w: &mut World, center: Vec2) -> BodyId {
    w.add(
        BodyDesc::new(center, v(160.0, 140.0))
            .home(None)
            .outline(Poly::unit_hexagon()),
    )
}

#[test]
fn the_predefined_shapes_are_convex_and_fill_their_rectangle() {
    let mut shapes = vec![Poly::unit_rect(), Poly::unit_hexagon()];
    for side in Side::ALL {
        shapes.push(Poly::unit_half_hexagon(side));
        shapes.push(Poly::unit_triangle(side));
    }
    for shape in shapes {
        assert!(shape.is_convex(), "{shape:?}");
        assert_eq!(shape.bounds(), unit(), "{shape:?}");
    }
    let r = Rect::from_min_size(v(10.0, 20.0), v(200.0, 100.0));
    let hex = Poly::unit_hexagon().fit(&r);
    assert!(close(hex.points()[0], v(60.0, 20.0)));
    assert!(close(hex.points()[2], v(210.0, 70.0)));
    assert!(close(Poly::to_unit(&r, v(210.0, 70.0)), v(1.0, 0.5)));
    // Two half hexagons make a whole one.
    let left = Poly::unit_half_hexagon(Side::Right);
    assert!(close(left.points()[4], v(0.0, 0.5)));
}

#[test]
fn a_dragged_vertex_stops_where_the_outline_would_dent() {
    // A corner of the square pulled inward stops on the line through its
    // neighbours: a triangle with one vertex in the middle of a side.
    let mut square = Poly::unit_rect();
    let at = square.move_vertex(2, v(0.2, 0.3), &unit());
    assert!((at.x + at.y - 1.0).abs() < 1e-3, "on the diagonal: {at:?}");
    assert!(square.is_convex());

    // A hexagon's tip pulled far out stops where its neighbours' far
    // edges meet (here the rectangle's edge comes first).
    let mut hex = Poly::unit_hexagon();
    let at = hex.move_vertex(2, v(3.0, 0.5), &unit());
    assert!(close(at, v(1.0, 0.5)), "{at:?}");
    // Along the top edge a vertex stops at the corner its neighbours'
    // edges would meet at.
    let at = hex.move_vertex(1, v(2.0, 0.0), &unit());
    assert!(close(at, v(1.0, 0.0)), "{at:?}");
    assert!(hex.is_convex());
    // Wherever it is dragged, the outline stays convex.
    let mut hex = Poly::unit_hexagon();
    for step in 0..200 {
        let a = step as f32 * 0.37;
        let to = v(
            0.5 + a.cos() * (step % 7) as f32 * 0.2,
            0.5 + a.sin() * (step % 5) as f32 * 0.3,
        );
        let at = hex.move_vertex(step % 6, to, &unit());
        assert!(unit().contains(at), "{at:?}");
        assert!(
            hex.is_convex(),
            "after dragging {} to {to:?}: {hex:?}",
            step % 6
        );
    }
}

#[test]
fn a_vertex_can_be_inserted_on_an_outline_and_dragged() {
    let p = Params {
        gravity_side: None,
        home_stiffness: 0.0,
        attract_range: 0.0,
        ..Params::default()
    };
    let mut w = world(600.0, 600.0, p);
    let id = w.add(BodyDesc::new(v(300.0, 300.0), v(200.0, 100.0)).home(None));
    assert!(w.body(id).unwrap().outline.is_none());
    // On the right edge, a third of the way down.
    let i = w
        .insert_vertex(id, v(410.0, 280.0))
        .expect("room for a vertex");
    let outline = w.body(id).unwrap().outline.expect("it has an outline now");
    assert_eq!(outline.points().len(), 5);
    assert!(close(outline.points()[i], v(1.0, 0.3)), "{outline:?}");
    assert!(close(
        w.body(id).unwrap().shape().points()[i],
        v(400.0, 280.0)
    ));
    // Dragged inward it would dent the outline: it stays on the edge.
    let at = w.move_vertex(id, i, v(350.0, 280.0)).unwrap();
    assert!((at.x - 400.0).abs() < 0.01, "{at:?}");
    // The corner above it can go inward: the outline loses that corner.
    let at = w.move_vertex(id, i - 1, v(360.0, 250.0)).unwrap();
    assert!(close(at, v(360.0, 250.0)), "{at:?}");
    assert!(w.body(id).unwrap().outline.unwrap().is_convex());
    // Taken out again; a triangle keeps its three.
    assert!(w.remove_vertex(id, i));
    assert_eq!(w.body(id).unwrap().outline.unwrap().points().len(), 4);
    w.set_outline(id, Some(Poly::unit_triangle(Side::Top)));
    assert!(!w.remove_vertex(id, 0));
    // The outline is full at twelve.
    w.set_outline(id, Some(Poly::unit_hexagon()));
    let mut added = 0;
    while w
        .insert_vertex(id, v(200.0 + added as f32 * 7.0, 250.0))
        .is_some()
    {
        added += 1;
    }
    assert_eq!(added, 6);
}

#[test]
fn hexagons_nest_where_their_rectangles_would_overlap() {
    let p = Params {
        home_stiffness: 0.0,
        attract_range: 0.0,
        ..Params::default()
    };
    // Too narrow for two side by side.
    let mut w = world(300.0, 800.0, p);
    // One under the top wall, one dropped under it to the side: it
    // slides up along the slanted edge into the notch.
    let a = hexagon(&mut w, v(86.0, 76.0));
    let b = hexagon(&mut w, v(210.0, 260.0));
    settle(&mut w);
    let (ra, rb) = (w.body(a).unwrap().rect, w.body(b).unwrap().rect);
    assert!(rb.min.y < ra.max.y - 20.0, "nested: {ra:?} {rb:?}");
    assert!(ra.overlap(&rb, 0.0).x > 0.0 && ra.overlap(&rb, 0.0).y > 0.0);
    assert!(w.max_overlap() < 0.01);
    assert!(w.max_gap_violation() < 0.05, "{}", w.max_gap_violation());
}

#[test]
fn a_triangle_fills_the_gap_between_two_hexagons() {
    let p = Params {
        home_stiffness: 0.0,
        attract_range: 0.0,
        ..Params::default()
    };
    // Just wide enough for the two hexagons: they cannot be wedged apart.
    let gap = p.gap;
    let mut w = world(3.0 * gap + 320.0, 800.0, p);
    let left = hexagon(&mut w, v(gap + 80.0, gap + 70.0));
    let right = hexagon(&mut w, v(gap + 160.0 + gap + 80.0, gap + 70.0));
    settle(&mut w);
    // Between their lower slanted edges is a notch; a triangle with its
    // tip up, dropped below it, rises into it.
    let notch = (w.body(left).unwrap().rect.max.x + w.body(right).unwrap().rect.min.x) * 0.5;
    let tri = w.add(
        BodyDesc::new(v(notch, 320.0), v(80.0, 70.0))
            .home(None)
            .outline(Poly::unit_triangle(Side::Top)),
    );
    settle(&mut w);
    let (rt, rl) = (w.body(tri).unwrap().rect, w.body(left).unwrap().rect);
    assert!(
        rt.min.y < rl.max.y - 20.0,
        "in the notch: {rt:?} under {rl:?}"
    );
    assert!((rt.center().x - notch).abs() < 2.0, "{rt:?}");
    assert!(w.max_overlap() < 0.01);
}

#[test]
fn polygons_and_rectangles_mix_without_overlap() {
    let p = Params::default();
    let mut w = world(700.0, 700.0, p);
    let shapes = [
        None,
        Some(Poly::unit_hexagon()),
        Some(Poly::unit_triangle(Side::Bottom)),
        Some(Poly::unit_half_hexagon(Side::Left)),
        None,
        Some(Poly::unit_hexagon()),
        Some(Poly::unit_triangle(Side::Top)),
        Some(Poly::unit_half_hexagon(Side::Top)),
    ];
    for (i, shape) in shapes.into_iter().enumerate() {
        let at = v(
            120.0 + 150.0 * (i % 4) as f32,
            100.0 + 160.0 * (i / 4) as f32,
        );
        let mut desc = BodyDesc::new(at, v(160.0, 130.0)).min_size(v(100.0, 80.0));
        if let Some(shape) = shape {
            desc = desc.outline(shape);
        }
        w.add(desc);
    }
    // Dropped on each other they are lifted and glide apart: no overlap
    // once they have all landed, and then never again.
    let mut landed = false;
    for _ in 0..8_000 {
        if !w.step() {
            break;
        }
        landed |= w.bodies().iter().all(|b| !b.lifted);
        if landed {
            assert!(w.max_overlap() < 0.01, "overlap {}", w.max_overlap());
        }
    }
    assert!(w.is_at_rest() && landed);
    for b in w.bodies() {
        assert!(b.rect.min.x >= 5.5 && b.rect.max.x <= 694.5, "{:?}", b.rect);
        assert_eq!(b.bulge, Vec2::ZERO);
    }
}

#[test]
fn a_polygons_vertex_grows_to_what_it_is_just_short_of() {
    let p = Params {
        gravity_side: None,
        home_stiffness: 0.0,
        snap_range: 0.0,
        ..Params::default()
    };
    let mut w = world(600.0, 400.0, p);
    // Its right tip 8 short of the right wall's gap line; the left one
    // 60 from a rectangle.
    let hex = hexagon(&mut w, v(600.0 - p.gap - 8.0 - 80.0, 200.0));
    w.add(
        BodyDesc::new(
            v(600.0 - p.gap - 8.0 - 160.0 - 60.0 - 50.0, 200.0),
            v(100.0, 100.0),
        )
        .home(None),
    );
    settle(&mut w);
    let b = w.body(hex).unwrap();
    let tip = b.shape().points()[2];
    assert!(
        (tip.x - (600.0 - p.gap)).abs() < 0.1,
        "tip at the wall: {tip:?}"
    );
    assert!((b.nudges()[2].x - 8.0).abs() < 0.1, "{:?}", b.nudges());
    assert_eq!(b.nudges()[5], Vec2::ZERO, "60 away stays: {:?}", b.nudges());
    assert!(b.shape().is_convex());
    assert_eq!(
        b.rect.size(),
        v(160.0, 140.0),
        "the vertex moved, not the body"
    );
    assert!(w.pulls().iter().any(|q| q.body == hex));

    // The same against a rectangle that comes near: the gap is kept.
    let near =
        w.add(BodyDesc::new(v(b.rect.min.x - p.gap - 9.0 - 30.0, 200.0), v(60.0, 60.0)).home(None));
    settle(&mut w);
    let b = w.body(hex).unwrap();
    let edge = w.body(near).unwrap().rect.max.x;
    let tip = b.shape().points()[5];
    assert!((tip.x - edge - p.gap).abs() < 0.1, "{tip:?} against {edge}");
    assert!(w.max_gap_violation() < 0.05);

    // Range 0 lets go.
    w.set_params(Params {
        attract_range: 0.0,
        ..p
    });
    settle(&mut w);
    assert!(
        w.body(hex)
            .unwrap()
            .nudges()
            .iter()
            .all(|&n| n == Vec2::ZERO)
    );
}
