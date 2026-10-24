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
