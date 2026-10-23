//! Stable command identifiers shared by CLI, UI and future control clients.
mod channel;
mod edit;
mod file;
mod pattern;
mod playlist;
mod project;
mod step;
mod transport;

use crate::{CmdError, Session};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize)]
pub struct DisabledReason(pub String);

#[derive(Debug, Clone, Copy, Serialize)]
pub enum UndoPolicy {
    None,
    Step,
    Coalesce,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct CommandSpec {
    pub id: &'static str,
    pub label: &'static str,
    pub menu: &'static str,
    pub shortcut: Option<&'static str>,
    pub params: &'static str,
    pub undo: UndoPolicy,
}

impl CommandSpec {
    pub fn enabled(&self, session: &Session) -> Result<(), DisabledReason> {
        match self.id {
            "file.save" if session.path.is_none() => {
                Err(DisabledReason("Save As a project first".into()))
            }
            "edit.undo_toggle" | "edit.undo_step"
                if !session.history.can_undo() && !session.history.can_redo() =>
            {
                Err(DisabledReason("No edit in history".into()))
            }
            "edit.redo_step" if !session.history.can_redo() => {
                Err(DisabledReason("No edit to redo".into()))
            }
            "pattern.delete" if session.project.patterns.len() <= 1 => {
                Err(DisabledReason("The last pattern cannot be deleted".into()))
            }
            _ => Ok(()),
        }
    }
}

pub fn command_specs() -> &'static [CommandSpec] {
    SPECS
}

macro_rules! spec {
    ($id:literal, $label:literal, $menu:literal, $key:expr, $params:literal, $undo:ident) => {
        CommandSpec {
            id: $id,
            label: $label,
            menu: $menu,
            shortcut: $key,
            params: $params,
            undo: UndoPolicy::$undo,
        }
    };
}

