//! Frame recording for a performance panel (BAS-10): frame times, named scopes, counters, repaint causes.
//!
//! A host (`ve_app::Shell`, or any eframe app) marks each frame's begin and end on a [`FrameRecorder`]; widgets
//! and app code mark named [`scopes`](Recorder::scope) inside it. The panel reads, over any [`Span`] of the
//! recorder's clock: [`Stats`] (last, median, p95, p99, worst, fps, dropped frames, pauses, idle), the
//! [`Breakdown`] by scope with the rest as "egui and OS", the repaint causes, the counters, and the series and
//! histograms for its charts ([`FrameSeries`]).
//!
//! ```
//! use std::time::Duration;
//! use ve_basics::perf::{FrameBegin, FrameEnd, Recorder, RingRecorder, Settings};
//!
//! let rec = Recorder::new(RingRecorder::new(Settings::default()).expect("the default settings are valid"));
//! rec.frame_begin(FrameBegin { continuous: true });
//! {
//!     let _g = rec.scope("tracks"); // one widget's ui; scopes nest
//!     rec.count("ws.rx_bytes", 1500);
//! }
//! rec.frame_end(&FrameEnd::default());
//! let s = rec.stats(rec.last(Duration::from_secs(5)));
//! assert_eq!(s.frames, 1);
//! ```
//!
//! [`RingRecorder`] is the default: fixed rings sized from the retention, allocated once when recording turns on
//! and freed when it turns off, so a frame costs two clock reads and two short lock holds; off, it costs one
//! atomic load. Another recorder (one writing into an eventfull track, say) implements the same trait and the
//! panel reads it unchanged. Time comes from a [`Clock`], so tests drive it by hand.
//!
//! Scopes belong to the thread that runs the frames: a scope begun on another thread, or outside a frame, is not
//! recorded and is counted in [`Health`]. Counters and gauges may be set from any thread.

mod clock;
mod hist;
mod mem;
mod readout;
mod ring;

#[cfg(test)]
mod tests;

use std::sync::Arc;
use std::time::Duration;

#[cfg(any(test, feature = "test-support"))]
pub use clock::ManualClock;
pub use clock::{Clock, SystemClock};
pub use hist::{HIST_BINS, HIST_BINS_PER_OCTAVE, HIST_FLOOR_NS, Histogram};
pub use mem::rss_bytes;
pub use readout::{READOUT_RATE_HZ, Readout, ReadoutSettings, Shown};
pub use ring::{
    CAUSES_MAX, ENTRIES_MAX, ENTRIES_PER_RECORD_MAX, FRAMES_PER_RECORD_MAX, NAMES_MAX, RECORDS_MAX,
    RingRecorder, SCOPE_DEPTH_MAX, Sizing,
};

/// The range [`Settings::idle_after`] is kept in.
pub const IDLE_AFTER_RANGE: std::ops::RangeInclusive<Duration> =
    Duration::from_millis(100)..=Duration::from_secs(60);

/// The display refresh rates a measured one snaps to when within 3 %.
pub const KNOWN_REFRESH_HZ: [f32; 9] = [30.0, 50.0, 60.0, 75.0, 90.0, 100.0, 120.0, 144.0, 165.0];

/// The frame budget when neither the display nor a measurement gives one: 60 Hz.
pub const BUDGET_FALLBACK_HZ: f32 = 60.0;

/// The refresh rates a budget may come from, in Hz.
pub const BUDGET_HZ: std::ops::RangeInclusive<f32> = 1.0..=1000.0;

/// The longest retention a recorder accepts.
pub const RETENTION_MAX: Duration = Duration::from_secs(24 * 3600);

/// The spans a coarse granularity may aggregate.
pub const GRANULARITY_SPAN: std::ops::RangeInclusive<Duration> =
    Duration::from_millis(1)..=Duration::from_secs(10);

/// A stretch of the recorder's clock, `from_ns` included, `to_ns` not.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Span {
    /// Its start, in nanoseconds on the recorder's clock.
    pub from_ns: u64,
    /// Its end, not included.
    pub to_ns: u64,
}

