use super::{invalid, parse};
use crate::{CmdError, Session};
use jet_model::{ChannelId, MAX_STEPS, PatternId, PatternLength, Project, Step, StepSeq};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SetParams {
    pattern: u64,
    channel: u64,
    index: usize,
    on: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PaintParams {
    pattern: u64,
    channel: u64,
    from: usize,
    to: usize,
    on: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PropsParams {
    pattern: u64,
    channel: u64,
    index: usize,
    #[serde(default)]
    velocity: Option<u8>,
    #[serde(default)]
    pan: Option<i16>,
    #[serde(default)]
    release: Option<u8>,
    #[serde(default)]
    pitch: Option<i16>,
    #[serde(default)]
    mod_x: Option<u8>,
    #[serde(default)]
    mod_y: Option<u8>,
    #[serde(default)]
    shift: Option<i16>,
    #[serde(default)]
    note: Option<u8>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FillParams {
    pattern: u64,
    channel: u64,
    every: usize,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ChannelParams {
    pattern: u64,
    channel: u64,
}

pub(super) fn run(session: &mut Session, id: &str, params: Value) -> Result<bool, CmdError> {
    match id {
        "step.set" => {
            let p: SetParams = parse(id, params)?;
            edit(session, id, p.pattern, p.channel, p.index, p.index, p.on)
        }
        "step.paint" => {
            let p: PaintParams = parse(id, params)?;
            edit(session, id, p.pattern, p.channel, p.from, p.to, p.on)
        }
        // Graph Editor values [Manual: channelrack, "Graph Editor"].
        "step.props.set" => {
            let p: PropsParams = parse(id, params)?;
            if p.index >= MAX_STEPS {
                return Err(invalid(id, "step index must be below 512"));
            }
            session.edit_project(|project| {
                let step_ticks = project.settings.step_ticks().unwrap_or(1);
                let seq = sequence_mut(project, p.pattern, p.channel)?;
                if seq.cells.len() <= p.index {
                    seq.cells.resize_with(p.index + 1, Step::default);
                }
                let props = &mut seq.cells[p.index].props;
                if let Some(v) = p.velocity {
                    props.velocity = v;
                }
                if let Some(v) = p.pan {
                    props.pan = v;
                }
                if let Some(v) = p.release {
                    props.release = v;
                }
                if let Some(v) = p.pitch {
                    props.pitch = v;
                }
                if let Some(v) = p.mod_x {
                    props.mod_x = v;
                }
                if let Some(v) = p.mod_y {
                    props.mod_y = v;
                }
                if let Some(v) = p.shift {
                    if i64::from(v) >= step_ticks || v < 0 {
                        return Err(CmdError::Failed("shift must stay within one step".into()));
                    }
                    props.shift = v;
                }
                if let Some(v) = p.note {
                    props.note = v;
                }
                Ok(())
            })
        }
        // "Fill each 2/4/8 steps - Fills step sequence steps according to the setting"
        // [Manual: channelrack, Channel Button menu].
        "step.fill" => {
            let p: FillParams = parse(id, params)?;
            if !matches!(p.every, 1 | 2 | 4 | 8) {
                return Err(invalid(id, "every must be 1, 2, 4 or 8"));
            }
            session.edit_project(|project| {
                let length = visible_steps(project, p.pattern)?;
                let seq = sequence_mut(project, p.pattern, p.channel)?;
                if seq.cells.len() < length {
                    seq.cells.resize_with(length, Step::default);
                }
                for (index, cell) in seq.cells.iter_mut().enumerate().take(length) {
                    cell.on = index % p.every == 0;
                }
                Ok(())
            })
        }
        "step.clear" => {
            let p: ChannelParams = parse(id, params)?;
            session.edit_project(|project| {
                let seq = sequence_mut(project, p.pattern, p.channel)?;
                for cell in &mut seq.cells {
                    cell.on = false;
                }
                Ok(())
            })
        }
        _ => Err(CmdError::UnknownCommand(id.to_owned())),
    }
}

/// Steps shown for the pattern: its explicit length, or at least one bar.
fn visible_steps(project: &Project, pattern: u64) -> Result<usize, CmdError> {
    let found = project
        .pattern(PatternId(pattern))
        .ok_or_else(|| CmdError::Failed("pattern does not exist".into()))?;
    let per_bar =
        usize::from(project.settings.steps_per_beat) * usize::from(project.settings.time_sig.num);
    Ok(match found.length {
        PatternLength::Steps(steps) => usize::from(steps),
        PatternLength::Auto => {
            let longest = found
                .steps
                .values()
                .map(|s| s.cells.len())
                .max()
                .unwrap_or(0);
            longest.max(per_bar).div_ceil(per_bar.max(1)) * per_bar.max(1)
        }
    }
    .min(MAX_STEPS))
}

fn sequence_mut(
    project: &mut Project,
    pattern: u64,
    channel: u64,
) -> Result<&mut StepSeq, CmdError> {
    if !project
        .channels
        .iter()
        .any(|item| item.id == ChannelId(channel))
    {
        return Err(CmdError::Failed("channel does not exist".into()));
    }
    let pattern = project
        .patterns
        .iter_mut()
        .find(|item| item.id == PatternId(pattern))
        .ok_or_else(|| CmdError::Failed("pattern does not exist".into()))?;
    Ok(pattern
        .steps
        .entry(ChannelId(channel))
        .or_insert_with(|| StepSeq { cells: Vec::new() }))
}

fn edit(
    session: &mut Session,
    id: &str,
    pattern: u64,
    channel: u64,
    from: usize,
    to: usize,
    on: bool,
) -> Result<bool, CmdError> {
    if from > to || to >= MAX_STEPS {
        return Err(invalid(id, "step range must be ordered and below 512"));
    }
    session.edit_project(|project| {
        let seq = sequence_mut(project, pattern, channel)?;
        if seq.cells.len() <= to {
            seq.cells.resize_with(to + 1, Step::default);
        }
        // Turning a step off keeps its properties ("Mute removed steps") [Manual: channelrack].
        for cell in &mut seq.cells[from..=to] {
            cell.on = on;
        }
        Ok(())
    })
}
