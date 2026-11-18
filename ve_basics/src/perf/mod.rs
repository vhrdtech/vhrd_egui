//! Frame recording for a performance panel (BAS-10): frame times, named scopes, counters, repaint causes.
//!
//! A host (`ve_app::Shell`, or any eframe app) marks each frame's begin and end on a [`FrameRecorder`]; widgets
//! and app code mark named [`scopes`](Recorder::scope) inside it. The panel reads [`Stats`] (last, median, p95,
//! p99, worst over a window, fps, dropped frames, idle), the [`Breakdown`] by scope with the rest as "egui and
//! OS", the repaint causes and the series for its charts ([`FrameSeries`]).
//!
//! ```
//! use std::time::Duration;
//! use ve_basics::perf::{FrameBegin, FrameEnd, Recorder, RingRecorder, Settings};
//!
//! let rec = Recorder::new(RingRecorder::new(Settings::default()));
//! rec.frame_begin(FrameBegin { continuous: true });
//! {
//!     let _g = rec.scope("tracks"); // one widget's ui; scopes nest
//!     rec.count("ws.rx_bytes", 1500);
//! }
//! rec.frame_end(&FrameEnd::default());
//! let s = rec.stats(Duration::from_secs(5));
//! assert_eq!(s.frames, 1);
//! ```
//!
//! [`RingRecorder`] is the default: fixed rings sized from the retention, allocated once when recording turns on
//! and freed when it turns off, so a frame costs two clock reads and two short lock holds; off, it costs one
//! atomic load. Another recorder (one writing into an eventfull track, say) implements the same trait and the
//! panel reads it unchanged. Time comes from a [`Clock`], so tests drive it by hand.
//!
//! Scopes and counters are meant for the thread that runs the frames: a scope begun outside a frame is not
//! recorded.

mod clock;
mod hist;
mod mem;
mod readout;
mod ring;

#[cfg(feature = "alloc-count")]
mod alloc;
#[cfg(test)]
mod tests;

use std::sync::Arc;
use std::time::Duration;

#[cfg(feature = "alloc-count")]
pub use alloc::{CountingAlloc, alloc_totals};
#[cfg(any(test, feature = "test-support"))]
pub use clock::ManualClock;
pub use clock::{Clock, SystemClock};
pub use hist::{HIST_BINS, HIST_BINS_PER_OCTAVE, HIST_FLOOR_NS, Histogram};
pub use mem::rss_bytes;
pub use readout::{READOUT_RATE_HZ, Readout, ReadoutSettings, Shown};
pub use ring::{
    CAUSES_MAX, ENTRIES_PER_RECORD_MAX, FRAMES_PER_RECORD_MAX, NAMES_MAX, RECORDS_MAX,
    RingRecorder, SCOPE_DEPTH_MAX, Sizing,
};

/// After this long without a frame the numbers say "idle" instead of showing stale values.
pub const IDLE_AFTER: Duration = Duration::from_secs(1);

/// The display refresh rates a measured one snaps to when within 3 %.
pub const KNOWN_REFRESH_HZ: [f32; 9] = [30.0, 50.0, 60.0, 75.0, 90.0, 100.0, 120.0, 144.0, 165.0];

/// The frame budget when neither the display nor a measurement gives one: 60 Hz.
pub const BUDGET_FALLBACK_HZ: f32 = 60.0;

/// What the host knows at a frame's begin.
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameBegin {
    /// The previous frame asked for this one at once (an animation, a stream), so the interval to it measures
    /// smoothness and a long one counts dropped frames. egui: `ctx.requested_repaint_last_pass()`.
    pub continuous: bool,
}

/// What the host knows at a frame's end.
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameEnd<'a> {
    /// Why this frame was painted (egui's `Context::repaint_causes`, input, ...).
    pub causes: &'a [Cause<'a>],
}

/// One reason a frame was painted: a call site that asked for it, or a named cause such as input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cause<'a> {
    /// Source file of the call that asked for the repaint; empty for a named cause.
    pub file: &'static str,
    /// Line of that call; 0 for a named cause.
    pub line: u32,
    /// What it says about itself; may be empty for a call site.
    pub reason: &'a str,
}

