//! `spacemouse.toml`: one mapping for every app, found the way Cargo finds its `config.toml` (SPM-5).
//!
//! The files, nearest first:
//!
//! 1. `.config/spacemouse.toml` in the current folder, then in each parent folder up to `/`;
//! 2. `spacemouse.toml` in the user's config folder: `$XDG_CONFIG_HOME`, else `~/.config` (skipped when the walk
//!    already met it, as Cargo skips `$CARGO_HOME` it met among the parents).
//!
//! They are merged key by key, the nearest file winning, as Cargo merges its config: a project's
//! `.config/spacemouse.toml` with `speed.pan = 3000.0` alone makes that project's pans faster and keeps the
//! calibration from `~/.config/spacemouse.toml`. A table merges per key (`speed.pan`, `buttons.left`), the `axes`
//! list counts as one key (the calibrate step measures all six together). A key no file sets takes
//! [`Mapping::default`]'s value.
//!
//! [`Config::source`] says which file a key came from; [`Config::save`] writes a mapping into one file with as few
//! keys as it needs and reports which keys a nearer file still overrides. [`Config::adopt`] moves an app's old
//! per-app mapping into the user's file once.
//!
//! The file holds a [`Mapping`] at its top level:
//!
//! ```toml
//! convention = "Content"
//! full_scale = 320.0
//! dead_zone = 0.08
//!
//! [speed]
//! pan = 1200.0
//!
//! [[axes]]   # one per output: move right, move up, grow, tilt forward, roll right, turn clockwise
//! raw = 0
//! invert = false
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use toml::{Table, Value};

use crate::mapping::Mapping;

/// The file's name, in a `.config` folder of a project or in the user's config folder.
pub const FILE_NAME: &str = "spacemouse.toml";

/// What a [`Config::save`] or [`Config::adopt`] writes above the keys.
const HEADER: &str = "# The SpaceMouse's mapping, shared by every app on ve_spacemouse (SPM-5). A .config/spacemouse.toml\n\
     # in a project folder or one of its parents overrides single keys of it for that project.\n\n";

/// One file that was found.
#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    /// The file.
    pub path: PathBuf,
    /// The user's file (`~/.config/spacemouse.toml`), not a project's.
    pub user: bool,
    /// Its keys, as read.
    pub table: Table,
}

impl Layer {
    /// The keys this file sets, as dotted paths (`speed.pan`), known keys only.
    pub fn keys(&self) -> Vec<String> {
        let known = known_keys();
        flatten(&self.table)
            .into_keys()
            .filter(|k| known.contains(k))
            .collect()
    }
}

/// Where a file can be: the user's file or a project's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    /// The path looked at.
    pub path: PathBuf,
    /// The user's file.
    pub user: bool,
}

/// The mapping from every `spacemouse.toml` found, merged, with where each key came from.
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    cwd: PathBuf,
    user_file: Option<PathBuf>,
    candidates: Vec<Candidate>,
    /// The files found, nearest first.
    layers: Vec<Layer>,
    /// Dotted key → index into `layers`.
    sources: BTreeMap<String, usize>,
    mapping: Mapping,
    problems: Vec<String>,
}

/// What [`Config::save`] did.
#[derive(Clone, Debug, PartialEq)]
pub struct Saved {
    /// The file written.
    pub path: PathBuf,
    /// The keys written into it.
    pub keys: Vec<String>,
    /// Keys a nearer file sets to another value: the saved value does not apply here until that file changes.
    pub shadowed: Vec<(String, PathBuf)>,
}

impl Saved {
    /// One line for the panel: where it went, and what still overrides it.
    pub fn line(&self) -> String {
        let mut s = format!("saved to {}", display_path(&self.path));
        if !self.shadowed.is_empty() {
            let keys: Vec<String> = self
                .shadowed
                .iter()
                .map(|(k, p)| format!("{k} ({})", display_path(p)))
                .collect();
            s += &format!("; a nearer file still sets {}", keys.join(", "));
        }
        s
    }
}

/// What [`Config::adopt`] did with an old per-app section.
#[derive(Clone, Debug, PartialEq)]
pub enum Adopted {
    /// Every key of the old section was already in the user's file: nothing written.
    AlreadyThere,
    /// These keys were written into the user's file; `kept` were already there and keep the file's values.
    Moved {
        /// The user's file.
        path: PathBuf,
        /// The keys taken from the old section.
        moved: Vec<String>,
        /// Keys the user's file had already: its values stay.
        kept: Vec<String>,
    },
}

