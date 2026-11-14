//! The signs the calibrate step derives (SPM-2): a calibration sequence goes through a fake spacenavd and the
//! client, as the person would do it, and the mapping that comes out makes the content follow the hand. Slide right
//! and the page moves right, lift and it moves up, push away and it gets smaller, rock right and it turns clockwise
//! on the screen, twist clockwise and it spins clockwise seen from above. The camera convention turns all of them
//! around. `ve_view3d/tests/puck_calibration.rs` plays a calibration through to the 3D camera.
//!
//! The sequence is shaped like spacenavd's stream on a device set up with `swap-yz` (each motion ramps up, holds
//! with a little cross-talk on the other axes, and comes back to a zero event); the raw signs in it are arbitrary
//! on purpose, since the calibrate step must not depend on them. Event times are given by the test, 20 ms apart.

#![cfg(unix)]

use std::time::{Duration, Instant};

use ve_spacemouse::{
    Axes, Calibration, Convention, Event, FakeSpacenavd, Mapping, Motion, Nav, Options, SpaceMouse,
};

const GUARD: Duration = Duration::from_secs(10);

/// One motion as the device sends it: 10 events ramping up to `peak`, 80 holding it (1.6 s of hold in all),
/// 5 easing off, then the zero event of the puck at rest.
fn motion(peak: Axes) -> Vec<Event> {
    let at = |f: f32| {
        let mut a = peak.map(|v| (v as f32 * f) as i32);
        // a little cross-talk on every other axis, as a real hand gives
        for (k, v) in a.iter_mut().enumerate() {
            if peak[k] == 0 {
                *v = (f * 9.0) as i32 * if k % 2 == 0 { 1 } else { -1 };
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

/// What each motion reads on this device, in `Motion::ALL` order.
const PEAKS: [(Motion, Axes); 6] = [
    (Motion::SlideRight, [310, 0, 0, 0, 0, 0]),
    (Motion::LiftUp, [0, 0, -290, 0, 0, 0]),
    (Motion::PushAway, [0, 330, 0, 0, 0, 0]),
    (Motion::TiltForward, [0, 0, 0, -260, 0, 0]),
    (Motion::RockRight, [0, 0, 0, 0, 0, 240]),
    (Motion::TwistClockwise, [0, 0, 0, 0, -280, 0]),
];

/// The whole sequence of `Calibration::full`: six motions, then the left and the right button.
fn sequence() -> Vec<Event> {
    let mut ev: Vec<Event> = PEAKS.iter().flat_map(|(_, p)| motion(*p)).collect();
    for n in [0, 1] {
        ev.extend([Event::button(n, true), Event::button(n, false)]);
    }
    ev
}

/// Play the sequence through a fake spacenavd and the client into a calibration.
fn calibrate_through_the_socket(name: &str) -> Calibration {
    let path =
        std::env::temp_dir().join(format!("ve_spacemouse-{}-{name}.sock", std::process::id()));
    let fake = FakeSpacenavd::start(&path).expect("fake spacenavd");
    let mouse = SpaceMouse::start(Options {
        socket: path,
        retry: Duration::from_millis(20),
    });
    assert!(fake.wait_for_clients(1, GUARD));
    let sent = sequence();
    for e in &sent {
        assert_eq!(fake.send(*e), 1);
    }
    let mut cal = Calibration::full(Mapping::default());
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
    cal
}

/// The content's response to holding `motion`'s peak.
fn response(m: &Mapping, motion: Motion) -> Nav {
    let peak = PEAKS
        .iter()
        .find(|(mo, _)| *mo == motion)
        .map(|(_, p)| *p)
        .expect("a peak per motion");
    m.apply(&peak)
}

#[test]
fn the_content_follows_the_hand() {
    let cal = calibrate_through_the_socket("content");
    assert!(cal.done(), "every step measured");
    assert!(
        cal.results()
            .iter()
            .flatten()
            .all(|r| r.warnings.is_empty()),
        "{:?}",
        cal.results()
    );
    let m = cal.mapping();
    assert_eq!(m.convention, Convention::Content);

    let right = response(&m, Motion::SlideRight);
    assert!(
        right.pan.x > 0.0 && right.pan.y == 0.0,
        "slide right: the page moves right, {right:?}"
    );
    assert!(right.pan_screen().x > 0.0);

    let up = response(&m, Motion::LiftUp);
    assert!(
        up.pan.y > 0.0 && up.pan.x == 0.0,
        "lift up: the page moves up, {up:?}"
    );
    assert!(up.pan_screen().y < 0.0, "up on the screen is -y in egui");

    let away = response(&m, Motion::PushAway);
    assert!(
        away.zoom < 0.0,
        "push away: the page gets smaller, {away:?}"
    );

    let twist = response(&m, Motion::TwistClockwise);
    assert!(
        twist.rotate[1] > 0.0 && twist.rotate[2] == 0.0,
        "twist clockwise: the page spins clockwise seen from above, {twist:?}"
    );
    assert!(
        response(&m, Motion::TiltForward).rotate[0] > 0.0,
        "tilt forward: the page tilts forward"
    );
    let rock = response(&m, Motion::RockRight);
    assert!(
        rock.rotate[2] > 0.0 && rock.rotate[1] == 0.0,
        "rock right: the page turns clockwise on the screen, {rock:?}"
    );

    // Each motion moves only its own output.
    assert_eq!(twist.pan, egui::Vec2::ZERO);
    assert_eq!(twist.zoom, 0.0);

    assert_eq!((m.buttons.left, m.buttons.right), (0, 1));
}

#[test]
fn the_camera_convention_turns_every_motion_around() {
    let mut cal = calibrate_through_the_socket("camera");
    cal.set_convention(Convention::Camera);
    let m = cal.mapping();
    assert_eq!(m.convention, Convention::Camera);
    assert!(
        response(&m, Motion::SlideRight).pan.x < 0.0,
        "the view moves right: the page moves left"
    );
    assert!(response(&m, Motion::LiftUp).pan.y < 0.0);
    assert!(
        response(&m, Motion::PushAway).zoom > 0.0,
        "the camera goes in: the page gets bigger"
    );
    assert!(response(&m, Motion::TwistClockwise).rotate[1] < 0.0);
    assert!(response(&m, Motion::RockRight).rotate[2] < 0.0);
}

#[test]
fn the_table_says_what_the_page_does() {
    assert_eq!(
        Motion::SlideRight.effect(Convention::Content),
        "the page moves right"
    );
    assert_eq!(
        Motion::LiftUp.effect(Convention::Content),
        "the page moves up"
    );
    assert_eq!(
        Motion::PushAway.effect(Convention::Content),
        "the page gets smaller"
    );
    assert_eq!(
        Motion::TwistClockwise.effect(Convention::Content),
        "the page spins clockwise seen from above, its right side coming towards you"
    );
    assert_eq!(
        Motion::RockRight.effect(Convention::Content),
        "the page turns clockwise on the screen, its right side going down"
    );
    assert_eq!(
        Motion::TiltForward.effect(Convention::Camera),
        "the page tilts back, its top coming towards you"
    );
    assert_eq!(
        Motion::SlideRight.effect(Convention::Camera),
        "the page moves left"
    );
    assert_eq!(
        Motion::PushAway.effect(Convention::Camera),
        "the page gets bigger"
    );
}