impl Cause<'static> {
    /// A cause with only a name (input, a resize).
    pub const fn named(reason: &'static str) -> Self {
        Self {
            file: "",
            line: 0,
            reason,
        }
    }
}

/// How a scope's time counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ScopeKind {
    /// Part of the frame's work, shown in the breakdown.
    Counted,
    /// Not the app's work: the performance panel's own ui. Its time is taken out of the frame's CPU time and every
    /// statistic; it still shows as a row of its own.
    NotCounted,
}

/// What a recorder hands out at a scope's begin and wants back at its end. The fields are the recorder's own;
/// [`ScopeGuard`] carries it between the two calls.
#[derive(Clone, Copy, Debug)]
pub struct ScopeToken {
    /// When the scope began, on the recorder's clock.
    pub start_ns: u64,
    /// The recorder's id for the scope's name.
    pub id: u16,
    /// The recorder's id for the enclosing scope.
    pub parent: u16,
    /// How many scopes were open around it.
    pub depth: u16,
    /// Recorder-defined flags.
    pub flags: u16,
}

/// Where the frame budget comes from (setting S19).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum BudgetSource {
    /// The display's refresh as the host reports it, else as measured from continuous frames.
    #[default]
    Display,
    /// A fixed refresh rate in Hz.
    Hz(f32),
}

/// How finely frames are kept (setting S27).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Granularity {
    /// Every frame on its own: exact statistics.
    #[default]
    EveryFrame,
    /// One aggregate per span of this length (10 ms, 100 ms): less memory for long retention; a statistic
    /// then sees each aggregate's mean and its worst frame.
    Every(Duration),
}

/// What a recorder keeps and how it judges frames: settings S19, S26 and S27.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    /// How far back history goes (S26).
    pub retention: Duration,
    /// How finely it is kept (S27).
    pub granularity: Granularity,
    /// Where the budget comes from (S19).
    pub budget: BudgetSource,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            retention: Duration::from_secs(300),
            granularity: Granularity::EveryFrame,
            budget: BudgetSource::Display,
        }
    }
}

/// A distribution's summary over a window. Median and percentiles come from a [`Histogram`] (within 1.1 %);
/// last and worst are exact.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Summary {
    /// How many frames it covers.
    pub count: u32,
    /// The newest frame's value.
    pub last: Duration,
    /// The arithmetic mean.
    pub mean: Duration,
    /// The median.
    pub median: Duration,
    /// The 95th percentile.
    pub p95: Duration,
    /// The 99th percentile.
    pub p99: Duration,
    /// The largest value.
    pub worst: Duration,
}

/// Frame statistics over a window ending now.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stats {
    /// The window asked for.
    pub window: Duration,
    /// Frames in it.
    pub frames: u32,
    /// Frames per second over the window, or over the recorded part when recording began inside it.
    pub fps: f32,
    /// Start-to-start intervals of continuous frames: what the eye sees as smoothness.
    pub interval: Summary,
    /// CPU time per frame, without the time of scopes that do not count.
    pub cpu: Summary,
    /// Frames missed: an interval of k budgets counts k - 1, only between continuous frames.
    pub dropped: u32,
    /// The budget frames are judged against.
    pub budget: Duration,
    /// The refresh rate the budget comes from.
    pub refresh_hz: f32,
    /// How long since the last frame, when that is past [`IDLE_AFTER`].
    pub idle: Option<Duration>,
}

/// One scope's statistics over a window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScopeStats {
    /// The scope's name.
    pub name: &'static str,
    /// How its time counts.
    pub kind: ScopeKind,
    /// Its time per frame it ran in, inner scopes included.
    pub time: Summary,
    /// Its time over the window, inner scopes included.
    pub total: Duration,
    /// Its time over the window without its inner scopes.
    pub self_total: Duration,
    /// Its share of the window's counted CPU time (0..1); 0 for scopes that do not count.
    pub share: f32,
    /// Whether it ran outside any other scope.
    pub top_level: bool,
}

