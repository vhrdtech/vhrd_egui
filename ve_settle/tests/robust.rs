//! Robustness (SETL-19): whatever the parameters are turned to, no two bodies share a middle and nothing
//! crosses a hard wall; put back to the defaults, the world comes to rest with nothing overlapping.

use ve_settle::{BodyDesc, Params, Poly, Rect, Side, Vec2, World};

/// A small deterministic generator.
struct Rng(u32);

impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 % 10_000) as f32 / 10_000.0
    }

    fn in_range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next()
    }
}

/// Parameters anywhere in the ranges of the sandbox's sliders; gravity's side and the walls stay.
fn random(rng: &mut Rng, base: &Params) -> Params {
    Params {
        gravity_side: base.gravity_side,
        gravity: rng.in_range(0.0, 3000.0),
        home_stiffness: rng.in_range(0.0, 300.0),
        home_drift: rng.in_range(0.0, 5.0),
        damping: rng.in_range(0.0, 40.0),
        gap: rng.in_range(0.0, 24.0),
        tension: rng.in_range(0.0, 1.0),
        restore: rng.in_range(0.0, 10.0),
        grow_speed: rng.in_range(20.0, 2000.0),
        glide_speed: rng.in_range(50.0, 3000.0),
        snap_range: rng.in_range(0.0, 80.0),
        snap_stiffness: rng.in_range(0.0, 6000.0),
        attract_range: rng.in_range(0.0, 80.0),
        attract_strength: rng.in_range(0.0, 60.0),
        friction: rng.in_range(0.0, 4.0),
        bulge_rest: rng.in_range(0.0, 20.0),
        bulge_from: rng.in_range(0.0, 20.0),
        bulge_gain: rng.in_range(0.0, 5.0),
        bulge_max: rng.in_range(0.0, 60.0),
        bulge_speed: rng.in_range(10.0, 1000.0),
        grid_pitch: rng.in_range(4.0, 96.0),
        grid_stiffness: rng.in_range(0.0, 3000.0),
        max_speed: rng.in_range(100.0, 6000.0),
        walls: base.walls,
        iterations: 1 + (rng.next() * 63.0) as u32,
        rest_eps: rng.in_range(0.0005, 0.5),
        rest_steps: 1 + (rng.next() * 239.0) as u32,
        dt: 1.0 / rng.in_range(30.0, 480.0),
        drag: rng.in_range(0.0, 0.5),
        push_out: rng.next() < 0.5,
        nest_range: rng.in_range(0.0, 200.0),
    }
}

/// A world that scrolls along gravity's axis: gravity's side and the two sides across are hard walls, the
/// side away from gravity is open. Twenty panels in rows that fit, every fourth a hexagon, some rigid.
fn field(gravity: Side) -> (World, Params) {
    let mut params = Params {
        gravity_side: Some(gravity),
        ..Params::default()
    };
    match gravity {
        Side::Top => params.walls.bottom = false,
        Side::Left => params.walls.right = false,
        Side::Bottom => params.walls.top = false,
        Side::Right => params.walls.left = false,
    }
    let bounds = Rect::from_min_size(Vec2::ZERO, Vec2::new(900.0, 700.0));
    let mut world = World::new(bounds, params);
    let sizes = [
        (280.0, 160.0),
        (210.0, 90.0),
        (310.0, 156.0),
        (250.0, 120.0),
        (200.0, 200.0),
    ];
    let (mut x, mut y, mut row) = (12.0, 12.0, 0.0f32);
    for i in 0..20 {
        let (w, h) = sizes[i % sizes.len()];
        if x + w + 12.0 > 900.0 {
            (x, y, row) = (12.0, y + row + 12.0, 0.0);
        }
        // along the open side: rows for a world that grows down, columns for one that grows right
        let at = match gravity.axis() {
            ve_settle::Axis::Y => Vec2::new(x + w / 2.0, y + h / 2.0),
            ve_settle::Axis::X => Vec2::new(y + w / 2.0, (x * 0.7) + h / 2.0),
        };
        let size = Vec2::new(w, h);
        let mut desc = BodyDesc::new(at, size).home(Some(at));
        desc = if i % 3 == 0 {
            desc.min_size(size).max_size(size)
        } else {
            desc.min_size(size * 0.6)
        };
        if i % 4 == 2 {
            desc = desc.outline(Poly::unit_hexagon());
        }
        world.add(desc);
        x += w + 12.0;
        row = row.max(h);
    }
    (world, params)
}

/// What holds on every step, whatever the parameters.
fn check(world: &World, what: &str) {
    for b in world.bodies() {
        let r = b.rect;
        assert!(
            r.min.x.is_finite()
                && r.min.y.is_finite()
                && r.max.x.is_finite()
                && r.max.y.is_finite(),
            "{what}: a body is nowhere: {r:?}"
        );
    }
    assert_eq!(
        world.max_escape(),
        0.0,
        "{what}: a body is past a hard wall"
    );
    assert!(
        world.max_core_overlap() <= 1e-3,
        "{what}: two bodies share a middle: {}",
        world.max_core_overlap()
    );
}

