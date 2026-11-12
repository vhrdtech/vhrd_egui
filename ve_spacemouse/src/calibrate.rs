//! The calibrate step: measure the puck, then let the person confirm (SPM-2).
//!
//! The person does one physical motion at a time ("slide the puck to the right, hold it about 2 s, then let go")
//! and presses each button once. [`Calibration`] records the raw events with their times, works out from the data
//! which raw axis and sign each motion was, and which number each button has. Each [`Motion`] drives one
//! [`Output`] with a fixed sign, so the mapping follows from the measurement: by default the content follows the
//! hand (slide right, the page moves right; push away, the page gets smaller). A table shows what the content
//! will then do, with a redo per row. Nothing is guessed from a description.
//!
//! The state machine takes the time from the caller ([`Calibration::feed`]), so tests drive it without a clock.

use crate::event::{AXIS_NAMES, Axes, Event};
use crate::mapping::{AxisMap, Convention, Mapping, Output, Side};
use crate::settings_ui::axes_bars;
use ve_theme::UiExt;

/// A hold shorter than this, in seconds, is asked again: a flick does not show which axis was meant.
const MIN_HOLD_S: f64 = 0.8;
/// The share of `full_scale` that starts a recording.
const START: f32 = 0.15;
/// The share of `full_scale` under which the puck counts as let go.
const REST: f32 = 0.06;
/// The second-largest axis above this share of the largest makes a motion ambiguous.
const MIXED: f32 = 0.5;
/// `full_scale` becomes this share of the typical recorded peak, so full speed does not need the hardest push.
const FULL_OF_PEAK: f32 = 0.8;

/// A physical motion of the puck the calibrate step asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Motion {
    /// Slide it to the right.
    SlideRight,
    /// Lift it up.
    LiftUp,
    /// Push it away from you.
    PushAway,
    /// Tilt it forward, its top away from you.
    TiltForward,
    /// Rock it to the right, its right side down.
    RockRight,
    /// Twist it clockwise, seen from above.
    TwistClockwise,
}

impl Motion {
    /// The motions a 2D view needs: move and zoom.
    pub const TWO_D: [Motion; 3] = [Motion::SlideRight, Motion::LiftUp, Motion::PushAway];
    /// All six.
    pub const ALL: [Motion; 6] = [
        Motion::SlideRight,
        Motion::LiftUp,
        Motion::PushAway,
        Motion::TiltForward,
        Motion::RockRight,
        Motion::TwistClockwise,
    ];

    /// A short name for the table: "slide right".
    pub fn name(self) -> &'static str {
        match self {
            Motion::SlideRight => "slide right",
            Motion::LiftUp => "lift up",
            Motion::PushAway => "push away",
            Motion::TiltForward => "tilt forward",
            Motion::RockRight => "rock right",
            Motion::TwistClockwise => "twist clockwise",
        }
    }

    /// What to tell the person to do.
    pub fn instruction(self) -> &'static str {
        match self {
            Motion::SlideRight => "Slide the puck to the right",
            Motion::LiftUp => "Lift the puck up",
            Motion::PushAway => "Push the puck away from you",
            Motion::TiltForward => "Tilt the puck forward, its top away from you",
            Motion::RockRight => "Rock the puck to the right, its right side down",
            Motion::TwistClockwise => "Twist the puck clockwise",
        }
    }

    /// The output this motion drives with the content following the hand, and the sign: pushing away makes the
    /// page smaller, every other motion moves or turns the page the same way as the hand.
    pub fn drives(self) -> (Output, f32) {
        match self {
            Motion::SlideRight => (Output::MoveRight, 1.0),
            Motion::LiftUp => (Output::MoveUp, 1.0),
            Motion::PushAway => (Output::Grow, -1.0),
            Motion::TiltForward => (Output::TiltForward, 1.0),
            Motion::RockRight => (Output::RollRight, 1.0),
            Motion::TwistClockwise => (Output::TurnClockwise, 1.0),
        }
    }

    /// What the page does on this motion under `convention`: "the page moves right", "the page gets smaller".
    pub fn effect(self, convention: Convention) -> &'static str {
        let (output, sign) = self.drives();
        output.effect(sign * convention.sign() > 0.0)
    }
}

