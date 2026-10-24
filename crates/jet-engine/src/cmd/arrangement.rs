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
