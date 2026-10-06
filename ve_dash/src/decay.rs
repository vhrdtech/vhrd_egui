//! Analog-needle smoothing for indicators fed with jumpy per-frame data.

/// Asymmetric exponential smoother: fast attack, slow release, like the
/// needle of an analog VU meter. Feed it the raw value every frame and
/// display [`Decay::value`] instead — rises snap, falls glide, and the
/// eye can actually follow a 60 Hz load bar.
#[derive(Clone, Debug)]
pub struct Decay {
    value: f32,
    /// Time constant going up, seconds.
    attack: f32,
    /// Time constant going down, seconds.
    release: f32,
}

impl Decay {
    /// VU-ish defaults: 0.1 s attack, 0.6 s release.
    pub fn new() -> Self {
        Self::with_times(0.1, 0.6)
    }

    /// Custom time constants (seconds to cover ~63 % of a step).
    /// A constant of 0 or less passes that direction through unsmoothed.
    pub fn with_times(attack: f32, release: f32) -> Self {
        Self {
            value: 0.0,
            attack,
            release,
        }
    }

    /// Move toward `target` by `dt` seconds and return the new value.
    /// Non-finite targets are ignored.
    pub fn update(&mut self, target: f32, dt: f32) -> f32 {
        if target.is_finite() {
            let tau = if target > self.value {
                self.attack
            } else {
                self.release
            };
            if tau > 0.0 {
                self.value += (target - self.value) * (1.0 - (-dt / tau).exp());
            } else {
                self.value = target;
            }
        }
        self.value
    }

    /// The current smoothed value.
    pub fn value(&self) -> f32 {
        self.value
    }

    /// Jump straight to `v` (e.g. on reset), skipping the smoothing.
    pub fn set(&mut self, v: f32) {
        if v.is_finite() {
            self.value = v;
        }
    }
}

impl Default for Decay {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rises_fast_falls_slow() {
        let mut d = Decay::with_times(0.1, 0.6);
        let up = d.update(1.0, 0.1); // one attack constant
        assert!(up > 0.6, "attack should cover ~63 % in one tau, got {up}");
        d.set(1.0);
        let down = d.update(0.0, 0.1); // a sixth of the release constant
        assert!(
            down > 0.8,
            "release should only drop ~15 % in dt = tau/6, got {down}"
        );
    }

    #[test]
    fn converges_and_ignores_garbage() {
        let mut d = Decay::new();
        for _ in 0..600 {
            d.update(0.5, 1.0 / 60.0);
        }
        assert!((d.value() - 0.5).abs() < 1e-3);
        d.update(f32::NAN, 0.016);
        d.update(f32::INFINITY, 0.016);
        assert!((d.value() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn zero_tau_passes_through() {
        let mut d = Decay::with_times(0.0, 0.0);
        assert_eq!(d.update(0.7, 0.016), 0.7);
    }
}
