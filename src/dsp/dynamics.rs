// Stereo-Linked Compressor with lock-free parameter updates
pub struct StereoCompressor {
    threshold_db: f32,
    ratio: f32,
    attack_coeff: f32,
    release_coeff: f32,
    envelope: f32,
    makeup_gain: f32,
}

impl StereoCompressor {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            threshold_db: -12.0,
            ratio: 4.0,
            attack_coeff: Self::calc_coeff(10.0, sample_rate),  // 10ms attack
            release_coeff: Self::calc_coeff(100.0, sample_rate), // 100ms release
            envelope: 1.0,
            makeup_gain: 1.0,
        }
    }

    #[inline(always)]
    fn calc_coeff(time_ms: f32, sr: u32) -> f32 {
        if time_ms <= 0.0 { return 1.0; }
        1.0 - (-1.0 / (time_ms * 0.001 * sr as f32)).exp()
    }

    #[inline(always)]
    pub fn set_threshold(&mut self, db: f32) { self.threshold_db = db; }
    
    #[inline(always)]
    pub fn set_ratio(&mut self, ratio: f32) { self.ratio = ratio.max(1.0); }

    #[inline(always)]
    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        // 1. Stereo-Linked Peak Detection
        let input_level = l.abs().max(r.abs());
        let input_db = if input_level > 1e-5 { 20.0 * input_level.log10() } else { -100.0 };
        
        // 2. Gain Computer
        let over_db = (input_db - self.threshold_db).max(0.0);
        let target_gain_db = -over_db * (1.0 - 1.0 / self.ratio);
        let target_gain_linear = 10.0_f32.powf(target_gain_db / 20.0);
        
        // 3. Ballistics (Attack/Release)
        let coeff = if target_gain_linear < self.envelope { self.attack_coeff } else { self.release_coeff };
        self.envelope += coeff * (target_gain_linear - self.envelope);
        
        // 4. Apply Gain + Makeup
        let final_gain = self.envelope * self.makeup_gain;
        (l * final_gain, r * final_gain)
    }
}