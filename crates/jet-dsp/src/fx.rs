//! Insert effects. Each effect owns its buffers (allocated in `new`, never while
//! processing) and processes stereo blocks in place.
//!
//! The algorithms are textbook designs, cited per effect; none reproduces an FL plugin.

use crate::filter::{Biquad, BiquadKind};
use crate::level::db_to_gain;
use jet_model::FxKind;

/// Stereo in-place processor.
pub trait Effect: Send {
    fn process(&mut self, left: &mut [f32], right: &mut [f32]);
    fn reset(&mut self);
    /// Latency the effect adds, in frames (for delay compensation).
    fn latency(&self) -> usize {
        0
    }
}

/// Build an effect for an FX slot description at `rate` and `tempo_bpm`.
pub fn build(kind: &FxKind, rate: u32, tempo_bpm: f64) -> Box<dyn Effect> {
    let rate_f = f64::from(rate.max(1));
    match kind {
        FxKind::Eq(bands) => Box::new(ParametricEq::new(bands, rate_f)),
        FxKind::Delay {
            time_beats_milli,
            feedback,
            mix,
            ping_pong,
        } => {
            let seconds = f64::from(*time_beats_milli) / 1000.0 * 60.0 / tempo_bpm.max(1.0);
            Box::new(Delay::new(
                seconds,
                f64::from(*feedback) / 1000.0,
                f64::from(*mix) / 1000.0,
                *ping_pong,
                rate_f,
            ))
        }
        FxKind::Reverb {
            room,
            damping,
            width,
            mix,
        } => Box::new(Reverb::new(
            f32::from(*room) / 1000.0,
            f32::from(*damping) / 1000.0,
            f32::from(*width) / 1000.0,
            f32::from(*mix) / 1000.0,
            rate_f,
        )),
        FxKind::Compressor {
            threshold_db_tenths,
            ratio_tenths,
            attack_ms,
            release_ms,
            makeup_db_tenths,
        } => Box::new(Compressor::new(
            f64::from(*threshold_db_tenths) / 10.0,
            f64::from(*ratio_tenths) / 10.0,
            f64::from(*attack_ms),
            f64::from(*release_ms),
            f64::from(*makeup_db_tenths) / 10.0,
            rate_f,
        )),
        FxKind::Limiter {
            ceiling_db_tenths,
            release_ms,
        } => Box::new(Limiter::new(
            f64::from(*ceiling_db_tenths) / 10.0,
            f64::from(*release_ms),
            rate_f,
        )),
        FxKind::Distortion { drive, mix } => Box::new(Distortion {
            drive: 1.0 + f32::from(*drive) / 1000.0 * 24.0,
            mix: f32::from(*mix) / 1000.0,
        }),
        FxKind::Chorus {
            rate_milli_hz,
            depth,
            mix,
        } => Box::new(Chorus::new(
            f64::from(*rate_milli_hz) / 1000.0,
            f64::from(*depth) / 1000.0,
            f32::from(*mix) / 1000.0,
            rate_f,
        )),
        FxKind::Gate {
            threshold_db_tenths,
            release_ms,
        } => Box::new(Gate::new(
            f64::from(*threshold_db_tenths) / 10.0,
            f64::from(*release_ms),
            rate_f,
        )),
        FxKind::Flanger {
            rate_milli_hz,
            depth,
            feedback,
            mix,
        } => Box::new(Flanger::new(
            f64::from(*rate_milli_hz) / 1000.0,
            f64::from(*depth) / 1000.0,
            f32::from(*feedback) / 1000.0,
            f32::from(*mix) / 1000.0,
            rate_f,
        )),
        FxKind::Phaser {
            rate_milli_hz,
            depth,
            feedback,
            mix,
        } => Box::new(Phaser::new(
            f64::from(*rate_milli_hz) / 1000.0,
            f64::from(*depth) / 1000.0,
            f32::from(*feedback) / 1000.0,
            f32::from(*mix) / 1000.0,
            rate_f,
        )),
        FxKind::Filter {
            mode,
            cutoff_hz,
            resonance,
        } => {
            let kind = match mode {
                1 => BiquadKind::HighPass,
                2 => BiquadKind::BandPass,
                _ => BiquadKind::LowPass,
            };
            // Resonance 0..1 maps to Q 0.5..20 (our mapping).
            let q = 0.5 + (f64::from(*resonance) / 1000.0).powi(2) * 19.5;
            Box::new(Filter {
                left: Biquad::new(kind, f64::from(*cutoff_hz), q, 0.0, rate_f),
                right: Biquad::new(kind, f64::from(*cutoff_hz), q, 0.0, rate_f),
            })
        }
        FxKind::StereoShaper {
            r_to_l,
            left,
            right,
            l_to_r,
            delay_ms,
            pre,
        } => {
            let gain = |g: i16| f32::from(g) / 1000.0;
            let frames = (f64::from(delay_ms.unsigned_abs()) * 0.001 * rate_f).round() as usize;
            Box::new(StereoShaper {
                matrix: [gain(*r_to_l), gain(*left), gain(*right), gain(*l_to_r)],
                delay_right: *delay_ms > 0,
                line: vec![0.0; frames.max(1)],
                frames,
                pos: 0,
                pre: *pre,
            })
        }
    }
}

