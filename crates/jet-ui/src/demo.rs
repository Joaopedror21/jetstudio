//! Demonstration project used by the offscreen screenshot tool and visual comparisons.
//!
//! Names and colours are ours; the channel/track counts approximate the density visible in the
//! master reference capture (12 channel rows, ~25 playlist rows) so geometry can be compared.

use jet_engine::Session;
use jet_model::{Channel, ClipInstance, Marker, Pattern, PlaylistTrack, Project, StepSeq};
use jet_time::Tick;

const CHANNELS: [(&str, u32); 12] = [
    ("Kick", 0xC8_7A4A),
    ("Clap", 0xC9_A84E),
    ("Hat Closed", 0x7D_8A93),
    ("Snare", 0x8F_9095),
    ("Perc Loop", 0x9C_7C9A),
    ("Sub Bass", 0xC2_A54C),
    ("Rim", 0x8E_8466),
    ("Riser", 0x9A_8A9E),
    ("FX Drop", 0x8C_7A86),
    ("Vox Chop", 0x95_8395),
    ("Crash", 0x8A_8A7C),
    ("Pad", 0x86_7D93),
];

const PATTERNS: [(&str, u32); 8] = [
    ("Intro Beat", 0x4A_5CB0),
    ("Pattern 2", 0x7A_2E45),
    ("Main Beat", 0x4A_5CB0),
    ("Main Beat #2", 0x4A_5CB0),
    ("Pattern 5", 0x3E_454B),
    ("Pattern 6", 0x3E_454B),
    ("Sub #2", 0x52_585E),
    ("Hat", 0xC5_D38A),
];

const TRACKS: [(&str, u32); 14] = [
    ("Hat Loop", 0xC0_7A45),
    ("Room Clap", 0x9A_8094),
    ("Night Lights", 0x8E_8A72),
    ("Room Perc", 0x7C_8C96),
    ("Short Kick", 0xC0_7A45),
    ("Kick 36", 0xC0_7A45),
    ("Kick Layer", 0xC0_7A45),
    ("Kick Ghost", 0x3F_464C),
    ("Clap Night", 0x8E_8A72),
    ("Snare B", 0xB8_A445),
    ("Clap Alt", 0x7F_9A84),
    ("Clap 2", 0xB8_A445),
    ("Clap 10", 0xB8_A445),
    ("Hat 3", 0x7F_9A84),
];

