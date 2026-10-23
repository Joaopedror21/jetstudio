use super::{empty, parse};
use crate::{CmdError, History, Session};
use serde::Deserialize;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PathParams {
    path: PathBuf,
}

pub(super) fn run(session: &mut Session, id: &str, params: Value) -> Result<bool, CmdError> {
    match id {
        "file.new" => {
            empty(id, params)?;
            session.project = Arc::new(jet_model::Project::new());
            session.history = History::default();
            session.path = None;
            session.selected_pattern = session
                .project
                .patterns
                .first()
                .map(|p| p.id)
                .unwrap_or(jet_model::PatternId(1));
            session.selected_channel = None;
            session.revision = session.revision.saturating_add(1);
            session
                .publisher
                .on_change(session.revision, Arc::clone(&session.project));
            Ok(true)
        }
        "file.open" => {
            let p: PathParams = parse(id, params)?;
            let project = jet_format::load(&p.path).map_err(|e| CmdError::Failed(e.to_string()))?;
            session.selected_pattern = project
                .patterns
                .first()
                .map(|p| p.id)
                .unwrap_or(jet_model::PatternId(1));
            session.selected_channel = None;
            session.project = Arc::new(project);
            session.history = History::default();
            session.path = Some(p.path);
            session.revision = session.revision.saturating_add(1);
            session
                .publisher
                .on_change(session.revision, Arc::clone(&session.project));
            Ok(true)
        }
        "file.save" => {
            empty(id, params)?;
            let path = session
                .path
                .as_ref()
                .ok_or_else(|| CmdError::Disabled("Save As a project first".into()))?;
            jet_format::save_atomic(path, &session.project)
                .map_err(|e| CmdError::Failed(e.to_string()))?;
            Ok(false)
        }
        "file.save_as" => {
            let p: PathParams = parse(id, params)?;
            jet_format::save_atomic(&p.path, &session.project)
                .map_err(|e| CmdError::Failed(e.to_string()))?;
            session.path = Some(p.path);
            Ok(false)
        }
        _ => Err(CmdError::UnknownCommand(id.to_owned())),
    }
}
