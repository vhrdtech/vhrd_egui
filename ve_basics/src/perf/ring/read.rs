//! The reads of a [`RingRecorder`]: statistics, the breakdown by scope, causes, counters and chart series.

use std::ops::Range;
use std::time::Duration;

use super::{Acc, COUNTER, IDLE_AFTER, Inner, NameKind, Record, RingRecorder, State, TOP};
use crate::perf::{
    Bin, Breakdown, CauseCount, CounterStats, FrameSeries, Histogram, ScopeId, ScopeKind,
    ScopeStats, Series, Span, Stats, Summary,
};

/// Count `n` frames whose values sum to `sum` with the largest `max`: the largest as itself, the rest as their
/// mean. Exact when `n` is 1.
fn add_spread(h: &mut Histogram, sum: u64, max: u64, n: u32) {
    match n {
        0 => {}
        1 => h.add(sum),
        _ => {
            h.add(max);
            h.add_n(sum.saturating_sub(max) / u64::from(n - 1), n - 1);
        }
    }
}

/// Fold a name's sum over its current record into its histogram.
fn flush(a: &mut Acc, h: &mut Histogram) {
    if a.cur_seq.take().is_some() && a.cur_frames > 0 {
        let mean = a.cur / u64::from(a.cur_frames);
        h.add_n(mean, a.cur_frames);
        a.last.get_or_insert(mean);
    }
}

impl State {
    fn counted(&self, id: u16) -> bool {
        self.names[usize::from(id)].kind == NameKind::Scope(ScopeKind::Counted)
    }

    /// One record's `series`: the sum over its frames, the largest of them and how many there are.
    fn series_value(
        &self,
        r: &Record,
        entries: Option<Range<u64>>,
        series: Series,
    ) -> Option<(u64, u64, u32)> {
        let frames = u32::from(r.frames);
        let held = entries.is_some();
        let entries = || {
            entries
                .clone()
                .into_iter()
                .flatten()
                .filter_map(|s| self.entries.get(s))
        };
        let (sum, max, n) = match series {
            Series::Cpu => (u64::from(r.cpu_sum_ns), u64::from(r.cpu_max_ns), frames),
            Series::Interval => (
                u64::from(r.gap_sum_ns),
                u64::from(r.gap_max_ns),
                u32::from(r.smooth),
            ),
            Series::Scope(ScopeId(id)) => {
                if !held {
                    return None;
                }
                let mut found = false;
                let sum: u64 = entries()
                    .filter(|e| u32::from(e.id) == id && e.parent != COUNTER)
                    .map(|e| {
                        found = true;
                        u64::from(e.value)
                    })
                    .sum();
                if !found {
                    return None;
                }
                (sum, sum / u64::from(frames.max(1)), frames)
            }
            Series::Rest => {
                if !held {
                    return None;
                }
                let top: u64 = entries()
                    .filter(|e| e.parent == TOP && self.counted(e.id))
                    .map(|e| u64::from(e.value))
                    .sum();
                let sum = u64::from(r.cpu_sum_ns).saturating_sub(top);
                (sum, sum / u64::from(frames.max(1)), frames)
            }
        };
        (n > 0).then_some((sum, max, n))
    }

