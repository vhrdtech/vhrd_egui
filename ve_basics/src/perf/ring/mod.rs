//! The default recorder: fixed rings of frame records and scope entries.

mod read;

use std::ops::Range;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use super::{
    BUDGET_FALLBACK_HZ, Breakdown, BudgetSource, Cause, CauseCount, Clock, CounterStats,
    FrameBegin, FrameEnd, FrameRecorder, GaugeValue, Granularity, ScopeKind, ScopeToken, Settings,
    Stats, SystemClock, snap_refresh_hz,
};

/// Distinct names (scopes, counters, gauges) a recorder keeps; further names are not recorded.
pub const NAMES_MAX: usize = 256;
/// Distinct repaint causes a recorder tells apart; further ones count as "other causes".
pub const CAUSES_MAX: usize = 15;
/// Scopes open inside each other at most; deeper ones are not recorded.
pub const SCOPE_DEPTH_MAX: usize = 32;
/// Scope and counter entries one record holds at most; further ones are not recorded.
pub const ENTRIES_PER_RECORD_MAX: usize = 1024;
/// Frames one aggregate holds at most (a coarse granularity at a high refresh rate starts a new one).
pub const FRAMES_PER_RECORD_MAX: u8 = u8::MAX;
/// Records a ring holds at most, whatever the retention: 160 MiB of them.
pub const RECORDS_MAX: usize = 1 << 22;
/// Continuous intervals kept to measure the display's refresh.
const GAPS_KEPT: usize = 128;
/// The measured refresh is recomputed every this many continuous frames.
const GAPS_REMEASURE: usize = 32;

/// Parent id of a top-level scope.
const TOP: u16 = u16::MAX;
/// Parent id marking a counter entry.
const COUNTER: u16 = u16::MAX - 1;
/// Cause bit for causes past [`CAUSES_MAX`].
const OTHER_CAUSE: u16 = 1 << 15;
/// Token flag: the outermost open scope that does not count.
const OUTERMOST_NOT_COUNTED: u16 = 1;
const _: () = assert!(
    CAUSES_MAX < 16,
    "causes are bits of a u16 with the top bit for other causes"
);
const _: () = assert!(
    NAMES_MAX < COUNTER as usize,
    "name ids stay below the parent markers"
);

/// How a [`RingRecorder`] sizes its rings for its retention.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sizing {
    /// The highest frame rate the retention holds at; faster, the oldest frames go sooner.
    pub rate_hz: u32,
    /// Scope and counter entries per record on average; more, and scope history is shorter than the frames'.
    pub entries_per_record: u32,
}

impl Default for Sizing {
    fn default() -> Self {
        Self {
            rate_hz: 120,
            entries_per_record: 2,
        }
    }
}

impl Sizing {
    /// Records for `settings`' retention.
    pub fn records(&self, settings: &Settings) -> usize {
        let secs = settings.retention.as_secs_f64();
        let by_rate = secs * f64::from(self.rate_hz);
        let n = match settings.granularity {
            Granularity::EveryFrame => by_rate,
            Granularity::Every(d) => by_rate.min(secs / d.as_secs_f64().max(1e-6)),
        };
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "clamped to RECORDS_MAX right after"
        )]
        let n = n.ceil().min(RECORDS_MAX as f64) as usize;
        n.clamp(16, RECORDS_MAX)
    }

    /// Scope and counter entries for `settings`' retention.
    pub fn entries(&self, settings: &Settings) -> usize {
        (self.records(settings) * self.entries_per_record as usize).max(2 * ENTRIES_PER_RECORD_MAX)
    }

    /// Bytes the rings take when full.
    pub fn bytes(&self, settings: &Settings) -> usize {
        self.records(settings) * size_of::<Record>() + self.entries(settings) * size_of::<Entry>()
    }
}