/// The user's config folder: `$XDG_CONFIG_HOME`, else `$HOME/.config`.
pub fn user_config_dir() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
}

/// The user's file: `spacemouse.toml` in [`user_config_dir`].
pub fn user_file() -> Option<PathBuf> {
    user_config_dir().map(|d| d.join(FILE_NAME))
}

/// Where the files can be, nearest first: `.config/spacemouse.toml` in `cwd` and every parent, then `user_file`
/// unless the walk met it already.
pub fn candidates(cwd: &Path, user_file: Option<&Path>) -> Vec<Candidate> {
    let mut out: Vec<Candidate> = cwd
        .ancestors()
        .map(|d| {
            let path = d.join(".config").join(FILE_NAME);
            let user = user_file == Some(path.as_path());
            Candidate { path, user }
        })
        .collect();
    if let Some(u) = user_file
        && !out.iter().any(|c| c.user)
    {
        out.push(Candidate {
            path: u.to_path_buf(),
            user: true,
        });
    }
    out
}

/// A path for people: the home folder as `~`.
pub fn display_path(path: &Path) -> String {
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from)
        && let Ok(rest) = path.strip_prefix(&home)
    {
        return format!("~/{}", rest.display());
    }
    path.display().to_string()
}

/// Every key a file can set, as dotted paths.
pub fn known_keys() -> Vec<String> {
    flatten(&mapping_table(&Mapping::default()))
        .into_keys()
        .collect()
}

/// A mapping's values by dotted key, as a file holds them.
pub(crate) fn key_values(mapping: &Mapping) -> BTreeMap<String, Value> {
    flatten(&mapping_table(mapping))
}

/// The mapping as a table, floats written short (`0.08`, not `0.07999999821186066`).
fn mapping_table(mapping: &Mapping) -> Table {
    let mut t = Table::try_from(mapping).unwrap_or_default();
    tidy(&mut t);
    t
}

fn tidy(t: &mut Table) {
    for (_, v) in t.iter_mut() {
        tidy_value(v);
    }
}

fn tidy_value(v: &mut Value) {
    match v {
        Value::Float(f) => {
            if let Ok(short) = (*f as f32).to_string().parse::<f64>() {
                *f = short;
            }
        }
        Value::Table(t) => tidy(t),
        Value::Array(a) => a.iter_mut().for_each(tidy_value),
        _ => {}
    }
}

/// The leaves of a table by dotted path; a list is one leaf.
fn flatten(t: &Table) -> BTreeMap<String, Value> {
    fn walk(prefix: &str, t: &Table, out: &mut BTreeMap<String, Value>) {
        for (k, v) in t {
            let key = if prefix.is_empty() {
                k.clone()
            } else {
                format!("{prefix}.{k}")
            };
            match v {
                Value::Table(inner) => walk(&key, inner, out),
                _ => {
                    out.insert(key, v.clone());
                }
            }
        }
    }
    let mut out = BTreeMap::new();
    walk("", t, &mut out);
    out
}

/// Set a dotted key in a table, making the tables on the way.
fn set_key(t: &mut Table, key: &str, value: Value) {
    match key.split_once('.') {
        None => {
            t.insert(key.into(), value);
        }
        Some((head, rest)) => {
            let inner = t
                .entry(head.to_string())
                .or_insert_with(|| Value::Table(Table::new()));
            if !inner.is_table() {
                *inner = Value::Table(Table::new());
            }
            if let Value::Table(inner) = inner {
                set_key(inner, rest, value);
            }
        }
    }
}

/// `over` on top of `base`, per key; tables merge, everything else (lists too) is replaced.
fn merge(base: &mut Table, over: &Table) {
    for (k, v) in over {
        match (base.get_mut(k), v) {
            (Some(Value::Table(b)), Value::Table(o)) => merge(b, o),
            _ => {
                base.insert(k.clone(), v.clone());
            }
        }
    }
}

fn to_mapping(t: &Table) -> Result<Mapping, String> {
    Mapping::deserialize(t.clone()).map_err(|e| e.to_string())
}

fn read_table(path: &Path) -> Result<Option<Table>, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => text
            .parse::<Table>()
            .map(Some)
            .map_err(|e| format!("{}: {e}", display_path(path))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("can't read {}: {e}", display_path(path))),
    }
}