/// Flanger: a short (0.5 to ~5 ms) delay swept by a sine LFO with feedback.
pub struct Flanger {
    left: Vec<f32>,
    right: Vec<f32>,
    write: usize,
    phase: f64,
    increment: f64,
    base: f64,
    depth: f64,
    feedback: f32,
    mix: f32,
}

impl Flanger {
    pub fn new(rate_hz: f64, depth: f64, feedback: f32, mix: f32, rate: f64) -> Self {
        let size = (rate * 0.012) as usize + 4;
        Self {
            left: vec![0.0; size],
            right: vec![0.0; size],
            write: 0,
            phase: 0.0,
            increment: rate_hz.clamp(0.01, 10.0) / rate,
            base: rate * 0.0005,
            depth: rate * 0.0045 * depth.clamp(0.0, 1.0),
            feedback: feedback.clamp(0.0, 0.95),
            mix: mix.clamp(0.0, 1.0),
        }
    }
}

impl Effect for Flanger {
    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let tau = std::f64::consts::TAU;
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let lfo = 0.5 - 0.5 * (tau * self.phase).cos();
            let delay = self.base + self.depth * lfo;
            let wl = Chorus::tap(&self.left, self.write, delay);
            let wr = Chorus::tap(&self.right, self.write, delay);
            self.left[self.write] = *l + wl * self.feedback;
            self.right[self.write] = *r + wr * self.feedback;
            *l = *l * (1.0 - self.mix * 0.5) + wl * self.mix * 0.5;
            *r = *r * (1.0 - self.mix * 0.5) + wr * self.mix * 0.5;
            self.write = (self.write + 1) % self.left.len();
            self.phase = (self.phase + self.increment) % 1.0;
        }
    }

    fn reset(&mut self) {
        self.left.fill(0.0);
        self.right.fill(0.0);
    }
}

/// Phaser: six first-order all-pass stages whose break frequency an LFO sweeps between
/// 200 Hz and ~5 kHz (exponentially), with feedback.
pub struct Phaser {
    state: [[f32; 6]; 2],
    last: [f32; 2],
    phase: f64,
    increment: f64,
    depth: f64,
    feedback: f32,
    mix: f32,
    rate: f64,
}

impl Phaser {
    pub fn new(rate_hz: f64, depth: f64, feedback: f32, mix: f32, rate: f64) -> Self {
        Self {
            state: [[0.0; 6]; 2],
            last: [0.0; 2],
            phase: 0.0,
            increment: rate_hz.clamp(0.01, 10.0) / rate,
            depth: depth.clamp(0.0, 1.0),
            feedback: feedback.clamp(0.0, 0.95),
            mix: mix.clamp(0.0, 1.0),
            rate,
        }
    }
}

