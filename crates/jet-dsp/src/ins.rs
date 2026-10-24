//! Sampler INS page modulation [Manual: chansettings_ins]: envelopes (Delay, Attack, Hold, Decay,
//! Sustain, Release, Amount) and LFOs (sine/triangle/square, Delay, Attack, Amount, Speed,
//! Global) on Pan, Volume, Mod X (filter cutoff), Mod Y (resonance) and Pitch, plus the filter.
//!
//! Hypothesis H-SMP-2: the manual names the controls but not their ranges. Ours: 100% Amount is
//! a full-range sweep of Mod X/Mod Y, +-1200 cents of pitch, full-left/right pan and a volume dip
//! to silence; Mod X maps to 20 Hz-20 kHz exponentially; Mod Y maps to the filter Q. Modulators are
//! evaluated every `CONTROL` frames.

use crate::filter::{Biquad, BiquadKind, Svf};
use jet_model::{FilterType, LfoShape, SamplerSettings};

/// Frames between modulation updates.
pub const CONTROL: u32 = 32;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EnvParams {
    pub delay: f32,
    pub attack: f32,
    pub hold: f32,
    pub decay: f32,
    pub sustain: f32,
    pub release: f32,
    pub amount: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LfoParams {
    pub shape: LfoShape,
    pub delay: f32,
    pub attack: f32,
    pub amount: f32,
    /// Cycles per frame.
    pub inc: f64,
    pub global: bool,
}

/// Frame-based INS settings for one channel (built once per plan).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InsParams {
    pub filter: FilterType,
    pub mod_x: f32,
    pub mod_y: f32,
    /// The filter runs (it is bypassed at Mod X 100% / Mod Y 0% with nothing modulating them).
    pub filtering: bool,
    /// Envelopes for Mod X, Mod Y, Pitch.
    pub env: [Option<EnvParams>; 3],
    /// LFOs for Pan, Volume, Mod X, Mod Y, Pitch.
    pub lfo: [Option<LfoParams>; 5],
    pub rate: f32,
}

impl InsParams {
    /// `None` when the INS page changes nothing (the default), so plain voices pay nothing.
    pub fn new(settings: &SamplerSettings, rate: u32, tempo_bpm: f64) -> Option<Self> {
        let frames = |ms: u16| f32::from(ms) * 0.001 * rate as f32;
        let env = std::array::from_fn(|i| {
            let e = settings.mod_env[i];
            (e.on && e.amount != 0).then(|| EnvParams {
                delay: frames(e.delay_ms),
                attack: frames(e.attack_ms),
                hold: frames(e.hold_ms),
                decay: frames(e.decay_ms),
                sustain: f32::from(e.sustain) / 1000.0,
                release: frames(e.release_ms),
                amount: f32::from(e.amount) / 1000.0,
            })
        });
        let lfo = std::array::from_fn(|i| {
            let l = settings.lfo[i];
            (l.amount != 0).then(|| {
                let per_second = f64::from(l.speed_milli) / 1000.0
                    * if l.tempo { tempo_bpm / 60.0 } else { 1.0 };
                LfoParams {
                    shape: l.shape,
                    delay: frames(l.delay_ms),
                    attack: frames(l.attack_ms),
                    amount: f32::from(l.amount) / 1000.0,
                    inc: per_second / f64::from(rate.max(1)),
                    global: l.global,
                }
            })
        });
        let filter = settings.filter;
        let env: [Option<EnvParams>; 3] = env;
        let lfo: [Option<LfoParams>; 5] = lfo;
        let filtering = filter.mod_x < 1000
            || filter.mod_y > 0
            || env[0].is_some()
            || env[1].is_some()
            || lfo[2].is_some()
            || lfo[3].is_some();
        let changes = filtering
            || env[2].is_some()
            || lfo[0].is_some()
            || lfo[1].is_some()
            || lfo[4].is_some();
        changes.then_some(Self {
            filter: filter.kind,
            mod_x: f32::from(filter.mod_x) / 1000.0,
            mod_y: f32::from(filter.mod_y) / 1000.0,
            filtering,
            env,
            lfo,
            rate: rate as f32,
        })
    }
}

