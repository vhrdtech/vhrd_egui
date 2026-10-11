//! Emulated transparency (BACK-2): where the compositor does not blur what is behind the window, a blurred
//! copy of the desktop wallpaper is drawn under the grid instead, dimmed by the theme's background colour.
//!
//! The wallpaper is found and decoded on a worker thread ([`crate::wallpaper`]); the UI thread only polls,
//! blurs the small copy (about a millisecond, when the blur setting changes) and draws one textured
//! rectangle, which works on every backend.

use std::path::PathBuf;

use egui::{Color32, Context, Rect, Shape, Vec2, pos2};

use crate::Settings;
use crate::position::{Placement, PositionProvider};

/// What the emulated transparency is doing, for a settings window to show (`text` says it in words).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GlassStatus {
    /// [`crate::Settings::glass`] is off.
    Off,
    /// Built without the `glass` feature, or a platform without files (wasm).
    Unsupported,
    /// Looking for the wallpaper and decoding it.
    Loading,
    /// The blurred wallpaper is drawn.
    Shown {
        /// The wallpaper file.
        path: PathBuf,
        /// Whether the window's position on the screen is known, so the copy lines up with the real one.
        aligned: bool,
        /// Width and height the wallpaper was decoded at: the working resolution chosen for the current
        /// blur and monitor (sharp and full size at no blur, smaller as the blur grows).
        working: [usize; 2],
    },
    /// No wallpaper was found on this desktop; name one with `Backdrop::set_wallpaper` or `VE_WALLPAPER`.
    NoWallpaper,
    /// The wallpaper could not be read.
    Failed {
        /// The file that was tried.
        path: PathBuf,
        /// What went wrong.
        error: String,
    },
}

impl GlassStatus {
    /// The status as a sentence.
    pub fn text(&self) -> String {
        match self {
            Self::Off => "off".to_owned(),
            Self::Unsupported => {
                "not available: built without wallpaper support, or the platform has no files"
                    .to_owned()
            }
            Self::Loading => "reading the wallpaper…".to_owned(),
            Self::Shown {
                path,
                aligned,
                working,
            } => format!(
                "showing {} at {}×{} ({})",
                path.display(),
                working[0],
                working[1],
                if *aligned {
                    "lined up with the desktop"
                } else {
                    "fitted to the window: this window system does not tell where the window is"
                }
            ),
            Self::NoWallpaper => {
                "no wallpaper found on this desktop: name one with VE_WALLPAPER".to_owned()
            }
            Self::Failed { path, error } => format!("could not read {}: {error}", path.display()),
        }
    }
}

/// The part of a wallpaper (UV, 0-1) that belongs under `rect`.
///
/// The wallpaper covers the monitor the way desktops show it (scaled to fill, centred, the overflow cut
/// off). With the monitor's size and the window's place on it known, `rect` (in window coordinates) gets the
/// piece that is really behind it; otherwise the wallpaper covers `rect` itself.
pub(crate) fn wallpaper_uv(aspect: f32, placed: Option<(Vec2, Rect)>, rect: Rect) -> Rect {
    let (screen, area) = match placed {
        Some((monitor, window)) if monitor.x > 0.0 && monitor.y > 0.0 => {
            // On a second monitor of the same size the picture repeats.
            let origin = pos2(
                window.min.x.rem_euclid(monitor.x),
                window.min.y.rem_euclid(monitor.y),
            );
            (monitor, rect.translate(origin.to_vec2()))
        }
        _ => (
            rect.size(),
            Rect::from_min_size(pos2(0.0, 0.0), rect.size()),
        ),
    };
    let image = Vec2::new(aspect.max(1e-3), 1.0);
    let scale = (screen.x / image.x).max(screen.y / image.y);
    let shown = image * scale;
    let offset = (screen - shown) * 0.5;
    let uv = |p: egui::Pos2| pos2((p.x - offset.x) / shown.x, (p.y - offset.y) / shown.y);
    Rect::from_min_max(uv(area.min), uv(area.max))
}

#[cfg(all(feature = "glass", not(target_arch = "wasm32")))]
mod imp {
    use std::sync::mpsc::{Receiver, TryRecvError, channel};

