//! The real-time renderer. `process` never allocates, locks or does I/O.

use crate::plan::{AutoApply, Instrument, RenderPlan, TRACK_COUNT};
use crate::{MAX_BLOCK, Meters};
use jet_dsp::glide::Glide;
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

// Voices live in a pool allocated once; boxing the large variant would allocate on the audio
// thread at every note.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, Default)]
enum VoiceKind {
    #[default]
    Idle,
    Sampler(SamplerVoice),
    Synth(SynthVoice),
}

/// A change to a sounding voice due at `frame` (mono legato or a slide note): glide to
/// `cents` over `frames`; mono legato also takes the new note's velocity and pan.
#[derive(Clone, Copy, Debug)]
struct Pending {
    frame: u64,
    cents: f64,
    frames: u32,
    velocity: Option<f32>,
    pan: Option<i16>,
}

#[derive(Clone, Debug, Default)]
struct Voice {
    kind: VoiceKind,
    channel: ChannelId,
    /// Key the voice was started at; its pitch is this plus the glide offset.
    key: u8,
    /// Key of the note now driving the voice (differs from `key` after a mono legato), used to
    /// match live note-offs.
    note_key: u8,
    fine_pitch: i16,
    color: u8,
    pending: Option<Pending>,
    /// Started by a live note-on rather than the sequence.
    live: bool,
    pan: i16,
    velocity: f32,
    /// Absolute frame where the voice starts sounding.
    start_frame: u64,
    /// Absolute frame of note-off.
    release_frame: u64,
    released: bool,
    /// Cut by Max poly; it is fading out and no longer counts toward the limit.
    stolen: bool,
    /// Audio Clip voice: the clip end cuts it instead of releasing it.
    hard_end: bool,
    cut_by: u8,
    age: u64,
}

impl Voice {
    fn active(&self) -> bool {
        !matches!(self.kind, VoiceKind::Idle)
    }

    fn glide_mut(&mut self) -> Option<&mut Glide> {
        match &mut self.kind {
            VoiceKind::Sampler(v) => Some(&mut v.glide),
            VoiceKind::Synth(v) => Some(&mut v.glide),
            VoiceKind::Idle => None,
        }
    }

    /// The pitch the voice is heading to, in cents (key x 100 + fine pitch + glide target).
    fn target_cents(&self) -> f64 {
        let offset = self.pending.map_or_else(
            || match &self.kind {
                VoiceKind::Sampler(v) => v.glide.target(),
                VoiceKind::Synth(v) => v.glide.target(),
                VoiceKind::Idle => 0.0,
            },
            |p| p.cents,
        );
        f64::from(self.key) * 100.0 + f64::from(self.fine_pitch) + offset
    }