impl Effect for Phaser {
    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let tau = std::f64::consts::TAU;
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let lfo = 0.5 - 0.5 * (tau * self.phase).cos();
            let freq = 200.0 * 25f64.powf(lfo * self.depth);
            let t = (std::f64::consts::PI * freq / self.rate).tan();
            let coeff = ((t - 1.0) / (t + 1.0)) as f32;
            for (side, sample) in [l, r].into_iter().enumerate() {
                let mut x = *sample + self.last[side] * self.feedback;
                for z in &mut self.state[side] {
                    // y = c*x + z;  z' = x - c*y  (first-order all-pass, transposed form)
                    let y = coeff * x + *z;
                    *z = x - coeff * y;
                    x = y;
                }
                self.last[side] = x;
                *sample = *sample * (1.0 - self.mix * 0.5) + x * self.mix * 0.5;
            }
            self.phase = (self.phase + self.increment) % 1.0;
        }
    }

    fn reset(&mut self) {
        self.state = [[0.0; 6]; 2];
        self.last = [0.0; 2];
    }
}

/// Resonant filter (RBJ biquad) as an insert effect.
pub struct Filter {
    left: Biquad,
    right: Biquad,
}

impl Effect for Filter {
    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        for sample in left.iter_mut() {
            *sample = self.left.process(*sample);
        }
        for sample in right.iter_mut() {
            *sample = self.right.process(*sample);
        }
    }

    fn reset(&mut self) {
        self.left.reset();
        self.right.reset();
    }
}

/// Parametric EQ made of RBJ biquads.
pub struct ParametricEq {
    left: Vec<Biquad>,
    right: Vec<Biquad>,
}

impl ParametricEq {
    pub fn new(bands: &[jet_model::EqBand], rate: f64) -> Self {
        let make = || {
            bands
                .iter()
                .map(|band| {
                    let kind = match band.shape {
                        jet_model::EqShape::Peak => BiquadKind::Peak,
                        jet_model::EqShape::LowShelf => BiquadKind::LowShelf,
                        jet_model::EqShape::HighShelf => BiquadKind::HighShelf,
                        jet_model::EqShape::LowPass => BiquadKind::LowPass,
                        jet_model::EqShape::HighPass => BiquadKind::HighPass,
                    };
                    Biquad::new(
                        kind,
                        f64::from(band.freq_hz),
                        f64::from(band.q_milli) / 1000.0,
                        f64::from(band.gain_db_tenths) / 10.0,
                        rate,
                    )
                })
                .collect::<Vec<_>>()
        };
        Self {
            left: make(),
            right: make(),
        }
    }
}

impl Effect for ParametricEq {
    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        for filter in &mut self.left {
            for sample in left.iter_mut() {
                *sample = filter.process(*sample);
            }
        }
        for filter in &mut self.right {
            for sample in right.iter_mut() {
                *sample = filter.process(*sample);
            }
        }
    }

    fn reset(&mut self) {
        self.left
            .iter_mut()
            .chain(self.right.iter_mut())
            .for_each(Biquad::reset);
    }
}

/// Feedback delay line with optional ping-pong cross-feedback.
pub struct Delay {
    left: Vec<f32>,
    right: Vec<f32>,
    write: usize,
    delay: usize,
    feedback: f32,
    mix: f32,
    ping_pong: bool,
}

impl Delay {
    pub fn new(seconds: f64, feedback: f64, mix: f64, ping_pong: bool, rate: f64) -> Self {
        let delay = ((seconds * rate) as usize).clamp(1, (rate * 4.0) as usize);
        Self {
            left: vec![0.0; delay + 1],
            right: vec![0.0; delay + 1],
            write: 0,
            delay,
            feedback: feedback.clamp(0.0, 0.95) as f32,
            mix: mix.clamp(0.0, 1.0) as f32,
            ping_pong,
        }
    }
}

impl Effect for Delay {
    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let size = self.left.len();
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let read = (self.write + size - self.delay) % size;
            let dl = self.left[read];
            let dr = self.right[read];
            let (fl, fr) = if self.ping_pong { (dr, dl) } else { (dl, dr) };
            let input_l = if self.ping_pong { (*l + *r) * 0.5 } else { *l };
            self.left[self.write] = input_l + fl * self.feedback;
            self.right[self.write] = if self.ping_pong { 0.0 } else { *r } + fr * self.feedback;
            *l = *l * (1.0 - self.mix) + dl * self.mix;
            *r = *r * (1.0 - self.mix) + dr * self.mix;
            self.write = (self.write + 1) % size;
        }
    }

    fn reset(&mut self) {
        self.left.fill(0.0);
        self.right.fill(0.0);
    }
}

struct Comb {
    buffer: Vec<f32>,
    index: usize,
    store: f32,
}