fn randomise_then_reset(gravity: Side, seed: u32) {
    let (mut world, defaults) = field(gravity);
    assert!(world.settle(6_000).is_some(), "the field settles");
    assert_eq!(world.max_overlap(), 0.0);
    let mut rng = Rng(seed);
    for round in 0..12 {
        let wild = random(&mut rng, &defaults);
        world.set_params(wild);
        let steps = 20 + (rng.next() * 200.0) as u32;
        for n in 0..steps {
            world.step();
            check(
                &world,
                &format!("{gravity:?} seed {seed} round {round} step {n}"),
            );
        }
    }
    // "Default parameters"
    world.set_params(defaults);
    let dt = defaults.dt;
    let mut rested = None;
    for n in 0..(REST_SECS / dt) as u32 {
        if !world.step() {
            rested = Some(n);
            break;
        }
        check(
            &world,
            &format!("{gravity:?} seed {seed} after the reset, step {n}"),
        );
    }
    let what = format!("{gravity:?} seed {seed}");
    assert!(
        rested.is_some(),
        "{what}: not at rest {REST_SECS} s after the reset"
    );
    assert_eq!(world.max_overlap(), 0.0, "{what}: an overlap at rest");
    assert!(
        world.bodies().iter().all(|b| !b.lifted),
        "{what}: a body left lifted"
    );
    // the world's extent is what a scroll area shows: finite, and it starts at the hard walls
    let far = world.bodies().iter().map(|b| b.rect.max[gravity.axis()]);
    let far = far.fold(0.0, f32::max);
    assert!(
        far.is_finite() && far < 20_000.0,
        "{what}: the world ran away to {far}"
    );
}

/// Seconds the world may take to rest once the parameters are back at their defaults.
const REST_SECS: f32 = 16.0;

#[test]
fn any_parameters_then_the_defaults_in_a_world_that_grows_down() {
    for seed in [0x2545_f491, 0x0bad_cafe, 7] {
        randomise_then_reset(Side::Top, seed);
    }
}

#[test]
fn any_parameters_then_the_defaults_in_a_world_that_grows_to_the_right() {
    for seed in [0x2545_f491, 0x0bad_cafe, 7] {
        randomise_then_reset(Side::Left, seed);
    }
}

/// A change of parameters comes in over a few steps and leaves no body faster than a glide.
#[test]
fn a_change_of_parameters_puts_no_energy_in() {
    let (mut world, defaults) = field(Side::Top);
    assert!(world.settle(6_000).is_some());
    let wild = Params {
        gravity: 3000.0,
        home_stiffness: 300.0,
        snap_stiffness: 6000.0,
        damping: 0.0,
        ..defaults
    };
    world.set_params(wild);
    assert_eq!(
        world.params().gravity,
        3000.0,
        "what was set is what is shown"
    );
    assert!(
        world.live_params().gravity < 3000.0,
        "and it comes in over a few steps"
    );
    for _ in 0..ve_settle::PARAM_RAMP_STEPS {
        world.step();
    }
    assert_eq!(world.live_params(), world.params());
    for _ in 0..120 {
        world.step();
    }
    world.set_params(defaults);
    let glide = defaults.glide_speed;
    for b in world.bodies() {
        assert!(
            b.vel.length() <= glide + 0.01,
            "kept {} points/s",
            b.vel.length()
        );
    }
    // nonsense is made sane, not obeyed
    world.set_params(Params {
        dt: f32::NAN,
        gap: -5.0,
        glide_speed: 0.0,
        iterations: 0,
        ..defaults
    });
    let p = world.params();
    assert!(p.dt > 0.0 && p.gap == 0.0 && p.glide_speed > 0.0 && p.iterations == 1);
}