/// Write a table (a temporary file renamed over it), making the folder.
fn write_table(path: &Path, t: &Table) -> Result<(), String> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("can't make {}: {e}", display_path(d)))?;
    }
    let text = toml::to_string(t).map_err(|e| format!("can't write the settings: {e}"))?;
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, format!("{HEADER}{text}"))
        .and_then(|_| std::fs::rename(&tmp, path))
        .map_err(|e| format!("can't write {}: {e}", display_path(path)))
}

impl Config {
    /// The files for the current folder and the user's config folder.
    pub fn load() -> Self {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self::load_from(&cwd, user_file())
    }

    /// The files for `cwd` and `user_file` (tests and apps with a folder of their own).
    pub fn load_from(cwd: &Path, user_file: Option<PathBuf>) -> Self {
        let mut c = Self {
            cwd: cwd.to_path_buf(),
            candidates: candidates(cwd, user_file.as_deref()),
            user_file,
            layers: Vec::new(),
            sources: BTreeMap::new(),
            mapping: Mapping::default(),
            problems: Vec::new(),
        };
        c.reload();
        c
    }

    /// No files at all: the defaults, nothing saved (tests, apps without a SpaceMouse).
    pub fn none() -> Self {
        Self {
            cwd: PathBuf::new(),
            user_file: None,
            candidates: Vec::new(),
            layers: Vec::new(),
            sources: BTreeMap::new(),
            mapping: Mapping::default(),
            problems: Vec::new(),
        }
    }

    /// Read the files again.
    pub fn reload(&mut self) {
        self.layers.clear();
        self.problems.clear();
        let known = known_keys();
        for c in &self.candidates {
            let table = match read_table(&c.path) {
                Ok(Some(t)) => t,
                Ok(None) => continue,
                Err(e) => {
                    self.problems.push(format!("{e}; skipped"));
                    continue;
                }
            };
            if let Err(e) = to_mapping(&table) {
                self.problems
                    .push(format!("{}: {e}; skipped", display_path(&c.path)));
                continue;
            }
            for k in flatten(&table).into_keys() {
                if !known.contains(&k) {
                    self.problems.push(format!(
                        "{}: `{k}` is no setting of the SpaceMouse; ignored",
                        display_path(&c.path)
                    ));
                }
            }
            self.layers.push(Layer {
                path: c.path.clone(),
                user: c.user,
                table,
            });
        }
        let (merged, sources) = self.merged(0);
        self.sources = sources;
        self.mapping = match to_mapping(&merged) {
            Ok(m) => m,
            Err(e) => {
                self.problems
                    .push(format!("the files together don't make a mapping: {e}"));
                Mapping::default()
            }
        };
    }

    /// The layers from `from` on (nearest first) merged over the defaults, and where each key came from.
    fn merged(&self, from: usize) -> (Table, BTreeMap<String, usize>) {
        let mut merged = mapping_table(&Mapping::default());
        let mut sources = BTreeMap::new();
        for (i, layer) in self.layers.iter().enumerate().skip(from).rev() {
            merge(&mut merged, &layer.table);
            for k in layer.keys() {
                sources.insert(k, i);
            }
        }
        (merged, sources)
    }

    /// The mapping all files give together.
    pub fn mapping(&self) -> &Mapping {
        &self.mapping
    }

    /// The files found, nearest first.
    pub fn layers(&self) -> &[Layer] {
        &self.layers
    }

    /// Every place looked at, nearest first, found or not.
    pub fn candidates(&self) -> &[Candidate] {
        &self.candidates
    }

    /// The file a key (`speed.pan`) came from; `None`: the default.
    pub fn source(&self, key: &str) -> Option<&Layer> {
        self.sources.get(key).map(|&i| &self.layers[i])
    }

    /// Files that could not be read, keys that mean nothing: one line each.
    pub fn problems(&self) -> &[String] {
        &self.problems
    }

    /// The user's file, found or not; `None` without `XDG_CONFIG_HOME` and `HOME`.
    pub fn user_file(&self) -> Option<&Path> {
        self.user_file.as_deref()
    }

    /// The nearest project file that exists.
    pub fn nearest_project_file(&self) -> Option<&Path> {
        self.layers
            .iter()
            .find(|l| !l.user)
            .map(|l| l.path.as_path())
    }

