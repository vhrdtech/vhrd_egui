//! From the puck's raw axes to what the app does with them: pan, zoom, rotate (SPM-2).

use serde::{Deserialize, Serialize};

use crate::event::{AXIS_NAMES, Axes};

/// What the puck does to the content, one value per output, each fed by one raw axis.
///
/// The outputs say what the *content* (the page, the board, the model) does, not the camera: positive
/// [`Output::MoveRight`] moves the page right on the screen. With [`Convention::Content`], the default, the content
/// follows the hand: slide the puck right and the page moves right. [`Convention::Camera`] turns every output
/// around, for people who think of the puck as the camera.
///
/// All six are in the screen's frame, x right, y up, z towards the viewer, the frame the puck itself is read in
/// (lifting it moves the content up, pushing it away moves the content away): the three rotations are about the
/// screen's x ([`Output::TiltForward`]), y ([`Output::SpinClockwise`]) and z ([`Output::TurnClockwise`]). Rocking
/// the puck to the right turns the content on the screen; twisting it spins the content about the screen's
/// vertical.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Output {
    /// The page moves right (positive) or left.
    MoveRight,
    /// The page moves up (positive) or down.
    MoveUp,
    /// The page gets bigger (positive: zoom in) or smaller.
    Grow,
    /// The page tilts forward about the screen's horizontal axis, its top going away from you (positive), or back.
    TiltForward,
    /// The page spins clockwise seen from above, about the screen's vertical axis: its right side comes towards
    /// you (positive), or goes away.
    SpinClockwise,
    /// The page turns clockwise on the screen, about the axis towards you: its right side goes down (positive), or
    /// up.
    TurnClockwise,
}

impl Output {
    /// All outputs, in [`Mapping::axes`] order.
    pub const ALL: [Output; 6] = [
        Output::MoveRight,
        Output::MoveUp,
        Output::Grow,
        Output::TiltForward,
        Output::SpinClockwise,
        Output::TurnClockwise,
    ];

    /// Index into [`Mapping::axes`].
    pub fn index(self) -> usize {
        self as usize
    }

    /// A short name for settings: "move right".
    pub fn name(self) -> &'static str {
        match self {
            Output::MoveRight => "move right",
            Output::MoveUp => "move up",
            Output::Grow => "grow (zoom in)",
            Output::TiltForward => "tilt forward",
            Output::SpinClockwise => "spin clockwise (seen from above)",
            Output::TurnClockwise => "turn clockwise (on the screen)",
        }
    }

    /// What the page does when this output goes positive (`true`) or negative: "the page moves right",
    /// "the page gets smaller".
    pub fn effect(self, positive: bool) -> &'static str {
        match (self, positive) {
            (Output::MoveRight, true) => "the page moves right",
            (Output::MoveRight, false) => "the page moves left",
            (Output::MoveUp, true) => "the page moves up",
            (Output::MoveUp, false) => "the page moves down",
            (Output::Grow, true) => "the page gets bigger",
            (Output::Grow, false) => "the page gets smaller",
            (Output::TiltForward, true) => "the page tilts forward, its top going away from you",
            (Output::TiltForward, false) => "the page tilts back, its top coming towards you",
            (Output::SpinClockwise, true) => {
                "the page spins clockwise seen from above, its right side coming towards you"
            }
            (Output::SpinClockwise, false) => {
                "the page spins counterclockwise seen from above, its right side going away"
            }
            (Output::TurnClockwise, true) => {
                "the page turns clockwise on the screen, its right side going down"
            }
            (Output::TurnClockwise, false) => {
                "the page turns counterclockwise on the screen, its right side going up"
            }
        }
    }
}

/// Whose motion the puck is: the content's or the camera's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Convention {
    /// The content follows the hand: slide right and the page moves right, push away and it gets smaller, twist
    /// clockwise and it turns clockwise. KiCad's 2D view with `reverse_pan_x` / `reverse_pan_y` and FreeCAD's
    /// object mode work this way.
    #[default]
    Content,
    /// The puck moves the camera: slide right and the view moves right, so the page moves left; push away and the
    /// camera goes in, so the page gets bigger.
    Camera,
}

