# ve_basics_benchmarks

Benchmarks of `ve_basics`, kept apart so the library carries no bench code.

`cargo bench -p ve_basics_benchmarks` prints what the frame recorder (`ve_basics::perf`) costs: per frame with recording on and off, per scope with few and with a hundred distinct names, a breakdown read, and the memory of its rings. `cargo test -p ve_basics_benchmarks` checks, through the counting allocator (`ve_alloc_count`), that a recorded frame allocates nothing.
