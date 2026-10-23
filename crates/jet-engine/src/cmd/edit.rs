use super::empty;
use crate::{CmdError, Session};
use serde_json::Value;
use std::sync::Arc;

pub(super) fn run(session: &mut Session, id: &str, params: Value) -> Result<bool, CmdError> {
    empty(id, params)?;
    let current = Arc::clone(&session.project);
    let next = match id {
        "edit.undo_toggle" => session.history.toggle_last(current),
        "edit.undo_step" => session.history.undo(current),
        "edit.redo_step" => session.history.redo(current),
        _ => return Err(CmdError::UnknownCommand(id.to_owned())),
    };
    if let Some(project) = next {
        session.swap_project(project);
        Ok(true)
    } else {
        Err(CmdError::Disabled("No edit in history".into()))
    }
}