impl Convention {
    /// A short name for settings.
    pub fn name(self) -> &'static str {
        match self {
            Convention::Content => "content follows the puck",
            Convention::Camera => "the puck moves the camera",
        }
    }

    /// `1.0` for content, `-1.0` for camera: what an output's sign is multiplied with.
    pub fn sign(self) -> f32 {
        match self {
            Convention::Content => 1.0,
            Convention::Camera => -1.0,
        }
    }
}

/// Which raw axis feeds an output, and whether it is reversed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AxisMap {
    /// Index into [`Axes`] (0 x, 1 y, 2 z, 3 rx, 4 ry, 5 rz); `None` turns the output off.
    pub raw: Option<usize>,
    /// Reverse the sign.
    pub invert: bool,
}

impl AxisMap {
    /// `raw` as is.
    pub fn new(raw: usize) -> Self {
        Self {
            raw: Some(raw),
            invert: false,
        }
    }

    /// `raw` reversed.
    pub fn inverted(raw: usize) -> Self {
        Self {
            raw: Some(raw),
            invert: true,
        }
    }

    /// "x", "-z" or "off", for tables and tooltips.
    pub fn describe(&self) -> String {
        match self.raw.and_then(|r| AXIS_NAMES.get(r)) {
            Some(name) if self.invert => format!("-{name}"),
            Some(name) => (*name).to_string(),
            None => "off".into(),
        }
    }
}

/// How fast each kind of output goes at a full push.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Speed {
    /// Screen points per second at a full push. An app pans its content in screen points, not in its own units,
    /// so the speed feels the same at every zoom: `offset += nav.pan_screen() * dt` with the offset in points.
    pub pan: f32,
    /// Zoom steps per second, as a natural log of the scale: 1.0 is ×2.7 a second.
    pub zoom: f32,
    /// Radians per second.
    pub rotate: f32,
}

impl Speed {
    /// The range [`crate::mapping_ui`] offers for [`Speed::pan`].
    pub const PAN_RANGE: std::ops::RangeInclusive<f32> = 50.0..=15000.0;
    /// The range offered for [`Speed::zoom`].
    pub const ZOOM_RANGE: std::ops::RangeInclusive<f32> = 0.1..=15.0;
    /// The range offered for [`Speed::rotate`].
    pub const ROTATE_RANGE: std::ops::RangeInclusive<f32> = 0.1..=30.0;
}

impl Default for Speed {
    fn default() -> Self {
        Self {
            pan: 1200.0,
            zoom: 1.5,
            rotate: 2.0,
        }
    }
}

/// The spacenavd button numbers of the two buttons.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Buttons {
    /// The left button's number.
    pub left: i32,
    /// The right button's number.
    pub right: i32,
}

impl Default for Buttons {
    fn default() -> Self {
        Self { left: 0, right: 1 }
    }
}

/// Left or right button of the device.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Side {
    /// The left button.
    Left,
    /// The right button.
    Right,
}

impl Side {
    /// "left" or "right".
    pub fn name(self) -> &'static str {
        match self {
            Side::Left => "left",
            Side::Right => "right",
        }
    }
}

/// The per-app settings of the puck: which raw axis does what, the convention, dead zone, speed, which button is
/// which.
///
/// Keep it in the app's settings (it is `serde`); [`crate::Calibration`] measures it with the person, and
/// [`crate::mapping_ui`] edits it by hand. The default axes are a guess; run the calibrate step once per device and
/// `/etc/spnavrc`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Mapping {
    /// One per [`Output`], in [`Output::ALL`] order: the raw axis and sign that make the content do it under
    /// [`Convention::Content`]. The calibrate step measures them.
    pub axes: [AxisMap; 6],
    /// Content (the default) or camera: [`Mapping::apply`] turns every output around for the camera.
    pub convention: Convention,
    /// The raw value of a full push: a deflection this big gives full speed. The calibrate step sets it from the
    /// pushes it recorded.
    pub full_scale: f32,
    /// The part of the travel, 0 to 1 of `full_scale`, that does nothing, so a hand resting on the puck does not
    /// drift the view.
    pub dead_zone: f32,
    /// Speeds at a full push.
    pub speed: Speed,
    /// Which button number is left and which right.
    pub buttons: Buttons,
}