impl Span {
    /// The last `len` before `now_ns`, both ends included.
    pub fn last(now_ns: u64, len: Duration) -> Self {
        Self {
            from_ns: now_ns.saturating_sub(u64::try_from(len.as_nanos()).unwrap_or(u64::MAX)),
            to_ns: now_ns.saturating_add(1),
        }
    }

    /// How long it is.
    pub fn len(&self) -> Duration {
        Duration::from_nanos(self.to_ns.saturating_sub(self.from_ns))
    }

    /// Whether it holds no time.
    pub fn is_empty(&self) -> bool {
        self.to_ns <= self.from_ns
    }

    /// Whether `t_ns` falls inside.
    pub fn contains(&self, t_ns: u64) -> bool {
        (self.from_ns..self.to_ns).contains(&t_ns)
    }
}

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
    /// Not the app's work: the performance panel's own ui. Its time is taken out of the frame's CPU time, every
    /// statistic and the scopes around it; scopes inside it do not count either. It still shows as a row.
    NotCounted,
}

/// What a scope is called: a static name and, for one of several instances (a widget per tab), its instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ScopeName {
    /// The name.
    pub name: &'static str,
    /// Which instance; `None` for a scope with one instance.
    pub instance: Option<u64>,
}

impl From<&'static str> for ScopeName {
    fn from(name: &'static str) -> Self {
        Self {
            name,
            instance: None,
        }
    }
}

/// A recorder's id for a scope name in one history; [`ScopeStats::id`] gives it, [`Series::Scope`] takes it.
/// After [`FrameRecorder::clear`], a new retention or granularity, or recording off and on, it names nothing:
/// a chart holding one shows no series and takes the new id from the next breakdown.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ScopeId {
    generation: u32,
    index: u32,
}

impl ScopeId {
    /// The id of name `index` in history `generation`; for recorders.
    pub fn new(generation: u32, index: u32) -> Self {
        Self { generation, index }
    }

    /// Which history it is from.
    pub fn generation(self) -> u32 {
        self.generation
    }

    /// The name's index in that history.
    pub fn index(self) -> u32 {
        self.index
    }
}

/// What a recorder hands out at a scope's begin and wants back at its end; its contents are the recorder's own.
#[derive(Clone, Copy, Debug)]
pub struct ScopeToken {
    start_ns: u64,
    payload: u64,
}

impl ScopeToken {
    /// A token for a recorder's own use: when the scope began and what the recorder needs at its end.
    pub fn new(start_ns: u64, payload: u64) -> Self {
        Self { start_ns, payload }
    }

    /// When the scope began, on the recorder's clock.
    pub fn start_ns(&self) -> u64 {
        self.start_ns
    }

    /// The recorder's own payload.
    pub fn payload(&self) -> u64 {
        self.payload
    }
}

/// Where the frame budget comes from (setting S19).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum BudgetSource {
    /// The display's refresh as the host reports it, else as measured from continuous frames.
    #[default]
    Display,
    /// A fixed refresh rate in Hz, within [`BUDGET_HZ`].
    Hz(f32),
}

/// How finely frames are kept (setting S27).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Granularity {
    /// Every frame on its own: exact statistics.
    #[default]
    EveryFrame,
    /// One aggregate per span of this length (10 ms, 100 ms; within [`GRANULARITY_SPAN`]): less memory for long
    /// retention; a statistic then sees each aggregate's mean and its worst frame.
    Every(Duration),
}