    use egui::{ColorImage, Mesh, TextureHandle, TextureOptions};

    use super::*;
    use crate::wallpaper::{self, Small};

    type Loaded = Result<(PathBuf, Small), GlassStatus>;

    /// Blur values closer than this share a texture.
    const BLUR_STEP: f32 = 0.02;
    /// Working widths closer than this (pixels) share a decode: redecoding is comparatively expensive (a
    /// worker thread, a full image decode), so a monitor estimate or a blur slider that only moved a little
    /// does not restart it.
    const WIDTH_STEP: u32 = 64;

    /// `w` rounded to the nearest [`WIDTH_STEP`], so small changes in the desired working width do not
    /// restart the decode.
    fn quantize_width(w: u32) -> u32 {
        ((w + WIDTH_STEP / 2) / WIDTH_STEP).max(1) * WIDTH_STEP
    }

    /// The wallpaper behind the backdrop: lookup, the worker thread, the texture.
    #[derive(Default)]
    pub(crate) struct Glass {
        wallpaper: Option<PathBuf>,
        provider: Option<Box<dyn PositionProvider>>,
        /// Whether the compositor's own provider was looked for already.
        provider_tried: bool,
        /// The monitor the wallpaper was looked up for.
        output: Option<String>,
        /// The working width ([`quantize_width`]-ed) of the job in flight or last loaded.
        requested_width: Option<u32>,
        job: Option<Receiver<Loaded>>,
        loaded: Option<(PathBuf, Small)>,
        failure: Option<GlassStatus>,
        texture: Option<(TextureHandle, f32, [usize; 2])>,
    }

    impl Glass {
        pub(crate) fn set_wallpaper(&mut self, path: Option<PathBuf>) {
            if self.wallpaper != path {
                self.wallpaper = path;
                self.reload();
            }
        }

        pub(crate) fn set_position_provider(
            &mut self,
            provider: Option<Box<dyn PositionProvider>>,
        ) {
            // `None` looks for the compositor's own again.
            self.provider_tried = provider.is_some();
            self.provider = provider;
        }

        /// Forget what was loaded: the next frame looks for the wallpaper again.
        pub(crate) fn reload(&mut self) {
            self.job = None;
            self.loaded = None;
            self.failure = None;
            self.texture = None;
            self.requested_width = None;
        }

        fn placement(&self) -> Option<Placement> {
            self.provider.as_ref().and_then(|p| p.placement())
        }

        /// The monitor's size and the window's content rectangle on it, in points, when known: from the
        /// provider (in its unit, brought to points by the content's width), else from the window system.
        fn placed(&self, ctx: &Context) -> Option<(Vec2, Rect)> {
            if let Some(p) = self.placement() {
                let scale = ctx.content_rect().width() / p.window.width();
                if scale.is_finite() && scale > 0.0 {
                    let window = Rect::from_min_size(
                        (p.window.min.to_vec2() * scale).to_pos2(),
                        p.window.size() * scale,
                    );
                    return Some((p.monitor * scale, window));
                }
            }
            ctx.input(|i| Some((i.viewport().monitor_size?, i.viewport().inner_rect?)))
        }

        /// The monitor's width in physical pixels, when known (the provider's or the window system's
        /// monitor size, in points, times `ctx.pixels_per_point()`), capped by `max_side`. Unknown (no
        /// provider, no window-system report) falls back to `max_side`: [`wallpaper::load`] then clamps to
        /// the image's own size, so the wallpaper decodes at its native resolution instead.
        fn full_width_hint(&self, ctx: &Context, max_side: u32) -> u32 {
            let physical = self
                .placed(ctx)
                .map(|(monitor, _)| monitor.x * ctx.pixels_per_point())
                .filter(|px| px.is_finite() && *px >= 1.0)
                .map(|px| px.round() as u32);
            physical.unwrap_or(max_side).min(max_side)
        }

        pub(crate) fn status(&self, ctx: &Context) -> GlassStatus {
            if let Some(failure) = &self.failure {
                return failure.clone();
            }
            match &self.loaded {
                Some((path, small)) => GlassStatus::Shown {
                    path: path.clone(),
                    aligned: self.placed(ctx).is_some(),
                    working: [small.size[0], small.size[1]],
                },
                None => GlassStatus::Loading,
            }
        }