impl Default for Mapping {
    fn default() -> Self {
        Self {
            axes: [
                AxisMap::new(0),
                AxisMap::new(1),
                AxisMap::new(2),
                AxisMap::new(3),
                AxisMap::new(4),
                AxisMap::new(5),
            ],
            convention: Convention::Content,
            full_scale: 350.0,
            dead_zone: 0.08,
            speed: Speed::default(),
            buttons: Buttons::default(),
        }
    }
}

/// What the puck asks the content to do now, per second: multiply by the frame time. The convention is already
/// in it, so an app always moves its content by these numbers.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Nav {
    /// The content's motion in points per second, x right, y up: `offset += nav.pan_screen() * dt`.
    pub pan: egui::Vec2,
    /// The content's growth, log scale per second: `scale *= (zoom * dt).exp()`. Positive is zoom in.
    pub zoom: f32,
    /// The content's rotation in radians per second about the screen's axes: tilt forward (about x, the top going
    /// away), spin clockwise seen from above (about y, the right side coming towards you), turn clockwise on the
    /// screen (about z, the right side going down). In a right-handed frame of x right, y up, z towards the viewer
    /// that is `[-rx, -ry, -rz]`: every positive output turns the content the negative way about its axis.
    pub rotate: [f32; 3],
}

impl Nav {
    /// Nothing to do: the puck is at rest or inside the dead zone.
    pub fn is_zero(&self) -> bool {
        self.pan == egui::Vec2::ZERO && self.zoom == 0.0 && self.rotate == [0.0; 3]
    }

    /// The content's motion in egui's screen direction (y down), points per second.
    pub fn pan_screen(&self) -> egui::Vec2 {
        egui::vec2(self.pan.x, -self.pan.y)
    }
}

impl Mapping {
    /// The map of one output.
    pub fn axis(&self, output: Output) -> AxisMap {
        self.axes[output.index()]
    }

    /// One output from the raw axes: -1 to 1, the dead zone taken out, the convention applied.
    pub fn level(&self, output: Output, raw: &Axes) -> f32 {
        let map = self.axis(output);
        let Some(value) = map.raw.and_then(|r| raw.get(r)) else {
            return 0.0;
        };
        let full = self.full_scale.max(1.0);
        let v = (*value as f32 / full).clamp(-1.0, 1.0);
        let dz = self.dead_zone.clamp(0.0, 0.95);
        let past = (v.abs() - dz).max(0.0) / (1.0 - dz);
        let v = past.copysign(v) * if map.invert { -1.0 } else { 1.0 };
        if past == 0.0 {
            0.0
        } else {
            v * self.convention.sign()
        }
    }

    /// What the puck asks the content to do now, per second.
    pub fn apply(&self, raw: &Axes) -> Nav {
        let l = |o| self.level(o, raw);
        Nav {
            pan: egui::vec2(l(Output::MoveRight), l(Output::MoveUp)) * self.speed.pan,
            zoom: l(Output::Grow) * self.speed.zoom,
            rotate: [
                l(Output::TiltForward) * self.speed.rotate,
                l(Output::SpinClockwise) * self.speed.rotate,
                l(Output::TurnClockwise) * self.speed.rotate,
            ],
        }
    }

