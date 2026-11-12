//! The client of spacenavd's socket: a thread that reads events, reconnects when spacenavd restarts, and hands the
//! events to the UI through a channel (SPM-1).

use std::collections::BTreeSet;
use std::io::{ErrorKind, Read};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::event::{Axes, EVENT_SIZE, Event};

/// Where spacenavd listens unless `/etc/spnavrc` says otherwise.
pub const DEFAULT_SOCKET: &str = "/var/run/spnav.sock";

/// How [`SpaceMouse`] connects.
#[derive(Clone, Debug)]
pub struct Options {
    /// spacenavd's socket, [`DEFAULT_SOCKET`] by default.
    pub socket: PathBuf,
    /// The wait before the next connection attempt after spacenavd went away or was not there; 2 s by default.
    pub retry: Duration,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            socket: PathBuf::from(DEFAULT_SOCKET),
            retry: Duration::from_secs(2),
        }
    }
}

/// Whether the client is talking to spacenavd now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    /// The first attempt has not finished yet.
    Connecting,
    /// Connected; events arrive.
    Connected,
    /// Not connected: why, and the next step for the person. The client keeps trying every [`Options::retry`].
    Down(String),
}

impl Status {
    /// A short line for a status bar or a settings page.
    pub fn line(&self) -> String {
        match self {
            Status::Connecting => "connecting to spacenavd".into(),
            Status::Connected => "connected to spacenavd".into(),
            Status::Down(why) => format!("not connected: {why}"),
        }
    }
}

struct Shared {
    stop: AtomicBool,
    status: Mutex<Status>,
    /// The open connection, so dropping the client can shut it and end a blocked read.
    stream: Mutex<Option<UnixStream>>,
    repaint: Mutex<Option<egui::Context>>,
    axes: Mutex<Axes>,
    held: Mutex<BTreeSet<i32>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// A connection to spacenavd, kept up by a thread of its own.
///
/// ```no_run
/// let mouse = ve_spacemouse::SpaceMouse::start(Default::default());
/// // once per frame, in ui():
/// for event in mouse.poll() {
///     // buttons go to the pie menu, see `PieMenu::show`
/// }
/// let deflection = mouse.axes(); // the puck now, for `Mapping::apply`
/// ```
///
/// The thread never blocks the UI: [`SpaceMouse::poll`] only drains a channel. When spacenavd goes away the
/// client says why in [`SpaceMouse::status`], releases the buttons it saw held and puts the axes back to rest
/// (the events of that are in the next poll, so a pie menu closes), and tries again every [`Options::retry`].
pub struct SpaceMouse {
    rx: Receiver<Event>,
    shared: Arc<Shared>,
    socket: PathBuf,
    thread: Option<JoinHandle<()>>,
}

impl SpaceMouse {
    /// Start the client thread. It connects in the background; nothing here blocks.
    pub fn start(options: Options) -> Self {
        let (tx, rx) = channel();
        let shared = Arc::new(Shared {
            stop: AtomicBool::new(false),
            status: Mutex::new(Status::Connecting),
            stream: Mutex::new(None),
            repaint: Mutex::new(None),
            axes: Mutex::new([0; 6]),
            held: Mutex::new(BTreeSet::new()),
        });
        let socket = options.socket.clone();
        let thread = {
            let shared = shared.clone();
            std::thread::Builder::new()
                .name("ve_spacemouse".into())
                .spawn(move || run(options, shared, tx))
                .map_err(|e| log::error!("ve_spacemouse: no thread for the spacenavd client: {e}"))
                .ok()
        };
        if thread.is_none() {
            *lock(&shared.status) = Status::Down("could not start the client thread (see the log)".into());
        }
        Self {
            rx,
            shared,
            socket,
            thread,
        }
    }

    /// Ask egui for a repaint on every event, so the app sees the puck move without polling on a timer.
    pub fn with_repaint(self, ctx: &egui::Context) -> Self {
        *lock(&self.shared.repaint) = Some(ctx.clone());
        self
    }

    /// The events since the last call, oldest first. Cheap: call it every frame.
    pub fn poll(&self) -> Vec<Event> {
        self.rx.try_iter().collect()
    }

    /// The puck's deflection now (the last motion event; all zero at rest or while not connected).
    pub fn axes(&self) -> Axes {
        *lock(&self.shared.axes)
    }

    /// The buttons held now, by spacenavd's number.
    pub fn held(&self) -> BTreeSet<i32> {
        lock(&self.shared.held).clone()
    }

