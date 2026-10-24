//! Built-in subtractive synthesizer voice: two band-limited oscillators, ADSR, low-pass SVF.
//! This is our own instrument; it does not reproduce any FL Studio plugin.

use crate::env::Adsr;
use crate::filter::Svf;
use jet_model::{SynthParams, Waveform};

#[derive(Clone, Copy, Debug, Default)]
struct Osc {
    phase: f64,
    increment: f64,
    integrator: f64,
}

/// PolyBLEP residual for a discontinuity at phase 0 (Välimäki & Huovilainen, 2007).
#[inline]
fn poly_blep(t: f64, dt: f64) -> f64 {
    if t < dt {
        let x = t / dt;
        x + x - x * x - 1.0
    } else if t > 1.0 - dt {
        let x = (t - 1.0) / dt;
        x * x + x + x + 1.0
    } else {
        0.0
    }
}

impl Osc {
    #[inline]
    fn next(&mut self, waveform: Waveform, noise: &mut u32) -> f64 {
        let t = self.phase;
        let dt = self.increment;
        let value = match waveform {
            Waveform::Sine => (t * std::f64::consts::TAU).sin(),
            Waveform::Saw => 2.0 * t - 1.0 - poly_blep(t, dt),
            Waveform::Square | Waveform::Triangle => {
                let mut sq = if t < 0.5 { 1.0 } else { -1.0 };
                sq += poly_blep(t, dt);
                sq -= poly_blep((t + 0.5) % 1.0, dt);
                if waveform == Waveform::Triangle {
                    // Leaky integration of the band-limited square.
                    self.integrator = dt * 4.0 * sq + (1.0 - dt * 0.05) * self.integrator;
                    self.integrator
                } else {
                    sq
                }
            }
            Waveform::Noise => {
                // xorshift32, deterministic per voice.
                *noise ^= *noise << 13;
                *noise ^= *noise >> 17;
                *noise ^= *noise << 5;
                f64::from(*noise) / f64::from(u32::MAX) * 2.0 - 1.0
            }
        };
        self.phase += dt;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        value
    }
}

#[derive(Clone, Debug, Default)]
pub struct SynthVoice {
    osc: [Osc; 2],
    second: bool,
    waveform: Waveform,
    env: Adsr,
    filter: [Svf; 1],
    noise: u32,
    active: bool,
}

impl SynthVoice {
    pub fn start(&mut self, params: &SynthParams, frequency: f64, rate: u32, seed: u32) {
        let rate_f = f64::from(rate.max(1));
        self.waveform = params.waveform;
        self.osc[0] = Osc {
            phase: 0.0,
            increment: (frequency / rate_f).min(0.49),
            integrator: 0.0,
        };
        let detune = crate::cents_ratio(f64::from(params.detune_cents));
        self.second = params.detune_cents != 0 && params.waveform != Waveform::Noise;
        self.osc[1] = Osc {
            phase: 0.25,
            increment: (frequency * detune / rate_f).min(0.49),
            integrator: 0.0,
        };
        self.env.configure(
            f32::from(params.attack_ms),
            f32::from(params.decay_ms),
            f32::from(params.sustain) / 1000.0,
            f32::from(params.release_ms),
            rate as f32,
        );
        self.env.trigger();
        self.filter[0].reset();
        self.filter[0].set(
            f32::from(params.cutoff_hz),
            f32::from(params.resonance) / 1000.0,
            rate as f32,
        );
        self.noise = seed | 1;
        self.active = true;
    }

    pub fn release(&mut self) {
        self.env.release();
    }

    /// Hard stop used by voice stealing and panic; the envelope releases quickly instead
    /// of jumping to zero.
    pub fn cut(&mut self, rate: u32) {
        self.env.configure(0.0, 0.0, 0.0, 5.0, rate as f32);
        self.env.release();
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn is_releasing(&self) -> bool {
        self.env.is_releasing()
    }

    pub fn render(
        &mut self,
        left: &mut [f32],
        right: &mut [f32],
        gain_l: f32,
        gain_r: f32,
    ) -> bool {
        if !self.active {
            return false;
        }
        let mix = if self.second { 0.5 } else { 1.0 };
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let mut value = self.osc[0].next(self.waveform, &mut self.noise);
            if self.second {
                value += self.osc[1].next(self.waveform, &mut self.noise);
            }
            let filtered = self.filter[0].lowpass((value * mix) as f32);
            let amp = self.env.tick();
            if !self.env.is_active() {
                self.active = false;
                break;
            }
            let out = filtered * amp;
            *l += out * gain_l;
            *r += out * gain_r;
        }
        self.active
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zero_crossings(data: &[f32]) -> usize {
        data.windows(2)
            .filter(|w| w[0] <= 0.0 && w[1] > 0.0)
            .count()
    }

    #[test]
    fn sine_voice_plays_the_requested_frequency() {
        let params = SynthParams {
            waveform: Waveform::Sine,
            detune_cents: 0,
            cutoff_hz: 20_000,
            resonance: 0,
            attack_ms: 0,
            ..SynthParams::default()
        };
        let mut voice = SynthVoice::default();
        voice.start(&params, 440.0, 48_000, 1);
        let mut l = vec![0.0; 48_000];
        let mut r = vec![0.0; 48_000];
        voice.render(&mut l, &mut r, 1.0, 1.0);
        let crossings = zero_crossings(&l);
        assert!((438..=442).contains(&crossings), "{crossings}");
    }

    #[test]
    fn release_ends_the_voice() {
        let mut voice = SynthVoice::default();
        voice.start(&SynthParams::default(), 220.0, 48_000, 1);
        let mut l = vec![0.0; 4800];
        let mut r = vec![0.0; 4800];
        voice.render(&mut l, &mut r, 1.0, 1.0);
        voice.release();
        let mut l = vec![0.0; 48_000];
        let mut r = vec![0.0; 48_000];
        assert!(!voice.render(&mut l, &mut r, 1.0, 1.0));
        assert!(l.iter().all(|v| v.is_finite()));
    }
}
