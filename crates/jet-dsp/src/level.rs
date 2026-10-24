//! Level and panning curves.
//!
//! Points with a manual source are exact; the curves between them are our own
//! interpolation and are tracked as hypotheses in docs/FL_STUDIO_COMPLETE_FEATURES.md.

use jet_model::{MIXER_UNITY, PanLaw};

pub fn db_to_gain(db: f64) -> f64 {
    10f64.powf(db / 20.0)
}

pub fn gain_to_db(gain: f64) -> f64 {
    if gain <= 0.0 {
        f64::NEG_INFINITY
    } else {
        20.0 * gain.log10()
    }
}

/// Exponent of the channel volume curve `gain = knob^k`.
/// Fixed by two manual points: 100% is 0 dB and 78% is -5.2 dB
/// [Manual: channelrack, "Samples ... set to -5.2 dB (78%) ... set volume to 100% to restore"].
/// Hypothesis H-VOL-1: the shape between those points is a power law.
pub const CHANNEL_VOLUME_EXPONENT: f64 = 2.4224;

/// Channel volume knob (0..=1000) to linear gain.
pub fn channel_volume_gain(volume: u16) -> f64 {
    let knob = f64::from(volume.min(1000)) / 1000.0;
    knob.powf(CHANNEL_VOLUME_EXPONENT)
}

/// Exponent of the mixer fader curve above and below unity.
/// Hypothesis H-MIX-1: unity at 80% travel and +5.6 dB at the top; not stated in the
/// downloaded manual pages, so both the points and the curve are unverified.
pub const MIXER_TOP_DB: f64 = 5.6;

/// Mixer fader (0..=1000) to linear gain.
pub fn mixer_fader_gain(volume: u16) -> f64 {
    let position = f64::from(volume.min(1000)) / f64::from(MIXER_UNITY);
    if position <= 0.0 {
        return 0.0;
    }
    // (1000/800)^k = +5.6 dB
    let k = db_to_gain(MIXER_TOP_DB).ln() / (1000.0 / f64::from(MIXER_UNITY)).ln();
    position.powf(k)
}

/// Send knob (0..=1000) to gain; the send knob shares the fader curve (Hypothesis H-MIX-2).
pub fn send_gain(level: u16) -> f64 {
    mixer_fader_gain(level)
}

/// Velocity (0..=127) to gain. Hypothesis H-VEL-1: velocity 100 is unity, because the
/// manual's "8.2 dB lower" total for a default Sampler is exactly the 5.2 dB volume plus
/// the 3 dB pan law [Manual: mixer_levelsandmixing].
pub fn velocity_gain(velocity: u8) -> f64 {
    f64::from(velocity.min(127)) / 100.0
}

/// Left/right gains for a pan of -1000..=1000 [Manual: songsettings_settings, "Panning law"].
/// Circular: constant power, -3 dB at center, 0 dB on the panned side at the extremes.
/// Triangular: no compensation; every position sits 3 dB under the circular extremes.
pub fn pan_gains(pan: i16, law: PanLaw) -> (f64, f64) {
    let p = f64::from(pan.clamp(-1000, 1000)) / 1000.0;
    match law {
        // Exact at the extremes, where cos(pi/2) is not exactly zero in floating point.
        PanLaw::Circular if p >= 1.0 => (0.0, 1.0),
        PanLaw::Circular if p <= -1.0 => (1.0, 0.0),
        PanLaw::Circular => {
            let angle = (p + 1.0) * std::f64::consts::FRAC_PI_4;
            (angle.cos(), angle.sin())
        }
        PanLaw::Triangular => {
            let half = std::f64::consts::FRAC_1_SQRT_2;
            ((1.0 - p).min(1.0) * half, (1.0 + p).min(1.0) * half)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tolerance: f64) -> bool {
        (a - b).abs() <= tolerance
    }

    #[test]
    fn channel_volume_matches_the_two_manual_points() {
        assert!(close(gain_to_db(channel_volume_gain(1000)), 0.0, 1e-9));
        assert!(close(gain_to_db(channel_volume_gain(781)), -5.2, 0.01));
        assert_eq!(channel_volume_gain(0), 0.0);
    }

    #[test]
    fn circular_pan_is_minus_three_db_at_center_and_unity_at_the_edge() {
        let (l, r) = pan_gains(0, PanLaw::Circular);
        assert!(close(gain_to_db(l), -3.0103, 1e-3) && close(l, r, 1e-12));
        let (l, r) = pan_gains(1000, PanLaw::Circular);
        assert!(close(l, 0.0, 1e-12) && close(r, 1.0, 1e-12));
        let (l, r) = pan_gains(-1000, PanLaw::Circular);
        assert!(close(l, 1.0, 1e-12) && close(r, 0.0, 1e-12));
    }

    #[test]
    fn triangular_is_three_db_under_circular_extremes() {
        let (_, r) = pan_gains(1000, PanLaw::Triangular);
        assert!(close(gain_to_db(r), -3.0103, 1e-3));
        let (l, r) = pan_gains(0, PanLaw::Triangular);
        assert!(close(l, r, 1e-12) && close(gain_to_db(l), -3.0103, 1e-3));
    }

    #[test]
    fn default_sampler_level_is_8_2_db_under_recorded_level() {
        // [Manual: mixer_levelsandmixing] "the default load state for a Channel Sampler can be 8.2 dB lower"
        let gain = channel_volume_gain(jet_model::DEFAULT_CHANNEL_VOLUME)
            * velocity_gain(jet_model::DEFAULT_VELOCITY)
            * pan_gains(0, PanLaw::Circular).0;
        assert!(close(gain_to_db(gain), -8.2, 0.02), "{}", gain_to_db(gain));
    }

    #[test]
    fn mixer_fader_unity_and_top() {
        assert!(close(mixer_fader_gain(MIXER_UNITY), 1.0, 1e-12));
        assert!(close(
            gain_to_db(mixer_fader_gain(1000)),
            MIXER_TOP_DB,
            1e-9
        ));
        assert_eq!(mixer_fader_gain(0), 0.0);
        assert!(mixer_fader_gain(400) < mixer_fader_gain(600));
    }
}
