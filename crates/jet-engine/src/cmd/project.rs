use super::parse;
use crate::{CmdError, Session};
use jet_time::Ppq;
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Unsigned {
    value: u32,
}

pub(super) fn run(session: &mut Session, id: &str, params: Value) -> Result<bool, CmdError> {
    let p: Unsigned = parse(id, params)?;
    match id {
        "project.swing.set" => {
            if p.value > 100 {
                return Err(CmdError::InvalidParams {
                    command: id.into(),
                    message: "swing must be 0..=100".into(),
                });
            }
            session.edit_project(|project| {
                project.settings.swing = p.value as u8;
                Ok(())
            })
        }
        "project.ppq.set" => {
            let ppq = Ppq::new(p.value).map_err(|e| CmdError::InvalidParams {
                command: id.into(),
                message: e.to_string(),
            })?;
            session.edit_project(|project| {
                project
                    .set_ppq(ppq)
                    .map_err(|e| CmdError::Failed(e.to_string()))
            })
        }
        _ => Err(CmdError::UnknownCommand(id.to_owned())),
    }
}