/// One frame, or the frames of one span at a coarse granularity.
#[derive(Clone, Copy, Debug, Default)]
struct Record {
    /// Begin of its first frame.
    start_ns: u64,
    cpu_sum_ns: u32,
    cpu_max_ns: u32,
    /// Intervals before its continuous frames.
    gap_sum_ns: u32,
    gap_max_ns: u32,
    causes: u16,
    dropped: u16,
    /// Its scope and counter entries: the newest this many before the next record's first.
    entries: u16,
    frames: u8,
    /// Its continuous frames: those with an interval in `gap_*`.
    smooth: u8,
}

/// A scope's time or a counter's value within one record.
#[derive(Clone, Copy, Debug, Default)]
struct Entry {
    value: u32,
    id: u16,
    /// The enclosing scope's id, [`TOP`] or [`COUNTER`].
    parent: u16,
}

/// A ring of `T`s addressed by sequence number; the oldest is overwritten when full.
#[derive(Debug)]
struct Ring<T> {
    buf: Vec<T>,
    cap: usize,
    total: u64,
}

impl<T: Copy> Ring<T> {
    fn new(cap: usize) -> Self {
        Self {
            buf: Vec::with_capacity(cap),
            cap,
            total: 0,
        }
    }

    fn push(&mut self, t: T) {
        if self.buf.len() < self.cap {
            self.buf.push(t);
        } else {
            let i = self.index(self.total);
            self.buf[i] = t;
        }
        self.total += 1;
    }

    fn oldest(&self) -> u64 {
        self.total.saturating_sub(self.cap as u64)
    }

    fn index(&self, seq: u64) -> usize {
        // The remainder is below `cap`, a usize.
        (seq % self.cap as u64) as usize
    }

    fn get(&self, seq: u64) -> Option<&T> {
        (seq >= self.oldest() && seq < self.total).then(|| &self.buf[self.index(seq)])
    }

    fn get_mut(&mut self, seq: u64) -> Option<&mut T> {
        if seq >= self.oldest() && seq < self.total {
            let i = self.index(seq);
            Some(&mut self.buf[i])
        } else {
            None
        }
    }

    fn newest_mut(&mut self) -> Option<&mut T> {
        self.total.checked_sub(1).and_then(|s| self.get_mut(s))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NameKind {
    Scope(ScopeKind),
    Counter,
    Gauge,
}

#[derive(Clone, Copy, Debug)]
struct Name {
    name: &'static str,
    kind: NameKind,
    gauge: u64,
    gauge_at_ns: u64,
}

#[derive(Clone, Debug)]
struct CauseSlot {
    file: &'static str,
    line: u32,
    label: String,
}

/// The frame whose CPU time the host may still report ([`FrameRecorder::previous_frame_cpu`]).
#[derive(Clone, Copy, Debug)]
struct Prev {
    record: u64,
    measured_ns: u64,
    not_counted_ns: u64,
    newest: bool,
}

/// What was not recorded because a limit was reached.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Lost {
    names: u32,
    depth: u32,
    entries: u32,
    outside_frame: u32,
}

/// Per-name sums of a breakdown read, reused between reads.
#[derive(Clone, Copy, Debug, Default)]
struct Acc {
    total: u64,
    children: u64,
    top_level: bool,
    seen: bool,
}

/// The history and the frame in progress; exists only while recording.
#[derive(Debug)]
struct State {
    records: Ring<Record>,
    entries: Ring<Entry>,
    names: Vec<Name>,
    causes: Vec<CauseSlot>,
    stack: [u16; SCOPE_DEPTH_MAX],
    /// Per open scope: time of the not-counted scopes inside it so far, taken out of its own time.
    not_counted_inside: [u64; SCOPE_DEPTH_MAX],
    depth: usize,
    in_frame: bool,
    frame_start_ns: u64,
    not_counted_ns: u64,
    not_counted_open: u16,
    first_ns: Option<u64>,
    last_start_ns: Option<u64>,
    last_end_ns: Option<u64>,
    last_cpu_ns: u64,
    last_gap_ns: u64,
    prev: Option<Prev>,
    gaps: [u32; GAPS_KEPT],
    gaps_total: usize,
    measured_hz: Option<f32>,
    lost: Lost,
    acc: Vec<Acc>,
    seqs: Vec<(u64, Range<u64>)>,
    #[cfg(feature = "alloc-count")]
    alloc_at_begin: Option<(u64, u64)>,
}

#[derive(Debug)]
struct Inner {
    settings: Settings,
    sizing: Sizing,
    display_hz: Option<f32>,
    state: Option<State>,
}

/// The default [`FrameRecorder`]: rings sized from the retention ([`Sizing`]), allocated when recording turns
/// on and dropped when it turns off; nothing allocates per frame once the name and cause tables have filled.
pub struct RingRecorder {
    enabled: AtomicBool,
    clock: Arc<dyn Clock>,
    inner: Mutex<Inner>,
}

impl RingRecorder {
    /// A recording recorder on the system clock.
    pub fn new(settings: Settings) -> Self {
        Self::with_clock(settings, SystemClock::default())
    }

