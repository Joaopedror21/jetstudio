#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

//! Compiles a [`Project`] into a sorted list of note events in project ticks.
//!
//! This crate is pure: no audio, no I/O, no clocks. Playback and offline
//! rendering consume the same [`Sequence`], which is what makes the bounce
//! identical to live playback.

pub mod automation;

use jet_model::{
    ChannelId, ClipSource, Pattern, PatternId, PatternLength, Project, ProjectSettings,
};

/// Fraction of a step by which full swing delays even steps.
/// Hypothesis H-SEQ-1: the manual says swing "holds odd 16th notes at the expense of even
/// 16th notes" [Manual: channelrack, "Global Swing"] but does not give the maximum offset.
pub const SWING_MAX_STEP_FRACTION: (i64, i64) = (1, 2);

/// One note to be played. `tick` is absolute within the compiled sequence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoteEvent {
    pub tick: i64,
    /// Length in ticks; zero is a step-style note (see [`Sequence::gate_for`]).
    pub length: i64,
    pub channel: ChannelId,
    pub key: u8,
    pub velocity: u8,
    pub pan: i16,
    pub fine_pitch: i16,
    pub release: u8,
    pub mod_x: u8,
    pub mod_y: u8,
    pub slide: bool,
    pub porta: bool,
    /// Audio Clip: the voice starts `audio_offset` ticks into the sample and is cut (with the
    /// declick) at the end of `length` instead of being released.
    pub audio: bool,
    pub audio_offset: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// Pattern mode: the selected pattern loops [Manual: toolbar_panels, PAT/SONG].
    Pattern(PatternId),
    /// Song mode: the active arrangement's Pattern Clips.
    Song,
}

/// Events sorted by tick, plus the loop length of the material.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Sequence {
    pub events: Vec<NoteEvent>,
    /// Loop length in ticks (always > 0).
    pub length: i64,
    /// Ticks in one step, used for step-style note gates.
    pub step_ticks: i64,
}

impl Sequence {
    /// Gate length for a note: zero-length (step) notes hold for one step.
    /// Hypothesis H-SEQ-2: the manual describes steps as zero-length notes but not their gate.
    pub fn gate_for(&self, event: &NoteEvent) -> i64 {
        if event.length > 0 {
            event.length
        } else {
            self.step_ticks.max(1)
        }
    }

    /// Index of the first event whose tick is `>= tick`.
    pub fn first_at_or_after(&self, tick: i64) -> usize {
        self.events.partition_point(|event| event.tick < tick)
    }
}

fn step_ticks(settings: &ProjectSettings) -> i64 {
    settings.step_ticks().unwrap_or(1).max(1)
}

fn bar_ticks(settings: &ProjectSettings) -> i64 {
    settings.bar_ticks().unwrap_or(1).max(1)
}

fn ceil_to(value: i64, unit: i64) -> i64 {
    if unit <= 0 {
        return value;
    }
    value.saturating_add(unit - 1) / unit * unit
}

/// Pattern length in ticks. `Auto` ends at "the end of the last bar with data in it"
/// [Manual: channelrack, "Pattern Length"]; an empty pattern lasts one bar (Hypothesis H-SEQ-3).
pub fn pattern_length(project: &Project, pattern: &Pattern) -> i64 {
    let settings = &project.settings;
    let step = step_ticks(settings);
    match pattern.length {
        PatternLength::Steps(steps) => i64::from(steps) * step,
        PatternLength::Auto => {
            let mut end = 0_i64;
            for seq in pattern.steps.values() {
                if let Some(last) = seq.cells.iter().rposition(|cell| cell.on) {
                    end = end.max((last as i64 + 1) * step);
                }
            }
            for notes in pattern.notes.values() {
                for note in notes {
                    end = end.max(note.start.0.saturating_add(note.length.0.max(1)));
                }
            }
            ceil_to(end.max(1), bar_ticks(settings))
        }
    }
}

/// Swing offset in ticks for step `index` of `channel` [Manual: channelrack, "Global Swing"]:
/// both the global and channel multipliers must be above zero, and only even steps
/// (2, 4, ... counted from one) move.
pub fn swing_offset(settings: &ProjectSettings, swing_mix: u16, index: usize) -> i64 {
    if index.is_multiple_of(2) || settings.swing == 0 || swing_mix == 0 {
        return 0;
    }
    let step = step_ticks(settings);
    let (num, den) = SWING_MAX_STEP_FRACTION;
    // swing: 0..=100 %, swing_mix: 0..=1000.
    step * num * i64::from(settings.swing) * i64::from(swing_mix) / (den * 100 * 1000)
}