/// What a recorder keeps and how it judges frames: settings S19, S26 and S27.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    /// How far back history goes (S26), up to [`RETENTION_MAX`].
    pub retention: Duration,
    /// How finely it is kept (S27).
    pub granularity: Granularity,
    /// Where the budget comes from (S19).
    pub budget: BudgetSource,
    /// Nothing running this long (a frame's begin minus the previous frame's end) is idle, within
    /// [`IDLE_AFTER_RANGE`]: the numbers say "idle" instead of showing stale values, and a continuous frame after
    /// such a gap ends a pause (a hidden window), not dropped frames. A long frame is never a pause.
    pub idle_after: Duration,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            retention: Duration::from_secs(300),
            granularity: Granularity::EveryFrame,
            budget: BudgetSource::Display,
            idle_after: Duration::from_secs(1),
        }
    }
}

impl Settings {
    /// `Ok` when every setting is in its range.
    pub fn validate(&self) -> Result<(), SettingsError> {
        if self.retention.is_zero() || self.retention > RETENTION_MAX {
            return Err(SettingsError::Retention(self.retention));
        }
        if let Granularity::Every(span) = self.granularity
            && !GRANULARITY_SPAN.contains(&span)
        {
            return Err(SettingsError::Granularity(span));
        }
        if let BudgetSource::Hz(hz) = self.budget
            && !BUDGET_HZ.contains(&hz)
        {
            return Err(SettingsError::BudgetHz(hz));
        }
        if !IDLE_AFTER_RANGE.contains(&self.idle_after) {
            return Err(SettingsError::IdleAfter(self.idle_after));
        }
        Ok(())
    }
}

/// A setting out of its range.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SettingsError {
    /// A retention of zero or past [`RETENTION_MAX`].
    Retention(Duration),
    /// A granularity span outside [`GRANULARITY_SPAN`].
    Granularity(Duration),
    /// A budget's refresh rate outside [`BUDGET_HZ`] (NaN included).
    BudgetHz(f32),
    /// An idle line outside [`IDLE_AFTER_RANGE`].
    IdleAfter(Duration),
    /// A readout rate outside [`READOUT_RATE_HZ`] (NaN included).
    ReadoutRateHz(f32),
    /// A worst-value hold of zero or past a minute.
    WorstHold(Duration),
}

impl std::fmt::Display for SettingsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Retention(d) => {
                write!(f, "retention {d:?} is not between 0 and {RETENTION_MAX:?}")
            }
            Self::Granularity(d) => write!(f, "granularity {d:?} is outside {GRANULARITY_SPAN:?}"),
            Self::BudgetHz(hz) => write!(f, "budget refresh {hz} Hz is outside {BUDGET_HZ:?}"),
            Self::IdleAfter(d) => write!(f, "idle line {d:?} is outside {IDLE_AFTER_RANGE:?}"),
            Self::ReadoutRateHz(hz) => {
                write!(f, "readout rate {hz} Hz is outside {READOUT_RATE_HZ:?}")
            }
            Self::WorstHold(d) => write!(f, "worst hold {d:?} is not between 0 and 60 s"),
        }
    }
}

impl std::error::Error for SettingsError {}

/// A distribution's summary over a span. Median and percentiles come from a [`Histogram`] (within 1.6 %); last
/// and worst are exact.
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

impl Summary {
    /// Whether it covers no frame.
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}

/// Frame statistics over a span.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stats {
    /// The span asked for.
    pub span: Span,
    /// The part of it the recorder holds frames for.
    pub covered: Span,
    /// Frames begun in it and ended.
    pub frames: u32,
    /// Frames per second over the covered part.
    pub fps: f32,
    /// Start-to-start intervals of continuous frames: what the eye sees as smoothness.
    pub interval: Summary,
    /// CPU time per frame, without the time of scopes that do not count.
    pub cpu: Summary,
    /// Frames missed: an interval of k budgets counts k - 1, only between continuous frames.
    pub dropped: u32,
    /// Continuous frames after nothing ran for [`Settings::idle_after`] (a hidden window): their interval is
    /// neither an interval nor drops.
    pub pauses: u32,
    /// The budget frames are judged against now.
    pub budget: Duration,
    /// The refresh rate the budget comes from.
    pub refresh_hz: f32,
    /// How long since the last frame ended, now, when that is past [`Settings::idle_after`].
    pub idle: Option<Duration>,
}

