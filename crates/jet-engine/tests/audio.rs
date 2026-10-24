//! Integration tests: commands → project → sequencer → renderer → WAV file.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use jet_engine::{ManualClock, Session};
use jet_media::{WavFormat, decode_file, write_wav};
use jet_model::ChannelId;
use serde_json::json;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static SEQ: AtomicU64 = AtomicU64::new(0);

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "jet-engine-audio-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn session_120() -> Session {
    let mut session = Session::with_clock(Box::new(ManualClock::new(48_000)));
    session
        .execute("transport.tempo.set", json!({"milli_bpm": 120_000}))
        .unwrap();
    session
}

fn export(session: &mut Session, mode: &str) -> (Vec<f32>, Vec<f32>) {
    let path = temp("out.wav");
    session
        .execute(
            "file.export_wav",
            json!({"path": path, "mode": mode, "bits": 32, "tail": "cut", "rate": 48_000}),
        )
        .unwrap();
    let data = decode_file(&path).unwrap();
    let right = data.right.clone().unwrap_or_else(|| data.left.clone());
    (data.left, right)
}

fn onsets(data: &[f32]) -> Vec<usize> {
    let mut found = Vec::new();
    let mut quiet = 0;
    for (i, v) in data.iter().enumerate() {
        if v.abs() > 1e-4 {
            if quiet > 200 || (i == 0) {
                found.push(i);
            }
            quiet = 0;
        } else {
            quiet += 1;
        }
    }
    found
}

fn add_sampler_with_click(session: &mut Session) -> u64 {
    let wav = temp("click.wav");
    // 100-frame full-scale click, mono, 48 kHz.
    write_wav(&wav, 48_000, 1, &[1.0; 100], WavFormat::Float32).unwrap();
    session
        .execute("channel.add", json!({"name": "Kick"}))
        .unwrap();
    let id = session.selected_channel().unwrap().0;
    session
        .execute("channel.sample.load", json!({"id": id, "path": wav}))
        .unwrap();
    id
}

#[test]
fn channel_rack_steps_render_at_the_step_grid() {
    let mut session = session_120();
    let id = add_sampler_with_click(&mut session);
    assert_eq!(session.project().channels[0].name, "click");
    for index in [0, 4, 8, 12] {
        session
            .execute(
                "step.set",
                json!({"pattern": 1, "channel": id, "index": index, "on": true}),
            )
            .unwrap();
    }
    let (left, _) = export(&mut session, "pattern");
    // One 4/4 bar at 120 BPM = 2 s = 96 000 frames; quarter notes every 24 000.
    assert_eq!(left.len(), 96_000);
    assert_eq!(onsets(&left), vec![0, 24_000, 48_000, 72_000]);
    // Default Sampler: -8.2 dB under the recorded full-scale level.
    let db = 20.0 * f64::from(left[50]).log10();
    assert!((db + 8.2).abs() < 0.05, "{db}");
}

#[test]
fn step_fill_and_graph_editor_change_what_plays() {
    let mut session = session_120();
    let id = add_sampler_with_click(&mut session);
    session
        .execute(
            "step.fill",
            json!({"pattern": 1, "channel": id, "every": 8}),
        )
        .unwrap();
    session
        .execute(
            "step.props.set",
            json!({"pattern": 1, "channel": id, "index": 8, "velocity": 50}),
        )
        .unwrap();
    let (left, _) = export(&mut session, "pattern");
    assert_eq!(onsets(&left), vec![0, 48_000]);
    let ratio = left[48_050] / left[50];
    assert!(
        (ratio - 0.5).abs() < 1e-3,
        "velocity 50 is half of 100: {ratio}"
    );
}

#[test]
fn graph_editor_pan_and_shift_change_the_render() {
    let mut session = session_120();
    let id = add_sampler_with_click(&mut session);
    for index in [0, 8] {
        session
            .execute(
                "step.set",
                json!({"pattern": 1, "channel": id, "index": index, "on": true}),
            )
            .unwrap();
    }
    // Step 0 hard right; step 8 shifted by half a step (12 of 24 ticks = 3 000 frames).
    session
        .execute(
            "step.props.set",
            json!({"pattern": 1, "channel": id, "index": 0, "pan": 1000}),
        )
        .unwrap();
    session
        .execute(
            "step.props.set",
            json!({"pattern": 1, "channel": id, "index": 8, "shift": 12}),
        )
        .unwrap();
    let (left, right) = export(&mut session, "pattern");
    assert_eq!(left[50], 0.0, "the panned step is silent on the left");
    assert!(right[50] > 0.3);
    assert_eq!(onsets(&left), vec![48_000 + 3_000]);
}

