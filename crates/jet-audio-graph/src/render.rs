//! The real-time renderer. `process` never allocates, locks or does I/O.

use crate::plan::{Instrument, RenderPlan, TRACK_COUNT};
use crate::{MAX_BLOCK, Meters};
use jet_dsp::sampler::SamplerVoice;
use jet_dsp::synth::SynthVoice;
use jet_dsp::{cents_ratio, key_hz, pan_gains, velocity_gain};
use jet_model::ChannelId;
use jet_seq::NoteEvent;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

pub const MAX_VOICES: usize = 256;

/// Transport state mirrored from the engine. Positions follow the same integer formula
/// as `jet_engine::Transport::snapshot`, so the UI clock and the audio agree exactly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransportSync {
    pub playing: bool,
    pub anchor_tick: i64,
    pub anchor_frame: u64,
}

#[derive(Clone, Debug, Default)]
enum VoiceKind {
    #[default]
    Idle,
    Sampler(SamplerVoice),
    Synth(SynthVoice),
}

#[derive(Clone, Debug, Default)]
struct Voice {
    kind: VoiceKind,
    channel: ChannelId,
    key: u8,
    /// Started by a live note-on rather than the sequence.
    live: bool,
    pan: i16,
    velocity: f32,
    /// Absolute frame where the voice starts sounding.
    start_frame: u64,
    /// Absolute frame of note-off.
    release_frame: u64,
    released: bool,
    cut_by: u8,
    age: u64,
}

impl Voice {
    fn active(&self) -> bool {
        !matches!(self.kind, VoiceKind::Idle)
    }
}

pub struct Renderer {
    plan: Box<RenderPlan>,
    voices: Vec<Voice>,
    transport: TransportSync,
    /// First tick not yet scheduled.
    next_tick: i64,
    frame: u64,
    age: u64,
    frame_counter: Arc<AtomicU64>,
    meters: Arc<Meters>,
    seed: u32,
    stop_events_after: Option<i64>,
    preview: SamplerVoice,
}

impl Renderer {
    pub fn new(plan: Box<RenderPlan>, frame_counter: Arc<AtomicU64>, meters: Arc<Meters>) -> Self {
        Self {
            plan,
            voices: vec![Voice::default(); MAX_VOICES],
            transport: TransportSync {
                playing: false,
                anchor_tick: 0,
                anchor_frame: 0,
            },
            next_tick: 0,
            frame: frame_counter.load(Ordering::Acquire),
            age: 0,
            frame_counter,
            meters,
            seed: 0x9E37_79B9,
            stop_events_after: None,
            preview: SamplerVoice::default(),
        }
    }

    pub fn frame(&self) -> u64 {
        self.frame
    }

    pub fn plan(&self) -> &RenderPlan {
        &self.plan
    }

    /// Install a new plan and return the previous one so the caller can drop it off-thread.
    /// Effect instances whose slot description did not change keep their state (tails).
    pub fn swap_plan(&mut self, mut next: Box<RenderPlan>) -> Box<RenderPlan> {
        for track in &mut next.tracks {
            let Some(old) = self.plan.tracks.iter_mut().find(|t| t.index == track.index) else {
                continue;
            };
            for (slot, instance) in track.fx.iter_mut().enumerate() {
                if let Some(previous) = old.fx.get_mut(slot)
                    && previous.kind == instance.kind
                {
                    std::mem::swap(&mut previous.effect, &mut instance.effect);
                }
            }
        }
        std::mem::replace(&mut self.plan, next)
    }

    /// Apply a transport change. Notes are released on stop/pause.
    pub fn sync(&mut self, sync: TransportSync) {
        if sync == self.transport {
            return;
        }
        let was_playing = self.transport.playing;
        self.transport = sync;
        // A sync anchored in the past still fires the events since its anchor.
        self.next_tick = sync.anchor_tick;
        if !sync.playing && was_playing {
            self.release_all();
        }
    }

    /// Offline renders stop scheduling notes after this tick (no loop wrap).
    pub fn stop_events_after(&mut self, tick: Option<i64>) {
        self.stop_events_after = tick;
    }

    fn release_all(&mut self) {
        let frame = self.frame;
        for voice in &mut self.voices {
            if voice.active() && !voice.released {
                voice.release_frame = voice.release_frame.min(frame);
            }
        }
    }

    /// Stop every voice with its declick/short release ("panic").
    pub fn panic(&mut self) {
        let rate = self.plan.rate;
        for voice in &mut self.voices {
            match &mut voice.kind {
                VoiceKind::Sampler(v) => v.cut(),
                VoiceKind::Synth(v) => v.cut(rate),
                VoiceKind::Idle => {}
            }
            voice.released = true;
        }
    }