/// Where the CPU time of a window went: the scopes, and the rest ("egui and OS").
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Breakdown {
    /// Every scope seen in the window, by total time, largest first.
    pub scopes: Vec<ScopeStats>,
    /// Per frame: CPU time outside every top-level counted scope.
    pub rest: Summary,
    /// The window's CPU time outside every top-level counted scope.
    pub rest_total: Duration,
}

/// A repaint cause and how many frames it caused in a window.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CauseCount {
    /// `reason` when there is one, else `file:line`.
    pub label: String,
    /// Frames (aggregates, at a coarse granularity) it caused.
    pub frames: u32,
}

/// An app counter over a window ([`FrameRecorder::count`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CounterStats {
    /// The counter's name.
    pub name: &'static str,
    /// Its sum over the window.
    pub total: u64,
    /// The most counted in one frame (one aggregate at a coarse granularity).
    pub worst: u64,
    /// Frames it was counted in.
    pub frames: u32,
}

/// A gauge's latest value ([`FrameRecorder::gauge`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GaugeValue {
    /// The gauge's name.
    pub name: &'static str,
    /// Its latest value.
    pub value: u64,
    /// When it was set, on the recorder's clock.
    pub at_ns: u64,
}

/// Which series a chart reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Series {
    /// CPU time per frame.
    Cpu,
    /// Interval between continuous frames.
    Interval,
    /// One scope's time per frame, by name.
    Scope(&'static str),
    /// CPU time outside every top-level counted scope.
    Rest,
}

/// One column of a chart: the frames whose begin falls into it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Bin {
    /// Frames in it with a value.
    pub count: u32,
    /// The smallest value.
    pub min: Duration,
    /// The largest value.
    pub max: Duration,
    /// The mean.
    pub mean: Duration,
}

/// What a chart reads: values binned into columns over a time span. The seam for other chart backends.
pub trait FrameSeries {
    /// Fill `out` with `out.len()` equal columns from `t0_ns` to `t1_ns` on the recorder's clock. Columns
    /// without a frame get `count == 0`.
    fn bins(&self, series: Series, t0_ns: u64, t1_ns: u64, out: &mut [Bin]);
}

/// Records frames, scopes and counters, and answers the panel's questions about them.
///
/// Implementations are cheap per frame and bounded in memory; reads may walk their history and are meant for a
/// few times a second.
pub trait FrameRecorder: FrameSeries + Send + Sync {
    /// Whether it records (setting S18). Off, every recording call is one atomic load.
    fn is_enabled(&self) -> bool;
    /// Turn recording on or off. Off drops the history and its memory; on starts an empty one.
    fn set_enabled(&self, on: bool);
    /// Its settings.
    fn settings(&self) -> Settings;
    /// Change the settings; a new retention or granularity starts an empty history.
    fn set_settings(&self, settings: Settings);
    /// Now on its clock, in nanoseconds since an origin of its own.
    fn now_ns(&self) -> u64;
    /// Forget the history, keep recording.
    fn clear(&self);