#[test]
fn channel_volume_pan_and_mute_change_the_audio() {
    let mut session = session_120();
    let id = add_sampler_with_click(&mut session);
    session
        .execute(
            "step.set",
            json!({"pattern":1,"channel":id,"index":0,"on":true}),
        )
        .unwrap();
    session
        .execute("channel.pan.set", json!({"id": id, "value": 1000}))
        .unwrap();
    let (left, right) = export(&mut session, "pattern");
    assert_eq!(left[50], 0.0);
    assert!(right[50] > 0.5);
    session
        .execute("channel.volume.set", json!({"id": id, "value": 1000}))
        .unwrap();
    let (_, right) = export(&mut session, "pattern");
    assert!(
        (right[50] - 1.0).abs() < 1e-4,
        "100% volume, hard right = recorded level"
    );
    session
        .execute("channel.mute.set", json!({"id": id, "mute": true}))
        .unwrap();
    let (left, right) = export(&mut session, "pattern");
    assert!(left.iter().chain(&right).all(|v| *v == 0.0));
}

#[test]
fn piano_roll_notes_drive_the_synth_at_their_pitch() {
    let mut session = session_120();
    session
        .execute("channel.add", json!({"name": "Lead", "kind": "synth"}))
        .unwrap();
    let id = session.selected_channel().unwrap().0;
    session
        .execute(
            "channel.synth.set",
            json!({"id": id, "params": {"waveform": "Sine", "attack_ms": 0, "decay_ms": 0,
                "sustain": 1000, "release_ms": 5, "cutoff_hz": 20000, "resonance": 0, "detune_cents": 0}}),
        )
        .unwrap();
    // A5 (key 69, FL's naming with C5 = 60) for one beat.
    session
        .execute(
            "note.add",
            json!({"pattern": 1, "channel": id, "start": 0, "length": 96, "key": 69}),
        )
        .unwrap();
    let (left, _) = export(&mut session, "pattern");
    let crossings = left[..24_000]
        .windows(2)
        .filter(|w| w[0] <= 0.0 && w[1] > 0.0)
        .count();
    // 0.5 s of 440 Hz = 220 cycles.
    assert!((219..=221).contains(&crossings), "{crossings}");
    assert!(
        left[30_000..].iter().all(|v| v.abs() < 1e-3),
        "note-off releases the voice"
    );
    // Transpose an octave down: 220 Hz.
    session
        .execute(
            "note.transpose",
            json!({"pattern":1,"channel":id,"semitones":-12}),
        )
        .unwrap();
    let (left, _) = export(&mut session, "pattern");
    let crossings = left[..24_000]
        .windows(2)
        .filter(|w| w[0] <= 0.0 && w[1] > 0.0)
        .count();
    assert!((109..=111).contains(&crossings), "{crossings}");
}

#[test]
fn piano_roll_editing_tools() {
    let mut session = session_120();
    session
        .execute("channel.add", json!({"name": "Keys", "kind": "synth"}))
        .unwrap();
    let id = session.selected_channel().unwrap().0;
    let channel = ChannelId(id);
    let notes = |s: &Session| {
        s.project().patterns[0]
            .notes
            .get(&channel)
            .cloned()
            .unwrap_or_default()
    };
    session
        .execute(
            "note.add",
            json!({"pattern":1,"channel":id,"start":5,"length":90,"key":60}),
        )
        .unwrap();
    session
        .execute("note.quantize", json!({"pattern":1,"channel":id,"grid":24}))
        .unwrap();
    assert_eq!(
        (notes(&session)[0].start.0, notes(&session)[0].length.0),
        (0, 96)
    );
    session
        .execute("note.chop", json!({"pattern":1,"channel":id,"grid":24}))
        .unwrap();
    assert_eq!(notes(&session).len(), 4);
    session
        .execute("note.glue", json!({"pattern":1,"channel":id}))
        .unwrap();
    assert_eq!(notes(&session).len(), 1);
    session
        .execute(
            "note.duplicate",
            json!({"pattern":1,"channel":id,"grid":96}),
        )
        .unwrap();
    let after = notes(&session);
    assert_eq!(
        after.iter().map(|n| n.start.0).collect::<Vec<_>>(),
        vec![0, 96]
    );
    session
        .execute(
            "note.delete",
            json!({"pattern":1,"channel":id,"indices":[0]}),
        )
        .unwrap();
    assert_eq!(notes(&session).len(), 1);
    session.execute("edit.undo_toggle", json!({})).unwrap();
    assert_eq!(notes(&session).len(), 2, "undo restores the deleted note");
    assert!(
        session
            .execute("note.move", json!({"pattern":1,"channel":id,"dkey":100}))
            .is_err(),
        "moving past the keyboard is rejected"
    );
    assert_eq!(
        notes(&session).len(),
        2,
        "a failed edit leaves the score untouched"
    );
}

