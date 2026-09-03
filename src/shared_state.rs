// Audio3DSP — Lock-free Shared Audio Parameters & Metering
//
// Uses atomic bit-patterns (f32::to_bits / f32::from_bits) to pass
// parameters between the UI thread and the WASAPI audio thread
// with zero mutexes, zero locks, and zero allocations.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

/// Thread-safe atomic parameter store for real-time DSP control & RMS metering.
pub struct SharedParams {
    side_gain_bits: AtomicU32,   // M/S Stereo Width Gain (1.0x to 3.0x)
    haas_delay_bits: AtomicU32,  // Haas Delay Time (0.0ms to 40.0ms)
    reverb_wet_bits: AtomicU32,  // Reverb Wet Mix (0.0 to 1.0)
    emboss_gain_bits: AtomicU32, // Vocal Emboss Presence Gain (0.0 dB to 6.0 dB)
    rms_left_bits: AtomicU32,    // Live RMS Level Left Channel (0.0 to 1.0)
    rms_right_bits: AtomicU32,   // Live RMS Level Right Channel (0.0 to 1.0)
}

impl SharedParams {
    /// Create a new shared parameter container initialized with defaults.
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            side_gain_bits: AtomicU32::new(2.0f32.to_bits()),
            haas_delay_bits: AtomicU32::new(10.0f32.to_bits()),
            reverb_wet_bits: AtomicU32::new(0.25f32.to_bits()),
            emboss_gain_bits: AtomicU32::new(3.0f32.to_bits()),
            rms_left_bits: AtomicU32::new(0.0f32.to_bits()),
            rms_right_bits: AtomicU32::new(0.0f32.to_bits()),
        })
    }

    /// Read M/S Side Gain multiplier (lock-free).
    #[inline(always)]
    pub fn get_side_gain(&self) -> f32 {
        f32::from_bits(self.side_gain_bits.load(Ordering::Relaxed))
    }

    /// Update M/S Side Gain multiplier (lock-free).
    pub fn set_side_gain(&self, val: f32) {
        self.side_gain_bits
            .store(val.clamp(1.0, 3.0).to_bits(), Ordering::Relaxed);
    }

    /// Read Haas Delay in milliseconds (lock-free).
    #[inline(always)]
    pub fn get_haas_delay_ms(&self) -> f32 {
        f32::from_bits(self.haas_delay_bits.load(Ordering::Relaxed))
    }

    /// Update Haas Delay in milliseconds (lock-free).
    pub fn set_haas_delay_ms(&self, val: f32) {
        self.haas_delay_bits
            .store(val.clamp(0.0, 40.0).to_bits(), Ordering::Relaxed);
    }

    /// Read Reverb Wet Mix ratio (lock-free).
    #[inline(always)]
    pub fn get_reverb_wet(&self) -> f32 {
        f32::from_bits(self.reverb_wet_bits.load(Ordering::Relaxed))
    }

    /// Update Reverb Wet Mix ratio (lock-free).
    pub fn set_reverb_wet(&self, val: f32) {
        self.reverb_wet_bits
            .store(val.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }

    /// Read Vocal Emboss Presence Gain in dB (lock-free).
    #[inline(always)]
    pub fn get_emboss_gain_db(&self) -> f32 {
        f32::from_bits(self.emboss_gain_bits.load(Ordering::Relaxed))
    }

    /// Update Vocal Emboss Presence Gain in dB (lock-free).
    pub fn set_emboss_gain_db(&self, val: f32) {
        self.emboss_gain_bits
            .store(val.clamp(0.0, 6.0).to_bits(), Ordering::Relaxed);
    }

    /// Read live RMS level (Left, Right) (lock-free).
    #[inline(always)]
    pub fn get_rms(&self) -> (f32, f32) {
        let left = f32::from_bits(self.rms_left_bits.load(Ordering::Relaxed));
        let right = f32::from_bits(self.rms_right_bits.load(Ordering::Relaxed));
        (left, right)
    }

    /// Update live RMS level (Left, Right) from audio callback (lock-free).
    #[inline(always)]
    pub fn set_rms(&self, left: f32, right: f32) {
        self.rms_left_bits
            .store(left.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
        self.rms_right_bits
            .store(right.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }
}
