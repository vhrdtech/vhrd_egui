//! Everything a person can tune about the phosphor view, with its default.

/// What the view draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Scene {
    /// A clock (square wave) with edge jitter, ringing and noise, plus rare events: a runt pulse, a narrow
    /// spike, a late edge ([`Signal`] has their rates).
    #[default]
    Clock,
    /// Spectrum persistence (DPX style): a strong carrier with its harmonic and phase-noise skirt over a noise
    /// floor more than 90 dB below it, and a short frequency-hopping burst every few seconds.
    Spectrum,
    /// An eye diagram of random NRZ data with jitter, inter-symbol interference and noise, plus rare marginal
    /// bits and crosstalk spikes.
    Eye,
    /// Sweeps the application streams in ([`crate::Phosphor::push_sweeps`]).
    Stream,
}

impl Scene {
    /// Every scene, in menu order.
    pub const ALL: [Self; 4] = [Self::Clock, Self::Spectrum, Self::Eye, Self::Stream];

    /// Short name for a menu.
    pub fn label(self) -> &'static str {
        match self {
            Self::Clock => "Clock + glitches",
            Self::Spectrum => "Spectrum (DPX)",
            Self::Eye => "Eye diagram",
            Self::Stream => "Streamed",
        }
    }

    /// One line on what it shows, for a tooltip.
    pub fn about(self) -> &'static str {
        match self {
            Self::Clock => {
                "A jittery clock with ringing and noise. Rare events: a runt pulse (1 in 100 000 sweeps), a narrow spike (1 in 1 000 000) and a late edge (1 in 10 000). They show as faint ghosts beside the glowing trace."
            }
            Self::Spectrum => {
                "Spectrum persistence: a -10 dBm carrier with harmonic and phase-noise skirt over a -100 dBm noise floor, 120 dB on screen, and a short frequency-hopping burst every few seconds."
            }
            Self::Eye => {
                "Eye diagram of random NRZ data: jitter, inter-symbol interference, noise; rare marginal bits and crosstalk spikes reach into the eye."
            }
            Self::Stream => "Sweeps the application pushes in (Phosphor::push_sweeps).",
        }
    }

    pub(crate) fn index(self) -> u32 {
        match self {
            Self::Clock => 0,
            Self::Spectrum => 1,
            Self::Eye => 2,
            Self::Stream => 3,
        }
    }
}

/// How the density becomes light.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Mode {
    /// The HDR tone map: phosphor light up to the peak headroom.
    Hdr,
    /// The classic SDR tone map, clamped to SDR white.
    Sdr,
    /// HDR on the left of the divider, SDR on the right: the same picture, side by side.
    #[default]
    Split,
}

impl Mode {
    /// Every mode, in menu order.
    pub const ALL: [Self; 3] = [Self::Hdr, Self::Sdr, Self::Split];

    /// Short name for a menu.
    pub fn label(self) -> &'static str {
        match self {
            Self::Hdr => "HDR",
            Self::Sdr => "SDR",
            Self::Split => "HDR | SDR",
        }
    }

    pub(crate) fn index(self) -> f32 {
        match self {
            Self::Hdr => 0.0,
            Self::Sdr => 1.0,
            Self::Split => 2.0,
        }
    }
}

/// The colours of one side.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Palette {
    /// One phosphor colour, brighter with density, whitening in the hottest part.
    #[default]
    Phosphor,
    /// The classic false-colour scale: blue for rare, through green and yellow, red and white for dense.
    Temperature,
    /// Hue = time since the last hit (fresh white-cyan to old violet), brightness = density.
    Age,
}

impl Palette {
    /// Every palette, in menu order.
    pub const ALL: [Self; 3] = [Self::Phosphor, Self::Temperature, Self::Age];

    /// Short name for a menu.
    pub fn label(self) -> &'static str {
        match self {
            Self::Phosphor => "Phosphor",
            Self::Temperature => "Temperature",
            Self::Age => "Age",
        }
    }

    /// One line on what it shows, for a tooltip.
    pub fn about(self) -> &'static str {
        match self {
            Self::Phosphor => {
                "One phosphor colour: brightness says how often a pixel is hit, the hottest part turns white."
            }
            Self::Temperature => {
                "The classic false-colour scale of digital-phosphor scopes: blue for rare, red and white for dense."
            }
            Self::Age => {
                "Hue says how long ago a pixel was last hit (fresh white-cyan, old violet), brightness how often: a CRT can't do this."
            }
        }
    }

    pub(crate) fn index(self) -> f32 {
        match self {
            Self::Phosphor => 0.0,
            Self::Temperature => 1.0,
            Self::Age => 2.0,
        }
    }
}

