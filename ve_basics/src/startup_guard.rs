//! Startup crash guard (PLT-4): when the last start died before its first frame, the persisted egui
//! state is moved aside, so a bad saved value (a huge window size that makes surface creation fail,
//! a layout the new build cannot read) does not take the app down at every start.
//!
//! Mechanism: a marker file in the app's storage directory, written before the window opens and
//! removed after the first frame. A marker that is already there at startup means the previous run
//! never reached its first frame.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Name of the marker file inside the storage directory.
const MARKER: &str = "starting";
/// eframe's persisted state file inside the storage directory.
const STATE: &str = "app.ron";

/// Guards one start of the app. Make it with [`StartupGuard::begin`] before the window opens and
/// finish it with [`StartupGuard::first_frame_done`] inside the first frame.
#[derive(Debug)]
pub struct StartupGuard {
    marker: Option<PathBuf>,
    /// Where the broken state was moved, when the previous start had died before its first frame.
    pub recovered: Option<PathBuf>,
}

impl StartupGuard {
    /// Start the guard for the app's storage directory (`eframe::storage_dir(app_id)`). With `None` the
    /// guard does nothing. When the marker of a previous start is still there, `app.ron` is renamed to
    /// `app.ron.broken-<unix seconds>` and [`StartupGuard::recovered`] names it; the app then starts
    /// with default state. Filesystem errors are ignored: the guard must never stop a start.
    pub fn begin(dir: Option<PathBuf>) -> Self {
        let Some(dir) = dir else {
            return Self {
                marker: None,
                recovered: None,
            };
        };
        let marker = dir.join(MARKER);
        let recovered = if marker.exists() {
            move_aside(&dir.join(STATE))
        } else {
            None
        };
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(&marker, b"");
        Self {
            marker: Some(marker),
            recovered,
        }
    }

    /// The first frame rendered: this start is good, the marker goes.
    pub fn first_frame_done(self) {
        if let Some(marker) = self.marker {
            let _ = std::fs::remove_file(marker);
        }
    }
}

/// Rename `path` to `path.broken-<unix seconds>`; `None` when there was nothing to move.
fn move_aside(path: &Path) -> Option<PathBuf> {
    if !path.exists() {
        return None;
    }
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let name = path.file_name()?.to_string_lossy().into_owned();
    let target = path.with_file_name(format!("{name}.broken-{secs}"));
    std::fs::rename(path, &target).ok()?;
    Some(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("ve_basics-guard-{tag}-{nanos}"));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn clean_start_leaves_state_alone() {
        let dir = temp_dir("clean");
        std::fs::create_dir_all(&dir).ok();
        std::fs::write(dir.join(STATE), b"state").ok();
        let guard = StartupGuard::begin(Some(dir.clone()));
        assert!(guard.recovered.is_none());
        assert!(dir.join(MARKER).exists());
        guard.first_frame_done();
        assert!(!dir.join(MARKER).exists());
        assert!(dir.join(STATE).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn stale_marker_moves_state_aside() {
        let dir = temp_dir("stale");
        std::fs::create_dir_all(&dir).ok();
        std::fs::write(dir.join(STATE), b"state").ok();
        std::fs::write(dir.join(MARKER), b"").ok();
        let guard = StartupGuard::begin(Some(dir.clone()));
        let moved = guard.recovered.clone().expect("state moved aside");
        assert!(moved.exists());
        assert!(!dir.join(STATE).exists());
        assert!(
            dir.join(MARKER).exists(),
            "a fresh marker guards this start"
        );
        guard.first_frame_done();
        assert!(!dir.join(MARKER).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn stale_marker_without_state_is_fine() {
        let dir = temp_dir("nostate");
        std::fs::create_dir_all(&dir).ok();
        std::fs::write(dir.join(MARKER), b"").ok();
        let guard = StartupGuard::begin(Some(dir.clone()));
        assert!(guard.recovered.is_none());
        guard.first_frame_done();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn no_dir_is_a_noop() {
        let guard = StartupGuard::begin(None);
        assert!(guard.recovered.is_none());
        guard.first_frame_done();
    }
}