/// One scope's statistics over a span.
#[derive(Clone, Debug, PartialEq)]
pub struct ScopeStats {
    /// The recorder's id for it, for [`Series::Scope`].
    pub id: ScopeId,
    /// Its name.
    pub name: Arc<str>,
    /// Its instance, for one of several.
    pub instance: Option<u64>,
    /// How its time counts.
    pub kind: ScopeKind,
    /// Its time per frame it ran in, inner scopes included; a scope inside one of the same name is part of
    /// the outer one.
    pub time: Summary,
    /// Its time over the covered span, inner scopes included.
    pub total: Duration,
    /// Its time over the covered span without its inner counted scopes.
    pub self_total: Duration,
    /// Its share of the covered span's counted CPU time (0..1); 0 for scopes that do not count.
    pub share: f32,
    /// Whether it ran outside any other scope.
    pub top_level: bool,
}

/// Where the CPU time of a span went: the scopes, and the rest ("egui and OS").
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Breakdown {
    /// The part of the span the recorder still holds scopes for: scope history may be shorter than frames'.
    pub covered: Span,
    /// Every scope seen in the covered span, by total time, largest first.
    pub scopes: Vec<ScopeStats>,
    /// Per frame: CPU time outside every top-level counted scope.
    pub rest: Summary,
    /// The covered span's CPU time outside every top-level counted scope.
    pub rest_total: Duration,
}

/// A repaint cause and how many frames it caused in a span.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CauseCount {
    /// `reason` when there is one, else the file's last two parts and the line.
    pub label: Arc<str>,
    /// Frames (aggregates, at a coarse granularity) it caused.
    pub frames: u32,
}

/// An app counter over a span ([`FrameRecorder::count`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CounterStats {
    /// The counter's name.
    pub name: Arc<str>,
    /// Its sum over the covered span.
    pub total: u64,
    /// The most counted in one frame (one aggregate at a coarse granularity).
    pub worst: u64,
    /// Frames it was counted in.
    pub frames: u32,
}

/// A gauge's latest value ([`FrameRecorder::gauge`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GaugeValue {
    /// The gauge's name.
    pub name: Arc<str>,
    /// Its latest value.
    pub value: u64,
    /// When it was set, on the recorder's clock.
    pub at_ns: u64,
}

/// What a recorder could not record, and what it holds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Health {
    /// The span it holds frames for.
    pub frames_covered: Span,
    /// The span it holds scopes and counters for.
    pub scopes_covered: Span,
    /// Names past its table.
    pub lost_names: u32,
    /// Scopes nested past its depth.
    pub lost_depth: u32,
    /// Scope and counter entries past a frame's limit.
    pub lost_entries: u32,
    /// Repaint causes past its table.
    pub lost_causes: u32,
    /// Scopes begun outside a frame.
    pub outside_frame: u32,
    /// Scopes begun on a thread other than the frames'.
    pub other_thread: u32,
    /// Scope ends that came out of order or after their frame.
    pub stale_scopes: u32,
    /// Frames begun and never ended.
    pub abandoned_frames: u32,
}

/// Which series a chart reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Series {
    /// CPU time per frame.
    Cpu,
    /// Interval between continuous frames.
    Interval,
    /// One scope's time per frame it ran in.
    Scope(ScopeId),
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
    /// The median, within 1.6 %.
    pub median: Duration,
    /// The 99th percentile, within 1.6 %.
    pub p99: Duration,
}

/// What a chart reads: values binned into columns, or one histogram, over a span. The seam for other chart
/// backends.
pub trait FrameSeries {
    /// Fill `out` with `out.len()` equal columns of `span`. Columns without a frame get `count == 0`.
    fn bins(&self, series: Series, span: Span, out: &mut [Bin]);
    /// Count `series` over `span` into `out` (cleared first).
    fn histogram(&self, series: Series, span: Span, out: &mut Histogram);
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
    /// Change the settings; a new retention or granularity starts an empty history. Out-of-range settings are
    /// refused and the old ones stay.
    fn set_settings(&self, settings: Settings) -> Result<(), SettingsError>;
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
    /// The display's refresh rate as the host reads it; `None`, or a rate outside [`BUDGET_HZ`], to measure it
    /// from continuous frames.
    fn set_display_hz(&self, hz: Option<f32>);