#[test]
fn playlist_song_mode_plays_clips_at_their_positions() {
    let mut session = session_120();
    let id = add_sampler_with_click(&mut session);
    session
        .execute(
            "step.set",
            json!({"pattern":1,"channel":id,"index":0,"on":true}),
        )
        .unwrap();
    session
        .execute("playlist.track.add", json!({"name": "Track 1"}))
        .unwrap();
    for start in [0, 768] {
        session
            .execute(
                "playlist.clip.add",
                json!({"track":1,"pattern":1,"start":start,"length":384}),
            )
            .unwrap();
    }
    let (left, _) = export(&mut session, "song");
    // Song ends at bar 3 (768 + 384 ticks = 6 s).
    assert_eq!(left.len(), 288_000);
    assert_eq!(onsets(&left), vec![0, 192_000]);
    session
        .execute("playlist.track.mute.set", json!({"id": 1, "mute": true}))
        .unwrap();
    let (left, _) = export(&mut session, "song");
    assert!(left.iter().all(|v| *v == 0.0));
}

#[test]
fn mixer_routing_faders_sends_and_effects_change_the_output() {
    let mut session = session_120();
    let id = add_sampler_with_click(&mut session);
    session
        .execute(
            "step.set",
            json!({"pattern":1,"channel":id,"index":0,"on":true}),
        )
        .unwrap();
    let (direct, _) = export(&mut session, "pattern");
    session
        .execute("channel.mixer_target.free", json!({"id": id}))
        .unwrap();
    assert_eq!(session.project().channels[0].mixer_target, 1);
    assert_eq!(session.project().mixer.inserts[0].name, "click");
    let (routed, _) = export(&mut session, "pattern");
    assert!((routed[50] - direct[50]).abs() < 1e-6, "unity insert");
    session
        .execute("mixer.track.mute.set", json!({"track": 1, "value": true}))
        .unwrap();
    let (muted, _) = export(&mut session, "pattern");
    assert!(muted.iter().all(|v| *v == 0.0));
    session
        .execute("mixer.track.mute.set", json!({"track": 1, "value": false}))
        .unwrap();
    session
        .execute(
            "mixer.track.to_master.set",
            json!({"track": 1, "value": false}),
        )
        .unwrap();
    session
        .execute("mixer.send.set", json!({"from": 1, "to": 2}))
        .unwrap();
    let (sent, _) = export(&mut session, "pattern");
    assert!(
        (sent[50] - direct[50]).abs() < 1e-6,
        "audio reaches the Master via insert 2"
    );
    assert!(
        session
            .execute("mixer.send.set", json!({"from": 2, "to": 1}))
            .is_err(),
        "feedback loops are refused"
    );
    session
        .execute("mixer.track.volume.set", json!({"track": 0, "value": 400}))
        .unwrap();
    let (quiet, _) = export(&mut session, "pattern");
    assert!(quiet[50] < sent[50] * 0.5);
    session
        .execute("mixer.fx.add", json!({"track": 2, "kind": "distortion"}))
        .unwrap();
    let (driven, _) = export(&mut session, "pattern");
    assert!(
        (driven[50] - quiet[50]).abs() > 1e-3,
        "insert effect processes the audio"
    );
    session
        .execute(
            "mixer.fx.enabled.set",
            json!({"track": 2, "slot": 0, "value": false}),
        )
        .unwrap();
    let (bypassed, _) = export(&mut session, "pattern");
    assert!((bypassed[50] - quiet[50]).abs() < 1e-6);
}

#[test]
fn save_and_load_keep_samples_and_render_identically() {
    let mut session = session_120();
    let id = add_sampler_with_click(&mut session);
    session
        .execute("step.fill", json!({"pattern":1,"channel":id,"every":4}))
        .unwrap();
    let (before, _) = export(&mut session, "pattern");
    let project_path = temp("song.jetproj");
    session
        .execute("file.save_as", json!({"path": project_path}))
        .unwrap();
    let mut loaded = session_120();
    loaded
        .execute("file.open", json!({"path": project_path}))
        .unwrap();
    assert_eq!(
        loaded.project().resources[0].status,
        jet_model::ResourceStatus::Available
    );
    let (after, _) = export(&mut loaded, "pattern");
    assert_eq!(before, after);
}

