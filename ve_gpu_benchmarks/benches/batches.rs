//! Frames of dense batches through the headless harness: 50 batches of about 1000 shapes, and 100 runs of dense
//! range columns, on the software adapter and the GPU. Prints the median of each step over the measured frames.
//!
//! `cargo bench -p ve_gpu_benchmarks [-- --gpu | --software]`

use std::time::Duration;

use egui::Context;
use ve_gpu::Batch;
use ve_gpu::headless::{AdapterPick, FrameTimes, Headless, HeadlessOptions};
use ve_gpu_benchmarks::{dense, row, series};

const WARM: usize = 5;
const FRAMES: usize = 40;
const SIZE_PX: [u32; 2] = [2000, 2000];

type Fill = fn(&mut Batch, egui::Rect, usize, f32);

fn median(mut v: Vec<Duration>) -> Duration {
    v.sort();
    v.get(v.len() / 2).copied().unwrap_or_default()
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1e3
}

fn run(pick: AdapterPick, label: &str, batches: usize, fill: Fill) {
    let mut h = match Headless::new(HeadlessOptions {
        adapter: pick,
        size_px: SIZE_PX,
        ..HeadlessOptions::default()
    }) {
        Ok(h) => h,
        Err(e) => {
            let cause = std::error::Error::source(&e)
                .map(|c| format!(": {c}"))
                .unwrap_or_default();
            println!("{label} on {pick:?}: skipped: {e}{cause}");
            return;
        }
    };
    let ctx = Context::default();
    let mut all: Vec<Batch> = (0..batches).map(|_| Batch::new()).collect();
    let mut times: Vec<FrameTimes> = Vec::new();
    for frame in 0..WARM + FRAMES {
        let t = h
            .frame(&ctx, |ui| {
                let screen = ui.max_rect();
                let ppp = ui.ctx().pixels_per_point();
                for (k, b) in all.iter_mut().enumerate() {
                    b.clear();
                    fill(b, row(screen, k, batches), k, ppp);
                    ui.painter().add(b.paint(screen));
                }
            })
            .expect("a frame");
        if frame >= WARM {
            times.push(t);
        }
    }
    let stats = ve_gpu::frame_stats(h.render_state()).expect("installed");
    let col = |f: fn(&FrameTimes) -> Duration| ms(median(times.iter().map(f).collect()));
    let gpu = if h.has_timestamps() {
        format!("{:.2}", col(|t| t.gpu.unwrap_or_default()))
    } else {
        "-".to_owned()
    };
    println!(
        "{label:<24} ui {:>7.2}  tessellate {:>6.2}  prepare {:>6.2}  encode {:>6.2}  gpu {gpu:>6}  wall {:>7.2} ms  \
         | {} shapes, {} columns, {} draws, {:.2} MB up",
        col(|t| t.ui),
        col(|t| t.tessellate),
        col(|t| t.prepare),
        col(|t| t.encode),
        col(|t| t.wall),
        stats.shapes,
        stats.columns,
        stats.draws,
        stats.upload_bytes as f64 / 1e6,
    );
    println!("{:<24} on {}", "", h.adapter_name());
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let picks: &[AdapterPick] = if args.iter().any(|a| a == "--gpu") {
        &[AdapterPick::Gpu]
    } else if args.iter().any(|a| a == "--software") {
        &[AdapterPick::Software]
    } else {
        &[AdapterPick::Software, AdapterPick::Gpu]
    };
    for &pick in picks {
        run(pick, "50 x 1000 shapes", 50, |b, r, k, _| dense(b, r, k));
        run(pick, "100 x 4000 columns", 100, series);
    }
}
