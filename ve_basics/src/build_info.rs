//! Build info: what is running, spelled out on hover.

use egui::{Color32, Response, RichText, Ui};
use web_time::{SystemTime, UNIX_EPOCH};

/// Identity of a build. Make it in the *app's* crate with [`build_info!`], so the debug / release marker and
/// the env vars are the app's, not this library's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildInfo {
    /// App name (`CARGO_PKG_NAME`).
    pub name: &'static str,
    /// App version (`CARGO_PKG_VERSION`).
    pub version: &'static str,
    /// Short git SHA, `-dirty` with local changes, `unknown` without git (`GIT_SHA` from the app's `build.rs`).
    pub git_sha: &'static str,
    /// Build time as text (`BUILD_TIME` from the app's `build.rs`), `unknown` when not set.
    pub build_time: &'static str,
    /// `cfg!(debug_assertions)` of the app's crate.
    pub debug: bool,
    /// Commit time in Unix seconds (`GIT_COMMIT_UNIX` from the app's `build.rs`); `None` shows no age.
    ///
    /// The age in [`BuildInfo::short`] counts from the commit, not from the compile: `build.rs` reads the
    /// commit's time on purpose, so its output stays reproducible, and cargo does not rerun it on every build.
    pub commit_unix: Option<i64>,
}

/// Capture the calling crate's build info: name and version from Cargo, `GIT_SHA` and `BUILD_TIME` from its
/// `build.rs` (optional, `unknown` when missing), `GIT_COMMIT_UNIX` (optional, parsed at run time; missing or
/// unparsable gives `None`) and `cfg!(debug_assertions)`.
///
/// `build.rs` of the app (a copy of the mesh dashboard app's, no extra crates):
///
/// ```ignore
/// fn main() {
///     println!("cargo:rerun-if-changed=.git/HEAD");
///     println!("cargo:rerun-if-changed=.git/index");
///     println!("cargo:rustc-env=GIT_SHA={}", git_sha());     // `git rev-parse --short HEAD`, `-dirty` if
///     println!("cargo:rustc-env=BUILD_TIME={}", build_time()); // `git status --porcelain` is not empty
///     println!("cargo:rustc-env=GIT_COMMIT_UNIX={}", commit_unix()); // `git log -1 --format=%ct`
/// }
/// ```
///
/// Then in the UI: `build_info_label(ui, &ve_basics::build_info!());`
#[macro_export]
macro_rules! build_info {
    () => {
        $crate::BuildInfo {
            name: env!("CARGO_PKG_NAME"),
            version: env!("CARGO_PKG_VERSION"),
            git_sha: match option_env!("GIT_SHA") {
                Some(s) => s,
                None => "unknown",
            },
            build_time: match option_env!("BUILD_TIME") {
                Some(s) => s,
                None => "unknown",
            },
            debug: cfg!(debug_assertions),
            commit_unix: match option_env!("GIT_COMMIT_UNIX") {
                Some(s) => s.parse::<i64>().ok(),
                None => None,
            },
        }
    };
}

impl BuildInfo {
    /// `"0.4.2 · a1b2c3d · debug · 28 h ago"`: the age from [`Self::commit_unix`] against the system clock,
    /// left out when the commit time is unknown.
    pub fn short(&self) -> String {
        let base = format!("{} · {} · {}", self.version, self.git_sha, self.profile());
        #[cfg_attr(
            not(target_arch = "wasm32"),
            expect(
                clippy::disallowed_methods,
                reason = "web_time::SystemTime is the clock that works on the web"
            )
        )]
        let now = SystemTime::now();
        let now_unix = now
            .duration_since(UNIX_EPOCH)
            .ok()
            .and_then(|d| i64::try_from(d.as_secs()).ok());
        match (self.commit_unix, now_unix) {
            (Some(commit), Some(now)) => format!("{base} · {}", ago(commit, now)),
            _ => base,
        }
    }

    /// `"debug"` or `"release"`.
    pub fn profile(&self) -> &'static str {
        if self.debug { "debug" } else { "release" }
    }

    /// Multi-line explanation for the tooltip.
    pub fn details(&self) -> String {
        let profile = if self.debug {
            "debug (unoptimized, with debug assertions: slower than a release build)"
        } else {
            "release (optimized)"
        };
        format!(
            "{}\nversion: {}\ngit commit: {} (-dirty means uncommitted changes were built in)\nbuilt: {}\nprofile: {}",
            self.name, self.version, self.git_sha, self.build_time, profile
        )
    }
}

/// How long ago `commit_unix` was at `now_unix`, both in Unix seconds.
///
/// Under an hour "N min ago", under 48 hours "N h ago", else "N d ago". A time in the future reads "0 min ago".
pub fn ago(commit_unix: i64, now_unix: i64) -> String {
    const MIN_S: i64 = 60;
    const HOUR_S: i64 = 60 * MIN_S;
    const DAY_S: i64 = 24 * HOUR_S;
    let secs = now_unix.saturating_sub(commit_unix).max(0);
    if secs < HOUR_S {
        format!("{} min ago", secs / MIN_S)
    } else if secs < 48 * HOUR_S {
        format!("{} h ago", secs / HOUR_S)
    } else {
        format!("{} d ago", secs / DAY_S)
    }
}

/// Small monospace line `version · sha · debug|release` with the details as tooltip. The debug marker is
/// orange ([`debug_marker_color`]) so a debug build is never mistaken for a release one.
pub fn build_info_label(ui: &mut Ui, info: &BuildInfo) -> Response {
    let text = RichText::new(info.short()).monospace().small();
    let text = if info.debug {
        text.color(debug_marker_color(ui))
    } else {
        text.weak()
    };
    ui.label(text).on_hover_text(info.details())
}

/// The debug-build marker's color: `ve_theme::Tokens::colors.debug.text` (CHR-4), inlined here since
/// ve_basics sits below ve_theme in the crate layering and cannot depend on it.
pub(crate) fn debug_marker_color(ui: &Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgb(0xff, 0xa2, 0x4d)
    } else {
        Color32::from_rgb(0x9c, 0x43, 0x00)
    }
}

#[cfg(test)]
mod tests {
    use super::ago;

    const NOW: i64 = 1_800_000_000;

    #[test]
    fn age_buckets_and_their_edges() {
        let cases = [
            (NOW + 5, "0 min ago"),
            (NOW, "0 min ago"),
            (NOW - 59, "0 min ago"),
            (NOW - 60, "1 min ago"),
            (NOW - 3599, "59 min ago"),
            (NOW - 3600, "1 h ago"),
            (NOW - 28 * 3600, "28 h ago"),
            (NOW - (48 * 3600 - 1), "47 h ago"),
            (NOW - 48 * 3600, "2 d ago"),
            (NOW - 10 * 86400 - 1, "10 d ago"),
            (i64::MIN, "106751991167300 d ago"),
        ];
        for (commit, want) in cases {
            assert_eq!(ago(commit, NOW), want, "commit {commit}");
        }
    }
}
