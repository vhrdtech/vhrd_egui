//! The recorder on a hand-moved clock: every number is exact, no wall clock decides an outcome.

use std::time::Duration;

use super::*;

const MS: Duration = Duration::from_millis(1);

fn recorder(settings: Settings) -> (Recorder, ManualClock) {
    let clock = ManualClock::new();
    (
        Recorder::new(RingRecorder::with_clock(settings, clock.clone())),
        clock,
    )
}

fn at(hz: f32) -> Settings {
    Settings {
        budget: BudgetSource::Hz(hz),
        ..Settings::default()
    }
}

/// One continuous frame: `cpu_ms` of work, then idle until `interval_ms` after its begin.
fn frame(rec: &Recorder, clock: &ManualClock, cpu_ms: f64, interval_ms: f64) {
    rec.frame_begin(FrameBegin { continuous: true });
    clock.advance_ms(cpu_ms);
    rec.frame_end(&FrameEnd::default());
    clock.advance_ms(interval_ms - cpu_ms);
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn close(a: Duration, b_ms: f64) -> bool {
    (ms(a) - b_ms).abs() <= b_ms * 0.011 + 1e-6
}

#[test]
fn records_are_small() {
    assert_eq!(ring::record_bytes(), (32, 8));
    // Five minutes at 120 Hz with two scopes per frame on average: 1.73 MB.
    let bytes = Sizing::default().bytes(&Settings::default());
    assert_eq!(bytes, 36_000 * 32 + 72_000 * 8);
}

#[test]
fn percentiles_of_cpu_time_over_a_window() {
    let (rec, clock) = recorder(at(60.0));
    // 100 frames of 1..=100 ms of CPU, each 200 ms apart.
    for i in 1..=100 {
        frame(&rec, &clock, f64::from(i), 200.0);
    }
    let s = rec.stats(Duration::from_secs(60));
    assert_eq!(s.frames, 100);
    assert!(close(s.cpu.median, 50.0), "{:?}", s.cpu);
    assert!(close(s.cpu.p95, 95.0), "{:?}", s.cpu);
    assert!(close(s.cpu.p99, 99.0), "{:?}", s.cpu);
    assert_eq!(s.cpu.worst, 100 * MS);
    assert_eq!(s.cpu.last, 100 * MS);
    assert!(close(s.cpu.mean, 50.5));
    // Only the last second: frames 96..=100.
    let s = rec.stats(Duration::from_secs(1));
    assert_eq!(s.frames, 5);
    assert_eq!(s.cpu.worst, 100 * MS);
    assert!(close(s.cpu.median, 98.0), "{:?}", s.cpu);
}

#[test]
fn intervals_fps_and_dropped_frames_against_the_budget() {
    let (rec, clock) = recorder(at(120.0));
    for _ in 0..119 {
        frame(&rec, &clock, 2.0, 1000.0 / 120.0);
    }
    // A hitch: 25 ms is three budgets, two frames dropped.
    frame(&rec, &clock, 20.0, 25.0);
    frame(&rec, &clock, 2.0, 1000.0 / 120.0);
    let s = rec.stats(Duration::from_secs(10));
    assert_eq!(s.dropped, 2);
    assert_eq!(s.budget, budget_at(120.0));
    assert!(close(s.interval.median, 1000.0 / 120.0), "{:?}", s.interval);
    assert!(close(s.interval.worst, 25.0), "{:?}", s.interval);
    assert_eq!(s.interval.count, 120, "the first frame has no interval");
    // 121 frames since recording began, about 1.03 s ago.
    let span = ms(Duration::from_nanos(rec.now_ns())) / 1000.0;
    #[expect(clippy::cast_possible_truncation, reason = "a test tolerance")]
    let fps = (121.0 / span) as f32;
    assert!((s.fps - fps).abs() < 0.01, "{} vs {fps}", s.fps);
}

#[test]
fn a_frame_after_a_pause_drops_nothing() {
    let (rec, clock) = recorder(at(60.0));
    frame(&rec, &clock, 1.0, 16.0);
    clock.advance_ms(500.0);
    // Not continuous: the previous frame did not ask for this one at once.
    rec.frame_begin(FrameBegin { continuous: false });
    rec.frame_end(&FrameEnd::default());
    let s = rec.stats(Duration::from_secs(5));
    assert_eq!(s.dropped, 0);
    assert_eq!(s.interval.count, 0);
}

#[test]
fn budgets_follow_the_refresh_rate() {
    for (hz, budget_ms) in [
        (60.0, 16.667),
        (75.0, 13.333),
        (90.0, 11.111),
        (120.0, 8.333),
        (144.0, 6.944),
        (165.0, 6.061),
        (240.0, 4.167),
    ] {
        let (rec, _clock) = recorder(Settings::default());
        rec.set_display_hz(Some(hz));
        let s = rec.stats(Duration::from_secs(1));
        assert!((ms(s.budget) - budget_ms).abs() < 0.001, "{hz} Hz");
        assert!((s.refresh_hz - hz).abs() < f32::EPSILON);
    }
    // No display rate and too few frames to measure: 60 Hz.
    let (rec, _clock) = recorder(Settings::default());
    assert!((rec.stats(MS).refresh_hz - BUDGET_FALLBACK_HZ).abs() < f32::EPSILON);
    // A fixed budget wins over the display.
    let (rec, _clock) = recorder(at(144.0));
    rec.set_display_hz(Some(60.0));
    assert!((rec.stats(MS).refresh_hz - 144.0).abs() < f32::EPSILON);
}

#[test]
fn the_refresh_is_measured_from_continuous_frames() {
    let (rec, clock) = recorder(Settings::default());
    for i in 0..64 {
        // 144 Hz with every fourth frame late: the shortest intervals are the display's.
        let late = if i % 4 == 0 { 3.0 } else { 0.0 };
        frame(&rec, &clock, 1.0, 1000.0 / 144.0 + late);
    }
    let s = rec.stats(Duration::from_secs(1));
    assert!(
        (s.refresh_hz - 144.0).abs() < f32::EPSILON,
        "{}",
        s.refresh_hz
    );
    assert!((snap_refresh_hz(59.5) - 60.0).abs() < f32::EPSILON);
    assert!((snap_refresh_hz(100.5) - 100.0).abs() < f32::EPSILON);
    assert!((snap_refresh_hz(110.0) - 110.0).abs() < f32::EPSILON);
}

#[test]
fn idle_after_a_second_without_frames() {
    let (rec, clock) = recorder(at(60.0));
    frame(&rec, &clock, 3.0, 16.0);
    clock.advance_ms(900.0);
    assert_eq!(rec.stats(Duration::from_secs(5)).idle, None);
    clock.advance_ms(1200.0);
    let s = rec.stats(Duration::from_secs(5));
    assert_eq!(s.idle, Some(Duration::from_millis(2113)));
    assert_eq!(s.cpu.last, 3 * MS, "the last frame's numbers stay");
}

#[test]
fn nested_scopes_give_totals_self_time_and_the_rest() {
    let (rec, clock) = recorder(at(60.0));
    for _ in 0..10 {
        rec.frame_begin(FrameBegin { continuous: true });
        {
            let _tracks = rec.scope("tracks");
            clock.advance_ms(2.0);
            for _ in 0..2 {
                let _decode = rec.scope("decode");
                clock.advance_ms(1.5);
            }
        }
        {
            let _layout = rec.scope("layout");
            clock.advance_ms(1.0);
        }
        clock.advance_ms(0.5);
        rec.frame_end(&FrameEnd::default());
        clock.advance_ms(10.0);
    }
    let mut b = Breakdown::default();
    rec.breakdown(Duration::from_secs(5), &mut b);
    let names: Vec<_> = b.scopes.iter().map(|s| s.name).collect();
    assert_eq!(names, ["tracks", "decode", "layout"]);
    let tracks = &b.scopes[0];
    assert_eq!(tracks.total, 50 * MS);
    assert_eq!(tracks.self_total, 20 * MS);
    assert!(tracks.top_level);
    assert!(close(tracks.time.median, 5.0));
    assert!((tracks.share - 5.0 / 6.5).abs() < 1e-4);
    let decode = &b.scopes[1];
    assert_eq!(decode.total, 30 * MS, "two calls a frame add up");
    assert!(!decode.top_level);
    assert!(close(decode.time.median, 3.0));
    assert_eq!(b.rest_total, 5 * MS);
    assert!(close(b.rest.median, 0.5));
    // The same through the chart seam: one column holding all ten frames.
    let mut bins = [Bin::default(); 1];
    rec.bins(Series::Scope("decode"), 0, rec.now_ns() + 1, &mut bins);
    assert_eq!(bins[0].count, 10);
    assert!(close(bins[0].mean, 3.0));
    rec.bins(Series::Rest, 0, rec.now_ns() + 1, &mut bins);
    assert!(close(bins[0].max, 0.5));
}

#[test]
fn a_scope_that_does_not_count_leaves_the_statistics() {
    let (rec, clock) = recorder(at(60.0));
    for _ in 0..5 {
        rec.frame_begin(FrameBegin { continuous: true });
        {
            let _app = rec.scope("app");
            clock.advance_ms(4.0);
        }
        {
            let _panel = rec.scope_not_counted("perf panel");
            clock.advance_ms(3.0);
            let _chart = rec.scope("chart");
            clock.advance_ms(2.0);
        }
        rec.frame_end(&FrameEnd::default());
        clock.advance_ms(10.0);
    }
    let s = rec.stats(Duration::from_secs(5));
    assert_eq!(s.cpu.worst, 4 * MS, "the panel's 5 ms are not the app's");
    let mut b = Breakdown::default();
    rec.breakdown(Duration::from_secs(5), &mut b);
    let panel = b.scopes.iter().find(|s| s.name == "perf panel").unwrap();
    assert_eq!(panel.kind, ScopeKind::NotCounted);
    assert_eq!(panel.total, 25 * MS, "still shown as a row of its own");
    assert!(panel.share.abs() < f32::EPSILON);
    let chart = b.scopes.iter().find(|s| s.name == "chart").unwrap();
    assert_eq!(
        chart.kind,
        ScopeKind::NotCounted,
        "inside the panel, so not counted"
    );
    let app = b.scopes.iter().find(|s| s.name == "app").unwrap();
    assert!((app.share - 1.0).abs() < 1e-6);
    assert_eq!(b.rest_total, Duration::ZERO);
    // The host's full measure of the frame also loses the panel's time.
    rec.frame_begin(FrameBegin { continuous: true });
    rec.previous_frame_cpu(12 * MS);
    assert_eq!(rec.stats(Duration::from_secs(5)).cpu.last, 7 * MS);
}

#[test]
fn the_hosts_measure_of_the_previous_frame_replaces_ours() {
    let (rec, clock) = recorder(at(60.0));
    frame(&rec, &clock, 2.0, 16.0);
    rec.frame_begin(FrameBegin { continuous: true });
    rec.previous_frame_cpu(9 * MS);
    clock.advance_ms(1.0);
    rec.frame_end(&FrameEnd::default());
    let s = rec.stats(Duration::from_secs(5));
    assert_eq!(s.cpu.worst, 9 * MS);
    assert_eq!(s.cpu.last, MS);
    // Only once per frame.
    rec.previous_frame_cpu(50 * MS);
    rec.previous_frame_cpu(60 * MS);
    assert_eq!(rec.stats(Duration::from_secs(5)).cpu.worst, 50 * MS);
}

#[test]
fn the_frame_in_progress_is_not_in_the_statistics() {
    let (rec, clock) = recorder(at(60.0));
    frame(&rec, &clock, 2.0, 16.0);
    rec.frame_begin(FrameBegin { continuous: true });
    clock.advance_ms(30.0);
    let s = rec.stats(Duration::from_secs(5));
    assert_eq!(s.frames, 1);
    assert_eq!(s.cpu.worst, 2 * MS);
}

#[test]
fn a_coarse_granularity_aggregates_and_keeps_the_worst() {
    let settings = Settings {
        granularity: Granularity::Every(100 * MS),
        ..at(100.0)
    };
    let (rec, clock) = recorder(settings);
    // 1 s at 100 Hz: 10 ms apart, CPU 1 ms except one frame of 30 ms (a 40 ms interval: 3 dropped).
    for i in 0..100 {
        let cpu = if i == 55 { 30.0 } else { 1.0 };
        let interval = if i == 55 { 40.0 } else { 10.0 };
        frame(&rec, &clock, cpu, interval);
    }
    let s = rec.stats(Duration::from_secs(5));
    assert_eq!(s.frames, 100);
    assert_eq!(s.cpu.worst, 30 * MS);
    assert!(close(s.cpu.median, 1.0), "{:?}", s.cpu);
    assert_eq!(s.interval.worst, 40 * MS);
    assert_eq!(s.dropped, 3);
    let mut bins = [Bin::default(); 10];
    rec.bins(
        Series::Cpu,
        0,
        Duration::from_secs(1).as_nanos() as u64,
        &mut bins,
    );
    let frames: u32 = bins.iter().map(|b| b.count).sum();
    assert!(frames >= 90, "about 10 frames a column: {bins:?}");
    assert!(bins.iter().any(|b| b.max == 30 * MS));
    // Ten or so records instead of a hundred: memory for a 100 ms span.
    let sizing = Sizing::default();
    assert_eq!(sizing.records(&settings), 3000);
}

#[test]
fn retention_bounds_what_is_read() {
    let settings = Settings {
        retention: Duration::from_secs(2),
        ..at(60.0)
    };
    let (rec, clock) = recorder(settings);
    for _ in 0..600 {
        frame(&rec, &clock, 1.0, 10.0);
    }
    let s = rec.stats(Duration::from_secs(60));
    assert!(s.frames <= 201, "{}", s.frames);
    assert!(s.frames >= 199, "{}", s.frames);
}

#[test]
fn off_records_nothing_and_holds_no_memory() {
    let clock = ManualClock::new();
    let ring = RingRecorder::with_clock(at(60.0), clock.clone());
    assert!(ring.memory_bytes() > 1_000_000);
    ring.set_enabled(false);
    assert_eq!(ring.memory_bytes(), 0);
    let rec = Recorder::new(ring);
    frame(&rec, &clock, 1.0, 16.0);
    let _g = rec.scope("x");
    rec.count("c", 1);
    assert_eq!(
        rec.stats(Duration::from_secs(5)),
        Stats {
            window: Duration::from_secs(5),
            ..Stats::default()
        }
    );
    drop(_g);
    // On again: an empty history that records.
    rec.set_enabled(true);
    frame(&rec, &clock, 1.0, 16.0);
    assert_eq!(rec.stats(Duration::from_secs(5)).frames, 1);
}

#[test]
fn repaint_causes_are_counted_per_frame() {
    let (rec, clock) = recorder(at(60.0));
    let anim = Cause {
        file: "src/anim.rs",
        line: 12,
        reason: "",
    };
    let input = Cause::named("input");
    for i in 0..10 {
        rec.frame_begin(FrameBegin { continuous: true });
        let causes: &[Cause<'_>] = if i % 2 == 0 { &[anim, input] } else { &[anim] };
        rec.frame_end(&FrameEnd { causes });
        clock.advance_ms(16.0);
    }
    let mut out = Vec::new();
    rec.causes(Duration::from_secs(5), &mut out);
    assert_eq!(
        out,
        [
            CauseCount {
                label: "src/anim.rs:12".to_owned(),
                frames: 10
            },
            CauseCount {
                label: "input".to_owned(),
                frames: 5
            },
        ]
    );
    // Past the table's size: "other causes".
    let files: Vec<&'static str> = (0..20)
        .map(|i| &*Box::leak(format!("f{i}.rs").into_boxed_str()))
        .collect();
    rec.frame_begin(FrameBegin::default());
    let many: Vec<_> = files
        .iter()
        .map(|f| Cause {
            file: f,
            line: 1,
            reason: "",
        })
        .collect();
    rec.frame_end(&FrameEnd { causes: &many });
    rec.causes(Duration::from_secs(5), &mut out);
    assert_eq!(out.len(), CAUSES_MAX + 1);
    assert!(
        out.iter()
            .any(|c| c.label == "other causes" && c.frames == 1)
    );
}

#[test]
fn counters_and_gauges() {
    let (rec, clock) = recorder(at(60.0));
    for i in 1..=4u64 {
        rec.frame_begin(FrameBegin { continuous: true });
        rec.count("rx_bytes", 100 * i);
        rec.count("rx_bytes", 1);
        rec.frame_end(&FrameEnd::default());
        clock.advance_ms(16.0);
    }
    rec.gauge("memory.rss", 1 << 30);
    let mut out = Vec::new();
    rec.counters(Duration::from_secs(5), &mut out);
    assert_eq!(
        out,
        [CounterStats {
            name: "rx_bytes",
            total: 1004,
            worst: 401,
            frames: 4
        }]
    );
    let mut g = Vec::new();
    rec.gauges(&mut g);
    assert_eq!(g.len(), 1);
    assert_eq!(g[0].value, 1 << 30);
    assert_eq!(g[0].at_ns, rec.now_ns());
}

#[test]
fn names_past_the_table_and_scopes_outside_a_frame_are_counted() {
    let ring = RingRecorder::with_clock(at(60.0), ManualClock::new());
    let names: Vec<&'static str> = (0..NAMES_MAX + 5)
        .map(|i| &*Box::leak(format!("s{i}").into_boxed_str()))
        .collect();
    assert!(ring.scope_begin("early", ScopeKind::Counted).is_none());
    ring.frame_begin(FrameBegin::default());
    for name in &names {
        if let Some(t) = ring.scope_begin(name, ScopeKind::Counted) {
            ring.scope_end(t);
        }
    }
    ring.frame_end(&FrameEnd::default());
    let (lost_names, _, _, outside) = ring::lost(&ring);
    assert_eq!(outside, 1);
    assert_eq!(lost_names, 5);
}

#[test]
fn scopes_deeper_than_the_limit_are_counted() {
    let clock = ManualClock::new();
    let ring = std::sync::Arc::new(RingRecorder::with_clock(at(60.0), clock.clone()));
    ring.frame_begin(FrameBegin::default());
    let mut tokens = Vec::new();
    for _ in 0..SCOPE_DEPTH_MAX + 3 {
        tokens.extend(ring.scope_begin("deep", ScopeKind::Counted));
    }
    assert_eq!(tokens.len(), SCOPE_DEPTH_MAX);
    while let Some(t) = tokens.pop() {
        ring.scope_end(t);
    }
    ring.frame_end(&FrameEnd::default());
    assert_eq!(ring::lost(&ring).1, 3);
}

#[test]
fn the_readout_ticks_at_its_rate_and_holds_the_worst() {
    let (rec, clock) = recorder(at(60.0));
    let mut readout = Readout::new(ReadoutSettings::default());
    let mut ticks = 0;
    let mut worst_seen = Vec::new();
    // 2 s at 60 Hz, CPU 2 ms with one 12 ms spike at frame 30 (0.5 s).
    for i in 0..120 {
        frame(
            &rec,
            &clock,
            if i == 30 { 12.0 } else { 2.0 },
            1000.0 / 60.0,
        );
        if readout.update(&*rec) {
            ticks += 1;
            worst_seen.push((i, readout.shown().worst_cpu));
        }
    }
    // 5 Hz over 2 s: the first update and one every 200 ms.
    assert_eq!(ticks, 10);
    let held: Vec<_> = worst_seen
        .iter()
        .filter(|(_, w)| *w == 12 * MS)
        .map(|(i, _)| *i)
        .collect();
    // Shown from the first tick after the spike until a second has passed.
    assert_eq!(held.first(), Some(&36));
    assert!(
        held.last().is_some_and(|&i| (84..96).contains(&i)),
        "{held:?}"
    );
    assert_eq!(readout.shown().cpu, 2 * MS);
    assert!(close(readout.shown().interval, 1000.0 / 60.0));
    assert!((readout.shown().fps - 60.0).abs() < 1.5);
    // Idle: the numbers stay, idle says how long.
    clock.advance(Duration::from_secs(2));
    assert!(readout.update(&*rec));
    assert_eq!(readout.shown().cpu, 2 * MS);
    assert!(readout.shown().idle.is_some());
}

#[test]
fn histogram_bins_cover_their_range() {
    let mut h = Histogram::new();
    assert_eq!(h.quantile_ns(0.5), 0);
    for ns in [500, 1_024, 1_000_000, 16_700_000, 40_000_000_000] {
        h.add(ns);
    }
    assert_eq!(h.count(), 5);
    assert_eq!(h.quantile_ns(1.0), 40_000_000_000);
    assert_eq!(h.quantile_ns(0.0), 500);
    let median = h.quantile_ns(0.5);
    assert!((median as f64 - 1e6).abs() / 1e6 < 0.011, "{median}");
    for i in 1..HIST_BINS {
        let (lo, hi) = Histogram::bin_range_ns(i);
        assert!(lo < hi);
        assert_eq!(
            Histogram::bin_range_ns(i - 1).1,
            lo,
            "bins are contiguous at {i}"
        );
        assert!((hi - lo) as f64 / lo as f64 <= 1.0 / HIST_BINS_PER_OCTAVE as f64 + 1e-9);
    }
}

#[test]
fn scopes_stay_with_their_frames_after_the_rings_wrap() {
    let settings = Settings {
        retention: Duration::from_secs(1),
        ..at(60.0)
    };
    let sizing = Sizing {
        rate_hz: 16,
        entries_per_record: 1,
    };
    let clock = ManualClock::new();
    let rec = Recorder::new(RingRecorder::with_sizing(settings, sizing, clock.clone()));
    // 16 records, 2048 entries: 3000 frames of three scopes wrap both many times.
    for i in 0..3000u32 {
        rec.frame_begin(FrameBegin { continuous: true });
        for name in ["a", "b", "c"] {
            let _g = rec.scope(name);
            clock.advance_ms(if name == "b" { f64::from(i % 7) } else { 1.0 });
        }
        rec.frame_end(&FrameEnd::default());
        clock.advance_ms(5.0);
    }
    let mut b = Breakdown::default();
    rec.breakdown(Duration::from_secs(60), &mut b);
    let total = |n: &str| b.scopes.iter().find(|s| s.name == n).unwrap().total;
    // The last 16 frames: i = 2984..3000.
    let b_ms: u32 = (2984..3000u32).map(|i| i % 7).sum();
    assert_eq!(total("a"), 16 * MS);
    assert_eq!(total("b"), Duration::from_millis(u64::from(b_ms)));
    assert_eq!(total("c"), 16 * MS);
    assert_eq!(b.rest_total, Duration::ZERO);
}

#[test]
fn a_coarse_granularity_merges_scopes_by_name() {
    let settings = Settings {
        granularity: Granularity::Every(100 * MS),
        ..at(100.0)
    };
    let (rec, clock) = recorder(settings);
    for _ in 0..10 {
        rec.frame_begin(FrameBegin { continuous: true });
        {
            let _g = rec.scope("draw");
            clock.advance_ms(2.0);
        }
        rec.frame_end(&FrameEnd::default());
        clock.advance_ms(8.0);
    }
    let mut b = Breakdown::default();
    rec.breakdown(Duration::from_secs(5), &mut b);
    assert_eq!(b.scopes.len(), 1);
    assert_eq!(b.scopes[0].total, 20 * MS);
    assert_eq!(b.scopes[0].time.count, 10);
    assert!(close(b.scopes[0].time.median, 2.0));
}

#[test]
fn a_scope_that_does_not_count_inside_a_counted_one_leaves_its_time() {
    let (rec, clock) = recorder(at(60.0));
    for _ in 0..4 {
        rec.frame_begin(FrameBegin { continuous: true });
        {
            let _widget = rec.scope("widget");
            clock.advance_ms(3.0);
            {
                let _inner = rec.scope("inner");
                clock.advance_ms(1.0);
                let _panel = rec.scope_not_counted("perf panel");
                clock.advance_ms(5.0);
            }
        }
        rec.frame_end(&FrameEnd::default());
        clock.advance_ms(10.0);
    }
    let s = rec.stats(Duration::from_secs(5));
    assert_eq!(s.cpu.worst, 4 * MS);
    let mut b = Breakdown::default();
    rec.breakdown(Duration::from_secs(5), &mut b);
    let row = |n: &str| *b.scopes.iter().find(|s| s.name == n).unwrap();
    assert_eq!(row("widget").total, 16 * MS);
    assert_eq!(row("widget").self_total, 12 * MS);
    assert_eq!(row("inner").total, 4 * MS);
    assert_eq!(row("perf panel").total, 20 * MS);
    assert_eq!(b.rest_total, Duration::ZERO);
}
