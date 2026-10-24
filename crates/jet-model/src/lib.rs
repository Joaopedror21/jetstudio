#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

//! Persistent project data. Selection, scroll, focus and open windows belong to the UI.

use jet_time::{Ppq, Tempo, Tick, TimeError, TimeSig};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const MAX_CHANNELS: usize = 10_000; // Our file-input safety limit; no FL limit claimed.
pub const MAX_PATTERNS: usize = 999;
pub const MAX_STEPS: usize = 512;
pub const MAX_INSERTS: usize = 500;
pub const MAX_TRACKS: usize = 10_000; // Our file-input safety limit.
pub const MAX_CLIPS: usize = 1_000_000; // Our file-input safety limit.
pub const MAX_RESOURCES: usize = 100_000; // Our file-input safety limit.
pub const MAX_NOTES_PER_SEQUENCE: usize = 1_000_000; // Our file-input safety limit.
/// Highest note key. FL's keyboard spans C0..B10, 132 keys [Manual: pianoroll]; 60 = C5.
pub const MAX_KEY: u8 = 131;
/// Default Sampler root key [Manual: chansettings_misc, "The default Root Key is C5"].
pub const DEFAULT_ROOT_KEY: u8 = 60;
/// Default Sampler volume knob position [Manual: channelrack, "-5.2 dB (78%)"].
pub const DEFAULT_CHANNEL_VOLUME: u16 = 781;
/// Default velocity [Manual: mixer_levelsandmixing, "The default note velocity ... is 100"].
pub const DEFAULT_VELOCITY: u8 = 100;
/// Fader position that maps to 0 dB. Hypothesis H-MIX-1 in docs/FL_STUDIO_COMPLETE_FEATURES.md.
pub const MIXER_UNITY: u16 = 800;

macro_rules! id_type {
    ($name:ident) => {
        #[derive(
            Clone,
            Copy,
            Debug,
            Default,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Hash,
            Serialize,
            Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub u64);
    };
}

id_type!(ChannelId);
id_type!(PatternId);
id_type!(ArrangementId);
id_type!(TrackId);
id_type!(ClipId);
id_type!(ResourceId);
id_type!(AutomationId);

/// Counters advance monotonically and are serialized with the project.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdAllocator {
    pub next_channel: u64,
    pub next_pattern: u64,
    pub next_arrangement: u64,
    pub next_track: u64,
    pub next_clip: u64,
    pub next_resource: u64,
    #[serde(default = "first_id")]
    pub next_automation: u64,
}

fn first_id() -> u64 {
    1
}

impl Default for IdAllocator {
    fn default() -> Self {
        Self {
            next_channel: 1,
            next_pattern: 1,
            next_arrangement: 1,
            next_track: 1,
            next_clip: 1,
            next_resource: 1,
            next_automation: 1,
        }
    }
}

impl IdAllocator {
    fn allocate(next: &mut u64) -> u64 {
        let id = *next;
        *next = next.saturating_add(1);
        id
    }

    pub fn channel(&mut self) -> ChannelId {
        ChannelId(Self::allocate(&mut self.next_channel))
    }

    pub fn pattern(&mut self) -> PatternId {
        PatternId(Self::allocate(&mut self.next_pattern))
    }

    pub fn arrangement(&mut self) -> ArrangementId {
        ArrangementId(Self::allocate(&mut self.next_arrangement))
    }

    pub fn track(&mut self) -> TrackId {
        TrackId(Self::allocate(&mut self.next_track))
    }

    pub fn automation(&mut self) -> AutomationId {
        AutomationId(Self::allocate(&mut self.next_automation))
    }

    pub fn clip(&mut self) -> ClipId {
        ClipId(Self::allocate(&mut self.next_clip))
    }

    pub fn resource(&mut self) -> ResourceId {
        ResourceId(Self::allocate(&mut self.next_resource))
    }
}

/// Project panning law [Manual: songsettings_settings, "Panning law"].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PanLaw {
    /// Level drops 3 dB at center pan.
    #[default]
    Circular,
    /// No level compensation as a function of pan position.
    Triangular,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectSettings {
    pub ppq: Ppq,
    pub tempo: Tempo,
    pub time_sig: TimeSig,
    pub steps_per_beat: u8,
    pub swing: u8,
    /// Internal fixed-point parameter, 0..=1000. This is not a measured UI scale.
    pub main_volume: u16,
    /// Internal fixed-point parameter, -1000..=1000. This is not a measured UI scale.
    pub main_pitch: i16,
    #[serde(default)]
    pub pan_law: PanLaw,
}

impl Default for ProjectSettings {
    fn default() -> Self {
        Self {
            ppq: Ppq::DEFAULT,
            tempo: Tempo(130_000), // [W] a new FL Studio 2026 project shows 130.000 BPM
            time_sig: TimeSig::default(),
            steps_per_beat: 4,
            swing: 0,
            main_volume: 1000,
            main_pitch: 0,
            pan_law: PanLaw::Circular,
        }
    }
}

impl ProjectSettings {
    /// Ticks in one Step Sequencer step, or `None` when the PPQ cannot represent it.
    pub fn step_ticks(&self) -> Option<i64> {
        let beat = i64::from(self.time_sig.ticks_per_beat(self.ppq).ok()?);
        let per_beat = i64::from(self.steps_per_beat);
        (per_beat > 0 && beat % per_beat == 0).then(|| beat / per_beat)
    }

    pub fn beat_ticks(&self) -> Option<i64> {
        self.time_sig.ticks_per_beat(self.ppq).ok().map(i64::from)
    }

    pub fn bar_ticks(&self) -> Option<i64> {
        Some(self.beat_ticks()? * i64::from(self.time_sig.num))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Waveform {
    #[default]
    Saw,
    Square,
    Triangle,
    Sine,
    Noise,
}

/// Parameters of the built-in synthesizer (our own design, not a copy of an FL plugin).
/// Times are milliseconds; levels are 0..=1000.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SynthParams {
    pub waveform: Waveform,
    pub attack_ms: u16,
    pub decay_ms: u16,
    pub sustain: u16,
    pub release_ms: u16,
    /// Low-pass cutoff in Hz.
    pub cutoff_hz: u16,
    pub resonance: u16,
    /// Detune of a second oscillator in cents; zero disables it.
    pub detune_cents: i16,
}

impl Default for SynthParams {
    fn default() -> Self {
        Self {
            waveform: Waveform::Saw,
            attack_ms: 5,
            decay_ms: 200,
            sustain: 700,
            release_ms: 120,
            cutoff_hz: 4000,
            resonance: 200,
            detune_cents: 7,
        }
    }
}

impl SynthParams {
    pub fn valid(&self) -> bool {
        self.sustain <= 1000
            && self.resonance <= 1000
            && (20..=20_000).contains(&self.cutoff_hz)
            && self.attack_ms <= 10_000
            && self.decay_ms <= 10_000
            && self.release_ms <= 10_000
            && i32::from(self.detune_cents).abs() <= 100
    }
}

/// Volume envelope; times in ms, sustain 0..=1000.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Envelope {
    pub attack_ms: u16,
    pub decay_ms: u16,
    pub sustain: u16,
    pub release_ms: u16,
}

