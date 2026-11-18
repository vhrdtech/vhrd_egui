//! The reads of a [`RingRecorder`]: statistics, the breakdown by scope, causes, counters and chart series.

use std::ops::Range;
use std::time::Duration;

use super::{Acc, COUNTER, Inner, NameKind, RingRecorder, State, TOP, ns};
use crate::perf::{
    Bin, Breakdown, CauseCount, CounterStats, FrameSeries, Histogram, IDLE_AFTER, ScopeKind,
    ScopeStats, Series, Stats,
};

impl State {
    pub(super) fn stats(&self, now: u64, window: Duration, inner: &Inner) -> Stats {
        let t0 = self.t0(now, window, inner.settings.retention);
        let (mut cpu, mut gap) = (Histogram::new(), Histogram::new());
        let (mut frames, mut dropped) = (0u32, 0u32);
        for (seq, _) in self.window(t0) {
            let Some(r) = self.records.get(seq) else {
                continue;
            };
            frames += u32::from(r.frames);
            dropped += u32::from(r.dropped);
            add_spread(
                &mut cpu,
                r.cpu_sum_ns,
                r.cpu_max_ns,
                self.cpu_frames(seq, r),
            );
            add_spread(&mut gap, r.gap_sum_ns, r.gap_max_ns, u32::from(r.smooth));
        }
        let span = self.first_ns.map_or(0, |f| now.saturating_sub(f.max(t0)));
        #[expect(
            clippy::cast_precision_loss,
            reason = "a frame count and a span in nanoseconds, shown with 3 digits"
        )]
        let fps = if span > 0 {
            f64::from(frames) * 1e9 / span as f64
        } else {
            0.0
        };
        let (budget, refresh_hz) = inner.budget_ns(self.measured_hz);
        let idle = match (self.in_frame, self.last_end_ns) {
            (false, Some(end)) => Some(Duration::from_nanos(now.saturating_sub(end)))
                .filter(|idle| *idle > IDLE_AFTER),
            _ => None,
        };
        #[expect(
            clippy::cast_possible_truncation,
            reason = "frames per second fit an f32"
        )]
        let fps = fps as f32;
        Stats {
            window,
            frames,
            fps,
            interval: gap.summary(self.last_gap_ns),
            cpu: cpu.summary(self.last_cpu_ns),
            dropped,
            budget: Duration::from_nanos(budget),
            refresh_hz,
            idle,
        }
    }

    pub(super) fn breakdown(&mut self, t0: u64, out: &mut Breakdown) {
        out.scopes.clear();
        let mut acc = std::mem::take(&mut self.acc);
        acc.clear();
        acc.resize(self.names.len(), Acc::default());
        let mut rest = Histogram::new();
        let (mut cpu_total, mut rest_total) = (0u64, 0u64);
        let mut seqs = std::mem::take(&mut self.seqs);
        seqs.clear();
        seqs.extend(self.window(t0));
        for (seq, range) in &seqs {
            let Some(r) = self.records.get(*seq) else {
                continue;
            };
            let mut top = 0u64;
            for e in range.clone().filter_map(|s| self.entries.get(s)) {
                if e.parent == COUNTER {
                    continue;
                }
                let Some(a) = acc.get_mut(usize::from(e.id)) else {
                    continue;
                };
                a.total += u64::from(e.value);
                a.seen = true;
                if e.parent == TOP {
                    a.top_level = true;
                    if self.names[usize::from(e.id)].kind == NameKind::Scope(ScopeKind::Counted) {
                        top += u64::from(e.value);
                    }
                } else if self.names[usize::from(e.id)].kind == NameKind::Scope(ScopeKind::Counted)
                    && let Some(p) = acc.get_mut(usize::from(e.parent))
                {
                    p.children += u64::from(e.value);
                }
            }
            let frames = self.cpu_frames(*seq, r);
            let cpu = u64::from(r.cpu_sum_ns);
            let r_rest = cpu.saturating_sub(top);
            cpu_total += cpu;
            rest_total += r_rest;
            if frames > 0 {
                rest.add_n(r_rest / u64::from(frames), frames);
            }
        }
        let last_rest = seqs
            .first()
            .and_then(|(seq, range)| self.series_value(*seq, range.clone(), Series::Rest, None))
            .map_or(0, |(mean, _, _)| mean);
        for (id, a) in acc.iter().enumerate() {
            if !a.seen {
                continue;
            }
            let NameKind::Scope(kind) = self.names[id].kind else {
                continue;
            };
            let id16 = u16::try_from(id).unwrap_or(u16::MAX);
            let mut h = Histogram::new();
            let mut last = None;
            for (seq, range) in &seqs {
                if let Some((mean, _, n)) =
                    self.series_value(*seq, range.clone(), Series::Scope(""), Some(id16))
                {
                    last.get_or_insert(mean);
                    h.add_n(mean, n);
                }
            }
            #[expect(clippy::cast_precision_loss, reason = "a share shown with 2 digits")]
            let share = match (kind, cpu_total) {
                (ScopeKind::Counted, c) if c > 0 => a.total as f32 / c as f32,
                _ => 0.0,
            };
            out.scopes.push(ScopeStats {
                name: self.names[id].name,
                kind,
                time: h.summary(last.unwrap_or(0)),
                total: Duration::from_nanos(a.total),
                self_total: Duration::from_nanos(a.total.saturating_sub(a.children)),
                share,
                top_level: a.top_level,
            });
        }
        out.scopes.sort_by_key(|s| std::cmp::Reverse(s.total));
        out.rest = rest.summary(last_rest);
        out.rest_total = Duration::from_nanos(rest_total);
        self.acc = acc;
        self.seqs = seqs;
    }

    /// One record's mean, largest value and frames of `series` (for a scope: `scope`, its id); `None` when it
    /// has none.
    pub(super) fn series_value(
        &self,
        seq: u64,
        entries: Range<u64>,
        series: Series,
        scope: Option<u16>,
    ) -> Option<(u64, u64, u32)> {
        let r = self.records.get(seq)?;
        let frames = self.cpu_frames(seq, r);
        let entries = || entries.clone().filter_map(|s| self.entries.get(s));
        let (sum, max, n) = match series {
            Series::Cpu => (u64::from(r.cpu_sum_ns), u64::from(r.cpu_max_ns), frames),
            Series::Interval => (
                u64::from(r.gap_sum_ns),
                u64::from(r.gap_max_ns),
                u32::from(r.smooth),
            ),
            Series::Scope(_) => {
                let id = scope?;
                let v: u64 = entries()
                    .filter(|e| e.id == id && e.parent != COUNTER)
                    .map(|e| u64::from(e.value))
                    .sum();
                if v == 0 {
                    return None;
                }
                (v, v, u32::from(r.frames))
            }
            Series::Rest => {
                let top: u64 = entries()
                    .filter(|e| {
                        e.parent == TOP
                            && self.names[usize::from(e.id)].kind
                                == NameKind::Scope(ScopeKind::Counted)
                    })
                    .map(|e| u64::from(e.value))
                    .sum();
                let sum = u64::from(r.cpu_sum_ns).saturating_sub(top);
                (sum, sum, frames)
            }
        };
        (n > 0).then(|| (sum / u64::from(n), max.max(sum / u64::from(n)), n))
    }

    /// The repaint causes of the records from `t0` on, into `out`, most frequent first.
    pub(super) fn causes(&self, t0: u64, out: &mut Vec<CauseCount>) {
        let st = self;
        let mut counts = [0u32; 16];
        for (seq, _) in st.window(t0) {
            if let Some(r) = st.records.get(seq) {
                for (bit, c) in counts.iter_mut().enumerate() {
                    if r.causes & (1 << bit) != 0 {
                        *c += 1;
                    }
                }
            }
        }
        for (slot, &frames) in st.causes.iter().zip(&counts) {
            if frames > 0 {
                out.push(CauseCount {
                    label: slot.label.clone(),
                    frames,
                });
            }
        }
        if counts[15] > 0 {
            out.push(CauseCount {
                label: "other causes".to_owned(),
                frames: counts[15],
            });
        }
        out.sort_by_key(|c| std::cmp::Reverse(c.frames));
    }

    /// The counters of the records from `t0` on, into `out`.
    pub(super) fn counters(&self, t0: u64, out: &mut Vec<CounterStats>) {
        let st = self;
        for (_, range) in st.window(t0) {
            for e in range.filter_map(|s| st.entries.get(s)) {
                if e.parent != COUNTER {
                    continue;
                }
                let name = st.names[usize::from(e.id)].name;
                let v = u64::from(e.value);
                match out.iter_mut().find(|c| std::ptr::eq(c.name, name)) {
                    Some(c) => {
                        c.total += v;
                        c.worst = c.worst.max(v);
                        c.frames += 1;
                    }
                    None => out.push(CounterStats {
                        name,
                        total: v,
                        worst: v,
                        frames: 1,
                    }),
                }
            }
        }
    }
}

