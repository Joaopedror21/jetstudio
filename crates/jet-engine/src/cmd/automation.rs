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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EventRecord {
    pattern: u64,
    target: AutoTarget,
    /// The previous tick recorded in this gesture: events after it up to `at` are replaced.
    #[serde(default)]
    from: Option<i64>,
    at: i64,
    value: u16,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EventToClip {
    pattern: u64,
    target: AutoTarget,
}

/// Ramer-Douglas-Peucker reduction of (tick, value) points to those that keep the shape within
/// `tolerance` value units (textbook polyline simplification).
fn reduce(points: &[(i64, f64)], tolerance: f64) -> Vec<usize> {
    fn walk(points: &[(i64, f64)], a: usize, b: usize, tolerance: f64, keep: &mut Vec<usize>) {
        let (x0, y0) = points[a];
        let (x1, y1) = points[b];
        let mut worst = (0.0, a);
        for (i, (x, y)) in points.iter().enumerate().take(b).skip(a + 1) {
            let line = if x1 == x0 {
                y0
            } else {
                y0 + (y1 - y0) * (x - x0) as f64 / (x1 - x0) as f64
            };
            let d = (y - line).abs();
            if d > worst.0 {
                worst = (d, i);
            }
        }
        if worst.0 > tolerance {
            walk(points, a, worst.1, tolerance, keep);
            keep.push(worst.1);
            walk(points, worst.1, b, tolerance, keep);
        }
    }
    if points.len() < 3 {
        return (0..points.len()).collect();
    }
    let mut keep = vec![0];
    walk(points, 0, points.len() - 1, tolerance, &mut keep);
    keep.push(points.len() - 1);
    keep.sort_unstable();
    keep
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
        // Live automation recording into the pattern's event data [Manual: recording_automation].
        "automation.event.record" => {
            let p: EventRecord = parse(id, params)?;
            if p.at < 0 || p.value > 1000 {
                return Err(invalid(id, "at must be >= 0 and value 0..=1000"));
            }
            session.edit_project(|project| {
                if p.target.current(project).is_none() {
                    return Err(CmdError::Failed("automation target does not exist".into()));
                }
                let pattern = project
                    .patterns
                    .iter_mut()
                    .find(|pat| pat.id == jet_model::PatternId(p.pattern))
                    .ok_or_else(|| CmdError::Failed("pattern does not exist".into()))?;
                let index = match pattern.events.iter().position(|l| l.target == p.target) {
                    Some(i) => i,
                    None => {
                        pattern.events.push(jet_model::EventLane {
                            target: p.target,
                            points: Vec::new(),
                        });
                        pattern.events.len() - 1
                    }
                };
                let lane = &mut pattern.events[index];
                // Moving a control while recording overwrites what was there.
                let from = p.from.unwrap_or(p.at - 1).min(p.at - 1);
                lane.points.retain(|(at, _)| at.0 <= from || at.0 > p.at);
                lane.points.push((Tick(p.at), p.value));
                lane.points.sort_by_key(|(at, _)| *at);
                Ok(())
            })
        }
        // "Turn into Automation Clip": the event data becomes a clip, reduced to a manageable
        // number of points ("live automation generates points equivalent to the PPQ setting ...
        // That's not a practical number of points to edit"); it starts where the pattern's first
        // clip does (or at 0) and the event lane is removed (hypothesis H-AUT-5).
        "automation.event.to_clip" => {
            let p: EventToClip = parse(id, params)?;
            session.edit_project(|project| {
                let pattern_id = jet_model::PatternId(p.pattern);
                let pattern = project
                    .patterns
                    .iter_mut()
                    .find(|pat| pat.id == pattern_id)
                    .ok_or_else(|| CmdError::Failed("pattern does not exist".into()))?;
                let index = pattern
                    .events
                    .iter()
                    .position(|l| l.target == p.target)
                    .ok_or_else(|| CmdError::Failed("no event data for this control".into()))?;
                let lane = pattern.events.remove(index);
                let raw: Vec<(i64, f64)> = lane
                    .points
                    .iter()
                    .map(|(at, v)| (at.0, f64::from(*v)))
                    .collect();
                let kept = reduce(&raw, 5.0);
                let mut points: Vec<AutoPoint> = kept
                    .into_iter()
                    .map(|i| AutoPoint {
                        at: Tick(raw[i].0),
                        value: raw[i].1 as u16,
                        tension: 0,
                        shape: CurveShape::SingleCurve,
                    })
                    .collect();
                if points.first().is_none_or(|p| p.at.0 > 0) {
                    let value = points.first().map_or(0, |p| p.value);
                    points.insert(
                        0,
                        AutoPoint {
                            at: Tick(0),
                            value,
                            tension: 0,
                            shape: CurveShape::SingleCurve,
                        },
                    );
                }
                let length = points.last().map_or(1, |p| p.at.0).max(1);
                let start = project
                    .active()
                    .and_then(|a| {
                        a.clips
                            .iter()
                            .filter(|c| c.source == ClipSource::Pattern(pattern_id))
                            .map(|c| c.start.0)
                            .min()
                    })
                    .unwrap_or(0);
                let automation = project.ids.automation();
                let name = p.target.label(project);
                let initial = points[0].value;
                project.automations.push(AutomationClip {
                    id: automation,
                    name: name.clone(),
                    target: p.target,
                    points,
                    initial,
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
                    start: Tick(start),
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
