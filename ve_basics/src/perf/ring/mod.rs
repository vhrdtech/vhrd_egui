//! The default recorder: fixed rings of frame records and scope entries.

mod read;

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};
use std::ops::Range;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use super::{
    Breakdown, BudgetSource, Cause, CauseCount, Clock, CounterStats, FrameBegin, FrameEnd,
    FrameRecorder, GaugeValue, Granularity, Health, ScopeKind, ScopeName, ScopeToken, Settings,
    SettingsError, Span, Stats, SystemClock, snap_refresh_hz,
};

/// Distinct names (scopes and their instances, counters, gauges) a recorder keeps; further ones are not
/// recorded.
pub const NAMES_MAX: usize = 256;
/// Repaint causes a recorder tells apart at once; one no longer in its history makes room for a new one,
/// further ones count as "other causes".
pub const CAUSES_MAX: usize = 15;
/// Scopes open inside each other at most; deeper ones are not recorded.
pub const SCOPE_DEPTH_MAX: usize = 32;
/// Scope and counter entries one record holds at most; further ones are not recorded.
pub const ENTRIES_PER_RECORD_MAX: usize = 1024;
/// Frames one aggregate holds at most (a coarse granularity at a high refresh rate starts a new one).
pub const FRAMES_PER_RECORD_MAX: u8 = u8::MAX;
/// Records a ring holds at most, whatever the retention: 128 MiB of them.
pub const RECORDS_MAX: usize = 1 << 22;
/// Scope and counter entries a ring holds at most: 128 MiB of them.
pub const ENTRIES_MAX: usize = 1 << 24;
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
/// Token flag: a scope inside one of the same name, whose time is the outer one's.
const RECURSIVE: u16 = 2;
/// `Record::entries` bit: its first frame came after a pause.
const PAUSED: u16 = 1 << 15;
const _: () = assert!(
    CAUSES_MAX < 16,
    "causes are bits of a u16 with the top bit for other causes"
);
const _: () = assert!(
    NAMES_MAX < COUNTER as usize,
    "name ids stay below the parent markers"
);
const _: () = assert!(
    ENTRIES_PER_RECORD_MAX < PAUSED as usize,
    "a record's entry count leaves its top bit to the pause flag"
);

/// How a [`RingRecorder`] sizes its rings for its retention.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sizing {
    /// The highest frame rate the retention holds at; faster, the oldest frames go sooner.
    pub rate_hz: u32,
    /// Bytes for scope and counter entries (8 each); more scopes a frame than they hold for the retention, and
    /// scope history is shorter than the frames' ([`Breakdown::covered`] says how long).
    pub entries_bytes: usize,
}

impl Default for Sizing {
    fn default() -> Self {
        Self {
            rate_hz: 120,
            // Two entries a frame for 5 min at 120 Hz.
            entries_bytes: 576_000,
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

    /// Scope and counter entries: [`Self::entries_bytes`] worth, within two records' limit and [`ENTRIES_MAX`].
    pub fn entries(&self) -> usize {
        (self.entries_bytes / size_of::<Entry>()).clamp(2 * ENTRIES_PER_RECORD_MAX, ENTRIES_MAX)
    }

    /// Bytes the rings take when full.
    pub fn bytes(&self, settings: &Settings) -> usize {
        self.records(settings) * size_of::<Record>() + self.entries() * size_of::<Entry>()
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
    /// Its scope and counter entries (the newest this many before the next record's first), and [`PAUSED`].
    entries: u16,
    /// Its ended frames.
    frames: u8,
    /// Its continuous frames: those with an interval in `gap_*`.
    smooth: u8,
}

impl Record {
    fn entry_count(&self) -> u16 {
        self.entries & !PAUSED
    }

    fn paused(&self) -> bool {
        self.entries & PAUSED != 0
    }
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

    fn newest(&self) -> Option<&T> {
        self.total.checked_sub(1).and_then(|s| self.get(s))
    }
}

/// A name's key in the index: its text's address and length, its instance and kind.
type NameAddr = (usize, usize, Option<u64>, NameKind);

/// The name index's hasher: a multiply and rotate per word. Its keys are addresses and ids from the program
/// itself, never outside input, so the flooding protection of the default hasher buys nothing and costs a
/// quarter of a scope.
#[derive(Default)]
struct AddrHasher(u64);

impl Hasher for AddrHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.write_u64(u64::from(b));
        }
    }

