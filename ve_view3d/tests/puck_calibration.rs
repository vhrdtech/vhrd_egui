//! A calibration as the person does it, played through a fake spacenavd, turns the 3D view the way each prompt said
//! (SPM-9).
//!
//! The raw signs and axes are those of a real device's calibration (a SpaceMouse on spacenavd whose saved mapping
//! read slide right +x, lift up +z, push away +y, tilt forward -rx, rock right +rz, twist clockwise -ry). The motions
//! go through the socket and the client into `Calibration::full`, the derived mapping turns each held motion into a
//! `Nav`, and the camera applies it to a front view of a cube: tilting the puck forward tips the top away, rocking it
//! right turns the cube clockwise on the screen, twisting it clockwise spins the cube's right side towards the
//! viewer. Before SPM-9 the last two were swapped.

#![cfg(unix)]

use std::time::{Duration, Instant};

use egui::{Pos2, Rect, Vec2, pos2};
use glam::Vec3;
use ve_spacemouse::{
    Axes, Calibration, Event, FakeSpacenavd, Mapping, Motion, Nav, Options, SpaceMouse, Step,
};
use ve_view3d::{Bounds, Camera, Projection, ViewPreset};

const GUARD: Duration = Duration::from_secs(10);

/// What each motion read on the device, in `Motion::ALL` order.
const PEAKS: [(Motion, Axes); 6] = [
    (Motion::SlideRight, [420, 0, 0, 0, 0, 0]),
    (Motion::LiftUp, [0, 0, 380, 0, 0, 0]),
    (Motion::PushAway, [0, 450, 0, 0, 0, 0]),
    (Motion::TiltForward, [0, 0, 0, -400, 0, 0]),
    (Motion::RockRight, [0, 0, 0, 0, 0, 360]),
    (Motion::TwistClockwise, [0, 0, 0, 0, -390, 0]),
];

/// One motion as the device sends it: a ramp up, 1.6 s of hold with a little cross-talk, easing off, the zero event.
fn motion(peak: Axes) -> Vec<Event> {
    let at = |f: f32| {
        let mut a = peak.map(|v| (v as f32 * f) as i32);
        for (k, v) in a.iter_mut().enumerate() {
            if peak[k] == 0 {
                *v = (f * 11.0) as i32 * if k % 2 == 0 { 1 } else { -1 };
            }
        }
        Event::Motion {
            axes: a,
            period_ms: 20,
        }
    };
    let mut ev: Vec<Event> = (1..=10).map(|i| at(i as f32 / 10.0)).collect();
    ev.extend((0..80).map(|i| at(1.0 - 0.05 * ((i % 7) as f32 / 7.0))));
    ev.extend((1..=5).map(|i| at(1.0 - i as f32 / 6.0)));
    ev.push(Event::Motion {
        axes: [0; 6],
        period_ms: 20,
    });
    ev
}

/// The full calibration through a fake spacenavd and the client, the prompts done in their order.
fn calibrate(name: &str) -> Mapping {
    let path =
        std::env::temp_dir().join(format!("ve_view3d-puck-{}-{name}.sock", std::process::id()));
    let fake = FakeSpacenavd::start(&path).expect("fake spacenavd");
    let mouse = SpaceMouse::start(Options {
        socket: path,
        retry: Duration::from_millis(20),
    });
    assert!(fake.wait_for_clients(1, GUARD));
    let mut cal = Calibration::full(Mapping::default());
    let mut sent = Vec::new();
    for step in cal.steps() {
        match step {
            Step::Axis(m) => {
                let (_, peak) = PEAKS.iter().find(|(p, _)| p == m).expect("a peak");
                sent.extend(motion(*peak));
            }
            Step::Button(side) => {
                let n = match side {
                    ve_spacemouse::Side::Left => 0,
                    ve_spacemouse::Side::Right => 1,
                };
                sent.extend([Event::button(n, true), Event::button(n, false)]);
            }
        }
    }
    for e in &sent {
        assert_eq!(fake.send(*e), 1);
    }
    let deadline = Instant::now() + GUARD;
    let mut got = 0;
    while got < sent.len() {
        assert!(
            Instant::now() < deadline,
            "only {got} of {} events arrived",
            sent.len()
        );
        for e in mouse.poll() {
            cal.feed(&e, got as f64 * 0.02);
            got += 1;
        }
        std::thread::yield_now();
    }
    assert!(cal.done(), "every step measured");
    assert!(
        cal.results()
            .iter()
            .flatten()
            .all(|r| r.warnings.is_empty()),
        "{:?}",
        cal.results()
    );
    cal.mapping()
}

fn held(m: &Mapping, motion: Motion) -> Nav {
    let (_, peak) = PEAKS.iter().find(|(p, _)| *p == motion).expect("a peak");
    m.apply(peak)
}