impl Comb {
    fn new(size: usize) -> Self {
        Self {
            buffer: vec![0.0; size.max(1)],
            index: 0,
            store: 0.0,
        }
    }

    #[inline]
    fn process(&mut self, input: f32, feedback: f32, damp: f32) -> f32 {
        let output = self.buffer[self.index];
        self.store = output * (1.0 - damp) + self.store * damp;
        self.buffer[self.index] = input + self.store * feedback;
        self.index = (self.index + 1) % self.buffer.len();
        output
    }
}

struct Allpass {
    buffer: Vec<f32>,
    index: usize,
}

impl Allpass {
    fn new(size: usize) -> Self {
        Self {
            buffer: vec![0.0; size.max(1)],
            index: 0,
        }
    }

    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let buffered = self.buffer[self.index];
        let output = buffered - input;
        self.buffer[self.index] = input + buffered * 0.5;
        self.index = (self.index + 1) % self.buffer.len();
        output
    }
}

/// Schroeder–Moorer reverb with the public-domain "Freeverb" tunings (Jezar, 2000):
/// 8 parallel low-pass combs and 4 series allpasses per side.
pub struct Reverb {
    combs: [Vec<Comb>; 2],
    allpasses: [Vec<Allpass>; 2],
    feedback: f32,
    damp: f32,
    wet1: f32,
    wet2: f32,
    dry: f32,
}

impl Reverb {
    const COMBS: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
    const ALLPASSES: [usize; 4] = [556, 441, 341, 225];
    const SPREAD: usize = 23;

    pub fn new(room: f32, damping: f32, width: f32, mix: f32, rate: f64) -> Self {
        let scale = rate / 44_100.0;
        let side = |spread: usize| {
            (
                Self::COMBS
                    .iter()
                    .map(|n| Comb::new(((n + spread) as f64 * scale) as usize))
                    .collect::<Vec<_>>(),
                Self::ALLPASSES
                    .iter()
                    .map(|n| Allpass::new(((n + spread) as f64 * scale) as usize))
                    .collect::<Vec<_>>(),
            )
        };
        let (cl, al) = side(0);
        let (cr, ar) = side(Self::SPREAD);
        let wet = mix.clamp(0.0, 1.0) * 3.0;
        let width = width.clamp(0.0, 1.0);
        Self {
            combs: [cl, cr],
            allpasses: [al, ar],
            feedback: 0.7 + room.clamp(0.0, 1.0) * 0.28,
            damp: damping.clamp(0.0, 1.0) * 0.4,
            wet1: wet * (width / 2.0 + 0.5),
            wet2: wet * ((1.0 - width) / 2.0),
            dry: 1.0 - mix.clamp(0.0, 1.0),
        }
    }
}

impl Effect for Reverb {
    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let input = (*l + *r) * 0.015;
            let mut outs = [0.0_f32; 2];
            for (side, out) in outs.iter_mut().enumerate() {
                let mut acc = 0.0;
                for comb in &mut self.combs[side] {
                    acc += comb.process(input, self.feedback, self.damp);
                }
                for allpass in &mut self.allpasses[side] {
                    acc = allpass.process(acc);
                }
                *out = acc;
            }
            let [ol, or] = outs;
            *l = *l * self.dry + ol * self.wet1 + or * self.wet2;
            *r = *r * self.dry + or * self.wet1 + ol * self.wet2;
        }
    }

    fn reset(&mut self) {
        for side in 0..2 {
            for comb in &mut self.combs[side] {
                comb.buffer.fill(0.0);
                comb.store = 0.0;
            }
            for allpass in &mut self.allpasses[side] {
                allpass.buffer.fill(0.0);
            }
        }
    }
}

fn coefficient(ms: f64, rate: f64) -> f64 {
    if ms <= 0.0 {
        0.0
    } else {
        (-1.0 / (ms * 0.001 * rate)).exp()
    }
}

/// Feed-forward peak compressor with a hard knee, stereo-linked detector.
pub struct Compressor {
    threshold_db: f64,
    ratio: f64,
    attack: f64,
    release: f64,
    makeup: f64,
    envelope_db: f64,
}