#[test]
fn transport_position_wraps_at_the_pattern_length() {
    let clock = ManualClock::new(48_000);
    let mut session = Session::with_clock(Box::new(clock.clone()));
    session
        .execute("transport.tempo.set", json!({"milli_bpm": 120_000}))
        .unwrap();
    session.execute("transport.play_stop", json!({})).unwrap();
    // 2.5 s into a one-bar (2 s) pattern = half a second = one beat.
    clock.set_frames(120_000);
    assert_eq!(session.transport_snapshot().position.0, 96);
    let sync = session.transport_sync();
    assert!(sync.playing);
    assert_eq!(sync.anchor_tick, 0);
}

#[test]
fn channel_solo_clone_and_move() {
    let mut session = session_120();
    for name in ["A", "B", "C"] {
        session
            .execute("channel.add", json!({"name": name}))
            .unwrap();
    }
    session.execute("channel.solo", json!({"id": 2})).unwrap();
    let mutes: Vec<bool> = session.project().channels.iter().map(|c| c.mute).collect();
    assert_eq!(mutes, vec![true, false, true]);
    session.execute("channel.solo", json!({"id": 2})).unwrap();
    assert!(session.project().channels.iter().all(|c| !c.mute));
    session.execute("channel.clone", json!({"id": 1})).unwrap();
    let names: Vec<&str> = session
        .project()
        .channels
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    assert_eq!(names, vec!["A", "A #2", "B", "C"]);
    session
        .execute("channel.move", json!({"id": 3, "delta": 10}))
        .unwrap();
    assert_eq!(session.project().channels.last().unwrap().name, "C");
    session
        .execute("channel.move", json!({"id": 3, "delta": -1}))
        .unwrap();
    let names: Vec<&str> = session
        .project()
        .channels
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    assert_eq!(names, vec!["A", "A #2", "C", "B"]);
}

#[test]
fn clip_mute_and_track_solo_change_the_song() {
    let mut session = session_120();
    let id = add_sampler_with_click(&mut session);
    session
        .execute(
            "step.set",
            json!({"pattern":1,"channel":id,"index":0,"on":true}),
        )
        .unwrap();
    for name in ["Track 1", "Track 2"] {
        session
            .execute("playlist.track.add", json!({"name": name}))
            .unwrap();
    }
    session
        .execute(
            "playlist.clip.add",
            json!({"track":1,"pattern":1,"start":0,"length":384}),
        )
        .unwrap();
    session
        .execute(
            "playlist.clip.add",
            json!({"track":2,"pattern":1,"start":384,"length":384}),
        )
        .unwrap();
    let (left, _) = export(&mut session, "song");
    assert_eq!(onsets(&left), vec![0, 96_000]);
    session
        .execute("playlist.clip.mute.set", json!({"id": 1, "muted": true}))
        .unwrap();
    let (left, _) = export(&mut session, "song");
    assert_eq!(onsets(&left), vec![96_000], "a muted clip is silent");
    session
        .execute("playlist.clip.mute.set", json!({"id": 1, "muted": false}))
        .unwrap();
    session
        .execute("playlist.track.solo", json!({"id": 1}))
        .unwrap();
    let (left, _) = export(&mut session, "song");
    assert_eq!(onsets(&left), vec![0], "solo silences the other tracks");
}

