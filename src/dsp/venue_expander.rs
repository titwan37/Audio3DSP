// Audio3DSP — Dynamic Venue Expander (Crowd & Room Bloom)
//
// Tracks Mid-channel vocal energy with an attack/release peak follower
// to dynamically swell the Side channel (reverb, crowd, room reflections),
// creating an organic, breathing live concert soundstage.
//
// Lock-free, zero heap allocation, real-time audio safe.

/// Dynamic Mid/Side venue room & crowd expander.
pub struct VenueExpander {
    mid_env: f32,
    attack_coeff: f32,
    release_coeff: f32,
    sensitivity: f32, // How much the sides react to the mid energy
    pub enabled: bool,
}

impl VenueExpander {
    /// Create a new venue expander calibrated for the audio sample rate.
    pub fn new(sample_rate: u32) -> Self {
        Self {
            mid_env: 0.0,
            attack_coeff: 1.0 - (-1.0 / (0.005 * sample_rate as f32)).exp(), // 5ms attack
            release_coeff: 1.0 - (-1.0 / (0.150 * sample_rate as f32)).exp(), // 150ms release
            sensitivity: 0.5,
            enabled: true,
        }
    }

    /// Set dynamic expansion sensitivity (0.0 = static width, 2.0 = high dynamic swell).
    #[inline(always)]
    pub fn set_sensitivity(&mut self, val: f32) {
        self.sensitivity = val.clamp(0.0, 2.0);
    }

    /// Read current sensitivity.
    #[allow(dead_code)]
    #[inline(always)]
    pub fn get_sensitivity(&self) -> f32 {
        self.sensitivity
    }

    /// Process a stereo frame through the dynamic Mid/Side room expander.
    #[inline(always)]
    pub fn process(&mut self, mut l: f32, mut r: f32) -> (f32, f32) {
        if !self.enabled || self.sensitivity == 0.0 {
            return (l, r);
        }

        // 1. Decode Mid/Side
        let mid = (l + r) * 0.5;
        let mut side = (l - r) * 0.5;

        // 2. Track Mid-channel envelope (fast attack / smooth release peak follower)
        let mid_abs = mid.abs();
        let coeff = if mid_abs > self.mid_env {
            self.attack_coeff
        } else {
            self.release_coeff
        };
        self.mid_env += coeff * (mid_abs - self.mid_env);

        // 3. Calculate dynamic side gain multiplier
        let side_gain = 1.0 + (self.mid_env * self.sensitivity);

        // 4. Apply expansion to Side and re-matrix back to stereo L/R
        side *= side_gain;
        l = mid + side;
        r = mid - side;

        (l, r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn venue_expander_silence() {
        let mut ve = VenueExpander::new(48000);
        let (l, r) = ve.process(0.0, 0.0);
        assert_eq!(l, 0.0);
        assert_eq!(r, 0.0);
    }

    #[test]
    fn venue_expander_bypass() {
        let mut ve = VenueExpander::new(48000);
        ve.enabled = false;
        let (l, r) = ve.process(0.6, -0.4);
        assert_eq!(l, 0.6);
        assert_eq!(r, -0.4);
    }

    #[test]
    fn venue_expander_blooms_side_on_loud_mid() {
        let mut ve = VenueExpander::new(48000);
        ve.set_sensitivity(1.0);

        // Prime the envelope with high mid energy
        for _ in 0..1000 {
            ve.process(0.8, 0.8);
        }

        // Now process an asymmetric signal (has Side)
        let (l, r) = ve.process(0.8, 0.2);
        // Original side was (0.8 - 0.2) / 2 = 0.3
        // Since mid_env > 0, side is expanded, so L increases and R decreases relative to unexpanded
        let orig_mid = (0.8 + 0.2) * 0.5;
        assert!(l > orig_mid + 0.3, "Side should be expanded wider");
        assert!(r < orig_mid - 0.3, "Side should be expanded wider");
    }
}