impl Compressor {
    pub fn new(
        threshold_db: f64,
        ratio: f64,
        attack_ms: f64,
        release_ms: f64,
        makeup_db: f64,
        rate: f64,
    ) -> Self {
        Self {
            threshold_db,
            ratio: ratio.max(1.0),
            attack: coefficient(attack_ms, rate),
            release: coefficient(release_ms, rate),
            makeup: db_to_gain(makeup_db),
            envelope_db: -120.0,
        }
    }

    /// Static gain reduction in dB for an input level in dB.
    pub fn reduction_db(&self, input_db: f64) -> f64 {
        if input_db <= self.threshold_db {
            0.0
        } else {
            (input_db - self.threshold_db) * (1.0 / self.ratio - 1.0)
        }
    }
}

impl Effect for Compressor {
    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let peak = f64::from(l.abs().max(r.abs()));
            let level_db = if peak > 1e-6 {
                20.0 * peak.log10()
            } else {
                -120.0
            };
            let coeff = if level_db > self.envelope_db {
                self.attack
            } else {
                self.release
            };
            self.envelope_db = level_db + coeff * (self.envelope_db - level_db);
            let gain = (db_to_gain(self.reduction_db(self.envelope_db)) * self.makeup) as f32;
            *l *= gain;
            *r *= gain;
        }
    }

    fn reset(&mut self) {
        self.envelope_db = -120.0;
    }
}

/// Peak limiter: instant attack, exponential release; never exceeds the ceiling.
pub struct Limiter {
    ceiling: f64,
    release: f64,
    gain: f64,
}

impl Limiter {
    pub fn new(ceiling_db: f64, release_ms: f64, rate: f64) -> Self {
        Self {
            ceiling: db_to_gain(ceiling_db.min(0.0)),
            release: coefficient(release_ms.max(1.0), rate),
            gain: 1.0,
        }
    }
}

impl Effect for Limiter {
    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let peak = f64::from(l.abs().max(r.abs()));
            let needed = if peak > self.ceiling {
                self.ceiling / peak
            } else {
                1.0
            };
            self.gain = if needed < self.gain {
                needed
            } else {
                needed + self.release * (self.gain - needed)
            };
            let gain = self.gain.min(needed) as f32;
            *l *= gain;
            *r *= gain;
        }
    }

    fn reset(&mut self) {
        self.gain = 1.0;
    }
}

/// Soft-clipping waveshaper `tanh(drive·x)` normalised by `tanh(drive)`.
pub struct Distortion {
    drive: f32,
    mix: f32,
}

impl Effect for Distortion {
    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let norm = 1.0 / self.drive.tanh();
        for sample in left.iter_mut().chain(right.iter_mut()) {
            let wet = (*sample * self.drive).tanh() * norm;
            *sample = *sample * (1.0 - self.mix) + wet * self.mix;
        }
    }

    fn reset(&mut self) {}
}

/// Two-voice modulated delay chorus (LFOs in quadrature between the sides).
pub struct Chorus {
    left: Vec<f32>,
    right: Vec<f32>,
    write: usize,
    phase: f64,
    increment: f64,
    base: f64,
    depth: f64,
    mix: f32,
}

impl Chorus {
    pub fn new(rate_hz: f64, depth: f64, mix: f32, rate: f64) -> Self {
        let size = (rate * 0.05) as usize + 4;
        Self {
            left: vec![0.0; size],
            right: vec![0.0; size],
            write: 0,
            phase: 0.0,
            increment: rate_hz.clamp(0.01, 10.0) / rate,
            base: rate * 0.015,
            depth: rate * 0.008 * depth.clamp(0.0, 1.0),
            mix: mix.clamp(0.0, 1.0),
        }
    }

    #[inline]
    fn tap(buffer: &[f32], write: usize, delay: f64) -> f32 {
        let size = buffer.len();
        let position = write as f64 + size as f64 - delay;
        let index = position.floor();
        let frac = (position - index) as f32;
        let i = index as usize % size;
        let j = (i + 1) % size;
        buffer[i] * (1.0 - frac) + buffer[j] * frac
    }
}

