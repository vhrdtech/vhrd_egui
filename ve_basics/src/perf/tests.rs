//! The recorder on a hand-moved clock: every number is exact, no wall clock decides an outcome.

use std::sync::Arc;
use std::time::Duration;

use super::*;

const MS: Duration = Duration::from_millis(1);
const SEC: Duration = Duration::from_secs(1);

fn recorder(settings: Settings) -> (Recorder, ManualClock) {
    let clock = ManualClock::new();
    let ring = RingRecorder::with_clock(settings, clock.clone()).expect("valid test settings");
    (Recorder::new(ring), clock)
}

fn ring(settings: Settings) -> (RingRecorder, ManualClock) {
    let clock = ManualClock::new();
    let ring = RingRecorder::with_clock(settings, clock.clone()).expect("valid test settings");
    (ring, clock)
}

fn at(hz: f32) -> Settings {
    Settings {
        budget: BudgetSource::Hz(hz),
        ..Settings::default()
    }
}

/// One frame: `cpu_ms` of work, then idle until `interval_ms` after its begin.
fn frame_with(
    rec: &Recorder,
    clock: &ManualClock,
    cpu_ms: f64,
    interval_ms: f64,
    continuous: bool,
) {
    rec.frame_begin(FrameBegin { continuous });
    clock.advance_ms(cpu_ms);
    rec.frame_end(&FrameEnd::default());
    clock.advance_ms(interval_ms - cpu_ms);
}

/// One continuous frame.
fn frame(rec: &Recorder, clock: &ManualClock, cpu_ms: f64, interval_ms: f64) {
    frame_with(rec, clock, cpu_ms, interval_ms, true);
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// Within the histogram's 1.6 %.
fn close(a: Duration, b_ms: f64) -> bool {
    (ms(a) - b_ms).abs() <= b_ms * 0.016 + 1e-6
}

fn ns_of(d: Duration) -> u64 {
    u64::try_from(d.as_nanos()).unwrap()
}

fn breakdown(rec: &Recorder, len: Duration) -> Breakdown {
    let mut b = Breakdown::default();
    rec.breakdown(rec.last(len), &mut b);
    b
}

fn row<'a>(b: &'a Breakdown, name: &str) -> &'a ScopeStats {
    b.scopes.iter().find(|s| &*s.name == name).unwrap()
}

#[test]
fn records_are_small() {
    assert_eq!(ring::record_bytes(), (32, 8));
    // Five minutes at 120 Hz with two scopes per frame on average: 1.73 MB.
    let sizing = Sizing::default();
    assert_eq!(sizing.bytes(&Settings::default()), 36_000 * 32 + 72_000 * 8);
    // Entries stay within two records' limit and ENTRIES_MAX whatever the bytes.
    let tiny = Sizing {
        entries_bytes: 0,
        ..sizing
    };
    assert_eq!(tiny.entries(), 2 * ENTRIES_PER_RECORD_MAX);
    let huge = Sizing {
        entries_bytes: usize::MAX,
        ..sizing
    };
    assert_eq!(huge.entries(), ENTRIES_MAX);
}

#[test]
fn percentiles_of_cpu_time_over_a_span() {
    let (rec, clock) = recorder(at(60.0));
    // 100 frames of 1..=100 ms of CPU, each 200 ms apart.
    for i in 1..=100 {
        frame(&rec, &clock, f64::from(i), 200.0);
    }
    let s = rec.stats(rec.last(Duration::from_secs(60)));
    assert_eq!(s.frames, 100);
    assert!(close(s.cpu.median, 50.0), "{:?}", s.cpu);
    assert!(close(s.cpu.p95, 95.0), "{:?}", s.cpu);
    assert!(close(s.cpu.p99, 99.0), "{:?}", s.cpu);
    assert_eq!(s.cpu.worst, 100 * MS);
    assert_eq!(s.cpu.last, 100 * MS);
    assert!(close(s.cpu.mean, 50.5));
    // Only the last second: frames 96..=100.
    let s = rec.stats(rec.last(SEC));
    assert_eq!(s.frames, 5);
    assert_eq!(s.cpu.worst, 100 * MS);
    assert!(close(s.cpu.median, 98.0), "{:?}", s.cpu);
}