        /// Start decoding the wallpaper at `width`, without disturbing what is already shown (if any):
        /// callers keep drawing the current texture, if there is one, until this job replaces it.
        fn start(&mut self, ctx: &Context, width: u32, max_side: u32, max_alloc: u64) {
            let (tx, rx) = channel();
            let explicit = self.wallpaper.clone();
            let output = self.output.clone();
            let dark = ctx.theme() == egui::Theme::Dark;
            let ctx = ctx.clone();
            let work = move || {
                let found = explicit.or_else(|| wallpaper::find(dark, output.as_deref()));
                let loaded = match found {
                    None => Err(GlassStatus::NoWallpaper),
                    Some(path) => match wallpaper::load(&path, width, max_side, max_alloc) {
                        Ok(small) => Ok((path, small)),
                        Err(error) => Err(GlassStatus::Failed { path, error }),
                    },
                };
                // The backdrop may be gone by now; then nobody is listening and that is fine.
                let _ = tx.send(loaded);
                ctx.request_repaint();
            };
            match std::thread::Builder::new()
                .name("ve_backdrop wallpaper".to_owned())
                .spawn(work)
            {
                Ok(_) => {
                    self.job = Some(rx);
                    self.requested_width = Some(width);
                }
                Err(e) => self.failure = Some(self.failed(format!("no worker thread: {e}"))),
            }
        }

        fn failed(&self, error: String) -> GlassStatus {
            GlassStatus::Failed {
                path: self.wallpaper.clone().unwrap_or_default(),
                error,
            }
        }

        /// Take the worker's answer when it is there; does nothing without a job in flight.
        fn poll(&mut self) {
            let Some(rx) = &self.job else { return };
            match rx.try_recv() {
                Ok(Ok(loaded)) => self.loaded = Some(loaded),
                Ok(Err(failure)) => self.failure = Some(failure),
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => {
                    self.failure = Some(self.failed("the wallpaper reader stopped".to_owned()));
                }
            }
            self.job = None;
        }

        /// Poll the worker, ask for a rescale if the blur or the monitor moved enough, and return the
        /// blurred wallpaper for `rect`, once there is one.
        pub(crate) fn shape(
            &mut self,
            ctx: &Context,
            rect: Rect,
            settings: &Settings,
        ) -> Option<Shape> {
            #[cfg(unix)]
            if !self.provider_tried {
                self.provider_tried = true;
                let wake = ctx.clone();
                self.provider = crate::hyprland::Hyprland::connect(move || wake.request_repaint())
                    .map(|hyprland| Box::new(hyprland) as Box<dyn PositionProvider>);
            }
            // Another monitor may have another wallpaper: look again when the window changes monitors.
            let output = self.placement().and_then(|p| p.output);
            if output.is_some() && output != self.output {
                self.output = output;
                if self.wallpaper.is_none() && (self.loaded.is_some() || self.job.is_some()) {
                    self.reload();
                }
            }
            self.poll();
            if self.failure.is_none() && self.job.is_none() {
                let full = self.full_width_hint(ctx, settings.glass_max_side);
                let target = quantize_width(wallpaper::working_width(
                    settings.glass_blur,
                    full,
                    settings.glass_min_width,
                ));
                if self.requested_width != Some(target) {
                    self.start(
                        ctx,
                        target,
                        settings.glass_max_side,
                        settings.glass_max_alloc,
                    );
                }
            }
            let (_, small) = self.loaded.as_ref()?;
            let blur = (settings.glass_blur / BLUR_STEP).round() * BLUR_STEP;
            if self
                .texture
                .as_ref()
                .is_none_or(|(_, b, size)| *b != blur || *size != small.size)
            {
                let frosted = small.frosted(blur);
                let image = ColorImage::from_rgba_unmultiplied(frosted.size, &frosted.rgba);
                let texture = ctx.load_texture("ve_backdrop_glass", image, TextureOptions::LINEAR);
                self.texture = Some((texture, blur, small.size));
            }
            let (texture, ..) = self.texture.as_ref()?;
            // `rect` is in window coordinates; the placement is that of the whole content.
            let area = rect.translate(-ctx.content_rect().min.to_vec2());
            let uv = wallpaper_uv(small.aspect, self.placed(ctx), area);
            let mut mesh = Mesh::with_texture(texture.id());
            mesh.add_rect_with_uv(rect, uv, Color32::WHITE);
            Some(Shape::mesh(mesh))
        }
    }
}