/// One thing the person does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// A motion of the puck, held about 2 s.
    Axis(Motion),
    /// Press and release a button.
    Button(Side),
}

impl Step {
    /// What to tell the person; `convention` says what the motion will do.
    pub fn prompt(&self, convention: Convention) -> String {
        match self {
            Step::Axis(m) => format!(
                "{}. Hold it about 2 s, then let go. In the app {}.",
                m.instruction(),
                m.effect(convention)
            ),
            Step::Button(s) => format!("Press and release the {} button.", s.name()),
        }
    }

    /// A short name for the table: "slide right", "left button".
    pub fn name(&self) -> String {
        match self {
            Step::Axis(m) => m.name().into(),
            Step::Button(s) => format!("{} button", s.name()),
        }
    }
}

/// What a step measured.
#[derive(Clone, Debug, PartialEq)]
pub enum Measured {
    /// The raw axis the motion moved most, its sign, and how clear that was.
    Axis {
        /// The raw axis and the sign the motion read on it (`invert`: it read negative).
        map: AxisMap,
        /// The largest deflection seen on that axis.
        peak: i32,
        /// That axis's share of all the movement recorded, 0 to 1.
        share: f32,
        /// How long the motion was held, in seconds.
        held_s: f64,
    },
    /// The spacenavd number of the button.
    Button(i32),
}

/// A finished step: the measurement and what looks wrong with it.
#[derive(Clone, Debug, PartialEq)]
pub struct StepResult {
    /// What was measured.
    pub measured: Measured,
    /// Problems the person should look at (mixed motion, an axis already used), empty when clean.
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug)]
enum Phase {
    Waiting,
    Holding { start: f64, samples: Vec<Axes> },
}

/// A guided calibration: the steps, what each measured, which one is next.
#[derive(Clone, Debug)]
pub struct Calibration {
    steps: Vec<Step>,
    results: Vec<Option<StepResult>>,
    current: Option<usize>,
    phase: Phase,
    note: Option<String>,
    live: Axes,
    base: Mapping,
}

/// What the person chose in [`calibration_ui`].
#[derive(Clone, Debug, PartialEq)]
pub enum CalibrationAction {
    /// Still going.
    None,
    /// Use this mapping (the app saves it in its settings).
    Apply(Mapping),
    /// Leave the mapping as it was.
    Cancel,
}

impl Calibration {
    /// These steps, starting from `base` (speeds, dead zone and outputs not measured are kept from it).
    pub fn new(steps: Vec<Step>, base: Mapping) -> Self {
        let n = steps.len();
        Self {
            steps,
            results: vec![None; n],
            current: if n > 0 { Some(0) } else { None },
            phase: Phase::Waiting,
            note: None,
            live: [0; 6],
            base,
        }
    }

    /// Slide right, lift up, push away, then the left and the right button: what a 2D view needs.
    pub fn two_d(base: Mapping) -> Self {
        let mut steps: Vec<Step> = Motion::TWO_D.into_iter().map(Step::Axis).collect();
        steps.extend([Step::Button(Side::Left), Step::Button(Side::Right)]);
        Self::new(steps, base)
    }

    /// All six motions and both buttons: for a view that also turns (or a 3D one).
    pub fn full(base: Mapping) -> Self {
        let mut steps: Vec<Step> = Motion::ALL.into_iter().map(Step::Axis).collect();
        steps.extend([Step::Button(Side::Left), Step::Button(Side::Right)]);
        Self::new(steps, base)
    }

    /// The steps, in order.
    pub fn steps(&self) -> &[Step] {
        &self.steps
    }

    /// What each step measured so far.
    pub fn results(&self) -> &[Option<StepResult>] {
        &self.results
    }

    /// The step waiting for the person, `None` when all are done.
    pub fn current(&self) -> Option<(usize, Step)> {
        self.current.map(|i| (i, self.steps[i]))
    }

    /// Whether a motion is being recorded now.
    pub fn recording(&self) -> bool {
        matches!(self.phase, Phase::Holding { .. })
    }

    /// Every step has a result.
    pub fn done(&self) -> bool {
        self.current.is_none()
    }

    /// A message about the last attempt ("that was short ..."), cleared by the next good one.
    pub fn note(&self) -> Option<&str> {
        self.note.as_deref()
    }

