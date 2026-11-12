//! A stand-in for spacenavd: listens on a socket of its own and sends the events a test asks for (SPM-1).
//!
//! It speaks the same protocol as spacenavd for a client that sends nothing (eight `i32`s per event), so a test
//! drives [`crate::SpaceMouse`] exactly as the real daemon would, and an app can run against it on a PC without
//! the device.

use std::io::Write;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::event::Event;

struct Clients {
    list: Mutex<Vec<UnixStream>>,
    joined: Condvar,
}

/// A fake spacenavd on `path`. Dropping it closes every connection and removes the socket.
pub struct FakeSpacenavd {
    path: PathBuf,
    clients: Arc<Clients>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl FakeSpacenavd {
    /// Listen on `path` (a stale file there is removed first) and accept clients on a thread.
    pub fn start(path: impl Into<PathBuf>) -> std::io::Result<Self> {
        let path = path.into();
        let _ = std::fs::remove_file(&path);
        let listener = UnixListener::bind(&path)?;
        let clients = Arc::new(Clients {
            list: Mutex::new(Vec::new()),
            joined: Condvar::new(),
        });
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let (clients, stop) = (clients.clone(), stop.clone());
            std::thread::Builder::new()
                .name("fake spacenavd".into())
                .spawn(move || {
                    for stream in listener.incoming() {
                        if stop.load(Ordering::SeqCst) {
                            break;
                        }
                        if let Ok(stream) = stream {
                            let mut list = clients.list.lock().unwrap_or_else(|p| p.into_inner());
                            list.push(stream);
                            clients.joined.notify_all();
                        }
                    }
                })?
        };
        Ok(Self {
            path,
            clients,
            stop,
            thread: Some(thread),
        })
    }

    /// The socket this fake listens on.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The number of clients connected now.
    pub fn clients(&self) -> usize {
        self.clients.list.lock().unwrap_or_else(|p| p.into_inner()).len()
    }

    /// Wait until at least `n` clients are connected; `false` if that did not happen within `timeout` (a failure
    /// guard for tests, never a pause that decides an outcome).
    pub fn wait_for_clients(&self, n: usize, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        let mut list = self.clients.list.lock().unwrap_or_else(|p| p.into_inner());
        while list.len() < n {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return false;
            }
            list = match self.clients.joined.wait_timeout(list, left) {
                Ok((list, _)) => list,
                Err(p) => p.into_inner().0,
            };
        }
        true
    }

    /// Send one event to every client; the number it reached. A client that went away is dropped.
    pub fn send(&self, event: Event) -> usize {
        let bytes = event.to_bytes();
        let mut list = self.clients.list.lock().unwrap_or_else(|p| p.into_inner());
        list.retain_mut(|s| s.write_all(&bytes).is_ok());
        list.len()
    }

    /// Close every connection, as spacenavd does when it stops or restarts. Clients may connect again.
    pub fn disconnect_all(&self) {
        let mut list = self.clients.list.lock().unwrap_or_else(|p| p.into_inner());
        for s in list.drain(..) {
            let _ = s.shutdown(std::net::Shutdown::Both);
        }
    }
}

impl Drop for FakeSpacenavd {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.disconnect_all();
        // Wake the blocked accept with a connection of our own.
        let _ = UnixStream::connect(&self.path);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        let _ = std::fs::remove_file(&self.path);
    }
}
