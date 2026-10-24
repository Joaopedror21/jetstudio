//! Arrangements [Manual: playlist, "Arrangements"]: "Arrangements share the Channel Rack and
//! Mixer layout"; the Playlist title menu offers Clone, Rename, Delete and Add one.

use super::{invalid, parse};
use crate::{CmdError, Session};
use jet_model::{Arrangement, ArrangementId, Marker, Project};
use jet_time::Tick;
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IdParams {
    id: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Rename {
    id: u64,
    name: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Add {
    #[serde(default)]
    name: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Merge {
    /// Arrangement merged into the active one (it stays intact).
    source: u64,
    /// "start", "current" (timeline selection start) or "end" (after the current data).
    position: String,
    /// "insert", "merge" or "replace".
    mode: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MarkerAdd {
    at: i64,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MarkerRef {
    index: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MarkerAction {
    index: usize,
    action: jet_model::MarkerAction,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MarkerRename {
    index: usize,
    name: String,
}

fn active_mut(project: &mut Project) -> Result<&mut Arrangement, CmdError> {
    let id = project.active_arrangement;
    project
        .arrangements
        .iter_mut()
        .find(|a| a.id == id)
        .ok_or_else(|| CmdError::Failed("arrangement does not exist".into()))
}

pub(super) fn run(session: &mut Session, id: &str, params: Value) -> Result<bool, CmdError> {
    match id {
        // "Add one - Add an empty Arrangement to the project."
        "arrangement.add" => {
            let p: Add = parse(id, params)?;
            session.edit_project(|project| {
                let new_id = project.ids.arrangement();
                let name = p
                    .name
                    .filter(|n| !n.trim().is_empty())
                    .unwrap_or_else(|| format!("Arrangement {}", project.arrangements.len() + 1));
                project.arrangements.push(Arrangement::new(new_id, name));
                project.active_arrangement = new_id;
                Ok(())
            })
        }
        // "Clone - Copy the current arrangement to a new arrangement."
        "arrangement.clone" => {
            super::empty(id, params)?;
            session.edit_project(|project| {
                let source = project
                    .active()
                    .cloned()
                    .ok_or_else(|| CmdError::Failed("arrangement does not exist".into()))?;
                let new_id = project.ids.arrangement();
                let mut copy = Arrangement::new(new_id, format!("{} (clone)", source.name));
                // Track and clip ids are unique project-wide, so the copy gets fresh ones.
                let mut track_map = Vec::new();
                for track in &source.tracks {
                    let mut t = track.clone();
                    t.id = project.ids.track();
                    track_map.push((track.id, t.id));
                    copy.tracks.push(t);
                }
                for clip in &source.clips {
                    let mut c = clip.clone();
                    c.id = project.ids.clip();
                    c.track = track_map
                        .iter()
                        .find(|(old, _)| *old == clip.track)
                        .map_or(c.track, |(_, new)| *new);
                    copy.clips.push(c);
                }
                copy.markers = source.markers.clone();
                project.arrangements.push(copy);
                project.active_arrangement = new_id;
                Ok(())
            })
        }
        "arrangement.select" => {
            let p: IdParams = parse(id, params)?;
            session.edit_project(|project| {
                if !project
                    .arrangements
                    .iter()
                    .any(|a| a.id == ArrangementId(p.id))
                {
                    return Err(CmdError::Failed("arrangement does not exist".into()));
                }
                project.active_arrangement = ArrangementId(p.id);
                Ok(())
            })
        }
        "arrangement.rename" => {
            let p: Rename = parse(id, params)?;
            if p.name.trim().is_empty() {
                return Err(invalid(id, "name cannot be empty"));
            }
            session.edit_project(|project| {
                let arrangement = project
                    .arrangements
                    .iter_mut()
                    .find(|a| a.id == ArrangementId(p.id))
                    .ok_or_else(|| CmdError::Failed("arrangement does not exist".into()))?;
                arrangement.name = p.name;
                Ok(())
            })
        }
        // "Delete - Deletes the current arrangement."
        "arrangement.delete" => {
            let p: IdParams = parse(id, params)?;
            session.edit_project(|project| {
                if project.arrangements.len() <= 1 {
                    return Err(CmdError::Disabled(
                        "The last arrangement cannot be deleted".into(),
                    ));
                }
                let before = project.arrangements.len();
                project.arrangements.retain(|a| a.id != ArrangementId(p.id));
                if project.arrangements.len() == before {
                    return Err(CmdError::Failed("arrangement does not exist".into()));
                }
                if project.active_arrangement == ArrangementId(p.id)
                    && let Some(first) = project.arrangements.first()
                {
                    project.active_arrangement = first.id;
                }
                Ok(())
            })
        }
        // "Merge with - Select another arrangement to Merge with the current Arrangement":
        // "Position - Start ... Current: Merge from the start of the current selection, End: Append
        // to the end of the current Playlist data"; "Mode - Insert: The inserted data moves the
        // existing data to the right. Merge: The merged data is blended with the destination data.
        // Replace: The source data replaces destination arrangement" [Manual: playlist]. Hypothesis
        // H-PL-4: Replace removes the destination clips over the merged span; source clips keep
        // their track row (tracks are added when needed).
        "arrangement.merge" => {
            let p: Merge = parse(id, params)?;
            if !matches!(p.mode.as_str(), "insert" | "merge" | "replace") {
                return Err(invalid(id, "mode must be insert, merge or replace"));
            }
            let selection = session.time_selection();
            session.edit_project(|project| {
                let source = project
                    .arrangements
                    .iter()
                    .find(|a| a.id == ArrangementId(p.source))
                    .cloned()
                    .ok_or_else(|| CmdError::Failed("arrangement does not exist".into()))?;
                if source.id == project.active_arrangement {
                    return Err(CmdError::Failed(
                        "choose another arrangement to merge".into(),
                    ));
                }
                let bar = project.settings.bar_ticks().unwrap_or(384).max(1);
                let span = source.clips.iter().map(|c| c.end().0).max().unwrap_or(0);
                let destination_end = active_mut(project)?
                    .clips
                    .iter()
                    .map(|c| c.end().0)
                    .max()
                    .unwrap_or(0);
                let at = match p.position.as_str() {
                    "start" => 0,
                    "current" => selection.map_or(0, |(start, _)| start),
                    "end" => (destination_end + bar - 1) / bar * bar,
                    _ => return Err(invalid(id, "position must be start, current or end")),
                };
                // Fresh ids first (they come from the project counters).
                let new_ids: Vec<jet_model::ClipId> =
                    source.clips.iter().map(|_| project.ids.clip()).collect();
                let rows_needed = source
                    .clips
                    .iter()
                    .filter_map(|c| source.tracks.iter().position(|t| t.id == c.track))
                    .max()
                    .map_or(0, |r| r + 1);
                let existing = active_mut(project)?.tracks.len();
                let new_tracks: Vec<jet_model::TrackId> = (existing..rows_needed)
                    .map(|_| project.ids.track())
                    .collect();
                let arrangement = active_mut(project)?;
                for (i, track) in new_tracks.into_iter().enumerate() {
                    let name = format!("Track {}", existing + i + 1);
                    arrangement
                        .tracks
                        .push(jet_model::PlaylistTrack::new(track, name));
                }
                match p.mode.as_str() {
                    "insert" => {
                        for clip in &mut arrangement.clips {
                            if clip.start.0 >= at {
                                clip.start = Tick(clip.start.0 + span);
                            }
                        }
                    }
                    "replace" => arrangement
                        .clips
                        .retain(|c| c.end().0 <= at || c.start.0 >= at + span),
                    _ => {}
                }
                for (clip, new_id) in source.clips.iter().zip(new_ids) {
                    let Some(row) = source.tracks.iter().position(|t| t.id == clip.track) else {
                        continue;
                    };
                    let mut placed = clip.clone();
                    placed.id = new_id;
                    placed.track = arrangement.tracks[row].id;
                    placed.start = Tick(clip.start.0 + at);
                    arrangement.clips.push(placed);
                }
                Ok(())
            })
        }
        // "Time Markers - Add by selecting (Alt/Opt+T)" [Manual: playlist].
        "marker.add" => {
            let p: MarkerAdd = parse(id, params)?;
            if p.at < 0 {
                return Err(invalid(id, "at must be >= 0"));
            }
            session.edit_project(|project| {
                let arrangement = active_mut(project)?;
                let name = p
                    .name
                    .unwrap_or_else(|| format!("Marker #{}", arrangement.markers.len() + 1));
                arrangement.markers.push(Marker {
                    at: Tick(p.at),
                    name,
                    action: jet_model::MarkerAction::None,
                });
                arrangement.markers.sort_by_key(|m| m.at);
                Ok(())
            })
        }
        "marker.delete" => {
            let p: MarkerRef = parse(id, params)?;
            session.edit_project(|project| {
                let arrangement = active_mut(project)?;
                if p.index >= arrangement.markers.len() {
                    return Err(CmdError::Failed("marker does not exist".into()));
                }
                arrangement.markers.remove(p.index);
                Ok(())
            })
        }
        // "Action - Set the Markers purpose" [Manual: playlist]. One marker per Start/Loop action:
        // setting one clears the action from the others.
        "marker.action.set" => {
            let p: MarkerAction = parse(id, params)?;
            session.edit_project(|project| {
                let markers = &mut active_mut(project)?.markers;
                if p.index >= markers.len() {
                    return Err(CmdError::Failed("marker does not exist".into()));
                }
                if p.action != jet_model::MarkerAction::None {
                    for marker in markers.iter_mut().filter(|m| m.action == p.action) {
                        marker.action = jet_model::MarkerAction::None;
                    }
                }
                markers[p.index].action = p.action;
                Ok(())
            })
        }
        "marker.rename" => {
            let p: MarkerRename = parse(id, params)?;
            session.edit_project(|project| {
                let marker = active_mut(project)?
                    .markers
                    .get_mut(p.index)
                    .ok_or_else(|| CmdError::Failed("marker does not exist".into()))?;
                marker.name = p.name;
                Ok(())
            })
        }
        _ => Err(CmdError::UnknownCommand(id.to_owned())),
    }
}
