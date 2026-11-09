//! Volume curve and ReplayGain handling.

/// ReplayGain for one track: gain in dB and the (linear) sample peak used to avoid clipping.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReplayGain {
    pub gain_db: f32,
    /// Linear sample peak of the track (1.0 = full scale). `None` when the tag is missing.
    pub peak: Option<f32>,
}

impl From<f32> for ReplayGain {
    /// A bare gain in dB, with no peak information (no clipping protection).
    fn from(gain_db: f32) -> Self {
        Self {
            gain_db,
            peak: None,
        }
    }
}

/// Maps a linear volume slider (`0.0..=1.0`) to an amplitude factor. Perceived loudness is closer
/// to logarithmic, so a square keeps the low half of the slider usable.
pub fn volume_curve(volume: f32) -> f32 {
    let v = volume.clamp(0.0, 1.0);
    v * v
}

/// Converts ReplayGain to a linear factor, reduced if needed so `peak * factor <= 1.0`.
/// `None` means no gain information: unity.
pub fn replay_gain_linear(gain: Option<ReplayGain>) -> f32 {
    let Some(rg) = gain else { return 1.0 };
    if !rg.gain_db.is_finite() {
        return 1.0;
    }
    let linear = 10f32.powf(rg.gain_db / 20.0);
    match rg.peak {
        Some(peak) if peak.is_finite() && peak > 0.0 => linear.min(1.0 / peak),
        _ => linear,
    }
}

/// Total linear factor applied to the decoded samples.
pub fn total_gain(volume: f32, replay_gain: Option<ReplayGain>) -> f32 {
    volume_curve(volume) * replay_gain_linear(replay_gain)
}

/// Applies a gain to interleaved samples, ramping linearly over one buffer when the target
/// changed since the previous call so volume changes do not click.
#[derive(Debug)]
pub struct GainRamp {
    current: f32,
}

impl GainRamp {
    pub fn new(initial: f32) -> Self {
        Self { current: initial }
    }

    /// Jumps to `gain` without ramping (used at track boundaries and after a flush).
    pub fn snap(&mut self, gain: f32) {
        self.current = gain;
    }

    pub fn apply(&mut self, buf: &mut [f32], channels: usize, target: f32) {
        if (self.current - target).abs() < 1e-6 {
            if (target - 1.0).abs() >= 1e-6 {
                for s in buf.iter_mut() {
                    *s *= target;
                }
            }
            self.current = target;
            return;
        }
        let frames = buf.len() / channels.max(1);
        let step = (target - self.current) / frames.max(1) as f32;
        let mut g = self.current;
        for frame in buf.chunks_mut(channels.max(1)) {
            g += step;
            for s in frame {
                *s *= g;
            }
        }
        self.current = target;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curve_is_square_and_clamped() {
        assert_eq!(volume_curve(0.5), 0.25);
        assert_eq!(volume_curve(2.0), 1.0);
        assert_eq!(volume_curve(-1.0), 0.0);
    }

    #[test]
    fn minus_six_db_is_about_half() {
        let g = replay_gain_linear(Some(ReplayGain::from(-6.0206)));
        assert!((g - 0.5).abs() < 1e-3);
    }

    #[test]
    fn peak_limits_positive_gain() {
        // +6 dB would be 2.0x, but a 0.8 peak only allows 1.25x.
        let g = replay_gain_linear(Some(ReplayGain {
            gain_db: 6.0,
            peak: Some(0.8),
        }));
        assert!((g - 1.25).abs() < 1e-6);
    }

    #[test]
    fn missing_gain_is_unity() {
        assert_eq!(replay_gain_linear(None), 1.0);
    }

    #[test]
    fn ramp_reaches_target() {
        let mut ramp = GainRamp::new(0.0);
        let mut buf = vec![1.0f32; 8];
        ramp.apply(&mut buf, 2, 1.0);
        assert!((buf[6] - 1.0).abs() < 1e-6 && (buf[7] - 1.0).abs() < 1e-6);
        assert!(buf[0] < 1.0);
    }
}
