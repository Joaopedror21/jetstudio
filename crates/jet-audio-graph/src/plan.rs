//! Compiling a project into an immutable render plan (on a non-real-time thread).

use crate::MAX_BLOCK;
use jet_dsp::fx::{self, Effect};
use jet_dsp::{SampleData, channel_volume_gain, mixer_fader_gain, send_gain};
use jet_model::{
    ChannelId, ChannelKind, FxKind, MAX_INSERTS, PanLaw, Project, ResourceId, SynthParams,
};
use jet_seq::automation::AutoLane;
use jet_seq::{Sequence, Source};
use std::sync::Arc;

pub const TRACK_COUNT: usize = MAX_INSERTS + 1;

#[derive(Clone, Debug)]
pub enum Instrument {
    /// `None` is an empty Sampler: it plays nothing.
    Sampler(Option<Arc<SampleData>>),
    Synth(SynthParams),
}

#[derive(Clone, Debug)]
pub struct ChannelPlan {
    pub id: ChannelId,
    pub instrument: Instrument,
    pub gain: f32,
    pub pan: i16,
    pub pitch_cents: i16,
    /// Values set by automation for the current block (`None` = the knob's own value).
    pub auto_gain: Option<f32>,
    pub auto_pan: Option<i16>,
    pub auto_pitch: Option<i16>,
    pub root_key: u8,
    /// Index into [`RenderPlan::tracks`].
    pub track_slot: usize,
    pub cut: u8,
    pub cut_by: u8,
}

pub struct FxInstance {
    pub kind: FxKind,
    pub enabled: bool,
    pub mix: f32,
    pub auto_mix: Option<f32>,
    pub effect: Option<Box<dyn Effect>>,
}

pub struct TrackPlan {
    /// Mixer index: 0 = Master, 1..=500 inserts.
    pub index: u16,
    pub gain: f32,
    pub pan: i16,
    pub auto_gain: Option<f32>,
    pub auto_pan: Option<i16>,
    pub mute: bool,
    pub stereo_sep: f32,
    pub invert: bool,
    pub swap: bool,
    /// Slot of the Master track when this track feeds it.
    pub to_master: bool,
    /// (destination slot, gain) for post-fader sends.
    pub sends: Vec<(usize, f32)>,
    pub fx: Vec<FxInstance>,
    pub fx_enabled: bool,
    pub left: Vec<f32>,
    pub right: Vec<f32>,
    /// Wet buffer used by FX slots with a mix below 100%.
    pub scratch_left: Vec<f32>,
    pub scratch_right: Vec<f32>,
}

/// Where an automation lane writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutoApply {
    ChannelVolume(usize),
    ChannelPan(usize),
    ChannelPitch(usize),
    TrackVolume(usize),
    TrackPan(usize),
    FxMix(usize, usize),
}

pub struct PlanLane {
    pub lane: AutoLane,
    pub apply: AutoApply,
}

/// Everything the audio thread needs, built off the audio thread.
pub struct RenderPlan {
    pub rate: u32,
    pub ppq: u32,
    pub tempo_milli: u32,
    pub pan_law: PanLaw,
    pub sequence: Sequence,
    pub looping: bool,
    pub channels: Vec<ChannelPlan>,
    /// Sorted by channel id for lookup without allocation.
    pub channel_index: Vec<(ChannelId, usize)>,
    /// Processing order: every source precedes its destinations; Master is last.
    pub tracks: Vec<TrackPlan>,
    pub master_slot: usize,
    /// Song-mode automation; empty in Pattern mode.
    pub automation: Vec<PlanLane>,
}

impl RenderPlan {
    pub fn channel(&self, id: ChannelId) -> Option<&ChannelPlan> {
        self.channel_index
            .binary_search_by_key(&id, |(key, _)| *key)
            .ok()
            .and_then(|i| self.channels.get(self.channel_index[i].1))
    }
}

fn topological(project: &Project, used: &[bool; TRACK_COUNT]) -> Vec<u16> {
    // Depth-first post-order over send edges; the model rejects cycles.
    fn visit(
        index: u16,
        project: &Project,
        used: &[bool; TRACK_COUNT],
        mark: &mut [u8; TRACK_COUNT],
        order: &mut Vec<u16>,
    ) {
        let i = usize::from(index);
        if mark[i] != 0 {
            return;
        }
        mark[i] = 1;
        if let Some(track) = project.mixer.track(index) {
            for send in &track.sends {
                if used[usize::from(send.to)] {
                    visit(send.to, project, used, mark, order);
                }
            }
        }
        mark[i] = 2;
        order.push(index);
    }
    let mut mark = [0u8; TRACK_COUNT];
    let mut order = Vec::new();
    for index in 1..TRACK_COUNT {
        if used[index] {
            visit(index as u16, project, used, &mut mark, &mut order);
        }
    }
    order.reverse(); // sources before destinations
    order.push(0);
    order
}