    /// A recording recorder on `clock`.
    pub fn with_clock(settings: Settings, clock: impl Clock + 'static) -> Self {
        Self::with_sizing(settings, Sizing::default(), clock)
    }

    /// A recording recorder on `clock`, its rings sized by `sizing`.
    pub fn with_sizing(settings: Settings, sizing: Sizing, clock: impl Clock + 'static) -> Self {
        Self {
            enabled: AtomicBool::new(true),
            clock: Arc::new(clock),
            inner: Mutex::new(Inner {
                settings,
                sizing,
                display_hz: None,
                state: Some(State::new(&settings, &sizing)),
            }),
        }
    }

    /// Bytes its rings take when full; 0 while off.
    pub fn memory_bytes(&self) -> usize {
        let inner = self.lock();
        match inner.state {
            Some(_) => inner.sizing.bytes(&inner.settings),
            None => 0,
        }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        // spirit-level: allow(lock-unwrap) a poisoned lock panics: the panic came from inside a recording call
        self.inner
            .lock()
            .expect("perf recorder lock poisoned: a recording call panicked")
    }

    fn now(&self) -> u64 {
        self.clock.now_ns()
    }
}

impl std::fmt::Debug for RingRecorder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RingRecorder")
            .field("enabled", &self.enabled.load(Ordering::Relaxed))
            .finish_non_exhaustive()
    }
}

fn ns(d: Duration) -> u64 {
    u64::try_from(d.as_nanos()).unwrap_or(u64::MAX)
}

fn sat32(v: u64) -> u32 {
    u32::try_from(v).unwrap_or(u32::MAX)
}

impl Inner {
    fn budget_ns(&self, measured_hz: Option<f32>) -> (u64, f32) {
        let hz = match self.settings.budget {
            BudgetSource::Hz(hz) => hz,
            BudgetSource::Display => self
                .display_hz
                .or(measured_hz)
                .unwrap_or(BUDGET_FALLBACK_HZ),
        };
        (ns(super::budget_at(hz)), hz)
    }
}

impl State {
    fn new(settings: &Settings, sizing: &Sizing) -> Self {
        Self {
            records: Ring::new(sizing.records(settings)),
            entries: Ring::new(sizing.entries(settings)),
            names: Vec::with_capacity(32),
            causes: Vec::with_capacity(CAUSES_MAX),
            stack: [0; SCOPE_DEPTH_MAX],
            not_counted_inside: [0; SCOPE_DEPTH_MAX],
            depth: 0,
            in_frame: false,
            frame_start_ns: 0,
            not_counted_ns: 0,
            not_counted_open: 0,
            first_ns: None,
            last_start_ns: None,
            last_end_ns: None,
            last_cpu_ns: 0,
            last_gap_ns: 0,
            prev: None,
            gaps: [0; GAPS_KEPT],
            gaps_total: 0,
            measured_hz: None,
            lost: Lost::default(),
            acc: Vec::new(),
            seqs: Vec::new(),
            #[cfg(feature = "alloc-count")]
            alloc_at_begin: None,
        }
    }

