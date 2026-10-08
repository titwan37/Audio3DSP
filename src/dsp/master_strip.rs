// Audio3DSP — Master Strip Orchestrator
//
// Combines the spatial matrix processing (M/S Widener, Haas Delay, Vocal Emboss, Reverb)
// with pro-audio stereo compression, EQ, Live Concert Venue simulation
// (Analog Console Saturation, Dynamic Venue Expander, Headphone Crossfeed),
// and master utility (fader & mute).

use super::chain::DspChain;
use super::crossfeed::HeadphoneCrossfeed;
use super::dynamics::StereoCompressor;
use super::emboss::PeakingEqFilter;
use super::saturation::ConsoleSaturation;
use super::utility::Utility;
use super::venue_expander::VenueExpander;

pub struct MasterStrip {
    pub chain: DspChain,
    pub eq_low_mid: PeakingEqFilter,
    pub eq_high_mid: PeakingEqFilter,
    pub comp: StereoCompressor,

    // ── LIVE VENUE MODULES ──
    pub saturation: ConsoleSaturation,
    pub venue_expander: VenueExpander,
    pub crossfeed: HeadphoneCrossfeed,

    // ── OUTPUT STAGE ──
    pub utility: Utility,
    pub comp_enabled: bool,
}

impl MasterStrip {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            chain: DspChain::new(sample_rate),
            eq_low_mid: PeakingEqFilter::new(sample_rate, 250.0, 0.0, 1.0),
            eq_high_mid: PeakingEqFilter::new(sample_rate, 4000.0, 0.0, 1.0),
            comp: StereoCompressor::new(sample_rate),

            saturation: ConsoleSaturation::new(),
            venue_expander: VenueExpander::new(sample_rate),
            crossfeed: HeadphoneCrossfeed::new(sample_rate),

            utility: Utility::new(),
            comp_enabled: true,
        }
    }

    #[inline(always)]
    pub fn process_frame(&mut self, mut l: f32, mut r: f32) -> (f32, f32) {
        // 1. Tonal Shaping (EQ)
        l = self.eq_low_mid.process(l);
        r = self.eq_low_mid.process(r);
        l = self.eq_high_mid.process(l);
        r = self.eq_high_mid.process(r);

        // 2. Dynamics (Stereo-Linked Compressor)
        if self.comp_enabled {
            (l, r) = self.comp.process(l, r);
        }

        // 3. Spatial Matrix Pipeline (Emboss / Haas / Reverb / M/S Widener)
        (l, r) = self.chain.process_frame(l, r);

        // ── 4. LIVE VENUE SIMULATION STAGES ──
        // A. Analog Console Glue (Soft Saturation)
        (l, r) = self.saturation.process(l, r);

        // B. Dynamic Venue Expander (Crowd/Room Bloom based on vocal energy)
        (l, r) = self.venue_expander.process(l, r);

        // C. Headphone Crossfeed (Moves soundstage out of the head into the room)
        (l, r) = self.crossfeed.process(l, r);

        // 5. Output Stage (Fader / Mute / Metering)
        (l, r) = self.utility.process(l, r);

        // 6. Hard Clipper (True Peak Protection)
        (l.clamp(-1.0, 1.0), r.clamp(-1.0, 1.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn master_strip_silence_in_silence_out() {
        let mut strip = MasterStrip::new(48000);
        let (l, r) = strip.process_frame(0.0, 0.0);
        assert!(!l.is_nan() && !r.is_nan());
        assert!(l.abs() < 1e-4 && r.abs() < 1e-4);
    }

    #[test]
    fn master_strip_mute_zeros_output() {
        let mut strip = MasterStrip::new(48000);
        strip.utility.set_mute(true);
        let (l, r) = strip.process_frame(0.5, 0.5);
        assert_eq!(l, 0.0);
        assert_eq!(r, 0.0);
    }

    #[test]
    fn master_strip_process_finite() {
        let mut strip = MasterStrip::new(48000);
        strip.comp.set_threshold(-6.0);
        strip.comp.set_ratio(4.0);
        strip.utility.set_gain_db(-2.0);
        let (l, r) = strip.process_frame(0.8, -0.7);
        assert!(!l.is_nan() && !r.is_nan());
        assert!(l.abs() <= 1.0 && r.abs() <= 1.0);
    }

    #[test]
    fn master_strip_comp_bypass_toggles() {
        let mut strip = MasterStrip::new(48000);
        assert!(strip.comp_enabled);
        strip.comp_enabled = false;
        assert!(!strip.comp_enabled);
        let (l, r) = strip.process_frame(0.5, 0.5);
        assert!(!l.is_nan() && !r.is_nan());
    }

    #[test]
    fn master_strip_live_venue_modules_active() {
        let mut strip = MasterStrip::new(48000);
        assert!(strip.saturation.enabled);
        assert!(strip.venue_expander.enabled);
        assert!(strip.crossfeed.enabled);
        let (l, r) = strip.process_frame(0.4, 0.3);
        assert!(!l.is_nan() && !r.is_nan());
    }
}