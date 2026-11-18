//! The allocator installed in this test binary counts what it should, and nothing else.

use std::hint::black_box;

use ve_alloc_count::{CountingAlloc, totals};

#[global_allocator]
static ALLOC: CountingAlloc = CountingAlloc;

/// Allocations and bytes counted while `f` runs.
fn counted(f: impl FnOnce()) -> (u64, u64) {
    let (n0, b0) = totals().unwrap_or_default();
    f();
    let (n1, b1) = totals().unwrap_or_default();
    (n1 - n0, b1 - b0)
}

/// The counts are process-wide, so the checks run one after another in one test: a test thread starting beside
/// them would allocate into their counts.
#[test]
fn counts_allocations_and_their_bytes() {
    // Once anything allocated, the totals are there.
    drop(black_box(vec![0u8; 1]));
    assert!(totals().is_some());

    let (n, bytes) = counted(|| drop(black_box(Vec::<u8>::with_capacity(100))));
    assert_eq!((n, bytes), (1, 100), "one allocation of 100 bytes");

    let (n, bytes) = counted(|| drop(black_box(vec![0u8; 4096])));
    assert_eq!((n, bytes), (1, 4096), "a zeroed allocation counts the same");

    let mut v = black_box(Vec::<u8>::with_capacity(16));
    let (n, bytes) = counted(|| v.reserve_exact(1000));
    assert_eq!((n, bytes), (1, 1000), "a realloc counts once, its new size");

    let (n, bytes) = counted(|| drop(v));
    assert_eq!((n, bytes), (0, 0), "a free counts nothing");

    let (n, _) = counted(|| {
        let x = black_box(7u64);
        black_box(x + 1);
    });
    assert_eq!(n, 0, "no allocation, no count");
}
