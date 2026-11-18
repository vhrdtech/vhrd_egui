//! What the recorder allocates, counted by `ve_alloc_count` installed as this binary's allocator.

use std::time::Duration;

use ve_alloc_count::{CountingAlloc, totals};
use ve_basics::perf::{Cause, FrameBegin, FrameEnd, Recorder, RingRecorder, Settings};

#[global_allocator]
static ALLOC: CountingAlloc = CountingAlloc;

fn allocs() -> u64 {
    totals().map_or(0, |(n, _)| n)
}

fn recorder() -> Recorder {
    Recorder::new(RingRecorder::new(Settings::default()).expect("the default settings are valid"))
}

fn frames(rec: &Recorder, n: u32) {
    let causes = [Cause::named("input")];
    for _ in 0..n {
        rec.frame_begin(FrameBegin { continuous: true });
        for name in ["tracks", "decode", "layout"] {
            let _g = rec.scope(name);
            rec.count("rx_bytes", 1);
        }
        drop(rec.scope_of("widget", 7));
        rec.frame_end(&FrameEnd { causes: &causes });
    }
}

/// BAS-10, review finding 18: once the name and cause tables hold their entries, recording allocates nothing.
fn a_recorded_frame_allocates_nothing() {
    let rec = recorder();
    frames(&rec, 10);
    let before = allocs();
    frames(&rec, 100_000);
    assert_eq!(allocs() - before, 0);
}

/// The counting allocator counts: a vector's allocation shows.
fn the_allocator_counts() {
    let before = allocs();
    let v = std::hint::black_box(vec![0u8; 100]);
    assert!(allocs() > before);
    drop(v);
}

/// With the feature `alloc-count`, each frame's allocations land in the counters `alloc.count` and `alloc.bytes`.
fn allocations_per_frame_are_counted() {
    let rec = recorder();
    for _ in 0..3 {
        rec.frame_begin(FrameBegin::default());
        drop(std::hint::black_box(vec![0u8; 64]));
        rec.frame_end(&FrameEnd::default());
    }
    let mut out = Vec::new();
    rec.counters(rec.last(Duration::from_secs(5)), &mut out);
    let count = out.iter().find(|c| &*c.name == "alloc.count").unwrap();
    assert!(count.total >= 3, "{out:?}");
    assert!(
        out.iter()
            .any(|c| &*c.name == "alloc.bytes" && c.total >= 3 * 64)
    );
}

/// The counts are process-wide, so the checks run one after another in one test: a test thread starting beside
/// them would allocate into their counts.
#[test]
fn allocations() {
    the_allocator_counts();
    a_recorded_frame_allocates_nothing();
    allocations_per_frame_are_counted();
}