/// A small, original project exercising every M1 surface.
pub fn demo_project() -> Project {
    let mut project = Project::new();
    let ppq = i64::from(project.settings.ppq.0);
    let bar = ppq * 4;
    project.patterns.clear();
    let mut patterns = Vec::new();
    for (name, color) in PATTERNS {
        let id = project.ids.pattern();
        let mut pattern = Pattern::new(id, name);
        pattern.color = Some(color);
        patterns.push(id);
        project.patterns.push(pattern);
    }
    for (index, (name, color)) in CHANNELS.iter().enumerate() {
        let id = project.ids.channel();
        // The last channel is our synth, so the Piano roll has a score to show.
        let mut channel = if index == CHANNELS.len() - 1 {
            Channel::synth(id, *name)
        } else {
            Channel::sampler(id, *name)
        };
        channel.color = Some(*color);
        channel.mixer_target = [2, 4, 3, 20, 21, 15, 22, 23, 26, 24, 25, 27][index];
        project.channels.push(channel);
        if let Ok(mut seq) = StepSeq::with_len(16) {
            for (step, cell) in seq.cells.iter_mut().enumerate() {
                cell.on = (4..8).contains(&step) || (index % 3 == 0 && step % 4 == 0);
            }
            if let Some(first) = project.patterns.first_mut() {
                first.steps.insert(id, seq);
            }
        }
    }
    // A four-chord progression (i-VI-III-VII in A minor) for the synth channel.
    if let (Some(pad), Some(first)) = (
        project.channels.last().map(|c| c.id),
        project.patterns.first_mut(),
    ) {
        let chords: [[u8; 3]; 4] = [[57, 60, 64], [53, 57, 60], [48, 52, 55], [55, 59, 62]];
        let mut notes = Vec::new();
        for (bar_index, chord) in chords.iter().enumerate() {
            for (voice, key) in chord.iter().enumerate() {
                // One chord per beat, each a little shorter than the beat.
                let mut note = jet_model::Note::new(
                    Tick(bar_index as i64 * (bar / 4)),
                    Tick(bar / 4 - 12),
                    *key,
                );
                note.velocity = 90 + voice as u8 * 10;
                notes.push(note);
            }
        }
        first.steps.remove(&pad);
        first.notes.insert(pad, notes);
    }
    for (index, name) in [(2u16, "Kick"), (3, "Hats"), (4, "Clap")] {
        if let Some(track) = project.mixer.track_mut(index) {
            track.name = name.to_owned();
        }
    }
    if let Some(fx) = jet_model::FxKind::default_for("limiter") {
        project.mixer.master.fx.push(jet_model::FxSlot {
            kind: fx,
            enabled: true,
            mix: 1000,
        });
    }
    if let Some(arrangement) = project.arrangements.first_mut() {
        let mut tracks = Vec::new();
        for (name, color) in TRACKS {
            let id = project.ids.track();
            let mut track = PlaylistTrack::new(id, name);
            track.color = Some(color);
            tracks.push(id);
            arrangement.tracks.push(track);
        }
        let mut clip = |track: usize, pattern: usize, start_bar: i64, bars: i64| {
            if let (Some(&track), Some(&pattern)) = (tracks.get(track), patterns.get(pattern)) {
                let id = project.ids.clip();
                arrangement.clips.push(ClipInstance::pattern(
                    id,
                    track,
                    pattern,
                    Tick(start_bar * bar),
                    Tick(bars * bar),
                ));
            }
        };
        for start in (16..28).chain(36..40) {
            clip(0, 0, start, 1);
        }
        for start in [16, 20, 24, 28] {
            clip(1, 1, start, 1);
        }
        for start in [17, 21, 25, 37] {
            clip(2, 2, start, 1);
        }
        for start in [16, 19, 22, 25, 28, 36] {
            clip(3, 3, start, 1);
        }
        for start in 16..40 {
            clip(4, 4, start, 1);
        }
        for start in 16..28 {
            clip(5, 5, start, 1);
        }
        clip(7, 6, 16, 2);
        for start in [20, 28] {
            clip(8, 7, start, 4);
        }
        for start in (16..40).step_by(2) {
            clip(9, 2, start, 1);
            clip(11, 3, start, 1);
        }
        arrangement.markers.push(Marker {
            at: Tick(28 * bar),
            name: "Drop 1".to_owned(),
            action: jet_model::MarkerAction::None,
        });
        arrangement.markers.push(Marker {
            at: Tick(36 * bar),
            name: "V2".to_owned(),
            action: jet_model::MarkerAction::None,
        });
    }
    project
}

pub fn demo_session() -> Session {
    let mut session =
        Session::from_project(demo_project()).unwrap_or_else(|_| Session::new_default());
    // An automation clip on the Master volume, shaped as a rise and a dip.
    if session
        .execute(
            "automation.create",
            serde_json::json!({ "target": { "MixerVolume": 0 } }),
        )
        .is_ok()
    {
        for (at, value) in [(16 * 384, 200), (24 * 384, 1000), (32 * 384, 500)] {
            let _ = session.execute(
                "automation.point.add",
                serde_json::json!({ "id": 1, "at": at, "value": value }),
            );
        }
    }
    let pad = session.project().channels.last().map(|c| c.id.0);
    if let Some(id) = pad {
        let _ = session.execute("channel.select", serde_json::json!({ "id": id }));
    }
    session
}
