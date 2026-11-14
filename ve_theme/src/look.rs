//! The live look: the tokens from a [`ColorSource`], following the desktop theme as it changes (THM-18).
//!
//! [`Look::setup`] replaces [`crate::setup`] in an app that follows the desktop: it installs the fonts and the
//! styles, and while the source is [`ColorSource::Desktop`] a thread reads omarchy's `colors.toml` once a
//! second and re-applies the styles when it changed, so a theme switch shows at once. The thread ends with the
//! last clone of the `Look`. Native only (a thread and the file system).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use egui::{Color32, Theme};

use crate::{ColorSource, DesktopColors, Tokens};

/// How often the desktop theme's file is read.
const POLL: Duration = Duration::from_secs(1);

/// The omarchy theme's colours: `~/.local/state/omarchy/current/theme/colors.toml` (`$XDG_STATE_HOME` when
/// set). `None` without a home folder.
pub fn omarchy_colors_path() -> Option<PathBuf> {
    let state = std::env::var_os("XDG_STATE_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))?;
    Some(state.join("omarchy/current/theme/colors.toml"))
}

#[derive(Debug)]
struct Inner {
    source: ColorSource,
    path: Option<PathBuf>,
    /// The desktop theme as last read; `None` when the file is missing or gives nothing.
    desktop: Option<DesktopColors>,
    /// Why there is no desktop theme, when there is none.
    missing: Option<String>,
    /// The file's text as last read, to tell a change.
    last: Option<String>,
    watching: bool,
}

/// The app's look: fonts, the dark and light styles from a [`ColorSource`], and the desktop theme followed
/// live. Transient: keep it beside the app (cloning shares it); save only the [`ColorSource`].
#[derive(Clone, Debug)]
pub struct Look {
    inner: Arc<Mutex<Inner>>,
}

impl Look {
    /// Install the fonts and the styles from `source`, following omarchy's theme file
    /// ([`omarchy_colors_path`]). Call once at startup instead of [`crate::setup`].
    pub fn setup(ctx: &egui::Context, source: ColorSource) -> Self {
        Self::setup_with_path(ctx, source, omarchy_colors_path())
    }

    /// [`Look::setup`] with the desktop theme's file named (tests, other desktops); `None`: no desktop theme.
    pub fn setup_with_path(
        ctx: &egui::Context,
        source: ColorSource,
        path: Option<PathBuf>,
    ) -> Self {
        #[cfg(feature = "fonts")]
        crate::install_fonts(ctx);
        ctx.options_mut(|o| o.fallback_theme = Theme::Dark);
        let look = Self {
            inner: Arc::new(Mutex::new(Inner {
                source,
                path,
                desktop: None,
                missing: None,
                last: None,
                watching: false,
            })),
        };
        look.reload(ctx);
        look.watch(ctx);
        look
    }

    /// The source in use.
    pub fn source(&self) -> ColorSource {
        self.lock().source
    }

    /// Switch the source and re-apply the styles.
    pub fn set_source(&self, ctx: &egui::Context, source: ColorSource) {
        self.lock().source = source;
        self.apply(ctx);
        self.watch(ctx);
    }

    /// The desktop theme as last read, if there is one.
    pub fn desktop(&self) -> Option<DesktopColors> {
        self.lock().desktop.clone()
    }

    /// Read the desktop theme's file again now and re-apply the styles if it changed.
    pub fn reload(&self, ctx: &egui::Context) {
        let path = self.lock().path.clone();
        let text = path.as_deref().map(read);
        if self.take(text) {
            self.apply(ctx);
        }
    }

    /// One line on where the colours come from now, for the settings: the theme followed and its file, or why
    /// the brand colours are in use.
    pub fn status(&self) -> String {
        let inner = self.lock();
        let file = inner
            .path
            .as_deref()
            .map_or("no file".into(), |p| p.display().to_string());
        match (inner.source, &inner.desktop) {
            (ColorSource::Brand, _) => "The vhrd brand colours.".into(),
            (ColorSource::Fixed(c), _) => {
                format!(
                    "The vhrd brand colours with the accent {}.",
                    crate::to_hex(c)
                )
            }
            (ColorSource::Desktop, Some(d)) => format!(
                "Following the desktop theme{} ({}): accent {}, from {file}; a theme switch shows at once.",
                d.name.as_deref().map_or(String::new(), |n| format!(" {n}")),
                if d.theme() == Theme::Dark {
                    "dark"
                } else {
                    "light"
                },
                d.accent.map_or("unchanged".into(), crate::to_hex),
            ),
            (ColorSource::Desktop, None) => format!(
                "No desktop theme: {}; the vhrd brand colours until it appears.",
                inner.missing.as_deref().unwrap_or("not read yet")
            ),
        }
    }

    /// The setting as a control: desktop theme, brand, or a fixed colour (with its picker), and the status
    /// line below. Returns the new source when the person changed it, for the app to save.
    pub fn ui(&self, ui: &mut egui::Ui) -> Option<ColorSource> {
        let current = self.source();
        let mut source = current;
        let fixed = match current {
            ColorSource::Fixed(c) => c,
            _ => Tokens::of_ui(ui).colors.accent,
        };
        ui.horizontal(|ui| {
            ui.selectable_value(&mut source, ColorSource::Desktop, "Desktop theme")
                .on_hover_text(
                    "Take the accent, background and text from the desktop's theme (omarchy) and follow it \
                     when it switches; the vhrd brand colours where there is none",
                );
            ui.selectable_value(&mut source, ColorSource::Brand, "Brand")
                .on_hover_text("The vhrd brand colours, whatever the desktop's theme");
            ui.selectable_value(&mut source, ColorSource::Fixed(fixed), "Fixed accent")
                .on_hover_text("The vhrd brand colours with an accent of your own");
            if let ColorSource::Fixed(mut c) = source
                && ui
                    .color_edit_button_srgba(&mut c)
                    .on_hover_text("The accent; text on it and around it stays legible")
                    .changed()
            {
                source = ColorSource::Fixed(Color32::from_rgb(c.r(), c.g(), c.b()));
            }
        });
        ui.weak(self.status());
        (source != current).then(|| {
            self.set_source(ui.ctx(), source);
            source
        })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Take the file's text (`Err`: why there is none); whether anything changed.
    fn take(&self, text: Option<Result<String, String>>) -> bool {
        let mut inner = self.lock();
        let (desktop, missing, last) = match text {
            None => (None, Some("no home folder to look in".into()), None),
            Some(Err(why)) => (None, Some(why), None),
            Some(Ok(text)) => {
                let mut d = DesktopColors::parse(&text);
                if d.is_empty() {
                    let why = "the theme file gives no accent, background or foreground".into();
                    (None, Some(why), Some(text))
                } else {
                    d.name = inner.path.as_deref().and_then(theme_name);
                    (Some(d), None, Some(text))
                }
            }
        };
        let changed = inner.last != last || inner.missing != missing;
        inner.desktop = desktop;
        inner.missing = missing;
        inner.last = last;
        changed
    }

    fn apply(&self, ctx: &egui::Context) {
        let (dark, light) = {
            let inner = self.lock();
            inner.source.tokens(inner.desktop.as_ref())
        };
        dark.apply(ctx, Theme::Dark);
        light.apply(ctx, Theme::Light);
        ctx.request_repaint();
    }

    /// Start the thread that follows the file, once, while the source is the desktop's.
    fn watch(&self, ctx: &egui::Context) {
        {
            let mut inner = self.lock();
            if inner.watching || inner.source != ColorSource::Desktop || inner.path.is_none() {
                return;
            }
            inner.watching = true;
        }
        let weak: Weak<Mutex<Inner>> = Arc::downgrade(&self.inner);
        let ctx = ctx.clone();
        // Native only: this module is not built for the web.
        #[allow(clippy::disallowed_methods)]
        let spawned = std::thread::Builder::new()
            .name("ve_theme desktop".into())
            .spawn(move || {
                loop {
                    #[allow(clippy::disallowed_methods)]
                    std::thread::sleep(POLL);
                    let Some(inner) = weak.upgrade() else {
                        return;
                    };
                    let look = Look { inner };
                    {
                        let mut i = look.lock();
                        if i.source != ColorSource::Desktop {
                            i.watching = false;
                            return;
                        }
                    }
                    look.reload(&ctx);
                }
            });
        if let Err(e) = spawned {
            log::warn!("ve_theme: can't follow the desktop theme: {e}");
            self.lock().watching = false;
        }
    }
}

/// The file's text, or why there is none.
// Native only: this module is not built for the web.
#[allow(clippy::disallowed_methods)]
fn read(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => format!("{} does not exist", path.display()),
        _ => format!("can't read {}: {e}", path.display()),
    })
}

