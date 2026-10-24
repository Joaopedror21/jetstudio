#![allow(clippy::unwrap_used, clippy::expect_used)]

use jet_engine::{ManualClock, Session, TransportState};
use jet_model::{ChannelId, ClipSource, PatternId};
use jet_time::Tick;
use serde_json::json;

#[test]
fn strict_command_validation_preserves_project_and_revision() {
    let mut session = Session::new_default();
    let before = session.project().clone();
    let revision = session.revision();
    let result = session.execute("channel.add", json!({"name":"Kick", "unknown": 1}));
    assert!(result.is_err());
    assert_eq!(session.project(), &before);
    assert_eq!(session.revision(), revision);
}

#[test]
fn patterns_keep_independent_steps_and_undo_toggle_restores_them() {
    let mut session = Session::new_default();
    session
        .execute("channel.add", json!({"name":"Kick"}))
        .unwrap();
    session
        .execute(
            "step.set",
            json!({"pattern":1,"channel":1,"index":0,"on":true}),
        )
        .unwrap();
    session.execute("pattern.next_empty", json!({})).unwrap();
    session
        .execute(
            "step.set",
            json!({"pattern":2,"channel":1,"index":1,"on":true}),
        )
        .unwrap();
    assert!(session.project().patterns[0].steps[&ChannelId(1)].cells[0].on);
    assert!(!session.project().patterns[1].steps[&ChannelId(1)].cells[0].on);
    assert!(session.project().patterns[1].steps[&ChannelId(1)].cells[1].on);

    session.execute("edit.undo_toggle", json!({})).unwrap();
    assert!(
        !session.project().patterns[1]
            .steps
            .contains_key(&ChannelId(1))
    );
    session.execute("edit.undo_toggle", json!({})).unwrap();
    assert!(session.project().patterns[1].steps[&ChannelId(1)].cells[1].on);
}

#[test]
fn playlist_clips_share_pattern_source() {
    let mut session = Session::new_default();
    session
        .execute("playlist.track.add", json!({"name":"Track 1"}))
        .unwrap();
    for start in [0, 384] {
        session
            .execute(
                "playlist.clip.add",
                json!({"track":1,"pattern":1,"start":start,"length":384}),
            )
            .unwrap();
    }
    let clips = &session.project().arrangements[0].clips;
    assert_eq!(clips.len(), 2);
    assert!(
        clips
            .iter()
            .all(|clip| clip.source == ClipSource::Pattern(PatternId(1)))
    );
    session
        .execute("playlist.clip.move", json!({"id":2,"track":1,"start":768}))
        .unwrap();
    assert_eq!(session.project().arrangements[0].clips[1].start, Tick(768));
}

#[test]
fn silent_transport_uses_injected_clock_and_pause_freezes_position() {
    let clock = ManualClock::new(48_000);
    let mut session = Session::with_clock(Box::new(clock.clone()));
    // Explicit tempo: half a second at 120 BPM is one beat (96 ticks at the default PPQ).
    session
        .execute("transport.tempo.set", json!({"milli_bpm": 120_000}))
        .unwrap();
    session.execute("transport.play_stop", json!({})).unwrap();
    clock.set_frames(24_000);
    assert_eq!(session.transport_snapshot().position, Tick(96));
    session.execute("transport.pause", json!({})).unwrap();
    assert_eq!(session.transport_snapshot().state, TransportState::Paused);
    clock.set_frames(48_000);
    assert_eq!(session.transport_snapshot().position, Tick(96));
}

#[test]
fn a_continuous_gesture_undoes_as_one_step() {
    let mut session = Session::new_default();
    session
        .execute("channel.add", json!({"name":"Kick"}))
        .unwrap();
    for value in [700, 600, 500, 400] {
        session
            .execute_gesture(
                "knob.volume.1",
                "channel.volume.set",
                json!({"id":1,"value":value}),
            )
            .unwrap();
    }
    session.end_gesture();
    assert_eq!(session.project().channels[0].volume, 400);
    session.execute("edit.undo_step", json!({})).unwrap();
    assert_eq!(
        session.project().channels[0].volume,
        jet_model::DEFAULT_CHANNEL_VOLUME
    );
    session.execute("edit.undo_step", json!({})).unwrap();
    assert!(session.project().channels.is_empty());
}

