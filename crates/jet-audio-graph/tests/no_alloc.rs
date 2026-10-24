//! The render callback must not allocate [docs/FINAL_ARCHITECTURE.md §1.5].
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use assert_no_alloc::{AllocDisabler, assert_no_alloc};
use jet_audio_graph::{Meters, Renderer, TransportSync, build_plan};
use jet_dsp::SampleData;
use jet_model::{Channel, ChannelKind, FxKind, FxSlot, Note, Project, StepSeq};
use jet_seq::Source;
use jet_time::Tick;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

#[test]
fn processing_a_busy_project_does_not_allocate() {
    let mut project = Project::new();
    let resource = project.ids.resource();
    project.resources.push(jet_model::Resource {
        id: resource,
        path_rel: "s.wav".into(),
        path_abs_hint: None,
        hash: None,
        status: jet_model::ResourceStatus::Available,
    });
    for n in 0..8 {
        let id = project.ids.channel();
        let mut channel = if n % 2 == 0 {
            Channel::synth(id, "Synth")
        } else {
            Channel::sampler(id, "Sampler")
        };
        if n % 2 == 1 {
            channel.kind = ChannelKind::Sampler {
                sample: Some(resource),
            };
        }
        channel.mixer_target = (n % 3) as u16;
        project.channels.push(channel);
        let mut seq = StepSeq::with_len(16).unwrap();
        for step in (0..16).step_by(1 + n) {
            seq.cells[step].on = true;
        }
        project.patterns[0].steps.insert(id, seq);
        if n == 0 {
            project.patterns[0].notes.insert(
                id,
                (0..32)
                    .map(|i| Note::new(Tick(i * 12), Tick(36), 48 + (i % 24) as u8))
                    .collect(),
            );
        }
    }
    // Every effect type, split over the Master and insert 1 (ten slots per track).
    for (i, name) in FxKind::NAMES.iter().enumerate() {
        let slot = FxSlot {
            kind: FxKind::default_for(name).unwrap(),
            enabled: true,
            mix: 700,
        };
        if i < 8 {
            project.mixer.master.fx.push(slot);
        } else {
            project.mixer.track_mut(1).unwrap().fx.push(slot);
        }
    }
    project
        .mixer
        .track_mut(1)
        .unwrap()
        .sends
        .push(jet_model::MixerSend { to: 2, level: 500 });
    let sample = Arc::new(SampleData::stereo(
        44_100,
        vec![0.3; 30_000],
        vec![-0.3; 30_000],
    ));
    let plan = build_plan(
        &project,
        Source::Pattern(project.patterns[0].id),
        48_000,
        true,
        &move |_| Some(Arc::clone(&sample)),
    );
    let mut renderer = Renderer::new(
        Box::new(plan),
        Arc::new(AtomicU64::new(0)),
        Arc::new(Meters::default()),
    );
    let mut out = vec![0.0_f32; 2 * 512];
    renderer.sync(TransportSync {
        playing: true,
        anchor_tick: 0,
        anchor_frame: 0,
    });
    assert_no_alloc(|| {
        for _ in 0..400 {
            renderer.process(&mut out);
        }
        renderer.sync(TransportSync {
            playing: false,
            anchor_tick: 0,
            anchor_frame: renderer.frame(),
        });
        renderer.process(&mut out);
        renderer.panic();
        renderer.process(&mut out);
    });
    assert!(out.iter().all(|v| v.is_finite()));
}

#[test]
fn song_mode_automation_does_not_allocate() {
    use jet_model::{
        AutoPoint, AutoTarget, AutomationClip, ClipInstance, ClipSource, CurveShape, PlaylistTrack,
    };
    let mut project = Project::new();
    let id = project.ids.channel();
    project.channels.push(Channel::synth(id, "Synth"));
    let mut seq = StepSeq::with_len(16).unwrap();
    seq.cells[0].on = true;
    project.patterns[0].steps.insert(id, seq);
    let track = project.ids.track();
    project.arrangements[0]
        .tracks
        .push(PlaylistTrack::new(track, "T"));
    let clip = project.ids.clip();
    project.arrangements[0].clips.push(ClipInstance::pattern(
        clip,
        track,
        project.patterns[0].id,
        Tick(0),
        Tick(768),
    ));
    for target in [
        AutoTarget::ChannelVolume(id),
        AutoTarget::MixerVolume(0),
        AutoTarget::ChannelPan(id),
    ] {
        let automation = project.ids.automation();
        project.automations.push(AutomationClip {
            id: automation,
            name: "auto".into(),
            target,
            points: vec![
                AutoPoint {
                    at: Tick(0),
                    value: 0,
                    tension: 0,
                    shape: CurveShape::SingleCurve,
                },
                AutoPoint {
                    at: Tick(768),
                    value: 1000,
                    tension: 300,
                    shape: CurveShape::Wave,
                },
            ],
            initial: 500,
            settings: jet_model::AutoSettings::default(),
        });
        let clip = project.ids.clip();
        let mut instance =
            ClipInstance::pattern(clip, track, project.patterns[0].id, Tick(0), Tick(768));
        instance.source = ClipSource::Automation(automation);
        project.arrangements[0].clips.push(instance);
    }
    assert_eq!(project.validate(), Ok(()));
    let plan = build_plan(&project, Source::Song, 48_000, true, &|_| None);
    assert_eq!(plan.automation.len(), 3);
    let mut renderer = Renderer::new(
        Box::new(plan),
        Arc::new(AtomicU64::new(0)),
        Arc::new(Meters::default()),
    );
    let mut out = vec![0.0_f32; 2 * 256];
    renderer.sync(TransportSync {
        playing: true,
        anchor_tick: 0,
        anchor_frame: 0,
    });
    assert_no_alloc(|| {
        for _ in 0..800 {
            renderer.process(&mut out);
        }
    });
    assert!(out.iter().all(|v| v.is_finite()));
}