/// BAS-10, review finding 4: a span in the past reads the same as it did then (a paused panel).
#[test]
fn a_span_in_the_past_reads_its_own_frames() {
    let (rec, clock) = recorder(at(60.0));
    for i in 0..100 {
        frame(&rec, &clock, if i < 50 { 2.0 } else { 8.0 }, 20.0);
    }
    // The first second: frames 0..50 at 2 ms.
    let first = Span {
        from_ns: 0,
        to_ns: 1_000_000_000,
    };
    let s = rec.stats(first);
    assert_eq!(s.frames, 50);
    assert_eq!(s.cpu.worst, 2 * MS);
    assert_eq!(
        s.cpu.last,
        2 * MS,
        "the newest frame of the span, not of now"
    );
    assert_eq!(s.covered, first);
    assert!((s.fps - 50.0).abs() < 0.01);
    let second = Span {
        from_ns: 1_000_000_000,
        to_ns: 2_000_000_000,
    };
    assert_eq!(rec.stats(second).cpu.worst, 8 * MS);
    // The same split through the histogram.
    let mut h = Histogram::new();
    rec.histogram(Series::Cpu, first, &mut h);
    assert_eq!(h.count(), 50);
    assert_eq!(h.quantile_ns(1.0), ns_of(2 * MS));
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
    let s = rec.stats(rec.last(Duration::from_secs(10)));
    assert_eq!(s.dropped, 2);
    assert_eq!(s.pauses, 0);
    assert_eq!(s.budget, budget_at(120.0));
    assert!(close(s.interval.median, 1000.0 / 120.0), "{:?}", s.interval);
    assert!(close(s.interval.worst, 25.0), "{:?}", s.interval);
    assert_eq!(s.interval.count, 120, "the first frame has no interval");
    // 121 frames since recording began.
    let span = ms(Duration::from_nanos(rec.now_ns())) / 1000.0;
    #[expect(clippy::cast_possible_truncation, reason = "a test tolerance")]
    let fps = (121.0 / span) as f32;
    assert!((s.fps - fps).abs() < 0.01, "{} vs {fps}", s.fps);
}

/// BAS-10, review finding 1: a hidden window sends no frames while the app still animates; the gap is a pause,
/// not thousands of dropped frames.
#[test]
fn a_gap_between_continuous_frames_is_a_pause() {
    let (rec, clock) = recorder(at(120.0));
    for _ in 0..120 {
        frame(&rec, &clock, 2.0, 1000.0 / 120.0);
    }
    clock.advance(Duration::from_secs(10));
    for _ in 0..10 {
        frame(&rec, &clock, 2.0, 1000.0 / 120.0);
    }
    let s = rec.stats(rec.last(Duration::from_secs(30)));
    assert_eq!(s.dropped, 0);
    assert_eq!(s.pauses, 1);
    assert_eq!(s.frames, 130);
    assert!(close(s.interval.worst, 1000.0 / 120.0), "{:?}", s.interval);
    assert_eq!(
        s.interval.count, 128,
        "the first frame and the one after the pause have none"
    );
}

#[test]
fn a_frame_after_a_pause_drops_nothing() {
    let (rec, clock) = recorder(at(60.0));
    frame(&rec, &clock, 1.0, 16.0);
    clock.advance_ms(500.0);
    // Not continuous: the previous frame did not ask for this one at once.
    frame_with(&rec, &clock, 1.0, 16.0, false);
    let s = rec.stats(rec.last(Duration::from_secs(5)));
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
        let s = rec.stats(rec.last(SEC));
        assert!((ms(s.budget) - budget_ms).abs() < 0.001, "{hz} Hz");
        assert!((s.refresh_hz - hz).abs() < f32::EPSILON);
    }
    // No display rate and too few frames to measure: 60 Hz.
    let (rec, _clock) = recorder(Settings::default());
    assert!((rec.stats(rec.last(MS)).refresh_hz - BUDGET_FALLBACK_HZ).abs() < f32::EPSILON);
    // A fixed budget wins over the display.
    let (rec, _clock) = recorder(at(144.0));
    rec.set_display_hz(Some(60.0));
    assert!((rec.stats(rec.last(MS)).refresh_hz - 144.0).abs() < f32::EPSILON);
}

/// BAS-10, review finding 11: settings out of range are refused with what was wrong; a host's odd display rate
/// is ignored.
#[test]
fn settings_out_of_range_are_refused() {
    let hz = |hz| Settings {
        budget: BudgetSource::Hz(hz),
        ..Settings::default()
    };
    assert!(matches!(
        hz(f32::INFINITY).validate(),
        Err(SettingsError::BudgetHz(_))
    ));
    assert!(matches!(
        hz(f32::NAN).validate(),
        Err(SettingsError::BudgetHz(_))
    ));
    assert!(matches!(
        hz(0.0).validate(),
        Err(SettingsError::BudgetHz(_))
    ));
    let zero = Settings {
        retention: Duration::ZERO,
        ..Settings::default()
    };
    assert_eq!(
        zero.validate(),
        Err(SettingsError::Retention(Duration::ZERO))
    );
    let coarse = Settings {
        granularity: Granularity::Every(Duration::ZERO),
        ..Settings::default()
    };
    assert!(RingRecorder::new(coarse).is_err());
    let (rec, _clock) = recorder(at(60.0));
    assert!(rec.set_settings(hz(f32::NAN)).is_err());
    assert_eq!(rec.settings(), at(60.0), "the old settings stay");
    let nan_rate = ReadoutSettings {
        rate_hz: f32::NAN,
        ..ReadoutSettings::default()
    };
    assert!(matches!(
        Readout::new(nan_rate),
        Err(SettingsError::ReadoutRateHz(_))
    ));
    let no_hold = ReadoutSettings {
        worst_hold: Duration::ZERO,
        ..ReadoutSettings::default()
    };
    assert!(Readout::new(no_hold).is_err());
    let (rec, _clock) = recorder(Settings::default());
    rec.set_display_hz(Some(f32::NAN));
    assert!((rec.stats(rec.last(MS)).refresh_hz - BUDGET_FALLBACK_HZ).abs() < f32::EPSILON);
    assert_eq!(budget_at(f32::INFINITY), budget_at(BUDGET_FALLBACK_HZ));
}