#[test]
fn automation_clips_drive_parameters_in_song_mode() {
    let mut session = session_120();
    // A long click so the level can be read at several points of the bar.
    let wav = temp("long.wav");
    write_wav(&wav, 48_000, 1, &vec![1.0; 96_000], WavFormat::Float32).unwrap();
    session
        .execute("channel.add", json!({"name": "Pad"}))
        .unwrap();
    session
        .execute("channel.sample.load", json!({"id": 1, "path": wav}))
        .unwrap();
    session
        .execute("channel.volume.set", json!({"id": 1, "value": 1000}))
        .unwrap();
    session
        .execute(
            "step.set",
            json!({"pattern":1,"channel":1,"index":0,"on":true}),
        )
        .unwrap();
    session
        .execute("playlist.track.add", json!({"name": "Track 1"}))
        .unwrap();
    session
        .execute(
            "playlist.clip.add",
            json!({"track":1,"pattern":1,"start":0,"length":384}),
        )
        .unwrap();
    // Right-click > Create automation clip on the channel volume.
    session
        .execute("automation.create", json!({"target": {"ChannelVolume": 1}}))
        .unwrap();
    let project = session.project();
    assert_eq!(project.automations.len(), 1);
    assert_eq!(project.automations[0].name, "long - Volume");
    assert_eq!(
        project.automations[0].points[0].value, 1000,
        "starts at the knob value"
    );
    let length = project.automations[0].points[1].at.0;
    assert_eq!(length, 4 * 384, "spans at least four bars");
    // Ramp the volume from 0 to full over the first bar.
    session
        .execute(
            "automation.point.move",
            json!({"id":1,"index":0,"at":0,"value":0}),
        )
        .unwrap();
    session
        .execute(
            "automation.point.add",
            json!({"id":1,"at":384,"value":1000}),
        )
        .unwrap();
    let (left, _) = export(&mut session, "song");
    let early = left[2_000].abs();
    let late = left[90_000].abs();
    assert!(early < 0.05, "starts silent: {early}");
    assert!(late > 0.5, "reaches full volume: {late}");
    // Pattern mode plays no Playlist automation: the knob value applies.
    let (pattern, _) = export(&mut session, "pattern");
    assert!(pattern[2_000].abs() > 0.5);
    session
        .execute("automation.delete", json!({"id": 1}))
        .unwrap();
    let (left, _) = export(&mut session, "song");
    assert!(
        left[2_000].abs() > 0.5,
        "deleting the clip restores the knob value"
    );
}

#[test]
fn sampler_settings_and_polyphony_shape_the_output() {
    let mut session = session_120();
    let wav = temp("long.wav");
    // Two seconds of a constant level: overlapping notes add up unless polyphony limits them.
    write_wav(&wav, 48_000, 1, &vec![0.5; 96_000], WavFormat::Float32).unwrap();
    session
        .execute("channel.add", json!({"name": "Pad"}))
        .unwrap();
    session
        .execute("channel.sample.load", json!({"id": 1, "path": wav}))
        .unwrap();
    for index in [0, 4] {
        session
            .execute(
                "step.set",
                json!({"pattern":1,"channel":1,"index":index,"on":true}),
            )
            .unwrap();
    }
    let (left, _) = export(&mut session, "pattern");
    let single = left[1_000];
    let stacked = left[30_000];
    assert!(
        (stacked - 2.0 * single).abs() < 1e-4,
        "two voices overlap: {single} {stacked}"
    );
    // Max poly 1: the second note cuts the first (10 ms declick).
    session
        .execute(
            "channel.polyphony.set",
            json!({"id": 1, "max": 1, "mono": false}),
        )
        .unwrap();
    let (left, _) = export(&mut session, "pattern");
    assert!(
        (left[30_000] - single).abs() < 1e-4,
        "only one voice sounds"
    );
    // Start offset at 75%: each note plays the last quarter (0.5 s = 24 000 frames).
    session
        .execute(
            "channel.polyphony.set",
            json!({"id": 1, "max": 0, "mono": false}),
        )
        .unwrap();
    session
        .execute(
            "channel.sampler.set",
            json!({"id": 1, "settings": {"start": 750, "looping": "Off", "loop_start": 0,
                "loop_end": 1000, "reverse": false, "envelope": null}}),
        )
        .unwrap();
    let (left, _) = export(&mut session, "pattern");
    // The second note starts at frame 24 000 and lasts 24 000 frames as well.
    assert!(left[47_000].abs() > 0.0);
    assert_eq!(
        left[48_000 + 200],
        0.0,
        "each note plays only the last quarter"
    );
    // Volume envelope with a short release: a step note (one step gate) stops soon after it.
    session
        .execute(
            "channel.sampler.set",
            json!({"id": 1, "settings": {"start": 0, "looping": "Off", "loop_start": 0,
                "loop_end": 1000, "reverse": false,
                "envelope": {"attack_ms": 0, "decay_ms": 0, "sustain": 1000, "release_ms": 20}}}),
        )
        .unwrap();
    let (left, _) = export(&mut session, "pattern");
    // One step = 6000 frames; plus 20 ms (960 frames) of release.
    assert!(left[3_000].abs() > 0.0);
    assert_eq!(left[6_000 + 960 + 100], 0.0, "released by the envelope");
}

