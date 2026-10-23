use super::parse;
use crate::{CmdError, Session};
use jet_model::{ChannelId, PatternId, Step, StepSeq};
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
        _ => Err(CmdError::UnknownCommand(id.to_owned())),
    }
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
    if from > to || to >= 512 {
        return Err(CmdError::InvalidParams {
            command: id.into(),
            message: "step range must be ordered and below 512".into(),
        });
    }
    session.edit_project(|project| {
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
        let seq = pattern
            .steps
            .entry(ChannelId(channel))
            .or_insert_with(|| StepSeq { cells: Vec::new() });
        if seq.cells.len() <= to {
            seq.cells.resize_with(to + 1, Step::default);
        }
        for cell in &mut seq.cells[from..=to] {
            cell.on = on;
        }
        Ok(())
    })
}