    /// The puck's deflection at the last event.
    pub fn live(&self) -> Axes {
        self.live
    }

    /// Ask step `i` again.
    pub fn redo(&mut self, i: usize) {
        if i < self.steps.len() {
            self.results[i] = None;
            self.current = Some(i);
            self.phase = Phase::Waiting;
            self.note = None;
            self.recheck();
        }
    }

    /// Start over with every step.
    pub fn restart(&mut self) {
        *self = Self::new(self.steps.clone(), self.base.clone());
    }

    /// Feed one event from spacenavd, received at `now` seconds (any steady clock, e.g. egui's `input.time`).
    pub fn feed(&mut self, event: &Event, now: f64) {
        if let Event::Motion { axes, .. } = event {
            self.live = *axes;
        }
        let Some(i) = self.current else { return };
        match (self.steps[i], event) {
            (Step::Axis(_), Event::Motion { axes, .. }) => self.motion(i, *axes, now),
            (
                Step::Button(side),
                Event::Button {
                    number,
                    pressed: true,
                },
            ) => {
                let taken = self
                    .steps
                    .iter()
                    .zip(&self.results)
                    .find_map(|(s, r)| match (s, r) {
                        (
                            Step::Button(other),
                            Some(StepResult {
                                measured: Measured::Button(n),
                                ..
                            }),
                        ) if n == number && *other != side => Some(*other),
                        _ => None,
                    });
                if let Some(other) = taken {
                    self.note = Some(format!(
                        "Button {number} is already the {} button: press the {} one.",
                        other.name(),
                        side.name()
                    ));
                    return;
                }
                self.finish(
                    i,
                    StepResult {
                        measured: Measured::Button(*number),
                        warnings: vec![],
                    },
                );
            }
            _ => {}
        }
    }

    fn motion(&mut self, i: usize, axes: Axes, now: f64) {
        let full = self.base.full_scale.max(1.0);
        let biggest = axes.iter().map(|v| v.unsigned_abs()).max().unwrap_or(0) as f32 / full;
        match &mut self.phase {
            Phase::Waiting => {
                if biggest >= START {
                    self.phase = Phase::Holding {
                        start: now,
                        samples: vec![axes],
                    };
                }
            }
            Phase::Holding { start, samples } => {
                if biggest > REST {
                    samples.push(axes);
                    return;
                }
                let held_s = now - *start;
                let samples = std::mem::take(samples);
                self.phase = Phase::Waiting;
                if held_s < MIN_HOLD_S {
                    self.note = Some(format!(
                        "That was {held_s:.1} s: hold the motion about 2 s, then let go."
                    ));
                    return;
                }
                let result = analyse(&samples, held_s);
                self.finish(i, result);
            }
        }
    }

    fn finish(&mut self, i: usize, result: StepResult) {
        self.results[i] = Some(result);
        self.note = None;
        self.recheck();
        let n = self.steps.len();
        self.current = (1..=n)
            .map(|k| (i + k) % n)
            .find(|&k| self.results[k].is_none());
    }

    /// Recompute the warnings that compare steps (an axis used twice).
    fn recheck(&mut self) {
        let raws: Vec<Option<usize>> = self
            .results
            .iter()
            .map(|r| match r {
                Some(StepResult {
                    measured: Measured::Axis { map, .. },
                    ..
                }) => map.raw,
                _ => None,
            })
            .collect();
        for (i, r) in self.results.iter_mut().enumerate() {
            let Some(r) = r else { continue };
            r.warnings.retain(|w| !w.starts_with("Same axis as"));
            let Some(raw) = raws[i] else { continue };
            for (j, other) in raws.iter().enumerate() {
                if j != i && *other == Some(raw) {
                    r.warnings.push(format!(
                        "Same axis as {}: redo one of them, moving only that way.",
                        self.steps[j].name()
                    ));
                }
            }
        }
    }

    /// The convention the table shows and the mapping gets (the base's at the start).
    pub fn convention(&self) -> Convention {
        self.base.convention
    }

    /// Choose content or camera; the table and the derived mapping follow.
    pub fn set_convention(&mut self, convention: Convention) {
        self.base.convention = convention;
    }

