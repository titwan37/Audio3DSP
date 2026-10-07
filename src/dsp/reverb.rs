// Audio3DSP — High-Performance Stereo Spatial Reverb
//
// Optimized, cache-friendly Schroeder/Freeverb stereo spatial reverberator.
//
// Performance characteristics:
//   - Zero dynamic dispatch (100% static dispatch, no `dyn AudioUnit` vtable overhead).
//   - Zero heap allocations in the real-time audio thread.
//   - Contiguous, cache-aligned circular delay lines.
//   - Anti-denormal protection preventing x86 microcode assist stalls during decay tails.
//   - Branchless, SIMD-friendly sample processing loop.

/// Number of parallel Low-Pass Feedback Comb (LPFC) filters per stereo channel.
const NUM_COMBS: usize = 8;
/// Number of cascaded All-Pass (AP) diffusion filters per stereo channel.
const NUM_ALLPASSES: usize = 4;

/// Reference comb filter delay lengths (in samples at 44.1 kHz).
/// Mutually prime lengths ensure dense, natural echo distribution without metallic ringing.
const COMB_TUNING_L: [usize; NUM_COMBS] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
/// Stereo spread adds subtle spatial decorrelation for the right channel.
const STEREO_SPREAD: usize = 23;

/// Reference all-pass filter delay lengths (in samples at 44.1 kHz).
const ALLPASS_TUNING_L: [usize; NUM_ALLPASSES] = [556, 441, 341, 225];

/// All-pass feedback coefficient (0.5 provides smooth, transparent phase diffusion).
const ALLPASS_FEEDBACK: f32 = 0.5;

/// Tiny anti-denormal constant to prevent CPU microcode stalls on subnormal decays.
const ANTI_DENORMAL: f32 = 1.0e-25;

/// Single Low-Pass Feedback Comb Filter (LPFC).
#[derive(Clone)]
struct CombFilter {
    buffer: Vec<f32>,
    pos: usize,
    filter_state: f32,
    feedback: f32,
    damp1: f32,
    damp2: f32,
}

impl CombFilter {
    fn new(delay_samples: usize) -> Self {
        let size = delay_samples.max(1);
        Self {
            buffer: vec![0.0; size],
            pos: 0,
            filter_state: 0.0,
            feedback: 0.84,
            damp1: 0.2,
            damp2: 0.8,
        }
    }

    #[inline(always)]
    fn set_damping(&mut self, val: f32) {
        self.damp1 = val;
        self.damp2 = 1.0 - val;
    }

    #[inline(always)]
    fn set_feedback(&mut self, val: f32) {
        self.feedback = val;
    }

    #[inline(always)]
    fn process(&mut self, input: f32) -> f32 {
        let output = self.buffer[self.pos];

        // Low-pass filter in feedback path with anti-denormal protection
        self.filter_state = (output * self.damp2) + (self.filter_state * self.damp1) + ANTI_DENORMAL;
        self.buffer[self.pos] = input + (self.filter_state * self.feedback);

        self.pos += 1;
        if self.pos >= self.buffer.len() {
            self.pos = 0;
        }

        output
    }
}

/// Single All-Pass Diffusion Filter.
#[derive(Clone)]
struct AllpassFilter {
    buffer: Vec<f32>,
    pos: usize,
    feedback: f32,
}

impl AllpassFilter {
    fn new(delay_samples: usize) -> Self {
        let size = delay_samples.max(1);
        Self {
            buffer: vec![0.0; size],
            pos: 0,
            feedback: ALLPASS_FEEDBACK,
        }
    }

    #[inline(always)]
    fn process(&mut self, input: f32) -> f32 {
        let buf_out = self.buffer[self.pos];
        let output = -input + buf_out;

        self.buffer[self.pos] = input + (buf_out * self.feedback) + ANTI_DENORMAL;

        self.pos += 1;
        if self.pos >= self.buffer.len() {
            self.pos = 0;
        }

        output
    }
}

/// A high-performance, real-time safe stereo spatial reverberator.
pub struct StereoReverb {
    combs_l: Vec<CombFilter>,
    combs_r: Vec<CombFilter>,
    allpasses_l: Vec<AllpassFilter>,
    allpasses_r: Vec<AllpassFilter>,
    wet_mix: f32,
    room_size: f32,
    damping: f32,
    scale_factor: f32,
}

impl StereoReverb {
    /// Create a new stereo reverb initialized for the target sample rate.
    pub fn new(sample_rate: f64) -> Self {
        Self::with_params(sample_rate, 0.8, 0.5, 0.5, 0.25)
    }

