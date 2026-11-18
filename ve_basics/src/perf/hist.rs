//! A fixed-size log histogram of durations: percentiles without sorting or allocating.

use std::time::Duration;

use super::Summary;

/// Bins per doubling: a bin is at most 3.1 % wide, its middle within 1.6 % of any value in it.
pub const HIST_BINS_PER_OCTAVE: usize = 32;
/// Values below this (1.024 µs) share the first bin.
pub const HIST_FLOOR_NS: u64 = 1 << FLOOR_LOG2;
const FLOOR_LOG2: u32 = 10;
/// Doublings above the floor: up to 17 s; longer values share the last bin.
const OCTAVES: usize = 24;
/// Bins in a [`Histogram`]: the floor bin, then [`HIST_BINS_PER_OCTAVE`] per doubling.
pub const HIST_BINS: usize = 1 + OCTAVES * HIST_BINS_PER_OCTAVE;
const SUB_LOG2: u32 = HIST_BINS_PER_OCTAVE.trailing_zeros();
const _: () = assert!(
    HIST_BINS_PER_OCTAVE.is_power_of_two(),
    "bin index math shifts by log2 of the bins per octave"
);

/// Durations counted into log bins; percentiles read back from the bins, min and max exact.
#[derive(Clone, Debug)]
pub struct Histogram {
    bins: [u32; HIST_BINS],
    count: u64,
    sum_ns: u128,
    min_ns: u64,
    max_ns: u64,
}

impl Default for Histogram {
    fn default() -> Self {
        Self {
            bins: [0; HIST_BINS],
            count: 0,
            sum_ns: 0,
            min_ns: u64::MAX,
            max_ns: 0,
        }
    }
}

impl Histogram {
    /// An empty histogram.
    pub fn new() -> Self {
        Self::default()
    }

    /// Empty it.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Count `n` values of `ns` nanoseconds.
    pub fn add_n(&mut self, ns: u64, n: u32) {
        if n == 0 {
            return;
        }
        let bin = &mut self.bins[bin_of(ns)];
        *bin = bin.saturating_add(n);
        self.count += u64::from(n);
        self.sum_ns += u128::from(ns) * u128::from(n);
        self.min_ns = self.min_ns.min(ns);
        self.max_ns = self.max_ns.max(ns);
    }

    /// Count one value.
    pub fn add(&mut self, ns: u64) {
        self.add_n(ns, 1);
    }

    /// Values counted.
    pub fn count(&self) -> u64 {
        self.count
    }

    /// The counts per bin, from the floor up.
    pub fn bins(&self) -> &[u32; HIST_BINS] {
        &self.bins
    }

    /// The value at quantile `q` (0..=1), within 1.6 % of the true one; 0 when empty.
    pub fn quantile_ns(&self, q: f64) -> u64 {
        if self.count == 0 {
            return 0;
        }
        if q >= 1.0 {
            return self.max_ns;
        }
        if q <= 0.0 {
            return self.min_ns;
        }
        // The rank of the value at q among the counted ones, 1-based (nearest-rank method).
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss,
            reason = "a rank between 1 and the count, which fits an f64 exactly up to 2^53"
        )]
        let rank = ((q * self.count as f64).ceil() as u64).max(1);
        let mut seen = 0u64;
        for (i, &c) in self.bins.iter().enumerate() {
            seen += u64::from(c);
            if seen >= rank {
                return bin_middle_ns(i).clamp(self.min_ns, self.max_ns);
            }
        }
        self.max_ns
    }

    /// Last, mean, median, p95, p99 and worst, with `last` given by the caller.
    pub fn summary(&self, last_ns: u64) -> Summary {
        if self.count == 0 {
            return Summary::default();
        }
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a mean of u64 values fits a u64"
        )]
        let mean = (self.sum_ns / u128::from(self.count)) as u64;
        Summary {
            count: u32::try_from(self.count).unwrap_or(u32::MAX),
            last: Duration::from_nanos(last_ns),
            mean: Duration::from_nanos(mean),
            median: Duration::from_nanos(self.quantile_ns(0.5)),
            p95: Duration::from_nanos(self.quantile_ns(0.95)),
            p99: Duration::from_nanos(self.quantile_ns(0.99)),
            worst: Duration::from_nanos(self.max_ns),
        }
    }

    /// The range of values bin `i` holds, in nanoseconds: `[lo, hi)`.
    pub fn bin_range_ns(i: usize) -> (u64, u64) {
        if i == 0 {
            return (0, HIST_FLOOR_NS);
        }
        let octave = u32::try_from((i - 1) / HIST_BINS_PER_OCTAVE).unwrap_or(u32::MAX);
        let sub = ((i - 1) % HIST_BINS_PER_OCTAVE) as u64;
        let shift = FLOOR_LOG2 + octave - SUB_LOG2;
        let base = HIST_BINS_PER_OCTAVE as u64;
        ((base + sub) << shift, (base + sub + 1) << shift)
    }
}

fn bin_of(ns: u64) -> usize {
    if ns < HIST_FLOOR_NS {
        return 0;
    }
    let log2 = 63 - ns.leading_zeros();
    let octave = (log2 - FLOOR_LOG2) as usize;
    if octave >= OCTAVES {
        return HIST_BINS - 1;
    }
    let sub = ((ns >> (log2 - SUB_LOG2)) as usize) & (HIST_BINS_PER_OCTAVE - 1);
    1 + octave * HIST_BINS_PER_OCTAVE + sub
}

fn bin_middle_ns(i: usize) -> u64 {
    let (lo, hi) = Histogram::bin_range_ns(i);
    lo + (hi - lo) / 2
}
