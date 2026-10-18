//! Group drag (SETL-22): the bodies stuck together with one, as `World::cluster` finds them.

use ve_settle::Settling;
use ve_settle::{BodyDesc, BodyId, Params, Rect, Vec2, World};

fn v(x: f32, y: f32) -> Vec2 {
    Vec2::new(x, y)
}

/// No gravity and no homes: what is put somewhere stays there.
fn world() -> World {
    let p = Params {
        gravity_side: None,
        home_stiffness: 0.0,
        ..Params::default()
    };
    World::new(Rect::from_min_size(Vec2::ZERO, v(1200.0, 900.0)), p).with_settling(Settling::Always)
}

fn add(w: &mut World, min: Vec2, size: Vec2) -> BodyId {
    w.add(BodyDesc::new(min + size * 0.5, size).home(None))
}

fn rest(w: &mut World) {
    assert!(w.settle(6_000).is_some(), "not at rest");
}

fn sorted(mut ids: Vec<BodyId>) -> Vec<BodyId> {
    ids.sort_by_key(|id| id.0);
    ids
}

#[test]
fn touching_bodies_away_from_the_walls_are_one_cluster() {
    let mut w = world();
    let gap = w.params().gap;
    let size = v(200.0, 100.0);
    // an L of three, the gap apart, and one farther off
    let a = add(&mut w, v(300.0, 300.0), size);
    let b = add(&mut w, v(500.0 + gap, 300.0), size);
    let c = add(&mut w, v(300.0, 400.0 + gap), size);
    let d = add(&mut w, v(800.0, 600.0), size);
    rest(&mut w);
    assert_eq!(sorted(w.cluster(b)), sorted(vec![a, b, c]));
    assert_eq!(w.cluster(b)[0], b, "the one asked for comes first");
    assert_eq!(w.cluster(d), vec![d]);
}

#[test]
fn a_cluster_at_a_wall_belongs_to_the_wall() {
    let mut w = world();
    let gap = w.params().gap;
    let size = v(200.0, 100.0);
    let a = add(&mut w, v(gap, 300.0), size);
    let b = add(&mut w, v(200.0 + 2.0 * gap, 300.0), size);
    rest(&mut w);
    assert_eq!(w.cluster(b), vec![b]);
    assert_eq!(w.cluster(a), vec![a]);
}

#[test]
fn a_stack_under_gravity_is_no_group() {
    let mut w = World::new(
        Rect::from_min_size(Vec2::ZERO, v(1200.0, 900.0)),
        Params {
            home_stiffness: 0.0,
            ..Params::default()
        },
    )
    .with_settling(Settling::Always);
    let a = add(&mut w, v(300.0, 300.0), v(200.0, 100.0));
    let b = add(&mut w, v(300.0, 420.0), v(200.0, 100.0));
    rest(&mut w);
    assert_eq!(w.cluster(a), vec![a]);
    assert_eq!(w.cluster(b), vec![b]);
}

#[test]
fn hexagons_nested_face_to_face_are_one_cluster() {
    let mut w = world();
    let gap = w.params().gap;
    let size = v(200.0, 173.2);
    let hex = |min: Vec2| {
        BodyDesc::new(min + size * 0.5, size)
            .home(None)
            .outline(ve_settle::Poly::unit_hexagon())
    };
    let a = w.add(hex(v(300.0, 300.0)));
    // the next column, half a hexagon down, across the slanted face (the gap apart along its normal)
    let normal = v(0.866, 0.5);
    let b = w.add(hex(v(450.0, 300.0 + size.y * 0.5) + normal * gap));
    rest(&mut w);
    assert_eq!(sorted(w.cluster(a)), sorted(vec![a, b]));
}

#[test]
fn a_lifted_or_unknown_body_is_alone() {
    let mut w = world();
    let a = add(&mut w, v(300.0, 300.0), v(200.0, 100.0));
    // just added: lifted until it lands
    assert_eq!(w.cluster(a), vec![a]);
    assert!(w.cluster(BodyId(999)).is_empty());
}
