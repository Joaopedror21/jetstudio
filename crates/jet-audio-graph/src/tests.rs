use super::*;
use jet_dsp::{SampleData, gain_to_db};
use jet_model::{Channel, ChannelKind, FxKind, FxSlot, MixerSend, Project, ResourceId, StepSeq};
use jet_seq::Source;
use jet_time::Tempo;

const RATE: u32 = 48_000;

fn dc_sample(frames: usize) -> Arc<SampleData> {
    Arc::new(SampleData::mono(RATE, vec![1.0; frames]))
}

/// One sampler channel with a full-scale DC sample and the given steps on.
fn project(steps: &[usize]) -> Project {
    let mut project = Project::new();
    project.settings.tempo = Tempo(120_000);
    let id = project.ids.channel();
    let resource = project.ids.resource();
    project.resources.push(jet_model::Resource {
        id: resource,
        path_rel: "dc.wav".into(),
        path_abs_hint: None,
        hash: None,
        status: jet_model::ResourceStatus::Available,
    });
    let mut channel = Channel::sampler(id, "DC");
    channel.kind = ChannelKind::Sampler {
        sample: Some(resource),
    };
    project.channels.push(channel);
    let mut seq = StepSeq::with_len(16).unwrap();
    for &step in steps {
        seq.cells[step].on = true;
    }
    project.patterns[0].steps.insert(id, seq);
    project
}

fn plan(project: &Project, sample: Arc<SampleData>) -> RenderPlan {
    build_plan(
        project,
        Source::Pattern(project.patterns[0].id),
        RATE,
        true,
        &move |_: ResourceId| Some(Arc::clone(&sample)),
    )
}

fn run(renderer: &mut Renderer, frames: usize, block: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(frames * 2);
    let mut buf = vec![0.0; block * 2];
    while out.len() < frames * 2 {
        let n = (frames - out.len() / 2).min(block);
        renderer.process(&mut buf[..n * 2]);
        out.extend_from_slice(&buf[..n * 2]);
    }
    out
}

fn playing(plan: RenderPlan) -> Renderer {
    let mut renderer = Renderer::new(
        Box::new(plan),
        Arc::new(AtomicU64::new(0)),
        Arc::new(Meters::default()),
    );
    renderer.sync(TransportSync {
        playing: true,
        anchor_tick: 0,
        anchor_frame: 0,
    });
    renderer
}

fn onsets(out: &[f32]) -> Vec<usize> {
    let mut found = Vec::new();
    let mut previous = 0.0_f32;
    for (i, frame) in out.as_chunks::<2>().0.iter().enumerate() {
        if previous.abs() < 1e-6 && frame[0].abs() > 1e-6 {
            found.push(i);
        }
        previous = frame[0];
    }
    found
}

#[test]
fn default_sampler_plays_8_2_db_under_the_recorded_level() {
    // [Manual: mixer_levelsandmixing] 5.2 dB volume + 3 dB circular pan law.
    let project = project(&[0]);
    let mut renderer = playing(plan(&project, dc_sample(2_000)));
    let out = run(&mut renderer, 4_000, 512);
    let level = out[200];
    assert!(
        (gain_to_db(f64::from(level)) - (-8.2)).abs() < 0.03,
        "{}",
        gain_to_db(f64::from(level))
    );
    assert_eq!(out[200], out[201], "center pan is symmetric");
}

#[test]
fn steps_start_on_their_exact_frame() {
    // 120 BPM, 96 PPQ, 48 kHz: one 16th step = 24 ticks = 6000 frames.
    let project = project(&[0, 4, 9]);
    let mut renderer = playing(plan(&project, dc_sample(100)));
    let out = run(&mut renderer, 60_000, 333);
    assert_eq!(onsets(&out), vec![0, 24_000, 54_000]);
}

