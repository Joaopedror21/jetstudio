//! Filters: a trapezoidal-integrated state-variable filter and RBJ biquads.
//!
//! References (public): A. Simper, "Linear Trapezoidal Integrated SVF" (Cytomic, 2013);
//! R. Bristow-Johnson, "Cookbook formulae for audio EQ biquad filter coefficients".

use std::f64::consts::PI;

/// Zero-delay-feedback state-variable filter, low-pass output.
#[derive(Clone, Copy, Debug, Default)]
pub struct Svf {
    ic1: f32,
    ic2: f32,
    a1: f32,
    a2: f32,
    a3: f32,
    k: f32,
}

impl Svf {
    /// `resonance` in 0..=1 maps to Q from 0.5 to about 20.
    pub fn set(&mut self, cutoff_hz: f32, resonance: f32, rate: f32) {
        let cutoff = cutoff_hz.clamp(10.0, rate * 0.49);
        let q = 0.5 + resonance.clamp(0.0, 1.0).powi(2) * 19.5;
        let g = (std::f32::consts::PI * cutoff / rate).tan();
        self.k = 1.0 / q;
        self.a1 = 1.0 / (1.0 + g * (g + self.k));
        self.a2 = g * self.a1;
        self.a3 = g * self.a2;
    }

    pub fn reset(&mut self) {
        self.ic1 = 0.0;
        self.ic2 = 0.0;
    }

    #[inline]
    pub fn lowpass(&mut self, v0: f32) -> f32 {
        let v3 = v0 - self.ic2;
        let v1 = self.a1 * self.ic1 + self.a2 * v3;
        let v2 = self.ic2 + self.a2 * self.ic1 + self.a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        v2
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BiquadKind {
    LowPass,
    HighPass,
    BandPass,
    Peak,
    LowShelf,
    HighShelf,
    Notch,
}

/// Direct form I biquad with RBJ cookbook coefficients.
#[derive(Clone, Copy, Debug, Default)]
pub struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    x1: f64,
    x2: f64,
    y1: f64,
    y2: f64,
}

impl Biquad {
    pub fn new(kind: BiquadKind, freq: f64, q: f64, gain_db: f64, rate: f64) -> Self {
        let mut filter = Self::default();
        filter.set(kind, freq, q, gain_db, rate);
        filter
    }

    pub fn set(&mut self, kind: BiquadKind, freq: f64, q: f64, gain_db: f64, rate: f64) {
        let f = freq.clamp(1.0, rate * 0.49);
        let q = q.max(0.05);
        let a = 10f64.powf(gain_db / 40.0);
        let w0 = 2.0 * PI * f / rate;
        let (sin, cos) = w0.sin_cos();
        let alpha = sin / (2.0 * q);
        let (b0, b1, b2, a0, a1, a2) = match kind {
            BiquadKind::LowPass => {
                let b1 = 1.0 - cos;
                (b1 / 2.0, b1, b1 / 2.0, 1.0 + alpha, -2.0 * cos, 1.0 - alpha)
            }
            BiquadKind::HighPass => {
                let b1 = -(1.0 + cos);
                (
                    -b1 / 2.0,
                    b1,
                    -b1 / 2.0,
                    1.0 + alpha,
                    -2.0 * cos,
                    1.0 - alpha,
                )
            }
            BiquadKind::BandPass => (alpha, 0.0, -alpha, 1.0 + alpha, -2.0 * cos, 1.0 - alpha),
            BiquadKind::Notch => (1.0, -2.0 * cos, 1.0, 1.0 + alpha, -2.0 * cos, 1.0 - alpha),
            BiquadKind::Peak => (
                1.0 + alpha * a,
                -2.0 * cos,
                1.0 - alpha * a,
                1.0 + alpha / a,
                -2.0 * cos,
                1.0 - alpha / a,
            ),
            BiquadKind::LowShelf => {
                let s = 2.0 * a.sqrt() * alpha;
                (
                    a * ((a + 1.0) - (a - 1.0) * cos + s),
                    2.0 * a * ((a - 1.0) - (a + 1.0) * cos),
                    a * ((a + 1.0) - (a - 1.0) * cos - s),
                    (a + 1.0) + (a - 1.0) * cos + s,
                    -2.0 * ((a - 1.0) + (a + 1.0) * cos),
                    (a + 1.0) + (a - 1.0) * cos - s,
                )
            }
            BiquadKind::HighShelf => {
                let s = 2.0 * a.sqrt() * alpha;
                (
                    a * ((a + 1.0) + (a - 1.0) * cos + s),
                    -2.0 * a * ((a - 1.0) + (a + 1.0) * cos),
                    a * ((a + 1.0) + (a - 1.0) * cos - s),
                    (a + 1.0) - (a - 1.0) * cos + s,
                    2.0 * ((a - 1.0) - (a + 1.0) * cos),
                    (a + 1.0) - (a - 1.0) * cos - s,
                )
            }
        };
        self.b0 = b0 / a0;
        self.b1 = b1 / a0;
        self.b2 = b2 / a0;
        self.a1 = a1 / a0;
        self.a2 = a2 / a0;
    }

    pub fn reset(&mut self) {
        self.x1 = 0.0;
        self.x2 = 0.0;
        self.y1 = 0.0;
        self.y2 = 0.0;
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let x = f64::from(x);
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2
            - self.a1 * self.y1
            - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        // Flush denormals so silent tails do not cost CPU.
        self.y1 = if y.abs() < 1e-30 { 0.0 } else { y };
        self.y2 = if self.y2.abs() < 1e-30 { 0.0 } else { self.y2 };
        y as f32
    }

    /// Magnitude response at `freq`, for tests and UI curves.
    pub fn magnitude(&self, freq: f64, rate: f64) -> f64 {
        let w = 2.0 * PI * freq / rate;
        let (s1, c1) = w.sin_cos();
        let (s2, c2) = (2.0 * w).sin_cos();
        let nr = self.b0 + self.b1 * c1 + self.b2 * c2;
        let ni = -(self.b1 * s1 + self.b2 * s2);
        let dr = 1.0 + self.a1 * c1 + self.a2 * c2;
        let di = -(self.a1 * s1 + self.a2 * s2);
        ((nr * nr + ni * ni) / (dr * dr + di * di)).sqrt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peak_filter_boosts_its_center_by_its_gain() {
        let filter = Biquad::new(BiquadKind::Peak, 1000.0, 1.0, 6.0, 48_000.0);
        let db = 20.0 * filter.magnitude(1000.0, 48_000.0).log10();
        assert!((db - 6.0).abs() < 0.01);
        let far = 20.0 * filter.magnitude(20.0, 48_000.0).log10();
        assert!(far.abs() < 0.1);
    }

    #[test]
    fn lowpass_passes_dc_and_cuts_highs() {
        let filter = Biquad::new(BiquadKind::LowPass, 1000.0, 0.707, 0.0, 48_000.0);
        assert!((filter.magnitude(1.0, 48_000.0) - 1.0).abs() < 1e-3);
        assert!(filter.magnitude(10_000.0, 48_000.0) < 0.02);
    }

    #[test]
    fn svf_lowpass_settles_to_dc_input() {
        let mut svf = Svf::default();
        svf.set(500.0, 0.0, 48_000.0);
        let mut y = 0.0;
        for _ in 0..48_000 {
            y = svf.lowpass(1.0);
        }
        assert!((y - 1.0).abs() < 1e-3);
    }
}
