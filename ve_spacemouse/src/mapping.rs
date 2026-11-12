//! From the puck's raw axes to what the app does with them: pan, zoom, rotate (SPM-2).

use serde::{Deserialize, Serialize};

use crate::event::{AXIS_NAMES, Axes};

/// What an app does with the puck: one value per output, each fed by one raw axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Output {
    /// Pan the view right (positive) or left.
    PanRight,
    /// Pan the view up (positive) or down.
    PanUp,
    /// Zoom in (positive) or out.
    ZoomIn,
    /// Tilt the view forward (positive) or back: rotation about the screen's horizontal axis.
    TiltForward,
    /// Turn the view right (positive) or left: rotation about the screen's vertical axis.
    TurnRight,
    /// Spin the view clockwise (positive): rotation about the axis into the screen.
    SpinClockwise,
}

impl Output {
    /// All outputs, in [`Mapping::axes`] order.
    pub const ALL: [Output; 6] = [
        Output::PanRight,
        Output::PanUp,
        Output::ZoomIn,
        Output::TiltForward,
        Output::TurnRight,
        Output::SpinClockwise,
    ];

    /// The three a 2D view needs (pan and zoom), what [`crate::Calibration::two_d`] asks for.
    pub const TWO_D: [Output; 3] = [Output::PanRight, Output::PanUp, Output::ZoomIn];

    /// Index into [`Mapping::axes`].
    pub fn index(self) -> usize {
        self as usize
    }

    /// A short name for settings: "pan right".
    pub fn name(self) -> &'static str {
        match self {
            Output::PanRight => "pan right",
            Output::PanUp => "pan up",
            Output::ZoomIn => "zoom in",
            Output::TiltForward => "tilt forward",
            Output::TurnRight => "turn right",
            Output::SpinClockwise => "spin clockwise",
        }
    }

    /// What the motion should do, for the calibration prompt: "pan the view right".
    pub fn verb(self) -> &'static str {
        match self {
            Output::PanRight => "pan the view right",
            Output::PanUp => "pan the view up",
            Output::ZoomIn => "zoom in",
            Output::TiltForward => "tilt the view forward",
            Output::TurnRight => "turn the view right",
            Output::SpinClockwise => "spin the view clockwise",
        }
    }

    /// What the view does, for the calibration table: "the view pans right".
    pub fn effect(self) -> &'static str {
        match self {
            Output::PanRight => "the view pans right",
            Output::PanUp => "the view pans up",
            Output::ZoomIn => "the view zooms in",
            Output::TiltForward => "the view tilts forward",
            Output::TurnRight => "the view turns right",
            Output::SpinClockwise => "the view spins clockwise",
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
    /// Points per second.
    pub pan: f32,
    /// Zoom steps per second, as a natural log of the scale: 1.0 is ×2.7 a second.
    pub zoom: f32,
    /// Radians per second.
    pub rotate: f32,
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

/// The per-app settings of the puck: which raw axis does what, dead zone, speed, which button is which.
///
/// Keep it in the app's settings (it is `serde`); [`crate::Calibration`] measures it with the person, and
/// [`crate::mapping_ui`] edits it by hand. The default is a guess that suits a 2D view with spacenavd's defaults;
/// run the calibrate step once per device and `/etc/spnavrc`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Mapping {
    /// One per [`Output`], in [`Output::ALL`] order.
    pub axes: [AxisMap; 6],
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
                AxisMap::inverted(2),
                AxisMap::new(3),
                AxisMap::new(4),
                AxisMap::new(5),
            ],
            full_scale: 350.0,
            dead_zone: 0.08,
            speed: Speed::default(),
            buttons: Buttons::default(),
        }
    }
}

/// What the puck asks of the view now, per second: multiply by the frame time.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Nav {
    /// Pan in points per second, x right, y up.
    pub pan: egui::Vec2,
    /// Zoom rate, log scale per second: `scale *= (zoom * dt).exp()`.
    pub zoom: f32,
    /// Rotation in radians per second: tilt forward, turn right, spin clockwise.
    pub rotate: [f32; 3],
}

impl Nav {
    /// Nothing to do: the puck is at rest or inside the dead zone.
    pub fn is_zero(&self) -> bool {
        self.pan == egui::Vec2::ZERO && self.zoom == 0.0 && self.rotate == [0.0; 3]
    }

    /// The pan in egui's screen direction (y down), points per second.
    pub fn pan_screen(&self) -> egui::Vec2 {
        egui::vec2(self.pan.x, -self.pan.y)
    }
}

impl Mapping {
    /// The map of one output.
    pub fn axis(&self, output: Output) -> AxisMap {
        self.axes[output.index()]
    }

    /// One output from the raw axes: -1 to 1, the dead zone taken out.
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
        if past == 0.0 { 0.0 } else { v }
    }

    /// What the puck asks of the view now, per second.
    pub fn apply(&self, raw: &Axes) -> Nav {
        let l = |o| self.level(o, raw);
        Nav {
            pan: egui::vec2(l(Output::PanRight), l(Output::PanUp)) * self.speed.pan,
            zoom: l(Output::ZoomIn) * self.speed.zoom,
            rotate: [
                l(Output::TiltForward) * self.speed.rotate,
                l(Output::TurnRight) * self.speed.rotate,
                l(Output::SpinClockwise) * self.speed.rotate,
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
        assert_eq!(m.level(Output::PanRight, &[5, 0, 0, 0, 0, 0]), 0.0);
        assert_eq!(m.level(Output::PanRight, &[-10, 0, 0, 0, 0, 0]), 0.0);
        assert!((m.level(Output::PanRight, &[55, 0, 0, 0, 0, 0]) - 0.5).abs() < 1e-6);
        assert_eq!(m.level(Output::PanRight, &[400, 0, 0, 0, 0, 0]), 1.0);
        assert_eq!(m.level(Output::PanRight, &[-400, 0, 0, 0, 0, 0]), -1.0);
    }

    #[test]
    fn invert_and_off() {
        let mut m = Mapping {
            dead_zone: 0.0,
            full_scale: 100.0,
            ..Default::default()
        };
        m.axes[Output::PanUp.index()] = AxisMap::inverted(2);
        m.axes[Output::ZoomIn.index()] = AxisMap::default();
        let nav = m.apply(&[0, 0, 50, 0, 0, 0]);
        assert_eq!(nav.pan, egui::vec2(0.0, -0.5 * m.speed.pan));
        assert_eq!(nav.zoom, 0.0);
        assert_eq!(nav.pan_screen().y, 0.5 * m.speed.pan);
        assert!(m.apply(&[0; 6]).is_zero());
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