impl Effect for Chorus {
    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let tau = std::f64::consts::TAU;
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            self.left[self.write] = *l;
            self.right[self.write] = *r;
            let dl = self.base + self.depth * (tau * self.phase).sin();
            let dr = self.base + self.depth * (tau * self.phase).cos();
            let wl = Self::tap(&self.left, self.write, dl);
            let wr = Self::tap(&self.right, self.write, dr);
            *l = *l * (1.0 - self.mix * 0.5) + wl * self.mix * 0.5;
            *r = *r * (1.0 - self.mix * 0.5) + wr * self.mix * 0.5;
            self.write = (self.write + 1) % self.left.len();
            self.phase = (self.phase + self.increment) % 1.0;
        }
    }

    fn reset(&mut self) {
        self.left.fill(0.0);
        self.right.fill(0.0);
    }
}

/// Noise gate: closes below the threshold with an exponential release.
pub struct Gate {
    threshold: f64,
    release: f64,
    gain: f64,
}

impl Gate {
    pub fn new(threshold_db: f64, release_ms: f64, rate: f64) -> Self {
        Self {
            threshold: db_to_gain(threshold_db),
            release: coefficient(release_ms.max(1.0), rate),
            gain: 0.0,
        }
    }
}

impl Effect for Gate {
    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let peak = f64::from(l.abs().max(r.abs()));
            self.gain = if peak >= self.threshold {
                1.0
            } else {
                self.gain * self.release
            };
            *l *= self.gain as f32;
            *r *= self.gain as f32;
        }
    }

    fn reset(&mut self) {
        self.gain = 0.0;
    }
}

/// A stereo mixer matrix (out L = L x left + R x r_to_l; out R = R x right + L x l_to_r; mid/side
/// processing is the textbook use) and a delay of one channel (a Haas-style pseudo-stereo).
pub struct StereoShaper {
    /// [r_to_l, left, right, l_to_r].
    matrix: [f32; 4],
    delay_right: bool,
    line: Vec<f32>,
    frames: usize,
    pos: usize,
    pre: bool,
}

impl StereoShaper {
    #[inline]
    fn delay(&mut self, l: &mut f32, r: &mut f32) {
        if self.frames == 0 {
            return;
        }
        let side = if self.delay_right { r } else { l };
        let out = self.line[self.pos];
        self.line[self.pos] = *side;
        self.pos = (self.pos + 1) % self.frames;
        *side = out;
    }
}

