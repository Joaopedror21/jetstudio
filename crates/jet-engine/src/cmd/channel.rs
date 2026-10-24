use super::{invalid, parse};
use crate::samples::new_resource;
use crate::{CmdError, Session};
use jet_model::{Channel, ChannelId, ChannelKind, MAX_INSERTS, MAX_KEY, Project, SynthParams};
use serde::Deserialize;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AddParams {
    name: String,
    #[serde(default)]
    kind: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IdParams {
    id: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RenameParams {
    id: u64,
    name: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MuteParams {
    id: u64,
    mute: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UnsignedParams {
    id: u64,
    value: u16,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SignedParams {
    id: u64,
    value: i16,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MoveParams {
    id: u64,
    delta: i32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CutParams {
    id: u64,
    cut: u8,
    cut_by: u8,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ColorParams {
    id: u64,
    color: Option<u32>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SampleParams {
    id: u64,
    path: PathBuf,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SynthSetParams {
    id: u64,
    params: SynthParams,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct KindParams {
    id: u64,
    kind: String,
}

fn kind_for(id: &str, name: Option<&str>) -> Result<ChannelKind, CmdError> {
    match name.unwrap_or("sampler") {
        "sampler" => Ok(ChannelKind::Sampler { sample: None }),
        "synth" => Ok(ChannelKind::Synth(SynthParams::default())),
        other => Err(invalid(id, format!("unknown channel kind {other:?}"))),
    }
}

fn non_empty(id: &str, name: &str) -> Result<(), CmdError> {
    if name.trim().is_empty() {
        Err(invalid(id, "name cannot be empty"))
    } else {
        Ok(())
    }
}

pub(super) fn run(session: &mut Session, id: &str, params: Value) -> Result<bool, CmdError> {
    match id {
        "channel.add" => {
            let p: AddParams = parse(id, params)?;
            non_empty(id, &p.name)?;
            let kind = kind_for(id, p.kind.as_deref())?;
            let mut new_id = ChannelId(0);
            let changed = session.edit_project(|project| {
                new_id = project.ids.channel();
                project
                    .channels
                    .push(Channel::with_kind(new_id, p.name, kind));
                Ok(())
            })?;
            session.selected_channel = Some(new_id);
            Ok(changed)
        }
        "channel.remove" => {
            let p: IdParams = parse(id, params)?;
            let target = ChannelId(p.id);
            let changed = session.edit_project(|project| {
                let before = project.channels.len();
                project.channels.retain(|channel| channel.id != target);
                if project.channels.len() == before {
                    return Err(CmdError::Failed("channel does not exist".into()));
                }
                for pattern in &mut project.patterns {
                    pattern.steps.remove(&target);
                    pattern.notes.remove(&target);
                }
                Ok(())
            })?;
            if session.selected_channel == Some(target) {
                session.selected_channel = None;
            }
            Ok(changed)
        }
        "channel.select" => {
            let p: IdParams = parse(id, params)?;
            if session.project.channel(ChannelId(p.id)).is_none() {
                return Err(CmdError::Failed("channel does not exist".into()));
            }
            let changed = session.selected_channel != Some(ChannelId(p.id));
            session.selected_channel = Some(ChannelId(p.id));
            Ok(changed)
        }
        "channel.rename" => {
            let p: RenameParams = parse(id, params)?;
            non_empty(id, &p.name)?;
            session.edit_project(|project| {
                channel_mut(project, p.id)?.name = p.name;
                Ok(())
            })
        }
        "channel.mute.set" => {
            let p: MuteParams = parse(id, params)?;
            session.edit_project(|project| {
                channel_mut(project, p.id)?.mute = p.mute;
                Ok(())
            })
        }
        // Solo mutes every other channel; soloing the only unmuted channel unmutes all
        // [Manual: channelrack, "Repeat this action to unsolo the Channel (unmute all)"].
        "channel.solo" => {
            let p: IdParams = parse(id, params)?;
            session.edit_project(|project| {
                let target = ChannelId(p.id);
                if project.channel(target).is_none() {
                    return Err(CmdError::Failed("channel does not exist".into()));
                }
                let soloed = project
                    .channels
                    .iter()
                    .all(|channel| channel.mute == (channel.id != target));
                for channel in &mut project.channels {
                    channel.mute = !soloed && channel.id != target;
                }
                Ok(())
            })
        }
        "channel.volume.set" => {
            let p: UnsignedParams = parse(id, params)?;
            if p.value > 1000 {
                return Err(invalid(id, "volume must be 0..=1000"));
            }
            session.edit_project(|project| {
                channel_mut(project, p.id)?.volume = p.value;
                Ok(())
            })
        }
        "channel.pan.set" => {
            let p: SignedParams = parse(id, params)?;
            if !(-1000..=1000).contains(&p.value) {
                return Err(invalid(id, "pan must be -1000..=1000"));
            }
            session.edit_project(|project| {
                channel_mut(project, p.id)?.pan = p.value;
                Ok(())
            })
        }
        "channel.pitch.set" => {
            let p: SignedParams = parse(id, params)?;
            if !(-4800..=4800).contains(&p.value) {
                return Err(invalid(id, "pitch must be -4800..=4800 cents"));
            }
            session.edit_project(|project| {
                channel_mut(project, p.id)?.pitch = p.value;
                Ok(())
            })
        }
        "channel.root_key.set" => {
            let p: UnsignedParams = parse(id, params)?;
            if p.value > u16::from(MAX_KEY) {
                return Err(invalid(id, "root key must be 0..=131"));
            }
            session.edit_project(|project| {
                channel_mut(project, p.id)?.root_key = p.value as u8;
                Ok(())
            })
        }
        "channel.swing_mix.set" => {
            let p: UnsignedParams = parse(id, params)?;
            if p.value > 1000 {
                return Err(invalid(id, "swing mix must be 0..=1000"));
            }
            session.edit_project(|project| {
                channel_mut(project, p.id)?.swing_mix = p.value;
                Ok(())
            })
        }
        "channel.cut.set" => {
            let p: CutParams = parse(id, params)?;
            session.edit_project(|project| {
                let channel = channel_mut(project, p.id)?;
                channel.cut = p.cut;
                channel.cut_by = p.cut_by;
                Ok(())
            })
        }
        "channel.color.set" => {
            let p: ColorParams = parse(id, params)?;
            if p.color.is_some_and(|c| c > 0xFF_FFFF) {
                return Err(invalid(id, "color must be 0xRRGGBB"));
            }
            session.edit_project(|project| {
                channel_mut(project, p.id)?.color = p.color;
                Ok(())
            })
        }
        "channel.mixer_target.set" => {
            let p: UnsignedParams = parse(id, params)?;
            if usize::from(p.value) > MAX_INSERTS {
                return Err(invalid(id, "mixer target must be 0..=500"));
            }
            session.edit_project(|project| {
                channel_mut(project, p.id)?.mixer_target = p.value;
                Ok(())
            })
        }
        // "Assigns selected Channel/s to the first available Mixer Track/s" [Manual: channelrack].
        "channel.mixer_target.free" => {
            let p: IdParams = parse(id, params)?;
            session.edit_project(|project| {
                let used: std::collections::BTreeSet<u16> = project
                    .channels
                    .iter()
                    .filter(|c| c.id != ChannelId(p.id))
                    .map(|c| c.mixer_target)
                    .collect();
                let free = (1..=MAX_INSERTS as u16)
                    .find(|index| !used.contains(index))
                    .ok_or_else(|| CmdError::Disabled("No free mixer track".into()))?;
                let name = channel_mut(project, p.id)?.name.clone();
                channel_mut(project, p.id)?.mixer_target = free;
                if let Some(track) = project.mixer.track_mut(free) {
                    track.name = name;
                }
                Ok(())
            })
        }
        // "Makes a copy of the current Channel including settings and generator type"
        // inserted after it; notes are not copied [Manual: channelrack, "Clone selected"].
        "channel.clone" => {
            let p: IdParams = parse(id, params)?;
            let mut new_id = ChannelId(0);
            let changed = session.edit_project(|project| {
                let index = channel_index(project, p.id)?;
                new_id = project.ids.channel();
                let mut clone = project.channels[index].clone();
                clone.id = new_id;
                clone.name = format!("{} #2", clone.name);
                project.channels.insert(index + 1, clone);
                Ok(())
            })?;
            session.selected_channel = Some(new_id);
            Ok(changed)
        }
        // Alt+Up/Down moves the channel one slot [Manual: channelrack, "Move selected up/down"].
        "channel.move" => {
            let p: MoveParams = parse(id, params)?;
            session.edit_project(|project| {
                let index = channel_index(project, p.id)? as i64;
                let last = project.channels.len() as i64 - 1;
                let target = (index + i64::from(p.delta)).clamp(0, last.max(0));
                let channel = project.channels.remove(index as usize);
                project.channels.insert(target as usize, channel);
                Ok(())
            })
        }
        "channel.kind.set" => {
            let p: KindParams = parse(id, params)?;
            let kind = kind_for(id, Some(&p.kind))?;
            session.edit_project(|project| {
                channel_mut(project, p.id)?.kind = kind;
                Ok(())
            })
        }
        "channel.synth.set" => {
            let p: SynthSetParams = parse(id, params)?;
            if !p.params.valid() {
                return Err(invalid(id, "synth parameters out of range"));
            }
            session.edit_project(|project| {
                let channel = channel_mut(project, p.id)?;
                if !matches!(channel.kind, ChannelKind::Synth(_)) {
                    return Err(CmdError::Failed("channel is not a synth".into()));
                }
                channel.kind = ChannelKind::Synth(p.params);
                Ok(())
            })
        }
        // Load a .wav/.mp3/.ogg/.flac into a Sampler channel [Manual: chansettings_sampler].
        "channel.sample.load" => {
            let p: SampleParams = parse(id, params)?;
            if !jet_media::is_supported(&p.path) {
                return Err(invalid(id, "unsupported audio format"));
            }
            if session.project.channel(ChannelId(p.id)).is_none() {
                return Err(CmdError::Failed("channel does not exist".into()));
            }
            let data =
                jet_media::decode_file(&p.path).map_err(|e| CmdError::Failed(e.to_string()))?;
            let base = session
                .path
                .as_deref()
                .and_then(std::path::Path::parent)
                .map(std::path::Path::to_path_buf);
            let mut resource_id = jet_model::ResourceId(0);
            let changed = session.edit_project(|project| {
                resource_id = project.ids.resource();
                project
                    .resources
                    .push(new_resource(resource_id, &p.path, base.as_deref()));
                let channel = channel_mut(project, p.id)?;
                channel.kind = ChannelKind::Sampler {
                    sample: Some(resource_id),
                };
                if let Some(stem) = p.path.file_stem().and_then(|s| s.to_str()) {
                    channel.name = stem.to_owned();
                }
                Ok(())
            })?;
            session.samples.insert(resource_id, Arc::new(data));
            Ok(changed)
        }
        // "Removes the sample ... it does not delete the original sample file".
        "channel.sample.remove" => {
            let p: IdParams = parse(id, params)?;
            session.edit_project(|project| {
                let channel = channel_mut(project, p.id)?;
                if !matches!(channel.kind, ChannelKind::Sampler { .. }) {
                    return Err(CmdError::Failed("channel is not a sampler".into()));
                }
                channel.kind = ChannelKind::Sampler { sample: None };
                Ok(())
            })
        }
        _ => Err(CmdError::UnknownCommand(id.to_owned())),
    }
}

fn channel_index(project: &Project, id: u64) -> Result<usize, CmdError> {
    project
        .channels
        .iter()
        .position(|channel| channel.id == ChannelId(id))
        .ok_or_else(|| CmdError::Failed("channel does not exist".into()))
}

pub(super) fn channel_mut(project: &mut Project, id: u64) -> Result<&mut Channel, CmdError> {
    project
        .channels
        .iter_mut()
        .find(|channel| channel.id == ChannelId(id))
        .ok_or_else(|| CmdError::Failed("channel does not exist".into()))
}