#[test]
fn the_refresh_is_measured_from_continuous_frames() {
    let (rec, clock) = recorder(Settings::default());
    for i in 0..64 {
        // 144 Hz with every fourth frame late: the shortest intervals are the display's.
        let late = if i % 4 == 0 { 3.0 } else { 0.0 };
        frame(&rec, &clock, 1.0, 1000.0 / 144.0 + late);
    }
    let s = rec.stats(rec.last(SEC));
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
    assert_eq!(rec.stats(rec.last(Duration::from_secs(5))).idle, None);
    clock.advance_ms(1200.0);
    let s = rec.stats(rec.last(Duration::from_secs(5)));
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
    let b = breakdown(&rec, Duration::from_secs(5));
    let names: Vec<_> = b.scopes.iter().map(|s| &*s.name).collect();
    assert_eq!(names, ["tracks", "decode", "layout"]);
    let tracks = row(&b, "tracks");
    assert_eq!(tracks.total, 50 * MS);
    assert_eq!(tracks.self_total, 20 * MS);
    assert!(tracks.top_level);
    assert!(close(tracks.time.median, 5.0));
    assert!((tracks.share - 5.0 / 6.5).abs() < 1e-4);
    let decode = row(&b, "decode");
    assert_eq!(decode.total, 30 * MS, "two calls a frame add up");
    assert!(!decode.top_level);
    assert!(close(decode.time.median, 3.0));
    assert_eq!(b.rest_total, 5 * MS);
    assert!(close(b.rest.median, 0.5));
    // The same through the chart seam: one column holding all ten frames.
    let all = Span {
        from_ns: 0,
        to_ns: rec.now_ns() + 1,
    };
    let mut bins = [Bin::default(); 1];
    rec.bins(Series::Scope(decode.id), all, &mut bins);
    assert_eq!(bins[0].count, 10);
    assert!(close(bins[0].mean, 3.0));
    assert!(close(bins[0].median, 3.0));
    rec.bins(Series::Rest, all, &mut bins);
    assert!(close(bins[0].max, 0.5));
}

/// BAS-10, review finding 4: a column has its median and p99, not only min, max and mean.
#[test]
fn chart_columns_carry_median_and_p99() {
    let (rec, clock) = recorder(at(60.0));
    // Two columns of one second each: 100 frames of 1..=100 ms of CPU each.
    for _ in 0..2 {
        for i in 1..=100 {
            frame(&rec, &clock, f64::from(i) / 10.0, 10.0);
        }
    }
    let mut bins = [Bin::default(); 2];
    rec.bins(
        Series::Cpu,
        Span {
            from_ns: 0,
            to_ns: 2_000_000_000,
        },
        &mut bins,
    );
    for b in bins {
        assert_eq!(b.count, 100);
        assert_eq!(b.min, Duration::from_micros(100));
        assert_eq!(b.max, 10 * MS);
        assert!(close(b.median, 5.0), "{b:?}");
        assert!(close(b.p99, 9.9), "{b:?}");
        assert!(close(b.mean, 5.05), "{b:?}");
    }
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
    let s = rec.stats(rec.last(Duration::from_secs(5)));
    assert_eq!(s.cpu.worst, 4 * MS, "the panel's 5 ms are not the app's");
    let b = breakdown(&rec, Duration::from_secs(5));
    let panel = row(&b, "perf panel");
    assert_eq!(panel.kind, ScopeKind::NotCounted);
    assert_eq!(panel.total, 25 * MS, "still shown as a row of its own");
    assert!(panel.share.abs() < f32::EPSILON);
    assert_eq!(
        row(&b, "chart").kind,
        ScopeKind::NotCounted,
        "inside the panel"
    );
    assert!((row(&b, "app").share - 1.0).abs() < 1e-6);
    assert_eq!(b.rest_total, Duration::ZERO);
    // The host's full measure of the frame also loses the panel's time.
    rec.frame_begin(FrameBegin { continuous: true });
    rec.previous_frame_cpu(12 * MS);
    assert_eq!(rec.stats(rec.last(Duration::from_secs(5))).cpu.last, 7 * MS);
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
    assert_eq!(
        rec.stats(rec.last(Duration::from_secs(5))).cpu.worst,
        4 * MS
    );
    let b = breakdown(&rec, Duration::from_secs(5));
    assert_eq!(row(&b, "widget").total, 16 * MS);
    assert_eq!(row(&b, "widget").self_total, 12 * MS);
    assert_eq!(row(&b, "inner").total, 4 * MS);
    assert_eq!(row(&b, "perf panel").total, 20 * MS);
    assert_eq!(b.rest_total, Duration::ZERO);
}