    /// A frame begins.
    fn frame_begin(&self, begin: FrameBegin);
    /// The host's own measure of the whole previous frame (eframe's `cpu_usage`: ui, tessellation, paint),
    /// replacing what begin and end saw of it. Call after [`Self::frame_begin`].
    fn previous_frame_cpu(&self, cpu: Duration);
    /// The frame's own work ends.
    fn frame_end(&self, end: &FrameEnd<'_>);
    /// The display's refresh rate as the host reads it; `None` to measure it from continuous frames.
    fn set_display_hz(&self, hz: Option<f32>);

    /// A named scope begins; `None` when it is not recorded (off, outside a frame, too deep).
    fn scope_begin(&self, name: &'static str, kind: ScopeKind) -> Option<ScopeToken>;
    /// The scope `token` came from ends.
    fn scope_end(&self, token: ScopeToken);
    /// Add `n` to a named counter for this frame.
    fn count(&self, name: &'static str, n: u64);
    /// Set a named gauge (memory, texture bytes).
    fn gauge(&self, name: &'static str, value: u64);

    /// Frame statistics over the last `window`.
    fn stats(&self, window: Duration) -> Stats;
    /// The breakdown by scope over the last `window`, into `out` (its buffers are reused).
    fn breakdown(&self, window: Duration, out: &mut Breakdown);
    /// The repaint causes over the last `window`, most frequent first, into `out`.
    fn causes(&self, window: Duration, out: &mut Vec<CauseCount>);
    /// The counters over the last `window`, into `out`.
    fn counters(&self, window: Duration, out: &mut Vec<CounterStats>);
    /// The gauges' latest values, into `out`.
    fn gauges(&self, out: &mut Vec<GaugeValue>);
}

/// A shared handle to a recorder: clones record into the same history.
#[derive(Clone)]
pub struct Recorder(Arc<dyn FrameRecorder>);

impl Recorder {
    /// Wrap a recorder.
    pub fn new(recorder: impl FrameRecorder + 'static) -> Self {
        Self(Arc::new(recorder))
    }

    /// Mark a named scope until the guard drops. Off, it is one atomic load and records nothing.
    pub fn scope(&self, name: &'static str) -> ScopeGuard<'_> {
        self.scope_kind(name, ScopeKind::Counted)
    }

    /// Mark a scope whose time is not the app's work (the performance panel itself): see
    /// [`ScopeKind::NotCounted`].
    pub fn scope_not_counted(&self, name: &'static str) -> ScopeGuard<'_> {
        self.scope_kind(name, ScopeKind::NotCounted)
    }

    fn scope_kind(&self, name: &'static str, kind: ScopeKind) -> ScopeGuard<'_> {
        let token = if self.0.is_enabled() {
            self.0.scope_begin(name, kind)
        } else {
            None
        };
        ScopeGuard {
            recorder: &*self.0,
            token,
        }
    }
}

impl std::ops::Deref for Recorder {
    type Target = dyn FrameRecorder;

    fn deref(&self) -> &Self::Target {
        &*self.0
    }
}

impl std::fmt::Debug for Recorder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Recorder")
            .field("enabled", &self.0.is_enabled())
            .finish()
    }
}

/// Ends its scope when dropped.
#[must_use = "the scope ends when the guard drops: bind it to a variable (`let _g = ...`)"]
pub struct ScopeGuard<'a> {
    recorder: &'a dyn FrameRecorder,
    token: Option<ScopeToken>,
}

impl Drop for ScopeGuard<'_> {
    fn drop(&mut self) {
        if let Some(token) = self.token.take() {
            self.recorder.scope_end(token);
        }
    }
}

/// The budget of one frame at `hz`.
pub fn budget_at(hz: f32) -> Duration {
    Duration::from_secs_f64(1.0 / f64::from(hz.max(1.0)))
}

/// `hz` snapped to the nearest of [`KNOWN_REFRESH_HZ`] when within 3 % of it.
pub fn snap_refresh_hz(hz: f32) -> f32 {
    KNOWN_REFRESH_HZ
        .iter()
        .copied()
        .find(|k| (hz - k).abs() <= k * 0.03)
        .unwrap_or(hz)
}

fn ctx_id() -> egui::Id {
    egui::Id::new("ve_basics::perf::Recorder")
}

/// Make `recorder` the one [`recorder`] finds through `ctx`, for code that has a `Ui` and not the host.
pub fn install(ctx: &egui::Context, recorder: Recorder) {
    ctx.data_mut(|d| d.insert_temp(ctx_id(), recorder));
}

/// The recorder [`install`] put into `ctx`, if any: `if let Some(rec) = perf::recorder(ui.ctx()) { let _g =
/// rec.scope("decode"); ... }`.
pub fn recorder(ctx: &egui::Context) -> Option<Recorder> {
    ctx.data(|d| d.get_temp::<Recorder>(ctx_id()))
}

impl Summary {
    /// Whether it covers no frame.
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}