fn cube() -> Bounds {
    Bounds {
        min: Vec3::splat(-1.0),
        max: Vec3::splat(1.0),
    }
}

const RECT: Rect = Rect::from_min_max(Pos2::ZERO, pos2(300.0, 300.0));

/// A front view of the cube: +X right on the screen, +Z up, -Y towards the viewer.
fn front() -> Camera {
    let mut cam = Camera {
        rotation: ViewPreset::Front.rotation(),
        projection: Projection::Orthographic,
        ..Default::default()
    };
    cam.fit(&cube(), 1.0);
    cam
}

/// Where `p` is on the screen and its depth (larger: farther) after holding `motion` for 0.2 s.
fn after(m: &Mapping, motion: Motion, p: Vec3) -> (Pos2, f32) {
    let mut cam = front();
    cam.apply_nav(&held(m, motion), 0.2, RECT.height());
    cam.projector(RECT, &cube()).project(p).expect("in front")
}

fn before(p: Vec3) -> (Pos2, f32) {
    front()
        .projector(RECT, &cube())
        .project(p)
        .expect("in front")
}

#[test]
fn each_motion_turns_the_view_as_its_prompt_said() {
    let m = calibrate("turns");
    let (right, top, near) = (Vec3::X, Vec3::Z, -Vec3::Y);
    let moved = |motion, p| {
        let (s0, d0) = before(p);
        let (s1, d1) = after(&m, motion, p);
        (s1 - s0, d1 - d0)
    };

    let (d, _) = moved(Motion::SlideRight, Vec3::ZERO);
    assert!(
        d.x > 5.0 && d.y.abs() < 1e-3,
        "slide right: moves right, {d:?}"
    );
    let (d, _) = moved(Motion::LiftUp, Vec3::ZERO);
    assert!(d.y < -5.0 && d.x.abs() < 1e-3, "lift up: moves up, {d:?}");
    let corner = Vec3::new(1.0, -1.0, 1.0);
    let c = pos2(150.0, 150.0);
    assert!(
        (after(&m, Motion::PushAway, corner).0 - c).length()
            < (before(corner).0 - c).length() - 5.0,
        "push away: gets smaller"
    );

    // Tilt forward: the top goes away, the right side stays where it is.
    let (_, depth) = moved(Motion::TiltForward, top);
    assert!(depth > 1e-3, "tilt forward: the top goes away, {depth}");
    let (d, depth) = moved(Motion::TiltForward, right);
    assert!(d.length() < 1e-2 && depth.abs() < 1e-4, "{d:?} {depth}");

    // Rock right: the cube turns clockwise on the screen, its right side down, nothing comes nearer.
    let (d, depth) = moved(Motion::RockRight, right);
    assert!(d.y > 5.0, "rock right: the right side goes down, {d:?}");
    assert!(
        depth.abs() < 1e-4,
        "rock right: on the screen's plane, {depth}"
    );
    let (d, _) = moved(Motion::RockRight, top);
    assert!(
        d.x > 5.0,
        "rock right: the top goes right (clockwise), {d:?}"
    );

    // Twist clockwise: the cube spins about the screen's vertical, its right side towards the viewer and its near
    // face to the left; the top stays.
    let (d, depth) = moved(Motion::TwistClockwise, right);
    assert!(
        depth < -1e-3,
        "twist clockwise: the right side comes nearer, {depth}"
    );
    assert!(
        d.y.abs() < 1e-2,
        "twist clockwise: no roll on the screen, {d:?}"
    );
    let (d, _) = moved(Motion::TwistClockwise, near);
    assert!(
        d.x < -5.0,
        "twist clockwise: the near face goes left, {d:?}"
    );
    let (d, depth) = moved(Motion::TwistClockwise, top);
    assert!(d.length() < 1e-2 && depth.abs() < 1e-4, "{d:?} {depth}");

    // The motions the person did not make stay still.
    let twist = held(&m, Motion::TwistClockwise);
    assert_eq!(
        (twist.pan, twist.zoom, twist.rotate[0], twist.rotate[2]),
        (Vec2::ZERO, 0.0, 0.0, 0.0)
    );
}

/// The device's saved mapping from before SPM-9 had the rock in the fourth slot and the twist in the fifth: the
/// fresh calibration puts them the other way, so that file has to be calibrated again.
#[test]
fn the_slots_follow_the_screen_axes() {
    let m = calibrate("slots");
    let raws: Vec<_> = m.axes.iter().map(|a| (a.raw, a.invert)).collect();
    assert_eq!(
        raws,
        [
            (Some(0), false),
            (Some(2), false),
            (Some(1), true),
            (Some(3), true),
            (Some(4), true),
            (Some(5), false),
        ]
    );
}
