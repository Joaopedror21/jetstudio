//! Pitch glide used by portamento, mono legato and slide notes [Manual: chansettings_misc,
//! "Porta", "Mono"; pianoroll, "How slide notes work"]: a pitch offset in cents that moves
//! linearly (in cents, i.e. exponentially in frequency) to a target over a number of frames.

use crate::cents_ratio;

#[derive(Clone, Copy, Debug)]
pub struct Glide {
    cents: f64,
    target: f64,
    step: f64,
    left: u32,
    ratio: f64,
}

impl Default for Glide {
    fn default() -> Self {
        Self {
            cents: 0.0,
            target: 0.0,
            step: 0.0,
            left: 0,
            ratio: 1.0,
        }
    }
}

impl Glide {
    /// Move from `from` to `to` cents over `frames` (zero jumps straight to `to`).
    pub fn set(&mut self, from: f64, to: f64, frames: u32) {
        if frames == 0 {
            self.cents = to;
            self.left = 0;
        } else {
            self.cents = from;
            self.step = (to - from) / f64::from(frames);
            self.left = frames;
        }
        self.target = to;
        self.ratio = cents_ratio(self.cents);
    }

    /// Glide from wherever the pitch is now.
    pub fn to(&mut self, to: f64, frames: u32) {
        self.set(self.cents, to, frames);
    }

    pub fn current(&self) -> f64 {
        self.cents
    }

    /// Where the pitch is heading (the offset that remains once the glide ends).
    pub fn target(&self) -> f64 {
        self.target
    }

    /// Advance one frame and return the frequency ratio for it.
    #[inline]
    pub fn tick(&mut self) -> f64 {
        if self.left > 0 {
            let ratio = self.ratio;
            self.left -= 1;
            self.cents = if self.left == 0 {
                self.target
            } else {
                self.cents + self.step
            };
            self.ratio = cents_ratio(self.cents);
            return ratio;
        }
        self.ratio
    }
}

#[cfg(test)]
mod tests {
    use super::Glide;

    #[test]
    fn glides_linearly_in_cents_and_holds_the_target() {
        let mut glide = Glide::default();
        assert_eq!(glide.tick(), 1.0);
        glide.set(-1200.0, 0.0, 4);
        let ratios: Vec<f64> = (0..6).map(|_| glide.tick()).collect();
        assert!((ratios[0] - 0.5).abs() < 1e-12, "starts an octave down");
        assert!(
            (ratios[2] - 2f64.powf(-0.5)).abs() < 1e-12,
            "half way in cents"
        );
        assert_eq!(ratios[4], 1.0);
        assert_eq!(ratios[5], 1.0);
        glide.to(700.0, 0);
        assert_eq!(glide.current(), 700.0);
        assert_eq!(glide.target(), 700.0);
    }
}