    /// Audition a sample straight to the Master at its recorded level (Browser preview).
    /// Hypothesis H-BR-1: the preview level is not documented; we use unity gain.
    pub fn preview(&mut self, sample: Arc<jet_dsp::SampleData>) {
        let rate = self.plan.rate;
        self.preview.start(sample, 1.0, rate);
    }

    pub fn stop_preview(&mut self) {
        self.preview.cut();
    }

    pub fn active_voices(&self) -> usize {
        self.voices.iter().filter(|v| v.active()).count()
    }

    /// k = ticks per frame as a rational: (tempo·ppq) / (rate·60000).
    fn tick_at(&self, frame: u64) -> i64 {
        let t = &self.transport;
        let frames = i128::from(frame.saturating_sub(t.anchor_frame));
        let numerator = frames * i128::from(self.plan.tempo_milli) * i128::from(self.plan.ppq);
        let denominator = i128::from(self.plan.rate.max(1)) * 60_000;
        t.anchor_tick
            .saturating_add((numerator / denominator) as i64)
    }

    /// First frame whose tick is >= `tick`.
    fn frame_of_tick(&self, tick: i64) -> u64 {
        let t = &self.transport;
        let delta = i128::from(tick - t.anchor_tick).max(0);
        let numerator = delta * i128::from(self.plan.rate.max(1)) * 60_000;
        let denominator = i128::from(self.plan.tempo_milli) * i128::from(self.plan.ppq.max(1));
        let frames = (numerator + denominator - 1) / denominator.max(1);
        t.anchor_frame.saturating_add(frames as u64)
    }

    fn ticks_to_frames(&self, ticks: i64) -> u64 {
        let numerator = i128::from(ticks.max(0)) * i128::from(self.plan.rate) * 60_000;
        let denominator = i128::from(self.plan.tempo_milli) * i128::from(self.plan.ppq.max(1));
        (numerator / denominator.max(1)) as u64
    }

    fn schedule(&mut self, from: i64, to: i64) {
        let length = self.plan.sequence.length.max(1);
        let limit = self.stop_events_after.unwrap_or(i64::MAX);
        let to = to.min(limit);
        if from >= to {
            return;
        }
        let mut base = if self.plan.looping {
            from.div_euclid(length) * length
        } else {
            0
        };
        while base < to {
            let local_from = (from - base).max(0);
            let local_to = to - base;
            let first = self.plan.sequence.first_at_or_after(local_from);
            let mut index = first;
            while let Some(event) = self.plan.sequence.events.get(index).copied() {
                if event.tick >= local_to || (self.plan.looping && event.tick >= length) {
                    break;
                }
                self.note_on(&event, base + event.tick);
                index += 1;
            }
            if !self.plan.looping {
                break;
            }
            base += length;
        }
    }

    fn note_on(&mut self, event: &NoteEvent, absolute_tick: i64) {
        let start_frame = self.frame_of_tick(absolute_tick).max(self.frame);
        let gate = self.plan.sequence.gate_for(event);
        let release_frame = start_frame + self.ticks_to_frames(gate).max(1);
        self.start_voice(event, start_frame, release_frame, false);
    }

    /// Play a note outside the sequence (preview keyboard, typing keyboard, MIDI input).
    /// It sounds until [`Renderer::live_note_off`].
    pub fn live_note_on(&mut self, channel: ChannelId, key: u8, velocity: u8) {
        let event = NoteEvent {
            tick: 0,
            length: 0,
            channel,
            key,
            velocity,
            pan: 0,
            fine_pitch: 0,
            release: 0,
            mod_x: 0,
            mod_y: 0,
            slide: false,
            porta: false,
        };
        let frame = self.frame;
        self.start_voice(&event, frame, u64::MAX, true);
    }

    pub fn live_note_off(&mut self, channel: ChannelId, key: u8) {
        let frame = self.frame;
        for voice in &mut self.voices {
            if voice.live && voice.channel == channel && voice.key == key && !voice.released {
                voice.release_frame = frame;
            }
        }
    }