impl Default for Envelope {
    fn default() -> Self {
        Self {
            attack_ms: 0,
            decay_ms: 200,
            sustain: 1000,
            release_ms: 100,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SampleLoop {
    #[default]
    Off,
    Forward,
    PingPong,
}

/// Sampler playback settings [Manual: chansettings_sampler, "Playback", "Precomputed"].
/// Positions are fractions of the sample, 0..=1000.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SamplerSettings {
    pub start: u16,
    pub looping: SampleLoop,
    pub loop_start: u16,
    pub loop_end: u16,
    pub reverse: bool,
    /// `Some` when the volume envelope is active.
    pub envelope: Option<Envelope>,
    /// INS page filter [Manual: chansettings_ins, "Filter Section"].
    #[serde(default)]
    pub filter: SamplerFilter,
    /// INS envelopes for [`ModTarget::ModX`], [`ModTarget::ModY`] and [`ModTarget::Pitch`] (in
    /// that order); the volume envelope is `envelope` and "pan does not support an envelope".
    #[serde(default)]
    pub mod_env: [ModEnvelope; 3],
    /// INS LFOs, indexed by [`ModTarget`].
    #[serde(default)]
    pub lfo: [Lfo; 5],
}

impl Default for SamplerSettings {
    fn default() -> Self {
        Self {
            start: 0,
            looping: SampleLoop::Off,
            loop_start: 0,
            loop_end: 1000,
            reverse: false,
            envelope: None,
            filter: SamplerFilter::default(),
            mod_env: [ModEnvelope::default(); 3],
            lfo: [Lfo::default(); 5],
        }
    }
}

/// Targets of the INS page tabs [Manual: chansettings_ins, "Contains envelopes and LFOs for
/// Panning, Volume, Mod X, Mod Y and Pitch"].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModTarget {
    Pan,
    Volume,
    ModX,
    ModY,
    Pitch,
}

impl ModTarget {
    pub const ALL: [ModTarget; 5] = [
        ModTarget::Pan,
        ModTarget::Volume,
        ModTarget::ModX,
        ModTarget::ModY,
        ModTarget::Pitch,
    ];

    /// Index into `SamplerSettings::mod_env`, for the targets that have one there.
    pub fn env_index(self) -> Option<usize> {
        match self {
            ModTarget::ModX => Some(0),
            ModTarget::ModY => Some(1),
            ModTarget::Pitch => Some(2),
            ModTarget::Pan | ModTarget::Volume => None,
        }
    }

    pub fn lfo_index(self) -> usize {
        self as usize
    }
}

/// "The drop-down menu selects the filter type" [Manual: chansettings_ins, "Filter Section"].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FilterType {
    #[default]
    FastLp,
    Lp,
    Bp,
    Hp,
    Bs,
    Lp2,
    SvfLp,
    SvfLp2,
}

/// "By default Mod X controls the Cutoff of a filter and Mod Y the resonance"; "When Mod X (Filter
/// Cutoff) is 100% and Mod Y (Resonance) is 0% the filtering is completely bypassed. This is the
/// default state for Sampler Channels."
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SamplerFilter {
    pub kind: FilterType,
    /// Cutoff 0..=1000.
    pub mod_x: u16,
    /// Resonance 0..=1000.
    pub mod_y: u16,
}

impl Default for SamplerFilter {
    fn default() -> Self {
        Self {
            kind: FilterType::FastLp,
            mod_x: 1000,
            mod_y: 0,
        }
    }
}

/// INS envelope: "Delay ... Attack ... Hold ... Decay ... Sustain ... Release ... Amount (AMT) - ...
/// If the Amount knob is centered (AMT = 0), the envelope will NOT be applied" [Manual:
/// chansettings_ins]. Times in ms; sustain 0..=1000; amount -1000..=1000. The envelope applies
/// only "when it is turned on".
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModEnvelope {
    pub on: bool,
    pub delay_ms: u16,
    pub attack_ms: u16,
    pub hold_ms: u16,
    pub decay_ms: u16,
    pub sustain: u16,
    pub release_ms: u16,
    pub amount: i16,
}

impl Default for ModEnvelope {
    fn default() -> Self {
        // Hypothesis H-SMP-2: FL's default envelope values are not documented.
        Self {
            on: false,
            delay_ms: 0,
            attack_ms: 10,
            hold_ms: 0,
            decay_ms: 300,
            sustain: 500,
            release_ms: 200,
            amount: 500,
        }
    }
}

/// "You can choose from three LFO shapes - sine, triangle and square."
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LfoShape {
    #[default]
    Sine,
    Triangle,
    Square,
}

/// INS LFO: Delay, Attack, Amount (0 = off, negative inverts the phase), Speed, Tempo, Global.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lfo {
    pub shape: LfoShape,
    pub delay_ms: u16,
    pub attack_ms: u16,
    pub amount: i16,
    /// Speed in thousandths of a cycle per second, or per beat when `tempo` is on.
    pub speed_milli: u32,
    pub tempo: bool,
    /// "does not restart in the beginning of each note".
    pub global: bool,
}

impl Default for Lfo {
    fn default() -> Self {
        Self {
            shape: LfoShape::Sine,
            delay_ms: 0,
            attack_ms: 0,
            amount: 0,
            speed_milli: 2000,
            tempo: false,
            global: false,
        }
    }
}

impl ModEnvelope {
    pub fn valid(&self) -> bool {
        self.sustain <= 1000 && i32::from(self.amount).abs() <= 1000
    }
}

impl Lfo {
    pub fn valid(&self) -> bool {
        i32::from(self.amount).abs() <= 1000 && (10..=50_000).contains(&self.speed_milli)
    }
}

impl SamplerSettings {
    pub fn valid(&self) -> bool {
        self.filter.mod_x <= 1000
            && self.filter.mod_y <= 1000
            && self.mod_env.iter().all(ModEnvelope::valid)
            && self.lfo.iter().all(Lfo::valid)
            && self.start <= 1000
            && self.loop_start < self.loop_end
            && self.loop_end <= 1000
            && self.envelope.is_none_or(|e| {
                e.sustain <= 1000
                    && e.attack_ms <= 10_000
                    && e.decay_ms <= 10_000
                    && e.release_ms <= 10_000
            })
    }
}

/// [Manual: chansettings_misc, "Polyphony"]: "MAX ... If the LCD displays dashes, it indicates
/// that polyphony is unrestricted"; "Mono mode overrides the 'Max poly' option".
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Polyphony {
    /// Zero means unrestricted.
    pub max: u16,
    pub mono: bool,
    /// "Porta - ... sliding the pitch from note to note".
    #[serde(default)]
    pub porta: bool,
    /// "SLIDE - Portamento Time ... the slide length when portamento is turned on. Also used for
    /// overlapping notes in monophonic mode." Hypothesis H-SMP-1: the manual gives no unit or
    /// range; we use 0..=MAX_SLIDE_MS milliseconds.
    #[serde(default = "default_slide_ms")]
    pub slide_ms: u16,
}

pub const MAX_SLIDE_MS: u16 = 2000;
pub const DEFAULT_SLIDE_MS: u16 = 100;

fn default_slide_ms() -> u16 {
    DEFAULT_SLIDE_MS
}