#[test]
fn audio_clips_play_their_sample_at_the_recorded_level() {
    let mut session = session_120();
    let wav = temp("loop.wav");
    // One second of full scale: 192 ticks at 120 BPM / 96 PPQ.
    write_wav(&wav, 48_000, 1, &vec![1.0; 48_000], WavFormat::Float32).unwrap();
    session
        .execute("playlist.track.add", json!({"name": "Track 1"}))
        .unwrap();
    session
        .execute(
            "playlist.audio.add",
            json!({"path": wav, "track": 1, "start": 384}),
        )
        .unwrap();
    let project = session.project();
    assert_eq!(project.channels[0].name, "loop");
    assert_eq!(project.channels[0].volume, 1000, "Audio Clips play at 100%");
    assert_eq!(project.arrangements[0].clips[0].length.0, 192);
    let (left, right) = export(&mut session, "song");
    assert_eq!(onsets(&left), vec![96_000], "starts at bar 2");
    // 0 dB channel and unity velocity; only the circular pan law's -3 dB applies [Manual:
    // mixer_levelsandmixing, "Audio Clips ... default to 100% volume, 0 dB"].
    let db = 20.0 * f64::from(left[100_000]).log10();
    assert!((db + 3.01).abs() < 0.02, "{db}");
    assert_eq!(left[100_000], right[100_000]);
    assert!(
        left[96_000 + 48_000 + 600].abs() < 1e-6,
        "ends with the sample"
    );
    // Shorter clip: cut at its end (10 ms declick).
    session
        .execute("playlist.clip.resize", json!({"id": 1, "length": 96}))
        .unwrap();
    let (left, _) = export(&mut session, "song");
    assert!(
        left[96_000 + 24_000 + 600].abs() < 1e-6,
        "cut at the clip end"
    );
    assert!(left[96_000 + 23_000].abs() > 0.5);
    // Deleting the channel removes its clips.
    session.execute("channel.remove", json!({"id": 1})).unwrap();
    assert!(session.project().arrangements[0].clips.is_empty());
}

#[test]
fn midi_export_import_and_note_clipboard() {
    let mut session = session_120();
    session
        .execute("channel.add", json!({"name": "Keys", "kind": "synth"}))
        .unwrap();
    session
        .execute("channel.add", json!({"name": "Kick"}))
        .unwrap();
    for (start, key) in [(0, 60), (96, 64), (192, 67)] {
        session
            .execute(
                "note.add",
                json!({"pattern":1,"channel":1,"start":start,"length":48,"key":key}),
            )
            .unwrap();
    }
    session
        .execute(
            "step.set",
            json!({"pattern":1,"channel":2,"index":4,"on":true}),
        )
        .unwrap();
    // Project export: one track per channel; steps become one-step notes.
    let path = temp("song.mid");
    session
        .execute("file.export_mid", json!({"path": path, "mode": "pattern"}))
        .unwrap();
    let smf = jet_format::smf::read(&path).unwrap();
    assert_eq!(smf.ppq, 96);
    assert_eq!(smf.tempo_us, 500_000, "120 BPM");
    assert_eq!(smf.tracks.len(), 2);
    assert_eq!(smf.tracks[0].name, "Keys");
    let keys: Vec<(u64, u8)> = smf.tracks[0]
        .notes
        .iter()
        .map(|n| (n.start, n.key))
        .collect();
    assert_eq!(keys, vec![(0, 60), (96, 64), (192, 67)]);
    assert_eq!(smf.tracks[1].notes[0].start, 96);
    assert_eq!(smf.tracks[1].notes[0].length, 24, "a step lasts one step");
    // Piano roll export then import into another pattern doubles nothing and keeps timing.
    let roll = temp("roll.mid");
    session
        .execute(
            "note.export_mid",
            json!({"pattern":1,"channel":1,"path": roll}),
        )
        .unwrap();
    session.execute("pattern.next_empty", json!({})).unwrap();
    session
        .execute(
            "note.import_mid",
            json!({"pattern":2,"channel":1,"path": roll}),
        )
        .unwrap();
    let imported = &session.project().patterns[1].notes[&ChannelId(1)];
    assert_eq!(imported.len(), 3);
    assert_eq!(
        (imported[2].start.0, imported[2].length.0, imported[2].key),
        (192, 48, 67)
    );
    // Clipboard: copy two notes, paste at bar 3 (tick 768).
    session
        .execute(
            "note.copy",
            json!({"pattern":1,"channel":1,"indices":[1,2]}),
        )
        .unwrap();
    session
        .execute("note.paste", json!({"pattern":1,"channel":1,"at":768}))
        .unwrap();
    let notes = &session.project().patterns[0].notes[&ChannelId(1)];
    let starts: Vec<i64> = notes.iter().map(|n| n.start.0).collect();
    assert_eq!(
        starts,
        vec![0, 96, 192, 768 + 96, 768 + 192],
        "keeps the place within the bar"
    );
    session
        .execute("note.cut", json!({"pattern":1,"channel":1,"indices":[0]}))
        .unwrap();
    assert_eq!(session.project().patterns[0].notes[&ChannelId(1)].len(), 4);
}