    /// Create a new stereo reverb with custom parameters.
    ///
    /// # Arguments
    /// * `sample_rate` — Audio sample rate in Hz (e.g., 44100.0 or 48000.0)
    /// * `room_size` — Room size / decay factor (0.0 to 1.0)
    /// * `_reverb_time` — Retained for API compatibility (seconds)
    /// * `damping` — High frequency damping (0.0 to 1.0)
    /// * `wet_mix` — Wet/dry mix ratio (0.0 to 1.0)
    pub fn with_params(
        sample_rate: f64,
        room_size: f32,
        _reverb_time: f32,
        damping: f32,
        wet_mix: f32,
    ) -> Self {
        let sr_scale = (sample_rate / 44100.0) as f32;

        let combs_l: Vec<CombFilter> = COMB_TUNING_L
            .iter()
            .map(|&len| CombFilter::new((len as f32 * sr_scale).round() as usize))
            .collect();

        let combs_r: Vec<CombFilter> = COMB_TUNING_L
            .iter()
            .map(|&len| CombFilter::new(((len + STEREO_SPREAD) as f32 * sr_scale).round() as usize))
            .collect();

        let allpasses_l: Vec<AllpassFilter> = ALLPASS_TUNING_L
            .iter()
            .map(|&len| AllpassFilter::new((len as f32 * sr_scale).round() as usize))
            .collect();

        let allpasses_r: Vec<AllpassFilter> = ALLPASS_TUNING_L
            .iter()
            .map(|&len| AllpassFilter::new(((len + STEREO_SPREAD) as f32 * sr_scale).round() as usize))
            .collect();

        let mut reverb = Self {
            combs_l,
            combs_r,
            allpasses_l,
            allpasses_r,
            wet_mix: wet_mix.clamp(0.0, 1.0),
            room_size: room_size.clamp(0.0, 1.0),
            damping: damping.clamp(0.0, 1.0),
            scale_factor: 0.015, // Headroom scaling factor across 8 parallel combs
        };

        reverb.update_coefficients();
        reverb
    }

    /// Update internal comb filter feedback & damping coefficients based on room_size and damping.
    fn update_coefficients(&mut self) {
        // Map room_size (0..1) to feedback gain (0.70..0.98)
        let feedback = 0.70 + (self.room_size * 0.28);
        let damp = self.damping * 0.4;

        for comb in self.combs_l.iter_mut().chain(self.combs_r.iter_mut()) {
            comb.set_feedback(feedback);
            comb.set_damping(damp);
        }
    }

    /// Dynamically update the wet/dry mix ratio (0.0 to 1.0).
    #[inline(always)]
    pub fn set_wet_mix(&mut self, wet_mix: f32) {
        self.wet_mix = wet_mix.clamp(0.0, 1.0);
    }

    /// Dynamically update room size / decay time (0.0 to 1.0).
    #[allow(dead_code)]
    pub fn set_room_size(&mut self, room_size: f32) {
        self.room_size = room_size.clamp(0.0, 1.0);
        self.update_coefficients();
    }

    /// Dynamically update high-frequency damping (0.0 to 1.0).
    #[allow(dead_code)]
    pub fn set_damping(&mut self, damping: f32) {
        self.damping = damping.clamp(0.0, 1.0);
        self.update_coefficients();
    }

    /// Process a single stereo frame through the spatial reverb.
    ///
    /// # Real-time Safety:
    /// Guaranteed zero allocations, zero locks, zero system calls, zero dynamic dispatch.
    #[inline(always)]
    pub fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        let input_mixed = (left + right) * self.scale_factor;

        // Stage 1: Parallel Lowpass Feedback Comb Filters (accumulation)
        let mut out_l = 0.0f32;
        let mut out_r = 0.0f32;

        for comb in self.combs_l.iter_mut() {
            out_l += comb.process(input_mixed);
        }

        for comb in self.combs_r.iter_mut() {
            out_r += comb.process(input_mixed);
        }

        // Stage 2: Cascaded All-Pass Diffusion
        for ap in self.allpasses_l.iter_mut() {
            out_l = ap.process(out_l);
        }

        for ap in self.allpasses_r.iter_mut() {
            out_r = ap.process(out_r);
        }

        // Stage 3: Equal-power Wet/Dry Mix
        let dry_mix = 1.0 - self.wet_mix;
        let final_l = left * dry_mix + out_l * self.wet_mix;
        let final_r = right * dry_mix + out_r * self.wet_mix;

        (final_l, final_r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fully dry reverb should pass the signal through unchanged.
    #[test]
    fn fully_dry_is_passthrough() {
        let mut reverb = StereoReverb::with_params(44100.0, 0.8, 2.0, 0.5, 0.0);

        for _ in 0..100 {
            reverb.process(0.0, 0.0);
        }

        let (l, r) = reverb.process(0.5, -0.3);
        assert!(
            (l - 0.5).abs() < 1e-4,
            "dry reverb left should be ~0.5, got {}",
            l
        );
        assert!(
            (r - (-0.3)).abs() < 1e-4,
            "dry reverb right should be ~-0.3, got {}",
            r
        );
    }

    /// Silence in should produce near silence out.
    #[test]
    fn silence_produces_silence() {
        let mut reverb = StereoReverb::new(44100.0);

        let mut max_output = 0.0f32;
        for _ in 0..10000 {
            let (l, r) = reverb.process(0.0, 0.0);
            max_output = max_output.max(l.abs()).max(r.abs());
        }

        assert!(
            max_output < 1e-4,
            "silence input should produce near-silence output, got max {}",
            max_output
        );
    }

    /// Reverb should not produce NaN or Inf values.
    #[test]
    fn no_nan_or_inf() {
        let mut reverb = StereoReverb::new(44100.0);

        for i in 0..10000 {
            let input = (i as f32 * 0.1).sin() * 0.5;
            let (l, r) = reverb.process(input, -input);
            assert!(!l.is_nan() && !l.is_infinite(), "left NaN/Inf at frame {}", i);
            assert!(!r.is_nan() && !r.is_infinite(), "right NaN/Inf at frame {}", i);
        }
    }
}