    fn intern(&mut self, name: &'static str, kind: NameKind) -> Option<u16> {
        let same = |n: &Name| n.kind == kind && std::ptr::eq(n.name, name);
        let found = self.names.iter().position(same).or_else(|| {
            self.names
                .iter()
                .position(|n| n.kind == kind && n.name == name)
        });
        if let Some(i) = found {
            return u16::try_from(i).ok();
        }
        if self.names.len() >= NAMES_MAX {
            self.lost.names += 1;
            return None;
        }
        self.names.push(Name {
            name,
            kind,
            gauge: 0,
            gauge_at_ns: 0,
        });
        u16::try_from(self.names.len() - 1).ok()
    }

    fn cause_bit(&mut self, cause: &Cause<'_>) -> u16 {
        let found = self.causes.iter().position(|c| {
            c.line == cause.line
                && c.file == cause.file
                && (!cause.file.is_empty() || c.label == cause.reason)
        });
        let i = match found {
            Some(i) => i,
            None if self.causes.len() < CAUSES_MAX => {
                let label = if cause.reason.is_empty() {
                    format!("{}:{}", cause.file, cause.line)
                } else {
                    cause.reason.to_owned()
                };
                self.causes.push(CauseSlot {
                    file: cause.file,
                    line: cause.line,
                    label,
                });
                self.causes.len() - 1
            }
            None => return OTHER_CAUSE,
        };
        1 << i
    }

