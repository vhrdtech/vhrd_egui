//! Window position from Hyprland, over its IPC sockets (no `hyprctl` process).
//!
//! A thread listens on the event socket (`.socket2.sock`) and, whenever anything happens on the desktop,
//! asks the request socket (`.socket.sock`) for `j/clients` (our window by pid: `at`, `size`, its monitor)
//! and `j/monitors` (that monitor's position, size, scale, rotation and name). The UI thread only reads the
//! last answer. Hyprland sends no event while a floating window is dragged, so while ours floats the
//! thread also asks a few times per second.

use std::io::{BufRead, BufReader, ErrorKind, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use egui::{Rect, pos2, vec2};
use serde_json::Value;

use crate::position::{Placement, PositionProvider};

/// Ask again this often even without an event (a missed one, an animation that ended).
const REFRESH: Duration = Duration::from_secs(1);
/// Ask this often while the window floats: dragging it sends no event.
const REFRESH_FLOATING: Duration = Duration::from_millis(50);
/// After an event, wait this long for the burst to end before asking.
const SETTLE: Duration = Duration::from_millis(15);
/// A request that takes longer than this is given up.
const REQUEST_TIMEOUT: Duration = Duration::from_millis(500);

#[derive(Default)]
struct Shared {
    placement: Mutex<Option<Placement>>,
    stop: AtomicBool,
}

impl Shared {
    fn get(&self) -> Option<Placement> {
        self.placement.lock().ok().and_then(|p| p.clone())
    }

    /// Store a new answer; returns whether it differs from the last one.
    fn set(&self, placement: Option<Placement>) -> bool {
        match self.placement.lock() {
            Ok(mut current) if *current != placement => {
                *current = placement;
                true
            }
            _ => false,
        }
    }
}

/// The window's place on a Hyprland desktop, kept up to date by a listener thread. Dropping it stops the
/// thread.
pub struct Hyprland {
    shared: Arc<Shared>,
}

impl Hyprland {
    /// Connect to the Hyprland this process runs under (`HYPRLAND_INSTANCE_SIGNATURE`,
    /// `XDG_RUNTIME_DIR`). `wake` is called from the listener thread whenever the window's place changed
    /// (`ctx.request_repaint()`). `None` when this is not Hyprland or its sockets do not answer.
    pub fn connect(wake: impl Fn() + Send + 'static) -> Option<Self> {
        let signature = std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE")?;
        let dir = PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR")?)
            .join("hypr")
            .join(signature);
        Self::with_sockets(
            dir.join(".socket.sock"),
            &dir.join(".socket2.sock"),
            std::process::id(),
            wake,
        )
    }

    /// Like [`Hyprland::connect`] with the request socket, the event socket and the window's process id
    /// given (tests, a window of another process).
    pub fn with_sockets(
        request: PathBuf,
        events: &Path,
        pid: u32,
        wake: impl Fn() + Send + 'static,
    ) -> Option<Self> {
        let events = UnixStream::connect(events).ok()?;
        let shared = Arc::new(Shared::default());
        let thread_shared = shared.clone();
        std::thread::Builder::new()
            .name("ve_backdrop hyprland".to_owned())
            .spawn(move || listen(&thread_shared, &request, events, pid, &wake))
            .ok()?;
        Some(Self { shared })
    }
}

impl Drop for Hyprland {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
    }
}

impl PositionProvider for Hyprland {
    fn placement(&self) -> Option<Placement> {
        self.shared.get()
    }
}

/// One request on Hyprland's request socket: a connection per command, the answer is everything sent back.
fn request(socket: &Path, command: &str) -> std::io::Result<String> {
    let mut stream = UnixStream::connect(socket)?;
    stream.set_read_timeout(Some(REQUEST_TIMEOUT))?;
    stream.set_write_timeout(Some(REQUEST_TIMEOUT))?;
    stream.write_all(command.as_bytes())?;
    let mut answer = String::new();
    stream.read_to_string(&mut answer)?;
    Ok(answer)
}

