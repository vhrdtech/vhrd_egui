//! Numbers held at a readable rate while the data underneath stays per frame.

use std::ops::RangeInclusive;
use std::time::Duration;

use super::FrameRecorder;

/// The range [`ReadoutSettings::rate_hz`] is kept in (setting S15).
pub const READOUT_RATE_HZ: RangeInclusive<f32> = 2.0..=10.0;

/// How shown numbers refresh: settings S15 and S16.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReadoutSettings {
    /// Ticks per second (S15, 2 to 10 Hz); each shows the median of the tick before it.
    pub rate_hz: f32,
    /// Worst is the maximum of this last span, so a spike stays this long (S16).
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

/// The numbers a readout shows until its next tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Shown {
    /// Median CPU time per frame over the last tick.
    pub cpu: Duration,
    /// Median interval between continuous frames over the last tick; zero when none was continuous.
    pub interval: Duration,
    /// Worst CPU time over the hold span.
    pub worst_cpu: Duration,
    /// Worst interval over the hold span.
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

/// Holds the shown numbers between ticks: call [`Self::update`] every frame, draw [`Self::shown`].
#[derive(Clone, Debug, Default)]
pub struct Readout {
    settings: ReadoutSettings,
    shown: Shown,
    next_tick_ns: Option<u64>,
}

impl Readout {
    /// A readout with no numbers yet.
    pub fn new(settings: ReadoutSettings) -> Self {
        Self {
            settings,
            ..Self::default()
        }
    }

    /// Its settings.
    pub fn settings(&self) -> ReadoutSettings {
        self.settings
    }

    /// Change its settings from the next tick on.
    pub fn set_settings(&mut self, settings: ReadoutSettings) {
        self.settings = settings;
        self.next_tick_ns = None;
    }

    /// The numbers to show now.
    pub fn shown(&self) -> &Shown {
        &self.shown
    }

    /// When the next tick is due, on the recorder's clock: a host that is idle otherwise may ask for one repaint
    /// then.
    pub fn next_tick_ns(&self) -> Option<u64> {
        self.next_tick_ns
    }

    /// Take new numbers from `rec` when a tick is due; `true` when they were taken.
    pub fn update(&mut self, rec: &dyn FrameRecorder) -> bool {
        let now = rec.now_ns();
        if self.next_tick_ns.is_some_and(|t| now < t) {
            return false;
        }
        let rate = self
            .settings
            .rate_hz
            .clamp(*READOUT_RATE_HZ.start(), *READOUT_RATE_HZ.end());
        let tick = Duration::from_secs_f32(1.0 / rate);
        let s = rec.stats(tick);
        let held = rec.stats(self.settings.worst_hold);
        if !s.cpu.is_empty() {
            self.shown.cpu = s.cpu.median;
        }
        if !s.interval.is_empty() {
            self.shown.interval = s.interval.median;
        }
        if held.frames > 0 {
            self.shown.worst_cpu = held.cpu.worst;
            self.shown.worst_interval = held.interval.worst;
        }
        self.shown.fps = held.fps;
        self.shown.dropped = held.dropped;
        self.shown.budget = held.budget;
        self.shown.idle = held.idle;
        self.next_tick_ns = Some(now + u64::try_from(tick.as_nanos()).unwrap_or(u64::MAX));
        true
    }
}
