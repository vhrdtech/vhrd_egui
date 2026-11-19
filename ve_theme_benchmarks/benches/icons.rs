//! The first raster of each SVG icon at 16, 24 and 32 px, as the cache does it on an icon's first use at a
//! pixel size (CHR-7): parsing the SVG once, then rendering every layer's mask (4 times supersampled).
//!
//! Each case runs `ROUNDS` times, five times over; the table shows the fastest run, per raster.

use std::hint::black_box;
use std::time::{Duration, Instant};

use ve_theme::icons::{Shapes, own};

const ROUNDS: u32 = 2_000;
const RUNS: usize = 5;
const SIZES_PX: [u32; 3] = [16, 24, 32];

fn fastest(mut run: impl FnMut()) -> Duration {
    (0..RUNS)
        .map(|_| {
            let t = Instant::now();
            run();
            t.elapsed()
        })
        .min()
        .unwrap_or_default()
}

fn per_round_us(d: Duration) -> f64 {
    d.as_secs_f64() * 1e6 / f64::from(ROUNDS)
}

fn main() {
    println!(
        "{:<12} {:>5} {:>7} {:>9} {:>10} {:>9}",
        "icon", "px", "layers", "parse µs", "render µs", "total µs"
    );
    for icon in own::ALL {
        let parse = per_round_us(fastest(|| {
            for _ in 0..ROUNDS {
                let _shapes = black_box(Shapes::parse(black_box(icon.svg)));
            }
        }));
        let Ok(shapes) = Shapes::parse(icon.svg) else {
            println!("{:<12} does not parse", icon.name);
            continue;
        };
        for px in SIZES_PX {
            let layers = shapes.render(px).map_or(0, |m| m.len());
            let render = per_round_us(fastest(|| {
                for _ in 0..ROUNDS {
                    let _masks = black_box(shapes.render(black_box(px)));
                }
            }));
            println!(
                "{:<12} {px:>5} {layers:>7} {parse:>9.1} {render:>10.1} {:>9.1}",
                icon.name,
                parse + render
            );
        }
    }
}
