//! Every tunable of the backdrop ([`Settings`]) and the form that edits them ([`Settings::ui`]).

use std::ops::RangeInclusive;

use egui::{Slider, Ui};
use ve_theme::UiExt as _;

/// What a grid cell draws.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Mark {
    /// Round dots.
    #[default]
    Dots,
    /// Small squares.
    Squares,
}

/// Every tunable of the backdrop. The defaults are calm: low contrast, slow.
///
/// Lengths are in points, times in seconds; "light" values add to a mark's opacity (0-1), "grow" values to
/// its size (0.5 = half again as big).
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct Settings {
    /// Dots or squares.
    pub mark: Mark,
    /// Distance between neighbouring marks.
    pub spacing: f32,
    /// Diameter of a dot, side of a square, at rest.
    pub size: f32,
    /// Opacity of a mark at rest.
    pub opacity: f32,
    /// Opacity of the theme's background colour under the marks: 1 hides what is behind the window, less
    /// lets a transparent window show the desktop (blurred, where the compositor does that), 0 paints none.
    pub background_opacity: f32,

    /// Emulated transparency: draw a blurred copy of the desktop wallpaper under the marks, for desktops
    /// that do not blur what is behind a window.
    pub glass: bool,
    /// How much of the theme's background colour lies over the blurred wallpaper (0 = the bare wallpaper,
    /// 1 = hidden).
    pub glass_dim: f32,
    /// How strongly the wallpaper is blurred (0 = sharp, 1 = strongest).
    pub glass_blur: f32,
    /// Narrowest the wallpaper is ever decoded to, in pixels: the floor the working resolution shrinks
    /// towards as [`Settings::glass_blur`] grows (it hides detail anyway, so there is no point decoding
    /// finer than this).
    pub glass_min_width: u32,
    /// Widest side the wallpaper decoder ever reads, in pixels: a safety cap on a huge or corrupt file.
    pub glass_max_side: u32,
    /// Memory the wallpaper decoder may use, in bytes: a safety cap alongside [`Settings::glass_max_side`].
    pub glass_max_alloc: u64,

    /// Freeze the backdrop: no drift, no pointer lens, no ripples, no fades; frames still gather light.
    pub reduced_motion: bool,
    /// How much the slow drift wave changes the opacity of the marks (0 = none, 1 = down to nothing).
    pub ambient: f32,
    /// Speed of the drift wave, in radians of its phase per second.
    pub speed: f32,
    /// Length of the drift wave.
    pub wave_length: f32,
    /// Repaints per second while only the drift moves (pointer movement repaints anyway).
    pub fps: f32,

    /// Radius of the lens around the pointer.
    pub lens_radius: f32,
    /// How far the lens pushes marks away from the pointer, at most.
    pub lens_push: f32,
    /// How much marks grow under the pointer.
    pub lens_grow: f32,
    /// How much marks brighten under the pointer.
    pub lens_light: f32,

    /// How much the ring of a click ripple brightens marks (0 = no ripples).
    pub ripple_light: f32,
    /// How much marks grow on the ring.
    pub ripple_grow: f32,
    /// How far the ring pushes marks outward.
    pub ripple_push: f32,
    /// Speed of the ring, points per second.
    pub ripple_speed: f32,
    /// Width of the ring.
    pub ripple_width: f32,
    /// How long a ripple lasts.
    pub ripple_life: f32,

    /// How far outside a frame its light reaches.
    pub glow_radius: f32,
    /// How much marks brighten under and around a frame.
    pub glow_light: f32,
    /// How much marks grow under and around a frame.
    pub glow_grow: f32,
    /// Light of the focused frame, as a multiple of a plain one.
    pub focus_boost: f32,
    /// Light of a floating frame, as a multiple of a plain one.
    pub float_boost: f32,
    /// How long the light of a frame takes to follow a change (focus moved, frame opened or closed).
    pub glow_fade: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            mark: Mark::Dots,
            spacing: 22.0,
            size: 2.0,
            opacity: 0.5,
            background_opacity: 1.0,
            glass: false,
            glass_dim: 0.97,
            glass_blur: 0.0,
            glass_min_width: 320,
            glass_max_side: 16_384,
            glass_max_alloc: 768 * 1024 * 1024,
            reduced_motion: false,
            ambient: 0.3,
            speed: 0.15,
            wave_length: 420.0,
            fps: 30.0,
            lens_radius: 110.0,
            lens_push: 5.0,
            lens_grow: 0.6,
            lens_light: 0.35,
            ripple_light: 0.5,
            ripple_grow: 0.8,
            ripple_push: 3.0,
            ripple_speed: 260.0,
            ripple_width: 28.0,
            ripple_life: 1.4,
            glow_radius: 48.0,
            glow_light: 0.6,
            glow_grow: 0.6,
            focus_boost: 1.8,
            float_boost: 1.4,
            glow_fade: 0.18,
        }
    }
}

