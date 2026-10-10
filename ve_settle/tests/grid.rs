//! The grid pitch: touching bodies line up at whole steps of one pitch,
//! a washboard along each contact (SETL-10).

use ve_settle::{BodyDesc, Params, Rect, Side, Vec2, World};

fn v(x: f32, y: f32) -> Vec2 {
    Vec2::new(x, y)
}

fn world(w: f32, h: f32, params: Params) -> World {
    World::new(Rect::from_min_size(Vec2::ZERO, v(w, h)), params)
}

/// Gravity up, no homes and no corner snaps: only the grid lines things up.
fn grid(stiffness: f32) -> Params {
    Params {
        home_stiffness: 0.0,
        snap_range: 0.0,
        grid_pitch: 24.0,
        grid_stiffness: stiffness,
        ..Params::default()
    }
}

/// How far `offset` is from the nearest whole pitch.
fn off_grid(offset: f32, pitch: f32) -> f32 {
    (offset - pitch * (offset / pitch).round()).abs()
}

/// A panel under the top wall and one under it, released off the grid
/// (gravity up); or the same turned on its side (gravity to the left).
fn released_off_grid(p: Params) -> World {
    let mut w = world(1000.0, 1000.0, p);
    let turn = |c: Vec2| {
        if p.gravity_side == Some(Side::Left) {
            v(c.y, c.x)
        } else {
            c
        }
    };
    w.add(BodyDesc::new(turn(v(237.0, 70.0)), turn(v(300.0, 120.0))));
    w.add(BodyDesc::new(turn(v(246.0, 220.0)), turn(v(260.0, 140.0))));
    let mut last = Vec::new();
    for n in 0..20_000 {
        if !w.step() {
            return w;
        }
        let max_move = (p.max_speed + p.glide_speed) * p.dt;
        assert!(w.last_move() <= max_move, "jump at step {n}");
        assert_eq!(w.max_overlap(), 0.0, "overlap at step {n}");
        last = w.bodies().to_vec();
    }
    panic!("still moving: {last:#?}");
}

#[test]
fn widgets_released_off_grid_settle_on_grid_steps() {
    let w = released_off_grid(grid(1500.0));
    let (a, b) = (&w.bodies()[0], &w.bodies()[1]);
    // `a` touches the top wall: its left edge is whole steps from the
    // walls' corner.
    assert!(off_grid(a.rect.min.x - 6.0, 24.0) < 0.5, "{a:?}");
    // `b` hangs under `a`: their left edges are whole steps apart.
    assert!(off_grid(b.rect.min.x - a.rect.min.x, 24.0) < 0.5, "{b:?}");
}

#[test]
fn the_grid_works_across_the_other_axis_too() {
    let p = Params {
        gravity_side: Some(Side::Left),
        ..grid(1500.0)
    };
    let w = released_off_grid(p);
    let (a, b) = (&w.bodies()[0], &w.bodies()[1]);
    assert!(off_grid(a.rect.min.y - 6.0, 24.0) < 0.5, "{a:?}");
    assert!(off_grid(b.rect.min.y - a.rect.min.y, 24.0) < 0.5, "{b:?}");
}

#[test]
fn without_grid_stiffness_bodies_rest_where_they_come_to() {
    let w = released_off_grid(grid(0.0));
    let (a, b) = (&w.bodies()[0], &w.bodies()[1]);
    assert!(off_grid(a.rect.min.x - 6.0, 24.0) > 2.0, "{a:?}");
    assert!(off_grid(b.rect.min.x - a.rect.min.x, 24.0) > 2.0, "{b:?}");
}
