//! The client against a fake spacenavd the test serves (SPM-1): events arrive, the state follows them, and the
//! client comes back after the daemon restarts or appears late. Every wait is for a condition, with a long
//! deadline only as a failure guard.

#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ve_spacemouse::{Event, FakeSpacenavd, Options, SpaceMouse, Status};

const GUARD: Duration = Duration::from_secs(10);

/// A socket path of this test's own in the temp dir, removed by the fake when it drops.
fn socket(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("ve_spacemouse-{}-{name}.sock", std::process::id()))
}

fn options(path: &Path) -> Options {
    Options {
        socket: path.to_path_buf(),
        retry: Duration::from_millis(20),
    }
}

/// Poll until `n` events have arrived.
fn take(mouse: &SpaceMouse, n: usize) -> Vec<Event> {
    let deadline = Instant::now() + GUARD;
    let mut got = Vec::new();
    while got.len() < n {
        assert!(
            Instant::now() < deadline,
            "only {got:?} of {n} events arrived"
        );
        got.extend(mouse.poll());
        std::thread::yield_now();
    }
    got
}

fn until(what: &str, mut cond: impl FnMut() -> bool) {
    let deadline = Instant::now() + GUARD;
    while !cond() {
        assert!(Instant::now() < deadline, "never happened: {what}");
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn events_arrive_in_order_and_the_state_follows() {
    let path = socket("events");
    let fake = FakeSpacenavd::start(&path).expect("fake spacenavd");
    let mouse = SpaceMouse::start(options(&path));
    assert!(fake.wait_for_clients(1, GUARD));
    let sent = [
        Event::motion([100, -20, 0, 0, 0, 5]),
        Event::button(1, true),
        Event::Motion {
            axes: [0; 6],
            period_ms: 16,
        },
    ];
    for e in sent {
        assert_eq!(fake.send(e), 1);
    }
    assert_eq!(take(&mouse, 3), sent);
    assert_eq!(mouse.axes(), [0; 6]);
    assert_eq!(mouse.held().into_iter().collect::<Vec<_>>(), vec![1]);
    assert_eq!(mouse.status(), Status::Connected);
}

#[test]
fn a_restart_releases_held_buttons_and_reconnects() {
    let path = socket("restart");
    let fake = FakeSpacenavd::start(&path).expect("fake spacenavd");
    let mouse = SpaceMouse::start(options(&path));
    assert!(fake.wait_for_clients(1, GUARD));
    fake.send(Event::button(0, true));
    fake.send(Event::motion([50, 0, 0, 0, 0, 0]));
    take(&mouse, 2);

    fake.disconnect_all();
    // Nothing stays stuck: the held button goes up and the puck comes to rest.
    let after = take(&mouse, 2);
    assert_eq!(after, vec![Event::button(0, false), Event::motion([0; 6])]);
    assert!(mouse.held().is_empty());

    assert!(fake.wait_for_clients(1, GUARD), "the client comes back");
    until("connected again", || mouse.status() == Status::Connected);
    fake.send(Event::button(1, true));
    assert_eq!(take(&mouse, 1), vec![Event::button(1, true)]);
}

#[test]
fn no_daemon_says_why_and_connects_once_it_appears() {
    let path = socket("late");
    let _ = std::fs::remove_file(&path);
    let mouse = SpaceMouse::start(options(&path));
    until("down with a reason", || {
        matches!(mouse.status(), Status::Down(_))
    });
    let Status::Down(why) = mouse.status() else {
        unreachable!()
    };
    assert!(
        why.contains("no socket at") && why.contains("spacenavd"),
        "{why}"
    );
    assert!(mouse.status().line().starts_with("not connected: "));

    let fake = FakeSpacenavd::start(&path).expect("fake spacenavd");
    assert!(fake.wait_for_clients(1, GUARD));
    until("connected", || mouse.status() == Status::Connected);
}

#[test]
fn dropping_the_client_ends_its_thread_while_it_waits_to_retry() {
    let path = socket("drop");
    let _ = std::fs::remove_file(&path);
    let mouse = SpaceMouse::start(Options {
        socket: path,
        retry: Duration::from_secs(3600),
    });
    until("first attempt done", || {
        matches!(mouse.status(), Status::Down(_))
    });
    // Returns at once: Drop wakes the thread instead of waiting out the hour.
    drop(mouse);
}

#[test]
fn dropping_the_client_while_connected_closes_the_connection() {
    let path = socket("drop-connected");
    let fake = FakeSpacenavd::start(&path).expect("fake spacenavd");
    let mouse = SpaceMouse::start(options(&path));
    assert!(fake.wait_for_clients(1, GUARD));
    drop(mouse);
    until("the fake sees the client gone", || {
        fake.send(Event::button(0, true)) == 0
    });
}
