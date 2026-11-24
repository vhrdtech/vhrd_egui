//! What the bar shows, as the app's installer tells it.

use std::path::PathBuf;
use std::sync::Arc;

/// The installer behind an app: which release runs, which is installed, which are kept, and how to switch.
///
/// Every method is called from the UI thread each frame, so none may block: [`Releases::view`] hands out the last
/// snapshot, and [`Releases::switch`] only starts the work, whose progress and outcome show in later views.
pub trait Releases {
    /// The last snapshot of the releases.
    fn view(&self) -> Arc<View>;

    /// Starts switching the installed release to `id`, one of the view's [`Release::id`].
    fn switch(&self, id: &str);

    /// The program Restart starts: the installed one, never the running file (which is the old release).
    fn program(&self) -> Result<PathBuf, String>;
}

/// The releases at one moment.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct View {
    /// What this process runs.
    pub running: Running,
    /// The [`Release::id`] of the installed release, the one a start runs; `None`: nothing is installed.
    pub installed: Option<String>,
    /// Every kept release, newest first; the running and the installed one among them.
    pub releases: Vec<Release>,
    /// Work under way, such as "switching to 0.4.1".
    pub working: Option<String>,
    /// What went wrong last: a switch refused, the installer's state unreadable.
    pub problem: Option<String>,
}

impl View {
    /// The release with `id`.
    pub fn release(&self, id: &str) -> Option<&Release> {
        self.releases.iter().find(|r| r.id == id)
    }

    /// The installed release, when it is not the one running: what Restart would start.
    pub fn newer(&self) -> Option<&Release> {
        let installed = self.installed.as_deref()?;
        match &self.running {
            Running::Release(id) if id == installed => None,
            _ => self.release(installed),
        }
    }
}

/// What this process runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Running {
    /// One of the kept releases, by [`Release::id`].
    Release(String),
    /// A build the installer does not keep (a development build): what it is.
    Other(String),
}

impl Default for Running {
    fn default() -> Self {
        Self::Other("not known yet".to_owned())
    }
}

/// One kept release.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    /// The key the installer switches by.
    pub id: String,
    /// The version, such as `0.4.2`.
    pub version: String,
    /// The short git commit, where the release records one.
    pub commit: Option<String>,
    /// The build's own id (a linker build id), shown short where no commit is recorded.
    pub build: Option<String>,
    /// When it was installed, in Unix seconds.
    pub installed_unix: Option<i64>,
    /// The installer's word on it: "proved itself here", "failed its check: ...".
    pub note: Option<String>,
    /// The colour of that word.
    pub tone: Tone,
    /// `Ok`: "Switch to this one" may be clicked; `Err`: why not, and what would allow it.
    pub switch: Result<(), String>,
}

impl Release {
    /// `0.4.2 (a1b2c3d)`, or `0.4.2 (build 3f9a1c0e)` without a commit.
    pub fn label(&self) -> String {
        match (&self.commit, &self.build) {
            (Some(c), _) => format!("{} ({c})", self.version),
            (None, Some(b)) => format!("{} (build {})", self.version, short(b)),
            (None, None) => self.version.clone(),
        }
    }
}

/// How many characters of a build id the bar shows.
const BUILD_SHOWN: usize = 8;

fn short(id: &str) -> &str {
    id.char_indices()
        .nth(BUILD_SHOWN)
        .map_or(id, |(at, _)| &id[..at])
}

/// The colour of a release's note.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tone {
    /// Proved itself.
    Good,
    /// Failed; the installer will not take it back by itself.
    Bad,
    /// No verdict.
    #[default]
    Neutral,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(id: &str) -> Release {
        Release {
            id: id.to_owned(),
            version: "0.4.2".to_owned(),
            commit: None,
            build: Some("3f9a1c0e5b7d".to_owned()),
            installed_unix: None,
            note: None,
            tone: Tone::Neutral,
            switch: Ok(()),
        }
    }

    #[test]
    fn newer_is_the_installed_release_when_another_runs() {
        let mut v = View {
            running: Running::Release("a".to_owned()),
            installed: Some("a".to_owned()),
            releases: vec![release("b"), release("a")],
            ..View::default()
        };
        assert_eq!(v.newer(), None, "the installed one runs");
        v.installed = Some("b".to_owned());
        assert_eq!(v.newer().map(|r| r.id.as_str()), Some("b"));
        v.running = Running::Other("a development build".to_owned());
        assert_eq!(v.newer().map(|r| r.id.as_str()), Some("b"));
        v.installed = None;
        assert_eq!(
            v.newer(),
            None,
            "nothing installed: nothing to restart into"
        );
    }

    #[test]
    fn label_prefers_the_commit_and_cuts_the_build_id() {
        let mut r = release("a");
        assert_eq!(r.label(), "0.4.2 (build 3f9a1c0e)");
        r.commit = Some("a1b2c3d".to_owned());
        assert_eq!(r.label(), "0.4.2 (a1b2c3d)");
        r.commit = None;
        r.build = None;
        assert_eq!(r.label(), "0.4.2");
    }
}