/// Events of one pattern relative to its start, unsorted, skipping muted channels.
/// Piano roll notes replace the step sequence of a channel that has them
/// [Manual: channelrack, "This mini-note-preview replaces the steps when a Piano roll is active"].
pub fn pattern_events(project: &Project, pattern: &Pattern, out: &mut Vec<NoteEvent>) {
    let settings = &project.settings;
    let step = step_ticks(settings);
    let limit = match pattern.length {
        PatternLength::Steps(steps) => usize::from(steps),
        PatternLength::Auto => usize::MAX,
    };
    for channel in &project.channels {
        if channel.mute {
            continue; // Mute applies to Step Sequencer and Piano roll playback [Manual: channelrack].
        }
        if let Some(notes) = pattern.notes.get(&channel.id).filter(|n| !n.is_empty()) {
            for note in notes.iter().filter(|note| !note.muted) {
                out.push(NoteEvent {
                    tick: note.start.0,
                    length: note.length.0,
                    channel: channel.id,
                    key: note.key,
                    velocity: note.velocity,
                    pan: note.pan,
                    fine_pitch: note.fine_pitch,
                    release: note.release,
                    mod_x: note.mod_x,
                    mod_y: note.mod_y,
                    slide: note.slide,
                    porta: note.porta,
                    audio: false,
                    audio_offset: 0,
                });
            }
            continue;
        }
        let Some(seq) = pattern.steps.get(&channel.id) else {
            continue;
        };
        for (index, cell) in seq.cells.iter().enumerate().take(limit) {
            if !cell.on {
                continue;
            }
            let shift = i64::from(cell.props.shift).clamp(0, step - 1);
            out.push(NoteEvent {
                tick: index as i64 * step
                    + shift
                    + swing_offset(settings, channel.swing_mix, index),
                length: 0,
                channel: channel.id,
                key: cell.props.note,
                velocity: cell.props.velocity,
                pan: cell.props.pan,
                fine_pitch: cell.props.pitch,
                release: cell.props.release,
                mod_x: cell.props.mod_x,
                mod_y: cell.props.mod_y,
                slide: false,
                porta: false,
                audio: false,
                audio_offset: 0,
            });
        }
    }
}

/// End of the last bar containing clip data in the active arrangement, in ticks
/// [Manual: fformats_save_export, "The end of the last bar containing data"].
pub fn song_length(project: &Project) -> i64 {
    let end = project
        .active()
        .map(|arrangement| {
            arrangement
                .clips
                .iter()
                .map(|clip| clip.end().0)
                .max()
                .unwrap_or(0)
        })
        .unwrap_or(0);
    ceil_to(end.max(1), bar_ticks(&project.settings))
}