    /// A scope begins; `None` when it is not recorded (off, outside a frame, another thread, too deep).
    fn scope_begin(&self, name: ScopeName, kind: ScopeKind) -> Option<ScopeToken>;
    /// The scope `token` came from ends.
    fn scope_end(&self, token: ScopeToken);
    /// Add `n` to a named counter for this frame.
    fn count(&self, name: &'static str, n: u64);
    /// Set a named gauge (memory, texture bytes).
    fn gauge(&self, name: &'static str, value: u64);

    /// Frame statistics over `span`.
    fn stats(&self, span: Span) -> Stats;
    /// The breakdown by scope over `span`, into `out` (its buffers are reused).
    fn breakdown(&self, span: Span, out: &mut Breakdown);
    /// The repaint causes over `span`, most frequent first, into `out`.
    fn causes(&self, span: Span, out: &mut Vec<CauseCount>);
    /// The counters over `span`, into `out`.
    fn counters(&self, span: Span, out: &mut Vec<CounterStats>);
    /// The gauges' latest values, into `out`.
    fn gauges(&self, out: &mut Vec<GaugeValue>);
    /// What it could not record, and the spans it holds.
    fn health(&self) -> Health;
}

/// A shared handle to a recorder: clones record into the same history.
///
/// [`recorder`] finds the one a host installed; fetch it once and keep it (a widget's transient state), since
/// each lookup takes egui's data lock.
#[derive(Clone)]
pub struct Recorder(Arc<dyn FrameRecorder>);

impl Recorder {
    /// Wrap a recorder.
    pub fn new(recorder: impl FrameRecorder + 'static) -> Self {
        Self(Arc::new(recorder))
    }

    /// The last `len` up to now on its clock.
    pub fn last(&self, len: Duration) -> Span {
        Span::last(self.0.now_ns(), len)
    }

    /// Mark a named scope until the guard drops. Off, it is one atomic load and records nothing.
    pub fn scope(&self, name: &'static str) -> ScopeGuard<'_> {
        self.scope_with(name.into(), ScopeKind::Counted)
    }

    /// Mark a scope of one of several instances (a widget per tab): each instance has its own row.
    pub fn scope_of(&self, name: &'static str, instance: u64) -> ScopeGuard<'_> {
        let name = ScopeName {
            name,
            instance: Some(instance),
        };
        self.scope_with(name, ScopeKind::Counted)
    }

    /// Mark a scope whose time is not the app's work (the performance panel itself): see
    /// [`ScopeKind::NotCounted`].
    pub fn scope_not_counted(&self, name: &'static str) -> ScopeGuard<'_> {
        self.scope_with(name.into(), ScopeKind::NotCounted)
    }

    /// Mark a scope with a name and a kind.
    pub fn scope_with(&self, name: ScopeName, kind: ScopeKind) -> ScopeGuard<'_> {
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

    /// Whether both handles share one recorder.
    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
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

/// The budget of one frame at `hz`, `hz` within [`BUDGET_HZ`] (else [`BUDGET_FALLBACK_HZ`]).
pub fn budget_at(hz: f32) -> Duration {
    let hz = if BUDGET_HZ.contains(&hz) {
        hz
    } else {
        BUDGET_FALLBACK_HZ
    };
    Duration::from_secs_f64(1.0 / f64::from(hz))
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

/// The recorder [`install`] put into `ctx`, if any. Each call takes egui's data lock: fetch it once and keep it.
pub fn recorder(ctx: &egui::Context) -> Option<Recorder> {
    ctx.data(|d| d.get_temp::<Recorder>(ctx_id()))
}
