//! Single-sample Sampler voice [Manual: chansettings_sampler].
//!
//! Without a volume envelope a sample "will continue to play after note release"
//! [Manual: chansettings_sampler, "ADSR & Filtering"], so note-off does not stop this
//! voice; only a cut (cut groups, stop-all) does, with the default "Out only" declick
//! of 10 ms cosine S-shaped fade [Manual: chansettings_sampler, "Declicking"].

use crate::sample::SampleData;
use std::sync::Arc;

pub const DECLICK_OUT_MS: f64 = 10.0;

#[derive(Clone, Debug, Default)]
pub struct SamplerVoice {
    sample: Option<Arc<SampleData>>,
    position: f64,
    increment: f64,
    fade_total: u32,
    fade_left: u32,
    cutting: bool,
}

impl SamplerVoice {
    /// Start playing `sample` with a pitch ratio (1.0 = recorded pitch) at output `rate`.
    pub fn start(&mut self, sample: Arc<SampleData>, ratio: f64, rate: u32) {
        self.increment = ratio * f64::from(sample.rate) / f64::from(rate.max(1));
        self.position = 0.0;
        self.fade_total = ((DECLICK_OUT_MS * 0.001 * f64::from(rate)) as u32).max(1);
        self.fade_left = 0;
        self.cutting = false;
        self.sample = Some(sample);
    }

    /// Begin the declick fade-out; the voice ends when it completes.
    pub fn cut(&mut self) {
        if self.sample.is_some() && !self.cutting {
            self.cutting = true;
            self.fade_left = self.fade_total;
        }
    }

    pub fn is_active(&self) -> bool {
        self.sample.is_some()
    }

    /// Mix into `left`/`right` with gains. Returns `false` once the voice has finished.
    pub fn render(
        &mut self,
        left: &mut [f32],
        right: &mut [f32],
        gain_l: f32,
        gain_r: f32,
    ) -> bool {
        let Some(sample) = self.sample.as_ref() else {
            return false;
        };
        let end = sample.frames() as f64;
        let mut alive = true;
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            if self.position >= end {
                alive = false;
                break;
            }
            let mut fade = 1.0_f32;
            if self.cutting {
                if self.fade_left == 0 {
                    alive = false;
                    break;
                }
                // Cosine S-curve from 1 to 0.
                let t = self.fade_left as f32 / self.fade_total as f32;
                fade = 0.5 - 0.5 * (std::f32::consts::PI * t).cos();
                self.fade_left -= 1;
            }
            let (sl, sr) = sample.read(self.position);
            *l += sl * gain_l * fade;
            *r += sr * gain_r * fade;
            self.position += self.increment;
        }
        if !alive {
            self.sample = None;
        }
        alive
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plays_the_whole_sample_once_at_recorded_pitch() {
        let data = Arc::new(SampleData::mono(48_000, vec![0.5; 100]));
        let mut voice = SamplerVoice::default();
        voice.start(data, 1.0, 48_000);
        let mut l = vec![0.0; 128];
        let mut r = vec![0.0; 128];
        assert!(!voice.render(&mut l, &mut r, 1.0, 1.0));
        assert!((l[50] - 0.5).abs() < 1e-6);
        assert_eq!(l[120], 0.0);
    }

    #[test]
    fn octave_up_halves_the_duration() {
        let data = Arc::new(SampleData::mono(48_000, vec![1.0; 1000]));
        let mut voice = SamplerVoice::default();
        voice.start(data, 2.0, 48_000);
        let mut l = vec![0.0; 1000];
        let mut r = vec![0.0; 1000];
        voice.render(&mut l, &mut r, 1.0, 1.0);
        let sounding = l.iter().filter(|v| v.abs() > 0.0).count();
        assert!((499..=501).contains(&sounding), "{sounding}");
    }

    #[test]
    fn cut_fades_out_within_ten_ms() {
        let data = Arc::new(SampleData::mono(48_000, vec![1.0; 48_000]));
        let mut voice = SamplerVoice::default();
        voice.start(data, 1.0, 48_000);
        voice.cut();
        let mut l = vec![0.0; 1000];
        let mut r = vec![0.0; 1000];
        voice.render(&mut l, &mut r, 1.0, 1.0);
        assert!(!voice.is_active());
        assert_eq!(l[480], 0.0);
        assert!(l[0] > 0.99 && l[240] < 0.6);
    }
}
