# ve_alloc_count

A counting global allocator. An app installs it in its binary; `ve_basics`' frame recorder (with its feature `alloc-count`) then records allocations and their bytes per frame.

```rust
#[global_allocator]
static ALLOC: ve_alloc_count::CountingAlloc = ve_alloc_count::CountingAlloc;
```

The counts are process-wide: every thread's allocations count. The crate holds the workspace's only unsafe code, the `GlobalAlloc` impl that forwards to the system allocator.