    fn start_voice(&mut self, event: &NoteEvent, start_frame: u64, release_frame: u64, live: bool) {
        let Some(channel) = self.plan.channel(event.channel) else {
            return;
        };
        let rate = self.plan.rate;
        let kind = match &channel.instrument {
            Instrument::Sampler(None) => return,
            Instrument::Sampler(Some(sample)) => {
                let semis = f64::from(event.key) - f64::from(channel.root_key);
                let cents = f64::from(channel.pitch_cents) + f64::from(event.fine_pitch);
                let mut voice = SamplerVoice::default();
                voice.start(Arc::clone(sample), cents_ratio(semis * 100.0 + cents), rate);
                VoiceKind::Sampler(voice)
            }
            Instrument::Synth(params) => {
                let cents = f64::from(channel.pitch_cents) + f64::from(event.fine_pitch);
                let mut voice = SynthVoice::default();
                self.seed = self
                    .seed
                    .wrapping_mul(1_664_525)
                    .wrapping_add(1_013_904_223);
                voice.start(
                    params,
                    key_hz(f64::from(event.key) + cents / 100.0),
                    rate,
                    self.seed,
                );
                VoiceKind::Synth(voice)
            }
        };
        let cut = channel.cut;
        let cut_by = channel.cut_by;
        // Cut groups: this note silences channels whose "cut by" matches its "cut"
        // [Manual: chansettings_misc, "Cut Groups"].
        if cut != 0 {
            for voice in &mut self.voices {
                if voice.active() && voice.cut_by == cut && voice.start_frame <= start_frame {
                    match &mut voice.kind {
                        VoiceKind::Sampler(v) => v.cut(),
                        VoiceKind::Synth(v) => v.release(),
                        VoiceKind::Idle => {}
                    }
                    voice.released = true;
                }
            }
        }
        let slot = match self.voices.iter().position(|v| !v.active()) {
            Some(slot) => slot,
            None => {
                // Steal the oldest voice.
                let mut oldest = 0;
                for (i, voice) in self.voices.iter().enumerate() {
                    if voice.age < self.voices[oldest].age {
                        oldest = i;
                    }
                }
                oldest
            }
        };
        self.age += 1;
        self.voices[slot] = Voice {
            kind,
            channel: event.channel,
            key: event.key,
            live,
            pan: event.pan,
            velocity: velocity_gain(event.velocity) as f32,
            start_frame,
            release_frame,
            released: false,
            cut_by,
            age: self.age,
        };
    }

    /// Render `out` (interleaved stereo) and advance the clock.
    pub fn process(&mut self, out: &mut [f32]) {
        for chunk in out.chunks_mut(MAX_BLOCK * 2) {
            self.process_block(chunk);
        }
    }

    fn process_block(&mut self, out: &mut [f32]) {
        let frames = out.len() / 2;
        let block_start = self.frame;
        let block_end = block_start + frames as u64;
        for track in &mut self.plan.tracks {
            track.left[..frames].fill(0.0);
            track.right[..frames].fill(0.0);
        }
        if self.transport.playing && block_end > self.transport.anchor_frame {
            // Event at tick T sounds at frame_of_tick(T); it belongs to this block when that
            // frame is below block_end, i.e. when T <= tick_at(block_end - 1).
            let to = self.tick_at(block_end - 1).saturating_add(1);
            let from = self.next_tick;
            self.schedule(from, to);
            self.next_tick = to.max(from);
        }
        self.render_voices(block_start, frames);
        if self.preview.is_active() {
            let master = &mut self.plan.tracks[self.plan.master_slot];
            self.preview.render(
                &mut master.left[..frames],
                &mut master.right[..frames],
                1.0,
                1.0,
            );
        }
        self.mix(frames, out);
        self.meters.record_scope(&out[..frames * 2]);
        self.meters
            .set_voices(self.voices.iter().filter(|v| v.active()).count() as u32);
        self.frame = block_end;
        self.frame_counter.store(block_end, Ordering::Release);
    }

    fn render_voices(&mut self, block_start: u64, frames: usize) {
        let block_end = block_start + frames as u64;
        let pan_law = self.plan.pan_law;
        let plan = &mut self.plan;
        for voice in &mut self.voices {
            if !voice.active() || voice.start_frame >= block_end {
                continue;
            }
            let Some(&(_, index)) = plan
                .channel_index
                .binary_search_by_key(&voice.channel, |(id, _)| *id)
                .ok()
                .and_then(|i| plan.channel_index.get(i))
            else {
                voice.kind = VoiceKind::Idle; // channel deleted
                continue;
            };
            let channel = &plan.channels[index];
            let pan = (i32::from(channel.pan) + i32::from(voice.pan)).clamp(-1000, 1000) as i16;
            let (pl, pr) = pan_gains(pan, pan_law);
            let gain = channel.gain * voice.velocity;
            let (gl, gr) = (gain * pl as f32, gain * pr as f32);
            let track = &mut plan.tracks[channel.track_slot];
            let begin = voice.start_frame.saturating_sub(block_start) as usize;
            // Split at note-off so the release starts on its exact frame.
            let split = if !voice.released && voice.release_frame < block_end {
                voice
                    .release_frame
                    .saturating_sub(block_start)
                    .max(begin as u64) as usize
            } else {
                frames
            };
            let mut alive = true;
            for (range_start, range_end, release_after) in
                [(begin, split, split < frames), (split, frames, false)]
            {
                if range_start >= range_end || !alive {
                    if release_after {
                        release(voice);
                    }
                    continue;
                }
                let l = &mut track.left[range_start..range_end];
                let r = &mut track.right[range_start..range_end];
                alive = match &mut voice.kind {
                    VoiceKind::Sampler(v) => v.render(l, r, gl, gr),
                    VoiceKind::Synth(v) => v.render(l, r, gl, gr),
                    VoiceKind::Idle => false,
                };
                if release_after {
                    release(voice);
                }
            }
            if !alive {
                voice.kind = VoiceKind::Idle;
            }
        }

        fn release(voice: &mut Voice) {
            if voice.released {
                return;
            }
            voice.released = true;
            // Samplers without a volume envelope keep playing after note-off
            // [Manual: chansettings_sampler, "samples will continue to play after note release"].
            if let VoiceKind::Synth(v) = &mut voice.kind {
                v.release();
            }
        }
    }

