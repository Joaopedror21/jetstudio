//! Linear-segment ADSR envelope evaluated per sample.

#[derive(Clone, Copy, Debug, PartialEq)]
enum Stage {
    Idle,
    Attack,
    Decay,
    Sustain,
    Release,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Adsr {
    stage: Stage,
    level: f32,
    attack_step: f32,
    decay_step: f32,
    sustain: f32,
    release_step: f32,
}

impl Default for Adsr {
    fn default() -> Self {
        Self {
            stage: Stage::Idle,
            level: 0.0,
            attack_step: 1.0,
            decay_step: 1.0,
            sustain: 1.0,
            release_step: 1.0,
        }
    }
}

fn step_for(ms: f32, rate: f32) -> f32 {
    let samples = (ms * 0.001 * rate).max(1.0);
    1.0 / samples
}

impl Adsr {
    pub fn configure(
        &mut self,
        attack_ms: f32,
        decay_ms: f32,
        sustain: f32,
        release_ms: f32,
        rate: f32,
    ) {
        self.attack_step = step_for(attack_ms, rate);
        self.decay_step = step_for(decay_ms, rate);
        self.sustain = sustain.clamp(0.0, 1.0);
        self.release_step = step_for(release_ms, rate);
    }

    pub fn trigger(&mut self) {
        self.stage = Stage::Attack;
    }

    pub fn release(&mut self) {
        if self.stage != Stage::Idle {
            self.stage = Stage::Release;
        }
    }

    pub fn is_active(&self) -> bool {
        self.stage != Stage::Idle
    }

    pub fn is_releasing(&self) -> bool {
        self.stage == Stage::Release
    }

    #[inline]
    pub fn tick(&mut self) -> f32 {
        match self.stage {
            Stage::Idle => {}
            Stage::Attack => {
                self.level += self.attack_step;
                if self.level >= 1.0 {
                    self.level = 1.0;
                    self.stage = Stage::Decay;
                }
            }
            Stage::Decay => {
                self.level -= self.decay_step * (1.0 - self.sustain).max(1e-6);
                if self.level <= self.sustain {
                    self.level = self.sustain;
                    self.stage = Stage::Sustain;
                }
            }
            Stage::Sustain => self.level = self.sustain,
            Stage::Release => {
                self.level -= self.release_step;
                if self.level <= 0.0 {
                    self.level = 0.0;
                    self.stage = Stage::Idle;
                }
            }
        }
        self.level
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_reaches_sustain_and_ends_after_release() {
        let mut env = Adsr::default();
        env.configure(1.0, 1.0, 0.5, 1.0, 1000.0);
        env.trigger();
        let peak = (0..3).map(|_| env.tick()).fold(0.0_f32, f32::max);
        assert_eq!(peak, 1.0);
        for _ in 0..5 {
            env.tick();
        }
        assert_eq!(env.tick(), 0.5);
        env.release();
        for _ in 0..5 {
            env.tick();
        }
        assert!(!env.is_active());
    }
}