    /// Where a project file goes when there is none yet: `.config/spacemouse.toml` in the current folder.
    pub fn new_project_file(&self) -> Option<PathBuf> {
        (!self.cwd.as_os_str().is_empty()).then(|| self.cwd.join(".config").join(FILE_NAME))
    }

    /// The project file to save into: the nearest one, else a new one in the current folder.
    pub fn project_target(&self) -> Option<PathBuf> {
        self.nearest_project_file()
            .map(Path::to_path_buf)
            .or_else(|| self.new_project_file())
    }

    /// Write `mapping` into `path` (one of [`Config::candidates`], or any file), then read the files again.
    ///
    /// The file gets the keys it already had, with the new values, and every key whose value differs from what
    /// the files farther away (and the defaults) give, so a project file holds only what the project changes. Its
    /// other contents stay. A file that does not read is left as it is.
    pub fn save(&mut self, mapping: &Mapping, path: &Path) -> Result<Saved, String> {
        let index = self.layers.iter().position(|l| l.path == path);
        let (mut table, below) = match index {
            Some(i) => (self.layers[i].table.clone(), self.merged(i + 1).0),
            None => {
                let table = read_table(path)?.unwrap_or_default();
                let at = self
                    .candidates
                    .iter()
                    .position(|c| c.path == path)
                    .unwrap_or(self.candidates.len());
                let from = self
                    .layers
                    .iter()
                    .position(|l| {
                        self.candidates
                            .iter()
                            .position(|c| c.path == l.path)
                            .is_some_and(|ci| ci > at)
                    })
                    .unwrap_or(self.layers.len());
                (table, self.merged(from).0)
            }
        };
        let had = flatten(&table);
        let below = flatten(&below);
        let mut keys = Vec::new();
        for (k, v) in flatten(&mapping_table(mapping)) {
            if had.contains_key(&k) || below.get(&k) != Some(&v) {
                set_key(&mut table, &k, v);
                keys.push(k);
            }
        }
        write_table(path, &table)?;
        self.reload();
        let wanted = flatten(&mapping_table(mapping));
        let mine = self.layers.iter().position(|l| l.path == path);
        let shadowed = self
            .sources
            .iter()
            .filter(|(k, i)| Some(**i) < mine && wanted.contains_key(*k))
            .filter(|(k, i)| flatten(&self.layers[**i].table).get(*k) != wanted.get(*k))
            .map(|(k, i)| (k.clone(), self.layers[*i].path.clone()))
            .collect();
        Ok(Saved {
            path: path.to_path_buf(),
            keys,
            shadowed,
        })
    }

    /// Move an app's old per-app mapping (its `[spacemouse]` section, say) into the user's file, once: keys the
    /// user's file lacks are taken from `old`, keys it has keep their values. `from` names the old place in the
    /// file's header line. The caller removes the old section afterwards, only when this returned `Ok`.
    pub fn adopt(&mut self, old: &Table, from: &str) -> Result<Adopted, String> {
        let path = self
            .user_file
            .clone()
            .ok_or("no user config folder: neither XDG_CONFIG_HOME nor HOME is set")?;
        to_mapping(old).map_err(|e| format!("the old section ({from}) is no mapping: {e}"))?;
        let mut table = read_table(&path)?.unwrap_or_default();
        let had = flatten(&table);
        let known = known_keys();
        let (mut moved, mut kept) = (Vec::new(), Vec::new());
        for (k, v) in flatten(old) {
            if !known.contains(&k) {
                continue;
            }
            if had.contains_key(&k) {
                kept.push(k);
            } else {
                set_key(&mut table, &k, v);
                moved.push(k);
            }
        }
        if moved.is_empty() {
            return Ok(Adopted::AlreadyThere);
        }
        write_table(&path, &table)?;
        self.reload();
        log::info!(
            "moved the SpaceMouse's mapping from {from} into {}",
            display_path(&path)
        );
        Ok(Adopted::Moved { path, moved, kept })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapping::{AxisMap, Convention, Output};

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    /// A tree: /home/me/.config (the user's), /home/me/work/.config, /home/me/work/board/ (the current folder).
    struct Tree {
        _dir: tempfile::TempDir,
        root: PathBuf,
    }

    impl Tree {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path().to_path_buf();
            std::fs::create_dir_all(root.join("home/me/work/board")).unwrap();
            Self { _dir: dir, root }
        }
        fn user(&self) -> PathBuf {
            self.root.join("home/me/.config").join(FILE_NAME)
        }
        fn work(&self) -> PathBuf {
            self.root.join("home/me/work/.config").join(FILE_NAME)
        }
        fn board(&self) -> PathBuf {
            self.root.join("home/me/work/board/.config").join(FILE_NAME)
        }
        fn cwd(&self) -> PathBuf {
            self.root.join("home/me/work/board")
        }
        fn load(&self) -> Config {
            Config::load_from(&self.cwd(), Some(self.user()))
        }
    }