impl Default for Polyphony {
    fn default() -> Self {
        Self {
            max: 0,
            mono: false,
            porta: false,
            slide_ms: DEFAULT_SLIDE_MS,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChannelKind {
    Sampler {
        sample: Option<ResourceId>,
    },
    Synth(SynthParams),
    /// The channel behind Audio Clips in the Playlist [Manual: playlist_audioclip;
    /// channelrack "Sort by ... Type: ... Audio Clips & Automation Clips"].
    AudioClip {
        sample: Option<ResourceId>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Channel {
    pub id: ChannelId,
    pub name: String,
    pub color: Option<u32>,
    pub kind: ChannelKind,
    pub mute: bool,
    /// Internal fixed-point parameter, 0..=1000.
    pub volume: u16,
    /// Internal fixed-point parameter, -1000..=1000.
    pub pan: i16,
    /// Zero refers to Master; positive values refer to inserts.
    pub mixer_target: u16,
    pub group: Option<String>,
    /// Channel pitch offset in cents [Manual: channelrack, "Transpose selected"].
    #[serde(default)]
    pub pitch: i16,
    /// Key that plays a sample at its recorded pitch.
    #[serde(default = "default_root_key")]
    pub root_key: u8,
    /// Per-channel swing multiplier 0..=1000, 100% by default [Manual: channelrack, "Global Swing"].
    #[serde(default = "default_swing_mix")]
    pub swing_mix: u16,
    /// Cut group; 0 means none [Manual: chansettings_misc, "Cut Groups"].
    #[serde(default)]
    pub cut: u8,
    /// Cut-by group; 0 means none.
    #[serde(default)]
    pub cut_by: u8,
    /// Used when the channel holds a Sampler.
    #[serde(default)]
    pub sampler: SamplerSettings,
    #[serde(default)]
    pub polyphony: Polyphony,
}

fn default_root_key() -> u8 {
    DEFAULT_ROOT_KEY
}

fn default_swing_mix() -> u16 {
    1000
}

impl Channel {
    pub fn sampler(id: ChannelId, name: impl Into<String>) -> Self {
        Self::with_kind(id, name, ChannelKind::Sampler { sample: None })
    }

    pub fn synth(id: ChannelId, name: impl Into<String>) -> Self {
        Self::with_kind(id, name, ChannelKind::Synth(SynthParams::default()))
    }

    pub fn with_kind(id: ChannelId, name: impl Into<String>, kind: ChannelKind) -> Self {
        Self {
            id,
            name: name.into(),
            color: None,
            kind,
            mute: false,
            volume: DEFAULT_CHANNEL_VOLUME,
            pan: 0,
            mixer_target: 0,
            group: None,
            pitch: 0,
            root_key: DEFAULT_ROOT_KEY,
            swing_mix: 1000,
            cut: 0,
            cut_by: 0,
            sampler: SamplerSettings::default(),
            polyphony: Polyphony::default(),
        }
    }

    pub fn sample(&self) -> Option<ResourceId> {
        match &self.kind {
            ChannelKind::Sampler { sample } | ChannelKind::AudioClip { sample } => *sample,
            ChannelKind::Synth(_) => None,
        }
    }

    /// Audio Clips "play at 100% their original level" [Manual: channelrack, §7-8].
    pub fn audio_clip(id: ChannelId, name: impl Into<String>, sample: ResourceId) -> Self {
        let mut channel = Self::with_kind(
            id,
            name,
            ChannelKind::AudioClip {
                sample: Some(sample),
            },
        );
        channel.volume = 1000;
        channel
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatternLength {
    Auto,
    Steps(u16),
}

/// Per-step properties edited by the Graph Editor [Manual: channelrack, "Graph Editor"].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepProps {
    /// 0..=127.
    pub velocity: u8,
    /// -1000..=1000.
    pub pan: i16,
    pub release: u8,
    /// Fine pitch in cents, -100..=100 [Manual: "Fine pitch - +/- 100 cents"].
    pub pitch: i16,
    pub mod_x: u8,
    pub mod_y: u8,
    /// Start delay in ticks, below one step [Manual: "100% shift equals 99% of ... one 16th note"].
    pub shift: i16,
    /// Note key of the step [Manual: Graph Editor "Note" target].
    #[serde(default = "default_root_key")]
    pub note: u8,
}

impl Default for StepProps {
    fn default() -> Self {
        Self {
            velocity: DEFAULT_VELOCITY,
            pan: 0,
            release: 0,
            pitch: 0,
            mod_x: 0,
            mod_y: 0,
            shift: 0,
            note: DEFAULT_ROOT_KEY,
        }
    }
}

impl StepProps {
    pub fn valid(&self) -> bool {
        self.velocity <= 127
            && i32::from(self.pan).abs() <= 1000
            && i32::from(self.pitch).abs() <= 100
            && self.shift >= 0
            && self.note <= MAX_KEY
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Step {
    pub on: bool,
    pub props: StepProps,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepSeq {
    pub cells: Vec<Step>,
}

impl StepSeq {
    pub fn with_len(length: usize) -> Result<Self, ModelError> {
        if length > MAX_STEPS {
            return Err(ModelError::Limit("steps"));
        }
        Ok(Self {
            cells: vec![Step::default(); length],
        })
    }
}

/// A Piano roll note. Times are project ticks relative to the pattern start.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note {
    pub start: Tick,
    /// Zero length is a step-style note [Manual: channelrack, "you need notes of zero length"].
    pub length: Tick,
    pub key: u8,
    /// 0..=127.
    pub velocity: u8,
    /// -1000..=1000.
    pub pan: i16,
    pub release: u8,
    /// Fine pitch in cents, -1200..=1200.
    pub fine_pitch: i16,
    pub mod_x: u8,
    pub mod_y: u8,
    pub muted: bool,
    pub slide: bool,
    pub porta: bool,
    /// Note color group 0..=15 [Manual: pianoroll, "one of the 16 colors"].
    pub color: u8,
}

impl Note {
    pub fn new(start: Tick, length: Tick, key: u8) -> Self {
        Self {
            start,
            length,
            key,
            velocity: DEFAULT_VELOCITY,
            pan: 0,
            release: 0,
            fine_pitch: 0,
            mod_x: 0,
            mod_y: 0,
            muted: false,
            slide: false,
            porta: false,
            color: 0,
        }
    }

    pub fn end(&self) -> Tick {
        Tick(self.start.0.saturating_add(self.length.0))
    }

    pub fn valid(&self) -> bool {
        self.start.0 >= 0
            && self.length.0 >= 0
            && self.key <= MAX_KEY
            && self.velocity <= 127
            && i32::from(self.pan).abs() <= 1000
            && i32::from(self.fine_pitch).abs() <= 1200
            && self.color <= 15
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pattern {
    pub id: PatternId,
    pub name: String,
    pub color: Option<u32>,
    pub length: PatternLength,
    pub steps: BTreeMap<ChannelId, StepSeq>,
    /// Piano roll scores, kept sorted by (start, key).
    #[serde(default)]
    pub notes: BTreeMap<ChannelId, Vec<Note>>,
}

impl Pattern {
    pub fn new(id: PatternId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            color: None,
            length: PatternLength::Auto,
            steps: BTreeMap::new(),
            notes: BTreeMap::new(),
        }
    }

    /// True when the pattern holds an active step or a note.
    pub fn has_data(&self) -> bool {
        self.steps
            .values()
            .any(|seq| seq.cells.iter().any(|cell| cell.on))
            || self.notes.values().any(|notes| !notes.is_empty())
    }

    /// True when `channel` is driven by its Piano roll in this pattern.
    pub fn uses_piano_roll(&self, channel: ChannelId) -> bool {
        self.notes
            .get(&channel)
            .is_some_and(|notes| !notes.is_empty())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaylistTrack {
    pub id: TrackId,
    pub name: String,
    pub color: Option<u32>,
    pub height: u16,
    pub mute: bool,
}

impl PlaylistTrack {
    pub fn new(id: TrackId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            color: None,
            height: 1,
            mute: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClipSource {
    Pattern(PatternId),
    Automation(AutomationId),
    /// An Audio Clip playing the sample of an `AudioClip` channel.
    Audio(ChannelId),
}

/// Segment curve types [Manual: playlist_automationclip, "Change curve type"]. The shape is
/// stored on the point that ends the segment, as FL edits it on "the right-most control point".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CurveShape {
    #[default]
    SingleCurve,
    DoubleCurve,
    AltSingleCurve,
    AltDoubleCurve,
    Hold,
    Stairs,
    SmoothStairs,
    Pulse,
    Wave,
    HalfSine,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutoPoint {
    /// Position relative to the clip's data start.
    pub at: Tick,
    /// 0..=1000 (the manual's 0 to 1).
    pub value: u16,
    /// -1000..=1000; zero is a straight segment for `SingleCurve`.
    pub tension: i16,
    /// Shape of the segment that ends at this point.
    pub shape: CurveShape,
}

/// Parameters an Automation Clip can drive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AutoTarget {
    ChannelVolume(ChannelId),
    ChannelPan(ChannelId),
    ChannelPitch(ChannelId),
    MixerVolume(u16),
    MixerPan(u16),
    FxMix { track: u16, slot: u8 },
}

impl AutoTarget {
    /// Normalised 0..=1000 value of the target's current setting in `project`.
    pub fn current(self, project: &Project) -> Option<u16> {
        Some(match self {
            Self::ChannelVolume(id) => project.channel(id)?.volume,
            Self::ChannelPan(id) => ((i32::from(project.channel(id)?.pan) + 1000) / 2) as u16,
            Self::ChannelPitch(id) => {
                ((i32::from(project.channel(id)?.pitch).clamp(-1200, 1200) + 1200) * 1000 / 2400)
                    as u16
            }
            Self::MixerVolume(track) => project.mixer.track(track)?.volume,
            Self::MixerPan(track) => {
                ((i32::from(project.mixer.track(track)?.pan) + 1000) / 2) as u16
            }
            Self::FxMix { track, slot } => {
                project.mixer.track(track)?.fx.get(usize::from(slot))?.mix
            }
        })
    }

    pub fn label(self, project: &Project) -> String {
        let channel = |id: ChannelId| {
            project
                .channel(id)
                .map_or_else(|| "Channel".to_owned(), |c| c.name.clone())
        };
        match self {
            Self::ChannelVolume(id) => format!("{} - Volume", channel(id)),
            Self::ChannelPan(id) => format!("{} - Panning", channel(id)),
            Self::ChannelPitch(id) => format!("{} - Pitch", channel(id)),
            Self::MixerVolume(0) => "Master - Volume".to_owned(),
            Self::MixerVolume(track) => format!("Insert {track} - Volume"),
            Self::MixerPan(0) => "Master - Pan".to_owned(),
            Self::MixerPan(track) => format!("Insert {track} - Pan"),
            Self::FxMix { track, slot } => format!("Insert {track} - Slot {} mix", slot + 1),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutomationClip {
    pub id: AutomationId,
    pub name: String,
    pub target: AutoTarget,
    /// Sorted by `at`; never empty.
    pub points: Vec<AutoPoint>,
    /// The value the control returns to when the song starts ("Initialized controls"
    /// [Manual: playlist_automationclip]).
    pub initial: u16,
    /// Automation Clip Settings: Min, Max, Time and the LFO.
    #[serde(default)]
    pub settings: AutoSettings,
}

/// [Manual: playlist_automationclip, "Settings"]: "Min - ... re-ranges the output"; "Max"; "Time -
/// Controls the speed at which the Automation Clip plays back"; "LFO Mode".
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutoSettings {
    /// Output range 0..=1000.
    pub min: u16,
    pub max: u16,
    /// Playback speed multiplier x1000 (100..=16000).
    pub time_milli: u16,
    pub lfo: AutoLfo,
}

impl Default for AutoSettings {
    fn default() -> Self {
        Self {
            min: 0,
            max: 1000,
            time_milli: 1000,
            lfo: AutoLfo::default(),
        }
    }
}

/// "LFO Switch"; "Speed (SPD)"; "Tension (TENS) - ... a shape morph ... starting with square
/// (pulse), sine, triangle and 'pulse'"; "Shape Skew"; "Pulse Width"; "Level (LVL) - ... the
/// middle position ... disable[s] the LFO. Turn it left for negative amplitude"; "Multiply - ...
/// the two values will be multiplied ... When the switch is disabled both values are added".
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutoLfo {
    pub on: bool,
    /// Cycles per beat x1000 (hypothesis H-AUT-4: the manual gives no unit).
    pub speed_milli: u32,
    /// -1000 (square) .. -333 (sine) .. 333 (triangle) .. 1000 (narrow pulse).
    pub tension: i16,
    /// -1000..=1000; with the triangle it gives saw shapes.
    pub skew: i16,
    /// 1..=999 (fraction of the cycle x1000).
    pub pulse_width: u16,
    /// -1000..=1000.
    pub level: i16,
    pub multiply: bool,
}

impl Default for AutoLfo {
    fn default() -> Self {
        Self {
            on: false,
            speed_milli: 1000,
            tension: -333,
            skew: 0,
            pulse_width: 500,
            level: 1000,
            multiply: false,
        }
    }
}

impl AutoSettings {
    pub fn valid(&self) -> bool {
        self.min <= 1000
            && self.max <= 1000
            && (100..=16_000).contains(&self.time_milli)
            && (10..=64_000).contains(&self.lfo.speed_milli)
            && i32::from(self.lfo.tension).abs() <= 1000
            && i32::from(self.lfo.skew).abs() <= 1000
            && (1..=999).contains(&self.lfo.pulse_width)
            && i32::from(self.lfo.level).abs() <= 1000
    }
}

impl AutomationClip {
    pub fn length(&self) -> Tick {
        self.points.last().map_or(Tick(0), |p| p.at)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipInstance {
    pub id: ClipId,
    pub track: TrackId,
    pub start: Tick,
    pub length: Tick,
    pub offset: Tick,
    pub source: ClipSource,
    pub muted: bool,
    /// Audio Clip instance settings (ignored by other clips).
    #[serde(default)]
    pub audio: AudioClipProps,
}

/// "Clip Properties allow you to work on instancies of a parent Audio Clip and create 'variants'
/// with different Gain, Pitch and Reverse settings"; "Audio Clips include integrated Volume
/// envelopes. Controls work per-instance" [Manual: playlist_audioclip].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioClipProps {
    /// Clip gain in hundredths of a dB: "+36 dB to -inf ... Below -96 dB the level snaps to -inf"
    /// (`GAIN_SILENT`).
    pub gain_cdb: i16,
    /// "Type in a fade value in milliseconds (ms)".
    pub fade_in_ms: u32,
    pub fade_out_ms: u32,
    pub reverse: bool,
}

impl AudioClipProps {
    pub const GAIN_SILENT: i16 = i16::MIN;
    pub const GAIN_MAX_CDB: i16 = 3600;
    pub const GAIN_MIN_CDB: i16 = -9600;
    pub const MAX_FADE_MS: u32 = 600_000;

    pub fn valid(&self) -> bool {
        (self.gain_cdb == Self::GAIN_SILENT
            || (Self::GAIN_MIN_CDB..=Self::GAIN_MAX_CDB).contains(&self.gain_cdb))
            && self.fade_in_ms <= Self::MAX_FADE_MS
            && self.fade_out_ms <= Self::MAX_FADE_MS
    }

    /// Linear gain factor.
    pub fn gain(&self) -> f64 {
        if self.gain_cdb == Self::GAIN_SILENT {
            0.0
        } else {
            10f64.powf(f64::from(self.gain_cdb) / 2000.0)
        }
    }
}

impl ClipInstance {
    pub fn pattern(
        id: ClipId,
        track: TrackId,
        pattern: PatternId,
        start: Tick,
        length: Tick,
    ) -> Self {
        Self {
            id,
            track,
            start,
            length,
            offset: Tick(0),
            source: ClipSource::Pattern(pattern),
            muted: false,
            audio: AudioClipProps::default(),
        }
    }

    pub fn end(&self) -> Tick {
        Tick(self.start.0.saturating_add(self.length.0))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Marker {
    pub at: Tick,
    pub name: String,
    #[serde(default)]
    pub action: MarkerAction,
}

/// "Action - Set the Markers purpose" [Manual: playlist, "Time Markers"]. The other actions the
/// manual lists (Marker loop, Marker skip, Marker pause, Time signature, Start/Stop recording)
/// are not modelled yet.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarkerAction {
    /// "The marker has no effect. Default type used where markers are just labels."
    #[default]
    None,
    /// "Playback and time = 0 starts from the set point."
    Start,
    /// "Playback will return to this marker when the end of all data in the Playlist is reached.
    /// ... Placing this marker type beyond the end of the last data will also cause FL Studio to
    /// render to that point."
    Loop,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Arrangement {
    pub id: ArrangementId,
    pub name: String,
    pub tracks: Vec<PlaylistTrack>,
    pub clips: Vec<ClipInstance>,
    pub markers: Vec<Marker>,
}

impl Arrangement {
    pub fn new(id: ArrangementId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            tracks: Vec::new(),
            clips: Vec::new(),
            markers: Vec::new(),
        }
    }
}

/// FX slots per mixer track [Manual: mixer, "Each Track has 10 effects slots"].
pub const MAX_FX_SLOTS: usize = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EqShape {
    Peak,
    LowShelf,
    HighShelf,
    LowPass,
    HighPass,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EqBand {
    pub shape: EqShape,
    pub freq_hz: u16,
    /// Q × 1000.
    pub q_milli: u16,
    /// Gain in tenths of a dB.
    pub gain_db_tenths: i16,
}

/// Built-in effects (our own designs). Integer fields keep the model `Eq` and diffable.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FxKind {
    Eq(Vec<EqBand>),
    Delay {
        /// Delay time in thousandths of a beat.
        time_beats_milli: u32,
        feedback: u16,
        mix: u16,
        ping_pong: bool,
    },
    Reverb {
        room: u16,
        damping: u16,
        width: u16,
        mix: u16,
    },
    Compressor {
        threshold_db_tenths: i16,
        ratio_tenths: u16,
        attack_ms: u16,
        release_ms: u16,
        makeup_db_tenths: i16,
    },
    Limiter {
        ceiling_db_tenths: i16,
        release_ms: u16,
    },
    Distortion {
        drive: u16,
        mix: u16,
    },
    Chorus {
        rate_milli_hz: u32,
        depth: u16,
        mix: u16,
    },
    Gate {
        threshold_db_tenths: i16,
        release_ms: u16,
    },
    Flanger {
        rate_milli_hz: u32,
        depth: u16,
        feedback: u16,
        mix: u16,
    },
    Phaser {
        rate_milli_hz: u32,
        depth: u16,
        feedback: u16,
        mix: u16,
    },
    /// 0 = low-pass, 1 = high-pass, 2 = band-pass.
    Filter {
        mode: u8,
        cutoff_hz: u16,
        resonance: u16,
    },
}

impl FxKind {
    /// Stable identifiers used by commands and the UI.
    pub const NAMES: [&'static str; 11] = [
        "eq",
        "delay",
        "reverb",
        "compressor",
        "limiter",
        "distortion",
        "chorus",
        "gate",
        "flanger",
        "phaser",
        "filter",
    ];

    pub fn default_for(name: &str) -> Option<Self> {
        Some(match name {
            "eq" => Self::Eq(vec![
                EqBand {
                    shape: EqShape::LowShelf,
                    freq_hz: 120,
                    q_milli: 707,
                    gain_db_tenths: 0,
                },
                EqBand {
                    shape: EqShape::Peak,
                    freq_hz: 1000,
                    q_milli: 1000,
                    gain_db_tenths: 0,
                },
                EqBand {
                    shape: EqShape::HighShelf,
                    freq_hz: 8000,
                    q_milli: 707,
                    gain_db_tenths: 0,
                },
            ]),
            "delay" => Self::Delay {
                time_beats_milli: 750,
                feedback: 400,
                mix: 300,
                ping_pong: true,
            },
            "reverb" => Self::Reverb {
                room: 600,
                damping: 500,
                width: 1000,
                mix: 250,
            },
            "compressor" => Self::Compressor {
                threshold_db_tenths: -180,
                ratio_tenths: 40,
                attack_ms: 10,
                release_ms: 120,
                makeup_db_tenths: 0,
            },
            "limiter" => Self::Limiter {
                ceiling_db_tenths: -3,
                release_ms: 80,
            },
            "distortion" => Self::Distortion {
                drive: 400,
                mix: 1000,
            },
            "chorus" => Self::Chorus {
                rate_milli_hz: 800,
                depth: 500,
                mix: 500,
            },
            "gate" => Self::Gate {
                threshold_db_tenths: -500,
                release_ms: 80,
            },
            "flanger" => Self::Flanger {
                rate_milli_hz: 250,
                depth: 600,
                feedback: 500,
                mix: 500,
            },
            "phaser" => Self::Phaser {
                rate_milli_hz: 400,
                depth: 700,
                feedback: 400,
                mix: 500,
            },
            "filter" => Self::Filter {
                mode: 0,
                cutoff_hz: 2000,
                resonance: 300,
            },
            _ => return None,
        })
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Eq(_) => "eq",
            Self::Delay { .. } => "delay",
            Self::Reverb { .. } => "reverb",
            Self::Compressor { .. } => "compressor",
            Self::Limiter { .. } => "limiter",
            Self::Distortion { .. } => "distortion",
            Self::Chorus { .. } => "chorus",
            Self::Gate { .. } => "gate",
            Self::Flanger { .. } => "flanger",
            Self::Phaser { .. } => "phaser",
            Self::Filter { .. } => "filter",
        }
    }

    fn valid(&self) -> bool {
        match self {
            Self::Eq(bands) => {
                bands.len() <= 16
                    && bands.iter().all(|band| {
                        (10..=22_000).contains(&band.freq_hz)
                            && (50..=40_000).contains(&band.q_milli)
                            && i32::from(band.gain_db_tenths).abs() <= 240
                    })
            }
            Self::Delay {
                time_beats_milli,
                feedback,
                mix,
                ..
            } => (1..=16_000).contains(time_beats_milli) && *feedback <= 950 && *mix <= 1000,
            Self::Reverb {
                room,
                damping,
                width,
                mix,
            } => *room <= 1000 && *damping <= 1000 && *width <= 1000 && *mix <= 1000,
            Self::Compressor {
                threshold_db_tenths,
                ratio_tenths,
                attack_ms,
                release_ms,
                makeup_db_tenths,
            } => {
                (-600..=0).contains(threshold_db_tenths)
                    && (10..=1000).contains(ratio_tenths)
                    && *attack_ms <= 1000
                    && *release_ms <= 5000
                    && (0..=360).contains(makeup_db_tenths)
            }
            Self::Limiter {
                ceiling_db_tenths,
                release_ms,
            } => (-300..=0).contains(ceiling_db_tenths) && *release_ms <= 5000,
            Self::Distortion { drive, mix } => *drive <= 1000 && *mix <= 1000,
            Self::Chorus {
                rate_milli_hz,
                depth,
                mix,
            } => (10..=10_000).contains(rate_milli_hz) && *depth <= 1000 && *mix <= 1000,
            Self::Gate {
                threshold_db_tenths,
                release_ms,
            } => (-900..=0).contains(threshold_db_tenths) && *release_ms <= 5000,
            Self::Flanger {
                rate_milli_hz,
                depth,
                feedback,
                mix,
            }
            | Self::Phaser {
                rate_milli_hz,
                depth,
                feedback,
                mix,
            } => {
                (10..=10_000).contains(rate_milli_hz)
                    && *depth <= 1000
                    && *feedback <= 950
                    && *mix <= 1000
            }
            Self::Filter {
                mode,
                cutoff_hz,
                resonance,
            } => *mode <= 2 && (20..=20_000).contains(cutoff_hz) && *resonance <= 1000,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FxSlot {
    pub kind: FxKind,
    pub enabled: bool,
    /// Wet/dry mix 0..=1000 [Manual: mixer, "FX Level"].
    pub mix: u16,
}

/// A post-fader send to another insert [Manual: mixer, "Post-fader Sends (default)"].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MixerSend {
    /// Destination insert, 1..=500.
    pub to: u16,
    /// 0..=1000; zero keeps a sidechain-only link [Manual: mixer, "Sidechain Sends"].
    pub level: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MixerInsert {
    pub name: String,
    /// Fader position 0..=1000; [`MIXER_UNITY`] is 0 dB.
    #[serde(default = "default_mixer_volume")]
    pub volume: u16,
    /// -1000..=1000.
    #[serde(default)]
    pub pan: i16,
    #[serde(default)]
    pub mute: bool,
    /// -1000 (wider) ..= 1000 (mono) [Manual: mixer, "Stereo Separation"].
    #[serde(default)]
    pub stereo_sep: i16,
    #[serde(default)]
    pub invert_phase: bool,
    #[serde(default)]
    pub swap_lr: bool,
    /// Inserts reach the Master unless this is off [Manual: mixer, "Finally the audio is sent to the Master"].
    #[serde(default = "default_true")]
    pub to_master: bool,
    #[serde(default)]
    pub sends: Vec<MixerSend>,
    /// Effects in processing order; at most [`MAX_FX_SLOTS`].
    #[serde(default)]
    pub fx: Vec<FxSlot>,
    /// Master switch for all effects on the track [Manual: mixer, "Mixer Track FX master switch"].
    #[serde(default = "default_true")]
    pub fx_enabled: bool,
    /// Integrated parametric EQ: "Band 1 - Low Shelf; Band 2 - Peaking; 3 - High Shelf"
    /// [Manual: mixer_trackprops]. Flat (0 dB) bands do nothing.
    #[serde(default = "default_track_eq")]
    pub eq: [EqBand; 3],
}

/// Default track EQ bands (hypothesis H-MIX-3: frequencies and Q of FL's integrated EQ are not
/// documented; the same as our EQ effect's defaults).
pub fn default_track_eq() -> [EqBand; 3] {
    [
        EqBand {
            shape: EqShape::LowShelf,
            freq_hz: 120,
            q_milli: 707,
            gain_db_tenths: 0,
        },
        EqBand {
            shape: EqShape::Peak,
            freq_hz: 1000,
            q_milli: 1000,
            gain_db_tenths: 0,
        },
        EqBand {
            shape: EqShape::HighShelf,
            freq_hz: 8000,
            q_milli: 707,
            gain_db_tenths: 0,
        },
    ]
}

fn default_mixer_volume() -> u16 {
    MIXER_UNITY
}

fn default_true() -> bool {
    true
}

impl MixerInsert {
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            volume: MIXER_UNITY,
            pan: 0,
            mute: false,
            stereo_sep: 0,
            invert_phase: false,
            swap_lr: false,
            to_master: true,
            sends: Vec::new(),
            fx: Vec::new(),
            fx_enabled: true,
            eq: default_track_eq(),
        }
    }

    /// The default insert at 1-based mixer index `index`.
    pub fn default_for(index: u16) -> Self {
        Self::named(format!("Insert {index}"))
    }

    fn valid(&self) -> bool {
        self.name.len() <= 4096
            && self.volume <= 1000
            && i32::from(self.pan).abs() <= 1000
            && i32::from(self.stereo_sep).abs() <= 1000
            && self.sends.len() <= MAX_INSERTS
            && self
                .sends
                .iter()
                .all(|send| send.level <= 1000 && (1..=MAX_INSERTS as u16).contains(&send.to))
            && self.fx.len() <= MAX_FX_SLOTS
            && self
                .fx
                .iter()
                .all(|slot| slot.mix <= 1000 && slot.kind.valid())
            && self.eq.iter().all(|band| {
                (20..=20_000).contains(&band.freq_hz)
                    && (100..=10_000).contains(&band.q_milli)
                    && i32::from(band.gain_db_tenths).abs() <= 180
            })
    }
}

/// `inserts[i]` is insert `i + 1`; indices past the end behave as [`MixerInsert::default_for`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mixer {
    pub master: MixerInsert,
    pub current: MixerInsert,
    pub inserts: Vec<MixerInsert>,
}

impl Default for Mixer {
    fn default() -> Self {
        Self {
            master: MixerInsert::named("Master"),
            current: MixerInsert::named("Current"),
            inserts: Vec::new(),
        }
    }
}

impl Mixer {
    /// Track by mixer index: 0 is Master, 1..=500 are inserts.
    pub fn track(&self, index: u16) -> Option<Cow<'_, MixerInsert>> {
        match index {
            0 => Some(Cow::Borrowed(&self.master)),
            i if usize::from(i) <= MAX_INSERTS => Some(
                self.inserts
                    .get(usize::from(i) - 1)
                    .map_or_else(|| Cow::Owned(MixerInsert::default_for(i)), Cow::Borrowed),
            ),
            _ => None,
        }
    }

    /// Mutable track by mixer index, materializing default inserts up to `index`.
    pub fn track_mut(&mut self, index: u16) -> Option<&mut MixerInsert> {
        match index {
            0 => Some(&mut self.master),
            i if usize::from(i) <= MAX_INSERTS => {
                while self.inserts.len() < usize::from(i) {
                    let next = self.inserts.len() as u16 + 1;
                    self.inserts.push(MixerInsert::default_for(next));
                }
                self.inserts.get_mut(usize::from(i) - 1)
            }
            _ => None,
        }
    }

    /// True when a send path leads from insert `from` back to itself through `to`.
    pub fn would_cycle(&self, from: u16, to: u16) -> bool {
        let mut stack = vec![to];
        let mut seen = BTreeSet::new();
        while let Some(index) = stack.pop() {
            if index == from {
                return true;
            }
            if !seen.insert(index) {
                continue;
            }
            if let Some(track) = self.inserts.get(usize::from(index).wrapping_sub(1)) {
                stack.extend(track.sends.iter().map(|send| send.to));
            }
        }
        false
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceStatus {
    Available,
    Missing,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resource {
    pub id: ResourceId,
    pub path_rel: String,
    pub path_abs_hint: Option<String>,
    pub hash: Option<String>,
    pub status: ResourceStatus,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub settings: ProjectSettings,
    pub channels: Vec<Channel>,
    pub patterns: Vec<Pattern>,
    pub arrangements: Vec<Arrangement>,
    pub active_arrangement: ArrangementId,
    pub mixer: Mixer,
    pub resources: Vec<Resource>,
    pub ids: IdAllocator,
    #[serde(default)]
    pub automations: Vec<AutomationClip>,
}

impl Project {
    pub fn new() -> Self {
        let mut ids = IdAllocator::default();
        let pattern_id = ids.pattern();
        let arrangement_id = ids.arrangement();
        Self {
            settings: ProjectSettings::default(),
            channels: Vec::new(),
            patterns: vec![Pattern::new(pattern_id, "Pattern 1")],
            arrangements: vec![Arrangement::new(arrangement_id, "Arrangement")],
            active_arrangement: arrangement_id,
            mixer: Mixer::default(),
            resources: Vec::new(),
            ids,
            automations: Vec::new(),
        }
    }

    pub fn automation(&self, id: AutomationId) -> Option<&AutomationClip> {
        self.automations.iter().find(|a| a.id == id)
    }

    pub fn channel(&self, id: ChannelId) -> Option<&Channel> {
        self.channels.iter().find(|channel| channel.id == id)
    }

    pub fn pattern(&self, id: PatternId) -> Option<&Pattern> {
        self.patterns.iter().find(|pattern| pattern.id == id)
    }

    pub fn active(&self) -> Option<&Arrangement> {
        self.arrangements
            .iter()
            .find(|arrangement| arrangement.id == self.active_arrangement)
    }

    pub fn validate(&self) -> Result<(), ModelError> {
        self.settings.ppq.validate()?;
        self.settings.tempo.validate()?;
        self.settings.time_sig.validate()?;
        if self.settings.steps_per_beat == 0 {
            return Err(ModelError::InvalidValue("steps_per_beat"));
        }
        if self.settings.swing > 100
            || self.settings.main_volume > 1000
            || i32::from(self.settings.main_pitch).abs() > 1000
        {
            return Err(ModelError::InvalidValue("project parameter"));
        }
        if self.channels.len() > MAX_CHANNELS {
            return Err(ModelError::Limit("channels"));
        }
        if self.patterns.len() > MAX_PATTERNS {
            return Err(ModelError::Limit("patterns"));
        }
        if self.arrangements.is_empty() {
            return Err(ModelError::Missing("arrangement"));
        }
        if self.mixer.inserts.len() > MAX_INSERTS {
            return Err(ModelError::Limit("mixer inserts"));
        }
        if !self.mixer.master.valid()
            || !self.mixer.current.valid()
            || !self.mixer.inserts.iter().all(MixerInsert::valid)
        {
            return Err(ModelError::InvalidValue("mixer track"));
        }
        for (index, insert) in self.mixer.inserts.iter().enumerate() {
            let from = index as u16 + 1;
            if insert
                .sends
                .iter()
                .any(|send| send.to == from || self.mixer.would_cycle(from, send.to))
            {
                return Err(ModelError::InvalidValue("mixer routing cycle"));
            }
        }
        if self.resources.len() > MAX_RESOURCES {
            return Err(ModelError::Limit("resources"));
        }

        let channels = unique_ids(self.channels.iter().map(|item| item.id), "channel")?;
        let patterns = unique_ids(self.patterns.iter().map(|item| item.id), "pattern")?;
        let arrangements = unique_ids(self.arrangements.iter().map(|item| item.id), "arrangement")?;
        let resources = unique_ids(self.resources.iter().map(|item| item.id), "resource")?;
        let automations = unique_ids(self.automations.iter().map(|item| item.id), "automation")?;
        for clip in &self.automations {
            if clip.points.is_empty()
                || clip.name.len() > 4096
                || clip.initial > 1000
                || clip.points.windows(2).any(|w| w[1].at < w[0].at)
                || clip
                    .points
                    .iter()
                    .any(|p| p.at.0 < 0 || p.value > 1000 || i32::from(p.tension).abs() > 1000)
            {
                return Err(ModelError::InvalidValue("automation clip"));
            }
            if clip.target.current(self).is_none() {
                return Err(ModelError::Orphan("automation target"));
            }
        }
        if !arrangements.contains(&self.active_arrangement) {
            return Err(ModelError::Orphan("active arrangement"));
        }

        for channel in &self.channels {
            if channel.name.len() > 4096
                || channel.volume > 1000
                || i32::from(channel.pan).abs() > 1000
                || usize::from(channel.mixer_target) > MAX_INSERTS
                || i32::from(channel.pitch).abs() > 4800
                || channel.root_key > MAX_KEY
                || channel.swing_mix > 1000
                || !channel.sampler.valid()
                || channel.polyphony.max > 256
            {
                return Err(ModelError::InvalidValue("channel"));
            }
            match &channel.kind {
                ChannelKind::Sampler { sample } | ChannelKind::AudioClip { sample } => {
                    if sample.is_some_and(|id| !resources.contains(&id)) {
                        return Err(ModelError::Orphan("channel sample"));
                    }
                }
                ChannelKind::Synth(params) => {
                    if !params.valid() {
                        return Err(ModelError::InvalidValue("synth parameters"));
                    }
                }
            }
        }
        for pattern in &self.patterns {
            if pattern.name.len() > 4096 {
                return Err(ModelError::InvalidValue("pattern name"));
            }
            if let PatternLength::Steps(steps) = pattern.length
                && (steps == 0 || usize::from(steps) > MAX_STEPS)
            {
                return Err(ModelError::InvalidValue("pattern length"));
            }
            for (channel, sequence) in &pattern.steps {
                if !channels.contains(channel) {
                    return Err(ModelError::Orphan("pattern channel"));
                }
                if sequence.cells.len() > MAX_STEPS {
                    return Err(ModelError::Limit("steps"));
                }
                if sequence.cells.iter().any(|cell| !cell.props.valid()) {
                    return Err(ModelError::InvalidValue("step properties"));
                }
            }
            for (channel, notes) in &pattern.notes {
                if !channels.contains(channel) {
                    return Err(ModelError::Orphan("pattern channel"));
                }
                if notes.len() > MAX_NOTES_PER_SEQUENCE {
                    return Err(ModelError::Limit("notes"));
                }
                if notes.iter().any(|note| !note.valid()) {
                    return Err(ModelError::InvalidValue("note"));
                }
            }
        }
        let mut all_tracks = BTreeSet::new();
        let mut all_clips = BTreeSet::new();
        for arrangement in &self.arrangements {
            if arrangement.tracks.len() > MAX_TRACKS {
                return Err(ModelError::Limit("tracks"));
            }
            if arrangement.clips.len() > MAX_CLIPS {
                return Err(ModelError::Limit("clips"));
            }
            let tracks = unique_ids(arrangement.tracks.iter().map(|item| item.id), "track")?;
            for track in &arrangement.tracks {
                if !all_tracks.insert(track.id) {
                    return Err(ModelError::Duplicate("track"));
                }
                if track.height == 0 || track.name.len() > 4096 {
                    return Err(ModelError::InvalidValue("track"));
                }
            }
            for clip in &arrangement.clips {
                if !all_clips.insert(clip.id) {
                    return Err(ModelError::Duplicate("clip"));
                }
                if !tracks.contains(&clip.track) {
                    return Err(ModelError::Orphan("clip track"));
                }
                if clip.start.0 < 0 || clip.length.0 <= 0 || clip.offset.0 < 0 {
                    return Err(ModelError::InvalidValue("clip time"));
                }
                match clip.source {
                    ClipSource::Pattern(id) if !patterns.contains(&id) => {
                        return Err(ModelError::Orphan("clip pattern"));
                    }
                    ClipSource::Automation(id) if !automations.contains(&id) => {
                        return Err(ModelError::Orphan("clip automation"));
                    }
                    ClipSource::Audio(id)
                        if !self
                            .channel(id)
                            .is_some_and(|c| matches!(c.kind, ChannelKind::AudioClip { .. })) =>
                    {
                        return Err(ModelError::Orphan("clip audio channel"));
                    }
                    ClipSource::Pattern(_) | ClipSource::Automation(_) | ClipSource::Audio(_) => {}
                }
            }
            for marker in &arrangement.markers {
                if marker.at.0 < 0 {
                    return Err(ModelError::InvalidValue("marker time"));
                }
            }
        }
        check_counter(self.ids.next_channel, &channels, "channel allocator")?;
        check_counter(self.ids.next_pattern, &patterns, "pattern allocator")?;
        check_counter(
            self.ids.next_arrangement,
            &arrangements,
            "arrangement allocator",
        )?;
        check_counter(self.ids.next_track, &all_tracks, "track allocator")?;
        check_counter(self.ids.next_clip, &all_clips, "clip allocator")?;
        check_counter(self.ids.next_resource, &resources, "resource allocator")?;
        check_counter(
            self.ids.next_automation,
            &automations,
            "automation allocator",
        )?;
        Ok(())
    }

    /// Rescale all stored tick positions when changing project resolution.
    pub fn set_ppq(&mut self, next: Ppq) -> Result<(), ModelError> {
        next.validate()?;
        self.validate()?;
        let old = self.settings.ppq;
        if old == next {
            return Ok(());
        }
        let mut candidate = self.clone();
        for arrangement in &mut candidate.arrangements {
            for clip in &mut arrangement.clips {
                clip.start = rescale_tick(clip.start, old, next);
                clip.length = Tick(rescale_tick(clip.length, old, next).0.max(1));
                clip.offset = rescale_tick(clip.offset, old, next);
            }
            for marker in &mut arrangement.markers {
                marker.at = rescale_tick(marker.at, old, next);
            }
        }
        for clip in &mut candidate.automations {
            for point in &mut clip.points {
                point.at = rescale_tick(point.at, old, next);
            }
        }
        for pattern in &mut candidate.patterns {
            for notes in pattern.notes.values_mut() {
                for note in notes.iter_mut() {
                    note.start = rescale_tick(note.start, old, next);
                    note.length = rescale_tick(note.length, old, next);
                }
            }
        }
        candidate.settings.ppq = next;
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }
}

impl Default for Project {
    fn default() -> Self {
        Self::new()
    }
}

pub fn validate(project: &Project) -> Result<(), ModelError> {
    project.validate()
}

fn unique_ids<T: Ord + Copy>(
    ids: impl Iterator<Item = T>,
    kind: &'static str,
) -> Result<BTreeSet<T>, ModelError> {
    let mut found = BTreeSet::new();
    for id in ids {
        if !found.insert(id) {
            return Err(ModelError::Duplicate(kind));
        }
    }
    Ok(found)
}

fn check_counter<T: Ord + Copy + Into<u64>>(
    next: u64,
    ids: &BTreeSet<T>,
    kind: &'static str,
) -> Result<(), ModelError> {
    if next == 0 || ids.iter().any(|id| next <= (*id).into()) {
        return Err(ModelError::InvalidValue(kind));
    }
    Ok(())
}

macro_rules! id_into_u64 {
    ($($name:ident),+ $(,)?) => { $(impl From<$name> for u64 { fn from(id: $name) -> Self { id.0 } })+ };
}
id_into_u64!(
    ChannelId,
    PatternId,
    ArrangementId,
    TrackId,
    ClipId,
    ResourceId,
    AutomationId
);

fn rescale_tick(tick: Tick, old: Ppq, next: Ppq) -> Tick {
    let numerator = i128::from(tick.0) * i128::from(next.0);
    let denominator = i128::from(old.0);
    let rounded = if numerator >= 0 {
        (numerator + denominator / 2) / denominator
    } else {
        -((-numerator + denominator / 2) / denominator)
    };
    Tick(rounded.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64)
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ModelError {
    #[error("duplicate {0} id")]
    Duplicate(&'static str),
    #[error("orphaned {0} reference")]
    Orphan(&'static str),
    #[error("missing {0}")]
    Missing(&'static str),
    #[error("{0} exceeds the supported limit")]
    Limit(&'static str),
    #[error("invalid {0}")]
    InvalidValue(&'static str),
    #[error("invalid musical time: {0}")]
    Time(#[from] TimeError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polyphony_from_older_files_gets_porta_off_and_the_default_slide_time() {
        let old: Polyphony = serde_json::from_str(r#"{"max":4,"mono":true}"#).unwrap();
        assert_eq!(
            old,
            Polyphony {
                max: 4,
                mono: true,
                porta: false,
                slide_ms: DEFAULT_SLIDE_MS,
            }
        );
    }

    #[test]
    fn project_starts_valid_and_ids_are_monotonic() {
        let mut project = Project::new();
        assert_eq!(project.validate(), Ok(()));
        let first = project.ids.channel();
        let second = project.ids.channel();
        assert!(second > first);
    }

    #[test]
    fn invalid_extreme_pan_returns_error_instead_of_panicking() {
        let mut project = Project::new();
        let id = project.ids.channel();
        let mut channel = Channel::sampler(id, "Channel");
        channel.pan = i16::MIN;
        project.channels.push(channel);
        assert_eq!(project.validate(), Err(ModelError::InvalidValue("channel")));
    }

    #[test]
    fn rejects_orphaned_pattern_channel_and_clip() {
        let mut project = Project::new();
        project.patterns[0]
            .steps
            .insert(ChannelId(999), StepSeq::default());
        assert_eq!(
            project.validate(),
            Err(ModelError::Orphan("pattern channel"))
        );
        project.patterns[0].steps.clear();
        project.patterns[0]
            .notes
            .insert(ChannelId(999), vec![Note::new(Tick(0), Tick(24), 60)]);
        assert_eq!(
            project.validate(),
            Err(ModelError::Orphan("pattern channel"))
        );
        project.patterns[0].notes.clear();
        let track = project.ids.track();
        let clip = project.ids.clip();
        project.arrangements[0]
            .tracks
            .push(PlaylistTrack::new(track, "Track"));
        project.arrangements[0].clips.push(ClipInstance::pattern(
            clip,
            track,
            PatternId(999),
            Tick(0),
            Tick(96),
        ));
        assert_eq!(project.validate(), Err(ModelError::Orphan("clip pattern")));
    }

    #[test]
    fn reducing_ppq_rescales_clip_and_rounds_to_new_grid() {
        let mut project = Project::new();
        let track = project.ids.track();
        let clip = project.ids.clip();
        project.arrangements[0]
            .tracks
            .push(PlaylistTrack::new(track, "Track"));
        project.arrangements[0].clips.push(ClipInstance::pattern(
            clip,
            track,
            project.patterns[0].id,
            Tick(95),
            Tick(97),
        ));
        assert_eq!(project.set_ppq(Ppq(48)), Ok(()));
        assert_eq!(project.arrangements[0].clips[0].start, Tick(48));
        assert_eq!(project.arrangements[0].clips[0].length, Tick(49));
    }

    #[test]
    fn ppq_change_rescales_piano_roll_notes() {
        let mut project = Project::new();
        let id = project.ids.channel();
        project.channels.push(Channel::synth(id, "Synth"));
        project.patterns[0]
            .notes
            .insert(id, vec![Note::new(Tick(96), Tick(48), 60)]);
        assert_eq!(project.set_ppq(Ppq(192)), Ok(()));
        let note = &project.patterns[0].notes[&id][0];
        assert_eq!((note.start, note.length), (Tick(192), Tick(96)));
    }

    #[test]
    fn mixer_tracks_default_lazily_and_reject_cycles() {
        let mut project = Project::new();
        assert_eq!(
            project.mixer.track(7).map(|t| t.name.clone()),
            Some("Insert 7".to_owned())
        );
        assert!(project.mixer.track(501).is_none());
        let Some(one) = project.mixer.track_mut(1) else {
            panic!("insert 1 exists");
        };
        one.sends.push(MixerSend { to: 2, level: 800 });
        assert_eq!(project.validate(), Ok(()));
        let Some(two) = project.mixer.track_mut(2) else {
            panic!("insert 2 exists");
        };
        two.sends.push(MixerSend { to: 1, level: 800 });
        assert_eq!(
            project.validate(),
            Err(ModelError::InvalidValue("mixer routing cycle"))
        );
    }

    #[test]
    fn version_one_documents_without_new_fields_still_parse() {
        let json = r#"{"id":1,"name":"Kick","color":null,"kind":{"Sampler":{"sample":null}},
            "mute":false,"volume":1000,"pan":0,"mixer_target":0,"group":null}"#;
        let channel: Channel = serde_json::from_str(json).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(channel.root_key, DEFAULT_ROOT_KEY);
        assert_eq!(channel.swing_mix, 1000);
    }

    #[test]
    fn step_ticks_follow_ppq_and_steps_per_beat() {
        let settings = ProjectSettings::default();
        assert_eq!(settings.step_ticks(), Some(24));
        assert_eq!(settings.bar_ticks(), Some(384));
    }
}