    fn remeasure(&mut self) {
        let n = self.gaps_total.min(GAPS_KEPT);
        if n < GAPS_REMEASURE {
            return;
        }
        let mut gaps = self.gaps;
        let gaps = &mut gaps[..n];
        // The display's refresh is the shortest interval the app reaches; a low percentile ignores outliers.
        let k = n / 10;
        let (_, p10, _) = gaps.select_nth_unstable(k);
        if *p10 > 0 {
            #[expect(
                clippy::cast_precision_loss,
                reason = "an interval in nanoseconds; f32 keeps 7 digits"
            )]
            let hz = 1e9 / *p10 as f32;
            self.measured_hz = Some(snap_refresh_hz(hz));
        }
    }

    fn begin(&mut self, now: u64, begin: FrameBegin, settings: &Settings, budget_ns: u64) {
        if self.in_frame {
            // The host began a frame without ending the last: that one is dropped from CPU statistics.
            self.in_frame = false;
        }
        let gap = self.last_start_ns.map(|s| now.saturating_sub(s));
        let mut dropped = 0;
        let continuous = begin.continuous && gap.is_some();
        if let (true, Some(gap)) = (continuous, gap) {
            self.gaps[self.gaps_total % GAPS_KEPT] = sat32(gap);
            self.gaps_total += 1;
            if self.gaps_total.is_multiple_of(GAPS_REMEASURE) {
                self.remeasure();
            }
            let budgets = (gap + budget_ns / 2) / budget_ns.max(1);
            dropped = budgets.saturating_sub(1);
            self.last_gap_ns = gap;
        }
        let new_record = match (settings.granularity, self.records.newest_mut()) {
            (Granularity::EveryFrame, _) | (_, None) => true,
            (Granularity::Every(span), Some(r)) => {
                let span = ns(span).max(1);
                r.start_ns / span != now / span || r.frames == FRAMES_PER_RECORD_MAX
            }
        };
        if new_record {
            self.records.push(Record {
                start_ns: now,
                ..Record::default()
            });
        }
        if let Some(r) = self.records.newest_mut() {
            r.frames = r.frames.saturating_add(1);
            if let (true, Some(gap)) = (continuous, gap) {
                r.smooth = r.smooth.saturating_add(1);
                r.gap_sum_ns = r.gap_sum_ns.saturating_add(sat32(gap));
                r.gap_max_ns = r.gap_max_ns.max(sat32(gap));
            }
            r.dropped = r
                .dropped
                .saturating_add(u16::try_from(dropped).unwrap_or(u16::MAX));
        }
        self.in_frame = true;
        self.frame_start_ns = now;
        self.not_counted_ns = 0;
        self.not_counted_open = 0;
        self.depth = 0;
        self.last_start_ns = Some(now);
        self.first_ns.get_or_insert(now);
        if let Some(p) = &mut self.prev {
            p.newest = false;
        }
        #[cfg(feature = "alloc-count")]
        {
            self.alloc_at_begin = super::alloc_totals();
        }
    }

    fn end(&mut self, now: u64, end: &FrameEnd<'_>) {
        if !self.in_frame {
            return;
        }
        let cpu = now
            .saturating_sub(self.frame_start_ns)
            .saturating_sub(self.not_counted_ns);
        let mut bits = 0;
        for c in end.causes {
            bits |= self.cause_bit(c);
        }
        #[cfg(feature = "alloc-count")]
        if let (Some((n0, b0)), Some((n1, b1))) = (self.alloc_at_begin, super::alloc_totals()) {
            for (name, v) in [("alloc.count", n1 - n0), ("alloc.bytes", b1 - b0)] {
                if let Some(id) = self.intern(name, NameKind::Counter) {
                    self.add_entry(id, COUNTER, v);
                }
            }
        }
        if let Some(r) = self.records.newest_mut() {
            r.cpu_sum_ns = r.cpu_sum_ns.saturating_add(sat32(cpu));
            r.cpu_max_ns = r.cpu_max_ns.max(sat32(cpu));
            r.causes |= bits;
        }
        self.prev = Some(Prev {
            record: self.records.total - 1,
            measured_ns: cpu,
            not_counted_ns: self.not_counted_ns,
            newest: true,
        });
        self.last_cpu_ns = cpu;
        self.last_end_ns = Some(now);
        self.in_frame = false;
    }

    fn previous_frame_cpu(&mut self, cpu: Duration) {
        let Some(prev) = self.prev.take() else {
            return;
        };
        let full = ns(cpu).saturating_sub(prev.not_counted_ns);
        if let Some(r) = self.records.get_mut(prev.record) {
            let sum = u64::from(r.cpu_sum_ns).saturating_sub(prev.measured_ns) + full;
            r.cpu_sum_ns = sat32(sum);
            r.cpu_max_ns = r.cpu_max_ns.max(sat32(full));
        }
        if prev.newest || self.in_frame {
            self.last_cpu_ns = full;
        }
    }

    /// Entry sequence numbers of the newest record.
    fn newest_entries(&self) -> Range<u64> {
        let n = self
            .records
            .total
            .checked_sub(1)
            .and_then(|s| self.records.get(s))
            .map_or(0, |r| u64::from(r.entries));
        let oldest = self.entries.oldest();
        self.entries.total.saturating_sub(n).max(oldest)..self.entries.total
    }

    fn add_entry(&mut self, id: u16, parent: u16, value: u64) {
        if self.records.total == 0 {
            return;
        }
        let range = self.newest_entries();
        for seq in range.clone() {
            if let Some(e) = self.entries.get_mut(seq)
                && e.id == id
                && e.parent == parent
            {
                e.value = e.value.saturating_add(sat32(value));
                return;
            }
        }
        if (range.end - range.start) as usize >= ENTRIES_PER_RECORD_MAX {
            self.lost.entries += 1;
            return;
        }
        self.entries.push(Entry {
            value: sat32(value),
            id,
            parent,
        });
        if let Some(r) = self.records.newest_mut() {
            r.entries += 1;
        }
    }

    /// The records whose begin is at or after `t0`, newest first, with the sequence numbers of their entries
    /// still held; without the newest when it holds only the frame in progress.
    fn window(&self, t0: u64) -> impl Iterator<Item = (u64, Range<u64>)> + '_ {
        let skip = usize::from(
            self.in_frame
                && self
                    .records
                    .total
                    .checked_sub(1)
                    .and_then(|s| self.records.get(s))
                    .is_some_and(|r| r.frames <= 1),
        );
        let oldest = self.entries.oldest();
        (self.records.oldest()..self.records.total)
            .rev()
            .scan(self.entries.total, move |end, seq| {
                let r = self.records.get(seq)?;
                let start = end.saturating_sub(u64::from(r.entries));
                let entries = start.max(oldest)..(*end).max(oldest);
                *end = start;
                Some((seq, r.start_ns, entries))
            })
            .skip(skip)
            .take_while(move |(_, start_ns, _)| *start_ns >= t0)
            .map(|(seq, _, entries)| (seq, entries))
    }

    /// Frames of record `seq` with a CPU time: all but the one in progress.
    fn cpu_frames(&self, seq: u64, r: &Record) -> u32 {
        let open = self.in_frame && seq + 1 == self.records.total;
        u32::from(r.frames) - u32::from(open)
    }

    fn t0(&self, now: u64, window: Duration, retention: Duration) -> u64 {
        now.saturating_sub(ns(window.min(retention)))
    }
}

