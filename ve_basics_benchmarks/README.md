# ve_basics_benchmarks

Benchmarks of `ve_basics`, kept apart so the library carries no bench code.

`cargo bench -p ve_basics_benchmarks` prints what the frame recorder (`ve_basics::perf`) costs: per frame with
recording on and off, per scope, and the memory of its rings. It also checks that a recorded frame allocates nothing,
through the counting allocator installed in the bench binary.
