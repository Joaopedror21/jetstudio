use super::{empty, parse};
use crate::{CmdError, Session};
use jet_model::{Pattern, PatternId, PatternLength};
use serde::Deserialize;
use serde_json::Value;

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
struct LengthParams {
    id: u64,
    steps: u16,
}

pub(super) fn run(session: &mut Session, id: &str, params: Value) -> Result<bool, CmdError> {
    match id {
        "pattern.select" => {
            let p: IdParams = parse(id, params)?;
            let target = PatternId(p.id);
            if !session
                .project
                .patterns
                .iter()
                .any(|pattern| pattern.id == target)
            {
                return Err(CmdError::Failed("pattern does not exist".into()));
            }
            let changed = session.selected_pattern != target;
            session.selected_pattern = target;
            Ok(changed)
        }
        "pattern.next" | "pattern.prev" => {
            empty(id, params)?;
            let len = session.project.patterns.len();
            if len == 0 {
                return Err(CmdError::Disabled("No patterns".into()));
            }
            let index = session
                .project
                .patterns
                .iter()
                .position(|p| p.id == session.selected_pattern)
                .unwrap_or(0);
            let next = if id == "pattern.next" {
                (index + 1) % len
            } else {
                (index + len - 1) % len
            };
            session.selected_pattern = session.project.patterns[next].id;
            Ok(true)
        }
        "pattern.next_empty" => {
            empty(id, params)?;
            let mut new_id = PatternId(0);
            let changed = session.edit_project(|project| {
                if project.patterns.len() >= 999 {
                    return Err(CmdError::Disabled("Pattern limit reached".into()));
                }
                new_id = project.ids.pattern();
                project
                    .patterns
                    .push(Pattern::new(new_id, format!("Pattern {}", new_id.0)));
                Ok(())
            })?;
            session.selected_pattern = new_id;
            Ok(changed)
        }
        "pattern.clone" => {
            let p: IdParams = parse(id, params)?;
            let mut new_id = PatternId(0);
            let changed = session.edit_project(|project| {
                let source = project
                    .patterns
                    .iter()
                    .find(|pattern| pattern.id == PatternId(p.id))
                    .cloned()
                    .ok_or_else(|| CmdError::Failed("pattern does not exist".into()))?;
                if project.patterns.len() >= 999 {
                    return Err(CmdError::Disabled("Pattern limit reached".into()));
                }
                new_id = project.ids.pattern();
                let mut clone = source;
                clone.id = new_id;
                clone.name = format!("{} #{}", clone.name, new_id.0);
                project.patterns.push(clone);
                Ok(())
            })?;
            session.selected_pattern = new_id;
            Ok(changed)
        }
        "pattern.rename" => {
            let p: RenameParams = parse(id, params)?;
            if p.name.trim().is_empty() {
                return Err(CmdError::InvalidParams {
                    command: id.into(),
                    message: "name cannot be empty".into(),
                });
            }
            session.edit_project(|project| {
                let pattern = project
                    .patterns
                    .iter_mut()
                    .find(|pattern| pattern.id == PatternId(p.id))
                    .ok_or_else(|| CmdError::Failed("pattern does not exist".into()))?;
                pattern.name = p.name;
                Ok(())
            })
        }
        "pattern.delete" => {
            let p: IdParams = parse(id, params)?;
            let target = PatternId(p.id);
            let changed = session.edit_project(|project| {
                if project.patterns.len() <= 1 {
                    return Err(CmdError::Disabled(
                        "The last pattern cannot be deleted".into(),
                    ));
                }
                if project
                    .arrangements
                    .iter()
                    .flat_map(|arr| arr.clips.iter())
                    .any(|clip| clip.source == jet_model::ClipSource::Pattern(target))
                {
                    return Err(CmdError::Disabled("Pattern is used by a clip".into()));
                }
                let before = project.patterns.len();
                project.patterns.retain(|pattern| pattern.id != target);
                if project.patterns.len() == before {
                    return Err(CmdError::Failed("pattern does not exist".into()));
                }
                Ok(())
            })?;
            if session.selected_pattern == target
                && let Some(pattern) = session.project.patterns.first()
            {
                session.selected_pattern = pattern.id;
            }
            Ok(changed)
        }
        "pattern.length.set" => {
            let p: LengthParams = parse(id, params)?;
            if !(1..=512).contains(&p.steps) {
                return Err(CmdError::InvalidParams {
                    command: id.into(),
                    message: "steps must be 1..=512".into(),
                });
            }
            session.edit_project(|project| {
                let pattern = project
                    .patterns
                    .iter_mut()
                    .find(|pattern| pattern.id == PatternId(p.id))
                    .ok_or_else(|| CmdError::Failed("pattern does not exist".into()))?;
                pattern.length = PatternLength::Steps(p.steps);
                Ok(())
            })
        }
        // "When set to Auto the length of the pattern will be set by the end of the last bar
        // with data in it" [Manual: channelrack, "4. Pattern Length"].
        "pattern.length.auto" => {
            let p: IdParams = parse(id, params)?;
            session.edit_project(|project| {
                let pattern = project
                    .patterns
                    .iter_mut()
                    .find(|pattern| pattern.id == PatternId(p.id))
                    .ok_or_else(|| CmdError::Failed("pattern does not exist".into()))?;
                pattern.length = PatternLength::Auto;
                Ok(())
            })
        }
        _ => Err(CmdError::UnknownCommand(id.to_owned())),
    }
}