fn query(socket: &Path, pid: u32) -> Option<(Placement, bool)> {
    let clients = request(socket, "j/clients").ok()?;
    let monitors = request(socket, "j/monitors").ok()?;
    placement_from(&clients, &monitors, pid)
}

/// The listener thread: ask, wait for an event (or the refresh time), ask again; ends when the provider is
/// dropped or Hyprland goes away.
fn listen(shared: &Shared, request_socket: &Path, events: UnixStream, pid: u32, wake: &dyn Fn()) {
    let mut reader = BufReader::new(events);
    let mut line = Vec::new();
    loop {
        let answer = query(request_socket, pid);
        let floating = answer.as_ref().is_some_and(|(_, floating)| *floating);
        if shared.set(answer.map(|(placement, _)| placement)) {
            wake();
        }
        let wait = if floating { REFRESH_FLOATING } else { REFRESH };
        let alive = wait_for_event(&mut reader, &mut line, wait)
            // Let the rest of a burst of events arrive, then ask once.
            && wait_for_quiet(&mut reader, &mut line);
        if !alive || shared.stop.load(Ordering::Relaxed) {
            break;
        }
    }
    if shared.set(None) {
        wake();
    }
}

/// Read up to one event line or until `timeout`. `false` when the socket is closed or broken.
fn wait_for_event(
    reader: &mut BufReader<UnixStream>,
    line: &mut Vec<u8>,
    timeout: Duration,
) -> bool {
    if reader.get_ref().set_read_timeout(Some(timeout)).is_err() {
        return false;
    }
    match reader.read_until(b'\n', line) {
        Ok(0) => false,
        Ok(_) => {
            // What the event says does not matter: anything may have moved the window.
            if line.ends_with(b"\n") {
                line.clear();
            }
            true
        }
        // A timeout keeps what was read of a half-arrived line in `line`.
        Err(e) => matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut),
    }
}

/// Swallow events until none arrives for [`SETTLE`]. `false` when the socket is closed or broken.
fn wait_for_quiet(reader: &mut BufReader<UnixStream>, line: &mut Vec<u8>) -> bool {
    if reader.get_ref().set_read_timeout(Some(SETTLE)).is_err() {
        return false;
    }
    loop {
        match reader.read_until(b'\n', line) {
            Ok(0) => return false,
            Ok(_) => {
                if line.ends_with(b"\n") {
                    line.clear();
                }
            }
            Err(e) => return matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut),
        }
    }
}

fn number(value: &Value, key: &str) -> Option<f32> {
    value.get(key)?.as_f64().map(|v| v as f32)
}

fn pair(value: &Value, key: &str) -> Option<egui::Vec2> {
    let pair = value.get(key)?.as_array()?;
    Some(vec2(
        pair.first()?.as_f64()? as f32,
        pair.get(1)?.as_f64()? as f32,
    ))
}

