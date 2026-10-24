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
    /// Slide note: silent; bends overlapping notes of the same color [Manual: pianoroll].
    pub slide: bool,
    pub porta: bool,
    /// Note color group 0..=15; slides and Mono act per group [Manual: pianoroll, "Color Groups"].
    pub color: u8,
    /// Audio Clip: the voice starts `audio_offset` ticks into the sample and is cut (with the
    /// declick) at the end of `length` instead of being released.
    pub audio: bool,
    pub audio_offset: i64,
    /// Audio Clip instance gain, fades and reverse.
    pub clip: jet_model::AudioClipProps,
    /// Ticks of the clip already elapsed when the voice starts (playback started inside it).
    pub audio_elapsed: i64,
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
    /// Loop length in ticks (always > 0): the end of the material, where playback wraps.
    pub length: i64,
    /// Where playback returns when it reaches `length` (a Song loop marker or a timeline
    /// selection start; 0 otherwise).
    pub loop_start: i64,
    /// Jumps (from, to) taken when the play position reaches `from` (marker skip and marker
    /// loop).
    pub jumps: Vec<(i64, i64)>,
    /// Positions where playback pauses (marker pause).
    pub pauses: Vec<i64>,
    /// Ticks in one step, used for step-style note gates.
    pub step_ticks: i64,
}

impl Sequence {
    /// The material tick for play tick `tick` of a playback that started at material position
    /// `anchor` (play tick `anchor` too).
    pub fn wrap_from(&self, anchor: i64, tick: i64) -> i64 {
        self.locate(anchor, tick).0
    }

    /// (material position, linear ticks left before the next jump, pause or loop) for play tick
    /// `tick` of a playback anchored at `anchor`. The play runs straight on; reaching a jump's
    /// `from` moves it to its `to`, reaching `length` moves it to `loop_start`, reaching a pause
    /// stops it there (0 ticks left). A jump fires when the position crosses `from`, not when it
    /// starts on it. Cycles are folded, so any `tick` takes a few steps and no allocation.
    pub fn locate(&self, anchor: i64, tick: i64) -> (i64, i64) {
        const END: usize = usize::MAX - 1;
        const PAUSE: usize = usize::MAX;
        let length = self.length.max(1);
        let start = if self.loop_start < length {
            self.loop_start.max(0)
        } else {
            0
        };
        let span = (length - start).max(1);
        let normalize = |m: i64| {
            if m >= length {
                start + (m - length).rem_euclid(span)
            } else {
                m.max(0)
            }
        };
        let mut m = normalize(anchor);
        let mut e = (tick - anchor).max(0);
        let mut seen = [(usize::MAX, 0_i64); 16];
        for _ in 0..256 {
            let mut next = (length, END);
            for (i, (from, _)) in self.jumps.iter().enumerate() {
                if *from > m && *from < next.0 {
                    next = (*from, i);
                }
            }
            for at in &self.pauses {
                if *at > m && *at < next.0 {
                    next = (*at, PAUSE);
                }
            }
            let d = next.0 - m;
            if e < d {
                return (m + e, d - e);
            }
            if next.1 == PAUSE {
                return (next.0, 0);
            }
            e -= d;
            m = if next.1 == END {
                start
            } else {
                normalize(self.jumps[next.1].1)
            };
            // Fold whole cycles: the same boundary reached again means the play repeats.
            match seen.iter_mut().find(|(kind, _)| *kind == next.1) {
                Some(entry) => {
                    let cycle = entry.1 - e;
                    if cycle > 0 && e >= cycle {
                        e %= cycle;
                    }
                    entry.1 = e;
                }
                None => {
                    if let Some(slot) = seen.iter_mut().find(|(kind, _)| *kind == usize::MAX) {
                        *slot = (next.1, e);
                    }
                }
            }
        }
        (m, 0)
    }

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
        let first = out.len();
        channel_events(project, pattern, channel, limit, out);
        apply_time(channel, step, &mut out[first..]);
    }
}

/// The channel's Time settings [Manual: chansettings_misc, "Time"]: Shift delays every note;
/// Gate truncates notes "when they pass the specified length" (step notes count as one step),
/// except porta notes with Full porta on. Slide notes keep their length (it is their slide time).
fn apply_time(channel: &jet_model::Channel, step: i64, events: &mut [NoteEvent]) {
    let shift = i64::from(channel.time_shift).clamp(0, step - 1);
    let gate = i64::from(channel.gate);
    for event in events {
        event.tick += shift;
        if gate > 0 && !event.slide && !(channel.full_porta && event.porta) {
            let length = if event.length > 0 { event.length } else { step };
            event.length = length.min(gate);
        }
    }
}