/// BAS-10, review finding 7: a scope inside one of the same name is part of the outer one, not counted twice.
#[test]
fn a_recursive_scope_counts_once() {
    let (rec, clock) = recorder(at(60.0));
    rec.frame_begin(FrameBegin { continuous: true });
    {
        let _a = rec.scope("tree");
        clock.advance(2 * MS);
        {
            let _b = rec.scope("tree");
            clock.advance(3 * MS);
            let _leaf = rec.scope("leaf");
            clock.advance(MS);
        }
    }
    rec.frame_end(&FrameEnd::default());
    let b = breakdown(&rec, Duration::from_secs(5));
    let tree = row(&b, "tree");
    assert_eq!(tree.total, 6 * MS);
    assert_eq!(tree.self_total, 5 * MS);
    assert!((tree.share - 1.0).abs() < 1e-6);
    assert_eq!(row(&b, "leaf").total, MS);
    assert_eq!(b.rest_total, Duration::ZERO);
}

/// BAS-10, review finding 17: instances of one scope name get rows of their own.
#[test]
fn instances_of_a_scope_have_rows_of_their_own() {
    let (rec, clock) = recorder(at(60.0));
    rec.frame_begin(FrameBegin { continuous: true });
    for (instance, cost) in [(1, 2), (2, 5)] {
        let _g = rec.scope_of("Note", instance);
        clock.advance(cost * MS);
    }
    rec.frame_end(&FrameEnd::default());
    let b = breakdown(&rec, Duration::from_secs(5));
    let rows: Vec<_> = b
        .scopes
        .iter()
        .map(|s| (&*s.name, s.instance, s.total))
        .collect();
    assert_eq!(rows, [("Note", Some(2), 5 * MS), ("Note", Some(1), 2 * MS)]);
    assert_ne!(b.scopes[0].id, b.scopes[1].id);
}

#[test]
fn the_hosts_measure_of_the_previous_frame_replaces_ours() {
    let (rec, clock) = recorder(at(60.0));
    frame(&rec, &clock, 2.0, 16.0);
    rec.frame_begin(FrameBegin { continuous: true });
    rec.previous_frame_cpu(9 * MS);
    clock.advance_ms(1.0);
    rec.frame_end(&FrameEnd::default());
    let s = rec.stats(rec.last(Duration::from_secs(5)));
    assert_eq!(s.cpu.worst, 9 * MS);
    assert_eq!(s.cpu.last, MS);
    // Only once per frame.
    rec.previous_frame_cpu(50 * MS);
    rec.previous_frame_cpu(60 * MS);
    assert_eq!(
        rec.stats(rec.last(Duration::from_secs(5))).cpu.worst,
        50 * MS
    );
}

#[test]
fn the_frame_in_progress_is_not_in_the_statistics() {
    let (rec, clock) = recorder(at(60.0));
    frame(&rec, &clock, 2.0, 16.0);
    rec.frame_begin(FrameBegin { continuous: true });
    clock.advance_ms(30.0);
    let s = rec.stats(rec.last(Duration::from_secs(5)));
    assert_eq!(s.frames, 1);
    assert_eq!(s.cpu.worst, 2 * MS);
}

/// BAS-10, review finding 8: a frame begun and never ended adds no CPU sample.
#[test]
fn an_abandoned_frame_adds_nothing() {
    let (rec, clock) = recorder(at(60.0));
    for _ in 0..3 {
        frame(&rec, &clock, 5.0, 16.0);
    }
    rec.frame_begin(FrameBegin { continuous: true });
    clock.advance_ms(16.0);
    frame(&rec, &clock, 5.0, 16.0);
    let s = rec.stats(rec.last(Duration::from_secs(5)));
    assert_eq!(s.frames, 4);
    assert_eq!(s.cpu.count, 4);
    assert_eq!(s.cpu.mean, 5 * MS);
    assert_eq!(rec.health().abandoned_frames, 1);
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
    let s = rec.stats(rec.last(Duration::from_secs(5)));
    assert_eq!(s.frames, 100);
    assert_eq!(s.cpu.worst, 30 * MS);
    assert!(close(s.cpu.median, 1.0), "{:?}", s.cpu);
    assert_eq!(s.interval.worst, 40 * MS);
    assert_eq!(s.dropped, 3);
    let mut bins = [Bin::default(); 10];
    rec.bins(
        Series::Cpu,
        Span {
            from_ns: 0,
            to_ns: 1_000_000_000,
        },
        &mut bins,
    );
    let frames: u32 = bins.iter().map(|b| b.count).sum();
    assert!(frames >= 90, "about 10 frames a column: {bins:?}");
    assert!(bins.iter().any(|b| b.max == 30 * MS));
    assert_eq!(Sizing::default().records(&settings), 3000);
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
    let b = breakdown(&rec, Duration::from_secs(5));
    assert_eq!(b.scopes.len(), 1);
    assert_eq!(b.scopes[0].total, 20 * MS);
    assert_eq!(b.scopes[0].time.count, 10);
    assert!(close(b.scopes[0].time.median, 2.0));
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
    let s = rec.stats(rec.last(Duration::from_secs(60)));
    assert!((199..=240).contains(&s.frames), "{}", s.frames);
    assert!(s.covered.len() <= Duration::from_secs(3));
}