impl Settings {
    /// The defaults with [`Settings::reduced_motion`] on.
    pub fn reduced() -> Self {
        Self {
            reduced_motion: true,
            ..Self::default()
        }
    }

    /// The defaults without anything that moves on its own: no drift, no click ripples, no fades (a frame's
    /// light jumps). Light comes from the frames and the pointer lens only, so the backdrop never asks for
    /// a repaint by itself: for a host that repaints on data and input only (a web dashboard, a phone).
    /// Unlike [`Settings::reduced`] the lens still follows the pointer.
    ///
    /// ```
    /// let s = ve_backdrop::Settings::still();
    /// assert!(s.ambient == 0.0 && s.ripple_light == 0.0 && s.glow_fade == 0.0 && !s.reduced_motion);
    /// ```
    pub fn still() -> Self {
        Self {
            ambient: 0.0,
            ripple_light: 0.0,
            glow_fade: 0.0,
            ..Self::default()
        }
    }

    /// Bring every value into the range the backdrop can draw (a settings file edited by hand, a slider typed
    /// past its end).
    pub fn sanitized(mut self) -> Self {
        let fix = |v: &mut f32, range: RangeInclusive<f32>, fallback: f32| {
            *v = if v.is_finite() {
                v.clamp(*range.start(), *range.end())
            } else {
                fallback
            };
        };
        let d = Self::default();
        fix(&mut self.spacing, 6.0..=200.0, d.spacing);
        fix(&mut self.size, 0.0..=200.0, d.size);
        fix(&mut self.opacity, 0.0..=1.0, d.opacity);
        fix(
            &mut self.background_opacity,
            0.0..=1.0,
            d.background_opacity,
        );
        fix(&mut self.glass_dim, 0.0..=1.0, d.glass_dim);
        fix(&mut self.glass_blur, 0.0..=1.0, d.glass_blur);
        self.glass_min_width = self.glass_min_width.clamp(16, 4_096);
        self.glass_max_side = self.glass_max_side.clamp(self.glass_min_width, 65_536);
        self.glass_max_alloc = self
            .glass_max_alloc
            .clamp(16 * 1024 * 1024, 4 * 1024 * 1024 * 1024);
        fix(&mut self.ambient, 0.0..=1.0, d.ambient);
        fix(&mut self.speed, 0.0..=10.0, d.speed);
        fix(&mut self.wave_length, 20.0..=10_000.0, d.wave_length);
        fix(&mut self.fps, 1.0..=240.0, d.fps);
        fix(&mut self.lens_radius, 1.0..=2000.0, d.lens_radius);
        fix(&mut self.lens_push, -100.0..=100.0, d.lens_push);
        fix(&mut self.lens_grow, -1.0..=10.0, d.lens_grow);
        fix(&mut self.lens_light, 0.0..=1.0, d.lens_light);
        fix(&mut self.ripple_light, 0.0..=1.0, d.ripple_light);
        fix(&mut self.ripple_grow, -1.0..=10.0, d.ripple_grow);
        fix(&mut self.ripple_push, -100.0..=100.0, d.ripple_push);
        fix(&mut self.ripple_speed, 1.0..=5000.0, d.ripple_speed);
        fix(&mut self.ripple_width, 1.0..=1000.0, d.ripple_width);
        fix(&mut self.ripple_life, 0.05..=30.0, d.ripple_life);
        fix(&mut self.glow_radius, 1.0..=2000.0, d.glow_radius);
        fix(&mut self.glow_light, 0.0..=1.0, d.glow_light);
        fix(&mut self.glow_grow, -1.0..=10.0, d.glow_grow);
        fix(&mut self.focus_boost, 0.0..=10.0, d.focus_boost);
        fix(&mut self.float_boost, 0.0..=10.0, d.float_boost);
        fix(&mut self.glow_fade, 0.0..=10.0, d.glow_fade);
        self
    }