/// The place of the window of process `pid` from the answers to `j/clients` and `j/monitors`, and whether
/// it floats. With several windows of the process, the one focused last.
pub(crate) fn placement_from(clients: &str, monitors: &str, pid: u32) -> Option<(Placement, bool)> {
    let clients: Value = serde_json::from_str(clients).ok()?;
    let monitors: Value = serde_json::from_str(monitors).ok()?;
    let flag = |c: &Value, key: &str| c.get(key).and_then(Value::as_bool);
    let client = clients
        .as_array()?
        .iter()
        .filter(|c| c.get("pid").and_then(Value::as_u64) == Some(u64::from(pid)))
        .filter(|c| flag(c, "mapped") != Some(false) && flag(c, "hidden") != Some(true))
        .min_by_key(|c| {
            c.get("focusHistoryID")
                .and_then(Value::as_i64)
                .unwrap_or(i64::MAX)
        })?;
    let at = pair(client, "at")?;
    let size = pair(client, "size")?;
    let monitor_id = client.get("monitor")?.as_i64()?;
    let monitor = monitors
        .as_array()?
        .iter()
        .find(|m| m.get("id").and_then(Value::as_i64) == Some(monitor_id))?;
    let origin = vec2(number(monitor, "x")?, number(monitor, "y")?);
    let scale = number(monitor, "scale").filter(|s| *s > 0.0).unwrap_or(1.0);
    let mut logical = vec2(number(monitor, "width")?, number(monitor, "height")?) / scale;
    // Transforms 1, 3, 5 and 7 are the quarter turns: width and height swap.
    if monitor
        .get("transform")
        .and_then(Value::as_i64)
        .unwrap_or(0)
        % 2
        == 1
    {
        logical = vec2(logical.y, logical.x);
    }
    if size.x <= 0.0 || size.y <= 0.0 || logical.x <= 0.0 || logical.y <= 0.0 {
        return None;
    }
    let placement = Placement {
        window: Rect::from_min_size(pos2(at.x - origin.x, at.y - origin.y), size),
        monitor: logical,
        output: monitor
            .get("name")
            .and_then(Value::as_str)
            .map(str::to_owned),
    };
    Some((placement, flag(client, "floating") == Some(true)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;
    use std::sync::atomic::AtomicI32;
    use std::time::Instant;

    const MONITORS: &str = r#"[
        {"id": 0, "name": "eDP-1", "width": 2560, "height": 1600, "x": 0, "y": 0, "scale": 2.0, "transform": 0},
        {"id": 1, "name": "DP-1", "width": 1920, "height": 1080, "x": 1280, "y": 0, "scale": 1.0, "transform": 1}
    ]"#;

    fn clients(x: i32, floating: bool) -> String {
        format!(
            r#"[
            {{"pid": 7, "at": [0, 0], "size": [10, 10], "monitor": 0, "mapped": true, "hidden": false,
              "floating": false, "focusHistoryID": 0}},
            {{"pid": 42, "at": [5, 5], "size": [50, 50], "monitor": 0, "mapped": true, "hidden": true,
              "floating": false, "focusHistoryID": 0}},
            {{"pid": 42, "at": [900, 40], "size": [300, 200], "monitor": 0, "mapped": true, "hidden": false,
              "floating": false, "focusHistoryID": 5}},
            {{"pid": 42, "at": [{x}, 123], "size": [640, 400], "monitor": 0, "mapped": true, "hidden": false,
              "floating": {floating}, "focusHistoryID": 1}}
            ]"#
        )
    }

    #[test]
    fn our_window_on_its_monitor() {
        let (p, floating) = placement_from(&clients(200, false), MONITORS, 42).expect("found");
        assert_eq!(
            p.window,
            Rect::from_min_size(pos2(200.0, 123.0), vec2(640.0, 400.0))
        );
        assert_eq!(
            p.monitor,
            vec2(1280.0, 800.0),
            "physical size over the scale"
        );
        assert_eq!(p.output.as_deref(), Some("eDP-1"));
        assert!(!floating);
        assert!(
            placement_from(&clients(200, true), MONITORS, 42)
                .expect("found")
                .1
        );
    }

    #[test]
    fn a_second_rotated_monitor() {
        let clients = r#"[{"pid": 42, "at": [1380, 60], "size": [400, 300], "monitor": 1}]"#;
        let (p, _) = placement_from(clients, MONITORS, 42).expect("found");
        assert_eq!(p.window.min, pos2(100.0, 60.0), "relative to the monitor");
        assert_eq!(
            p.monitor,
            vec2(1080.0, 1920.0),
            "a quarter turn swaps the sides"
        );
        assert_eq!(p.output.as_deref(), Some("DP-1"));
    }

    #[test]
    fn nonsense_is_no_answer() {
        assert!(placement_from("", MONITORS, 42).is_none());
        assert!(placement_from("{}", "[]", 42).is_none());
        assert!(placement_from(&clients(0, false), "[]", 42).is_none());
        assert!(placement_from(&clients(0, false), MONITORS, 43).is_none());
        let empty = r#"[{"pid": 42, "at": [0, 0], "size": [0, 0], "monitor": 0}]"#;
        assert!(placement_from(empty, MONITORS, 42).is_none());
    }

    fn wait_until(what: &str, mut done: impl FnMut() -> bool) {
        let start = Instant::now();
        while !done() {
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "timed out: {what}"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// A fake Hyprland: a request socket that answers `j/clients` and `j/monitors`, an event socket the
    /// test writes events to. The provider follows an event, and forgets the place when Hyprland goes away.
    #[test]
    fn follows_events_from_a_fake_hyprland() {
        let dir = std::env::temp_dir().join(format!("ve_backdrop_hypr_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let request_path = dir.join(".socket.sock");
        let events_path = dir.join(".socket2.sock");
        let _ = std::fs::remove_file(&request_path);
        let _ = std::fs::remove_file(&events_path);

        let x = Arc::new(AtomicI32::new(200));
        let requests = UnixListener::bind(&request_path).expect("request socket");
        let server_x = x.clone();
        std::thread::spawn(move || {
            for stream in requests.incoming() {
                let Ok(mut stream) = stream else { break };
                let mut command = [0u8; 64];
                let n = stream.read(&mut command).unwrap_or(0);
                let answer = match &command[..n] {
                    b"j/clients" => clients(server_x.load(Ordering::Relaxed), false),
                    b"j/monitors" => MONITORS.to_owned(),
                    _ => "unknown request".to_owned(),
                };
                let _ = stream.write_all(answer.as_bytes());
            }
        });
        let events = UnixListener::bind(&events_path).expect("event socket");

        let woken = Arc::new(AtomicI32::new(0));
        let wake_count = woken.clone();
        let provider = Hyprland::with_sockets(request_path, &events_path, 42, move || {
            wake_count.fetch_add(1, Ordering::Relaxed);
        })
        .expect("connects to the fake");
        let (mut event_stream, _) = events.accept().expect("the provider listens");

        wait_until("the first answer", || provider.placement().is_some());
        assert_eq!(
            provider.placement().expect("placed").window.min,
            pos2(200.0, 123.0)
        );
        assert_eq!(woken.load(Ordering::Relaxed), 1);

        x.store(520, Ordering::Relaxed);
        event_stream
            .write_all(b"activewindow>>kitty,term\nmovewindowv2>>abc,2,two\n")
            .expect("send events");
        wait_until("the move is seen", || {
            provider
                .placement()
                .is_some_and(|p| p.window.min.x == 520.0)
        });
        assert_eq!(woken.load(Ordering::Relaxed), 2, "one wake for the burst");

        drop(event_stream);
        wait_until("Hyprland gone: no place", || provider.placement().is_none());

        drop(provider);
        std::fs::remove_dir_all(&dir).expect("clean up");
    }

    #[test]
    fn no_socket_no_provider() {
        let missing = std::env::temp_dir().join("ve_backdrop_no_such_socket");
        assert!(Hyprland::with_sockets(missing.clone(), &missing, 1, || {}).is_none());
    }
}

#[cfg(test)]
mod real {
    use super::*;

    /// Against the Hyprland this runs under, for the window of process `VE_TEST_PID`:
    /// `VE_TEST_PID=$(hyprctl activewindow -j | jq .pid) cargo test -p ve_backdrop real_hyprland -- --ignored --nocapture`
    #[test]
    #[ignore = "needs a running Hyprland and VE_TEST_PID"]
    fn real_hyprland() {
        let pid = std::env::var("VE_TEST_PID")
            .ok()
            .and_then(|p| p.parse().ok())
            .expect("VE_TEST_PID names a process with a window");
        let dir = PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR").expect("XDG_RUNTIME_DIR"))
            .join("hypr")
            .join(std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").expect("under Hyprland"));
        let provider = Hyprland::with_sockets(
            dir.join(".socket.sock"),
            &dir.join(".socket2.sock"),
            pid,
            || {},
        )
        .expect("Hyprland's sockets answer");
        let start = std::time::Instant::now();
        while provider.placement().is_none() && start.elapsed() < Duration::from_secs(3) {
            std::thread::sleep(Duration::from_millis(10));
        }
        let placement = provider.placement().expect("the window is found");
        println!("{placement:?} after {:?}", start.elapsed());
        assert!(placement.monitor.x > 0.0 && placement.window.width() > 0.0);
    }
}