    fn mix(&mut self, frames: usize, out: &mut [f32]) {
        let master_slot = self.plan.master_slot;
        let count = self.plan.tracks.len();
        for slot in 0..count {
            let (_, rest) = self.plan.tracks.split_at_mut(slot);
            let Some((track, after)) = rest.split_first_mut() else {
                continue;
            };
            process_track(track, frames);
            if track.mute {
                track.left[..frames].fill(0.0);
                track.right[..frames].fill(0.0);
            }
            let index = usize::from(track.index).min(TRACK_COUNT - 1);
            self.meters
                .record(index, &track.left[..frames], &track.right[..frames]);
            if track.mute {
                continue;
            }
            // Destinations always come later in processing order.
            if track.to_master && slot != master_slot {
                let offset = master_slot - slot - 1;
                if let Some(master) = after.get_mut(offset) {
                    add(&mut master.left[..frames], &track.left[..frames], 1.0);
                    add(&mut master.right[..frames], &track.right[..frames], 1.0);
                }
            }
            for &(destination, gain) in &track.sends {
                if destination > slot
                    && let Some(target) = after.get_mut(destination - slot - 1)
                {
                    add(&mut target.left[..frames], &track.left[..frames], gain);
                    add(&mut target.right[..frames], &track.right[..frames], gain);
                }
            }
        }
        let master = &self.plan.tracks[master_slot];
        for (i, frame) in out
            .as_chunks_mut::<2>()
            .0
            .iter_mut()
            .enumerate()
            .take(frames)
        {
            frame[0] = master.left[i];
            frame[1] = master.right[i];
        }
    }
}

fn add(dst: &mut [f32], src: &[f32], gain: f32) {
    for (d, s) in dst.iter_mut().zip(src) {
        *d += s * gain;
    }
}

/// FX chain, then the integrated phase/swap/separation/pan stages, then the fader
/// [Manual: mixer, "audio passes through any Effects ... and then through the integrated effects"].
fn process_track(track: &mut crate::plan::TrackPlan, frames: usize) {
    if track.fx_enabled {
        for instance in &mut track.fx {
            let Some(effect) = instance.effect.as_mut() else {
                continue;
            };
            if !instance.enabled {
                continue;
            }
            if instance.mix >= 0.999 {
                effect.process(&mut track.left[..frames], &mut track.right[..frames]);
            } else {
                track.scratch_left[..frames].copy_from_slice(&track.left[..frames]);
                track.scratch_right[..frames].copy_from_slice(&track.right[..frames]);
                effect.process(
                    &mut track.scratch_left[..frames],
                    &mut track.scratch_right[..frames],
                );
                let wet = instance.mix;
                for i in 0..frames {
                    track.left[i] += (track.scratch_left[i] - track.left[i]) * wet;
                    track.right[i] += (track.scratch_right[i] - track.right[i]) * wet;
                }
            }
        }
    }
    let p = f32::from(track.pan) / 1000.0;
    let sep = track.stereo_sep;
    for i in 0..frames {
        let (mut l, mut r) = (track.left[i], track.right[i]);
        if track.invert {
            l = -l;
            r = -r;
        }
        if track.swap {
            std::mem::swap(&mut l, &mut r);
        }
        if sep != 0.0 {
            // Positive narrows toward mono, negative widens (mid/side scaling).
            let mid = (l + r) * 0.5;
            let side = (l - r) * 0.5 * (1.0 - sep);
            l = mid + side;
            r = mid - side;
        }
        // Mixer pan mixes one side into the other; 100% is the mono sum
        // [Manual: mixer, "Panning ... a 100% pan is the mono-sum of the Left and Right channels"].
        if p > 0.0 {
            r += l * p;
            l *= 1.0 - p;
        } else if p < 0.0 {
            l += r * -p;
            r *= 1.0 + p;
        }
        track.left[i] = l * track.gain;
        track.right[i] = r * track.gain;
    }
}
