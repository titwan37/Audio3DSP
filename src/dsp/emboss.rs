// Audio3DSP — Vocal "Emboss" Presence & Saturation Processor
//
// Enhances lead vocals (Mid channel) through a two-stage signal pipeline:
//   1. Targeted Parametric Peak EQ Filter boosting the presence zone (2 kHz - 4 kHz)
//   2. Lightweight Soft-Clipping Saturation / Harmonic Exciter algorithm for texture & definition
//
// Zero heap allocations during processing — safe for real-time audio thread execution.

use std::f32::consts::TAU;

/// A Direct Form II Transposed Biquad Peaking EQ Filter.
///
/// Implements standard RBJ Audio EQ Cookbook equations for a peaking/bell filter.
pub struct PeakingEqFilter {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    // State variables for Transposed Direct Form II
    d1: f32,
    d2: f32,
}

impl PeakingEqFilter {
    /// Create a new peaking EQ filter.
    ///
    /// # Arguments
    /// * `sample_rate` — Audio sample rate in Hz (e.g., 44100 or 48000)
    /// * `freq_hz` — Center frequency in Hz (presence zone ~3000 Hz)
    /// * `gain_db` — Peak boost/cut in dB (+3.0 dB default)
    /// * `q` — Quality factor (Q = 1.0 for smooth presence peak)
    pub fn new(sample_rate: u32, freq_hz: f32, gain_db: f32, q: f32) -> Self {
        let mut filter = Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            d1: 0.0,
            d2: 0.0,
        };
        filter.update_coefficients(sample_rate, freq_hz, gain_db, q);
        filter
    }

    /// Recalculate filter coefficients for new parameters.
    pub fn update_coefficients(&mut self, sample_rate: u32, freq_hz: f32, gain_db: f32, q: f32) {
        // Clamp gain to reasonable bounds [0.0, 12.0] dB
        let gain_db = gain_db.clamp(0.0, 12.0);

        // RBJ Peaking EQ Coefficients
        let w0 = TAU * (freq_hz / sample_rate as f32);
        let alpha = w0.sin() / (2.0 * q.max(0.1));
        let a = 10.0_f32.powf(gain_db / 40.0); // sqrt of linear gain
        let cos_w0 = w0.cos();

        let b0_raw = 1.0 + alpha * a;
        let b1_raw = -2.0 * cos_w0;
        let b2_raw = 1.0 - alpha * a;
        let a0_raw = 1.0 + alpha / a;
        let a1_raw = -2.0 * cos_w0;
        let a2_raw = 1.0 - alpha / a;

        // Normalize by a0
        let inv_a0 = 1.0 / a0_raw;
        self.b0 = b0_raw * inv_a0;
        self.b1 = b1_raw * inv_a0;
        self.b2 = b2_raw * inv_a0;
        self.a1 = a1_raw * inv_a0;
        self.a2 = a2_raw * inv_a0;
    }

    /// Reset filter state variables to zero (clears memory).
    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.d1 = 0.0;
        self.d2 = 0.0;
    }

    /// Process a single sample through the peaking EQ filter.
    #[inline(always)]
    pub fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.d1;
        self.d1 = self.b1 * x - self.a1 * y + self.d2;
        self.d2 = self.b2 * x - self.a2 * y;
        y
    }
}

/// Vocal "Emboss" Pipeline Processor.
///
/// Combines a 3 kHz presence boost biquad filter with a lightweight
/// soft-clipping saturation / harmonic exciter.
pub struct VocalEmboss {
    eq: PeakingEqFilter,
    gain_db: f32,
    sample_rate: u32,
    saturation_drive: f32,
}

impl VocalEmboss {
    /// Create a new vocal emboss processor.
    ///
    /// # Arguments
    /// * `sample_rate` — Audio sample rate in Hz
    /// * `gain_db` — Presence boost gain in dB (default 3.0 dB)
    pub fn new(sample_rate: u32, gain_db: f32) -> Self {
        let eq = PeakingEqFilter::new(sample_rate, 3000.0, gain_db, 1.0);
        Self {
            eq,
            gain_db,
            sample_rate,
            saturation_drive: 1.15, // Subtle warm harmonic excitation multiplier
        }
    }

    /// Dynamically update presence boost gain in dB.
    pub fn set_gain_db(&mut self, gain_db: f32) {
        self.gain_db = gain_db.max(0.0);
        self.eq
            .update_coefficients(self.sample_rate, 3000.0, self.gain_db, 1.0);
    }

    /// Process a single Mid (vocal) channel sample.
    ///
    /// # Signal Flow:
    /// 1. Presence Peak EQ boost around 3 kHz
    /// 2. Soft-clipping polynomial saturation for rich harmonic texturing
    /// 3. Level-compensated output
    #[inline(always)]
    pub fn process(&mut self, mid_sample: f32) -> f32 {
        // Stage 1: Presence zone parametric EQ boost
        let eq_out = self.eq.process(mid_sample);

        // Stage 2: Soft-clipping harmonic exciter / saturator
        // Cubic soft clipper: y = x - (x^3 / 3) for |x| <= 1.0
        let driven = eq_out * self.saturation_drive;
        let saturated = if driven > 1.0 {
            1.0
        } else if driven < -1.0 {
            -1.0
        } else {
            driven - (driven * driven * driven) * (1.0 / 3.0)
        };

        // Level compensation: scale back by drive factor
        saturated * (1.0 / self.saturation_drive)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eq_zero_gain_is_passthrough() {
        let mut emboss = VocalEmboss::new(44100, 0.0);
        let sample = 0.01f32;

        // Warm up filter
        for _ in 0..100 {
            emboss.process(0.0);
        }

        let out = emboss.process(sample);
        assert!(
            (out - sample).abs() < 1e-4,
            "0dB gain at low amplitude should produce identity output, got {}",
            out
        );
    }

    #[test]
    fn presence_boost_increases_3khz_gain() {
        let mut emboss_flat = VocalEmboss::new(44100, 0.0);
        let mut emboss_boost = VocalEmboss::new(44100, 4.0);

        let dt = 1.0 / 44100.0;
        let freq = 3000.0; // 3 kHz tone inside presence band

        let mut max_flat = 0.0f32;
        let mut max_boost = 0.0f32;

        for i in 0..1000 {
            let t = i as f32 * dt;
            let val = (TAU * freq * t).sin() * 0.2;

            let out_flat = emboss_flat.process(val);
            let out_boost = emboss_boost.process(val);

            if i > 500 {
                max_flat = max_flat.max(out_flat.abs());
                max_boost = max_boost.max(out_boost.abs());
            }
        }

        assert!(
            max_boost > max_flat * 1.1,
            "3kHz presence boost should increase amplitude: boost={}, flat={}",
            max_boost,
            max_flat
        );
    }

    #[test]
    fn saturation_clamps_large_signals() {
        let mut emboss = VocalEmboss::new(44100, 3.0);

        let large_in = 2.5f32;
        let out = emboss.process(large_in);
        assert!(
            !out.is_nan() && !out.is_infinite(),
            "output must be finite"
        );
        assert!(
            out <= 1.0,
            "saturator must clamp output to <= 1.0, got {}",
            out
        );
    }
}
