//! Single-sample Sampler voice [Manual: chansettings_sampler].
//!
//! Without a volume envelope a sample "will continue to play after note release"
//! [Manual: chansettings_sampler, "ADSR & Filtering"], so note-off does not stop this
//! voice; only a cut (cut groups, stop-all) does, with the default "Out only" declick
//! of 10 ms cosine S-shaped fade [Manual: chansettings_sampler, "Declicking"].
//! Loops repeat "while a note is held" and the volume envelope, when enabled, releases on
//! note-off [Manual: chansettings_sampler, "Playback"].

use crate::env::Adsr;
use crate::glide::Glide;
use crate::sample::SampleData;
use std::sync::Arc;

pub const DECLICK_OUT_MS: f64 = 10.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LoopMode {
    #[default]
    Off,
    Forward,
    /// "The play-position bounces back and forth between the loop start and loop end points".
    PingPong,
}

/// Per-note playback options derived from the channel's Sampler settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SamplerOptions {
    /// Start offset as a fraction of the sample.
    pub start: f64,
    pub loop_mode: LoopMode,
    /// Loop region as fractions of the sample.
    pub loop_start: f64,
    pub loop_end: f64,
    /// Play the sample backwards ("Reverse" precomputed tool).
    pub reverse: bool,
    /// Volume envelope (attack ms, decay ms, sustain 0..1, release ms) when enabled.
    pub envelope: Option<(f32, f32, f32, f32)>,
    /// Extra start position in seconds of the sample (Audio Clip offsets).
    pub start_seconds: f64,
    /// Audio Clip instance gain and fades.
    pub clip: Option<ClipEnvelope>,
}

/// Audio Clip volume envelope [Manual: playlist_audioclip, "Volume Envelopes & Controls"]: gain,
/// linear fade-in from the clip start and fade-out ending at the clip end (hypothesis H-PL-2:
/// the default fade curve is not described; ours is linear in amplitude).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClipEnvelope {
    pub gain: f32,
    pub fade_in: u64,
    pub fade_out: u64,
    /// Clip length in output frames.
    pub length: u64,
    /// Frames of the clip already elapsed at the voice start.
    pub position: u64,
}

impl ClipEnvelope {
    #[inline]
    fn amp(&self) -> f32 {
        let mut amp = self.gain;
        if self.fade_in > 0 && self.position < self.fade_in {
            amp *= self.position as f32 / self.fade_in as f32;
        }
        let left = self.length.saturating_sub(self.position);
        if self.fade_out > 0 && left < self.fade_out {
            amp *= left as f32 / self.fade_out as f32;
        }
        amp
    }
}