/// Notes or steps of one (unmuted) channel in `pattern`.
fn channel_events(
    project: &Project,
    pattern: &Pattern,
    channel: &jet_model::Channel,
    limit: usize,
    out: &mut Vec<NoteEvent>,
) {
    let settings = &project.settings;
    let step = step_ticks(settings);
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
                color: note.color,
                audio: false,
                audio_offset: 0,
                clip: jet_model::AudioClipProps::default(),
                audio_elapsed: 0,
            });
        }
        return;
    }
    let Some(seq) = pattern.steps.get(&channel.id) else {
        return;
    };
    for (index, cell) in seq.cells.iter().enumerate().take(limit) {
        if !cell.on {
            continue;
        }
        let shift = i64::from(cell.props.shift).clamp(0, step - 1);
        out.push(NoteEvent {
            tick: index as i64 * step + shift + swing_offset(settings, channel.swing_mix, index),
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
            color: 0,
            audio: false,
            audio_offset: 0,
            clip: jet_model::AudioClipProps::default(),
            audio_elapsed: 0,
        });
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
    let data = ceil_to(end.max(1), bar_ticks(&project.settings));
    // A Loop marker past the last data extends the song (and the render) to it.
    loop_marker(project).map_or(data, |at| data.max(at))
}

/// Tick of the active arrangement's first marker with `action`.
pub fn marker_at(project: &Project, action: jet_model::MarkerAction) -> Option<i64> {
    project
        .active()?
        .markers
        .iter()
        .find(|m| m.action == action)
        .map(|m| m.at.0)
}

/// Jumps and pauses from the active arrangement's markers [Manual: playlist, "Time Markers",
/// "Action"]: a Skip marker jumps to the next marker; a Marker loop returns from the next marker
/// to itself; a Pause marker pauses unless the preceding marker is a loop. "The next marker" of the
/// last one is the song end.
pub fn marker_jumps(project: &Project, length: i64) -> (Vec<(i64, i64)>, Vec<i64>) {
    use jet_model::MarkerAction;
    let Some(arrangement) = project.active() else {
        return (Vec::new(), Vec::new());
    };
    let mut markers: Vec<&jet_model::Marker> = arrangement.markers.iter().collect();
    markers.sort_by_key(|m| m.at);
    let mut jumps = Vec::new();
    let mut pauses = Vec::new();
    for (i, marker) in markers.iter().enumerate() {
        let at = marker.at.0;
        let next = markers.get(i + 1).map_or(length, |m| m.at.0);
        match marker.action {
            MarkerAction::Skip if next > at => jumps.push((at, next)),
            MarkerAction::MarkerLoop if next > at => jumps.push((next, at)),
            MarkerAction::Pause => {
                let after_loop = i > 0
                    && matches!(
                        markers[i - 1].action,
                        MarkerAction::Loop | MarkerAction::MarkerLoop
                    );
                if !after_loop {
                    pauses.push(at);
                }
            }
            _ => {}
        }
    }
    (jumps, pauses)
}

/// The Song loop point: a Loop marker [Manual: playlist, "Song Loop (uses Time Markers)"].
pub fn loop_marker(project: &Project) -> Option<i64> {
    marker_at(project, jet_model::MarkerAction::Loop)
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
                                    color: 0,
                                    audio: true,
                                    audio_offset: clip.offset.0,
                                    clip: clip.audio,
                                    audio_elapsed: 0,
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
    // Slide notes follow the notes that start with them, so they can bend those notes.
    events.sort_by_key(|event| (event.tick, event.channel, event.slide, event.key));
    let loop_start = match source {
        Source::Song => loop_marker(project).filter(|at| *at < length).unwrap_or(0),
        Source::Pattern(_) => 0,
    };
    let (jumps, pauses) = match source {
        Source::Song => marker_jumps(project, length),
        Source::Pattern(_) => (Vec::new(), Vec::new()),
    };
    Sequence {
        events,
        length: length.max(1),
        loop_start,
        jumps,
        pauses,
        step_ticks: step_ticks(&project.settings),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jet_model::{Channel, ClipInstance, Note, PlaylistTrack, StepSeq};
    use jet_time::Tick;

    fn timeline(length: i64, jumps: Vec<(i64, i64)>, pauses: Vec<i64>) -> Sequence {
        Sequence {
            length,
            jumps,
            pauses,
            ..Sequence::default()
        }
    }

    #[test]
    fn locate_follows_skips_marker_loops_and_pauses() {
        // A skip from 100 to 300 in a 400-tick song.
        let seq = timeline(400, vec![(100, 300)], vec![]);
        assert_eq!(seq.locate(0, 50), (50, 50));
        assert_eq!(seq.locate(0, 150), (350, 50), "100 + 50 past the skip");
        assert_eq!(seq.locate(0, 250), (50, 50), "then the song loops from 0");
        // Starting on the skip marker does not skip.
        assert_eq!(seq.wrap_from(100, 110), 110);
        // A marker loop 200 -> 100: after 200 it repeats 100..200 for ever (cycles folded).
        let seq = timeline(400, vec![(200, 100)], vec![]);
        assert_eq!(seq.wrap_from(0, 250), 150);
        assert_eq!(seq.wrap_from(0, 200 + 1_000_000 * 100 + 30), 130);
        // A pause at 120 stops the position there.
        let seq = timeline(400, vec![], vec![120]);
        assert_eq!(seq.locate(0, 500), (120, 0));
        assert_eq!(seq.locate(120, 130), (130, 270), "resumed from the pause");
    }

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