    pub(super) fn stats(&self, now: u64, span: Span, inner: &Inner) -> Stats {
        let (mut cpu, mut gap) = (Histogram::new(), Histogram::new());
        let (mut frames, mut dropped, mut pauses) = (0u32, 0u32, 0u32);
        let (mut oldest, mut newest) = (None, None);
        for (_, r, _) in self.window(span) {
            newest.get_or_insert(*r);
            oldest = Some(r.start_ns);
            frames += u32::from(r.frames);
            dropped += u32::from(r.dropped);
            pauses += u32::from(r.paused());
            add_spread(
                &mut cpu,
                u64::from(r.cpu_sum_ns),
                u64::from(r.cpu_max_ns),
                u32::from(r.frames),
            );
            add_spread(
                &mut gap,
                u64::from(r.gap_sum_ns),
                u64::from(r.gap_max_ns),
                u32::from(r.smooth),
            );
        }
        let covered = Span {
            from_ns: oldest.map_or(span.to_ns, |o| span.from_ns.max(o.min(span.to_ns))),
            to_ns: span.to_ns.min(now.saturating_add(1)),
        };
        // The newest frame exactly when the span reaches it, else its record's mean.
        let reaches_now = self.last_end_ns.is_some_and(|e| e < span.to_ns);
        let mean = |sum: u32, n: u8| u64::from(sum) / u64::from(n.max(1));
        let (last_cpu, last_gap) = match (reaches_now, newest) {
            (true, _) => (self.last_cpu_ns, self.last_gap_ns),
            (false, Some(r)) => (mean(r.cpu_sum_ns, r.frames), mean(r.gap_sum_ns, r.smooth)),
            (false, None) => (0, 0),
        };
        let len = covered.len().as_secs_f64();
        #[expect(
            clippy::cast_possible_truncation,
            reason = "frames per second fit an f32"
        )]
        let fps = if len > 0.0 {
            (f64::from(frames) / len) as f32
        } else {
            0.0
        };
        let (budget, refresh_hz) = inner.budget(self.measured_hz);
        let idle = match (self.open, self.last_end_ns) {
            (None, Some(end)) => Some(Duration::from_nanos(now.saturating_sub(end)))
                .filter(|idle| *idle > IDLE_AFTER),
            _ => None,
        };
        Stats {
            span,
            covered,
            frames,
            fps,
            interval: gap.summary(last_gap),
            cpu: cpu.summary(last_cpu),
            dropped,
            pauses,
            budget: Duration::from_nanos(budget),
            refresh_hz,
            idle,
        }
    }

    pub(super) fn breakdown(&mut self, span: Span, out: &mut Breakdown) {
        out.scopes.clear();
        let mut acc = std::mem::take(&mut self.acc);
        let mut hists = std::mem::take(&mut self.hists);
        acc.clear();
        acc.resize(self.names.len(), Acc::default());
        hists.resize_with(self.names.len(), Histogram::new);
        let mut rest = Histogram::new();
        let (mut cpu_total, mut rest_total, mut last_rest) = (0u64, 0u64, None);
        let mut covered_from = span.to_ns;
        for (record, r, entries) in self.window(span) {
            let Some(entries) = entries else {
                break;
            };
            covered_from = r.start_ns;
            let frames = u32::from(r.frames);
            if frames == 0 {
                continue;
            }
            let mut top = 0u64;
            for e in entries.filter_map(|s| self.entries.get(s)) {
                if e.parent == COUNTER {
                    continue;
                }
                let id = usize::from(e.id);
                let v = u64::from(e.value);
                if e.parent == TOP {
                    if self.counted(e.id) {
                        top += v;
                    }
                } else if self.counted(e.id)
                    && let Some(p) = acc.get_mut(usize::from(e.parent))
                {
                    p.children += v;
                }
                let (Some(a), Some(h)) = (acc.get_mut(id), hists.get_mut(id)) else {
                    continue;
                };
                if !a.seen {
                    a.seen = true;
                    h.clear();
                }
                a.total += v;
                a.top_level |= e.parent == TOP;
                if a.cur_seq == Some(record) {
                    a.cur += v;
                } else {
                    flush(a, h);
                    a.cur_seq = Some(record);
                    a.cur = v;
                    a.cur_frames = frames;
                }
            }
            let cpu = u64::from(r.cpu_sum_ns);
            let r_rest = cpu.saturating_sub(top);
            cpu_total += cpu;
            rest_total += r_rest;
            last_rest.get_or_insert(r_rest / u64::from(frames));
            rest.add_n(r_rest / u64::from(frames), frames);
        }
        for (id, (a, h)) in acc.iter_mut().zip(&mut hists).enumerate() {
            if !a.seen {
                continue;
            }
            flush(a, h);
            let name = &self.names[id];
            let NameKind::Scope(kind) = name.kind else {
                continue;
            };
            #[expect(clippy::cast_precision_loss, reason = "a share shown with 2 digits")]
            let share = match (kind, cpu_total) {
                (ScopeKind::Counted, c) if c > 0 => a.total as f32 / c as f32,
                _ => 0.0,
            };
            out.scopes.push(ScopeStats {
                id: ScopeId(u32::try_from(id).unwrap_or(u32::MAX)),
                name: name.shared.clone(),
                instance: name.key.instance,
                kind,
                time: h.summary(a.last.unwrap_or(0)),
                total: Duration::from_nanos(a.total),
                self_total: Duration::from_nanos(a.total.saturating_sub(a.children)),
                share,
                top_level: a.top_level,
            });
        }
        out.scopes.sort_by_key(|s| std::cmp::Reverse(s.total));
        out.covered = Span {
            from_ns: covered_from,
            to_ns: span.to_ns,
        };
        out.rest = rest.summary(last_rest.unwrap_or(0));
        out.rest_total = Duration::from_nanos(rest_total);
        self.acc = acc;
        self.hists = hists;
    }

    /// The repaint causes of the records in `span`, into `out`, most frequent first.
    pub(super) fn causes(&self, span: Span, out: &mut Vec<CauseCount>) {
        let mut counts = [0u32; 16];
        for (_, r, _) in self.window(span) {
            for (bit, c) in counts.iter_mut().enumerate() {
                if r.causes & (1 << bit) != 0 {
                    *c += 1;
                }
            }
        }
        for (slot, &frames) in self.causes.iter().zip(&counts) {
            if frames > 0 {
                out.push(CauseCount {
                    label: slot.label.clone(),
                    frames,
                });
            }
        }
        if counts[15] > 0 {
            out.push(CauseCount {
                label: "other causes".into(),
                frames: counts[15],
            });
        }
        out.sort_by_key(|c| std::cmp::Reverse(c.frames));
    }

    /// The counters of the records in `span` whose entries are held, into `out`.
    pub(super) fn counters(&self, span: Span, out: &mut Vec<CounterStats>) {
        for (_, _, entries) in self.window(span) {
            let Some(entries) = entries else {
                break;
            };
            for e in entries.filter_map(|s| self.entries.get(s)) {
                if e.parent != COUNTER {
                    continue;
                }
                let name = &self.names[usize::from(e.id)].shared;
                let v = u64::from(e.value);
                match out
                    .iter_mut()
                    .find(|c| std::sync::Arc::ptr_eq(&c.name, name))
                {
                    Some(c) => {
                        c.total += v;
                        c.worst = c.worst.max(v);
                        c.frames += 1;
                    }
                    None => out.push(CounterStats {
                        name: name.clone(),
                        total: v,
                        worst: v,
                        frames: 1,
                    }),
                }
            }
        }
    }

    fn bins(&self, series: Series, span: Span, out: &mut [Bin]) {
        let len = span.to_ns.saturating_sub(span.from_ns);
        if out.is_empty() || len == 0 {
            return;
        }
        let cols = out.len() as u64;
        let mut h = Histogram::new();
        let mut col = None;
        for (_, r, entries) in self.window(span) {
            let Some((sum, max, n)) = self.series_value(r, entries, series) else {
                continue;
            };
            // Below `cols`, a usize.
            let c = (u128::from(r.start_ns - span.from_ns) * u128::from(cols) / u128::from(len))
                as usize;
            if col != Some(c) {
                if let Some(prev) = col {
                    finish(&mut out[prev], &h);
                }
                h.clear();
                col = Some(c);
            }
            add_spread(&mut h, sum, max, n);
            let b = &mut out[c];
            let mean = sum / u64::from(n);
            let (mean_d, max_d) = (Duration::from_nanos(mean), Duration::from_nanos(max));
            b.min = if b.count == 0 {
                mean_d
            } else {
                b.min.min(mean_d)
            };
            b.max = b.max.max(max_d);
            b.count = b.count.saturating_add(n);
        }
        if let Some(c) = col {
            finish(&mut out[c], &h);
        }
    }

    fn histogram(&self, series: Series, span: Span, out: &mut Histogram) {
        for (_, r, entries) in self.window(span) {
            if let Some((sum, max, n)) = self.series_value(r, entries, series) {
                add_spread(out, sum, max, n);
            }
        }
    }
}

/// Mean, median and p99 of a finished column from its histogram.
fn finish(b: &mut Bin, h: &Histogram) {
    let s: Summary = h.summary(0);
    b.mean = s.mean;
    b.median = s.median;
    b.p99 = s.p99;
}

impl FrameSeries for RingRecorder {
    fn bins(&self, series: Series, span: Span, out: &mut [Bin]) {
        out.fill(Bin::default());
        if let Some(st) = &self.lock().state {
            st.bins(series, span, out);
        }
    }

    fn histogram(&self, series: Series, span: Span, out: &mut Histogram) {
        out.clear();
        if let Some(st) = &self.lock().state {
            st.histogram(series, span, out);
        }
    }
}
