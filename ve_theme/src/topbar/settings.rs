//! The top bar's settings: every alternative of the design's section A stays reachable, the pick is the
//! default (app-chrome.md S1, S4, S5, S13, S23, S24).

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// How the menus show (S1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum MenuStyle {
    /// The menu row and the command field, which shows the document's name until it is clicked (C, the pick).
    #[default]
    RowAndField,
    /// A classic menu row; the document's name centred as a title; search as a button (A).
    Classic,
    /// One menu button by the name; the document as a breadcrumb after it; the field says "Search or run a
    /// command" (B).
    Button,
}

/// The app's own window controls: minimise, maximise, close (S4).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum WindowControls {
    /// From the desktop ([`Desktop`]): none under a tiling window manager, ours at the left on macOS and at the
    /// right elsewhere.
    #[default]
    Auto,
    /// Ours, on the side the desktop puts them.
    Ours,
    /// The system's title bar draws them; the bar has none.
    System,
    /// None at all.
    None,
}

/// Where the build badge sits (S5).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum BadgePlace {
    /// At the right end, before the global actions (the pick).
    #[default]
    Right,
    /// Right after the app's name.
    ByName,
}

/// What the command field shows, or none at all (S13, S24).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum FieldMode {
    /// The document's name and its unsaved dot; "Search or run a command" with no document.
    #[default]
    Document,
    /// Always "Search or run a command".
    Search,
    /// No field: the centre is empty and Ctrl K still opens the palette.
    None,
}

/// What the collapsed menu row turns into (S23).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum MenuButton {
    /// The app's name with a chevron is the button (the pick).
    #[default]
    Name,
    /// The logo mark is the button, with a chevron on its corner.
    Mark,
    /// A 3 by 3 grid of dots.
    Dots,
    /// The word "Menu" with a chevron.
    Word,
    /// Three lines.
    Lines,
}

/// The top bar's settings, kept in the app's settings (serde with the feature `serde`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize), serde(default))]
pub struct TopBarSettings {
    /// S1.
    pub menu_style: MenuStyle,
    /// S4.
    pub window_controls: WindowControls,
    /// S5.
    pub badge: BadgePlace,
    /// S13 and S24.
    pub field: FieldMode,
    /// S23.
    pub menu_button: MenuButton,
}

/// The desktop the window lives on, for [`WindowControls::Auto`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Desktop {
    /// A tiling window manager (Hyprland, Sway, i3, niri, river, ...) or a browser: no controls.
    Tiling,
    /// macOS: controls at the left, close first.
    MacOs,
    /// Windows, GNOME, KDE and other stacking desktops: controls at the right.
    Stacking,
}

impl Desktop {
    /// The desktop this process runs on: the OS, and on Linux the session's window manager from the
    /// environment (`HYPRLAND_INSTANCE_SIGNATURE`, `SWAYSOCK`, `I3SOCK`, `NIRI_SOCKET`,
    /// `XDG_CURRENT_DESKTOP`). In a browser: [`Desktop::Tiling`] (the page has no window to control).
    pub fn detect() -> Self {
        #[cfg(target_arch = "wasm32")]
        {
            Self::Tiling
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            if cfg!(target_os = "macos") {
                return Self::MacOs;
            }
            if cfg!(target_os = "windows") {
                return Self::Stacking;
            }
            let var = |name: &str| std::env::var(name).ok();
            Self::of_linux(&var)
        }
    }

    /// The Linux desktop from the environment as `var` reads it.
    #[cfg(not(target_arch = "wasm32"))]
    fn of_linux(var: &dyn Fn(&str) -> Option<String>) -> Self {
        const SOCKETS: [&str; 4] = [
            "HYPRLAND_INSTANCE_SIGNATURE",
            "SWAYSOCK",
            "I3SOCK",
            "NIRI_SOCKET",
        ];
        const TILING: [&str; 8] = [
            "hyprland", "sway", "i3", "niri", "river", "bspwm", "dwm", "qtile",
        ];
        if SOCKETS
            .iter()
            .any(|s| var(s).is_some_and(|v| !v.is_empty()))
        {
            return Self::Tiling;
        }
        let current = var("XDG_CURRENT_DESKTOP")
            .unwrap_or_default()
            .to_lowercase();
        if current.split(':').any(|d| TILING.contains(&d)) {
            Self::Tiling
        } else {
            Self::Stacking
        }
    }
}

/// Where the bar draws window controls, once [`WindowControls`] met the [`Desktop`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ControlsAt {
    /// None in the bar.
    Nowhere,
    /// At the left end: close, minimise, maximise.
    Left,
    /// At the right end: minimise, maximise, close.
    Right,
}

impl WindowControls {
    /// Where the controls go on `desktop`.
    pub(crate) fn place(self, desktop: Desktop) -> ControlsAt {
        let side = if desktop == Desktop::MacOs {
            ControlsAt::Left
        } else {
            ControlsAt::Right
        };
        match (self, desktop) {
            (Self::System | Self::None, _) | (Self::Auto, Desktop::Tiling) => ControlsAt::Nowhere,
            (Self::Auto | Self::Ours, _) => side,
        }
    }

    /// Whether the system should draw its title bar and decorations.
    pub fn system_decorations(self, desktop: Desktop) -> bool {
        self == Self::System || (self == Self::Auto && desktop == Desktop::Tiling)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controls_follow_the_desktop() {
        use ControlsAt::*;
        for (wc, d, at) in [
            (WindowControls::Auto, Desktop::Tiling, Nowhere),
            (WindowControls::Auto, Desktop::MacOs, Left),
            (WindowControls::Auto, Desktop::Stacking, Right),
            (WindowControls::Ours, Desktop::Tiling, Right),
            (WindowControls::System, Desktop::Stacking, Nowhere),
            (WindowControls::None, Desktop::MacOs, Nowhere),
        ] {
            assert_eq!(wc.place(d), at, "{wc:?} on {d:?}");
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn linux_desktop_from_the_environment() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |k: &str| {
                pairs
                    .iter()
                    .find(|(n, _)| *n == k)
                    .map(|(_, v)| (*v).to_owned())
            }
        };
        assert_eq!(
            Desktop::of_linux(&env(&[("HYPRLAND_INSTANCE_SIGNATURE", "abc")])),
            Desktop::Tiling
        );
        assert_eq!(
            Desktop::of_linux(&env(&[("XDG_CURRENT_DESKTOP", "sway")])),
            Desktop::Tiling
        );
        assert_eq!(
            Desktop::of_linux(&env(&[("XDG_CURRENT_DESKTOP", "ubuntu:GNOME")])),
            Desktop::Stacking
        );
        assert_eq!(
            Desktop::of_linux(&env(&[("SWAYSOCK", "")])),
            Desktop::Stacking
        );
        assert_eq!(Desktop::of_linux(&env(&[])), Desktop::Stacking);
    }
}