/// A rigid body in a full row between two hard walls grows (a frame switched to a wider size): the row is
/// too full, and the one that gives way leaves it at a glide, slower than the body grows. Whatever the
/// widths, the rectangles never overlap on the way (the jam takes up the gaps, not the rectangles), and
/// the body ends at its new size. (Seen in the sandbox once the frame's header grew wider, FRM-11: the row
/// was 0.9 points too deep.)
#[test]
fn a_body_growing_in_a_full_row_never_overlaps_its_neighbours() {
    for grow_by in [20.0, 45.0, 70.0, 110.0, 160.0] {
        for first in [250.0, 280.0, 310.0] {
            let params = Params {
                walls: ve_settle::Walls {
                    bottom: false,
                    ..ve_settle::Walls::ALL
                },
                ..Params::default()
            };
            let bounds = Rect::from_min_size(Vec2::ZERO, Vec2::new(900.0, 700.0));
            let mut world = World::new(bounds, params);
            // a full row along the top, a second one under it
            let rows = [
                (6.0, [first, 250.0, 870.0 - first - 250.0 - 12.0], 160.0),
                (172.0, [300.0, 260.0, 290.0], 120.0),
            ];
            let mut grower = None;
            for (y, widths, h) in rows {
                let mut x = 6.0;
                for w in widths {
                    let size = Vec2::new(w, h);
                    let at = Vec2::new(x + w / 2.0, y + h / 2.0);
                    let id = world.add(BodyDesc::new(at, size).min_size(size).max_size(size));
                    grower.get_or_insert(id);
                    x += w + 6.0;
                }
            }
            let what = format!("{first} grown by {grow_by}");
            assert!(world.settle(6_000).is_some(), "{what}: the rows settle");
            assert_eq!(world.max_overlap(), 0.0, "{what}");
            let id = grower.expect("a body");
            let wide = Vec2::new(first + grow_by, 160.0);
            world.set_limits(id, wide, wide);
            world.set_want(id, wide);
            for n in 0..6_000 {
                if !world.step() {
                    break;
                }
                assert!(
                    world.max_overlap() <= 0.01,
                    "{what}: two rectangles overlap by {} at step {n}",
                    world.max_overlap()
                );
            }
            assert!(world.is_at_rest(), "{what}: it comes to rest");
            let size = world.body(id).expect("the body").rect.size();
            assert!(
                (size.x - wide.x).abs() < 0.5,
                "{what}: it grew to {size:?}, not {wide:?}"
            );
        }
    }
}

/// Whatever the widths of the panels, a window made narrower than its rows (each row too full: one of
/// it gives way and drops into the row after it) and then wider again moves no body more than two
/// glides in a step, no two rectangles overlap, and it comes to rest. The one giving way leaves its row
/// at a glide whatever pushes it: the slides its neighbours at a hard wall could not do were handed on
/// to it, and a corner it came off pushed it out by the whole gap, 18 points in a step (SETL-32, seen
/// in the sandbox once the frame's header got narrower, FRM-14).
#[test]
fn a_window_narrower_than_its_rows_moves_nothing_by_a_jump_whatever_the_widths() {
    let mut params = Params {
        home_stiffness: 0.0,
        push_out: true,
        ..Params::default()
    };
    params.max_speed = params.glide_speed;
    params.walls.bottom = false;
    // the sandbox's rectangle frames: medium, tiny, big, medium with sides
    let sizes = [
        (300.3, 162.0),
        (235.7, 88.0),
        (338.0, 242.0),
        (300.3, 116.0),
    ];
    for dx in [-40.0, -23.0, -17.0, 0.0, 17.0, 23.0, 40.0] {
        let wide = 900.0;
        let mut world = World::new(
            Rect::from_min_size(Vec2::ZERO, Vec2::new(wide, 760.0)),
            params,
        );
        let (mut x, mut y, mut row) = (6.0, 6.0, 0.0f32);
        for i in 0..14 {
            let (w, h) = sizes[i % sizes.len()];
            let size = Vec2::new(w + dx, h);
            if x + size.x + 6.0 > wide {
                (x, y, row) = (6.0, y + row + 6.0, 0.0);
            }
            let at = Vec2::new(x + size.x / 2.0, y + h / 2.0);
            let desc = BodyDesc::new(at, size);
            world.add(if i % 3 == 0 {
                desc.min_size(size).max_size(size)
            } else {
                desc.min_size(Vec2::new(size.x * 0.7, h)).max_size(size)
            });
            x += size.x + 6.0;
            row = row.max(h);
        }
        assert!(world.settle(6_000).is_some(), "{dx}: the rows settle");
        let most = 2.0 * params.glide_speed * params.dt;
        for width in [750.0, 1350.0] {
            let what = format!("{dx} points wider, the window {width} wide");
            world.set_bounds(Rect::from_min_size(Vec2::ZERO, Vec2::new(width, 760.0)));
            let mut n = 0;
            loop {
                let before: Vec<Rect> = world.bodies().iter().map(|b| b.rect).collect();
                if !world.step() {
                    break;
                }
                n += 1;
                assert!(n < 6_000, "{what}: rests");
                for (b, was) in world.bodies().iter().zip(&before) {
                    let moved = (b.rect.min - was.min)
                        .max_abs()
                        .max((b.rect.max - was.max).max_abs());
                    assert!(
                        moved <= most,
                        "{what}: #{} moved {moved} points in step {n}",
                        b.id.0
                    );
                }
                assert!(
                    world.max_overlap() <= 0.01,
                    "{what}: two rectangles overlap by {} at step {n}",
                    world.max_overlap()
                );
            }
        }
    }
}
