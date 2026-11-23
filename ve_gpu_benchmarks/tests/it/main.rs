//! What a warm frame allocates in ve_gpu's code, counted by `ve_alloc_count` installed as this binary's allocator.

use egui::Context;
use ve_alloc_count::{CountingAlloc, totals};
use ve_gpu::Batch;
use ve_gpu::headless::{Headless, HeadlessOptions};
use ve_gpu_benchmarks::{dense, row, series};

#[global_allocator]
static ALLOC: CountingAlloc = CountingAlloc;

const BATCHES: usize = 20;

fn allocs() -> u64 {
    totals().map_or(0, |(n, _)| n)
}

/// Once the batches and the GPU buffers have their room, filling the batches, painting them and the callbacks'
/// own work (queueing and packing the frame) allocate nothing; egui's and wgpu's own work is not counted.
fn a_warm_frame_allocates_nothing() {
    let mut h = Headless::new(HeadlessOptions {
        size_px: [800, 600],
        ..HeadlessOptions::default()
    })
    .expect("a wgpu adapter");
    let ctx = Context::default();
    let mut batches: Vec<Batch> = (0..BATCHES).map(|_| Batch::new()).collect();
    for frame in 0..8 {
        let mut ours = 0;
        h.frame(&ctx, |ui| {
            let screen = ui.max_rect();
            let ppp = ui.ctx().pixels_per_point();
            for (k, b) in batches.iter_mut().enumerate() {
                let before = allocs();
                b.clear();
                let r = row(screen, k, BATCHES);
                if k % 2 == 0 {
                    dense(b, r, k);
                } else {
                    series(b, r, k, ppp);
                }
                let shape = b.paint(screen);
                ours += allocs() - before;
                ui.painter().add(shape);
            }
        })
        .expect("a frame");
        let stats = ve_gpu::frame_stats(h.render_state()).expect("installed");
        assert_eq!(stats.batches, BATCHES);
        if frame >= 3 {
            assert_eq!(ours, 0, "frame {frame}: filling and painting the batches");
            assert_eq!(stats.allocations, Some(0), "frame {frame}: the callbacks");
        }
    }
}

/// The counting allocator counts, so a zero above means something.
fn the_allocator_counts() {
    let before = allocs();
    let v = std::hint::black_box(vec![0u8; 100]);
    assert!(allocs() > before);
    drop(v);
}

/// The counts are process-wide, so the checks run one after another in one test.
#[test]
fn allocations() {
    the_allocator_counts();
    a_warm_frame_allocates_nothing();
}