#[test]
fn input_frames_map_to_unwrapped_ticks_while_playing() {
    let clock = ManualClock::new(48_000);
    let mut session = Session::with_clock(Box::new(clock.clone()));
    session
        .execute("transport.tempo.set", json!({"milli_bpm": 120_000}))
        .unwrap();
    assert_eq!(session.tick_at_frame(0), None, "stopped");
    clock.set_frames(1_000);
    session.execute("transport.play_stop", json!({})).unwrap();
    // 250 frames per tick at 120 BPM and 96 PPQ; the anchor is frame 1 000.
    assert_eq!(session.tick_at_frame(1_000), Some(0));
    assert_eq!(session.tick_at_frame(1_249), Some(0));
    assert_eq!(session.tick_at_frame(1_250), Some(1));
    // Past the pattern loop the tick keeps counting (the recorder wraps it itself).
    assert_eq!(session.tick_at_frame(1_000 + 48_000 * 4), Some(768));
    assert_eq!(session.now_frame(), 1_000);
}

#[test]
fn a_pattern_clip_without_a_track_goes_to_the_first_free_one() {
    let mut session = Session::with_clock(Box::new(ManualClock::new(48_000)));
    session
        .execute("playlist.track.add", json!({"name": "Track 1"}))
        .unwrap();
    session
        .execute(
            "playlist.clip.add",
            json!({"pattern": 1, "start": 0, "length": 384}),
        )
        .unwrap();
    // The only track is busy over 0..384, so a second clip there gets a new track.
    session
        .execute(
            "playlist.clip.add",
            json!({"pattern": 1, "start": 192, "length": 384}),
        )
        .unwrap();
    let arrangement = session.project().active().unwrap();
    assert_eq!(arrangement.tracks.len(), 2);
    assert_ne!(arrangement.clips[0].track, arrangement.clips[1].track);
}

#[test]
fn remember_seek_time_returns_stop_to_the_set_position() {
    let clock = ManualClock::new(48_000);
    let mut session = Session::with_clock(Box::new(clock.clone()));
    session
        .execute("transport.position.set", json!({"tick": 192}))
        .unwrap();
    session.execute("transport.play_stop", json!({})).unwrap();
    session.execute("transport.stop", json!({})).unwrap();
    assert_eq!(
        session.transport_snapshot().position,
        Tick(0),
        "off: back to 0"
    );

    session
        .execute("transport.remember_seek.toggle", json!({}))
        .unwrap();
    assert!(session.remember_seek());
    session.execute("transport.play_stop", json!({})).unwrap();
    clock.set_frames(48_000);
    session.execute("transport.play_stop", json!({})).unwrap();
    assert_eq!(
        session.transport_snapshot().position,
        Tick(192),
        "on: Stop returns to the position set last"
    );
}

#[test]
fn slicing_and_slipping_clips() {
    let mut session = Session::with_clock(Box::new(ManualClock::new(48_000)));
    session
        .execute("playlist.track.add", json!({"name": "Track 1"}))
        .unwrap();
    session
        .execute(
            "playlist.clip.add",
            json!({"track": 1, "pattern": 1, "start": 384, "length": 768}),
        )
        .unwrap();
    let id = session.project().active().unwrap().clips[0].id.0;
    session
        .execute("playlist.clip.slice", json!({"id": id, "at": 576}))
        .unwrap();
    let clips = &session.project().active().unwrap().clips;
    assert_eq!(clips.len(), 2);
    assert_eq!(
        (clips[0].start.0, clips[0].length.0, clips[0].offset.0),
        (384, 192, 0)
    );
    assert_eq!(
        (clips[1].start.0, clips[1].length.0, clips[1].offset.0),
        (576, 576, 192)
    );
    assert!(
        session
            .execute("playlist.clip.slice", json!({"id": id, "at": 384}))
            .is_err(),
        "a slice on the clip edge is refused"
    );
    session
        .execute("playlist.clip.slip", json!({"id": id, "offset": 96}))
        .unwrap();
    let clip = &session.project().active().unwrap().clips[0];
    assert_eq!((clip.start.0, clip.length.0, clip.offset.0), (384, 192, 96));
    session.execute("edit.undo_step", json!({})).unwrap();
    session.execute("edit.undo_step", json!({})).unwrap();
    assert_eq!(session.project().active().unwrap().clips.len(), 1);
}