    /// Held (not released) at `frame` on the channel and color group.
    fn holds(&self, channel: ChannelId, color: u8, frame: u64) -> bool {
        self.active()
            && self.channel == channel
            && self.color == color
            && !self.released
            && self.start_frame <= frame
            && self.release_frame > frame
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
    clicks: [Click; 4],
}

/// One metronome click in flight: our own synthesized sound (a decaying sine burst; FL plays
/// its Metronome.wav files, which we do not ship).
#[derive(Clone, Copy, Debug, Default)]
struct Click {
    start_frame: u64,
    accent: bool,
    active: bool,
}

const CLICK_FRAMES: u64 = 1200;

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
            clicks: [Click::default(); 4],
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
            if let (Some(eq), Some(previous)) = (&mut track.eq, &old.eq) {
                eq.take_state(previous);
            }
            for (slot, instance) in track.fx.iter_mut().enumerate() {
                if let Some(previous) = old.fx.get_mut(slot)
                    && previous.kind == instance.kind
                {
                    std::mem::swap(&mut previous.effect, &mut instance.effect);
                }
            }
        }
        for channel in &mut next.channels {
            channel.last_cents = self.plan.channel(channel.id).and_then(|c| c.last_cents);
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
        if sync.playing && !was_playing {
            for channel in &mut self.plan.channels {
                channel.last_cents = None;
            }
            self.start_audio_in_progress(sync.anchor_tick);
        }
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

    /// Pitch targets of the voices not yet released, in cents, ascending (tests).
    #[cfg(test)]
    pub(crate) fn held_cents(&self) -> Vec<f64> {
        let mut cents: Vec<f64> = self
            .voices
            .iter()
            .filter(|v| v.active() && !v.released)
            .map(Voice::target_cents)
            .collect();
        cents.sort_by(f64::total_cmp);
        cents
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

    fn ticks_to_seconds(&self, ticks: i64) -> f64 {
        ticks.max(0) as f64 * 60_000.0
            / (f64::from(self.plan.tempo_milli) * f64::from(self.plan.ppq.max(1)))
    }

    fn ticks_to_frames(&self, ticks: i64) -> u64 {
        let numerator = i128::from(ticks.max(0)) * i128::from(self.plan.rate) * 60_000;
        let denominator = i128::from(self.plan.tempo_milli) * i128::from(self.plan.ppq.max(1));
        (numerator / denominator.max(1)) as u64
    }

    /// Schedule the events of play ticks `from..to`. Looping plans follow the material's jumps,
    /// pauses and end loop (see `Sequence::locate`); each linear stretch is one segment.
    fn schedule(&mut self, from: i64, to: i64) {
        let limit = self.stop_events_after.unwrap_or(i64::MAX);
        let to = to.min(limit);
        let anchor = self.transport.anchor_tick;
        let mut from = from;
        while from < to {
            let (local_from, left) = if self.plan.looping {
                self.plan.sequence.locate(anchor, from)
            } else {
                (from, i64::MAX)
            };
            if left == 0 {
                break; // stopped at a pause marker
            }
            let segment_to = to.min(from.saturating_add(left));
            let local_to = local_from + (segment_to - from);
            let mut index = self.plan.sequence.first_at_or_after(local_from);
            while let Some(event) = self.plan.sequence.events.get(index).copied() {
                if event.tick >= local_to {
                    break;
                }
                self.note_on(&event, from + (event.tick - local_from));
                index += 1;
            }
            if segment_to <= from {
                break;
            }
            from = segment_to;
        }
    }

    fn note_on(&mut self, event: &NoteEvent, absolute_tick: i64) {
        let start_frame = self.frame_of_tick(absolute_tick).max(self.frame);
        let gate = self.plan.sequence.gate_for(event);
        let release_frame = start_frame + self.ticks_to_frames(gate).max(1);
        self.start_voice(event, start_frame, release_frame, false);
    }

    /// When playback starts inside an Audio Clip, the clip sounds from that point (pattern
    /// notes that started earlier are not restarted).
    fn start_audio_in_progress(&mut self, tick: i64) {
        let local = if self.plan.looping {
            self.plan
                .sequence
                .wrap_from(self.transport.anchor_tick, tick)
        } else {
            tick
        };
        let mut index = 0;
        while let Some(event) = self.plan.sequence.events.get(index).copied() {
            index += 1;
            if event.tick >= local {
                break;
            }
            if !event.audio || event.tick + event.length <= local {
                continue;
            }
            let mut late = event;
            late.audio_offset += local - event.tick;
            late.audio_elapsed = local - event.tick;
            late.length -= local - event.tick;
            let start_frame = self.frame;
            let release_frame = start_frame + self.ticks_to_frames(late.length).max(1);
            self.start_voice(&late, start_frame, release_frame, false);
        }
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
            color: 0,
            audio: false,
            audio_offset: 0,
            clip: jet_model::AudioClipProps::default(),
            audio_elapsed: 0,
        };
        let frame = self.frame;
        self.start_voice(&event, frame, u64::MAX, true);
    }

    pub fn live_note_off(&mut self, channel: ChannelId, key: u8) {
        let frame = self.frame;
        for voice in &mut self.voices {
            if voice.live && voice.channel == channel && voice.note_key == key && !voice.released {
                voice.release_frame = frame;
            }
        }
    }

    fn start_voice(&mut self, event: &NoteEvent, start_frame: u64, release_frame: u64, live: bool) {
        let Some(channel) = self.plan.channel(event.channel) else {
            return;
        };
        let rate = self.plan.rate;
        let polyphony = channel.polyphony;
        let last_cents = channel.last_cents;
        let cents = f64::from(event.key) * 100.0 + f64::from(event.fine_pitch);
        let slide_frames = (u64::from(polyphony.slide_ms) * u64::from(rate) / 1000) as u32;
        if !event.audio && self.glide_existing(event, cents, start_frame, release_frame, live) {
            if let Some(channel) = self.plan.channel_mut(event.channel) {
                channel.last_cents = Some(cents);
            }
            return;
        }
        let Some(channel) = self.plan.channel(event.channel) else {
            return;
        };
        let kind = match &channel.instrument {
            Instrument::Sampler(None) => return,
            Instrument::Sampler(Some(sample)) => {
                let semis = f64::from(event.key) - f64::from(channel.root_key);
                let pitch = channel.auto_pitch.unwrap_or(channel.pitch_cents);
                let cents = f64::from(pitch) + f64::from(event.fine_pitch);
                let mut voice = SamplerVoice::default();
                let mut options = channel.sampler;
                options.start_frame = start_frame;
                if event.audio {
                    // Audio Clips ignore the Sampler page and start at their clip offset; the
                    // instance's gain, fades and reverse apply [Manual: playlist_audioclip].
                    options = jet_dsp::sampler::SamplerOptions::default();
                    options.start_seconds = self.ticks_to_seconds(event.audio_offset);
                    options.reverse = event.clip.reverse;
                    let ms = |ms: u32| u64::from(ms) * u64::from(rate) / 1000;
                    options.clip = Some(jet_dsp::sampler::ClipEnvelope {
                        gain: event.clip.gain() as f32,
                        fade_in: ms(event.clip.fade_in_ms),
                        fade_out: ms(event.clip.fade_out_ms),
                        length: self.ticks_to_frames(event.audio_elapsed + event.length),
                        position: self.ticks_to_frames(event.audio_elapsed),
                    });
                }
                voice.start_with(
                    Arc::clone(sample),
                    cents_ratio(semis * 100.0 + cents),
                    rate,
                    options,
                );
                VoiceKind::Sampler(voice)
            }
            Instrument::Synth(params) => {
                let pitch = channel.auto_pitch.unwrap_or(channel.pitch_cents);
                let cents = f64::from(pitch) + f64::from(event.fine_pitch);
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
        // "The portamento flag causes a short slide in pitch from the end of one note to the note
        // with the portamento flag"; the channel's Porta button does it for every note; the time
        // is the SLIDE knob [Manual: pianoroll, "Slide & Portamento"; chansettings_misc].
        let mut kind = kind;
        if (polyphony.porta || event.porta)
            && !event.audio
            && let Some(from) = last_cents
        {
            let glide = match &mut kind {
                VoiceKind::Sampler(v) => Some(&mut v.glide),
                VoiceKind::Synth(v) => Some(&mut v.glide),
                VoiceKind::Idle => None,
            };
            if let Some(glide) = glide {
                glide.set(from - cents, 0.0, slide_frames);
            }
        }
        if !event.audio
            && let Some(channel) = self.plan.channel_mut(event.channel)
        {
            channel.last_cents = Some(cents);
        }
        // Mono "triggers the current release state of notes"; Max poly "causes a forced
        // 'release' of oldest notes over the poly limit. Notes stop playing immediately"
        // [Manual: chansettings_misc, "Polyphony"].
        if polyphony.mono {
            for voice in &mut self.voices {
                if voice.active() && voice.channel == event.channel && !voice.released {
                    voice.release_frame = voice.release_frame.min(start_frame);
                }
            }
        } else if polyphony.max > 0 {
            loop {
                let sounding = self
                    .voices
                    .iter()
                    .filter(|v| v.active() && v.channel == event.channel && !v.stolen)
                    .count();
                if sounding < usize::from(polyphony.max) {
                    break;
                }
                let Some(oldest) = self
                    .voices
                    .iter_mut()
                    .filter(|v| v.active() && v.channel == event.channel && !v.stolen)
                    .min_by_key(|v| v.age)
                else {
                    break;
                };
                oldest.stolen = true;
                oldest.released = true;
                match &mut oldest.kind {
                    VoiceKind::Sampler(v) => v.cut(),
                    VoiceKind::Synth(v) => v.cut(rate),
                    VoiceKind::Idle => {}
                }
            }
        }
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
            note_key: event.key,
            fine_pitch: event.fine_pitch,
            color: event.color,
            pending: None,
            live,
            pan: event.pan,
            velocity: velocity_gain(event.velocity) as f32,
            start_frame,
            release_frame,
            released: false,
            stolen: false,
            hard_end: event.audio,
            cut_by,
            age: self.age,
        };
    }

    /// Slide notes and mono legato change voices that are already sounding instead of starting
    /// one. Returns `true` when the event was consumed.
    fn glide_existing(
        &mut self,
        event: &NoteEvent,
        cents: f64,
        start_frame: u64,
        release_frame: u64,
        live: bool,
    ) -> bool {
        let Some(polyphony) = self.plan.channel(event.channel).map(|c| c.polyphony) else {
            return false;
        };
        let rate = u64::from(self.plan.rate);
        if event.slide {
            // "When the slide note is reached the pitch of the original (standard) note will
            // start sliding toward the slide notes pitch, reaching the same pitch at the end of
            // the slide note event"; in a chord "the topmost note is used as the reference" and
            // all notes move the same interval; only notes of the slide's color are affected;
            // "Slides themselves do NOT produce sound" [Manual: pianoroll, "How slide notes work"].
            let length = release_frame
                .saturating_sub(start_frame)
                .min(u64::from(u32::MAX)) as u32;
            let top = self
                .voices
                .iter()
                .filter(|v| v.holds(event.channel, event.color, start_frame))
                .map(Voice::target_cents)
                .fold(f64::NEG_INFINITY, f64::max);
            if top.is_finite() {
                let delta = cents - top;
                for voice in &mut self.voices {
                    if voice.holds(event.channel, event.color, start_frame) {
                        let base = f64::from(voice.key) * 100.0 + f64::from(voice.fine_pitch);
                        voice.pending = Some(Pending {
                            frame: start_frame,
                            cents: voice.target_cents() + delta - base,
                            frames: length,
                            velocity: None,
                            pan: None,
                        });
                    }
                }
            }
            return true;
        }
        // Mono: "when two notes overlap ... they will 'slide' one to another ... You can set the
        // transition length with the Portamento Time (SLIDE) knob"; "Mono mode applies to each
        // color group separately" [Manual: chansettings_misc; pianoroll, "Color Groups"].
        if !polyphony.mono {
            return false;
        }
        let Some(voice) = self
            .voices
            .iter_mut()
            .filter(|v| v.holds(event.channel, event.color, start_frame))
            .max_by_key(|v| v.age)
        else {
            return false;
        };
        let base = f64::from(voice.key) * 100.0 + f64::from(voice.fine_pitch);
        voice.pending = Some(Pending {
            frame: start_frame,
            cents: cents - base,
            frames: (u64::from(polyphony.slide_ms) * rate / 1000) as u32,
            velocity: Some(velocity_gain(event.velocity) as f32),
            pan: Some(event.pan),
        });
        voice.note_key = event.key;
        voice.release_frame = release_frame;
        voice.live = live;
        true
    }

    /// Evaluate the automation lanes at the block start (block-rate control, our choice; FL's
    /// control rate is not documented). Stopped transport returns every control to its knob.
    fn apply_automation(&mut self, block_start: u64) {
        let playing = self.transport.playing && block_start >= self.transport.anchor_frame;
        let tick = if playing {
            let t = self.tick_at(block_start);
            if self.plan.looping {
                self.plan.sequence.wrap_from(self.transport.anchor_tick, t)
            } else {
                t
            }
        } else {
            0
        };
        let plan = &mut self.plan;
        for channel in &mut plan.channels {
            channel.auto_gain = None;
            channel.auto_pan = None;
            channel.auto_pitch = None;
        }
        for track in &mut plan.tracks {
            track.auto_gain = None;
            track.auto_pan = None;
            for fx in &mut track.fx {
                fx.auto_mix = None;
            }
        }
        if !playing {
            return;
        }
        for lane in &plan.automation {
            let Some(v) = lane.lane.value_at(tick) else {
                continue;
            };
            let knob = (v * 1000.0).round() as u16;
            let pan = (v * 2000.0 - 1000.0).round() as i16;
            match lane.apply {
                AutoApply::ChannelVolume(i) => {
                    if let Some(c) = plan.channels.get_mut(i) {
                        c.auto_gain = Some(jet_dsp::channel_volume_gain(knob) as f32);
                    }
                }
                AutoApply::ChannelPan(i) => {
                    if let Some(c) = plan.channels.get_mut(i) {
                        c.auto_pan = Some(pan);
                    }
                }
                AutoApply::ChannelPitch(i) => {
                    if let Some(c) = plan.channels.get_mut(i) {
                        c.auto_pitch = Some((v * 2400.0 - 1200.0).round() as i16);
                    }
                }
                AutoApply::TrackVolume(s) => {
                    if let Some(t) = plan.tracks.get_mut(s) {
                        t.auto_gain = Some(jet_dsp::mixer_fader_gain(knob) as f32);
                    }
                }
                AutoApply::TrackPan(s) => {
                    if let Some(t) = plan.tracks.get_mut(s) {
                        t.auto_pan = Some(pan);
                    }
                }
                AutoApply::FxMix(s, slot) => {
                    if let Some(fx) = plan.tracks.get_mut(s).and_then(|t| t.fx.get_mut(slot)) {
                        fx.auto_mix = Some(v as f32);
                    }
                }
            }
        }
    }

    fn schedule_clicks(&mut self, from: i64, to: i64) {
        let beat = self.plan.beat_ticks.max(1);
        let mut tick = from.div_euclid(beat) * beat;
        if tick < from {
            tick += beat;
        }
        while tick < to {
            let local = if self.plan.looping {
                self.plan
                    .sequence
                    .wrap_from(self.transport.anchor_tick, tick)
            } else {
                tick
            };
            let accent = (local / beat).rem_euclid(self.plan.beats_per_bar.max(1)) == 0;
            let start_frame = self.frame_of_tick(tick).max(self.frame);
            if let Some(slot) = self.clicks.iter_mut().find(|c| !c.active) {
                *slot = Click {
                    start_frame,
                    accent,
                    active: true,
                };
            }
            tick += beat;
        }
    }

    fn render_clicks(&mut self, block_start: u64, frames: usize) {
        let rate = f64::from(self.plan.rate.max(1));
        let master = &mut self.plan.tracks[self.plan.master_slot];
        for click in &mut self.clicks {
            if !click.active {
                continue;
            }
            let freq = if click.accent { 1_760.0 } else { 1_320.0 };
            for i in 0..frames {
                let frame = block_start + i as u64;
                if frame < click.start_frame {
                    continue;
                }
                let n = frame - click.start_frame;
                if n >= CLICK_FRAMES {
                    click.active = false;
                    break;
                }
                let t = n as f64 / rate;
                let v =
                    (0.35 * (std::f64::consts::TAU * freq * t).sin() * (-t * 120.0).exp()) as f32;
                master.left[i] += v;
                master.right[i] += v;
            }
        }
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
        self.apply_automation(block_start);
        if self.transport.playing && block_end > self.transport.anchor_frame {
            // Event at tick T sounds at frame_of_tick(T); it belongs to this block when that
            // frame is below block_end, i.e. when T <= tick_at(block_end - 1).
            let to = self.tick_at(block_end - 1).saturating_add(1);
            let from = self.next_tick;
            self.schedule(from, to);
            if self.plan.metronome {
                self.schedule_clicks(from, to);
            }
            self.next_tick = to.max(from);
        }
        self.render_voices(block_start, frames);
        self.render_clicks(block_start, frames);
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
            let track = &mut plan.tracks[channel.track_slot];
            // Split the block at note-off and at pending glides so each starts on its exact frame.
            let mut cursor = voice.start_frame.saturating_sub(block_start) as usize;
            let mut alive = true;
            while alive && cursor < frames {
                let at = |frame: u64| frame.saturating_sub(block_start).max(cursor as u64) as usize;
                let release_at = (!voice.released && voice.release_frame < block_end)
                    .then(|| at(voice.release_frame));
                let pending_at = voice
                    .pending
                    .filter(|p| p.frame < block_end)
                    .map(|p| at(p.frame));
                let next = release_at
                    .into_iter()
                    .chain(pending_at)
                    .min()
                    .unwrap_or(frames);
                if next > cursor {
                    let channel_pan = channel.auto_pan.unwrap_or(channel.pan);
                    let pan =
                        (i32::from(channel_pan) + i32::from(voice.pan)).clamp(-1000, 1000) as i16;
                    let (pl, pr) = pan_gains(pan, pan_law);
                    let gain = channel.auto_gain.unwrap_or(channel.gain) * voice.velocity;
                    let (gl, gr) = (gain * pl as f32, gain * pr as f32);
                    let l = &mut track.left[cursor..next];
                    let r = &mut track.right[cursor..next];
                    alive = match &mut voice.kind {
                        VoiceKind::Sampler(v) => v.render(l, r, gl, gr),
                        VoiceKind::Synth(v) => v.render(l, r, gl, gr),
                        VoiceKind::Idle => false,
                    };
                    cursor = next;
                }
                if pending_at == Some(next)
                    && let Some(p) = voice.pending.take()
                {
                    if let Some(glide) = voice.glide_mut() {
                        glide.to(p.cents, p.frames);
                    }
                    if let Some(velocity) = p.velocity {
                        voice.velocity = velocity;
                    }
                    if let Some(pan) = p.pan {
                        voice.pan = pan;
                    }
                }
                if release_at == Some(next) {
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
            match &mut voice.kind {
                VoiceKind::Synth(v) => v.release(),
                VoiceKind::Sampler(v) if voice.hard_end => v.cut(),
                VoiceKind::Sampler(v) => v.release(),
                VoiceKind::Idle => {}
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
            if self.meters.tap_track() == Some(index) {
                self.meters
                    .record_tap(&track.left[..frames], &track.right[..frames]);
            }
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
            let mix = instance.auto_mix.unwrap_or(instance.mix);
            if mix >= 0.999 {
                effect.process(&mut track.left[..frames], &mut track.right[..frames]);
            } else {
                track.scratch_left[..frames].copy_from_slice(&track.left[..frames]);
                track.scratch_right[..frames].copy_from_slice(&track.right[..frames]);
                effect.process(
                    &mut track.scratch_left[..frames],
                    &mut track.scratch_right[..frames],
                );
                let wet = mix;
                for i in 0..frames {
                    track.left[i] += (track.scratch_left[i] - track.left[i]) * wet;
                    track.right[i] += (track.scratch_right[i] - track.right[i]) * wet;
                }
            }
        }
    }
    // "The integrated effects are the last processing/FX stage prior to audio leaving the Mixer
    // Track and so follow any plugin FX loaded into slots" [Manual: mixer_trackprops].
    if let Some(eq) = &mut track.eq {
        eq.process(&mut track.left[..frames], &mut track.right[..frames]);
    }
    let p = f32::from(track.auto_pan.unwrap_or(track.pan)) / 1000.0;
    let gain = track.auto_gain.unwrap_or(track.gain);
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
        track.left[i] = l * gain;
        track.right[i] = r * gain;
    }
}
