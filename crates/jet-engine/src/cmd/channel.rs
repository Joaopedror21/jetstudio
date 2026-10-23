use super::parse;
use crate::{CmdError, Session};
use jet_model::{Channel, ChannelId, ChannelKind};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AddParams {
    name: String,
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

pub(super) fn run(session: &mut Session, id: &str, params: Value) -> Result<bool, CmdError> {
    match id {
        "channel.add" => {
            let p: AddParams = parse(id, params)?;
            if p.name.trim().is_empty() {
                return Err(CmdError::InvalidParams {
                    command: id.into(),
                    message: "name cannot be empty".into(),
                });
            }
            let mut new_id = ChannelId(0);
            let changed = session.edit_project(|project| {
                new_id = project.ids.channel();
                project.channels.push(Channel {
                    id: new_id,
                    name: p.name,
                    color: None,
                    kind: ChannelKind::Sampler { sample: None },
                    mute: false,
                    volume: 1000,
                    pan: 0,
                    mixer_target: 0,
                    group: None,
                });
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
                }
                Ok(())
            })?;
            if session.selected_channel == Some(target) {
                session.selected_channel = None;
            }
            Ok(changed)
        }
        "channel.rename" => {
            let p: RenameParams = parse(id, params)?;
            if p.name.trim().is_empty() {
                return Err(CmdError::InvalidParams {
                    command: id.into(),
                    message: "name cannot be empty".into(),
                });
            }
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
        "channel.volume.set" => {
            let p: UnsignedParams = parse(id, params)?;
            if p.value > 1000 {
                return Err(CmdError::InvalidParams {
                    command: id.into(),
                    message: "volume must be 0..=1000".into(),
                });
            }
            session.edit_project(|project| {
                channel_mut(project, p.id)?.volume = p.value;
                Ok(())
            })
        }
        "channel.pan.set" => {
            let p: SignedParams = parse(id, params)?;
            if !(-1000..=1000).contains(&p.value) {
                return Err(CmdError::InvalidParams {
                    command: id.into(),
                    message: "pan must be -1000..=1000".into(),
                });
            }
            session.edit_project(|project| {
                channel_mut(project, p.id)?.pan = p.value;
                Ok(())
            })
        }
        "channel.mixer_target.set" => {
            let p: UnsignedParams = parse(id, params)?;
            if p.value > 500 {
                return Err(CmdError::InvalidParams {
                    command: id.into(),
                    message: "mixer target must be 0..=500".into(),
                });
            }
            session.edit_project(|project| {
                channel_mut(project, p.id)?.mixer_target = p.value;
                Ok(())
            })
        }
        _ => Err(CmdError::UnknownCommand(id.to_owned())),
    }
}

fn channel_mut(project: &mut jet_model::Project, id: u64) -> Result<&mut Channel, CmdError> {
    project
        .channels
        .iter_mut()
        .find(|channel| channel.id == ChannelId(id))
        .ok_or_else(|| CmdError::Failed("channel does not exist".into()))
}