    /// The side of a spacenavd button number, `None` for a button that is neither.
    pub fn side_of(&self, number: i32) -> Option<Side> {
        if number == self.buttons.left {
            Some(Side::Left)
        } else if number == self.buttons.right {
            Some(Side::Right)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dead_zone_and_full_scale() {
        let m = Mapping {
            dead_zone: 0.1,
            full_scale: 100.0,
            ..Default::default()
        };
        assert_eq!(m.level(Output::MoveRight, &[5, 0, 0, 0, 0, 0]), 0.0);
        assert_eq!(m.level(Output::MoveRight, &[-10, 0, 0, 0, 0, 0]), 0.0);
        assert!((m.level(Output::MoveRight, &[55, 0, 0, 0, 0, 0]) - 0.5).abs() < 1e-6);
        assert_eq!(m.level(Output::MoveRight, &[400, 0, 0, 0, 0, 0]), 1.0);
        assert_eq!(m.level(Output::MoveRight, &[-400, 0, 0, 0, 0, 0]), -1.0);
    }

    #[test]
    fn invert_and_off() {
        let mut m = Mapping {
            dead_zone: 0.0,
            full_scale: 100.0,
            ..Default::default()
        };
        m.axes[Output::MoveUp.index()] = AxisMap::inverted(2);
        m.axes[Output::Grow.index()] = AxisMap::default();
        let nav = m.apply(&[0, 0, 50, 0, 0, 0]);
        assert_eq!(nav.pan, egui::vec2(0.0, -0.5 * m.speed.pan));
        assert_eq!(nav.zoom, 0.0);
        assert_eq!(nav.pan_screen().y, 0.5 * m.speed.pan);
        assert!(m.apply(&[0; 6]).is_zero());
    }

    /// The sliders reach three times the first ranges (pan 5000, zoom 5, rotate 10), the defaults in the middle.
    #[test]
    fn speed_ranges_reach_well_past_the_defaults() {
        let d = Speed::default();
        assert_eq!(*Speed::PAN_RANGE.end(), 3.0 * 5000.0);
        assert_eq!(*Speed::ZOOM_RANGE.end(), 3.0 * 5.0);
        assert_eq!(*Speed::ROTATE_RANGE.end(), 3.0 * 10.0);
        assert!(Speed::PAN_RANGE.contains(&d.pan));
        assert!(Speed::ZOOM_RANGE.contains(&d.zoom));
        assert!(Speed::ROTATE_RANGE.contains(&d.rotate));
    }

    #[test]
    fn camera_turns_every_output_around() {
        let content = Mapping {
            dead_zone: 0.0,
            full_scale: 100.0,
            ..Default::default()
        };
        let camera = Mapping {
            convention: Convention::Camera,
            ..content.clone()
        };
        let raw = [50, -50, 50, 50, -50, 50];
        let (a, b) = (content.apply(&raw), camera.apply(&raw));
        assert_eq!(a.pan, -b.pan);
        assert_eq!(a.zoom, -b.zoom);
        assert_eq!(a.rotate.map(|r| -r), b.rotate);
        assert_ne!(a.pan, egui::Vec2::ZERO);
    }

    #[test]
    fn buttons_by_side() {
        let m = Mapping {
            buttons: Buttons {
                left: 12,
                right: 13,
            },
            ..Default::default()
        };
        assert_eq!(m.side_of(12), Some(Side::Left));
        assert_eq!(m.side_of(13), Some(Side::Right));
        assert_eq!(m.side_of(0), None);
    }

    /// A saved mapping from before a field existed loads, the missing fields at their defaults.
    #[test]
    fn old_settings_load_with_defaults() {
        use serde::de::value::{Error, MapDeserializer};
        let m = Mapping::deserialize(MapDeserializer::<_, Error>::new(
            [("dead_zone", 0.2f32)].into_iter(),
        ))
        .expect("a partial mapping loads");
        assert_eq!(m.dead_zone, 0.2);
        assert_eq!(m.speed, Speed::default());
        assert_eq!(m.buttons, Buttons::default());
    }
}
