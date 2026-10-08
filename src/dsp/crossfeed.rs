// Audio3DSP — Headphone Crossfeed & Natural Acoustic Speaker Simulation
//
// Simulates Head-Related Transfer Function (HRTF) acoustic crosstalk
// to move the soundstage "out of the head" into a physical concert room,
// eliminating headphone super-stereo fatigue.
//
// Lock-free, zero heap allocation, real-time audio safe.

const MAX_CROSSFEED_DELAY: usize = 128; // ~2.6ms at 48kHz (plenty for ITD)

/// Interaural Time & Intensity Difference (ITD/IID) crossfeed processor.
pub struct HeadphoneCrossfeed {
    // 1-pole low-pass filter state (simulates head shadowing / high-frequency attenuation)
    lp_state_l: f32,
    lp_state_r: f32,
    lp_coeff: f32,
    // Circular delay buffers for Interaural Time Difference (ITD)
    delay_buf_l: [f32; MAX_CROSSFEED_DELAY],
    delay_buf_r: [f32; MAX_CROSSFEED_DELAY],
    write_pos: usize,
    delay_samples: usize,
    // Crossfeed amount (0.0 = pure headphone, 1.0 = full speaker simulation)
    mix: f32,
    pub enabled: bool,
}

impl HeadphoneCrossfeed {
    /// Create a new crossfeed processor calibrated for the given audio sample rate.
    pub fn new(sample_rate: u32) -> Self {
        // ~0.6ms delay (typical human head diameter interaural time difference)
        let delay_samples = (0.0006 * sample_rate as f32).round() as usize;
        // Low-pass filter at ~700Hz (simulates acoustic head shadowing)
        let rc = 1.0 / (2.0 * std::f32::consts::PI * 700.0);
        let dt = 1.0 / sample_rate as f32;
        let lp_coeff = dt / (rc + dt);

        Self {
            lp_state_l: 0.0,
            lp_state_r: 0.0,
            lp_coeff,
            delay_buf_l: [0.0; MAX_CROSSFEED_DELAY],
            delay_buf_r: [0.0; MAX_CROSSFEED_DELAY],
            write_pos: 0,
            delay_samples: delay_samples.min(MAX_CROSSFEED_DELAY - 1),
            mix: 0.4, // Default to 40% crossfeed (natural room feel)
            enabled: true,
        }
    }

    /// Update crossfeed blend ratio (0.0 = off, 1.0 = max speaker illusion).
    #[inline(always)]
    pub fn set_mix(&mut self, mix: f32) {
        self.mix = mix.clamp(0.0, 1.0);
    }

    /// Read current crossfeed mix ratio.
    #[allow(dead_code)]
    #[inline(always)]
    pub fn get_mix(&self) -> f32 {
        self.mix
    }

    /// Process a stereo frame through the acoustic crossfeed matrix.
    #[inline(always)]
    pub fn process(&mut self, mut l: f32, mut r: f32) -> (f32, f32) {
        if !self.enabled || self.mix == 0.0 {
            return (l, r);
        }

        // 1. Read delayed signals (opposite ear paths)
        let read_pos = (self.write_pos + MAX_CROSSFEED_DELAY - self.delay_samples) % MAX_CROSSFEED_DELAY;
        let delayed_l_raw = self.delay_buf_l[read_pos];
        let delayed_r_raw = self.delay_buf_r[read_pos];

        // 2. Apply 1-pole Low-Pass Filter (Head Shadowing)
        self.lp_state_l += self.lp_coeff * (delayed_l_raw - self.lp_state_l);
        self.lp_state_r += self.lp_coeff * (delayed_r_raw - self.lp_state_r);
        let delayed_l = self.lp_state_l;
        let delayed_r = self.lp_state_r;

        // 3. Write current samples to delay buffer
        self.delay_buf_l[self.write_pos] = l;
        self.delay_buf_r[self.write_pos] = r;
        self.write_pos = (self.write_pos + 1) % MAX_CROSSFEED_DELAY;

        // 4. Mix opposite crossfeed signal into each channel
        l += delayed_r * self.mix;
        r += delayed_l * self.mix;

        (l, r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crossfeed_silence() {
        let mut cf = HeadphoneCrossfeed::new(48000);
        let (l, r) = cf.process(0.0, 0.0);
        assert!(!l.is_nan() && !r.is_nan());
        assert_eq!(l, 0.0);
        assert_eq!(r, 0.0);
    }

    #[test]
    fn crossfeed_bypass() {
        let mut cf = HeadphoneCrossfeed::new(48000);
        cf.enabled = false;
        let (l, r) = cf.process(0.5, -0.3);
        assert_eq!(l, 0.5);
        assert_eq!(r, -0.3);
    }

    #[test]
    fn crossfeed_crosstalk_occurs() {
        let mut cf = HeadphoneCrossfeed::new(48000);
        cf.set_mix(0.5);
        // Feed only Left channel for several samples
        let mut right_had_sound = false;
        for _ in 0..100 {
            let (_, r) = cf.process(1.0, 0.0);
            if r.abs() > 0.01 {
                right_had_sound = true;
            }
        }
        assert!(right_had_sound, "crossfeed should bleed filtered Left into Right channel");
    }
}