// Audio3DSP — Stereo Spatial Reverb (fundsp wrapper)
//
// Wraps fundsp's `reverb_stereo` into a pre-allocated, real-time-safe
// unit with wet/dry mixing.
//
// The fundsp graph is compiled and allocated at initialization time.
// During processing, only `tick()` is called — zero allocations.
//
// fundsp 0.20 AudioUnit::tick signature:
//   fn tick(&mut self, input: &[f32], output: &mut [f32]);

use fundsp::hacker::*;

/// Default reverb parameters.
const DEFAULT_ROOM_SIZE: f32 = 10.0; // meters
const DEFAULT_REVERB_TIME: f32 = 2.0; // seconds
const DEFAULT_DAMPING: f32 = 0.5;
const DEFAULT_WET_MIX: f32 = 0.25; // 25% wet

/// A pre-allocated stereo reverb processor wrapping fundsp's reverb_stereo.
///
/// The internal DSP graph is compiled once at construction. The `process`
/// method performs only arithmetic — no allocations or system calls.
pub struct StereoReverb {
    // The reverb unit processes stereo frames (2 in → 2 out).
    unit: Box<dyn AudioUnit>,
    wet_mix: f32,
    // Pre-allocated output buffer for tick() — avoids stack allocation per call
    output_buf: [f32; 2],
}

impl StereoReverb {
    /// Create a new stereo reverb with default parameters.
    pub fn new(sample_rate: f64) -> Self {
        Self::with_params(
            sample_rate,
            DEFAULT_ROOM_SIZE,
            DEFAULT_REVERB_TIME,
            DEFAULT_DAMPING,
            DEFAULT_WET_MIX,
        )
    }

    /// Create a new stereo reverb with custom parameters.
    ///
    /// # Arguments
    /// * `sample_rate` — Audio sample rate in Hz (e.g., 44100.0)
    /// * `room_size` — Simulated room size in meters
    /// * `reverb_time` — Reverb tail decay time in seconds
    /// * `damping` — High-frequency damping (0.0 = bright, 1.0 = dark)
    /// * `wet_mix` — Wet/dry mix ratio (0.0 = fully dry, 1.0 = fully wet)
    pub fn with_params(
        sample_rate: f64,
        room_size: f32,
        reverb_time: f32,
        damping: f32,
        wet_mix: f32,
    ) -> Self {
        // Build the fundsp reverb graph.
        // reverb_stereo takes (room_size, reverb_time, damping) and produces
        // a stereo-in, stereo-out AudioUnit.
        let graph = reverb_stereo(room_size, reverb_time, damping);
        let mut unit = Box::new(graph) as Box<dyn AudioUnit>;

        // Configure sample rate and pre-allocate all internal delay lines.
        unit.set_sample_rate(sample_rate);
        unit.allocate();

        Self {
            unit,
            wet_mix,
            output_buf: [0.0; 2],
        }
    }

    /// Dynamically update the wet/dry mix ratio (0.0 to 1.0).
    pub fn set_wet_mix(&mut self, wet_mix: f32) {
        self.wet_mix = wet_mix.clamp(0.0, 1.0);
    }

    /// Process a single stereo frame through the reverb.
    ///
    /// Returns `(left_out, right_out)` with wet/dry mixing applied.
    /// This method performs zero allocations.
    #[inline(always)]
    pub fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        // Tick the reverb unit with the input frame
        let input = [left, right];
        self.unit.tick(&input, &mut self.output_buf);

        // Mix wet and dry signals
        let dry_mix = 1.0 - self.wet_mix;
        let left_out = left * dry_mix + self.output_buf[0] * self.wet_mix;
        let right_out = right * dry_mix + self.output_buf[1] * self.wet_mix;

        (left_out, right_out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fully dry reverb should pass the signal through unchanged.
    #[test]
    fn fully_dry_is_passthrough() {
        let mut reverb = StereoReverb::with_params(44100.0, 10.0, 2.0, 0.5, 0.0);

        // Feed a few frames to let the reverb initialize
        for _ in 0..100 {
            reverb.process(0.0, 0.0);
        }

        // A dry-only reverb should return the input unchanged
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

    /// Silence in should produce (near) silence out.
    #[test]
    fn silence_produces_silence() {
        let mut reverb = StereoReverb::new(44100.0);

        // Process many silent frames
        let mut max_output = 0.0f32;
        for _ in 0..10000 {
            let (l, r) = reverb.process(0.0, 0.0);
            max_output = max_output.max(l.abs()).max(r.abs());
        }

        assert!(
            max_output < 1e-6,
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