#[test]
fn autosave_backs_up_changes_and_revert_restores_them() {
    let mut session = session_120();
    let dir = temp("backup").with_file_name("Backup");
    session.set_backup_dir(Some(dir.clone()));
    session
        .execute("channel.add", json!({"name": "Kick"}))
        .unwrap();
    assert!(session.needs_backup());
    session.execute("file.autosave", json!({})).unwrap();
    assert!(dir.join("Untitled (autosave).jetproj").is_file());
    assert!(!session.needs_backup(), "nothing new to back up");
    session
        .execute("channel.add", json!({"name": "Snare"}))
        .unwrap();
    assert_eq!(session.project().channels.len(), 2);
    session.execute("file.revert_autosave", json!({})).unwrap();
    assert_eq!(
        session.project().channels.len(),
        1,
        "back to the autosaved state"
    );
    // A manual save writes the backup too, under the project's name.
    let path = temp("Beat.jetproj");
    session
        .execute("file.save_as", json!({"path": path}))
        .unwrap();
    assert!(dir.join("Beat (autosave).jetproj").is_file());
}

#[test]
fn audio_clips_without_a_track_go_to_the_first_track_with_room() {
    let mut session = session_120();
    let wav = temp("take.wav");
    write_wav(&wav, 48_000, 1, &vec![0.25; 48_000], WavFormat::Float32).unwrap();
    for _ in 0..2 {
        session
            .execute("playlist.audio.add", json!({"path": wav, "start": 0}))
            .unwrap();
    }
    session
        .execute("playlist.audio.add", json!({"path": wav, "start": 384}))
        .unwrap();
    let arrangement = &session.project().arrangements[0];
    assert_eq!(
        arrangement.tracks.len(),
        2,
        "the overlapping take needed a new track"
    );
    let tracks: Vec<u64> = arrangement.clips.iter().map(|c| c.track.0).collect();
    let first = arrangement.tracks[0].id.0;
    let second = arrangement.tracks[1].id.0;
    assert_eq!(
        tracks,
        vec![first, second, first],
        "later take fits on the first track"
    );
}

#[test]
fn mixer_phase_swap_and_stereo_separation_change_the_output() {
    let mut session = session_120();
    let id = add_sampler_with_click(&mut session);
    session
        .execute(
            "step.set",
            json!({"pattern":1,"channel":id,"index":0,"on":true}),
        )
        .unwrap();
    session
        .execute("channel.pan.set", json!({"id": id, "value": -1000}))
        .unwrap();
    let (left, right) = export(&mut session, "pattern");
    assert!(left[50] > 0.3 && right[50] == 0.0, "panned hard left");
    session
        .execute("mixer.track.swap.set", json!({"track": 0, "value": true}))
        .unwrap();
    let (left, right) = export(&mut session, "pattern");
    assert!(left[50] == 0.0 && right[50] > 0.3, "swap moves it right");
    session
        .execute("mixer.track.invert.set", json!({"track": 0, "value": true}))
        .unwrap();
    let (_, right) = export(&mut session, "pattern");
    assert!(right[50] < -0.3, "invert flips the sign");
    session
        .execute("mixer.track.swap.set", json!({"track": 0, "value": false}))
        .unwrap();
    session
        .execute(
            "mixer.track.invert.set",
            json!({"track": 0, "value": false}),
        )
        .unwrap();
    session
        .execute(
            "mixer.track.stereo_sep.set",
            json!({"track": 0, "value": 1000}),
        )
        .unwrap();
    let (left, right) = export(&mut session, "pattern");
    assert!(
        (left[50] - right[50]).abs() < 1e-6 && left[50] > 0.1,
        "fully narrowed = mono"
    );
}

