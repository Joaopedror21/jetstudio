use super::parse;
use crate::{CmdError, Session};
use jet_model::{Arrangement, ClipId, ClipInstance, ClipSource, PatternId, PlaylistTrack, TrackId};
use jet_time::Tick;
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AddTrack {
    name: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RenameTrack {
    id: u64,
    name: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MuteTrack {
    id: u64,
    mute: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AddClip {
    track: u64,
    pattern: u64,
    start: i64,
    length: i64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MoveClip {
    id: u64,
    track: u64,
    start: i64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResizeClip {
    id: u64,
    length: i64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IdParams {
    id: u64,
}

pub(super) fn run(session: &mut Session, id: &str, params: Value) -> Result<bool, CmdError> {
    match id {
        "playlist.track.add" => {
            let p: AddTrack = parse(id, params)?;
            if p.name.trim().is_empty() {
                return Err(CmdError::InvalidParams {
                    command: id.into(),
                    message: "name cannot be empty".into(),
                });
            }
            session.edit_project(|project| {
                let active_id = project.active_arrangement;
                let track = project.ids.track();
                arrangement_mut(project, active_id)?
                    .tracks
                    .push(PlaylistTrack::new(track, p.name));
                Ok(())
            })
        }
        "playlist.track.rename" => {
            let p: RenameTrack = parse(id, params)?;
            if p.name.trim().is_empty() {
                return Err(CmdError::InvalidParams {
                    command: id.into(),
                    message: "name cannot be empty".into(),
                });
            }
            session.edit_project(|project| {
                track_mut(project, TrackId(p.id))?.name = p.name;
                Ok(())
            })
        }
        "playlist.track.mute.set" => {
            let p: MuteTrack = parse(id, params)?;
            session.edit_project(|project| {
                track_mut(project, TrackId(p.id))?.mute = p.mute;
                Ok(())
            })
        }
        "playlist.clip.add" => {
            let p: AddClip = parse(id, params)?;
            if p.start < 0 || p.length <= 0 {
                return Err(CmdError::InvalidParams {
                    command: id.into(),
                    message: "start must be >= 0 and length > 0".into(),
                });
            }
            session.edit_project(|project| {
                if !project
                    .patterns
                    .iter()
                    .any(|pattern| pattern.id == PatternId(p.pattern))
                {
                    return Err(CmdError::Failed("pattern does not exist".into()));
                }
                let active_id = project.active_arrangement;
                let clip_id = project.ids.clip();
                let arrangement = arrangement_mut(project, active_id)?;
                if !arrangement
                    .tracks
                    .iter()
                    .any(|track| track.id == TrackId(p.track))
                {
                    return Err(CmdError::Failed("track does not exist".into()));
                }
                arrangement.clips.push(ClipInstance {
                    id: clip_id,
                    track: TrackId(p.track),
                    start: Tick(p.start),
                    length: Tick(p.length),
                    offset: Tick(0),
                    source: ClipSource::Pattern(PatternId(p.pattern)),
                    muted: false,
                });
                Ok(())
            })
        }
        "playlist.clip.move" => {
            let p: MoveClip = parse(id, params)?;
            if p.start < 0 {
                return Err(CmdError::InvalidParams {
                    command: id.into(),
                    message: "start must be >= 0".into(),
                });
            }
            session.edit_project(|project| {
                let arrangement = active_arrangement_mut(project)?;
                if !arrangement
                    .tracks
                    .iter()
                    .any(|track| track.id == TrackId(p.track))
                {
                    return Err(CmdError::Failed("track does not exist".into()));
                }
                let clip = arrangement
                    .clips
                    .iter_mut()
                    .find(|clip| clip.id == ClipId(p.id))
                    .ok_or_else(|| CmdError::Failed("clip does not exist".into()))?;
                clip.track = TrackId(p.track);
                clip.start = Tick(p.start);
                Ok(())
            })
        }
        "playlist.clip.resize" => {
            let p: ResizeClip = parse(id, params)?;
            if p.length <= 0 {
                return Err(CmdError::InvalidParams {
                    command: id.into(),
                    message: "length must be > 0".into(),
                });
            }
            session.edit_project(|project| {
                let clip = active_arrangement_mut(project)?
                    .clips
                    .iter_mut()
                    .find(|clip| clip.id == ClipId(p.id))
                    .ok_or_else(|| CmdError::Failed("clip does not exist".into()))?;
                clip.length = Tick(p.length);
                Ok(())
            })
        }
        "playlist.clip.delete" => {
            let p: IdParams = parse(id, params)?;
            session.edit_project(|project| {
                let arrangement = active_arrangement_mut(project)?;
                let before = arrangement.clips.len();
                arrangement.clips.retain(|clip| clip.id != ClipId(p.id));
                if before == arrangement.clips.len() {
                    return Err(CmdError::Failed("clip does not exist".into()));
                }
                Ok(())
            })
        }
        _ => Err(CmdError::UnknownCommand(id.to_owned())),
    }
}

fn arrangement_mut(
    project: &mut jet_model::Project,
    id: jet_model::ArrangementId,
) -> Result<&mut Arrangement, CmdError> {
    project
        .arrangements
        .iter_mut()
        .find(|arrangement| arrangement.id == id)
        .ok_or_else(|| CmdError::Failed("arrangement does not exist".into()))
}

fn active_arrangement_mut(project: &mut jet_model::Project) -> Result<&mut Arrangement, CmdError> {
    arrangement_mut(project, project.active_arrangement)
}

fn track_mut(
    project: &mut jet_model::Project,
    id: TrackId,
) -> Result<&mut PlaylistTrack, CmdError> {
    active_arrangement_mut(project)?
        .tracks
        .iter_mut()
        .find(|track| track.id == id)
        .ok_or_else(|| CmdError::Failed("track does not exist".into()))
}