/// Compile the material that plays in `source`.
pub fn compile(project: &Project, source: Source) -> Sequence {
    let mut events = Vec::new();
    let length = match source {
        Source::Pattern(id) => match project.pattern(id) {
            Some(pattern) => {
                pattern_events(project, pattern, &mut events);
                let length = pattern_length(project, pattern);
                // Notes past an explicit length do not play in pattern mode.
                events.retain(|event| event.tick < length);
                length
            }
            None => bar_ticks(&project.settings),
        },
        Source::Song => {
            if let Some(arrangement) = project.active() {
                let mut scratch = Vec::new();
                for clip in &arrangement.clips {
                    let track_muted = arrangement
                        .tracks
                        .iter()
                        .find(|track| track.id == clip.track)
                        .is_none_or(|track| track.mute);
                    if clip.muted || track_muted {
                        continue;
                    }
                    let pattern_id = match clip.source {
                        ClipSource::Pattern(id) => id,
                        ClipSource::Audio(channel_id) => {
                            if let Some(channel) = project.channel(channel_id).filter(|c| !c.mute) {
                                events.push(NoteEvent {
                                    tick: clip.start.0,
                                    length: clip.length.0,
                                    channel: channel_id,
                                    key: channel.root_key,
                                    velocity: jet_model::DEFAULT_VELOCITY,
                                    pan: 0,
                                    fine_pitch: 0,
                                    release: 0,
                                    mod_x: 0,
                                    mod_y: 0,
                                    slide: false,
                                    porta: false,
                                    audio: true,
                                    audio_offset: clip.offset.0,
                                });
                            }
                            continue;
                        }
                        ClipSource::Automation(_) => continue,
                    };
                    let Some(pattern) = project.pattern(pattern_id) else {
                        continue;
                    };
                    scratch.clear();
                    pattern_events(project, pattern, &mut scratch);
                    // A clip is a window over the pattern data [Manual: playlist, "Resizing & Slip-editing Clips"].
                    let window = clip.offset.0..clip.offset.0.saturating_add(clip.length.0);
                    for event in &scratch {
                        if window.contains(&event.tick) {
                            let mut placed = *event;
                            placed.tick = clip.start.0 + (event.tick - clip.offset.0);
                            events.push(placed);
                        }
                    }
                }
            }
            song_length(project)
        }
    };
    events.sort_by_key(|event| (event.tick, event.channel, event.key));
    Sequence {
        events,
        length: length.max(1),
        step_ticks: step_ticks(&project.settings),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jet_model::{Channel, ClipInstance, Note, PlaylistTrack, StepSeq};
    use jet_time::Tick;

    fn project_with_kick() -> (Project, ChannelId) {
        let mut project = Project::new();
        let id = project.ids.channel();
        project.channels.push(Channel::sampler(id, "Kick"));
        let mut seq = StepSeq::with_len(16).unwrap();
        for index in [0, 4, 8, 12] {
            seq.cells[index].on = true;
        }
        project.patterns[0].steps.insert(id, seq);
        (project, id)
    }

    #[test]
    fn four_on_the_floor_lands_on_beats_of_one_bar() {
        let (project, _) = project_with_kick();
        let sequence = compile(&project, Source::Pattern(project.patterns[0].id));
        let ticks: Vec<i64> = sequence.events.iter().map(|e| e.tick).collect();
        assert_eq!(ticks, vec![0, 96, 192, 288]);
        assert_eq!(sequence.length, 384);
        assert_eq!(sequence.gate_for(&sequence.events[0]), 24);
    }

    #[test]
    fn auto_length_rounds_to_bars_and_explicit_length_truncates() {
        let (mut project, id) = project_with_kick();
        let seq = project.patterns[0].steps.get_mut(&id).unwrap();
        seq.cells.resize(20, Default::default());
        seq.cells[17].on = true;
        assert_eq!(pattern_length(&project, &project.patterns[0]), 768);
        project.patterns[0].length = PatternLength::Steps(8);
        let sequence = compile(&project, Source::Pattern(project.patterns[0].id));
        assert_eq!(sequence.length, 192);
        assert_eq!(sequence.events.len(), 2);
    }

    #[test]
    fn swing_needs_both_multipliers_and_moves_only_even_steps() {
        let (mut project, id) = project_with_kick();
        let seq = project.patterns[0].steps.get_mut(&id).unwrap();
        seq.cells[1].on = true;
        project.settings.swing = 100;
        let sequence = compile(&project, Source::Pattern(project.patterns[0].id));
        let second = sequence.events.iter().find(|e| e.tick > 0 && e.tick < 96);
        assert_eq!(second.map(|e| e.tick), Some(24 + 12));
        assert_eq!(sequence.events[0].tick, 0, "odd steps keep their place");
        project.channels[0].swing_mix = 0;
        let sequence = compile(&project, Source::Pattern(project.patterns[0].id));
        assert!(sequence.events.iter().any(|e| e.tick == 24));
    }

    #[test]
    fn muted_channels_produce_no_events() {
        let (mut project, _) = project_with_kick();
        project.channels[0].mute = true;
        assert!(
            compile(&project, Source::Pattern(project.patterns[0].id))
                .events
                .is_empty()
        );
    }

    #[test]
    fn piano_roll_notes_replace_steps_for_that_channel() {
        let (mut project, id) = project_with_kick();
        project.patterns[0]
            .notes
            .insert(id, vec![Note::new(Tick(48), Tick(96), 64)]);
        let sequence = compile(&project, Source::Pattern(project.patterns[0].id));
        assert_eq!(sequence.events.len(), 1);
        assert_eq!((sequence.events[0].tick, sequence.events[0].key), (48, 64));
        assert_eq!(sequence.gate_for(&sequence.events[0]), 96);
    }

    #[test]
    fn song_mode_places_clip_windows_and_skips_muted_tracks() {
        let (mut project, _) = project_with_kick();
        let pattern = project.patterns[0].id;
        let track = project.ids.track();
        let muted_track = project.ids.track();
        project.arrangements[0]
            .tracks
            .push(PlaylistTrack::new(track, "Track 1"));
        let mut muted = PlaylistTrack::new(muted_track, "Track 2");
        muted.mute = true;
        project.arrangements[0].tracks.push(muted);
        let mut clip =
            ClipInstance::pattern(project.ids.clip(), track, pattern, Tick(384), Tick(192));
        clip.offset = Tick(96);
        project.arrangements[0].clips.push(clip);
        let hidden =
            ClipInstance::pattern(project.ids.clip(), muted_track, pattern, Tick(0), Tick(384));
        project.arrangements[0].clips.push(hidden);
        let sequence = compile(&project, Source::Song);
        let ticks: Vec<i64> = sequence.events.iter().map(|e| e.tick).collect();
        assert_eq!(ticks, vec![384, 480]);
        assert_eq!(sequence.length, 768);
    }
}
