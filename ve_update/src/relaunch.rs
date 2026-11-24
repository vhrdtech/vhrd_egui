//! The restart: the installed program started again the way this one was, the window where it was.

use std::ffi::OsString;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

/// A restart asked for, done once the window has closed.
///
/// Cloned into the app and the bar; the bar's Restart sets it once and closes the window. After
/// `eframe::run_native` returns, the app calls [`Relaunch::exec`].
#[derive(Clone, Debug, Default)]
pub struct Relaunch {
    planned: Arc<OnceLock<Planned>>,
}

/// What a restart starts.
#[derive(Clone, Debug, PartialEq)]
pub struct Planned {
    /// The installed program.
    pub program: PathBuf,
    /// This process's arguments, without the program.
    pub args: Vec<OsString>,
    /// Where the window was.
    pub placement: Placement,
}

impl Relaunch {
    /// No restart asked for yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Plans a restart into `program` with this process's arguments and the window's placement, and closes the
    /// window. False when a restart was planned already (the first one stays).
    pub fn request(&self, ctx: &egui::Context, program: PathBuf) -> bool {
        let planned = Planned {
            program,
            args: std::env::args_os().skip(1).collect(),
            placement: Placement::of(ctx),
        };
        let first = self.planned.set(planned).is_ok();
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        first
    }

    /// The restart planned, if any.
    pub fn planned(&self) -> Option<&Planned> {
        self.planned.get()
    }

    /// Starts the planned program, after the window closed; `Ok` with nothing planned.
    ///
    /// On Unix the program replaces this process (same pid, parent and environment) and this returns only when it
    /// could not start; the placement waits in `store` under the pid for [`PlacementStore::take_own`]. Elsewhere
    /// the program starts as a new process, without the placement, and this returns once it started.
    pub fn exec(&self, store: &PlacementStore) -> std::io::Result<()> {
        let Some(p) = self.planned.get() else {
            return Ok(());
        };
        let mut cmd = std::process::Command::new(&p.program);
        cmd.args(&p.args);
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt as _;
            let pid = std::process::id();
            store.save(pid, &p.placement)?;
            let failed = cmd.exec();
            store.take(pid);
            Err(failed)
        }
        #[cfg(not(unix))]
        {
            let _ = store;
            cmd.spawn().map(drop)
        }
    }
}

/// Where the window was: its size and position, maximized or full screen.
///
/// Wayland lets no app place its window, so there the position is ignored and only the size and state carry over.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Placement {
    /// The outer top-left corner, in points.
    pub pos: Option<egui::Pos2>,
    /// The inner size, in points.
    pub size: Option<egui::Vec2>,
    /// The window was maximized.
    pub maximized: bool,
    /// The window was full screen.
    pub fullscreen: bool,
}

/// The largest coordinate or size a placement carries, in points: beyond it the text is not a placement.
const COORD_MAX: f32 = 100_000.0;
/// The longest placement file read.
const FILE_BYTES_MAX: u64 = 256;

impl Placement {
    /// The placement of `ctx`'s root window now.
    pub fn of(ctx: &egui::Context) -> Self {
        ctx.input(|i| {
            let v = i.viewport();
            Self {
                pos: v.outer_rect.map(|r| r.min),
                size: v.inner_rect.map(|r| r.size()),
                maximized: v.maximized.unwrap_or(false),
                fullscreen: v.fullscreen.unwrap_or(false),
            }
        })
    }

    /// `builder` with this placement on it.
    pub fn apply(&self, mut builder: egui::ViewportBuilder) -> egui::ViewportBuilder {
        if let Some(size) = self.size {
            builder = builder.with_inner_size(size);
        }
        if let Some(pos) = self.pos {
            builder = builder.with_position(pos);
        }
        builder
            .with_maximized(self.maximized)
            .with_fullscreen(self.fullscreen)
    }

    /// `pos x y`, `size w h`, `maximized`, `fullscreen`, a line each.
    fn text(&self) -> String {
        let mut t = String::new();
        if let Some(p) = self.pos {
            t.push_str(&format!("pos {} {}\n", p.x, p.y));
        }
        if let Some(s) = self.size {
            t.push_str(&format!("size {} {}\n", s.x, s.y));
        }
        if self.maximized {
            t.push_str("maximized\n");
        }
        if self.fullscreen {
            t.push_str("fullscreen\n");
        }
        t
    }