impl FrameRecorder for RingRecorder {
    fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    fn set_enabled(&self, on: bool) {
        let mut inner = self.lock();
        self.enabled.store(on, Ordering::Relaxed);
        if on && inner.state.is_none() {
            inner.state = Some(State::new(&inner.settings, &inner.sizing));
        } else if !on {
            inner.state = None;
        }
    }

    fn settings(&self) -> Settings {
        self.lock().settings
    }

    fn set_settings(&self, settings: Settings) {
        let mut inner = self.lock();
        let rebuild = settings.retention != inner.settings.retention
            || settings.granularity != inner.settings.granularity;
        inner.settings = settings;
        if rebuild && inner.state.is_some() {
            inner.state = Some(State::new(&inner.settings, &inner.sizing));
        }
    }

    fn now_ns(&self) -> u64 {
        self.now()
    }

    fn clear(&self) {
        let mut inner = self.lock();
        if inner.state.is_some() {
            inner.state = Some(State::new(&inner.settings, &inner.sizing));
        }
    }

    fn frame_begin(&self, begin: FrameBegin) {
        if !self.is_enabled() {
            return;
        }
        let now = self.now();
        let mut inner = self.lock();
        let Some(measured_hz) = inner.state.as_ref().map(|st| st.measured_hz) else {
            return;
        };
        let (budget, _) = inner.budget_ns(measured_hz);
        let settings = inner.settings;
        if let Some(st) = &mut inner.state {
            st.begin(now, begin, &settings, budget);
        }
    }

    fn previous_frame_cpu(&self, cpu: Duration) {
        if !self.is_enabled() {
            return;
        }
        if let Some(st) = &mut self.lock().state {
            st.previous_frame_cpu(cpu);
        }
    }

    fn frame_end(&self, end: &FrameEnd<'_>) {
        if !self.is_enabled() {
            return;
        }
        let now = self.now();
        if let Some(st) = &mut self.lock().state {
            st.end(now, end);
        }
    }

    fn set_display_hz(&self, hz: Option<f32>) {
        self.lock().display_hz = hz.filter(|hz| *hz >= 1.0);
    }

    fn scope_begin(&self, name: &'static str, kind: ScopeKind) -> Option<ScopeToken> {
        if !self.is_enabled() {
            return None;
        }
        let token = {
            let mut inner = self.lock();
            let st = inner.state.as_mut()?;
            if !st.in_frame {
                st.lost.outside_frame += 1;
                return None;
            }
            if st.depth >= SCOPE_DEPTH_MAX {
                st.lost.depth += 1;
                return None;
            }
            // Everything inside a scope that does not count does not count either.
            let kind = match st.not_counted_open {
                0 => kind,
                _ => ScopeKind::NotCounted,
            };
            let id = st.intern(name, NameKind::Scope(kind))?;
            let parent = match st.depth {
                0 => TOP,
                d => st.stack[d - 1],
            };
            st.stack[st.depth] = id;
            st.not_counted_inside[st.depth] = 0;
            st.depth += 1;
            let mut flags = 0;
            if kind == ScopeKind::NotCounted {
                if st.not_counted_open == 0 {
                    flags |= OUTERMOST_NOT_COUNTED;
                }
                st.not_counted_open += 1;
            }
            ScopeToken {
                start_ns: 0,
                id,
                parent,
                depth: u16::try_from(st.depth - 1).unwrap_or(u16::MAX),
                flags,
            }
        };
        Some(ScopeToken {
            start_ns: self.now(),
            ..token
        })
    }

