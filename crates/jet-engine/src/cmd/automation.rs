//! Automation Clips [Manual: playlist_automationclip].

use super::{invalid, parse};
use crate::{CmdError, Session};
use jet_model::{
    AutoPoint, AutoTarget, AutomationClip, AutomationId, ClipInstance, ClipSource, CurveShape,
    PlaylistTrack, Project,
};
use jet_time::Tick;
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Create {
    target: AutoTarget,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AddPoint {
    id: u64,
    at: i64,
    value: u16,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MovePoint {
    id: u64,
    index: usize,
    at: i64,
    value: u16,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PointRef {
    id: u64,
    index: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Shape {
    id: u64,
    index: usize,
    shape: CurveShape,
    tension: i16,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IdParams {
    id: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SettingsParams {
    id: u64,
    settings: jet_model::AutoSettings,
}

fn clip_mut(project: &mut Project, id: u64) -> Result<&mut AutomationClip, CmdError> {
    project
        .automations
        .iter_mut()
        .find(|a| a.id == AutomationId(id))
        .ok_or_else(|| CmdError::Failed("automation clip does not exist".into()))
}

fn sort(points: &mut [AutoPoint]) {
    points.sort_by_key(|p| p.at);
}

pub(super) fn run(session: &mut Session, id: &str, params: Value) -> Result<bool, CmdError> {
    match id {
        // "Create automation clip": the clip spans "the entire length containing data" and
        // starts at the control's current value [Manual: playlist_automationclip].
        "automation.create" => {
            let p: Create = parse(id, params)?;
            session.edit_project(|project| {
                let current = p
                    .target
                    .current(project)
                    .ok_or_else(|| CmdError::Failed("automation target does not exist".into()))?;
                let bar = project.settings.bar_ticks().unwrap_or(384).max(1);
                // An empty song still gets a usable clip: four bars (our choice).
                let length = jet_seq::song_length(project).max(4 * bar);
                let automation = project.ids.automation();
                let name = p.target.label(project);
                project.automations.push(AutomationClip {
                    id: automation,
                    name: name.clone(),
                    target: p.target,
                    points: vec![
                        AutoPoint {
                            at: Tick(0),
                            value: current,
                            tension: 0,
                            shape: CurveShape::SingleCurve,
                        },
                        AutoPoint {
                            at: Tick(length),
                            value: current,
                            tension: 0,
                            shape: CurveShape::SingleCurve,
                        },
                    ],
                    initial: current,
                    settings: jet_model::AutoSettings::default(),
                });
                let track = project.ids.track();
                let clip = project.ids.clip();
                let active = project.active_arrangement;
                let arrangement = project
                    .arrangements
                    .iter_mut()
                    .find(|a| a.id == active)
                    .ok_or_else(|| CmdError::Failed("arrangement does not exist".into()))?;
                arrangement.tracks.push(PlaylistTrack::new(track, name));
                arrangement.clips.push(ClipInstance {
                    id: clip,
                    track,
                    start: Tick(0),
                    length: Tick(length),
                    offset: Tick(0),
                    source: ClipSource::Automation(automation),
                    muted: false,
                    audio: jet_model::AudioClipProps::default(),
                });
                Ok(())
            })
        }
        // "Add control points - (Right-Click) on the clip area".
        "automation.point.add" => {
            let p: AddPoint = parse(id, params)?;
            if p.at < 0 || p.value > 1000 {
                return Err(invalid(id, "at must be >= 0 and value 0..=1000"));
            }
            session.edit_project(|project| {
                let clip = clip_mut(project, p.id)?;
                clip.points.push(AutoPoint {
                    at: Tick(p.at),
                    value: p.value,
                    tension: 0,
                    shape: CurveShape::SingleCurve,
                });
                sort(&mut clip.points);
                Ok(())
            })
        }
        // "Move control points - Left-click the control point and drag".
        "automation.point.move" => {
            let p: MovePoint = parse(id, params)?;
            if p.at < 0 || p.value > 1000 {
                return Err(invalid(id, "at must be >= 0 and value 0..=1000"));
            }
            session.edit_project(|project| {
                let clip = clip_mut(project, p.id)?;
                let point = clip
                    .points
                    .get_mut(p.index)
                    .ok_or_else(|| CmdError::Failed("point index out of range".into()))?;
                // The first point stays at the clip start (our choice, keeps the envelope anchored).
                point.at = if p.index == 0 { Tick(0) } else { Tick(p.at) };
                point.value = p.value;
                sort(&mut clip.points);
                Ok(())
            })
        }
        "automation.point.delete" => {
            let p: PointRef = parse(id, params)?;
            session.edit_project(|project| {
                let clip = clip_mut(project, p.id)?;
                if clip.points.len() <= 1 {
                    return Err(CmdError::Disabled(
                        "An automation clip keeps at least one point".into(),
                    ));
                }
                if p.index >= clip.points.len() {
                    return Err(CmdError::Failed("point index out of range".into()));
                }
                clip.points.remove(p.index);
                Ok(())
            })
        }
        // "Change curve type - Right-Click the right-most control point of the segment".
        "automation.point.shape.set" => {
            let p: Shape = parse(id, params)?;
            if !(-1000..=1000).contains(&p.tension) {
                return Err(invalid(id, "tension must be -1000..=1000"));
            }
            session.edit_project(|project| {
                let clip = clip_mut(project, p.id)?;
                let point = clip
                    .points
                    .get_mut(p.index)
                    .ok_or_else(|| CmdError::Failed("point index out of range".into()))?;
                point.shape = p.shape;
                point.tension = p.tension;
                Ok(())
            })
        }
        // Removing the clip definition removes its Playlist instances too.
        // Automation Clip Settings: Min, Max, Time, LFO [Manual: playlist_automationclip].
        "automation.settings.set" => {
            let p: SettingsParams = parse(id, params)?;
            if !p.settings.valid() {
                return Err(invalid(id, "settings out of range"));
            }
            session.edit_project(|project| {
                clip_mut(project, p.id)?.settings = p.settings;
                Ok(())
            })
        }
        // "Flip vertically - Inverts the current envelope."
        "automation.flip" => {
            let p: IdParams = parse(id, params)?;
            session.edit_project(|project| {
                for point in &mut clip_mut(project, p.id)?.points {
                    point.value = 1000 - point.value;
                }
                Ok(())
            })
        }
        // "Normalize levels - Scales the envelope so the highest and/or lowest levels reach
        // +/- 100%."
        "automation.normalize" => {
            let p: IdParams = parse(id, params)?;
            session.edit_project(|project| {
                let points = &mut clip_mut(project, p.id)?.points;
                let low = points.iter().map(|p| p.value).min().unwrap_or(0);
                let high = points.iter().map(|p| p.value).max().unwrap_or(1000);
                if high > low {
                    for point in points.iter_mut() {
                        point.value =
                            (u32::from(point.value - low) * 1000 / u32::from(high - low)) as u16;
                    }
                }
                Ok(())
            })
        }
        "automation.delete" => {
            let p: IdParams = parse(id, params)?;
            session.edit_project(|project| {
                let target = AutomationId(p.id);
                let before = project.automations.len();
                project.automations.retain(|a| a.id != target);
                if project.automations.len() == before {
                    return Err(CmdError::Failed("automation clip does not exist".into()));
                }
                for arrangement in &mut project.arrangements {
                    arrangement
                        .clips
                        .retain(|c| c.source != ClipSource::Automation(target));
                }
                Ok(())
            })
        }
        _ => Err(CmdError::UnknownCommand(id.to_owned())),
    }
}