    /// Whether the client is connected now, or why not.
    pub fn status(&self) -> Status {
        lock(&self.shared.status).clone()
    }

    /// The socket this client connects to.
    pub fn socket(&self) -> &Path {
        &self.socket
    }
}

impl Drop for SpaceMouse {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        if let Some(stream) = lock(&self.shared.stream).take() {
            let _ = stream.shutdown(std::net::Shutdown::Both);
        }
        if let Some(thread) = self.thread.take() {
            thread.thread().unpark();
            let _ = thread.join();
        }
    }
}

/// Why a connection attempt failed, with the next step for the person.
fn why_not(socket: &Path, e: &std::io::Error) -> String {
    let at = socket.display();
    match e.kind() {
        ErrorKind::NotFound => format!("no socket at {at}: is spacenavd installed and running? (systemctl status spacenavd)"),
        ErrorKind::PermissionDenied => format!("no permission to open {at}"),
        ErrorKind::ConnectionRefused => {
            format!("nothing listens on {at}: spacenavd stopped and left its socket (systemctl restart spacenavd)")
        }
        _ => format!("{at}: {e}"),
    }
}

fn run(options: Options, shared: Arc<Shared>, tx: Sender<Event>) {
    let send = |e: Event| {
        if tx.send(e).is_ok()
            && let Some(ctx) = lock(&shared.repaint).as_ref()
        {
            ctx.request_repaint();
        }
    };
    while !shared.stop.load(Ordering::SeqCst) {
        match UnixStream::connect(&options.socket) {
            Ok(stream) => {
                let reader = stream.try_clone();
                *lock(&shared.stream) = Some(stream);
                if shared.stop.load(Ordering::SeqCst) {
                    break;
                }
                *lock(&shared.status) = Status::Connected;
                send_status_change(&shared);
                let why = match reader {
                    Ok(mut reader) => read_events(&mut reader, &shared, &send),
                    Err(e) => format!("{}: {e}", options.socket.display()),
                };
                lock(&shared.stream).take();
                if shared.stop.load(Ordering::SeqCst) {
                    break;
                }
                log::warn!("ve_spacemouse: {why}");
                *lock(&shared.status) = Status::Down(why);
                to_rest(&shared, &send);
            }
            Err(e) => {
                let why = why_not(&options.socket, &e);
                let mut status = lock(&shared.status);
                if *status != Status::Down(why.clone()) {
                    log::info!("ve_spacemouse: {why}");
                    *status = Status::Down(why);
                    drop(status);
                    send_status_change(&shared);
                }
            }
        }
        // Woken early by Drop through unpark.
        if !shared.stop.load(Ordering::SeqCst) {
            std::thread::park_timeout(options.retry);
        }
    }
}

fn send_status_change(shared: &Shared) {
    if let Some(ctx) = lock(&shared.repaint).as_ref() {
        ctx.request_repaint();
    }
}

/// Read events until the connection ends; the reason it ended.
fn read_events(stream: &mut UnixStream, shared: &Shared, send: &impl Fn(Event)) -> String {
    let mut buf = [0u8; EVENT_SIZE];
    loop {
        match stream.read_exact(&mut buf) {
            Ok(()) => {
                let event = Event::from_bytes(&buf);
                match event {
                    Event::Motion { axes, .. } => *lock(&shared.axes) = axes,
                    Event::Button { number, pressed } => {
                        let mut held = lock(&shared.held);
                        if pressed {
                            held.insert(number);
                        } else {
                            held.remove(&number);
                        }
                    }
                    Event::Other { kind } => log::debug!("ve_spacemouse: event type {kind} ignored"),
                }
                send(event);
            }
            Err(e) if e.kind() == ErrorKind::UnexpectedEof => {
                return "spacenavd closed the connection (stopped or restarted); reconnecting".into();
            }
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => return format!("reading from spacenavd failed: {e}; reconnecting"),
        }
    }
}

/// After a lost connection: release what was held and put the puck at rest, so nothing stays stuck.
fn to_rest(shared: &Shared, send: &impl Fn(Event)) {
    let held = std::mem::take(&mut *lock(&shared.held));
    for number in held {
        send(Event::button(number, false));
    }
    let was = std::mem::replace(&mut *lock(&shared.axes), [0; 6]);
    if was != [0; 6] {
        send(Event::motion([0; 6]));
    }
}