    #[test]
    fn candidates_walk_up_then_the_user_file_once() {
        let c = candidates(Path::new("/a/b"), Some(Path::new("/u/spacemouse.toml")));
        let paths: Vec<_> = c.iter().map(|c| c.path.display().to_string()).collect();
        assert_eq!(
            paths,
            [
                "/a/b/.config/spacemouse.toml",
                "/a/.config/spacemouse.toml",
                "/.config/spacemouse.toml",
                "/u/spacemouse.toml"
            ]
        );
        assert!(c[3].user && !c[0].user);
        // The user's file met on the walk (the current folder under home) is not read twice.
        let c = candidates(
            Path::new("/home/me/x"),
            Some(Path::new("/home/me/.config/spacemouse.toml")),
        );
        assert_eq!(c.len(), 4);
        assert!(c[1].user);
        assert_eq!(c.iter().filter(|c| c.user).count(), 1);
    }

    #[test]
    fn nearest_file_wins_per_key() {
        let t = Tree::new();
        write(
            &t.user(),
            "dead_zone = 0.1\nfull_scale = 300.0\n[speed]\npan = 1000.0\nzoom = 2.0\n[[axes]]\nraw = 1\n[[axes]]\nraw = 0\ninvert = true\n[[axes]]\nraw = 2\n[[axes]]\nraw = 3\n[[axes]]\nraw = 4\n[[axes]]\nraw = 5\n",
        );
        write(&t.work(), "[speed]\npan = 3000.0\n");
        write(&t.board(), "dead_zone = 0.2\n");
        let c = t.load();
        let m = c.mapping();
        assert_eq!(m.dead_zone, 0.2);
        assert_eq!(m.full_scale, 300.0);
        assert_eq!(m.speed.pan, 3000.0);
        assert_eq!(m.speed.zoom, 2.0);
        assert_eq!(m.speed.rotate, Mapping::default().speed.rotate);
        assert_eq!(m.axis(Output::MoveRight), AxisMap::new(1));
        assert_eq!(m.axis(Output::MoveUp), AxisMap::inverted(0));
        assert_eq!(c.source("dead_zone").unwrap().path, t.board());
        assert_eq!(c.source("speed.pan").unwrap().path, t.work());
        assert_eq!(c.source("speed.zoom").unwrap().path, t.user());
        assert!(c.source("axes").unwrap().user);
        assert!(c.source("speed.rotate").is_none());
        assert_eq!(c.layers().len(), 3);
        assert_eq!(c.nearest_project_file(), Some(t.board().as_path()));
        assert!(c.problems().is_empty(), "{:?}", c.problems());
    }

    #[test]
    fn no_files_give_the_defaults() {
        let t = Tree::new();
        let c = t.load();
        assert_eq!(c.mapping(), &Mapping::default());
        assert!(c.layers().is_empty());
        assert_eq!(c.project_target(), Some(t.board()));
        assert_eq!(Config::none().mapping(), &Mapping::default());
        assert_eq!(Config::none().project_target(), None);
    }

    #[test]
    fn a_broken_file_is_skipped_and_said() {
        let t = Tree::new();
        write(&t.user(), "dead_zone = 0.3\n");
        write(&t.work(), "dead_zone = \"much\"\n");
        write(&t.board(), "this is no toml\n");
        let c = t.load();
        assert_eq!(c.mapping().dead_zone, 0.3);
        assert_eq!(c.problems().len(), 2, "{:?}", c.problems());
        // A key that means nothing is said too, the rest of the file read.
        write(&t.board(), "dead_zon = 0.2\nfull_scale = 200.0\n");
        write(&t.work(), "");
        let c = t.load();
        assert_eq!(c.mapping().full_scale, 200.0);
        assert!(c.problems()[0].contains("`dead_zon` is no setting"));
    }

