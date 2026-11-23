//! The eframe [`NativeOptions`](eframe::NativeOptions) every vhrd egui app starts from (PLT-11).
//!
//! - [`native_options`]: eframe's defaults with vsync set per [`Session`], for each renderer the crate is built
//!   with (features `glow`, `wgpu`). The app fills in its viewport and renderer on top:
//!   `NativeOptions { viewport, renderer, ..ve_native::native_options() }`.
//! - [`Session`]: whether the app runs on Wayland, read from the environment the way winit picks its backend.
//!
//! # Why vsync is off on Wayland
//!
//! Hyprland and other wlroots compositors stop sending frame callbacks to a window on a hidden workspace. A swap
//! with vsync (glow with `vsync`, wgpu with `PresentMode::AutoVsync` or `Fifo`) waits for the next callback, so
//! the event loop blocks until the workspace is shown again and the compositor flags the app as not responding.
//! On Wayland the options turn vsync off: glow swaps without waiting, wgpu presents `AutoNoVsync`. X11, Windows
//! and macOS keep vsync. egui still paints only when asked, so an idle window costs nothing. With wgpu, winit
//! still holds each redraw until the compositor's next frame callback, so a visible window keeps the display's
//! rate. glow has no such hold: a glow app that repaints every frame paces itself with `request_repaint_after`.

use std::ffi::OsString;

/// The display server the app's window lands on, as far as vsync is concerned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Session {
    /// A Wayland compositor: a hidden window gets no frame callbacks, so vsync would block the event loop.
    Wayland,
    /// X11, Windows, macOS, the web: vsync is safe.
    Other,
}

impl Session {
    /// The session of this process, from its environment.
    ///
    /// Wayland on Linux and the BSDs when `WAYLAND_DISPLAY` or `WAYLAND_SOCKET` is set and not empty: winit
    /// then opens the window on Wayland, even with `DISPLAY` set too. Other everywhere else.
    #[must_use]
    pub fn detect() -> Self {
        #[cfg(all(
            unix,
            not(any(target_os = "macos", target_os = "ios", target_os = "android"))
        ))]
        return Self::from_env(|name| std::env::var_os(name));
        #[cfg(not(all(
            unix,
            not(any(target_os = "macos", target_os = "ios", target_os = "android"))
        )))]
        Self::Other
    }

    /// The session an environment means on Linux, with `var` reading one variable.
    ///
    /// [`Session::detect`] calls it with the process environment; tests pass their own.
    #[must_use]
    pub fn from_env(var: impl Fn(&str) -> Option<OsString>) -> Self {
        let set = |name: &str| var(name).is_some_and(|value| !value.is_empty());
        if set("WAYLAND_DISPLAY") || set("WAYLAND_SOCKET") {
            Self::Wayland
        } else {
            Self::Other
        }
    }

    /// Whether a swap may wait for vsync in this session.
    #[must_use]
    pub fn vsync(self) -> bool {
        self == Self::Other
    }
}

/// eframe's defaults with vsync set for this process's [`Session`]; see the crate docs for why.
///
/// Sets `glow_options.vsync` with the feature `glow` and `wgpu_options.surface.present_mode` with `wgpu`.
#[cfg(all(any(feature = "glow", feature = "wgpu"), not(target_arch = "wasm32")))]
#[must_use]
pub fn native_options() -> eframe::NativeOptions {
    options_for(Session::detect())
}

/// eframe's defaults with vsync set for `session`: [`native_options`] for a given session.
#[cfg(all(any(feature = "glow", feature = "wgpu"), not(target_arch = "wasm32")))]
#[must_use]
pub fn options_for(session: Session) -> eframe::NativeOptions {
    let mut options = eframe::NativeOptions::default();
    #[cfg(feature = "glow")]
    {
        options.glow_options.vsync = session.vsync();
    }
    #[cfg(feature = "wgpu")]
    {
        options.wgpu_options.surface.present_mode = present_mode(session);
    }
    options
}

/// The wgpu present mode for `session`: `AutoVsync`, or `AutoNoVsync` on Wayland (Mailbox where the driver has
/// it, else Immediate).
#[cfg(all(feature = "wgpu", not(target_arch = "wasm32")))]
#[must_use]
pub fn present_mode(session: Session) -> eframe::wgpu::PresentMode {
    if session.vsync() {
        eframe::wgpu::PresentMode::AutoVsync
    } else {
        eframe::wgpu::PresentMode::AutoNoVsync
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(vars: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
        let vars: Vec<(String, String)> = vars
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        move |name| vars.iter().find(|(k, _)| k == name).map(|(_, v)| v.into())
    }

    #[test]
    fn wayland_display_means_wayland() {
        let s = Session::from_env(env(&[("WAYLAND_DISPLAY", "wayland-1")]));
        assert_eq!(s, Session::Wayland);
        assert!(!s.vsync());
    }

    #[test]
    fn wayland_socket_means_wayland() {
        let s = Session::from_env(env(&[("WAYLAND_SOCKET", "3")]));
        assert_eq!(s, Session::Wayland);
    }

    #[test]
    fn wayland_wins_over_x11() {
        let s = Session::from_env(env(&[("DISPLAY", ":0"), ("WAYLAND_DISPLAY", "wayland-1")]));
        assert_eq!(s, Session::Wayland);
    }

    #[test]
    fn x11_or_nothing_keeps_vsync() {
        for vars in [&[("DISPLAY", ":0")][..], &[][..]] {
            let s = Session::from_env(env(vars));
            assert_eq!(s, Session::Other);
            assert!(s.vsync());
        }
    }

    #[test]
    fn empty_wayland_display_is_not_wayland() {
        let s = Session::from_env(env(&[("WAYLAND_DISPLAY", ""), ("DISPLAY", ":0")]));
        assert_eq!(s, Session::Other);
    }

    #[cfg(feature = "glow")]
    #[test]
    fn glow_vsync_off_only_on_wayland() {
        assert!(!options_for(Session::Wayland).glow_options.vsync);
        assert!(options_for(Session::Other).glow_options.vsync);
    }

    #[cfg(feature = "wgpu")]
    #[test]
    fn wgpu_present_mode_no_vsync_only_on_wayland() {
        use eframe::wgpu::PresentMode;
        assert_eq!(
            options_for(Session::Wayland)
                .wgpu_options
                .surface
                .present_mode,
            PresentMode::AutoNoVsync
        );
        assert_eq!(
            options_for(Session::Other)
                .wgpu_options
                .surface
                .present_mode,
            PresentMode::AutoVsync
        );
    }
}
