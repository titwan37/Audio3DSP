// Audio3DSP — Haas Effect / Interaural Time Difference Delay
//
// Applies a micro-delay (0.3ms–0.7ms) to one audio channel to create
// a psychoacoustic widening effect (Haas / Precedence Effect).
//
// The delay buffer is stack-allocated as a fixed-size array to guarantee
// zero heap allocations in the audio callback.
//
// At 48000 Hz, 0.7ms ≈ 34 samples. MAX_DELAY_SAMPLES = 64 provides
// generous headroom for any supported sample rate.

/// Maximum delay in samples. Covers up to ~42ms at 48kHz (48000 * 0.040 = 1920 samples).
const MAX_DELAY_SAMPLES: usize = 2048;

/// A fixed-size circular delay line for the Haas effect.
///
/// Delays a single audio channel by a configurable number of samples.
/// All memory is pre-allocated — no heap allocations occur during processing.
pub struct HaasDelay {
    buffer: [f32; MAX_DELAY_SAMPLES],
    write_pos: usize,
    delay_samples: usize,
}

impl HaasDelay {
    /// Create a new Haas delay line.
    ///
    /// # Arguments
    /// * `delay_samples` — Number of samples to delay (0 to `MAX_DELAY_SAMPLES - 1`).
    ///
    /// # Panics
    /// Panics if `delay_samples >= MAX_DELAY_SAMPLES`.
    pub fn new(delay_samples: usize) -> Self {
        assert!(
            delay_samples < MAX_DELAY_SAMPLES,
            "delay_samples ({}) must be < MAX_DELAY_SAMPLES ({})",
            delay_samples,
            MAX_DELAY_SAMPLES
        );
        Self {
            buffer: [0.0; MAX_DELAY_SAMPLES],
            write_pos: 0,
            delay_samples,
        }
    }

    /// Create a Haas delay from a time in milliseconds and sample rate.
    pub fn from_ms(delay_ms: f32, sample_rate: u32) -> Self {
        let samples = (delay_ms * 0.001 * sample_rate as f32).round() as usize;
        Self::new(samples.min(MAX_DELAY_SAMPLES - 1))
    }

    /// Dynamically update the delay time in milliseconds.
    pub fn set_delay_ms(&mut self, delay_ms: f32, sample_rate: u32) {
        let samples = (delay_ms.max(0.0) * 0.001 * sample_rate as f32).round() as usize;
        self.set_delay_samples(samples);
    }

    /// Dynamically update the delay time in samples.
    pub fn set_delay_samples(&mut self, delay_samples: usize) {
        self.delay_samples = delay_samples.min(MAX_DELAY_SAMPLES - 1);
    }

    /// Process a single sample through the delay line.
    ///
    /// Returns the delayed sample. The current sample is written into the
    /// buffer and will emerge `delay_samples` ticks later.
    #[inline(always)]
    pub fn process(&mut self, sample: f32) -> f32 {
        // Zero delay is a true pass-through (no buffering)
        if self.delay_samples == 0 {
            return sample;
        }

        // Read from the delayed position
        let read_pos = (self.write_pos + MAX_DELAY_SAMPLES - self.delay_samples) % MAX_DELAY_SAMPLES;
        let delayed = self.buffer[read_pos];

        // Write current sample
        self.buffer[self.write_pos] = sample;
        self.write_pos = (self.write_pos + 1) % MAX_DELAY_SAMPLES;

        delayed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Zero delay should act as true identity (pass-through).
    #[test]
    fn zero_delay_is_identity() {
        let mut delay = HaasDelay::new(0);
        for i in 0..100 {
            let sample = i as f32 * 0.01;
            let out = delay.process(sample);
            assert!(
                (out - sample).abs() < 1e-6,
                "zero delay should be identity at sample {}",
                i
            );
        }
    }

    /// A delay of N samples should shift the output by exactly N samples.
    #[test]
    fn delay_shifts_by_n() {
        let n = 20;
        let mut delay = HaasDelay::new(n);

        // Feed N samples of silence, then a known pattern
        for _ in 0..n {
            let out = delay.process(0.0);
            assert!((out - 0.0).abs() < 1e-6, "should output silence during fill");
        }

        // Now the first non-zero sample should appear
        let out = delay.process(1.0);
        assert!((out - 0.0).abs() < 1e-6, "input 1.0 should not appear yet");

        // Feed (n-1) more zeros, then we should see the 1.0
        for _ in 0..(n - 1) {
            delay.process(0.0);
        }
        let out = delay.process(0.0);
        assert!(
            (out - 1.0).abs() < 1e-6,
            "the 1.0 impulse should emerge after exactly {} samples delay",
            n
        );
    }

    /// `from_ms` should calculate the correct number of samples.
    #[test]
    fn from_ms_calculation() {
        // 0.5ms at 44100 Hz = 22.05 → rounds to 22
        let delay = HaasDelay::from_ms(0.5, 44100);
        assert_eq!(delay.delay_samples, 22);

        // 0.3ms at 48000 Hz = 14.4 → rounds to 14
        let delay = HaasDelay::from_ms(0.3, 48000);
        assert_eq!(delay.delay_samples, 14);
    }

    /// Should clamp to MAX_DELAY_SAMPLES - 1 if delay_ms would exceed it.
    #[test]
    fn from_ms_clamps() {
        let delay = HaasDelay::from_ms(100.0, 48000); // way too large
        assert_eq!(delay.delay_samples, MAX_DELAY_SAMPLES - 1);
    }
}