/// Count `n` frames whose values sum to `sum` with the largest `max`: the largest as itself, the rest as their
/// mean. Exact when `n` is 1.
fn add_spread(h: &mut Histogram, sum: u32, max: u32, n: u32) {
    match n {
        0 => {}
        1 => h.add(u64::from(sum)),
        _ => {
            h.add(u64::from(max));
            let rest = u64::from(sum.saturating_sub(max)) / u64::from(n - 1);
            h.add_n(rest, n - 1);
        }
    }
}

impl FrameSeries for RingRecorder {
    fn bins(&self, series: Series, t0_ns: u64, t1_ns: u64, out: &mut [Bin]) {
        out.fill(Bin::default());
        let inner = self.lock();
        let Some(st) = &inner.state else {
            return;
        };
        if out.is_empty() || t1_ns <= t0_ns {
            return;
        }
        let scope = match series {
            Series::Scope(name) => st
                .names
                .iter()
                .position(|n| {
                    n.kind != NameKind::Counter && n.kind != NameKind::Gauge && n.name == name
                })
                .and_then(|i| u16::try_from(i).ok()),
            _ => None,
        };
        let span = t1_ns - t0_ns;
        let cols = out.len() as u64;
        for (seq, range) in st.window(t0_ns) {
            let Some(r) = st.records.get(seq) else {
                continue;
            };
            if r.start_ns >= t1_ns {
                continue;
            }
            let Some((mean, max, n)) = st.series_value(seq, range, series, scope) else {
                continue;
            };
            let col = usize::try_from(
                ((r.start_ns - t0_ns) as u128 * u128::from(cols) / u128::from(span)) as u64,
            )
            .unwrap_or(usize::MAX)
            .min(out.len() - 1);
            let b = &mut out[col];
            let (mean_d, max_d) = (Duration::from_nanos(mean), Duration::from_nanos(max));
            if b.count == 0 {
                b.min = mean_d;
                b.max = max_d;
                b.mean = Duration::ZERO;
            } else {
                b.min = b.min.min(mean_d);
                b.max = b.max.max(max_d);
            }
            // Running mean weighted by frames.
            let total = u64::from(b.count) + u64::from(n);
            let prev = ns(b.mean) * u64::from(b.count);
            b.mean = Duration::from_nanos((prev + mean * u64::from(n)) / total.max(1));
            b.count = u32::try_from(total).unwrap_or(u32::MAX);
        }
    }
}