#[test]
fn rendering_is_independent_of_block_size() {
    let mut project = project(&[0, 3, 6, 10, 15]);
    project.settings.swing = 60;
    let reference = run(
        &mut playing(plan(&project, dc_sample(3_000))),
        100_000,
        1024,
    );
    for block in [1, 64, 77, 1000] {
        let other = run(
            &mut playing(plan(&project, dc_sample(3_000))),
            100_000,
            block,
        );
        assert_eq!(reference, other, "block size {block}");
    }
}

#[test]
fn pattern_mode_loops_the_pattern() {
    let project = project(&[0]);
    let mut renderer = playing(plan(&project, dc_sample(100)));
    // One 4/4 bar at 120 BPM is 2 s = 96 000 frames; three bars give three kicks.
    let out = run(&mut renderer, 96_000 * 3, 512);
    assert_eq!(onsets(&out), vec![0, 96_000, 192_000]);
}

#[test]
fn a_late_sync_still_fires_the_events_since_its_anchor() {
    let project = project(&[0]);
    let mut renderer = Renderer::new(
        Box::new(plan(&project, dc_sample(100))),
        Arc::new(AtomicU64::new(0)),
        Arc::new(Meters::default()),
    );
    let mut silent = vec![0.0; 1024];
    renderer.process(&mut silent);
    renderer.sync(TransportSync {
        playing: true,
        anchor_tick: 0,
        anchor_frame: 0,
    });
    let out = run(&mut renderer, 1000, 256);
    assert_eq!(onsets(&out), vec![0]);
}

#[test]
fn muting_a_channel_or_its_insert_silences_it() {
    let mut project = project(&[0]);
    project.channels[0].mute = true;
    let out = run(&mut playing(plan(&project, dc_sample(500))), 2000, 256);
    assert!(out.iter().all(|v| *v == 0.0));

    let mut project = self::project(&[0]);
    project.channels[0].mixer_target = 3;
    project.mixer.track_mut(3).unwrap().mute = true;
    let out = run(&mut playing(plan(&project, dc_sample(500))), 2000, 256);
    assert!(out.iter().all(|v| *v == 0.0));
}

#[test]
fn sends_route_post_fader_and_master_routing_can_be_disabled() {
    let mut project = project(&[0]);
    project.channels[0].mixer_target = 1;
    {
        let one = project.mixer.track_mut(1).unwrap();
        one.to_master = false;
        one.sends.push(MixerSend {
            to: 2,
            level: jet_model::MIXER_UNITY,
        });
    }
    project.mixer.track_mut(2).unwrap();
    let direct = run(
        &mut playing(plan(&self::project(&[0]), dc_sample(500))),
        400,
        128,
    );
    let routed = run(&mut playing(plan(&project, dc_sample(500))), 400, 128);
    assert!(
        (direct[100] - routed[100]).abs() < 1e-6,
        "unity send = direct path"
    );
    project.mixer.track_mut(1).unwrap().volume = 0;
    let silent = run(&mut playing(plan(&project, dc_sample(500))), 400, 128);
    assert!(silent.iter().all(|v| *v == 0.0), "send is post-fader");
}

#[test]
fn cut_groups_choke_the_cut_by_channel() {
    let mut project = project(&[0]);
    let closed = project.ids.channel();
    let mut channel = Channel::sampler(closed, "Closed");
    channel.kind = project.channels[0].kind.clone();
    channel.cut = 1;
    project.channels.push(channel);
    project.channels[0].cut_by = 1; // the open hat is cut by group 1
    let mut seq = StepSeq::with_len(16).unwrap();
    seq.cells[1].on = true; // closed hat at frame 6000
    project.patterns[0].steps.insert(closed, seq);
    let out = run(&mut playing(plan(&project, dc_sample(20_000))), 12_000, 256);
    // Before the cut both play; 10 ms after it only the closed hat remains.
    let alone = out[2 * 100];
    let after = out[2 * (6000 + 600)];
    assert!(
        (after - alone).abs() < 1e-5,
        "open hat choked: {alone} vs {after}"
    );
}