    #[test]
    fn save_writes_only_what_the_file_needs() {
        let t = Tree::new();
        write(&t.user(), "dead_zone = 0.1\n");
        let mut c = t.load();
        let mut m = c.mapping().clone();
        m.speed.pan = 2500.0;
        // Into a new project file: only the key that differs from the user's file and the defaults.
        let target = c.project_target().unwrap();
        let saved = c.save(&m, &target).unwrap();
        assert_eq!(saved.keys, ["speed.pan"]);
        assert!(saved.shadowed.is_empty());
        let text = std::fs::read_to_string(t.board()).unwrap();
        assert!(text.starts_with("# The SpaceMouse's mapping"), "{text}");
        assert!(!text.contains("dead_zone"), "{text}");
        assert_eq!(c.mapping(), &m);
        assert_eq!(c.source("speed.pan").unwrap().path, t.board());
        // Into the user's file: every key that differs from the defaults, and the keys it had; floats short.
        m.convention = Convention::Camera;
        m.speed.pan = 4000.0;
        let saved = c.save(&m, &t.user()).unwrap();
        assert!(saved.keys.contains(&"dead_zone".to_string()));
        assert!(saved.keys.contains(&"convention".to_string()));
        assert_eq!(
            saved.shadowed,
            [("speed.pan".to_string(), t.board())],
            "the project still sets its own pan"
        );
        assert!(saved.line().contains("a nearer file still sets speed.pan"));
        let text = std::fs::read_to_string(t.user()).unwrap();
        assert!(text.contains("dead_zone = 0.1\n"), "{text}");
        assert_eq!(c.mapping().speed.pan, 2500.0);
        assert_eq!(c.mapping().convention, Convention::Camera);
    }

    #[test]
    fn save_keeps_other_keys_of_the_file() {
        let t = Tree::new();
        write(&t.user(), "# mine\nfull_scale = 280.0\n");
        let mut c = t.load();
        let mut m = c.mapping().clone();
        m.dead_zone = 0.2;
        c.save(&m, &t.user()).unwrap();
        let again = t.load();
        assert_eq!(again.mapping().full_scale, 280.0);
        assert_eq!(again.mapping().dead_zone, 0.2);
    }

    #[test]
    fn adopt_moves_an_old_section_once() {
        let t = Tree::new();
        let old: Table = "full_scale = 320.0\ndead_zone = 0.05\n[speed]\npan = 3000.0\n[buttons]\nleft = 12\nright = 13\n"
            .parse()
            .unwrap();
        let mut c = t.load();
        let Adopted::Moved { path, moved, kept } = c.adopt(&old, "the old file").unwrap() else {
            panic!("moved")
        };
        assert_eq!(path, t.user());
        assert_eq!(moved.len(), 5);
        assert!(kept.is_empty());
        assert_eq!(c.mapping().full_scale, 320.0);
        assert_eq!(c.mapping().buttons.left, 12);
        // Again: nothing to do.
        assert_eq!(
            c.adopt(&old, "the old file").unwrap(),
            Adopted::AlreadyThere
        );
        // A user's file that already has a key keeps its value.
        write(&t.user(), "full_scale = 400.0\n");
        let mut c = t.load();
        let Adopted::Moved { kept, .. } = c.adopt(&old, "the old file").unwrap() else {
            panic!("moved")
        };
        assert_eq!(kept, ["full_scale"]);
        assert_eq!(c.mapping().full_scale, 400.0);
        assert_eq!(c.mapping().speed.pan, 3000.0);
        // An old section that is no mapping is refused and nothing written.
        let bad: Table = "dead_zone = \"x\"\n".parse().unwrap();
        assert!(Config::none().adopt(&bad, "x").is_err());
        assert!(t.load().adopt(&bad, "x").is_err());
    }

    /// What a mapping saved by better_erc_app (`Settings::to_toml`, `[spacemouse]`) looks like goes through as is.
    #[test]
    fn a_whole_mapping_round_trips() {
        let t = Tree::new();
        let mut m = Mapping {
            full_scale: 333.0,
            dead_zone: 0.07,
            ..Default::default()
        };
        m.axes[Output::Grow.index()] = AxisMap::inverted(1);
        m.axes[Output::RollRight.index()] = AxisMap::default();
        let old = Table::try_from(&m).unwrap();
        let mut c = t.load();
        c.adopt(&old, "x").unwrap();
        assert_eq!(t.load().mapping(), &m);
    }
}