    fn write_u64(&mut self, n: u64) {
        self.0 = (self.0.rotate_left(5) ^ n).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }

    fn write_usize(&mut self, n: usize) {
        self.write_u64(n as u64);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum NameKind {
    Scope(ScopeKind),
    Counter,
    Gauge,
}

#[derive(Clone, Debug)]
struct Name {
    key: ScopeName,
    kind: NameKind,
    shared: Arc<str>,
    gauge: u64,
    gauge_at_ns: u64,
    /// The record, entry and parent of its newest entry, so a repeat within a record adds to it.
    last_entry: Option<(u64, u64, u16)>,
}

#[derive(Clone, Debug)]
struct CauseSlot {
    file: &'static str,
    line: u32,
    label: Arc<str>,
    /// The newest record it was seen in; once that is gone, the slot may go to another cause.
    last_record: u64,
}

/// The frame whose CPU time the host may still report ([`FrameRecorder::previous_frame_cpu`]).
#[derive(Clone, Copy, Debug)]
struct Prev {
    record: u64,
    measured_ns: u64,
    not_counted_ns: u64,
    newest: bool,
}

/// The frame in progress: what its begin found, booked at its end.
#[derive(Clone, Copy, Debug)]
struct Open {
    start_ns: u64,
    gap_ns: u64,
    smooth: bool,
    pause: bool,
    dropped: u64,
}

/// Per-name sums of a breakdown read, reused between reads.
#[derive(Clone, Copy, Debug, Default)]
struct Acc {
    total: u64,
    children: u64,
    top_level: bool,
    seen: bool,
    /// The record being summed, its sum and frames, before it goes into the histogram.
    cur_seq: Option<u64>,
    cur: u64,
    cur_frames: u32,
    last: Option<u64>,
}

thread_local! {
    // One byte per thread whose address tells threads apart; owned by `thread_mark`. Cheaper than
    // `thread::current()`, which clones an `Arc` on every call.
    static MARK: u8 = const { 0 };
}

/// An id of the calling thread, unique among live threads.
fn thread_mark() -> usize {
    MARK.with(|m| std::ptr::from_ref(m) as usize)
}

fn bump(n: &mut u32) {
    *n = n.saturating_add(1);
}

/// The history and the frame in progress; exists only while recording.
#[derive(Debug)]
struct State {
    /// Which history this is; part of every [`super::ScopeId`] it hands out.
    generation: u32,
    records: Ring<Record>,
    entries: Ring<Entry>,
    names: Vec<Name>,
    /// Name ids by the static name's address, instance and kind; a name with the same text at another address
    /// is found by text once and then cached by its address too.
    index: HashMap<NameAddr, u16, BuildHasherDefault<AddrHasher>>,
    causes: Vec<CauseSlot>,
    stack: [u16; SCOPE_DEPTH_MAX],
    /// Per open scope: time of the not-counted scopes inside it so far, taken out of its own time.
    not_counted_inside: [u64; SCOPE_DEPTH_MAX],
    depth: usize,
    open: Option<Open>,
    frame_thread: Option<usize>,
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
    health: Health,
    acc: Vec<Acc>,
    hists: Vec<super::Histogram>,
    #[cfg(feature = "alloc-count")]
    alloc_at_begin: Option<(u64, u64)>,
}

#[derive(Debug)]
struct Inner {
    settings: Settings,
    sizing: Sizing,
    display_hz: Option<f32>,
    state: Option<State>,
    /// Bumped for every new history, so a [`super::ScopeId`] of an older one names nothing.
    generation: u32,
}

impl Inner {
    /// Start an empty history.
    fn fresh(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.state = Some(State::new(&self.settings, &self.sizing, self.generation));
    }
}

/// The default [`FrameRecorder`]: rings sized from the retention ([`Sizing`]), allocated when recording turns
/// on and dropped when it turns off; nothing allocates per frame once the name and cause tables hold their
/// entries.
pub struct RingRecorder {
    enabled: AtomicBool,
    clock: Arc<dyn Clock>,
    inner: Mutex<Inner>,
}

impl RingRecorder {
    /// A recording recorder on the system clock; `Err` for settings out of range.
    pub fn new(settings: Settings) -> Result<Self, SettingsError> {
        Self::with_clock(settings, SystemClock::default())
    }

    /// A recording recorder on `clock`.
    pub fn with_clock(
        settings: Settings,
        clock: impl Clock + 'static,
    ) -> Result<Self, SettingsError> {
        Self::with_sizing(settings, Sizing::default(), clock)
    }

    /// A recording recorder on `clock`, its rings sized by `sizing`.
    pub fn with_sizing(
        settings: Settings,
        sizing: Sizing,
        clock: impl Clock + 'static,
    ) -> Result<Self, SettingsError> {
        settings.validate()?;
        Ok(Self {
            enabled: AtomicBool::new(true),
            clock: Arc::new(clock),
            inner: Mutex::new(Inner {
                settings,
                sizing,
                display_hz: None,
                state: Some(State::new(&settings, &sizing, 0)),
                generation: 0,
            }),
        })
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

/// The file's last two parts: `src/anim.rs` of `/home/x/app/src/anim.rs`.
fn short_file(file: &str) -> &str {
    let mut cut = file.len();
    for _ in 0..2 {
        match file[..cut].rfind(['/', '\\']) {
            Some(i) => cut = i,
            None => return file,
        }
    }
    &file[cut + 1..]
}

impl Inner {
    fn budget(&self, measured_hz: Option<f32>) -> (u64, f32) {
        let hz = match self.settings.budget {
            BudgetSource::Hz(hz) => hz,
            BudgetSource::Display => self
                .display_hz
                .or(measured_hz)
                .unwrap_or(super::BUDGET_FALLBACK_HZ),
        };
        (ns(super::budget_at(hz)), hz)
    }
}

impl State {
    fn new(settings: &Settings, sizing: &Sizing, generation: u32) -> Self {
        Self {
            generation,
            records: Ring::new(sizing.records(settings)),
            entries: Ring::new(sizing.entries()),
            names: Vec::with_capacity(32),
            index: HashMap::with_capacity_and_hasher(64, BuildHasherDefault::default()),
            causes: Vec::with_capacity(CAUSES_MAX),
            stack: [0; SCOPE_DEPTH_MAX],
            not_counted_inside: [0; SCOPE_DEPTH_MAX],
            depth: 0,
            open: None,
            frame_thread: None,
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
            health: Health::default(),
            acc: Vec::new(),
            hists: Vec::new(),
            #[cfg(feature = "alloc-count")]
            alloc_at_begin: None,
        }
    }

    fn intern(&mut self, key: ScopeName, kind: NameKind) -> Option<u16> {
        let by_addr = (
            key.name.as_ptr() as usize,
            key.name.len(),
            key.instance,
            kind,
        );
        if let Some(&id) = self.index.get(&by_addr) {
            return Some(id);
        }
        let by_text = self
            .names
            .iter()
            .position(|n| n.kind == kind && n.key == key)
            .and_then(|i| u16::try_from(i).ok());
        let id = match by_text {
            Some(id) => id,
            None if self.names.len() < NAMES_MAX => {
                self.names.push(Name {
                    key,
                    kind,
                    shared: key.name.into(),
                    gauge: 0,
                    gauge_at_ns: 0,
                    last_entry: None,
                });
                u16::try_from(self.names.len() - 1).ok()?
            }
            None => {
                bump(&mut self.health.lost_names);
                return None;
            }
        };
        self.index.insert(by_addr, id);
        Some(id)
    }

    fn cause_bit(&mut self, cause: &Cause<'_>) -> u16 {
        let current = self.records.total.saturating_sub(1);
        let found = self.causes.iter().position(|c| {
            c.line == cause.line
                && c.file == cause.file
                && (!cause.file.is_empty() || &*c.label == cause.reason)
        });
        let free = || {
            // A slot no held record names any more.
            self.causes
                .iter()
                .position(|c| c.last_record < self.records.oldest())
        };
        let i = match found {
            Some(i) => i,
            None => {
                let label: Arc<str> = if cause.reason.is_empty() {
                    format!("{}:{}", short_file(cause.file), cause.line).into()
                } else {
                    cause.reason.into()
                };
                let slot = CauseSlot {
                    file: cause.file,
                    line: cause.line,
                    label,
                    last_record: current,
                };
                if self.causes.len() < CAUSES_MAX {
                    self.causes.push(slot);
                    self.causes.len() - 1
                } else if let Some(i) = free() {
                    self.causes[i] = slot;
                    i
                } else {
                    bump(&mut self.health.lost_causes);
                    return OTHER_CAUSE;
                }
            }
        };
        self.causes[i].last_record = current;
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
        let (_, p10, _) = gaps.select_nth_unstable(n / 10);
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
        if self.open.take().is_some() {
            // The host began a frame without ending the last: that one is not booked.
            bump(&mut self.health.abandoned_frames);
        }
        let gap = self.last_start_ns.map(|s| now.saturating_sub(s));
        let continuous = begin.continuous && gap.is_some();
        let gap = gap.unwrap_or(0);
        // Nothing ran past the idle line: the window was hidden or the app idle. A long frame is not a pause.
        let quiet = self.last_end_ns.map(|e| now.saturating_sub(e));
        let pause = continuous && quiet.is_some_and(|q| q > ns(settings.idle_after));
        let smooth = continuous && !pause;
        let mut dropped = 0;
        if smooth {
            self.gaps[self.gaps_total % GAPS_KEPT] = sat32(gap);
            self.gaps_total += 1;
            if self.gaps_total.is_multiple_of(GAPS_REMEASURE) {
                self.remeasure();
            }
            dropped = ((gap + budget_ns / 2) / budget_ns.max(1)).saturating_sub(1);
        }
        let new_record = pause
            || match (settings.granularity, self.records.newest()) {
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
        self.open = Some(Open {
            start_ns: now,
            gap_ns: gap,
            smooth,
            pause,
            dropped,
        });
        self.frame_thread = Some(thread_mark());
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
            self.alloc_at_begin = ve_alloc_count::totals();
        }
    }

    fn end(&mut self, now: u64, end: &FrameEnd<'_>) {
        let Some(open) = self.open.take() else {
            return;
        };
        let cpu = now
            .saturating_sub(open.start_ns)
            .saturating_sub(self.not_counted_ns);
        let mut bits = 0;
        for c in end.causes {
            bits |= self.cause_bit(c);
        }
        #[cfg(feature = "alloc-count")]
        if let (Some((n0, b0)), Some((n1, b1))) = (self.alloc_at_begin, ve_alloc_count::totals()) {
            for (name, v) in [("alloc.count", n1 - n0), ("alloc.bytes", b1 - b0)] {
                if let Some(id) = self.intern(name.into(), NameKind::Counter) {
                    self.add_entry(id, COUNTER, v);
                }
            }
        }
        if let Some(r) = self.records.newest_mut() {
            r.frames += 1;
            r.cpu_sum_ns = r.cpu_sum_ns.saturating_add(sat32(cpu));
            r.cpu_max_ns = r.cpu_max_ns.max(sat32(cpu));
            r.causes |= bits;
            r.dropped = r
                .dropped
                .saturating_add(u16::try_from(open.dropped).unwrap_or(u16::MAX));
            if open.smooth {
                r.smooth += 1;
                r.gap_sum_ns = r.gap_sum_ns.saturating_add(sat32(open.gap_ns));
                r.gap_max_ns = r.gap_max_ns.max(sat32(open.gap_ns));
                self.last_gap_ns = open.gap_ns;
            }
            if open.pause {
                r.entries |= PAUSED;
            }
        }
        self.prev = Some(Prev {
            record: self.records.total - 1,
            measured_ns: cpu,
            not_counted_ns: self.not_counted_ns,
            newest: true,
        });
        self.last_cpu_ns = cpu;
        self.last_end_ns = Some(now);
        self.depth = 0;
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
        if prev.newest || self.open.is_some() {
            self.last_cpu_ns = full;
        }
    }

    fn add_entry(&mut self, id: u16, parent: u16, value: u64) {
        let Some(newest) = self.records.total.checked_sub(1) else {
            return;
        };
        let name = &mut self.names[usize::from(id)];
        if let Some((record, seq, p)) = name.last_entry
            && record == newest
            && p == parent
            && let Some(e) = self.entries.get_mut(seq)
        {
            e.value = e.value.saturating_add(sat32(value));
            return;
        }
        let Some(r) = self.records.newest_mut() else {
            return;
        };
        if usize::from(r.entry_count()) >= ENTRIES_PER_RECORD_MAX {
            bump(&mut self.health.lost_entries);
            return;
        }
        r.entries += 1;
        name.last_entry = Some((newest, self.entries.total, parent));
        self.entries.push(Entry {
            value: sat32(value),
            id,
            parent,
        });
    }

    /// The records whose begin falls into `span`, newest first, with the sequence numbers of their entries
    /// when all of them are still held.
    fn window(&self, span: Span) -> impl Iterator<Item = (u64, &Record, Option<Range<u64>>)> + '_ {
        let oldest = self.entries.oldest();
        (self.records.oldest()..self.records.total)
            .rev()
            .scan(self.entries.total, move |end, seq| {
                let r = self.records.get(seq)?;
                let start = end.saturating_sub(u64::from(r.entry_count()));
                let entries = (start >= oldest).then_some(start..*end);
                *end = start;
                Some((seq, r, entries))
            })
            .skip_while(move |(_, r, _)| r.start_ns >= span.to_ns)
            .take_while(move |(_, r, _)| r.start_ns >= span.from_ns)
    }

    fn health(&self, now: u64) -> Health {
        let to_ns = now.saturating_add(1);
        let from = |r: Option<&Record>| r.map_or(to_ns, |r| r.start_ns);
        let oldest_held = self
            .window(Span { from_ns: 0, to_ns })
            .take_while(|(_, _, entries)| entries.is_some())
            .last()
            .map(|(_, r, _)| r);
        Health {
            frames_covered: Span {
                from_ns: from(self.records.get(self.records.oldest())),
                to_ns,
            },
            scopes_covered: Span {
                from_ns: from(oldest_held),
                to_ns,
            },
            ..self.health
        }
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
            inner.fresh();
        } else if !on {
            inner.state = None;
        }
    }

    fn settings(&self) -> Settings {
        self.lock().settings
    }

    fn set_settings(&self, settings: Settings) -> Result<(), SettingsError> {
        settings.validate()?;
        let mut inner = self.lock();
        let rebuild = settings.retention != inner.settings.retention
            || settings.granularity != inner.settings.granularity;
        inner.settings = settings;
        if rebuild && inner.state.is_some() {
            inner.fresh();
        }
        Ok(())
    }

    fn now_ns(&self) -> u64 {
        self.now()
    }

    fn clear(&self) {
        let mut inner = self.lock();
        if inner.state.is_some() {
            inner.fresh();
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
        let (budget, _) = inner.budget(measured_hz);
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
        self.lock().display_hz = hz.filter(|hz| super::BUDGET_HZ.contains(hz));
    }

    fn scope_begin(&self, name: ScopeName, kind: ScopeKind) -> Option<ScopeToken> {
        if !self.is_enabled() {
            return None;
        }
        let payload = {
            let mut inner = self.lock();
            let st = inner.state.as_mut()?;
            if st.open.is_none() {
                bump(&mut st.health.outside_frame);
                return None;
            }
            if st.frame_thread != Some(thread_mark()) {
                bump(&mut st.health.other_thread);
                return None;
            }
            if st.depth >= SCOPE_DEPTH_MAX {
                bump(&mut st.health.lost_depth);
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
            let mut flags = 0;
            if st.stack[..st.depth].contains(&id) {
                flags |= RECURSIVE;
            }
            if kind == ScopeKind::NotCounted {
                if st.not_counted_open == 0 {
                    flags |= OUTERMOST_NOT_COUNTED;
                }
                st.not_counted_open += 1;
            }
            let depth = st.depth;
            st.stack[depth] = id;
            st.not_counted_inside[depth] = 0;
            st.depth += 1;
            u64::from(id) | u64::from(parent) << 16 | (depth as u64) << 32 | u64::from(flags) << 48
        };
        Some(ScopeToken::new(self.now(), payload))
    }

    fn scope_end(&self, token: ScopeToken) {
        let now = self.now();
        let mut inner = self.lock();
        let Some(st) = &mut inner.state else {
            return;
        };
        let p = token.payload();
        // The fields scope_begin packed.
        let (id, parent) = ((p & 0xffff) as u16, ((p >> 16) & 0xffff) as u16);
        let (depth, flags) = (((p >> 32) & 0xffff) as usize, (p >> 48) as u16);
        let current = st.open.is_some_and(|o| token.start_ns() >= o.start_ns);
        if !current || depth >= st.depth || st.stack[depth] != id {
            bump(&mut st.health.stale_scopes);
            return;
        }
        // Scopes still open inside this one end unrecorded.
        let inner_open = st.depth - depth - 1;
        for _ in 0..inner_open {
            bump(&mut st.health.stale_scopes);
        }
        st.depth = depth;
        let dur = now.saturating_sub(token.start_ns());
        let inside = st.not_counted_inside[depth];
        let not_counted = st.names.get(usize::from(id)).map(|n| n.kind)
            == Some(NameKind::Scope(ScopeKind::NotCounted));
        // A counted scope keeps its counted time; a not-counted one shows all of its time on its own row.
        let (value, hidden) = if not_counted {
            st.not_counted_open = st.not_counted_open.saturating_sub(1);
            if flags & OUTERMOST_NOT_COUNTED != 0 {
                st.not_counted_ns += dur;
            }
            (dur, dur)
        } else {
            (dur.saturating_sub(inside), inside)
        };
        if let Some(outer) = depth.checked_sub(1) {
            st.not_counted_inside[outer] += hidden;
        }
        if flags & RECURSIVE == 0 {
            st.add_entry(id, parent, value);
        }
    }

    fn count(&self, name: &'static str, n: u64) {
        if !self.is_enabled() {
            return;
        }
        if let Some(st) = &mut self.lock().state
            && let Some(id) = st.intern(name.into(), NameKind::Counter)
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
            && let Some(id) = st.intern(name.into(), NameKind::Gauge)
        {
            let g = &mut st.names[usize::from(id)];
            g.gauge = value;
            g.gauge_at_ns = now;
        }
    }

    fn stats(&self, span: Span) -> Stats {
        let now = self.now();
        let inner = self.lock();
        match &inner.state {
            Some(st) => st.stats(now, span, &inner),
            None => Stats {
                span,
                ..Stats::default()
            },
        }
    }

    fn breakdown(&self, span: Span, out: &mut Breakdown) {
        let mut inner = self.lock();
        match &mut inner.state {
            Some(st) => st.breakdown(span, out),
            None => {
                out.scopes.clear();
                out.covered = Span::default();
                out.rest = super::Summary::default();
                out.rest_total = Duration::ZERO;
            }
        }
    }

    fn causes(&self, span: Span, out: &mut Vec<CauseCount>) {
        out.clear();
        if let Some(st) = &self.lock().state {
            st.causes(span, out);
        }
    }

    fn counters(&self, span: Span, out: &mut Vec<CounterStats>) {
        out.clear();
        if let Some(st) = &self.lock().state {
            st.counters(span, out);
        }
    }

    fn gauges(&self, out: &mut Vec<GaugeValue>) {
        out.clear();
        if let Some(st) = &self.lock().state {
            out.extend(
                st.names
                    .iter()
                    .filter(|n| n.kind == NameKind::Gauge)
                    .map(|n| GaugeValue {
                        name: n.shared.clone(),
                        value: n.gauge,
                        at_ns: n.gauge_at_ns,
                    }),
            );
        }
    }

    fn health(&self) -> Health {
        let now = self.now();
        match &self.lock().state {
            Some(st) => st.health(now),
            None => Health::default(),
        }
    }
}

#[cfg(test)]
pub(super) fn record_bytes() -> (usize, usize) {
    (size_of::<Record>(), size_of::<Entry>())
}

#[cfg(test)]
mod tests {
    #[test]
    fn files_are_cut_to_their_last_two_parts() {
        assert_eq!(super::short_file("/home/x/app/src/anim.rs"), "src/anim.rs");
        assert_eq!(super::short_file("src/anim.rs"), "src/anim.rs");
        assert_eq!(super::short_file("anim.rs"), "anim.rs");
        assert_eq!(super::short_file(r"C:\x\src\anim.rs"), r"src\anim.rs");
    }
}