impl Default for SamplerOptions {
    fn default() -> Self {
        Self {
            start: 0.0,
            loop_mode: LoopMode::Off,
            loop_start: 0.0,
            loop_end: 1.0,
            reverse: false,
            envelope: None,
            start_seconds: 0.0,
            clip: None,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct SamplerVoice {
    sample: Option<Arc<SampleData>>,
    /// Logical position (0 = first frame played; reversed samples map it backwards).
    position: f64,
    increment: f64,
    direction: f64,
    fade_total: u32,
    fade_left: u32,
    cutting: bool,
    released: bool,
    options: SamplerOptions,
    env: Adsr,
    pub glide: Glide,
}

impl SamplerVoice {
    /// Start playing `sample` with a pitch ratio (1.0 = recorded pitch) at output `rate`.
    pub fn start(&mut self, sample: Arc<SampleData>, ratio: f64, rate: u32) {
        self.start_with(sample, ratio, rate, SamplerOptions::default());
    }

    pub fn start_with(
        &mut self,
        sample: Arc<SampleData>,
        ratio: f64,
        rate: u32,
        options: SamplerOptions,
    ) {
        let frames = sample.frames() as f64;
        self.increment = ratio * f64::from(sample.rate) / f64::from(rate.max(1));
        self.position = (options.start.clamp(0.0, 1.0) * frames).floor()
            + (options.start_seconds.max(0.0) * f64::from(sample.rate)).floor();
        self.direction = 1.0;
        self.fade_total = ((DECLICK_OUT_MS * 0.001 * f64::from(rate)) as u32).max(1);
        self.fade_left = 0;
        self.cutting = false;
        self.released = false;
        self.options = options;
        self.glide = Glide::default();
        if let Some((a, d, s, r)) = options.envelope {
            self.env.configure(a, d, s, r, rate as f32);
            self.env.trigger();
        }
        self.sample = Some(sample);
    }

    /// Note-off: loops stop repeating and the volume envelope (if any) releases.
    pub fn release(&mut self) {
        self.released = true;
        if self.options.envelope.is_some() {
            self.env.release();
        }
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
        let frames = sample.frames() as f64;
        let end = frames;
        let loop_start = (self.options.loop_start.clamp(0.0, 1.0) * frames).floor();
        let loop_end = (self.options.loop_end.clamp(0.0, 1.0) * frames)
            .floor()
            .max(loop_start + 1.0)
            .min(frames);
        let looping = self.options.loop_mode != LoopMode::Off && loop_end - loop_start >= 2.0;
        let mut alive = true;
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            if looping && !self.released {
                match self.options.loop_mode {
                    LoopMode::Forward if self.position >= loop_end => {
                        self.position = loop_start + (self.position - loop_end);
                    }
                    LoopMode::PingPong if self.direction > 0.0 && self.position >= loop_end => {
                        self.position = loop_end - (self.position - loop_end) - 1.0;
                        self.direction = -1.0;
                    }
                    LoopMode::PingPong if self.direction < 0.0 && self.position <= loop_start => {
                        self.position = loop_start + (loop_start - self.position);
                        self.direction = 1.0;
                    }
                    _ => {}
                }
            }
            if self.position >= end || self.position < 0.0 {
                alive = false;
                break;
            }
            let mut amp = 1.0_f32;
            if self.cutting {
                if self.fade_left == 0 {
                    alive = false;
                    break;
                }
                // Cosine S-curve from 1 to 0.
                let t = self.fade_left as f32 / self.fade_total as f32;
                amp = 0.5 - 0.5 * (std::f32::consts::PI * t).cos();
                self.fade_left -= 1;
            }
            if let Some(clip) = &mut self.options.clip {
                amp *= clip.amp();
                clip.position += 1;
            }
            if self.options.envelope.is_some() {
                amp *= self.env.tick();
                if !self.env.is_active() {
                    alive = false;
                    break;
                }
            }
            let physical = if self.options.reverse {
                frames - 1.0 - self.position
            } else {
                self.position
            };
            let (sl, sr) = sample.read(physical);
            *l += sl * gain_l * amp;
            *r += sr * gain_r * amp;
            self.position += self.increment * self.glide.tick() * self.direction;
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

    fn ramp(frames: usize) -> Arc<SampleData> {
        Arc::new(SampleData::mono(
            48_000,
            (0..frames).map(|i| i as f32 / frames as f32).collect(),
        ))
    }

    fn render(voice: &mut SamplerVoice, frames: usize) -> Vec<f32> {
        let mut l = vec![0.0; frames];
        let mut r = vec![0.0; frames];
        voice.render(&mut l, &mut r, 1.0, 1.0);
        l
    }

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
        let l = render(&mut voice, 1000);
        let sounding = l.iter().filter(|v| v.abs() > 0.0).count();
        assert!((499..=501).contains(&sounding), "{sounding}");
    }

    #[test]
    fn cut_fades_out_within_ten_ms() {
        let data = Arc::new(SampleData::mono(48_000, vec![1.0; 48_000]));
        let mut voice = SamplerVoice::default();
        voice.start(data, 1.0, 48_000);
        voice.cut();
        let l = render(&mut voice, 1000);
        assert!(!voice.is_active());
        assert_eq!(l[480], 0.0);
        assert!(l[0] > 0.99 && l[240] < 0.6);
    }

    #[test]
    fn start_offset_skips_the_beginning() {
        let mut voice = SamplerVoice::default();
        voice.start_with(
            ramp(1000),
            1.0,
            48_000,
            SamplerOptions {
                start: 0.5,
                ..SamplerOptions::default()
            },
        );
        let l = render(&mut voice, 1000);
        assert!((l[0] - 0.5).abs() < 1e-3);
        assert_eq!(l.iter().filter(|v| **v != 0.0).count(), 500);
    }

    #[test]
    fn forward_loop_repeats_while_held_and_finishes_after_release() {
        let mut voice = SamplerVoice::default();
        voice.start_with(
            ramp(1000),
            1.0,
            48_000,
            SamplerOptions {
                loop_mode: LoopMode::Forward,
                loop_start: 0.5,
                loop_end: 0.6,
                ..SamplerOptions::default()
            },
        );
        let l = render(&mut voice, 5000);
        assert!(voice.is_active(), "still looping while held");
        assert!(
            l[4999] >= 0.5 && l[4999] < 0.6,
            "inside the loop: {}",
            l[4999]
        );
        voice.release();
        render(&mut voice, 2000);
        assert!(!voice.is_active(), "plays out to the end after release");
    }

    #[test]
    fn ping_pong_reverses_direction_at_the_loop_end() {
        let mut voice = SamplerVoice::default();
        voice.start_with(
            ramp(1000),
            1.0,
            48_000,
            SamplerOptions {
                loop_mode: LoopMode::PingPong,
                loop_start: 0.5,
                loop_end: 0.6,
                ..SamplerOptions::default()
            },
        );
        let l = render(&mut voice, 700);
        // Forward to frame 600, then back down.
        assert!(l[640] < l[600], "{} vs {}", l[640], l[600]);
    }

    #[test]
    fn reverse_starts_from_the_end() {
        let mut voice = SamplerVoice::default();
        voice.start_with(
            ramp(1000),
            1.0,
            48_000,
            SamplerOptions {
                reverse: true,
                ..SamplerOptions::default()
            },
        );
        let l = render(&mut voice, 1000);
        assert!(l[0] > 0.99 && l[900] < 0.11);
    }

    #[test]
    fn envelope_release_ends_the_voice() {
        let data = Arc::new(SampleData::mono(48_000, vec![1.0; 48_000]));
        let mut voice = SamplerVoice::default();
        voice.start_with(
            data,
            1.0,
            48_000,
            SamplerOptions {
                envelope: Some((0.0, 0.0, 1.0, 10.0)),
                ..SamplerOptions::default()
            },
        );
        render(&mut voice, 100);
        voice.release();
        render(&mut voice, 1000);
        assert!(!voice.is_active(), "the envelope's release ends the note");
    }
}
