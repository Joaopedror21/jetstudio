#![forbid(unsafe_code)]

//! Persistent project data. Selection, scroll, focus and open windows belong to the UI.

use jet_time::{Ppq, Tempo, Tick, TimeError, TimeSig};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const MAX_CHANNELS: usize = 10_000; // Our file-input safety limit; no FL limit claimed.
pub const MAX_PATTERNS: usize = 999;
pub const MAX_STEPS: usize = 512;
pub const MAX_INSERTS: usize = 500;
pub const MAX_TRACKS: usize = 10_000; // Our file-input safety limit.
pub const MAX_CLIPS: usize = 1_000_000; // Our file-input safety limit.
pub const MAX_RESOURCES: usize = 100_000; // Our file-input safety limit.

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

/// Counters advance monotonically and are serialized with the project.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdAllocator {
    pub next_channel: u64,
    pub next_pattern: u64,
    pub next_arrangement: u64,
    pub next_track: u64,
    pub next_clip: u64,
    pub next_resource: u64,
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

    pub fn clip(&mut self) -> ClipId {
        ClipId(Self::allocate(&mut self.next_clip))
    }

    pub fn resource(&mut self) -> ResourceId {
        ResourceId(Self::allocate(&mut self.next_resource))
    }
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
}

impl Default for ProjectSettings {
    fn default() -> Self {
        Self {
            ppq: Ppq::DEFAULT,
            tempo: Tempo::DEFAULT,
            time_sig: TimeSig::default(),
            steps_per_beat: 4,
            swing: 0,
            main_volume: 1000,
            main_pitch: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChannelKind {
    Sampler { sample: Option<ResourceId> },
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
}

impl Channel {
    pub fn sampler(id: ChannelId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            color: None,
            kind: ChannelKind::Sampler { sample: None },
            mute: false,
            volume: 1000,
            pan: 0,
            mixer_target: 0,
            group: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatternLength {
    Auto,
    Steps(u16),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepProps {
    pub velocity: u8,
    pub pan: i16,
    pub release: u8,
    pub pitch: i16,
    pub mod_x: u8,
    pub mod_y: u8,
    pub shift: i16,
}

impl Default for StepProps {
    fn default() -> Self {
        Self {
            velocity: 100,
            pan: 0,
            release: 0,
            pitch: 0,
            mod_x: 0,
            mod_y: 0,
            shift: 0,
        }
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pattern {
    pub id: PatternId,
    pub name: String,
    pub color: Option<u32>,
    pub length: PatternLength,
    pub steps: BTreeMap<ChannelId, StepSeq>,
}

impl Pattern {
    pub fn new(id: PatternId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            color: None,
            length: PatternLength::Auto,
            steps: BTreeMap::new(),
        }
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
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Marker {
    pub at: Tick,
    pub name: String,
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MixerInsert {
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mixer {
    pub master: MixerInsert,
    pub current: MixerInsert,
    pub inserts: Vec<MixerInsert>,
}

impl Default for Mixer {
    fn default() -> Self {
        Self {
            master: MixerInsert {
                name: "Master".to_owned(),
            },
            current: MixerInsert {
                name: "Current".to_owned(),
            },
            inserts: Vec::new(),
        }
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
            arrangements: vec![Arrangement::new(arrangement_id, "Arrangement 1")],
            active_arrangement: arrangement_id,
            mixer: Mixer::default(),
            resources: Vec::new(),
            ids,
        }
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
        if self.resources.len() > MAX_RESOURCES {
            return Err(ModelError::Limit("resources"));
        }

        let channels = unique_ids(self.channels.iter().map(|item| item.id), "channel")?;
        let patterns = unique_ids(self.patterns.iter().map(|item| item.id), "pattern")?;
        let arrangements = unique_ids(self.arrangements.iter().map(|item| item.id), "arrangement")?;
        let resources = unique_ids(self.resources.iter().map(|item| item.id), "resource")?;
        if !arrangements.contains(&self.active_arrangement) {
            return Err(ModelError::Orphan("active arrangement"));
        }

        for channel in &self.channels {
            if channel.name.len() > 4096
                || channel.volume > 1000
                || i32::from(channel.pan).abs() > 1000
                || usize::from(channel.mixer_target) > MAX_INSERTS
            {
                return Err(ModelError::InvalidValue("channel"));
            }
            let ChannelKind::Sampler { sample } = &channel.kind;
            if sample.is_some_and(|id| !resources.contains(&id)) {
                return Err(ModelError::Orphan("channel sample"));
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
                    ClipSource::Pattern(_) => {}
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
    ResourceId
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
}