#[test]
fn off_records_nothing_and_holds_no_memory() {
    let (ring, clock) = ring(at(60.0));
    assert!(ring.memory_bytes() > 1_000_000);
    ring.set_enabled(false);
    assert_eq!(ring.memory_bytes(), 0);
    let rec = Recorder::new(ring);
    frame(&rec, &clock, 1.0, 16.0);
    let span = rec.last(Duration::from_secs(5));
    {
        let _g = rec.scope("x");
        rec.count("c", 1);
    }
    assert_eq!(
        rec.stats(span),
        Stats {
            span,
            ..Stats::default()
        }
    );
    // On again: an empty history that records.
    rec.set_enabled(true);
    frame(&rec, &clock, 1.0, 16.0);
    assert_eq!(rec.stats(rec.last(Duration::from_secs(5))).frames, 1);
}

#[test]
fn repaint_causes_are_counted_per_frame() {
    let (rec, clock) = recorder(at(60.0));
    let anim = Cause {
        file: "/home/x/app/src/anim.rs",
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
    rec.causes(rec.last(Duration::from_secs(5)), &mut out);
    assert_eq!(
        out,
        [
            CauseCount {
                label: "src/anim.rs:12".into(),
                frames: 10
            },
            CauseCount {
                label: "input".into(),
                frames: 5
            },
        ]
    );
}

fn leaked(prefix: &str, n: usize) -> Vec<&'static str> {
    (0..n)
        .map(|i| &*Box::leak(format!("{prefix}{i}").into_boxed_str()))
        .collect()
}