    /// The mapping from the measurements: the base with every measured axis, button and the full scale.
    ///
    /// Each motion drives its output ([`Motion::drives`]) with the content following the hand: the raw axis it
    /// moved, signed so that the motion gives the output's sign. The convention is kept from the base and applied
    /// by [`Mapping::apply`].
    pub fn mapping(&self) -> Mapping {
        let mut m = self.base.clone();
        let mut peaks = Vec::new();
        for (step, r) in self.steps.iter().zip(&self.results) {
            match (step, r.as_ref().map(|r| &r.measured)) {
                (Step::Axis(motion), Some(Measured::Axis { map, peak, .. })) => {
                    let (output, sign) = motion.drives();
                    m.axes[output.index()] = AxisMap {
                        raw: map.raw,
                        invert: map.invert != (sign < 0.0),
                    };
                    peaks.push(*peak);
                }
                (Step::Button(Side::Left), Some(Measured::Button(n))) => m.buttons.left = *n,
                (Step::Button(Side::Right), Some(Measured::Button(n))) => m.buttons.right = *n,
                _ => {}
            }
        }
        if !peaks.is_empty() {
            peaks.sort_unstable();
            let median = peaks[peaks.len() / 2] as f32;
            m.full_scale = (median * FULL_OF_PEAK).max(1.0);
        }
        m
    }
}

/// Which axis a held motion moved most, and how clearly.
fn analyse(samples: &[Axes], held_s: f64) -> StepResult {
    let mut abs = [0f64; 6];
    let mut signed = [0f64; 6];
    let mut peak = [0i32; 6];
    for s in samples {
        for k in 0..6 {
            abs[k] += f64::from(s[k].unsigned_abs());
            signed[k] += f64::from(s[k]);
            peak[k] = peak[k].max(s[k].saturating_abs());
        }
    }
    let mut order: Vec<usize> = (0..6).collect();
    order.sort_by(|&a, &b| abs[b].total_cmp(&abs[a]));
    let (top, second) = (order[0], order[1]);
    let total: f64 = abs.iter().sum();
    let share = if total > 0.0 {
        (abs[top] / total) as f32
    } else {
        0.0
    };
    let mut warnings = Vec::new();
    if abs[top] > 0.0 && abs[second] / abs[top] > f64::from(MIXED) {
        warnings.push(format!(
            "Mixed motion: {} moved {:.0} % as much as {}. Redo it, moving only that way.",
            AXIS_NAMES[second],
            100.0 * abs[second] / abs[top],
            AXIS_NAMES[top]
        ));
    }
    StepResult {
        measured: Measured::Axis {
            map: AxisMap {
                raw: Some(top),
                invert: signed[top] < 0.0,
            },
            peak: peak[top],
            share,
            held_s,
        },
        warnings,
    }
}