    fn scope_end(&self, token: ScopeToken) {
        let now = self.now();
        let mut inner = self.lock();
        let Some(st) = &mut inner.state else {
            return;
        };
        if !st.in_frame || token.start_ns < st.frame_start_ns {
            return;
        }
        let dur = now.saturating_sub(token.start_ns);
        let depth = usize::from(token.depth);
        if depth >= st.depth {
            // Its guard outlived an outer scope's: the stack no longer holds it.
            return;
        }
        st.depth = depth;
        let inside = st.not_counted_inside[depth];
        let not_counted = st.names.get(usize::from(token.id)).map(|n| n.kind)
            == Some(NameKind::Scope(ScopeKind::NotCounted));
        // A counted scope keeps its counted time; a not-counted one shows all of its time on its own row.
        let (value, hidden) = if not_counted {
            st.not_counted_open = st.not_counted_open.saturating_sub(1);
            if token.flags & OUTERMOST_NOT_COUNTED != 0 {
                st.not_counted_ns += dur;
            }
            (dur, dur)
        } else {
            (dur.saturating_sub(inside), inside)
        };
        if let Some(outer) = depth.checked_sub(1) {
            st.not_counted_inside[outer] += hidden;
        }
        st.add_entry(token.id, token.parent, value);
    }

    fn count(&self, name: &'static str, n: u64) {
        if !self.is_enabled() {
            return;
        }
        if let Some(st) = &mut self.lock().state
            && let Some(id) = st.intern(name, NameKind::Counter)
        {
            st.add_entry(id, COUNTER, n);
        }
    }

    fn gauge(&self, name: &'static str, value: u64) {
        if !self.is_enabled() {
            return;
        }
        let now = self.now();
        if let Some(st) = &mut self.lock().state
            && let Some(id) = st.intern(name, NameKind::Gauge)
        {
            let g = &mut st.names[usize::from(id)];
            g.gauge = value;
            g.gauge_at_ns = now;
        }
    }

    fn stats(&self, window: Duration) -> Stats {
        let now = self.now();
        let inner = self.lock();
        match &inner.state {
            Some(st) => st.stats(now, window, &inner),
            None => Stats {
                window,
                ..Stats::default()
            },
        }
    }

    fn breakdown(&self, window: Duration, out: &mut Breakdown) {
        let now = self.now();
        let mut inner = self.lock();
        let retention = inner.settings.retention;
        match &mut inner.state {
            Some(st) => {
                let t0 = st.t0(now, window, retention);
                st.breakdown(t0, out);
            }
            None => *out = Breakdown::default(),
        }
    }

    fn causes(&self, window: Duration, out: &mut Vec<CauseCount>) {
        out.clear();
        let now = self.now();
        let inner = self.lock();
        if let Some(st) = &inner.state {
            st.causes(st.t0(now, window, inner.settings.retention), out);
        }
    }

    fn counters(&self, window: Duration, out: &mut Vec<CounterStats>) {
        out.clear();
        let now = self.now();
        let inner = self.lock();
        if let Some(st) = &inner.state {
            st.counters(st.t0(now, window, inner.settings.retention), out);
        }
    }

    fn gauges(&self, out: &mut Vec<GaugeValue>) {
        out.clear();
        let inner = self.lock();
        let Some(st) = &inner.state else {
            return;
        };
        out.extend(
            st.names
                .iter()
                .filter(|n| n.kind == NameKind::Gauge)
                .map(|n| GaugeValue {
                    name: n.name,
                    value: n.gauge,
                    at_ns: n.gauge_at_ns,
                }),
        );
    }
}

#[cfg(test)]
pub(super) fn record_bytes() -> (usize, usize) {
    (size_of::<Record>(), size_of::<Entry>())
}

#[cfg(test)]
pub(super) fn lost(rec: &RingRecorder) -> (u32, u32, u32, u32) {
    let inner = rec.lock();
    let l = inner.state.as_ref().map(|s| s.lost).unwrap_or_default();
    (l.names, l.depth, l.entries, l.outside_frame)
}