/// omarchy's `theme.name` beside the theme folder (`.../current/theme.name` for `.../current/theme/colors.toml`).
#[allow(clippy::disallowed_methods)]
fn theme_name(colors: &Path) -> Option<String> {
    let current = colors.parent()?.parent()?;
    let name = std::fs::read_to_string(current.join("theme.name")).ok()?;
    let name = name.trim();
    (!name.is_empty()).then(|| name.to_string())
}

#[cfg(test)]
// Native-only tests on a temp folder; never reach a wasm build.
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::*;

    const TOKYO: &str = "mode = \"dark\"\naccent = \"#7aa2f7\"\nbackground = \"#1a1b26\"\nforeground = \"#a9b1d6\"\n";
    const OTHER: &str = "mode = \"dark\"\naccent = \"#9ece6a\"\nbackground = \"#101010\"\nforeground = \"#d0d0d0\"\n";

    fn folder() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ve_theme_look_{}_{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(dir.join("current/theme")).unwrap();
        dir
    }

    #[test]
    fn follows_the_file_and_falls_back_to_brand() {
        let dir = folder();
        let file = dir.join("current/theme/colors.toml");
        std::fs::write(&file, TOKYO).unwrap();
        std::fs::write(dir.join("current/theme.name"), "tokyo-night\n").unwrap();
        let ctx = egui::Context::default();
        let look = Look::setup_with_path(&ctx, ColorSource::Desktop, Some(file.clone()));
        let accent = |ctx: &egui::Context| Tokens::of(ctx, Theme::Dark).colors.accent;
        assert_eq!(accent(&ctx), crate::parse_hex("#7aa2f7").unwrap());
        assert!(look.status().contains("tokyo-night"), "{}", look.status());

        // A theme switch: the next read applies it.
        std::fs::write(&file, OTHER).unwrap();
        look.reload(&ctx);
        assert_eq!(accent(&ctx), crate::parse_hex("#9ece6a").unwrap());

        look.set_source(&ctx, ColorSource::Brand);
        assert_eq!(accent(&ctx), Tokens::dark().colors.accent);
        look.set_source(&ctx, ColorSource::Desktop);

        // No file: brand, and the status says why.
        std::fs::remove_file(&file).unwrap();
        look.reload(&ctx);
        assert_eq!(accent(&ctx), Tokens::dark().colors.accent);
        assert!(
            look.status().contains("does not exist"),
            "{}",
            look.status()
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_watcher_applies_a_switch_by_itself() {
        let dir = folder().join("w");
        std::fs::create_dir_all(dir.join("current/theme")).unwrap();
        let file = dir.join("current/theme/colors.toml");
        std::fs::write(&file, TOKYO).unwrap();
        let ctx = egui::Context::default();
        let _look = Look::setup_with_path(&ctx, ColorSource::Desktop, Some(file.clone()));
        std::fs::write(&file, OTHER).unwrap();
        let want = crate::parse_hex("#9ece6a").unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while Tokens::of(&ctx, Theme::Dark).colors.accent != want {
            assert!(std::time::Instant::now() < deadline, "not applied in 5 s");
            std::thread::sleep(Duration::from_millis(50));
        }
        std::fs::remove_dir_all(&dir).ok();
    }
}