#[test]
fn stop_releases_synth_voices_and_keeps_sampler_tails() {
    let mut project = project(&[0]);
    let id = project.ids.channel();
    project.channels.push(Channel::synth(id, "Synth"));
    let mut renderer = playing(plan(&project, dc_sample(48_000)));
    run(&mut renderer, 1000, 256);
    renderer.sync(TransportSync {
        playing: false,
        anchor_tick: 0,
        anchor_frame: 1000,
    });
    let out = run(&mut renderer, 20_000, 256);
    assert!(
        out[2 * 19_000].abs() > 0.0,
        "sampler continues after note-off"
    );
}

#[test]
fn effects_keep_their_state_across_plan_swaps() {
    let mut project = project(&[0]);
    project.mixer.master.fx.push(FxSlot {
        kind: FxKind::default_for("delay").unwrap(),
        enabled: true,
        mix: 1000,
    });
    let mut renderer = playing(plan(&project, dc_sample(10)));
    run(&mut renderer, 10_000, 256);
    let old = renderer.swap_plan(Box::new(plan(&project, dc_sample(10))));
    drop(old);
    // The delay line still holds the first hit: an echo arrives 0.75 beat (18 000 frames) later.
    let out = run(&mut renderer, 20_000, 256);
    assert!(out[2 * 8_000..2 * 8_010].iter().any(|v| v.abs() > 0.0));
}

#[test]
fn offline_render_leaves_the_remainder_and_matches_live_playback() {
    let project = project(&[15]);
    let live = run(&mut playing(plan(&project, dc_sample(20_000))), 96_000, 512);
    let rendered = render_offline(plan(&project, dc_sample(20_000)), 384, Tail::LeaveRemainder);
    assert!(rendered.len() > 96_000 * 2, "tail extends past the bar");
    assert_eq!(&rendered[..96_000 * 2], &live[..]);
    let cut = render_offline(plan(&project, dc_sample(20_000)), 384, Tail::Cut);
    assert_eq!(cut.len(), 96_000 * 2);
    let wrapped = render_offline(plan(&project, dc_sample(20_000)), 384, Tail::Wrap);
    assert_eq!(wrapped.len(), 96_000 * 2);
    assert!(wrapped[0].abs() > 0.0, "decay wrapped into the start");
}

#[test]
fn meters_report_master_peaks() {
    let project = project(&[0]);
    let meters = Arc::new(Meters::default());
    let mut renderer = Renderer::new(
        Box::new(plan(&project, dc_sample(500))),
        Arc::new(AtomicU64::new(0)),
        Arc::clone(&meters),
    );
    renderer.sync(TransportSync {
        playing: true,
        anchor_tick: 0,
        anchor_frame: 0,
    });
    run(&mut renderer, 1000, 256);
    let (l, r) = meters.take(0);
    assert!(l > 0.3 && (l - r).abs() < 1e-6);
    assert_eq!(meters.take(0), (0.0, 0.0), "take resets");
}

#[test]
fn live_notes_sound_until_note_off() {
    let mut project = Project::new();
    let id = project.ids.channel();
    project.channels.push(Channel::synth(id, "Synth"));
    let plan = build_plan(
        &project,
        Source::Pattern(project.patterns[0].id),
        RATE,
        true,
        &|_: ResourceId| None,
    );
    let mut renderer = Renderer::new(
        Box::new(plan),
        Arc::new(AtomicU64::new(0)),
        Arc::new(Meters::default()),
    );
    renderer.live_note_on(id, 60, 100);
    let held = run(&mut renderer, 48_000, 512);
    assert!(
        held[2 * 40_000..].iter().any(|v| v.abs() > 0.01),
        "sustains while held"
    );
    renderer.live_note_off(id, 60);
    let after = run(&mut renderer, 48_000, 512);
    assert!(
        after[2 * 24_000..].iter().all(|v| v.abs() < 1e-4),
        "released"
    );
    assert_eq!(renderer.active_voices(), 0);
}
