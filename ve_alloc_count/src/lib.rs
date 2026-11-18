//! A counting global allocator: [`CountingAlloc`] forwards to the system allocator and counts each allocation
//! and its bytes; [`totals`] reads the counts.
//!
//! The counts are process-wide: every thread's allocations count, not only the UI thread's. A `realloc` counts
//! as one allocation of its new size. Each count is one relaxed atomic add on a cache line of its own.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering};

/// A counter alone on its cache line, so the two counters do not contend.
#[repr(align(64))]
struct Padded(AtomicU64);

static ALLOCS: Padded = Padded(AtomicU64::new(0));
static ALLOC_BYTES: Padded = Padded(AtomicU64::new(0));

/// The system allocator, counting allocations and their bytes. Install it in the app's binary:
///
/// ```ignore
/// #[global_allocator]
/// static ALLOC: ve_alloc_count::CountingAlloc = ve_alloc_count::CountingAlloc;
/// ```
#[derive(Clone, Copy, Debug, Default)]
pub struct CountingAlloc;

// SAFETY: every call goes to `System` with the caller's arguments unchanged; the counters do not touch memory the
// allocator hands out.
#[expect(unsafe_code, reason = "a GlobalAlloc impl is unsafe by definition")]
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
    ALLOCS.0.fetch_add(1, Ordering::Relaxed);
    ALLOC_BYTES.0.fetch_add(bytes as u64, Ordering::Relaxed);
}

/// Allocations and their bytes since start, process-wide, when [`CountingAlloc`] is the global allocator; `None`
/// when it is not (nothing counted yet).
pub fn totals() -> Option<(u64, u64)> {
    let n = ALLOCS.0.load(Ordering::Relaxed);
    (n > 0).then(|| (n, ALLOC_BYTES.0.load(Ordering::Relaxed)))
}
