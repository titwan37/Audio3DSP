// Audio3DSP — DSP Chain Orchestrator
//
// Composes all DSP effects into a single processing pipeline:
//   1. M/S Widener  — stereo width expansion
//   2. Haas Delay   — psychoacoustic width via micro-delay on one channel
//   3. Stereo Reverb — spatial depth and room simulation
//
// The chain is designed to be called once per stereo frame inside the
// audio output callback. All internal state is pre-allocated.

use super::haas::HaasDelay;
use super::reverb::StereoReverb;
use super::widener::MsWidener;

/// Default DSP parameters.
const DEFAULT_SIDE_GAIN: f32 = 2.0;  // stereo width gain
const DEFAULT_HAAS_DELAY_MS: f32 = 10.0; // 10.0ms Haas delay
const DEFAULT_OUTPUT_GAIN_DB: f32 = 3.0; // +3dB gain to compensate for perceived volume loss

/// The complete DSP processing chain.
///
/// Owns all effect instances and processes stereo audio frame-by-frame.
/// All memory is pre-allocated at construction time.
pub struct DspChain {
    widener: MsWidener,
    haas: HaasDelay,
    reverb: StereoReverb,
    output_gain_linear: f32,
    sample_rate: u32,
}

impl DspChain {
    /// Create a new DSP chain with default parameters.
    ///
    /// # Arguments
    /// * `sample_rate` — Audio sample rate in Hz (e.g., 44100)
    pub fn new(sample_rate: u32) -> Self {
        Self {
            widener: MsWidener::new(DEFAULT_SIDE_GAIN),
            haas: HaasDelay::from_ms(DEFAULT_HAAS_DELAY_MS, sample_rate),
            reverb: StereoReverb::new(sample_rate as f64),
            output_gain_linear: 10_f32.powf(DEFAULT_OUTPUT_GAIN_DB / 20.0),
            sample_rate,
        }
    }

    /// Update stereo width gain (1.0x to 3.0x).
    pub fn set_side_gain(&mut self, gain: f32) {
        self.widener.set_side_gain(gain);
    }

    /// Update Haas delay time in milliseconds (0ms to 40ms).
    pub fn set_haas_delay_ms(&mut self, delay_ms: f32) {
        self.haas.set_delay_ms(delay_ms, self.sample_rate);
    }

    /// Update reverb wet mix ratio (0.0 to 1.0).
    pub fn set_reverb_wet(&mut self, wet_mix: f32) {
        self.reverb.set_wet_mix(wet_mix);
    }

    /// Process a single stereo frame through the entire DSP chain.
    ///
    /// # Arguments
    /// * `left` — Left channel input sample (f32)
    /// * `right` — Right channel input sample (f32)
    ///
    /// # Returns
    /// `(left_out, right_out)` — Processed stereo frame.
    ///
    /// This method performs zero allocations and is safe to call
    /// from a real-time audio callback.
    #[inline(always)]
    pub fn process_frame(&mut self, left: f32, right: f32) -> (f32, f32) {
        // Stage 1: M/S stereo widening
        let (l, r) = self.widener.process(left, right);

        // Stage 2: Haas delay on the RIGHT channel only
        // This creates a psychoacoustic perception that the sound source
        // is wider than the physical headphone drivers.
        let r_delayed = self.haas.process(r);

        // Stage 3: Stereo reverb (operates in f32)
        let (mut l_out, mut r_out) = self.reverb.process(l, r_delayed);

        // Stage 4: Output gain & clipping protection
        // Apply the +3dB gain, and clamp to the safe digital audio range [-1.0, 1.0]
        // This prevents the audio from exceeding the boundaries and creating harsh crackles.
        l_out = (l_out * self.output_gain_linear).clamp(-1.0, 1.0);
        r_out = (r_out * self.output_gain_linear).clamp(-1.0, 1.0);

        (l_out, r_out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Silence in should produce (near) silence out — no DC offset or NaN.
    #[test]
    fn silence_in_silence_out() {
        let mut chain = DspChain::new(44100);

        let mut max_output = 0.0f32;
        for _ in 0..10000 {
            let (l, r) = chain.process_frame(0.0, 0.0);
            assert!(!l.is_nan() && !l.is_infinite());
            assert!(!r.is_nan() && !r.is_infinite());
            max_output = max_output.max(l.abs()).max(r.abs());
        }

        assert!(
            max_output < 1e-5,
            "silence should produce near-silence, got max {}",
            max_output
        );
    }

    /// Processing should not produce NaN or Inf for normal audio input.
    #[test]
    fn no_nan_or_inf_with_signal() {
        let mut chain = DspChain::new(44100);

        for i in 0..44100 {
            // Simulate a sine wave
            let t = i as f32 / 44100.0;
            let sample = (t * 440.0 * std::f32::consts::TAU).sin() * 0.5;
            let (l, r) = chain.process_frame(sample, sample * 0.8);
            assert!(!l.is_nan() && !l.is_infinite(), "NaN/Inf at frame {}", i);
            assert!(!r.is_nan() && !r.is_infinite(), "NaN/Inf at frame {}", i);
        }
    }
}
