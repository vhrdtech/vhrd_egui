//! What the frame recorder costs: per frame on and off, per scope on and off with few and with a hundred names,
//! a breakdown read, and its memory.
//!
//! Each case runs five times; the table shows the fastest run, per frame or per scope.

use std::hint::black_box;
use std::time::{Duration, Instant};

use ve_basics::perf::{
    Breakdown, Cause, FrameBegin, FrameEnd, Recorder, RingRecorder, Settings, Sizing,
};

const FRAMES: u32 = 1_000_000;
const RUNS: usize = 5;
const SCOPES: u32 = 10;
const NAMES: usize = 100;

fn fastest(mut run: impl FnMut()) -> Duration {
    (0..RUNS)
        .map(|_| {
            let t = Instant::now();
            run();
            t.elapsed()
        })
        .min()
        .unwrap_or_default()
}

fn frames(rec: &Recorder, n: u32, names: &[&'static str]) {
    let causes = [Cause::named("input")];
    for _ in 0..n {
        rec.frame_begin(FrameBegin { continuous: true });
        for name in names {
            let _g = black_box(rec.scope(name));
        }
        rec.frame_end(&FrameEnd { causes: &causes });
    }
}

fn per(d: Duration, n: u64) -> f64 {
    #[expect(clippy::cast_precision_loss, reason = "a count shown as a rate")]
    let n = n as f64;
    d.as_secs_f64() * 1e9 / n
}

fn recorder() -> Recorder {
    Recorder::new(RingRecorder::new(Settings::default()).expect("the default settings are valid"))
}

fn main() {
    let on = recorder();
    let off = recorder();
    off.set_enabled(false);
    let few: Vec<&'static str> = vec!["widget"; SCOPES as usize];
    let many: Vec<&'static str> = (0..NAMES)
        .map(|i| &*Box::leak(format!("w{i}").into_boxed_str()))
        .collect();

    let frame_on = fastest(|| frames(&on, FRAMES, &[]));
    let frame_off = fastest(|| frames(&off, FRAMES, &[]));
    let scoped_on = fastest(|| frames(&on, FRAMES, &few));
    let scoped_off = fastest(|| frames(&off, FRAMES, &few));
    let scope_on = per(
        scoped_on.saturating_sub(frame_on),
        u64::from(FRAMES * SCOPES),
    );
    let scope_off = per(
        scoped_off.saturating_sub(frame_off),
        u64::from(FRAMES * SCOPES),
    );
    // A hundred distinct names a frame: the name lookup and the entry merge stay constant.
    let many_frames = FRAMES / 10;
    let many_on = fastest(|| frames(&on, many_frames, &many));
    let base = frame_on / 10;
    let scope_many = per(
        many_on.saturating_sub(base),
        u64::from(many_frames) * NAMES as u64,
    );

    // A breakdown of 60 s at 120 Hz with a hundred scopes a frame, rings sized to hold them.
    let sizing = Sizing {
        entries_bytes: 8 * NAMES * 120 * 300,
        ..Sizing::default()
    };
    let big = Recorder::new(
        RingRecorder::with_sizing(
            Settings::default(),
            sizing,
            ve_basics::perf::SystemClock::default(),
        )
        .expect("the default settings are valid"),
    );
    frames(&big, 120 * 60, &many);
    let mut b = Breakdown::default();
    let span = big.last(Duration::from_secs(3600));
    let breakdown = fastest(|| big.breakdown(span, &mut b));

    let settings = Settings::default();
    let sizing = Sizing::default();
    let (records, entries) = (sizing.records(&settings), sizing.entries());
    let bytes = sizing.bytes(&settings);
    println!("frame recorder, fastest of {RUNS} runs");
    println!(
        "  frame, recording on                 {:7.1} ns",
        per(frame_on, u64::from(FRAMES))
    );
    println!(
        "  frame, recording off                {:7.1} ns",
        per(frame_off, u64::from(FRAMES))
    );
    println!("  scope, recording on                 {scope_on:7.1} ns");
    println!("  scope, recording off                {scope_off:7.1} ns");
    println!("  scope, on, {NAMES} distinct names      {scope_many:7.1} ns");
    println!(
        "  breakdown of {} frames x {NAMES} scopes {:7.2} ms",
        120 * 60,
        breakdown.as_secs_f64() * 1e3
    );
    #[expect(clippy::cast_precision_loss, reason = "megabytes shown with 2 digits")]
    let mb = bytes as f64 / 1e6;
    println!(
        "  rings for {} s at {} Hz: {records} frames and {entries} scope entries, {mb:.2} MB",
        settings.retention.as_secs(),
        sizing.rate_hz,
    );
}
