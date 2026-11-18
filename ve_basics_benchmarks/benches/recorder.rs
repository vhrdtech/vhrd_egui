//! What the frame recorder costs: per frame on and off, per scope on and off, and its memory.
//!
//! Each case runs `FRAMES` frames, five times; the table shows the fastest run, per frame or per scope.

use std::hint::black_box;
use std::time::{Duration, Instant};

use ve_basics::perf::{
    Cause, CountingAlloc, FrameBegin, FrameEnd, Recorder, RingRecorder, Settings, Sizing,
    alloc_totals,
};

#[global_allocator]
static ALLOC: CountingAlloc = CountingAlloc;

const FRAMES: u32 = 1_000_000;
const RUNS: usize = 5;
const SCOPES: u32 = 10;

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

fn frames(rec: &Recorder, scopes: u32) {
    let causes = [Cause::named("input")];
    for _ in 0..FRAMES {
        rec.frame_begin(FrameBegin { continuous: true });
        for _ in 0..scopes {
            let _g = black_box(rec.scope("widget"));
        }
        rec.frame_end(&FrameEnd { causes: &causes });
    }
}

fn per(d: Duration, n: u32) -> f64 {
    d.as_secs_f64() * 1e9 / f64::from(n)
}

fn main() {
    let on = Recorder::new(RingRecorder::new(Settings::default()));
    let off = Recorder::new(RingRecorder::new(Settings::default()));
    off.set_enabled(false);

    let frame_on = fastest(|| frames(&on, 0));
    let frame_off = fastest(|| frames(&off, 0));
    let scoped_on = fastest(|| frames(&on, SCOPES));
    let scoped_off = fastest(|| frames(&off, SCOPES));
    let scope_on = per(scoped_on.saturating_sub(frame_on), FRAMES * SCOPES);
    let scope_off = per(scoped_off.saturating_sub(frame_off), FRAMES * SCOPES);

    // A recorded frame allocates nothing once the name and cause tables hold their entries.
    let before = alloc_totals().map_or(0, |(n, _)| n);
    frames(&on, SCOPES);
    let allocs = alloc_totals().map_or(0, |(n, _)| n) - before;

    let settings = Settings::default();
    let sizing = Sizing::default();
    println!("frame recorder, {FRAMES} frames, fastest of {RUNS} runs");
    println!("  frame, recording on   {:7.1} ns", per(frame_on, FRAMES));
    println!("  frame, recording off  {:7.1} ns", per(frame_off, FRAMES));
    println!("  scope, recording on   {scope_on:7.1} ns");
    println!("  scope, recording off  {scope_off:7.1} ns");
    println!(
        "  rings for {} s at {} Hz, {} entries a frame: {:.2} MB ({} B a frame, {} B an entry)",
        settings.retention.as_secs(),
        sizing.rate_hz,
        sizing.entries_per_record,
        sizing.bytes(&settings) as f64 / 1e6,
        sizing.bytes(&settings) / sizing.records(&settings)
            - 8 * sizing.entries_per_record as usize,
        8,
    );
    println!("  allocations in {FRAMES} recorded frames: {allocs}");
    assert_eq!(allocs, 0, "recording a frame must not allocate");
}
