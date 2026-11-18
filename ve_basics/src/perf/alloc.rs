//! An allocator that counts: allocations per frame for the recorder, when an app installs it.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering};

static ALLOCS: AtomicU64 = AtomicU64::new(0);
static ALLOC_BYTES: AtomicU64 = AtomicU64::new(0);

/// The system allocator, counting allocations and their bytes. Install it in the app's binary:
///
/// ```ignore
/// #[global_allocator]
/// static ALLOC: ve_basics::perf::CountingAlloc = ve_basics::perf::CountingAlloc;
/// ```
///
/// A recorder then counts `alloc.count` and `alloc.bytes` per frame. Each allocation costs two relaxed atomic adds.
#[derive(Clone, Copy, Debug, Default)]
pub struct CountingAlloc;

// SAFETY: every call goes to `System` with the caller's arguments unchanged; the counters do not touch memory
// the allocator hands out.
unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count(layout.size());
        // SAFETY: the caller upholds `GlobalAlloc::alloc`'s contract, which `System` shares.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count(layout.size());
        // SAFETY: as for `alloc`.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        count(new_size);
        // SAFETY: `ptr` and `layout` came from this allocator, which is `System`'s; the caller upholds the rest.
        unsafe { System.realloc(ptr, layout, new_size) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: `ptr` and `layout` came from this allocator, which is `System`'s.
        unsafe { System.dealloc(ptr, layout) }
    }
}

fn count(bytes: usize) {
    ALLOCS.fetch_add(1, Ordering::Relaxed);
    ALLOC_BYTES.fetch_add(bytes as u64, Ordering::Relaxed);
}

/// Allocations and their bytes since start, when [`CountingAlloc`] is the global allocator; `None` when it is
/// not (nothing counted yet).
pub fn alloc_totals() -> Option<(u64, u64)> {
    let n = ALLOCS.load(Ordering::Relaxed);
    (n > 0).then(|| (n, ALLOC_BYTES.load(Ordering::Relaxed)))
}