    /// A form with a slider for every tunable, grouped, each with a tooltip saying what it does and its unit.
    /// Returns whether anything changed (persist the settings then).
    pub fn ui(&mut self, ui: &mut Ui) -> bool {
        let before = *self;

        ui.section_header("Grid", "the marks at rest");
        ui.horizontal(|ui| {
            ui.label("Mark").on_hover_text("what every grid cell draws");
            ui.selectable_value(&mut self.mark, Mark::Dots, "Dots")
                .on_hover_text("round dots");
            ui.selectable_value(&mut self.mark, Mark::Squares, "Squares")
                .on_hover_text("small squares");
        });
        slider(
            ui,
            &mut self.spacing,
            8.0..=64.0,
            "Spacing",
            " pt",
            "distance between neighbouring marks, in points",
        );
        slider(
            ui,
            &mut self.size,
            0.5..=12.0,
            "Size",
            " pt",
            "diameter of a dot or side of a square at rest, in points",
        );
        slider(
            ui,
            &mut self.opacity,
            0.0..=1.0,
            "Opacity",
            "",
            "opacity of a mark at rest: 0 invisible, 1 the full mark colour",
        );
        slider(
            ui,
            &mut self.background_opacity,
            0.0..=1.0,
            "Background",
            "",
            "opacity of the theme's background colour under the marks: below 1 a transparent window shows \
             the desktop behind it (blurred where the compositor does that)",
        );

        ui.section_header(
            "Glass",
            "emulated transparency: a blurred copy of the desktop wallpaper under the marks, for desktops \
             that do not blur what is behind a window",
        );
        ui.checkbox(&mut self.glass, "Emulated transparency")
            .on_hover_text(
                "draw a blurred, dimmed copy of the desktop wallpaper behind the grid, so the window looks \
                 like frosted glass also where the compositor does not blur",
            );
        ui.add_enabled_ui(self.glass, |ui| {
            slider(
                ui,
                &mut self.glass_dim,
                0.0..=1.0,
                "Dim",
                "",
                "how much of the theme's background colour lies over the wallpaper: 0 the bare wallpaper, \
                 1 hidden",
            );
            slider(
                ui,
                &mut self.glass_blur,
                0.0..=1.0,
                "Blur",
                "",
                "how strongly the wallpaper is blurred: 0 sharp, 1 strongest",
            );
        })
        .response
        .on_disabled_hover_text("off: emulated transparency is not turned on");

        ui.section_header("Motion", "the slow drift and how often it repaints");
        ui.checkbox(&mut self.reduced_motion, "Reduced motion")
            .on_hover_text(
                "freeze the backdrop: no drift, no pointer lens, no ripples, no fades; frames still gather light",
            );
        ui.add_enabled_ui(!self.reduced_motion, |ui| {
            slider(
                ui,
                &mut self.ambient,
                0.0..=1.0,
                "Drift",
                "",
                "how much the slow wave changes the opacity of the marks: 0 none, 1 down to nothing",
            );
            slider(
                ui,
                &mut self.speed,
                0.0..=2.0,
                "Speed",
                " rad/s",
                "speed of the drift wave, in radians of its phase per second",
            );
            slider(
                ui,
                &mut self.fps,
                5.0..=120.0,
                "Frame rate",
                " fps",
                "repaints per second while only the drift moves; lower saves power",
            );
        })
        .response
        .on_disabled_hover_text("frozen: reduced motion is on");
        slider(
            ui,
            &mut self.wave_length,
            60.0..=2000.0,
            "Wave length",
            " pt",
            "length of the drift wave, in points",
        );

        ui.section_header("Pointer", "the soft lens around the mouse pointer");
        ui.add_enabled_ui(!self.reduced_motion, |ui| {
            slider(
                ui,
                &mut self.lens_radius,
                20.0..=400.0,
                "Lens radius",
                " pt",
                "radius of the lens around the pointer, in points",
            );
            slider(
                ui,
                &mut self.lens_push,
                -12.0..=12.0,
                "Lens push",
                " pt",
                "how far marks move away from the pointer at most, in points; negative pulls them in",
            );
            slider(
                ui,
                &mut self.lens_grow,
                0.0..=3.0,
                "Lens grow",
                "×",
                "how much marks grow under the pointer: 1 doubles them",
            );
            slider(
                ui,
                &mut self.lens_light,
                0.0..=1.0,
                "Lens light",
                "",
                "opacity added to marks under the pointer",
            );
        })
        .response
        .on_disabled_hover_text("off: reduced motion is on");

        ui.section_header("Ripple", "the ring that spreads from a click");
        ui.add_enabled_ui(!self.reduced_motion, |ui| {
            slider(
                ui,
                &mut self.ripple_light,
                0.0..=1.0,
                "Ripple light",
                "",
                "opacity the ring adds to marks; 0 turns ripples off",
            );
            slider(
                ui,
                &mut self.ripple_grow,
                0.0..=3.0,
                "Ripple grow",
                "×",
                "how much marks grow on the ring: 1 doubles them",
            );
            slider(
                ui,
                &mut self.ripple_push,
                -12.0..=12.0,
                "Ripple push",
                " pt",
                "how far the ring moves marks outward, in points",
            );
            slider(
                ui,
                &mut self.ripple_speed,
                40.0..=1200.0,
                "Ripple speed",
                " pt/s",
                "how fast the ring spreads, in points per second",
            );
            slider(
                ui,
                &mut self.ripple_width,
                4.0..=120.0,
                "Ripple width",
                " pt",
                "width of the ring, in points",
            );
            slider(
                ui,
                &mut self.ripple_life,
                0.2..=5.0,
                "Ripple life",
                " s",
                "how long a ripple lasts, in seconds",
            );
        })
        .response
        .on_disabled_hover_text("off: reduced motion is on");

        ui.section_header("Frames", "light gathering under widget frames");
        slider(
            ui,
            &mut self.glow_radius,
            4.0..=240.0,
            "Glow reach",
            " pt",
            "how far outside a frame its light reaches, in points",
        );
        slider(
            ui,
            &mut self.glow_light,
            0.0..=1.0,
            "Glow light",
            "",
            "opacity added to marks under and around a frame",
        );
        slider(
            ui,
            &mut self.glow_grow,
            0.0..=3.0,
            "Glow grow",
            "×",
            "how much marks grow under and around a frame: 1 doubles them",
        );
        slider(
            ui,
            &mut self.focus_boost,
            0.0..=4.0,
            "Focused",
            "×",
            "light of the focused frame as a multiple of a plain one",
        );
        slider(
            ui,
            &mut self.float_boost,
            0.0..=4.0,
            "Floating",
            "×",
            "light of a floating frame as a multiple of a plain one",
        );
        ui.add_enabled_ui(!self.reduced_motion, |ui| {
            slider(
                ui,
                &mut self.glow_fade,
                0.0..=1.5,
                "Glow fade",
                " s",
                "how long a frame's light takes to follow a change, in seconds; 0 jumps",
            );
        })
        .response
        .on_disabled_hover_text("jumps: reduced motion is on");

        let gap = ui.tokens().space.m;
        ui.add_space(gap);
        if ui
            .button("Reset to defaults")
            .on_hover_text("set every backdrop value back to its default")
            .clicked()
        {
            *self = Self::default();
        }

        *self = self.sanitized();
        *self != before
    }
}

fn slider(
    ui: &mut Ui,
    value: &mut f32,
    range: RangeInclusive<f32>,
    label: &str,
    suffix: &str,
    tooltip: &str,
) {
    ui.add(Slider::new(value, range).text(label).suffix(suffix))
        .on_hover_text(tooltip);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_calm_and_survive_sanitizing() {
        let d = Settings::default();
        assert_eq!(d.sanitized(), d);
        assert!(d.opacity <= 0.6 && d.speed <= 0.3 && !d.reduced_motion);
        // A faint hint of the wallpaper, sharp: no blur unless the user turns it up.
        assert!(d.glass_dim > 0.9 && d.glass_blur == 0.0);
    }

    #[test]
    fn sanitizing_repairs_nonsense() {
        let s = Settings {
            spacing: 0.0,
            size: f32::NAN,
            opacity: 7.0,
            fps: -3.0,
            ..Settings::default()
        }
        .sanitized();
        assert_eq!(s.spacing, 6.0);
        assert_eq!(s.size, Settings::default().size);
        assert_eq!(s.opacity, 1.0);
        assert_eq!(s.fps, 1.0);
    }
}
