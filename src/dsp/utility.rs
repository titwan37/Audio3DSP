pub struct Utility {
    gain_linear: f32,
    mute: bool,
    // RT-safe metering state
    pub peak_l: f32,
    pub peak_r: f32,
}

impl Utility {
    pub fn new() -> Self {
        Self { gain_linear: 1.0, mute: false, peak_l: 0.0, peak_r: 0.0 }
    }

    #[inline(always)]
    pub fn set_gain_db(&mut self, db: f32) {
        self.gain_linear = 10.0_f32.powf(db / 20.0);
    }

    #[inline(always)]
    pub fn set_mute(&mut self, mute: bool) { self.mute = mute; }

    #[inline(always)]
    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        if self.mute {
            // Still update meters for visual feedback even when muted
            self.peak_l = self.peak_l.max(l.abs());
            self.peak_r = self.peak_r.max(r.abs());
            return (0.0, 0.0);
        }

        let out_l = l * self.gain_linear;
        let out_r = r * self.gain_linear;

        // Peak hold with slight decay for UI smoothness
        self.peak_l = self.peak_l.max(out_l.abs());
        self.peak_r = self.peak_r.max(out_r.abs());

        (out_l, out_r)
    }
}