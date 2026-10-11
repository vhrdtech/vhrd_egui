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
        push_out: rng.next() < 0.5,
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
