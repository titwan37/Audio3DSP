// Audio3DSP — Parallel Matrix DSP Pipeline Orchestrator
//
// Implements an advanced multi-band / matrix processing routing architecture:
//   1. Mid/Side Matrix Split: Decodes stereo input (L, R) into Mid (vocals/center)
//      and Side (instruments/spatial ambience) channels.
//   2. Vocal "Emboss" Pipeline (Mid Channel): Applies a 3 kHz parametric presence EQ
//      boost and soft-clipping saturation to give lead vocals definition & texture.
//   3. Instrumental 3D Widening (Side Channel): Applies Side gain scaling and
//      Haas micro-delay exclusively to off-center instrumental signals, pushing
//      them to the extreme edges of the soundstage.
//   4. Stereo Re-Matrix: Recombines embossed Mid and widened Side back into L/R.
//   5. Stereo Spatial Reverb & Peak Protection: Routes matrixed stereo through
//      the reverb room simulator, applying +3dB output compensation and clamping.
//
// Pre-allocated, lock-free, zero heap allocation real-time audio loop execution.

use super::emboss::VocalEmboss;
use super::haas::HaasDelay;
use super::reverb::StereoReverb;

/// Default DSP parameters.
const DEFAULT_SIDE_GAIN: f32 = 2.0; // stereo width gain (Side channel multiplier)
const DEFAULT_HAAS_DELAY_MS: f32 = 10.0; // 10.0ms Haas delay on Side channel
const DEFAULT_EMBOSS_GAIN_DB: f32 = 3.0; // +3.0dB vocal presence boost
const DEFAULT_OUTPUT_GAIN_DB: f32 = 3.0; // +3dB gain compensation

/// The complete parallel matrix DSP processing chain.
pub struct DspChain {
    side_gain: f32,
    haas: HaasDelay,
    emboss: VocalEmboss,
    reverb: StereoReverb,
    output_gain_linear: f32,
    sample_rate: u32,
}

impl DspChain {
    /// Create a new multi-band matrix DSP chain with default parameters.
    ///
    /// # Arguments
    /// * `sample_rate` — Audio sample rate in Hz (e.g., 44100 or 48000)
    pub fn new(sample_rate: u32) -> Self {
        Self {
            side_gain: DEFAULT_SIDE_GAIN,
            haas: HaasDelay::from_ms(DEFAULT_HAAS_DELAY_MS, sample_rate),
            emboss: VocalEmboss::new(sample_rate, DEFAULT_EMBOSS_GAIN_DB),
            reverb: StereoReverb::new(sample_rate as f64),
            output_gain_linear: 10_f32.powf(DEFAULT_OUTPUT_GAIN_DB / 20.0),
            sample_rate,
        }
    }

    /// Update Side channel gain multiplier (1.0x to 3.0x).
    pub fn set_side_gain(&mut self, gain: f32) {
        self.side_gain = gain.max(0.0);
    }

    /// Update Haas delay time in milliseconds (0ms to 40ms).
    pub fn set_haas_delay_ms(&mut self, delay_ms: f32) {
        self.haas.set_delay_ms(delay_ms, self.sample_rate);
    }

    /// Update Vocal Emboss presence boost in dB (0.0dB to 6.0dB).
    pub fn set_emboss_gain_db(&mut self, gain_db: f32) {
        self.emboss.set_gain_db(gain_db);
    }

    /// Update reverb wet mix ratio (0.0 to 1.0).
    pub fn set_reverb_wet(&mut self, wet_mix: f32) {
        self.reverb.set_wet_mix(wet_mix);
    }

    /// Process a single stereo frame through the parallel matrix routing pipeline.
    ///
    /// # Signal Flow:
    /// 1. `(Mid, Side)` Decode from input `(Left, Right)`
    /// 2. Mid branch: `VocalEmboss` (3 kHz presence EQ boost + soft saturation)
    /// 3. Side branch: `Side * side_gain` -> `HaasDelay` (3D spatial widening)
    /// 4. Re-Matrix: `L_matrix = Mid' + Side'`, `R_matrix = Mid' - Side'`
    /// 5. Stereo Reverb space wet/dry mix
    /// 6. Output gain scaling & clipping protection
    #[inline(always)]
    pub fn process_frame(&mut self, left: f32, right: f32) -> (f32, f32) {
        // Stage 1: Mid/Side Matrix Split
        let mid = (left + right) * 0.5;
        let side = (left - right) * 0.5;

        // Stage 2: Vocal "Emboss" Pipeline on Mid (center vocal) Channel
        let mid_embossed = self.emboss.process(mid);

        // Stage 3: Instrumental 3D Widening on Side Channel
        let side_wide = side * self.side_gain;
        let side_spatial = self.haas.process(side_wide);

        // Stage 4: Stereo Re-Matrix
        let l_matrix = mid_embossed + side_spatial;
        let r_matrix = mid_embossed - side_spatial;

        // Stage 5: Stereo Reverb Space
        let (mut l_out, mut r_out) = self.reverb.process(l_matrix, r_matrix);

        // Stage 6: Output Gain Compensation & Digital Peak Clipping Protection
        l_out = (l_out * self.output_gain_linear).clamp(-1.0, 1.0);
        r_out = (r_out * self.output_gain_linear).clamp(-1.0, 1.0);

        (l_out, r_out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Silence input should produce near-silence without NaN or Inf.
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
            max_output < 1e-4,
            "silence should produce near-silence, got max {}",
            max_output
        );
    }

    /// Pure center (mono vocal) signal should be processed by Emboss pipeline.
    #[test]
    fn mono_vocal_embossed() {
        let mut chain = DspChain::new(44100);
        chain.set_emboss_gain_db(4.0);

        let sample = 0.4f32;
        for _ in 0..500 {
            chain.process_frame(sample, sample);
        }

        let (l, r) = chain.process_frame(sample, sample);
        assert!(!l.is_nan() && !r.is_nan());
        // In mono (L == R), Side = 0, so L and R output should be identical before reverb
        assert!(
            (l - r).abs() < 0.1,
            "mono vocal should remain symmetric in L/R, got L={}, R={}",
            l,
            r
        );
    }

    /// Stereo signal processing should run continuously without error or clipping.
    #[test]
    fn full_matrix_pipeline_audio_run() {
        let mut chain = DspChain::new(48000);

        for i in 0..48000 {
            let t = i as f32 / 48000.0;
            let left_in = (t * 440.0 * std::f32::consts::TAU).sin() * 0.4;
            let right_in = (t * 880.0 * std::f32::consts::TAU).sin() * 0.3;

            let (l, r) = chain.process_frame(left_in, right_in);
            assert!(!l.is_nan() && !l.is_infinite(), "Left NaN/Inf at sample {}", i);
            assert!(!r.is_nan() && !r.is_infinite(), "Right NaN/Inf at sample {}", i);
            assert!(l.abs() <= 1.0, "Left clipping at sample {}", i);
            assert!(r.abs() <= 1.0, "Right clipping at sample {}", i);
        }
    }
}
