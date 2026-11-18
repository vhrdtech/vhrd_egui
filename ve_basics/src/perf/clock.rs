//! The time source of a recorder: the system's monotonic clock, or one a test moves by hand.

/// A monotonic clock.
pub trait Clock: Send + Sync {
    /// Nanoseconds since an origin of the clock's own; never goes back.
    fn now_ns(&self) -> u64;
}

/// The system's monotonic clock (`performance.now()` on the web).
#[derive(Clone, Copy, Debug)]
pub struct SystemClock {
    origin: web_time::Instant,
}

impl Default for SystemClock {
    #[cfg_attr(
        not(target_arch = "wasm32"),
        expect(
            clippy::disallowed_methods,
            reason = "web_time::Instant is the clock that works on the web"
        )
    )]
    fn default() -> Self {
        Self {
            origin: web_time::Instant::now(),
        }
    }
}

impl Clock for SystemClock {
    fn now_ns(&self) -> u64 {
        u64::try_from(self.origin.elapsed().as_nanos()).unwrap_or(u64::MAX)
    }
}

/// A clock that moves only when told to; clones share it. For tests.
#[cfg(any(test, feature = "test-support"))]
#[derive(Clone, Debug, Default)]
pub struct ManualClock(std::sync::Arc<std::sync::atomic::AtomicU64>);

#[cfg(any(test, feature = "test-support"))]
impl ManualClock {
    /// A clock at 0.
    pub fn new() -> Self {
        Self::default()
    }

    /// Move it forward by `d`.
    pub fn advance(&self, d: std::time::Duration) {
        let ns = u64::try_from(d.as_nanos()).unwrap_or(u64::MAX);
        self.0.fetch_add(ns, std::sync::atomic::Ordering::Relaxed);
    }

    /// Move it forward by `ms` milliseconds.
    pub fn advance_ms(&self, ms: f64) {
        self.advance(std::time::Duration::from_secs_f64(ms / 1000.0));
    }
}

#[cfg(any(test, feature = "test-support"))]
impl Clock for ManualClock {
    fn now_ns(&self) -> u64 {
        self.0.load(std::sync::atomic::Ordering::Relaxed)
    }
}