/// Compile `project` for `source`. `sample` resolves decoded resources.
pub fn build_plan(
    project: &Project,
    source: Source,
    rate: u32,
    looping: bool,
    sample: &dyn Fn(ResourceId) -> Option<Arc<SampleData>>,
) -> RenderPlan {
    let sequence = jet_seq::compile(project, source);
    let mut used = [false; TRACK_COUNT];
    for channel in &project.channels {
        used[usize::from(channel.mixer_target).min(MAX_INSERTS)] = true;
    }
    // Send destinations are reachable, so they are processed too.
    let mut changed = true;
    while changed {
        changed = false;
        for index in 1..TRACK_COUNT {
            if !used[index] {
                continue;
            }
            if let Some(track) = project.mixer.inserts.get(index - 1) {
                for send in &track.sends {
                    let to = usize::from(send.to);
                    if to < TRACK_COUNT && !used[to] {
                        used[to] = true;
                        changed = true;
                    }
                }
            }
        }
    }
    used[0] = false;
    let order = topological(project, &used);
    let mut slot_of = vec![usize::MAX; TRACK_COUNT];
    for (slot, index) in order.iter().enumerate() {
        slot_of[usize::from(*index)] = slot;
    }
    let master_slot = order.len() - 1;
    let tempo_bpm = f64::from(project.settings.tempo.0) / 1000.0;
    let tracks = order
        .iter()
        .map(|&index| {
            let track = project
                .mixer
                .track(index)
                .map(std::borrow::Cow::into_owned)
                .unwrap_or_else(|| jet_model::MixerInsert::default_for(index));
            TrackPlan {
                index,
                gain: mixer_fader_gain(track.volume) as f32,
                pan: track.pan,
                auto_gain: None,
                auto_pan: None,
                mute: track.mute,
                stereo_sep: f32::from(track.stereo_sep) / 1000.0,
                invert: track.invert_phase,
                swap: track.swap_lr,
                to_master: index != 0 && track.to_master,
                sends: track
                    .sends
                    .iter()
                    .filter_map(|send| {
                        let slot = *slot_of.get(usize::from(send.to))?;
                        (slot != usize::MAX).then_some((slot, send_gain(send.level) as f32))
                    })
                    .collect(),
                fx: track
                    .fx
                    .iter()
                    .map(|slot| FxInstance {
                        kind: slot.kind.clone(),
                        enabled: slot.enabled,
                        mix: f32::from(slot.mix) / 1000.0,
                        auto_mix: None,
                        effect: Some(fx::build(&slot.kind, rate, tempo_bpm)),
                    })
                    .collect(),
                fx_enabled: track.fx_enabled,
                left: vec![0.0; MAX_BLOCK],
                right: vec![0.0; MAX_BLOCK],
                scratch_left: vec![0.0; MAX_BLOCK],
                scratch_right: vec![0.0; MAX_BLOCK],
            }
        })
        .collect();
    let channels: Vec<ChannelPlan> = project
        .channels
        .iter()
        .map(|channel| ChannelPlan {
            id: channel.id,
            instrument: match &channel.kind {
                ChannelKind::Sampler { sample: id } => Instrument::Sampler(id.and_then(sample)),
                ChannelKind::Synth(params) => Instrument::Synth(params.clone()),
            },
            gain: channel_volume_gain(channel.volume) as f32,
            pan: channel.pan,
            pitch_cents: channel.pitch,
            auto_gain: None,
            auto_pan: None,
            auto_pitch: None,
            root_key: channel.root_key,
            track_slot: slot_of
                .get(usize::from(channel.mixer_target))
                .copied()
                .filter(|slot| *slot != usize::MAX)
                .unwrap_or(master_slot),
            cut: channel.cut,
            cut_by: channel.cut_by,
        })
        .collect();
    let mut channel_index: Vec<(ChannelId, usize)> = channels
        .iter()
        .enumerate()
        .map(|(i, channel)| (channel.id, i))
        .collect();
    channel_index.sort_by_key(|(id, _)| *id);
    let automation = if source == Source::Song {
        jet_seq::automation::lanes(project)
            .into_iter()
            .filter_map(|lane| {
                use jet_model::AutoTarget;
                let channel = |id| {
                    channel_index
                        .iter()
                        .find(|(c, _)| *c == id)
                        .map(|(_, i)| *i)
                };
                let track = |index: u16| {
                    slot_of
                        .get(usize::from(index))
                        .copied()
                        .filter(|s| *s != usize::MAX)
                };
                let apply = match lane.target {
                    AutoTarget::ChannelVolume(id) => AutoApply::ChannelVolume(channel(id)?),
                    AutoTarget::ChannelPan(id) => AutoApply::ChannelPan(channel(id)?),
                    AutoTarget::ChannelPitch(id) => AutoApply::ChannelPitch(channel(id)?),
                    AutoTarget::MixerVolume(index) => AutoApply::TrackVolume(track(index)?),
                    AutoTarget::MixerPan(index) => AutoApply::TrackPan(track(index)?),
                    AutoTarget::FxMix { track: index, slot } => {
                        AutoApply::FxMix(track(index)?, usize::from(slot))
                    }
                };
                Some(PlanLane { lane, apply })
            })
            .collect()
    } else {
        Vec::new()
    };
    RenderPlan {
        automation,
        rate,
        ppq: project.settings.ppq.0,
        tempo_milli: project.settings.tempo.0,
        pan_law: project.settings.pan_law,
        sequence,
        looping,
        channels,
        channel_index,
        tracks,
        master_slot,
    }
}
