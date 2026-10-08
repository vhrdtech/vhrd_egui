//! Build info: what is running, spelled out on hover.

use egui::{Color32, Response, RichText, Ui};

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
}

/// Capture the calling crate's build info: name and version from Cargo, `GIT_SHA` and `BUILD_TIME` from its
/// `build.rs` (optional, `unknown` when missing) and `cfg!(debug_assertions)`.
///
/// `build.rs` of the app (a copy of tpm_mesh_dash's, no extra crates):
///
/// ```ignore
/// fn main() {
///     println!("cargo:rerun-if-changed=.git/HEAD");
///     println!("cargo:rerun-if-changed=.git/index");
///     println!("cargo:rustc-env=GIT_SHA={}", git_sha());     // `git rev-parse --short HEAD`, `-dirty` if
///     println!("cargo:rustc-env=BUILD_TIME={}", build_time()); // `git status --porcelain` is not empty
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
        }
    };
}

impl BuildInfo {
    /// `"0.4.2 · a1b2c3d · debug"`.
    pub fn short(&self) -> String {
        format!("{} · {} · {}", self.version, self.git_sha, self.profile())
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

/// Small monospace line `version · sha · debug|release` with the details as tooltip. The debug marker is
/// orange so a debug build is never mistaken for a release one.
pub fn build_info_label(ui: &mut Ui, info: &BuildInfo) -> Response {
    let text = RichText::new(info.short()).monospace().small();
    let text = if info.debug {
        text.color(Color32::from_rgb(0xe0, 0x8a, 0x2e))
    } else {
        text.weak()
    };
    ui.label(text).on_hover_text(info.details())
}
