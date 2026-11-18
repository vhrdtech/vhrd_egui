//! Numbers held at a readable rate while the data underneath stays per frame.

use std::ops::RangeInclusive;
use std::time::Duration;

use super::{FrameRecorder, SettingsError, Span};

/// The range [`ReadoutSettings::rate_hz`] is kept in (setting S15).
pub const READOUT_RATE_HZ: RangeInclusive<f32> = 2.0..=10.0;

/// The longest [`ReadoutSettings::worst_hold`].
const WORST_HOLD_MAX: Duration = Duration::from_secs(60);

/// How shown numbers refresh: settings S15 and S16.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReadoutSettings {
    /// Ticks per second (S15, within [`READOUT_RATE_HZ`]); each shows the median of the tick before it.
    pub rate_hz: f32,
    /// Worst is the maximum of this last span, and once shown it stays at least this long (S16).
    pub worst_hold: Duration,
}

impl Default for ReadoutSettings {
    fn default() -> Self {
        Self {
            rate_hz: 5.0,
            worst_hold: Duration::from_secs(1),
        }
    }
}

impl ReadoutSettings {
    /// `Ok` when both settings are in their range.
    pub fn validate(&self) -> Result<(), SettingsError> {
        if !READOUT_RATE_HZ.contains(&self.rate_hz) {
            return Err(SettingsError::ReadoutRateHz(self.rate_hz));
        }
        if self.worst_hold.is_zero() || self.worst_hold > WORST_HOLD_MAX {
            return Err(SettingsError::WorstHold(self.worst_hold));
        }
        Ok(())
    }
}

/// The numbers a readout shows until its next tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Shown {
    /// Median CPU time per frame over the last tick; the last one seen when the tick had no frame.
    pub cpu: Duration,
    /// Median interval between continuous frames over the last tick; zero when none was continuous.
    pub interval: Duration,
    /// Worst CPU time: the maximum of the hold span, held at least that long.
    pub worst_cpu: Duration,
    /// Worst interval, held the same way.
    pub worst_interval: Duration,
    /// Frames per second over the hold span.
    pub fps: f32,
    /// Frames dropped over the hold span.
    pub dropped: u32,
    /// The budget frames are judged against.
    pub budget: Duration,
    /// Time since the last frame, once past [`super::IDLE_AFTER`]; the other numbers are then the last ones seen.
    pub idle: Option<Duration>,
}

/// A worst value and when it was taken.
#[derive(Clone, Copy, Debug, Default)]
struct Held {
    value: Duration,
    at_ns: u64,
}

impl Held {
    /// Take `candidate` when it is larger, or when the held value has been shown for `hold`.
    fn update(&mut self, candidate: Duration, now: u64, hold: Duration) {
        let age = Duration::from_nanos(now.saturating_sub(self.at_ns));
        if candidate >= self.value || age >= hold {
            *self = Self {
                value: candidate,
                at_ns: now,
            };
        }
    }
}

/// Holds the shown numbers between ticks: call [`Self::update`] every frame, before the frame begins (so a frame
/// after a pause first shows the idle time), and draw [`Self::shown`].
#[derive(Clone, Debug, Default)]
pub struct Readout {
    settings: ReadoutSettings,
    shown: Shown,
    worst_cpu: Held,
    worst_interval: Held,
    next_tick_ns: Option<u64>,
}

impl Readout {
    /// A readout with no numbers yet; `Err` for settings out of range.
    pub fn new(settings: ReadoutSettings) -> Result<Self, SettingsError> {
        settings.validate()?;
        Ok(Self {
            settings,
            ..Self::default()
        })
    }

    /// Its settings.
    pub fn settings(&self) -> ReadoutSettings {
        self.settings
    }

    /// Change its settings from the next tick on; out-of-range settings are refused and the old ones stay.
    pub fn set_settings(&mut self, settings: ReadoutSettings) -> Result<(), SettingsError> {
        settings.validate()?;
        self.settings = settings;
        self.next_tick_ns = None;
        Ok(())
    }

    /// The numbers to show now.
    pub fn shown(&self) -> &Shown {
        &self.shown
    }

    /// When the next tick is due, on the recorder's clock.
    pub fn next_tick_ns(&self) -> Option<u64> {
        self.next_tick_ns
    }

    /// Take new numbers from `rec` when a tick is due; `true` when they were taken.
    pub fn update(&mut self, rec: &dyn FrameRecorder) -> bool {
        let now = rec.now_ns();
        if self.next_tick_ns.is_some_and(|t| now < t) {
            return false;
        }
        let hold = self.settings.worst_hold;
        let tick = Duration::from_secs_f32(1.0 / self.settings.rate_hz);
        let s = rec.stats(Span::last(now, tick));
        let held = rec.stats(Span::last(now, hold));
        if !s.cpu.is_empty() {
            self.shown.cpu = s.cpu.median;
        }
        self.shown.interval = s.interval.median;
        if held.frames > 0 {
            self.worst_cpu.update(held.cpu.worst, now, hold);
            self.worst_interval.update(held.interval.worst, now, hold);
            self.shown.worst_cpu = self.worst_cpu.value;
            self.shown.worst_interval = self.worst_interval.value;
        }
        self.shown.fps = held.fps;
        self.shown.dropped = held.dropped;
        self.shown.budget = held.budget;
        self.shown.idle = held.idle;
        self.next_tick_ns = Some(now + u64::try_from(tick.as_nanos()).unwrap_or(u64::MAX));
        true
    }
}
