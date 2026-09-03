// Audio3DSP — Mid/Side Matrix Stereo Widener
//
// Decodes stereo L/R into Mid and Side components, scales the Side
// channel to widen the stereo image, then encodes back to L/R.
//
// Algorithm:
//   mid  = (L + R) * 0.5
//   side = (L - R) * 0.5
//   side *= side_gain
//   L_out = mid + side
//   R_out = mid - side
//
// All operations are pure arithmetic — zero allocations, no branching.

/// Mid/Side matrix stereo widener.
///
/// A `side_gain` of 1.0 is unity (no change). Values > 1.0 widen the
/// stereo image; values < 1.0 narrow it toward mono.
pub struct MsWidener {
    side_gain: f32,
}

impl MsWidener {
    /// Create a new widener with the given side channel gain multiplier.
    ///
    /// # Panics
    /// Panics if `side_gain` is negative.
    pub fn new(side_gain: f32) -> Self {
        assert!(side_gain >= 0.0, "side_gain must be non-negative");
        Self { side_gain }
    }

    /// Dynamically update the side channel gain.
    pub fn set_side_gain(&mut self, side_gain: f32) {
        self.side_gain = side_gain.max(0.0);
    }

    /// Process a single stereo frame. Returns `(left_out, right_out)`.
    #[inline(always)]
    pub fn process(&self, left: f32, right: f32) -> (f32, f32) {
        let mid = (left + right) * 0.5;
        let side = (left - right) * 0.5 * self.side_gain;
        (mid + side, mid - side)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Unity gain should produce (approximately) the original signal.
    #[test]
    fn unity_gain_is_identity() {
        let w = MsWidener::new(1.0);
        let (l, r) = w.process(0.8, -0.3);
        assert!((l - 0.8).abs() < 1e-6, "left mismatch: {}", l);
        assert!((r - (-0.3)).abs() < 1e-6, "right mismatch: {}", r);
    }

    /// Zero gain should collapse to mono (mid only).
    #[test]
    fn zero_gain_is_mono() {
        let w = MsWidener::new(0.0);
        let (l, r) = w.process(1.0, -1.0);
        // mid = (1.0 + -1.0) * 0.5 = 0.0, side = 0
        assert!((l - 0.0).abs() < 1e-6);
        assert!((r - 0.0).abs() < 1e-6);
    }

    /// Higher side gain should increase stereo width (side energy).
    #[test]
    fn higher_gain_widens() {
        let w_normal = MsWidener::new(1.0);
        let w_wide = MsWidener::new(1.4);

        let (ln, rn) = w_normal.process(0.7, 0.3);
        let (lw, rw) = w_wide.process(0.7, 0.3);

        // Side energy = (L - R)^2; wider gain should produce larger difference
        let side_normal = (ln - rn).abs();
        let side_wide = (lw - rw).abs();
        assert!(
            side_wide > side_normal,
            "wider gain should produce larger stereo difference"
        );
    }

    /// Mono input (L == R) should be unaffected by side gain.
    #[test]
    fn mono_input_unaffected() {
        let w = MsWidener::new(2.0);
        let (l, r) = w.process(0.5, 0.5);
        assert!((l - 0.5).abs() < 1e-6);
        assert!((r - 0.5).abs() < 1e-6);
    }
}