#[cfg(not(all(feature = "glass", not(target_arch = "wasm32"))))]
mod imp {
    use super::*;

    /// No wallpaper support in this build: nothing is ever drawn.
    #[derive(Default)]
    pub(crate) struct Glass {
        _nothing: (),
    }

    impl Glass {
        pub(crate) fn set_wallpaper(&mut self, _path: Option<PathBuf>) {}

        pub(crate) fn set_position_provider(
            &mut self,
            _provider: Option<Box<dyn PositionProvider>>,
        ) {
        }

        pub(crate) fn reload(&mut self) {}

        pub(crate) fn status(&self, _ctx: &Context) -> GlassStatus {
            GlassStatus::Unsupported
        }

        pub(crate) fn shape(
            &mut self,
            _ctx: &Context,
            _rect: Rect,
            _settings: &Settings,
        ) -> Option<Shape> {
            let _ = (wallpaper_uv, Color32::WHITE, Placement::placement);
            None
        }
    }
}

pub(crate) use imp::Glass;

#[cfg(test)]
mod tests {
    use super::*;
    use egui::vec2;

    fn close(a: Rect, b: Rect) -> bool {
        (a.min - b.min).length() < 1e-4 && (a.max - b.max).length() < 1e-4
    }

    #[test]
    fn without_a_position_the_wallpaper_covers_the_rect() {
        let rect = Rect::from_min_size(pos2(50.0, 20.0), vec2(400.0, 400.0));
        // A 2:1 picture on a square: full height, the middle half of the width.
        let uv = wallpaper_uv(2.0, None, rect);
        assert!(
            close(uv, Rect::from_min_max(pos2(0.25, 0.0), pos2(0.75, 1.0))),
            "{uv:?}"
        );
    }

    #[test]
    fn with_a_position_the_piece_behind_the_window_is_shown() {
        let monitor = vec2(1600.0, 800.0);
        let window = Rect::from_min_size(pos2(400.0, 200.0), vec2(800.0, 400.0));
        let rect = Rect::from_min_size(pos2(0.0, 0.0), window.size());
        let uv = wallpaper_uv(2.0, Some((monitor, window)), rect);
        assert!(
            close(uv, Rect::from_min_max(pos2(0.25, 0.25), pos2(0.75, 0.75))),
            "{uv:?}"
        );
        // Moving the window moves the piece.
        let moved = wallpaper_uv(
            2.0,
            Some((monitor, window.translate(vec2(160.0, 0.0)))),
            rect,
        );
        assert!((moved.min.x - 0.35).abs() < 1e-4);
        // The same place on a second monitor shows the same piece.
        let second = wallpaper_uv(
            2.0,
            Some((monitor, window.translate(vec2(1600.0, 0.0)))),
            rect,
        );
        assert!(close(uv, second));
    }

    #[test]
    fn a_wider_monitor_cuts_the_picture_top_and_bottom() {
        let monitor = vec2(2000.0, 500.0);
        let window = Rect::from_min_size(pos2(0.0, 0.0), monitor);
        let uv = wallpaper_uv(2.0, Some((monitor, window)), window);
        assert!(
            close(uv, Rect::from_min_max(pos2(0.0, 0.25), pos2(1.0, 0.75))),
            "{uv:?}"
        );
    }

    #[test]
    fn every_status_says_something() {
        let path = PathBuf::from("/a/w.jpg");
        for status in [
            GlassStatus::Off,
            GlassStatus::Unsupported,
            GlassStatus::Loading,
            GlassStatus::Shown {
                path: path.clone(),
                aligned: false,
                working: [320, 180],
            },
            GlassStatus::Shown {
                path: path.clone(),
                aligned: true,
                working: [1920, 1080],
            },
            GlassStatus::NoWallpaper,
            GlassStatus::Failed {
                path,
                error: "broken".to_owned(),
            },
        ] {
            assert!(!status.text().is_empty());
        }
    }
}
