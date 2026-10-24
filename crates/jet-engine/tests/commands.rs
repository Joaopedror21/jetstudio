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
