// Audio3DSP — Analog Console & Tape Saturation Emulator
//
// Emulates analog mixing desk summing & gentle tape compression.
// Uses a fast polynomial soft-clipper (Padé approximant of tanh)
// to avoid expensive transcendentals (`tanh` / `exp`).
//
// Lock-free, zero heap allocation, real-time audio safe.

/// Analog console saturation stage.
pub struct ConsoleSaturation {
    drive: f32,
    mix: f32,
    pub enabled: bool,
}

impl ConsoleSaturation {
    /// Create a new console saturation unit with subtle warm defaults.
    pub fn new() -> Self {
        Self {
            drive: 1.2, // Subtle warm drive
            mix: 0.3,   // 30% wet parallel blend
            enabled: true,
        }
    }

    /// Set saturation drive factor (1.0 = clean, >1.0 = warm harmonic saturation).
    #[inline(always)]
    pub fn set_drive(&mut self, drive: f32) {
        self.drive = drive.clamp(0.5, 4.0);
    }

    /// Read current drive factor.
    #[allow(dead_code)]
    #[inline(always)]
    pub fn get_drive(&self) -> f32 {
        self.drive
    }

    /// Set parallel wet/dry blend ratio (0.0 to 1.0).
    #[inline(always)]
    pub fn set_mix(&mut self, mix: f32) {
        self.mix = mix.clamp(0.0, 1.0);
    }

    /// Read current wet/dry blend ratio.
    #[allow(dead_code)]
    #[inline(always)]
    pub fn get_mix(&self) -> f32 {
        self.mix
    }

    /// Fast polynomial soft-clipping approximation (Padé approximant of tanh).
    ///
    /// `y = x * (27 + x^2) / (27 + 9*x^2)`
    #[inline(always)]
    fn soft_clip(x: f32) -> f32 {
        let x_clamped = x.clamp(-3.0, 3.0);
        let x2 = x_clamped * x_clamped;
        (x_clamped * (27.0 + x2) / (27.0 + 9.0 * x2)).clamp(-1.0, 1.0)
    }

    /// Process a stereo frame through parallel analog saturation.
    #[inline(always)]
    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        if !self.enabled || self.mix == 0.0 {
            return (l, r);
        }

        let driven_l = l * self.drive;
        let driven_r = r * self.drive;

        let sat_l = Self::soft_clip(driven_l);
        let sat_r = Self::soft_clip(driven_r);

        // Parallel wet/dry mix
        let out_l = l * (1.0 - self.mix) + sat_l * self.mix;
        let out_r = r * (1.0 - self.mix) + sat_r * self.mix;

        (out_l, out_r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saturation_silence() {
        let mut sat = ConsoleSaturation::new();
        let (l, r) = sat.process(0.0, 0.0);
        assert_eq!(l, 0.0);
        assert_eq!(r, 0.0);
    }

    #[test]
    fn saturation_bypass() {
        let mut sat = ConsoleSaturation::new();
        sat.enabled = false;
        let (l, r) = sat.process(0.8, -0.8);
        assert_eq!(l, 0.8);
        assert_eq!(r, -0.8);
    }

    #[test]
    fn saturation_soft_clips_high_amplitude() {
        let mut sat = ConsoleSaturation::new();
        sat.set_drive(3.0);
        sat.set_mix(1.0);
        let (l, r) = sat.process(2.0, -2.0);
        assert!(!l.is_nan() && !r.is_nan());
        assert!(l.abs() <= 1.0);
        assert!(r.abs() <= 1.0);
    }
}