/// The generated signals' parameters. Times and positions are in screen widths, levels in divisions (the
/// screen is ±4 divisions high).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Signal {
    /// Seed of the random numbers (noise, jitter, rare events, data bits).
    pub seed: u32,
    /// σ of the vertical noise, divisions.
    pub noise: f32,
    /// σ of the edge jitter, as a fraction of the clock period (of the unit interval in the eye).
    pub jitter: f32,
    /// Clock period, screen widths.
    pub period: f32,
    /// Clock and eye swing, divisions (low to high).
    pub swing: f32,
    /// Rise time (10-90 % roughly), screen widths.
    pub rise: f32,
    /// Overshoot and ringing after an edge, fraction of the swing.
    pub ringing: f32,
    /// Runt pulses per sweep (clock), marginal bits per sweep (eye).
    pub runt_rate: f32,
    /// Narrow spikes per sweep.
    pub spike_rate: f32,
    /// Late edges per sweep (clock).
    pub late_rate: f32,
    /// How late a late edge comes, fraction of the period.
    pub late_by: f32,
    /// Spectrum: noise floor, dBm per bin.
    pub floor_dbm: f32,
    /// Spectrum: carrier power, dBm.
    pub carrier_dbm: f32,
    /// Spectrum: seconds from one hopping burst to the next.
    pub burst_period_s: f32,
    /// Spectrum: how long a burst lasts, seconds.
    pub burst_len_s: f32,
    /// Spectrum: how long the burst stays on one frequency, seconds.
    pub hop_dwell_s: f32,
    /// Spectrum: the burst's power, dBm.
    pub burst_dbm: f32,
}

impl Default for Signal {
    fn default() -> Self {
        Self {
            seed: 0x5eed_1636,
            noise: 0.05,
            jitter: 0.012,
            period: 0.4,
            swing: 3.0,
            rise: 0.004,
            ringing: 0.08,
            runt_rate: 1.0e-5,
            spike_rate: 1.0e-6,
            late_rate: 1.0e-4,
            late_by: 0.15,
            floor_dbm: -100.0,
            carrier_dbm: -10.0,
            burst_period_s: 3.0,
            burst_len_s: 0.024,
            hop_dwell_s: 0.003,
            burst_dbm: -45.0,
        }
    }
}

/// The view's settings: scene, tone map, look. `Default` is the demo's starting point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    /// What is drawn.
    pub scene: Scene,
    /// HDR, SDR or both side by side.
    pub mode: Mode,
    /// Where the divider of the split view sits, fraction of the width from the left.
    pub split: f32,
    /// Sweeps (waveforms) drawn per second; per frame at most [`Settings::MAX_SWEEPS_PER_FRAME`].
    pub sweeps_per_s: f32,
    /// Persistence time constant, seconds (a hit fades to 1/e in this time).
    pub persistence_s: f32,
    /// Keep every hit for ever (infinite persistence).
    pub infinite: bool,
    /// Exposure: multiplies the density before the log scale, so rare hits come up.
    pub intensity: f32,
    /// Luminance of a single hit (the ghost floor), linear, 1.0 = SDR white.
    pub ghost: f32,
    /// Peak of the HDR side, × SDR white: the display's headroom.
    pub peak: f32,
    /// Densities below this many hits are black, so decayed ghosts go away.
    pub cutoff: f32,
    /// Palette of the HDR side.
    pub hdr_palette: Palette,
    /// Palette of the SDR side.
    pub sdr_palette: Palette,
    /// Colour of the phosphor palette, linear RGB (P31-ish green by default).
    pub phosphor_color: [f32; 3],
    /// Seconds the age palette spans, fresh to old.
    pub age_span_s: f32,
    /// Bloom around the hottest pixels of the HDR side, 0 = off.
    pub bloom: f32,
    /// Light above this (× SDR white) blooms.
    pub bloom_threshold: f32,
    /// Bloom radius, physical pixels.
    pub bloom_radius: f32,
    /// Graticule brightness, linear (kept dim: OLED burn-in).
    pub graticule: f32,
    /// On an output that can't go past SDR white, scale the HDR side down by its peak (darker, but its
    /// gradation shows) instead of clipping it.
    pub fit_to_sdr: bool,
    /// The generated signals.
    pub signal: Signal,
}