impl Effect for StereoShaper {
    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let [r_to_l, gl, gr, l_to_r] = self.matrix;
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            if self.pre {
                self.delay(l, r);
            }
            let (x, y) = (*l, *r);
            *l = x * gl + y * r_to_l;
            *r = y * gr + x * l_to_r;
            if !self.pre {
                self.delay(l, r);
            }
        }
    }

    fn reset(&mut self) {
        self.line.iter_mut().for_each(|v| *v = 0.0);
        self.pos = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stereo_shaper_matrix_and_delay() {
        let shaper = |r_to_l: i16, left: i16, right: i16, l_to_r: i16, delay_ms: i16, pre: bool| {
            build(
                &FxKind::StereoShaper {
                    r_to_l,
                    left,
                    right,
                    l_to_r,
                    delay_ms,
                    pre,
                },
                48_000,
                120.0,
            )
        };
        // Side only (L - R into the left, R - L into the right): a mono signal cancels.
        let mut fx = shaper(-1000, 1000, 1000, -1000, 0, false);
        let (mut l, mut r) = (vec![0.5_f32; 64], vec![0.5_f32; 64]);
        fx.process(&mut l, &mut r);
        assert!(l.iter().chain(&r).all(|v| v.abs() < 1e-7));
        // Swap: only the cross gains.
        let mut fx = shaper(1000, 0, 0, 1000, 0, false);
        let (mut l, mut r) = (vec![1.0_f32; 4], vec![0.25_f32; 4]);
        fx.process(&mut l, &mut r);
        assert_eq!((l[0], r[0]), (0.25, 1.0));
        // +10 ms delays the right channel by 480 frames.
        let mut fx = shaper(0, 1000, 1000, 0, 10, false);
        let mut l = vec![0.0_f32; 1000];
        let mut r = vec![0.0_f32; 1000];
        l[0] = 1.0;
        r[0] = 1.0;
        fx.process(&mut l, &mut r);
        assert_eq!(l[0], 1.0);
        assert_eq!(r[0], 0.0);
        assert_eq!(r[480], 1.0);
        // Pre: the delay happens before the matrix, so the swapped left carries the delay.
        let mut fx = shaper(1000, 0, 0, 1000, 10, true);
        let mut l = vec![0.0_f32; 1000];
        let mut r = vec![0.0_f32; 1000];
        l[0] = 1.0;
        r[0] = 1.0;
        fx.process(&mut l, &mut r);
        assert_eq!((l[0], l[480], r[0]), (0.0, 1.0, 1.0));
    }

    fn sine(n: usize, amp: f32) -> Vec<f32> {
        (0..n).map(|i| amp * (i as f32 * 0.05).sin()).collect()
    }

    #[test]
    fn delay_repeats_an_impulse_after_its_time() {
        let mut delay = Delay::new(0.01, 0.5, 1.0, false, 48_000.0);
        let mut l = vec![0.0; 2000];
        let mut r = vec![0.0; 2000];
        l[0] = 1.0;
        delay.process(&mut l, &mut r);
        assert!((l[480] - 1.0).abs() < 1e-6);
        assert!((l[960] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn limiter_never_exceeds_its_ceiling() {
        let mut limiter = Limiter::new(-1.0, 50.0, 48_000.0);
        let mut l = sine(48_000, 2.0);
        let mut r = l.clone();
        limiter.process(&mut l, &mut r);
        let ceiling = db_to_gain(-1.0) as f32 + 1e-6;
        assert!(l.iter().all(|v| v.abs() <= ceiling));
    }

    #[test]
    fn compressor_static_curve_follows_ratio() {
        let comp = Compressor::new(-20.0, 4.0, 1.0, 100.0, 0.0, 48_000.0);
        assert_eq!(comp.reduction_db(-30.0), 0.0);
        assert!((comp.reduction_db(0.0) - (-15.0)).abs() < 1e-9);
    }

    #[test]
    fn reverb_produces_a_decaying_tail() {
        let mut reverb = Reverb::new(0.8, 0.5, 1.0, 1.0, 48_000.0);
        let mut l = vec![0.0; 48_000];
        let mut r = vec![0.0; 48_000];
        l[0] = 1.0;
        reverb.process(&mut l, &mut r);
        let early: f32 = l[2_000..6_000].iter().map(|v| v * v).sum();
        let late: f32 = l[40_000..44_000].iter().map(|v| v * v).sum();
        assert!(early > 0.0 && late < early);
        assert!(l.iter().all(|v| v.is_finite()));
    }

    fn energy(data: &[f32]) -> f32 {
        data.iter().map(|v| v * v).sum()
    }

    #[test]
    fn filter_effect_cuts_highs_in_low_pass_mode() {
        let mut fx = build(
            &FxKind::Filter {
                mode: 0,
                cutoff_hz: 300,
                resonance: 0,
            },
            48_000,
            120.0,
        );
        // 5 kHz tone.
        let mut l: Vec<f32> = (0..4800)
            .map(|i| (i as f32 * std::f32::consts::TAU * 5000.0 / 48_000.0).sin())
            .collect();
        let before = energy(&l);
        let mut r = l.clone();
        fx.process(&mut l, &mut r);
        assert!(energy(&l[480..]) < before * 0.01);
    }

    #[test]
    fn flanger_and_phaser_change_the_signal_and_stay_finite() {
        for name in ["flanger", "phaser"] {
            let kind = FxKind::default_for(name).unwrap();
            let mut fx = build(&kind, 48_000, 120.0);
            let dry = sine(48_000, 0.5);
            let mut l = dry.clone();
            let mut r = dry.clone();
            fx.process(&mut l, &mut r);
            assert!(l.iter().all(|v| v.is_finite()), "{name}");
            let diff: f32 = l.iter().zip(&dry).map(|(a, b)| (a - b).abs()).sum();
            assert!(diff > 1.0, "{name} must alter the signal");
        }
    }

    #[test]
    fn gate_silences_signal_below_threshold() {
        let mut gate = Gate::new(-20.0, 5.0, 48_000.0);
        let mut l = sine(4_800, 0.01);
        let mut r = l.clone();
        gate.process(&mut l, &mut r);
        assert!(l[4_000..].iter().all(|v| v.abs() < 1e-4));
    }
}