    /// The placement [`Self::text`] wrote; `None` for anything else.
    fn parse(text: &str) -> Option<Self> {
        let mut p = Self::default();
        for line in text.lines() {
            let mut words = line.split(' ');
            let key = words.next()?;
            let mut num = || -> Option<f32> {
                let n: f32 = words.next()?.parse().ok()?;
                (n.is_finite() && n.abs() <= COORD_MAX).then_some(n)
            };
            match key {
                "pos" => p.pos = Some(egui::pos2(num()?, num()?)),
                "size" => {
                    let s = egui::vec2(num()?, num()?);
                    if s.x < 1.0 || s.y < 1.0 {
                        return None;
                    }
                    p.size = Some(s);
                }
                "maximized" => p.maximized = true,
                "fullscreen" => p.fullscreen = true,
                _ => return None,
            }
            if words.next().is_some() {
                return None;
            }
        }
        Some(p)
    }
}

/// Where a placement waits for the restarted process, a file per pid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlacementStore {
    dir: PathBuf,
}

impl PlacementStore {
    /// `$XDG_RUNTIME_DIR/ve_update`, else `ve_update` in the temp dir.
    pub fn runtime() -> Self {
        let base = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .filter(|d| d.is_absolute())
            .unwrap_or_else(std::env::temp_dir);
        Self::in_dir(base.join("ve_update"))
    }

    /// The store in `dir` (a test's scratch dir).
    pub fn in_dir(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    fn file(&self, pid: u32) -> PathBuf {
        self.dir.join(format!("{pid}.placement"))
    }

    /// Keeps `p` for the process `pid`.
    pub fn save(&self, pid: u32, p: &Placement) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        std::fs::write(self.file(pid), p.text())
    }

    /// The placement kept for `pid`, removed from the store; `None` when there is none or it does not read.
    pub fn take(&self, pid: u32) -> Option<Placement> {
        let file = self.file(pid);
        let text = read_bounded(&file);
        // a file that does not read must not be met again by the next process with this pid
        let _gone = std::fs::remove_file(&file);
        Placement::parse(&text?)
    }

    /// The placement kept for this process: what a restart left for it.
    pub fn take_own(&self) -> Option<Placement> {
        self.take(std::process::id())
    }
}

fn read_bounded(file: &Path) -> Option<String> {
    let mut text = String::new();
    std::fs::File::open(file)
        .ok()?
        .take(FILE_BYTES_MAX)
        .read_to_string(&mut text)
        .ok()?;
    Some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placement_text_reads_back() {
        let p = Placement {
            pos: Some(egui::pos2(-10.5, 20.0)),
            size: Some(egui::vec2(520.0, 420.0)),
            maximized: true,
            fullscreen: false,
        };
        assert_eq!(Placement::parse(&p.text()), Some(p));
        assert_eq!(
            Placement::parse(&Placement::default().text()),
            Some(Placement::default())
        );
    }

    #[test]
    fn placement_refuses_what_it_did_not_write() {
        for bad in [
            "pos 1\n",
            "pos 1 2 3\n",
            "size 0 10\n",
            "size NaN 10\n",
            "size 1e9 10\n",
            "where 1 2\n",
        ] {
            assert_eq!(Placement::parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn store_hands_a_placement_out_once() {
        let dir = std::env::temp_dir().join(format!("ve_update-test-{}", std::process::id()));
        let store = PlacementStore::in_dir(&dir);
        let p = Placement {
            size: Some(egui::vec2(300.0, 200.0)),
            ..Placement::default()
        };
        store.save(7, &p).expect("saved");
        assert_eq!(store.take(7), Some(p));
        assert_eq!(store.take(7), None, "taken already");
        std::fs::write(dir.join("8.placement"), "garbage").expect("written");
        assert_eq!(store.take(8), None);
        assert!(!dir.join("8.placement").exists(), "a bad file goes too");
        std::fs::remove_dir_all(&dir).expect("cleaned up");
    }
}