impl Settings {
    /// Most sweeps one frame draws (a GPU dispatch dimension).
    pub const MAX_SWEEPS_PER_FRAME: u32 = 65_535;
    /// Largest bloom radius, physical pixels.
    pub const MAX_BLOOM_RADIUS: f32 = 32.0;

    /// The settings with every number in its range.
    pub fn sanitized(mut self) -> Self {
        let fin = |v: f32, d: f32| if v.is_finite() { v } else { d };
        self.split = fin(self.split, 0.5).clamp(0.0, 1.0);
        self.sweeps_per_s = fin(self.sweeps_per_s, 0.0).clamp(0.0, 1.0e8);
        self.persistence_s = fin(self.persistence_s, 1.0).clamp(1.0e-3, 1.0e4);
        self.intensity = fin(self.intensity, 1.0).clamp(1.0e-3, 1.0e4);
        self.ghost = fin(self.ghost, 0.03).clamp(1.0e-4, 1.0);
        self.peak = fin(self.peak, 8.0).clamp(1.0, 100.0);
        self.cutoff = fin(self.cutoff, 0.05).clamp(0.0, 10.0);
        self.age_span_s = fin(self.age_span_s, 1.0).clamp(1.0e-3, 1.0e4);
        self.bloom = fin(self.bloom, 0.0).clamp(0.0, 4.0);
        self.bloom_threshold = fin(self.bloom_threshold, 1.0).clamp(0.0, 100.0);
        self.bloom_radius = fin(self.bloom_radius, 6.0).clamp(1.0, Self::MAX_BLOOM_RADIUS);
        self.graticule = fin(self.graticule, 0.04).clamp(0.0, 1.0);
        for c in &mut self.phosphor_color {
            *c = fin(*c, 0.5).clamp(0.0, 1.0);
        }
        let s = &mut self.signal;
        s.period = fin(s.period, 0.4).clamp(0.02, 4.0);
        s.rise = fin(s.rise, 0.004).clamp(1.0e-5, 0.2);
        s.burst_period_s = fin(s.burst_period_s, 3.0).max(1.0e-3);
        s.hop_dwell_s = fin(s.hop_dwell_s, 0.003).max(1.0e-5);
        for r in [&mut s.runt_rate, &mut s.spike_rate, &mut s.late_rate] {
            *r = fin(*r, 0.0).clamp(0.0, 1.0);
        }
        self
    }

    /// Sweeps to draw in a frame of `dt` seconds.
    pub fn sweeps_in(&self, dt: f32) -> u32 {
        (self.sweeps_per_s * dt.max(0.0))
            .round()
            .min(Self::MAX_SWEEPS_PER_FRAME as f32) as u32
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            scene: Scene::Clock,
            mode: Mode::Split,
            split: 0.5,
            sweeps_per_s: 150_000.0,
            persistence_s: 1.0,
            infinite: false,
            intensity: 1.0,
            ghost: 0.03,
            peak: 8.0,
            cutoff: 0.05,
            hdr_palette: Palette::Phosphor,
            sdr_palette: Palette::Temperature,
            phosphor_color: [0.25, 1.0, 0.45],
            age_span_s: 1.0,
            bloom: 0.35,
            bloom_threshold: 1.0,
            bloom_radius: 6.0,
            graticule: 0.035,
            fit_to_sdr: true,
            signal: Signal::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitized_keeps_the_defaults_and_fixes_nonsense() {
        assert_eq!(Settings::default().sanitized(), Settings::default());
        let bad = Settings {
            split: f32::NAN,
            peak: 0.0,
            bloom_radius: 1000.0,
            ..Settings::default()
        }
        .sanitized();
        assert_eq!(bad.split, 0.5);
        assert_eq!(bad.peak, 1.0);
        assert_eq!(bad.bloom_radius, Settings::MAX_BLOOM_RADIUS);
    }

    #[test]
    fn sweeps_follow_the_rate_and_stay_in_one_dispatch() {
        let s = Settings::default();
        assert_eq!(s.sweeps_in(1.0 / 60.0), 2500);
        let fast = Settings {
            sweeps_per_s: 1.0e8,
            ..s
        };
        assert_eq!(fast.sweeps_in(1.0), Settings::MAX_SWEEPS_PER_FRAME);
    }
}
