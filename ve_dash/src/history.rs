//! Fixed-capacity ring buffer for time series shown in [`crate::Sparkline`].

use std::collections::VecDeque;

/// Last-N samples of a metric. Push from your polling code, hand
/// [`History::values`] to a sparkline each frame.
#[derive(Clone, Debug, Default)]
pub struct History {
    buf: VecDeque<f32>,
    cap: usize,
}

impl History {
    /// A history keeping at most `cap` samples (at least 2).
    pub fn new(cap: usize) -> Self {
        let cap = cap.max(2);
        Self {
            buf: VecDeque::with_capacity(cap),
            cap,
        }
    }

    /// Append a sample, dropping the oldest when full. Non-finite values are stored as 0.
    pub fn push(&mut self, v: f32) {
        if self.buf.len() == self.cap {
            self.buf.pop_front();
        }
        self.buf.push_back(if v.is_finite() { v } else { 0.0 });
    }

    /// The samples oldest → newest as one slice.
    pub fn values(&mut self) -> &[f32] {
        self.buf.make_contiguous()
    }

    /// The newest sample, if any.
    pub fn last(&self) -> Option<f32> {
        self.buf.back().copied()
    }

    pub fn len(&self) -> usize {
        self.buf.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.cap
    }

    pub fn clear(&mut self) {
        self.buf.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_and_stays_ordered() {
        let mut h = History::new(3);
        for v in [1.0, 2.0, 3.0, 4.0, 5.0] {
            h.push(v);
        }
        assert_eq!(h.values(), &[3.0, 4.0, 5.0]);
        assert_eq!(h.last(), Some(5.0));
        assert_eq!(h.len(), 3);
    }

    #[test]
    fn sanitizes_non_finite() {
        let mut h = History::new(4);
        h.push(f32::NAN);
        h.push(f32::INFINITY);
        assert_eq!(h.values(), &[0.0, 0.0]);
    }

    #[test]
    fn min_capacity_is_two() {
        let mut h = History::new(0);
        h.push(1.0);
        h.push(2.0);
        h.push(3.0);
        assert_eq!(h.values(), &[2.0, 3.0]);
    }
}