#[test]
fn arrangements_share_channels_and_play_their_own_clips() {
    let mut session = session_120();
    let id = add_sampler_with_click(&mut session);
    session
        .execute(
            "step.set",
            json!({"pattern":1,"channel":id,"index":0,"on":true}),
        )
        .unwrap();
    session
        .execute("playlist.track.add", json!({"name": "Track 1"}))
        .unwrap();
    session
        .execute(
            "playlist.clip.add",
            json!({"track":1,"pattern":1,"start":0,"length":384}),
        )
        .unwrap();
    session.execute("arrangement.clone", json!({})).unwrap();
    session
        .execute(
            "playlist.clip.move",
            json!({"id": 2, "track": 2, "start": 384}),
        )
        .unwrap();
    let (left, _) = export(&mut session, "song");
    assert_eq!(
        onsets(&left),
        vec![96_000],
        "the clone plays its own moved clip"
    );
    session
        .execute("arrangement.select", json!({"id": 1}))
        .unwrap();
    let (left, _) = export(&mut session, "song");
    assert_eq!(onsets(&left), vec![0], "the original is untouched");
    session.execute("arrangement.add", json!({})).unwrap();
    assert_eq!(session.project().arrangements.len(), 3);
    assert!(session.project().active().unwrap().clips.is_empty());
    session
        .execute("arrangement.delete", json!({"id": 3}))
        .unwrap();
    assert_eq!(
        session.project().active_arrangement,
        jet_model::ArrangementId(1)
    );
    session.execute("marker.add", json!({"at": 768})).unwrap();
    session
        .execute("marker.add", json!({"at": 0, "name": "Intro"}))
        .unwrap();
    let markers = &session.project().active().unwrap().markers;
    assert_eq!(markers[0].name, "Intro");
    assert_eq!(markers[1].at.0, 768);
}

#[test]
fn piano_roll_tools_flip_strum_limit_stamp_arpeggiate_and_snap() {
    let mut session = session_120();
    session
        .execute("channel.add", json!({"name": "Keys", "kind": "synth"}))
        .unwrap();
    let notes = |s: &Session| s.project().patterns[0].notes[&ChannelId(1)].clone();
    // Chord stamp: C major at C5.
    session
        .execute(
            "note.stamp",
            json!({"pattern":1,"channel":1,"start":0,"length":96,"root":60,"chord":"major"}),
        )
        .unwrap();
    let keys: Vec<u8> = notes(&session).iter().map(|n| n.key).collect();
    assert_eq!(keys, vec![60, 64, 67]);
    // Strum: 6 ticks per note from low to high, velocity falling by 10.
    session
        .execute(
            "note.strum",
            json!({"pattern":1,"channel":1,"time":6,"velocity":10}),
        )
        .unwrap();
    let strummed: Vec<(i64, u8, u8)> = notes(&session)
        .iter()
        .map(|n| (n.start.0, n.key, n.velocity))
        .collect();
    assert_eq!(strummed, vec![(0, 60, 100), (6, 64, 90), (12, 67, 80)]);
    session.execute("edit.undo_step", json!({})).unwrap();
    // Flip vertically around the chord's range.
    session
        .execute(
            "note.flip",
            json!({"pattern":1,"channel":1,"vertical":true}),
        )
        .unwrap();
    let keys: Vec<u8> = notes(&session).iter().map(|n| n.key).collect();
    assert_eq!(keys, vec![60, 63, 67]);
    session.execute("edit.undo_step", json!({})).unwrap();
    // Limit to C6..B6 moves everything up an octave.
    session
        .execute(
            "note.limit",
            json!({"pattern":1,"channel":1,"low":72,"high":83}),
        )
        .unwrap();
    let keys: Vec<u8> = notes(&session).iter().map(|n| n.key).collect();
    assert_eq!(keys, vec![72, 76, 79]);
    session.execute("edit.undo_step", json!({})).unwrap();
    // Arpeggiate the chord in 24-tick steps: C E G C over one beat.
    session
        .execute(
            "note.arpeggiate",
            json!({"pattern":1,"channel":1,"step":24,"direction":"up"}),
        )
        .unwrap();
    let arp: Vec<(i64, u8)> = notes(&session).iter().map(|n| (n.start.0, n.key)).collect();
    assert_eq!(arp, vec![(0, 60), (24, 64), (48, 67), (72, 60)]);
    // Snap to A minor: C# (61) goes down to C (60).
    session
        .execute(
            "note.add",
            json!({"pattern":1,"channel":1,"start":200,"length":24,"key":61}),
        )
        .unwrap();
    session
        .execute(
            "note.snap_scale",
            json!({"pattern":1,"channel":1,"root":9,"scale":"minor"}),
        )
        .unwrap();
    assert!(notes(&session).iter().all(|n| n.key != 61));
}