fn env_value(p: &EnvParams, t: f32, released: Option<(f32, f32)>) -> f32 {
    let held = |t: f32| {
        if t < p.delay {
            0.0
        } else if t < p.delay + p.attack {
            (t - p.delay) / p.attack.max(1.0)
        } else if t < p.delay + p.attack + p.hold {
            1.0
        } else if t < p.delay + p.attack + p.hold + p.decay {
            let f = (t - p.delay - p.attack - p.hold) / p.decay.max(1.0);
            1.0 - (1.0 - p.sustain) * f
        } else {
            p.sustain
        }
    };
    match released {
        Some((at, level)) => level * (1.0 - (t - at) / p.release.max(1.0)).max(0.0),
        None => held(t),
    }
}

fn lfo_value(p: &LfoParams, t: f32, start_frame: u64) -> f32 {
    if t < p.delay {
        return 0.0;
    }
    let ramp = if p.attack > 0.0 {
        ((t - p.delay) / p.attack).min(1.0)
    } else {
        1.0
    };
    // "Global - ... a 'free running' modulation source. When deselected the LFO will restart with
    // each note": a global LFO's phase follows the output frame counter.
    let elapsed = if p.global {
        start_frame as f64 + f64::from(t)
    } else {
        f64::from(t - p.delay)
    };
    let phase = (elapsed * p.inc).rem_euclid(1.0) as f32;
    let wave = match p.shape {
        LfoShape::Sine => (phase * std::f32::consts::TAU).sin(),
        LfoShape::Triangle => 1.0 - 4.0 * (phase - 0.5).abs(),
        LfoShape::Square => {
            if phase < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
    };
    wave * ramp * p.amount
}

/// Per-voice modulation state.
#[derive(Clone, Copy, Debug)]
pub struct InsState {
    params: InsParams,
    /// Frames since the note started.
    t: f32,
    start_frame: u64,
    /// (frame of note-off, envelope levels then) once released.
    released: Option<(f32, [f32; 3])>,
    left: u32,
    /// Outputs for the current control block.
    pub ratio: f64,
    pub volume: f32,
    /// Balance -1..=1.
    pub pan: f32,
    filters: [[Filter; 2]; 2],
}

#[derive(Clone, Copy, Debug, Default)]
struct Filter {
    svf: Svf,
    biquad: Biquad,
}

impl InsState {
    pub fn new(params: InsParams, start_frame: u64) -> Self {
        Self {
            params,
            t: 0.0,
            start_frame,
            released: None,
            left: 0,
            ratio: 1.0,
            volume: 1.0,
            pan: 0.0,
            filters: [[Filter::default(); 2]; 2],
        }
    }

    pub fn release(&mut self) {
        if self.released.is_none() {
            let levels = std::array::from_fn(|i| {
                self.params.env[i].map_or(0.0, |p| env_value(&p, self.t, None))
            });
            self.released = Some((self.t, levels));
        }
    }

    fn env(&self, i: usize) -> f32 {
        let Some(p) = self.params.env[i] else {
            return 0.0;
        };
        let released = self.released.map(|(at, levels)| (at, levels[i]));
        env_value(&p, self.t, released) * p.amount
    }

    fn lfo(&self, i: usize) -> f32 {
        self.params.lfo[i].map_or(0.0, |p| lfo_value(&p, self.t, self.start_frame))
    }

    fn update(&mut self) {
        let p = self.params;
        // Pitch: envelope and LFO, 100% = +-1200 cents.
        let cents = (self.env(2) + self.lfo(4)) * 1200.0;
        self.ratio = crate::cents_ratio(f64::from(cents));
        // Volume LFO: a dip of |amount| (the sign inverts its phase).
        self.volume = p.lfo[1].map_or(1.0, |l| {
            let wave = if l.amount != 0.0 {
                self.lfo(1) / l.amount.abs()
            } else {
                0.0
            };
            1.0 - l.amount.abs() * (1.0 - wave) * 0.5
        });
        self.pan = self.lfo(0).clamp(-1.0, 1.0);
        if p.filtering {
            let x = (p.mod_x + self.env(0) + self.lfo(2)).clamp(0.0, 1.0);
            let y = (p.mod_y + self.env(1) + self.lfo(3)).clamp(0.0, 1.0);
            let cutoff = 20.0 * 1000f32.powf(x);
            let q = 0.707 + y * 9.3;
            let kind = match p.filter {
                FilterType::Bp => BiquadKind::BandPass,
                FilterType::Hp => BiquadKind::HighPass,
                FilterType::Bs => BiquadKind::Notch,
                _ => BiquadKind::LowPass,
            };
            for stage in &mut self.filters {
                for side in stage.iter_mut() {
                    side.svf.set(cutoff, y, p.rate);
                    side.biquad.set(
                        kind,
                        f64::from(cutoff),
                        f64::from(q),
                        0.0,
                        f64::from(p.rate),
                    );
                }
            }
        }
    }

    /// Call once per output frame before using the outputs.
    #[inline]
    pub fn tick(&mut self) {
        if self.left == 0 {
            self.update();
            self.left = CONTROL;
        }
        self.left -= 1;
        self.t += 1.0;
    }

    /// Filter one stereo frame.
    #[inline]
    pub fn filter(&mut self, l: f32, r: f32) -> (f32, f32) {
        if !self.params.filtering {
            return (l, r);
        }
        let kind = self.params.filter;
        let mut out = [l, r];
        for (side, value) in out.iter_mut().enumerate() {
            let [first, second] = &mut self.filters;
            *value = match kind {
                FilterType::FastLp | FilterType::SvfLp => first[side].svf.lowpass(*value),
                FilterType::SvfLp2 => {
                    let v = first[side].svf.lowpass(*value);
                    second[side].svf.lowpass(v)
                }
                FilterType::Lp2 => {
                    let v = first[side].biquad.process(*value);
                    second[side].biquad.process(v)
                }
                FilterType::Lp | FilterType::Bp | FilterType::Hp | FilterType::Bs => {
                    first[side].biquad.process(*value)
                }
            };
        }
        (out[0], out[1])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jet_model::{Lfo, ModEnvelope};

    #[test]
    fn the_default_ins_page_changes_nothing() {
        assert!(InsParams::new(&SamplerSettings::default(), 48_000, 120.0).is_none());
    }

    #[test]
    fn envelope_stages_follow_delay_attack_hold_decay_sustain_release() {
        let p = EnvParams {
            delay: 10.0,
            attack: 10.0,
            hold: 10.0,
            decay: 10.0,
            sustain: 0.5,
            release: 10.0,
            amount: 1.0,
        };
        assert_eq!(env_value(&p, 5.0, None), 0.0);
        assert!((env_value(&p, 15.0, None) - 0.5).abs() < 1e-6);
        assert_eq!(env_value(&p, 25.0, None), 1.0);
        assert!((env_value(&p, 35.0, None) - 0.75).abs() < 1e-6);
        assert_eq!(env_value(&p, 100.0, None), 0.5);
        assert!((env_value(&p, 105.0, Some((100.0, 0.5))) - 0.25).abs() < 1e-6);
        assert_eq!(env_value(&p, 200.0, Some((100.0, 0.5))), 0.0);
    }

    #[test]
    fn pitch_envelope_and_lfo_set_the_ratio() {
        let mut settings = SamplerSettings::default();
        settings.mod_env[2] = ModEnvelope {
            on: true,
            delay_ms: 0,
            attack_ms: 0,
            hold_ms: 1000,
            decay_ms: 0,
            sustain: 1000,
            release_ms: 0,
            amount: 1000,
        };
        let params = InsParams::new(&settings, 48_000, 120.0).unwrap();
        assert!(!params.filtering);
        let mut state = InsState::new(params, 0);
        for _ in 0..100 {
            state.tick();
        }
        assert!(
            (state.ratio - 2.0).abs() < 1e-6,
            "+1200 cents at full amount"
        );

        let mut settings = SamplerSettings::default();
        settings.lfo[0] = Lfo {
            amount: -1000,
            speed_milli: 1000,
            ..Lfo::default()
        };
        let mut state = InsState::new(InsParams::new(&settings, 48_000, 120.0).unwrap(), 0);
        // A quarter cycle of a 1 Hz sine: the inverted pan LFO is fully left.
        for _ in 0..12_001 {
            state.tick();
        }
        assert!((state.pan + 1.0).abs() < 0.01, "{}", state.pan);
    }
}
