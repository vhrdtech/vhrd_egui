# ve_gpu_benchmarks

Benchmarks of `ve_gpu`, headless:

- `cargo bench -p ve_gpu_benchmarks`: frames of 50 batches of about 1000 shapes (rects, segments, diamonds,
  hatching), and of 100 runs of dense range columns. Median per step (UI, tessellation, prepare, encode, GPU, wall)
  on the software adapter and, where there is one, the GPU. `-- --gpu` or `-- --software` picks one.
- `cargo test -p ve_gpu_benchmarks`: a warm frame allocates nothing in ve_gpu's code, counted by
  `ve_alloc_count` installed as the test's allocator.
