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
    /// Absent: the first track free over the clip's span, or a new track.
    #[serde(default)]
    track: Option<u64>,
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
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AddAudio {
    path: std::path::PathBuf,
    /// Omitted: "the first track with enough room to fit the Audio Clip, without overlap"
    /// [Manual: recording_audio, "Audio Clip placement"].
    #[serde(default)]
    track: Option<u64>,
    start: i64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MuteClip {
    id: u64,
    muted: bool,
}

/// The given track, or else the first track with no clip over `from..to` ("the first track with
/// enough room"), or else a new track appended at the bottom. A track id is only allocated when
/// a track is created.
fn place_track(
    project: &mut jet_model::Project,
    track: Option<u64>,
    from: i64,
    to: i64,
) -> Result<TrackId, CmdError> {
    let active = project.active_arrangement;
    let arrangement = arrangement_mut(project, active)?;
    if let Some(track) = track {
        if !arrangement.tracks.iter().any(|t| t.id == TrackId(track)) {
            return Err(CmdError::Failed("track does not exist".into()));
        }
        return Ok(TrackId(track));
    }
    let free = arrangement.tracks.iter().map(|t| t.id).find(|id| {
        !arrangement
            .clips
            .iter()
            .any(|c| c.track == *id && c.start.0 < to && c.end().0 > from)
    });
    if let Some(id) = free {
        return Ok(id);
    }
    let new_track = project.ids.track();
    let arrangement = arrangement_mut(project, active)?;
    let name = format!("Track {}", arrangement.tracks.len() + 1);
    arrangement.tracks.push(PlaylistTrack::new(new_track, name));
    Ok(new_track)
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
        // Solo mutes the other tracks; soloing the only audible track unmutes all
        // [Manual: playlist, "Track Mute/Solo"].
        "playlist.track.solo" => {
            let p: IdParams = parse(id, params)?;
            session.edit_project(|project| {
                let arrangement = active_arrangement_mut(project)?;
                let target = TrackId(p.id);
                if !arrangement.tracks.iter().any(|t| t.id == target) {
                    return Err(CmdError::Failed("track does not exist".into()));
                }
                let soloed = arrangement
                    .tracks
                    .iter()
                    .all(|t| t.mute == (t.id != target));
                for track in &mut arrangement.tracks {
                    track.mute = !soloed && track.id != target;
                }
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
        // Dropping an audio file on the Playlist creates an Audio Clip [Manual:
        // chansettings_sampler, "If you drop the sample on the Playlist you will create an
        // Audio Clip"]. Its length is the sample's duration at the current tempo.
        "playlist.audio.add" => {
            let p: AddAudio = parse(id, params)?;
            if p.start < 0 || !jet_media::is_supported(&p.path) {
                return Err(super::invalid(
                    id,
                    "start must be >= 0 and the file a supported audio format",
                ));
            }
            let data =
                jet_media::decode_file(&p.path).map_err(|e| CmdError::Failed(e.to_string()))?;
            let seconds = data.frames() as f64 / f64::from(data.rate.max(1));
            let base = session
                .path
                .as_deref()
                .and_then(std::path::Path::parent)
                .map(std::path::Path::to_path_buf);
            let mut resource = jet_model::ResourceId(0);
            let changed = session.edit_project(|project| {
                let ticks_per_second = f64::from(project.settings.tempo.0) / 60_000.0
                    * f64::from(project.settings.ppq.0);
                let length = (seconds * ticks_per_second).ceil().max(1.0) as i64;
                resource = project.ids.resource();
                project.resources.push(crate::samples::new_resource(
                    resource,
                    &p.path,
                    base.as_deref(),
                ));
                let channel = project.ids.channel();
                let name = p.path.file_stem().map_or_else(
                    || "Audio clip".to_owned(),
                    |s| s.to_string_lossy().into_owned(),
                );
                project
                    .channels
                    .push(jet_model::Channel::audio_clip(channel, name, resource));
                let active = project.active_arrangement;
                let clip = project.ids.clip();
                let track = place_track(project, p.track, p.start, p.start + length)?;
                let arrangement = arrangement_mut(project, active)?;
                arrangement.clips.push(ClipInstance {
                    id: clip,
                    track,
                    start: Tick(p.start),
                    length: Tick(length),
                    offset: Tick(0),
                    source: ClipSource::Audio(channel),
                    muted: false,
                });
                Ok(())
            })?;
            session.samples.insert(resource, std::sync::Arc::new(data));
            Ok(changed)
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
                let track = place_track(project, p.track, p.start, p.start + p.length)?;
                let clip_id = project.ids.clip();
                let arrangement = arrangement_mut(project, active_id)?;
                arrangement.clips.push(ClipInstance {
                    id: clip_id,
                    track,
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
        // The Mute tool mutes single clips, independent of the track mute [Manual: playlist, "Mute (T)"].
        "playlist.clip.mute.set" => {
            let p: MuteClip = parse(id, params)?;
            session.edit_project(|project| {
                let clip = active_arrangement_mut(project)?
                    .clips
                    .iter_mut()
                    .find(|clip| clip.id == ClipId(p.id))
                    .ok_or_else(|| CmdError::Failed("clip does not exist".into()))?;
                clip.muted = p.muted;
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