/// BAS-10, review finding 13: a cause no held frame names any more makes room for a new one.
#[test]
fn causes_age_out_of_the_table() {
    let settings = Settings {
        retention: SEC,
        ..at(60.0)
    };
    let (rec, clock) = recorder(settings);
    let cause = |file| Cause {
        file,
        line: 1,
        reason: "",
    };
    let old = leaked("old", CAUSES_MAX + 1);
    rec.frame_begin(FrameBegin::default());
    let many: Vec<_> = old.iter().map(|f| cause(f)).collect();
    rec.frame_end(&FrameEnd { causes: &many });
    assert_eq!(rec.health().lost_causes, 1, "one past the table");
    let mut out = Vec::new();
    rec.causes(rec.last(SEC), &mut out);
    assert!(out.iter().any(|c| &*c.label == "other causes"));
    // Long enough for the ring to drop that frame: the slots free up.
    for _ in 0..200 {
        frame(&rec, &clock, 1.0, 10.0);
    }
    let new = leaked("new", CAUSES_MAX);
    rec.frame_begin(FrameBegin::default());
    let fresh: Vec<_> = new.iter().map(|f| cause(f)).collect();
    rec.frame_end(&FrameEnd { causes: &fresh });
    rec.causes(rec.last(SEC), &mut out);
    assert_eq!(out.len(), CAUSES_MAX);
    assert!(out.iter().all(|c| c.label.starts_with("new")), "{out:?}");
    assert_eq!(rec.health().lost_causes, 1);
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
    rec.counters(rec.last(Duration::from_secs(5)), &mut out);
    assert_eq!(
        out,
        [CounterStats {
            name: "rx_bytes".into(),
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
    let (ring, _clock) = ring(at(60.0));
    let names = leaked("s", NAMES_MAX + 5);
    assert!(
        ring.scope_begin("early".into(), ScopeKind::Counted)
            .is_none()
    );
    ring.frame_begin(FrameBegin::default());
    for name in &names {
        if let Some(t) = ring.scope_begin((*name).into(), ScopeKind::Counted) {
            ring.scope_end(t);
        }
    }
    ring.frame_end(&FrameEnd::default());
    let h = ring.health();
    assert_eq!(h.outside_frame, 1);
    assert_eq!(h.lost_names, 5);
}

/// BAS-10, review finding 16: many distinct names a frame are all recorded, each by its own row.
#[test]
fn a_hundred_scopes_a_frame_are_recorded() {
    let sizing = Sizing {
        entries_bytes: 8 * 100 * 1000,
        ..Sizing::default()
    };
    let clock = ManualClock::new();
    let rec = Recorder::new(RingRecorder::with_sizing(at(60.0), sizing, clock.clone()).unwrap());
    let names = leaked("w", 100);
    for _ in 0..50 {
        rec.frame_begin(FrameBegin { continuous: true });
        for n in &names {
            let _g = rec.scope(n);
            clock.advance(Duration::from_micros(10));
        }
        rec.frame_end(&FrameEnd::default());
        clock.advance_ms(10.0);
    }
    let b = breakdown(&rec, Duration::from_secs(5));
    assert_eq!(b.scopes.len(), 100);
    assert!(
        b.scopes
            .iter()
            .all(|s| s.total == 50 * Duration::from_micros(10))
    );
    assert_eq!(b.rest_total, Duration::ZERO);
}

#[test]
fn scopes_deeper_than_the_limit_are_counted() {
    let (ring, _clock) = ring(at(60.0));
    ring.frame_begin(FrameBegin::default());
    let names = leaked("d", SCOPE_DEPTH_MAX + 3);
    let mut tokens = Vec::new();
    for name in &names {
        tokens.extend(ring.scope_begin((*name).into(), ScopeKind::Counted));
    }
    assert_eq!(tokens.len(), SCOPE_DEPTH_MAX);
    while let Some(t) = tokens.pop() {
        ring.scope_end(t);
    }
    ring.frame_end(&FrameEnd::default());
    assert_eq!(ring.health().lost_depth, 3);
    assert_eq!(ring.health().stale_scopes, 0);
}

/// BAS-10, review finding 6: scopes from another thread are refused and counted; out-of-order ends are counted.
#[test]
#[expect(
    clippy::disallowed_methods,
    reason = "a native test needs a second thread"
)]
fn scopes_from_another_thread_or_out_of_order_are_counted() {
    let (ring, clock) = ring(at(60.0));
    let ring = Arc::new(ring);
    ring.frame_begin(FrameBegin { continuous: true });
    let other = Arc::clone(&ring);
    let token = std::thread::spawn(move || other.scope_begin("worker".into(), ScopeKind::Counted))
        .join()
        .unwrap();
    assert!(token.is_none());
    assert_eq!(ring.health().other_thread, 1);
    // Ended out of order: the outer first, the inner is lost and counted, its late end too.
    let outer = ring
        .scope_begin("outer".into(), ScopeKind::Counted)
        .unwrap();
    let inner = ring
        .scope_begin("inner".into(), ScopeKind::Counted)
        .unwrap();
    clock.advance(2 * MS);
    ring.scope_end(outer);
    ring.scope_end(inner);
    ring.frame_end(&FrameEnd::default());
    assert_eq!(ring.health().stale_scopes, 2);
    let rec = Recorder::new(Arc::try_unwrap(ring).unwrap());
    let b = breakdown(&rec, Duration::from_secs(5));
    let names: Vec<_> = b.scopes.iter().map(|s| &*s.name).collect();
    assert_eq!(names, ["outer"]);
    // A token kept past its frame is stale too.
    rec.frame_begin(FrameBegin::default());
    let g = rec.scope("kept");
    rec.frame_end(&FrameEnd::default());
    rec.frame_begin(FrameBegin::default());
    drop(g);
    assert_eq!(rec.health().stale_scopes, 3);
}

#[test]
fn scopes_stay_with_their_frames_after_the_rings_wrap() {
    let settings = Settings {
        retention: SEC,
        ..at(60.0)
    };
    let sizing = Sizing {
        rate_hz: 16,
        entries_bytes: 0,
    };
    let clock = ManualClock::new();
    let rec = Recorder::new(RingRecorder::with_sizing(settings, sizing, clock.clone()).unwrap());
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
    let b = breakdown(&rec, Duration::from_secs(60));
    // The last 16 frames: i = 2984..3000.
    let b_ms: u32 = (2984..3000u32).map(|i| i % 7).sum();
    assert_eq!(row(&b, "a").total, 16 * MS);
    assert_eq!(row(&b, "b").total, Duration::from_millis(u64::from(b_ms)));
    assert_eq!(row(&b, "c").total, 16 * MS);
    assert_eq!(b.rest_total, Duration::ZERO);
}

/// BAS-10, review finding 2: when scope history is shorter than the frames', the breakdown covers only what it
/// holds and says so, instead of booking the rest as "egui and OS".
#[test]
fn a_breakdown_covers_only_the_frames_whose_scopes_are_held() {
    let (rec, clock) = recorder(at(120.0));
    let names = leaked("w", 50);
    for _ in 0..(120 * 60) {
        rec.frame_begin(FrameBegin { continuous: true });
        for n in &names {
            let _g = rec.scope(n);
            clock.advance(Duration::from_micros(20));
        }
        rec.frame_end(&FrameEnd::default());
        clock.advance(Duration::from_nanos(8_333_333 - 1_000_000));
    }
    let span = rec.last(Duration::from_secs(60));
    let b = breakdown(&rec, Duration::from_secs(60));
    let share: f32 = b.scopes.iter().map(|s| s.share).sum();
    assert!((share - 1.0).abs() < 1e-3, "{share}");
    assert_eq!(b.rest_total, Duration::ZERO);
    // 72 000 entries hold 1440 frames of 50 scopes: 12 s of the 60.
    assert!(b.covered.from_ns > span.from_ns);
    let held = b.covered.len().as_secs_f64();
    assert!((11.9..12.1).contains(&held), "{held}");
    let h = rec.health();
    assert!((11.9..12.1).contains(&h.scopes_covered.len().as_secs_f64()));
    assert!(h.frames_covered.len() > Duration::from_secs(59));
    let mut out = Vec::new();
    rec.counters(span, &mut out);
    assert!(out.is_empty());
}

#[test]
fn the_readout_ticks_at_its_rate_and_holds_the_worst() {
    let (rec, clock) = recorder(at(60.0));
    let mut readout = Readout::new(ReadoutSettings::default()).unwrap();
    let mut ticks = 0;
    let mut shown_at = Vec::new();
    // 4 s at 60 Hz, CPU 2 ms with one 12 ms spike at frame 30 (0.5 s).
    for i in 0..240 {
        if readout.update(&*rec) {
            ticks += 1;
            if readout.shown().worst_cpu == 12 * MS {
                shown_at.push(rec.now_ns());
            }
        }
        frame(
            &rec,
            &clock,
            if i == 30 { 12.0 } else { 2.0 },
            1000.0 / 60.0,
        );
    }
    assert_eq!(ticks, 20, "5 Hz over 4 s");
    // Review finding 20: in the last second's maximum for 1 s, then held a further second.
    let first = *shown_at.first().unwrap();
    let last = *shown_at.last().unwrap();
    let shown_for = Duration::from_nanos(last - first);
    assert!(
        (Duration::from_millis(1600)..=Duration::from_millis(2000)).contains(&shown_for),
        "{shown_for:?}"
    );
    assert!(close(readout.shown().cpu, 2.0));
    assert!(close(readout.shown().interval, 1000.0 / 60.0));
    assert!((readout.shown().fps - 60.0).abs() < 1.5);
}

/// BAS-10, review findings 5 and 12: updated before a frame begins, the readout shows idle after a pause; a tick
/// without continuous frames shows no interval.
#[test]
fn the_readout_shows_idle_and_no_stale_interval() {
    let (rec, clock) = recorder(at(60.0));
    let mut readout = Readout::new(ReadoutSettings::default()).unwrap();
    for _ in 0..30 {
        readout.update(&*rec);
        frame(&rec, &clock, 1.0, 1000.0 / 60.0);
    }
    assert!(close(readout.shown().interval, 1000.0 / 60.0));
    for _ in 0..30 {
        readout.update(&*rec);
        frame_with(&rec, &clock, 1.0, 1000.0 / 60.0, false);
    }
    assert_eq!(readout.shown().interval, Duration::ZERO);
    clock.advance(Duration::from_secs(3));
    assert!(readout.update(&*rec));
    assert!(
        readout
            .shown()
            .idle
            .is_some_and(|i| i > Duration::from_secs(2))
    );
    assert!(close(readout.shown().cpu, 1.0), "the last numbers stay");
}

/// BAS-10, review finding 9: the median of values at a bin's lower edge is within the stated 1.6 %.
#[test]
fn histogram_error_is_within_what_the_docs_say() {
    let mut h = Histogram::new();
    let v = 1u64 << 23;
    h.add(1_000_000);
    h.add_n(v, 3);
    h.add(20_000_000);
    #[expect(clippy::cast_precision_loss, reason = "a relative error")]
    let err = (h.quantile_ns(0.5) as f64 - v as f64).abs() / v as f64;
    assert!(err <= 0.016, "{err}");
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
    #[expect(clippy::cast_precision_loss, reason = "a relative error")]
    let err = (h.quantile_ns(0.5) as f64 - 1e6).abs() / 1e6;
    assert!(err < 0.016, "{err}");
    for i in 1..HIST_BINS {
        let (lo, hi) = Histogram::bin_range_ns(i);
        assert!(lo < hi);
        assert_eq!(
            Histogram::bin_range_ns(i - 1).1,
            lo,
            "bins are contiguous at {i}"
        );
        #[expect(clippy::cast_precision_loss, reason = "a relative width")]
        let width = (hi - lo) as f64 / lo as f64;
        assert!(width <= 1.0 / HIST_BINS_PER_OCTAVE as f64 + 1e-9);
    }
    h.clear();
    assert_eq!(h.count(), 0);
}

/// Random frames and nested scopes against a plain model, on rings of random small size: frames, drops, worst
/// values, the rest and every scope's total and self time match. From the review of 18 Nov 2026.
#[test]
fn the_ring_matches_a_plain_model() {
    #[derive(Default)]
    struct F {
        start: u64,
        cpu: u64,
        gap: Option<u64>,
        dropped: u64,
        entries: Vec<(&'static str, Option<&'static str>, u64)>,
    }
    struct Rng(u64);
    impl Rng {
        fn below(&mut self, n: u64) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0 % n
        }
    }
    const NAMES: [&str; 5] = ["a", "b", "c", "d", "e"];
    fn push_entry(f: &mut F, name: &'static str, parent: Option<&'static str>, dur: u64) {
        match f.entries.iter_mut().find(|e| e.0 == name && e.1 == parent) {
            Some(e) => e.2 += dur,
            None => f.entries.push((name, parent, dur)),
        }
    }
    let budget = ns_of(budget_at(100.0));
    for seed in 1..400u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let rate = 1 + u32::try_from(rng.below(40)).unwrap();
        let settings = Settings {
            retention: SEC,
            ..at(100.0)
        };
        let sizing = Sizing {
            rate_hz: rate,
            entries_bytes: 8 * 64 * 64,
        };
        let cap = sizing.records(&settings);
        let clock = ManualClock::new();
        let rec =
            Recorder::new(RingRecorder::with_sizing(settings, sizing, clock.clone()).unwrap());
        let n_frames = rng.below(5 * cap as u64 + 3);
        let mut model: Vec<F> = Vec::new();
        let mut last_start: Option<u64> = None;
        for _ in 0..n_frames {
            let continuous = rng.below(4) != 0;
            let start = clock.now_ns();
            let mut f = F {
                start,
                ..F::default()
            };
            if let (true, Some(ls)) = (continuous, last_start) {
                let g = start - ls;
                f.gap = Some(g);
                f.dropped = ((g + budget / 2) / budget).saturating_sub(1);
            }
            last_start = Some(start);
            rec.frame_begin(FrameBegin { continuous });
            let mut stack: Vec<(&'static str, ScopeGuard<'_>, u64)> = Vec::new();
            for _ in 0..rng.below(12) {
                clock.advance(Duration::from_micros(1 + rng.below(3000)));
                if rng.below(2) == 0 && stack.len() < 4 {
                    let name = NAMES[usize::try_from(rng.below(5)).unwrap()];
                    if stack.iter().any(|(n, _, _)| *n == name) {
                        continue;
                    }
                    let g = rec.scope(name);
                    stack.push((name, g, clock.now_ns()));
                } else if let Some((name, g, s)) = stack.pop() {
                    drop(g);
                    let parent = stack.last().map(|(n, _, _)| *n);
                    push_entry(&mut f, name, parent, clock.now_ns() - s);
                }
            }
            while let Some((name, g, s)) = stack.pop() {
                clock.advance(Duration::from_micros(1 + rng.below(500)));
                drop(g);
                let parent = stack.last().map(|(n, _, _)| *n);
                push_entry(&mut f, name, parent, clock.now_ns() - s);
            }
            clock.advance(Duration::from_micros(rng.below(2000)));
            f.cpu = clock.now_ns() - start;
            rec.frame_end(&FrameEnd::default());
            clock.advance(Duration::from_micros(1 + rng.below(30_000)));
            model.push(f);
        }
        let span = rec.last(Duration::from_micros(rng.below(1_500_000) + 1));
        let held = &model[model.len().saturating_sub(cap)..];
        let in_w: Vec<&F> = held.iter().filter(|f| span.contains(f.start)).collect();
        let s = rec.stats(span);
        let mut b = Breakdown::default();
        rec.breakdown(span, &mut b);
        assert_eq!(s.frames as usize, in_w.len(), "seed {seed}");
        assert_eq!(
            u64::from(s.dropped),
            in_w.iter().map(|f| f.dropped).sum::<u64>(),
            "seed {seed}"
        );
        assert_eq!(
            ns_of(s.cpu.worst),
            in_w.iter().map(|f| f.cpu).max().unwrap_or(0),
            "seed {seed}"
        );
        let gap_worst = in_w.iter().filter_map(|f| f.gap).max().unwrap_or(0);
        assert_eq!(ns_of(s.interval.worst), gap_worst, "seed {seed}");
        // The breakdown covers the frames whose scopes are held.
        let covered: Vec<&&F> = in_w
            .iter()
            .filter(|f| b.covered.contains(f.start))
            .collect();
        let rest: u64 = covered
            .iter()
            .map(|f| {
                f.cpu
                    - f.entries
                        .iter()
                        .filter(|e| e.1.is_none())
                        .map(|e| e.2)
                        .sum::<u64>()
            })
            .sum();
        assert_eq!(ns_of(b.rest_total), rest, "seed {seed}");
        for name in NAMES {
            let entries = || covered.iter().flat_map(|f| &f.entries);
            let total: u64 = entries().filter(|e| e.0 == name).map(|e| e.2).sum();
            let children: u64 = entries().filter(|e| e.1 == Some(name)).map(|e| e.2).sum();
            let (t, s) = b
                .scopes
                .iter()
                .find(|r| &*r.name == name)
                .map_or((0, 0), |r| (ns_of(r.total), ns_of(r.self_total)));
            assert_eq!(
                (t, s),
                (total, total - children),
                "seed {seed} scope {name}"
            );
        }
    }
}