static SPECS: &[CommandSpec] = &[
    spec!("file.new", "New", "FILE", None, "{}", None),
    spec!(
        "file.open",
        "Open",
        "FILE",
        Some("Ctrl+O"),
        "{path:string}",
        None
    ),
    spec!("file.save", "Save", "FILE", Some("Ctrl+S"), "{}", None),
    spec!(
        "file.save_as",
        "Save As",
        "FILE",
        Some("Ctrl+Shift+S"),
        "{path:string}",
        None
    ),
    spec!(
        "edit.undo_toggle",
        "Undo/Redo last edit",
        "EDIT",
        Some("Ctrl+Z"),
        "{}",
        None
    ),
    spec!(
        "edit.undo_step",
        "Undo step-by-step",
        "EDIT",
        Some("Ctrl+Alt+Z"),
        "{}",
        None
    ),
    spec!("edit.redo_step", "Redo step", "EDIT", None, "{}", None),
    spec!(
        "transport.play_stop",
        "Play/Stop",
        "",
        Some("Space"),
        "{}",
        None
    ),
    spec!("transport.stop", "Stop", "", None, "{}", None),
    spec!(
        "transport.pause",
        "Play/Pause",
        "",
        Some("Ctrl+Space"),
        "{}",
        None
    ),
    spec!(
        "transport.mode.toggle",
        "PAT/SONG",
        "",
        Some("L"),
        "{}",
        None
    ),
    spec!(
        "transport.tempo.set",
        "Tempo",
        "",
        None,
        "{milli_bpm:uint}",
        Step
    ),
    spec!(
        "transport.position.set",
        "Position",
        "",
        None,
        "{tick:int}",
        None
    ),
    spec!(
        "pattern.select",
        "Select pattern",
        "PATTERNS",
        None,
        "{id:uint}",
        None
    ),
    spec!(
        "pattern.next",
        "Next pattern",
        "PATTERNS",
        Some("+"),
        "{}",
        None
    ),
    spec!(
        "pattern.prev",
        "Previous pattern",
        "PATTERNS",
        Some("-"),
        "{}",
        None
    ),
    spec!(
        "pattern.next_empty",
        "Next empty pattern",
        "PATTERNS",
        Some("F4"),
        "{}",
        Step
    ),
    spec!(
        "pattern.clone",
        "Clone pattern",
        "PATTERNS",
        Some("Alt+C"),
        "{id:uint}",
        Step
    ),
    spec!(
        "pattern.rename",
        "Rename pattern",
        "PATTERNS",
        None,
        "{id:uint,name:string}",
        Step
    ),
    spec!(
        "pattern.delete",
        "Delete pattern",
        "PATTERNS",
        None,
        "{id:uint}",
        Step
    ),
    spec!(
        "pattern.length.set",
        "Pattern length",
        "PATTERNS",
        None,
        "{id:uint,steps:uint}",
        Step
    ),
    spec!(
        "channel.add",
        "Add channel",
        "ADD",
        None,
        "{name:string}",
        Step
    ),
    spec!(
        "channel.remove",
        "Remove channel",
        "",
        None,
        "{id:uint}",
        Step
    ),
    spec!(
        "channel.rename",
        "Rename channel",
        "",
        None,
        "{id:uint,name:string}",
        Step
    ),
    spec!(
        "channel.mute.set",
        "Mute channel",
        "",
        None,
        "{id:uint,mute:bool}",
        Step
    ),
    spec!(
        "channel.volume.set",
        "Channel volume",
        "",
        None,
        "{id:uint,value:uint}",
        Step
    ),
    spec!(
        "channel.pan.set",
        "Channel pan",
        "",
        None,
        "{id:uint,value:int}",
        Step
    ),
    spec!(
        "channel.mixer_target.set",
        "Mixer target",
        "",
        None,
        "{id:uint,value:uint}",
        Step
    ),
    spec!(
        "step.set",
        "Set step",
        "",
        None,
        "{pattern:uint,channel:uint,index:uint,on:bool}",
        Step
    ),
    spec!(
        "step.paint",
        "Paint steps",
        "",
        None,
        "{pattern:uint,channel:uint,from:uint,to:uint,on:bool}",
        Coalesce
    ),
    spec!("project.swing.set", "Swing", "", None, "{value:uint}", Step),
    spec!("project.ppq.set", "PPQ", "", None, "{value:uint}", Step),
    spec!(
        "playlist.track.add",
        "Add track",
        "",
        None,
        "{name:string}",
        Step
    ),
    spec!(
        "playlist.track.rename",
        "Rename track",
        "",
        None,
        "{id:uint,name:string}",
        Step
    ),
    spec!(
        "playlist.track.mute.set",
        "Mute track",
        "",
        None,
        "{id:uint,mute:bool}",
        Step
    ),
    spec!(
        "playlist.clip.add",
        "Add Pattern Clip",
        "",
        None,
        "{track:uint,pattern:uint,start:int,length:int}",
        Step
    ),
    spec!(
        "playlist.clip.move",
        "Move clip",
        "",
        None,
        "{id:uint,track:uint,start:int}",
        Step
    ),
    spec!(
        "playlist.clip.resize",
        "Resize clip",
        "",
        None,
        "{id:uint,length:int}",
        Step
    ),
    spec!(
        "playlist.clip.delete",
        "Delete clip",
        "",
        None,
        "{id:uint}",
        Step
    ),
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}

pub(super) fn parse<T: DeserializeOwned>(id: &str, params: Value) -> Result<T, CmdError> {
    serde_json::from_value(params).map_err(|e| CmdError::InvalidParams {
        command: id.to_owned(),
        message: e.to_string(),
    })
}

pub(super) fn empty(id: &str, params: Value) -> Result<(), CmdError> {
    let _: Empty = parse(id, params)?;
    Ok(())
}

pub(super) fn dispatch(session: &mut Session, id: &str, params: Value) -> Result<bool, CmdError> {
    let spec = SPECS
        .iter()
        .find(|spec| spec.id == id)
        .ok_or_else(|| CmdError::UnknownCommand(id.to_owned()))?;
    spec.enabled(session)
        .map_err(|reason| CmdError::Disabled(reason.0))?;
    match id.split('.').next() {
        Some("file") => file::run(session, id, params),
        Some("edit") => edit::run(session, id, params),
        Some("transport") => transport::run(session, id, params),
        Some("pattern") => pattern::run(session, id, params),
        Some("channel") => channel::run(session, id, params),
        Some("step") => step::run(session, id, params),
        Some("project") => project::run(session, id, params),
        Some("playlist") => playlist::run(session, id, params),
        _ => Err(CmdError::UnknownCommand(id.to_owned())),
    }
}