/// The calibration as a panel: the prompt, the live axes, the steps so far, and once all are done the table to
/// confirm. Feed events with [`Calibration::feed`] before calling it.
pub fn calibration_ui(ui: &mut egui::Ui, cal: &mut Calibration) -> CalibrationAction {
    let t = ui.tokens();
    let mut action = CalibrationAction::None;
    if let Some((i, step)) = cal.current() {
        ui.label(
            egui::RichText::new(format!("Step {} of {}", i + 1, cal.steps().len()))
                .color(t.colors.text_muted),
        );
        ui.label(egui::RichText::new(step.prompt(cal.convention())).strong());
        let state = if cal.recording() {
            "recording: let go when done"
        } else {
            "waiting for the motion"
        };
        if matches!(step, Step::Axis(_)) {
            ui.label(egui::RichText::new(state).color(t.colors.text_muted));
        }
    } else {
        ui.label(
            egui::RichText::new("All steps done. Check what the puck will do, then use it.")
                .strong(),
        );
    }
    if let Some(note) = cal.note() {
        ui.colored_label(t.colors.warn, note);
    }
    ui.add_space(t.space.s);
    axes_bars(ui, &cal.live(), cal.base.full_scale);
    ui.add_space(t.space.s);

    let mut convention = cal.convention();
    ui.horizontal(|ui| {
        ui.label("The puck moves")
            .on_hover_text("Whose motion the puck is; the table below and the mapping follow");
        ui.radio_value(&mut convention, Convention::Content, "the content")
            .on_hover_text(
                "The content follows the hand: slide right and the page moves right, push away and it gets smaller, \
                 twist clockwise and it turns clockwise (KiCad with reverse pan, FreeCAD's object mode)",
            );
        ui.radio_value(&mut convention, Convention::Camera, "the camera")
            .on_hover_text(
                "The puck is the camera: slide right and the view moves right, so the page moves left; push away \
                 and the camera goes in, so the page gets bigger",
            );
    });
    cal.set_convention(convention);
    ui.add_space(t.space.s);

    let mapping = cal.mapping();
    let mut redo = None;
    egui::Grid::new("ve_spacemouse_calibration").striped(true).show(ui, |ui| {
        ui.label("Step").on_hover_text("What you were asked to do");
        ui.label("Measured").on_hover_text("The raw axis and sign the motion moved most, or the button number");
        ui.label("In the app").on_hover_text("What the page does from now on when you do this");
        ui.label("");
        ui.end_row();
        for (i, step) in cal.steps().iter().enumerate() {
            ui.label(step.name());
            match &cal.results()[i] {
                Some(r) => {
                    let (measured, tip) = match &r.measured {
                        Measured::Axis { map, peak, share, held_s } => (
                            format!("axis {}", map.describe()),
                            format!(
                                "peak {peak}, {:.0} % of all movement on this axis, held {held_s:.1} s",
                                share * 100.0
                            ),
                        ),
                        Measured::Button(n) => (format!("button {n}"), "spacenavd's number for it".to_string()),
                    };
                    ui.label(measured).on_hover_text(tip);
                    let effect = match step {
                        Step::Axis(m) => m.effect(convention).to_string(),
                        Step::Button(s) => format!("opens the {} pie menu", s.name()),
                    };
                    if r.warnings.is_empty() {
                        ui.label(effect);
                    } else {
                        ui.colored_label(t.colors.warn, format!("⚠ {effect}"))
                            .on_hover_text(r.warnings.join("\n"));
                    }
                }
                None if cal.current().map(|c| c.0) == Some(i) => {
                    ui.colored_label(t.colors.accent, "now");
                    ui.label("");
                }
                None => {
                    ui.colored_label(t.colors.text_muted, "to do");
                    ui.label("");
                }
            }
            let has = cal.results()[i].is_some();
            if ui
                .add_enabled(has, egui::Button::new("Redo"))
                .on_hover_text("Ask this step again")
                .on_disabled_hover_text("Not measured yet: do the step first")
                .clicked()
            {
                redo = Some(i);
            }
            ui.end_row();
        }
    });
    if let Some(i) = redo {
        cal.redo(i);
    }
    ui.label(
        egui::RichText::new(format!(
            "A full push will be a raw value of {:.0} (80 % of the typical peak above)",
            mapping.full_scale
        ))
        .color(t.colors.text_muted),
    );
    ui.add_space(t.space.s);
    ui.horizontal(|ui| {
        let warnings = cal
            .results()
            .iter()
            .flatten()
            .any(|r| !r.warnings.is_empty());
        let why = if !cal.done() {
            Some("Not every step is done yet".to_string())
        } else if warnings {
            Some("A step has a warning (⚠): redo it first".to_string())
        } else {
            None
        };
        let mut use_it = ui
            .add_enabled_ui(why.is_none(), |ui| {
                ui.primary_button(
                    "Use this mapping",
                    "Save the measured axes, signs, buttons, full scale and the convention in this app's settings",
                )
            })
            .inner;
        if let Some(why) = &why {
            use_it = use_it.on_disabled_hover_text(why);
        }
        if use_it.clicked() {
            action = CalibrationAction::Apply(mapping.clone());
        }
        if ui
            .button("Start over")
            .on_hover_text("Forget every measurement and begin at step 1")
            .clicked()
        {
            cal.restart();
        }
        if ui
            .button("Cancel")
            .on_hover_text("Keep the mapping as it was")
            .clicked()
        {
            action = CalibrationAction::Cancel;
        }
    });
    action
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hold `axes` from `t` for `secs`, a sample every 0.1 s, then let go.
    fn hold(c: &mut Calibration, t: &mut f64, axes: Axes, secs: f64) {
        let n = (secs / 0.1).round() as usize;
        for _ in 0..=n {
            c.feed(&Event::motion(axes), *t);
            *t += 0.1;
        }
        c.feed(&Event::motion([0; 6]), *t);
        *t += 1.0;
    }

    fn press(c: &mut Calibration, n: i32) {
        c.feed(&Event::button(n, true), 0.0);
        c.feed(&Event::button(n, false), 0.0);
    }

    #[test]
    fn two_d_measures_axes_signs_and_buttons() {
        let mut c = Calibration::two_d(Mapping::default());
        let mut t = 0.0;
        // slide right reads +x, lift up reads -z, push away reads +y
        hold(&mut c, &mut t, [300, 10, -20, 5, 0, 0], 2.0);
        hold(&mut c, &mut t, [10, 15, -280, 0, 0, 0], 2.0);
        hold(&mut c, &mut t, [0, 320, 30, 0, 0, 0], 2.0);
        press(&mut c, 12);
        press(&mut c, 13);
        assert!(c.done());
        assert!(c.results().iter().flatten().all(|r| r.warnings.is_empty()));
        let m = c.mapping();
        assert_eq!(m.axis(Output::MoveRight), AxisMap::new(0));
        assert_eq!(m.axis(Output::MoveUp), AxisMap::inverted(2));
        // pushing away makes the page smaller: +y is shrink, so grow is -y
        assert_eq!(m.axis(Output::Grow), AxisMap::inverted(1));
        assert_eq!((m.buttons.left, m.buttons.right), (12, 13));
        assert_eq!(m.full_scale, 300.0 * FULL_OF_PEAK);
        // not measured: kept from the base
        assert_eq!(
            m.axis(Output::TurnClockwise),
            Mapping::default().axis(Output::TurnClockwise)
        );
    }

    #[test]
    fn a_short_flick_is_asked_again() {
        let mut c = Calibration::two_d(Mapping::default());
        let mut t = 0.0;
        hold(&mut c, &mut t, [300, 0, 0, 0, 0, 0], 0.3);
        assert_eq!(c.current().map(|c| c.0), Some(0));
        assert!(c.note().is_some_and(|n| n.contains("hold the motion")));
        hold(&mut c, &mut t, [300, 0, 0, 0, 0, 0], 2.0);
        assert_eq!(c.current().map(|c| c.0), Some(1));
        assert_eq!(c.note(), None);
    }

    #[test]
    fn small_noise_does_not_start_a_recording() {
        let mut c = Calibration::two_d(Mapping::default());
        c.feed(&Event::motion([20, -10, 5, 0, 0, 0]), 0.0);
        assert!(!c.recording());
        c.feed(&Event::motion([200, 0, 0, 0, 0, 0]), 0.1);
        assert!(c.recording());
    }

    #[test]
    fn mixed_motion_and_a_reused_axis_warn_and_redo_clears() {
        let mut c = Calibration::two_d(Mapping::default());
        let mut t = 0.0;
        hold(&mut c, &mut t, [300, 200, 0, 0, 0, 0], 2.0);
        let w = &c.results()[0].as_ref().unwrap().warnings;
        assert!(w.iter().any(|w| w.starts_with("Mixed motion: y")), "{w:?}");
        hold(&mut c, &mut t, [280, 0, 0, 0, 0, 0], 2.0);
        assert!(
            c.results()[1]
                .as_ref()
                .unwrap()
                .warnings
                .iter()
                .any(|w| w.contains("Same axis as slide right"))
        );
        c.redo(1);
        hold(&mut c, &mut t, [0, 0, -280, 0, 0, 0], 2.0);
        assert!(
            c.results()[0]
                .as_ref()
                .unwrap()
                .warnings
                .iter()
                .all(|w| !w.starts_with("Same axis"))
        );
        // after the redo it goes on with the first step not done
        assert_eq!(c.current().map(|c| c.0), Some(2));
    }

    #[test]
    fn a_button_cannot_be_both_sides() {
        let mut c = Calibration::new(
            vec![Step::Button(Side::Left), Step::Button(Side::Right)],
            Mapping::default(),
        );
        press(&mut c, 4);
        press(&mut c, 4);
        assert_eq!(c.current().map(|c| c.0), Some(1));
        assert!(c.note().is_some_and(|n| n.contains("already the left")));
        press(&mut c, 5);
        assert!(c.done());
    }
}
