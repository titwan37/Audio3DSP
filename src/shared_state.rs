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

    // Pro Stereo Dynamics + Utility
    comp_thresh_bits: AtomicU32,   // Compressor Threshold (-40.0 to 0.0 dB)
    comp_ratio_bits: AtomicU32,    // Compressor Ratio (1.0 to 10.0)
    fader_gain_bits: AtomicU32,    // Master Fader (-20.0 to +6.0 dB)
    mute_state: std::sync::atomic::AtomicBool,

    // Bypass ON/OFF toggles (lock-free)
    widener_enabled: std::sync::atomic::AtomicBool,
    haas_enabled: std::sync::atomic::AtomicBool,
    reverb_enabled: std::sync::atomic::AtomicBool,
    emboss_enabled: std::sync::atomic::AtomicBool,
    comp_enabled: std::sync::atomic::AtomicBool,

    // Live Venue Simulator parameters
    saturation_drive_bits: AtomicU32, // Drive (0.5 to 4.0)
    saturation_mix_bits: AtomicU32,   // Mix (0.0 to 1.0)
    venue_sens_bits: AtomicU32,       // Room Bloom Sensitivity (0.0 to 2.0)
    crossfeed_mix_bits: AtomicU32,    // Crossfeed Mix (0.0 to 1.0)

    // Live Venue bypass toggles
    saturation_enabled: std::sync::atomic::AtomicBool,
    venue_enabled: std::sync::atomic::AtomicBool,
    crossfeed_enabled: std::sync::atomic::AtomicBool,
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

            comp_thresh_bits: AtomicU32::new((-12.0f32).to_bits()),
            comp_ratio_bits: AtomicU32::new(4.0f32.to_bits()),
            fader_gain_bits: AtomicU32::new(0.0f32.to_bits()),
            mute_state: std::sync::atomic::AtomicBool::new(false),

            widener_enabled: std::sync::atomic::AtomicBool::new(true),
            haas_enabled: std::sync::atomic::AtomicBool::new(true),
            reverb_enabled: std::sync::atomic::AtomicBool::new(true),
            emboss_enabled: std::sync::atomic::AtomicBool::new(true),
            comp_enabled: std::sync::atomic::AtomicBool::new(true),

            saturation_drive_bits: AtomicU32::new(1.2f32.to_bits()),
            saturation_mix_bits: AtomicU32::new(0.3f32.to_bits()),
            venue_sens_bits: AtomicU32::new(0.5f32.to_bits()),
            crossfeed_mix_bits: AtomicU32::new(0.4f32.to_bits()),

            saturation_enabled: std::sync::atomic::AtomicBool::new(true),
            venue_enabled: std::sync::atomic::AtomicBool::new(true),
            crossfeed_enabled: std::sync::atomic::AtomicBool::new(true),
        })
    }

    #[inline(always)]
    pub fn get_fader_gain_db(&self) -> f32 {
        f32::from_bits(self.fader_gain_bits.load(Ordering::Relaxed))
    }
    pub fn set_fader_gain_db(&self, val: f32) {
        self.fader_gain_bits.store(val.clamp(-20.0, 6.0).to_bits(), Ordering::Relaxed);
    }

    pub fn is_muted(&self) -> bool { self.mute_state.load(Ordering::Relaxed) }
    pub fn toggle_mute(&self) -> bool { !self.mute_state.fetch_xor(true, Ordering::Relaxed) }

    #[inline(always)]
    pub fn get_comp_thresh(&self) -> f32 {
        f32::from_bits(self.comp_thresh_bits.load(Ordering::Relaxed))
    }
    pub fn set_comp_thresh(&self, val: f32) {
        self.comp_thresh_bits.store(val.clamp(-40.0, 0.0).to_bits(), Ordering::Relaxed);
    }

    #[inline(always)]
    pub fn get_comp_ratio(&self) -> f32 {
        f32::from_bits(self.comp_ratio_bits.load(Ordering::Relaxed))
    }
    pub fn set_comp_ratio(&self, val: f32) {
        self.comp_ratio_bits.store(val.clamp(1.0, 10.0).to_bits(), Ordering::Relaxed);
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

    /// Check if stereo widener module is enabled (lock-free).
    #[inline(always)]
    pub fn is_widener_enabled(&self) -> bool {
        self.widener_enabled.load(Ordering::Relaxed)
    }

    /// Toggle stereo widener bypass state.
    pub fn toggle_widener(&self) -> bool {
        !self.widener_enabled.fetch_xor(true, Ordering::Relaxed)
    }

    /// Set stereo widener enabled state.
    #[allow(dead_code)]
    pub fn set_widener_enabled(&self, val: bool) {
        self.widener_enabled.store(val, Ordering::Relaxed);
    }

    /// Check if Haas delay module is enabled (lock-free).
    #[inline(always)]
    pub fn is_haas_enabled(&self) -> bool {
        self.haas_enabled.load(Ordering::Relaxed)
    }

    /// Toggle Haas delay bypass state.
    pub fn toggle_haas(&self) -> bool {
        !self.haas_enabled.fetch_xor(true, Ordering::Relaxed)
    }

    /// Set Haas delay enabled state.
    #[allow(dead_code)]
    pub fn set_haas_enabled(&self, val: bool) {
        self.haas_enabled.store(val, Ordering::Relaxed);
    }

    /// Check if reverb module is enabled (lock-free).
    #[inline(always)]
    pub fn is_reverb_enabled(&self) -> bool {
        self.reverb_enabled.load(Ordering::Relaxed)
    }

    /// Toggle reverb bypass state.
    pub fn toggle_reverb(&self) -> bool {
        !self.reverb_enabled.fetch_xor(true, Ordering::Relaxed)
    }

    /// Set reverb enabled state.
    #[allow(dead_code)]
    pub fn set_reverb_enabled(&self, val: bool) {
        self.reverb_enabled.store(val, Ordering::Relaxed);
    }

    /// Check if vocal emboss module is enabled (lock-free).
    #[inline(always)]
    pub fn is_emboss_enabled(&self) -> bool {
        self.emboss_enabled.load(Ordering::Relaxed)
    }

    /// Toggle vocal emboss bypass state.
    pub fn toggle_emboss(&self) -> bool {
        !self.emboss_enabled.fetch_xor(true, Ordering::Relaxed)
    }

    /// Set vocal emboss enabled state.
    #[allow(dead_code)]
    pub fn set_emboss_enabled(&self, val: bool) {
        self.emboss_enabled.store(val, Ordering::Relaxed);
    }

    /// Check if stereo compressor dynamics module is enabled (lock-free).
    #[inline(always)]
    pub fn is_comp_enabled(&self) -> bool {
        self.comp_enabled.load(Ordering::Relaxed)
    }

    /// Toggle stereo compressor dynamics bypass state.
    pub fn toggle_comp(&self) -> bool {
        !self.comp_enabled.fetch_xor(true, Ordering::Relaxed)
    }

    /// Set stereo compressor dynamics enabled state.
    #[allow(dead_code)]
    pub fn set_comp_enabled(&self, val: bool) {
        self.comp_enabled.store(val, Ordering::Relaxed);
    }

    // ── Live Venue: Console Saturation ──
    #[inline(always)]
    pub fn get_saturation_drive(&self) -> f32 {
        f32::from_bits(self.saturation_drive_bits.load(Ordering::Relaxed))
    }
    pub fn set_saturation_drive(&self, val: f32) {
        self.saturation_drive_bits.store(val.clamp(0.5, 4.0).to_bits(), Ordering::Relaxed);
    }
    #[inline(always)]
    pub fn get_saturation_mix(&self) -> f32 {
        f32::from_bits(self.saturation_mix_bits.load(Ordering::Relaxed))
    }
    pub fn set_saturation_mix(&self, val: f32) {
        self.saturation_mix_bits.store(val.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }
    #[inline(always)]
    pub fn is_saturation_enabled(&self) -> bool {
        self.saturation_enabled.load(Ordering::Relaxed)
    }
    pub fn toggle_saturation(&self) -> bool {
        !self.saturation_enabled.fetch_xor(true, Ordering::Relaxed)
    }

    // ── Live Venue: Dynamic Venue Expander ──
    #[inline(always)]
    pub fn get_venue_sensitivity(&self) -> f32 {
        f32::from_bits(self.venue_sens_bits.load(Ordering::Relaxed))
    }
    pub fn set_venue_sensitivity(&self, val: f32) {
        self.venue_sens_bits.store(val.clamp(0.0, 2.0).to_bits(), Ordering::Relaxed);
    }
    #[inline(always)]
    pub fn is_venue_enabled(&self) -> bool {
        self.venue_enabled.load(Ordering::Relaxed)
    }
    pub fn toggle_venue(&self) -> bool {
        !self.venue_enabled.fetch_xor(true, Ordering::Relaxed)
    }

    // ── Live Venue: Headphone Crossfeed ──
    #[inline(always)]
    pub fn get_crossfeed_mix(&self) -> f32 {
        f32::from_bits(self.crossfeed_mix_bits.load(Ordering::Relaxed))
    }
    pub fn set_crossfeed_mix(&self, val: f32) {
        self.crossfeed_mix_bits.store(val.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }
    #[inline(always)]
    pub fn is_crossfeed_enabled(&self) -> bool {
        self.crossfeed_enabled.load(Ordering::Relaxed)
    }
    pub fn toggle_crossfeed(&self) -> bool {
        !self.crossfeed_enabled.fetch_xor(true, Ordering::Relaxed)
    }
}
