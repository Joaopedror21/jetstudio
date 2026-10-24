//! Stable command identifiers shared by CLI, UI and future control clients.
mod arrangement;
mod automation;
mod channel;
mod edit;
mod file;
mod midi_file;
mod mixer;
mod note;
pub(crate) mod note_tools;
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
    spec!("file.new", "New", "FILE", Some("Ctrl+N"), "{}", None),
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
    spec!("file.autosave", "Autosave", "", None, "{}", None),
    spec!(
        "file.revert_autosave",
        "Revert to last autosave",
        "FILE",
        None,
        "{}",
        None
    ),
    spec!(
        "file.export_mid",
        "Export > MIDI file",
        "FILE",
        Some("Shift+Ctrl+M"),
        "{path:string,mode?:song|pattern}",
        None
    ),
    spec!(
        "file.export_wav",
        "Export > Wave file",
        "FILE",
        Some("Ctrl+R"),
        "{path:string,mode?:song|pattern,bits?:16|24|32,tail?:leave|cut|wrap,rate?:uint}",
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
        "transport.metronome.toggle",
        "Metronome",
        "OPTIONS",
        Some("Ctrl+M"),
        "{}",
        None
    ),
    spec!("transport.panic", "Stop all notes", "", None, "{}", None),
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
        "pattern.length.auto",
        "Pattern length: Auto",
        "PATTERNS",
        None,
        "{id:uint}",
        Step
    ),
    spec!(
        "channel.add",
        "Add channel",
        "ADD",
        None,
        "{name:string,kind?:sampler|synth}",
        Step
    ),
    spec!(
        "channel.remove",
        "Delete",
        "",
        Some("Alt+Del"),
        "{id:uint}",
        Step
    ),
    spec!(
        "channel.select",
        "Select channel",
        "",
        None,
        "{id:uint}",
        None
    ),
    spec!(
        "channel.rename",
        "Rename",
        "",
        Some("F2"),
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
    spec!("channel.solo", "Solo channel", "", None, "{id:uint}", Step),
    spec!(
        "channel.volume.set",
        "Channel volume",
        "",
        None,
        "{id:uint,value:uint}",
        Coalesce
    ),
    spec!(
        "channel.pan.set",
        "Channel pan",
        "",
        None,
        "{id:uint,value:int}",
        Coalesce
    ),
    spec!(
        "channel.pitch.set",
        "Channel pitch",
        "",
        None,
        "{id:uint,value:int}",
        Coalesce
    ),
    spec!(
        "channel.root_key.set",
        "Root key",
        "",
        None,
        "{id:uint,value:uint}",
        Step
    ),
    spec!(
        "channel.swing_mix.set",
        "Swing mix",
        "",
        None,
        "{id:uint,value:uint}",
        Step
    ),
    spec!(
        "channel.cut.set",
        "Cut groups",
        "",
        None,
        "{id:uint,cut:uint,cut_by:uint}",
        Step
    ),
    spec!(
        "channel.color.set",
        "Change color",
        "",
        None,
        "{id:uint,color:uint|null}",
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
        "channel.mixer_target.free",
        "Assign to free mixer track",
        "",
        Some("Ctrl+L"),
        "{id:uint}",
        Step
    ),
    spec!(
        "channel.clone",
        "Clone",
        "",
        Some("Alt+C"),
        "{id:uint}",
        Step
    ),
    spec!(
        "channel.move",
        "Move channel",
        "",
        Some("Alt+Up/Down"),
        "{id:uint,delta:int}",
        Step
    ),
    spec!(
        "channel.kind.set",
        "Replace",
        "",
        None,
        "{id:uint,kind:sampler|synth}",
        Step
    ),
    spec!(
        "channel.sampler.set",
        "Sampler settings",
        "",
        None,
        "{id:uint,settings:SamplerSettings}",
        Coalesce
    ),
    spec!(
        "channel.polyphony.set",
        "Polyphony",
        "",
        None,
        "{id:uint,max:uint,mono:bool,porta?:bool,slide_ms?:uint}",
        Step
    ),
    spec!(
        "channel.synth.set",
        "Synth parameters",
        "",
        None,
        "{id:uint,params:SynthParams}",
        Coalesce
    ),
    spec!(
        "channel.sample.load",
        "Load sample",
        "",
        None,
        "{id:uint,path:string}",
        Step
    ),
    spec!(
        "channel.sample.remove",
        "Remove sample",
        "",
        None,
        "{id:uint}",
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
    spec!(
        "step.props.set",
        "Graph editor",
        "",
        None,
        "{pattern:uint,channel:uint,index:uint,velocity?,pan?,release?,pitch?,mod_x?,mod_y?,shift?,note?}",
        Coalesce
    ),
    spec!(
        "step.fill",
        "Fill each N steps",
        "",
        None,
        "{pattern:uint,channel:uint,every:1|2|4|8}",
        Step
    ),
    spec!(
        "step.clear",
        "Clear steps",
        "",
        None,
        "{pattern:uint,channel:uint}",
        Step
    ),
    spec!(
        "note.export_mid",
        "Export as MIDI file",
        "",
        Some("Shift+Ctrl+M"),
        "{pattern:uint,channel:uint,path:string}",
        None
    ),
    spec!(
        "note.import_mid",
        "Import MIDI file",
        "",
        Some("Ctrl+M"),
        "{pattern:uint,channel:uint,path:string}",
        Step
    ),
    spec!(
        "note.copy",
        "Copy",
        "",
        Some("Ctrl+C"),
        "{pattern:uint,channel:uint,indices?:[uint]}",
        None
    ),
    spec!(
        "note.cut",
        "Cut",
        "",
        Some("Ctrl+X"),
        "{pattern:uint,channel:uint,indices?:[uint]}",
        Step
    ),
    spec!(
        "note.paste",
        "Paste",
        "",
        Some("Ctrl+V"),
        "{pattern:uint,channel:uint,at:int}",
        Step
    ),
    spec!(
        "note.add",
        "Add note",
        "",
        None,
        "{pattern:uint,channel:uint,start:int,length:int,key:uint,velocity?,pan?,slide?,porta?,color?}",
        Step
    ),
    spec!(
        "note.delete",
        "Delete notes",
        "",
        Some("Del"),
        "{pattern:uint,channel:uint,indices?:[uint]}",
        Step
    ),
    spec!(
        "note.move",
        "Move notes",
        "",
        None,
        "{pattern:uint,channel:uint,indices?:[uint],dt?:int,dkey?:int}",
        Coalesce
    ),
    spec!(
        "note.length.set",
        "Resize notes",
        "",
        None,
        "{pattern:uint,channel:uint,indices?:[uint],length?:int,delta?:int}",
        Coalesce
    ),
    spec!(
        "note.props.set",
        "Note properties",
        "",
        None,
        "{pattern:uint,channel:uint,indices?:[uint],velocity?,pan?,release?,fine_pitch?,mod_x?,mod_y?,muted?,slide?,porta?,color?}",
        Step
    ),
    spec!(
        "note.transpose",
        "Transpose",
        "",
        Some("Shift+Up/Down"),
        "{pattern:uint,channel:uint,indices?:[uint],semitones:int}",
        Step
    ),
    spec!(
        "note.quantize",
        "Quick quantize",
        "",
        Some("Ctrl+Q"),
        "{pattern:uint,channel:uint,indices?:[uint],grid:int,starts_only?:bool}",
        Step
    ),
    spec!(
        "note.duplicate",
        "Duplicate",
        "",
        Some("Ctrl+B"),
        "{pattern:uint,channel:uint,indices?:[uint],grid:int}",
        Step
    ),
    spec!(
        "note.chop",
        "Quick chop",
        "",
        Some("Ctrl+U"),
        "{pattern:uint,channel:uint,indices?:[uint],grid:int}",
        Step
    ),
    spec!(
        "note.glue",
        "Glue",
        "",
        Some("Ctrl+G"),
        "{pattern:uint,channel:uint,indices?:[uint]}",
        Step
    ),
    spec!(
        "note.discard_lengths",
        "Discard lengths",
        "",
        Some("Shift+D"),
        "{pattern:uint,channel:uint,indices?:[uint],grid:int}",
        Step
    ),
    spec!(
        "mixer.track.volume.set",
        "Fader",
        "",
        None,
        "{track:uint,value:int}",
        Coalesce
    ),
    spec!(
        "mixer.track.pan.set",
        "Mixer pan",
        "",
        None,
        "{track:uint,value:int}",
        Coalesce
    ),
    spec!(
        "mixer.track.stereo_sep.set",
        "Stereo separation",
        "",
        None,
        "{track:uint,value:int}",
        Coalesce
    ),
    spec!(
        "mixer.track.mute.set",
        "Mute track",
        "",
        None,
        "{track:uint,value:bool}",
        Step
    ),
    spec!(
        "mixer.track.solo",
        "Solo track",
        "",
        None,
        "{track:uint}",
        Step
    ),
    spec!(
        "mixer.track.invert.set",
        "Invert phase",
        "",
        None,
        "{track:uint,value:bool}",
        Step
    ),
    spec!(
        "mixer.track.swap.set",
        "Swap channels",
        "",
        None,
        "{track:uint,value:bool}",
        Step
    ),
    spec!(
        "mixer.track.to_master.set",
        "Send to Master",
        "",
        None,
        "{track:uint,value:bool}",
        Step
    ),
    spec!(
        "mixer.track.fx_enabled.set",
        "FX master switch",
        "",
        None,
        "{track:uint,value:bool}",
        Step
    ),
    spec!(
        "mixer.track.rename",
        "Rename track",
        "",
        Some("F2"),
        "{track:uint,name:string}",
        Step
    ),
    spec!(
        "mixer.send.set",
        "Send",
        "",
        None,
        "{from:uint,to:uint,level?:uint}",
        Coalesce
    ),
    spec!(
        "mixer.send.remove",
        "Remove send",
        "",
        None,
        "{from:uint,to:uint}",
        Step
    ),
    spec!(
        "mixer.fx.add",
        "Add effect",
        "",
        None,
        "{track:uint,kind:eq|delay|reverb|compressor|limiter|distortion|chorus|gate|flanger|phaser|filter}",
        Step
    ),
    spec!(
        "mixer.fx.remove",
        "Delete effect",
        "",
        None,
        "{track:uint,slot:uint}",
        Step
    ),
    spec!(
        "mixer.fx.move",
        "Move effect",
        "",
        None,
        "{track:uint,slot:uint,delta:int}",
        Step
    ),
    spec!(
        "mixer.fx.enabled.set",
        "Effect on/off",
        "",
        None,
        "{track:uint,slot:uint,value:bool}",
        Step
    ),
    spec!(
        "mixer.fx.mix.set",
        "Effect mix",
        "",
        None,
        "{track:uint,slot:uint,value:uint}",
        Coalesce
    ),
    spec!(
        "mixer.fx.set",
        "Effect parameters",
        "",
        None,
        "{track:uint,slot:uint,kind:FxKind}",
        Coalesce
    ),
    spec!(
        "project.swing.set",
        "Swing",
        "",
        None,
        "{value:uint}",
        Coalesce
    ),
    spec!("project.ppq.set", "PPQ", "", None, "{value:uint}", Step),
    spec!(
        "project.pan_law.set",
        "Panning law",
        "",
        None,
        "{value:circular|triangular}",
        Step
    ),
    spec!(
        "project.time_sig.set",
        "Time signature",
        "",
        None,
        "{num:uint,den:uint}",
        Step
    ),
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
        "{track?:uint,pattern:uint,start:int,length:int}",
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
    spec!(
        "playlist.audio.add",
        "Add Audio Clip",
        "",
        None,
        "{path:string,track?:uint,start:int}",
        Step
    ),
    spec!(
        "playlist.clip.mute.set",
        "Mute clip",
        "",
        None,
        "{id:uint,muted:bool}",
        Step
    ),
    spec!(
        "playlist.clip.audio.set",
        "Audio Clip gain, fades and reverse",
        "",
        None,
        "{id:uint,gain_cdb?:int,fade_in_ms?:uint,fade_out_ms?:uint,reverse?:bool}",
        Coalesce
    ),
    spec!(
        "playlist.track.solo",
        "Solo track",
        "",
        None,
        "{id:uint}",
        Step
    ),
    spec!(
        "note.flip",
        "Flip",
        "",
        Some("Alt+Y"),
        "{pattern:uint,channel:uint,indices?:[uint],horizontal?:bool,vertical?:bool}",
        Step
    ),
    spec!(
        "note.strum",
        "Strum",
        "",
        Some("Alt+S"),
        "{pattern:uint,channel:uint,indices?:[uint],time:int,velocity?:int,preserve_end?:bool}",
        Step
    ),
    spec!(
        "note.limit",
        "Limit",
        "",
        Some("Alt+K"),
        "{pattern:uint,channel:uint,indices?:[uint],low:uint,high:uint}",
        Step
    ),
    spec!(
        "note.stamp",
        "Chord stamp",
        "",
        None,
        "{pattern:uint,channel:uint,start:int,length:int,root:uint,chord:string}",
        Step
    ),
    spec!(
        "note.arpeggiate",
        "Arpeggiate",
        "",
        Some("Alt+A"),
        "{pattern:uint,channel:uint,indices?:[uint],step:int,direction:up|down|updown}",
        Step
    ),
    spec!(
        "note.snap_scale",
        "Snap to scale",
        "",
        None,
        "{pattern:uint,channel:uint,indices?:[uint],root:uint,scale:string}",
        Step
    ),
    spec!(
        "arrangement.add",
        "Add one",
        "",
        None,
        "{name?:string}",
        Step
    ),
    spec!("arrangement.clone", "Clone", "", None, "{}", Step),
    spec!(
        "arrangement.select",
        "Select arrangement",
        "",
        None,
        "{id:uint}",
        Step
    ),
    spec!(
        "arrangement.rename",
        "Rename arrangement",
        "",
        None,
        "{id:uint,name:string}",
        Step
    ),
    spec!(
        "arrangement.delete",
        "Delete arrangement",
        "",
        None,
        "{id:uint}",
        Step
    ),
    spec!(
        "marker.add",
        "Add time marker",
        "",
        Some("Alt+T"),
        "{at:int,name?:string}",
        Step
    ),
    spec!(
        "marker.delete",
        "Delete time marker",
        "",
        None,
        "{index:uint}",
        Step
    ),
    spec!(
        "marker.rename",
        "Rename time marker",
        "",
        None,
        "{index:uint,name:string}",
        Step
    ),
    spec!(
        "marker.action.set",
        "Time marker action",
        "",
        None,
        "{index:uint,action:None|Start|Loop}",
        Step
    ),
    spec!(
        "playlist.selection.set",
        "Timeline selection",
        "",
        None,
        "{start:int,end:int}",
        Step
    ),
    spec!(
        "playlist.selection.clear",
        "Clear timeline selection",
        "",
        None,
        "{}",
        Step
    ),
    spec!(
        "automation.create",
        "Create automation clip",
        "",
        None,
        "{target:AutoTarget}",
        Step
    ),
    spec!(
        "automation.point.add",
        "Add control point",
        "",
        None,
        "{id:uint,at:int,value:uint}",
        Step
    ),
    spec!(
        "automation.point.move",
        "Move control point",
        "",
        None,
        "{id:uint,index:uint,at:int,value:uint}",
        Coalesce
    ),
    spec!(
        "automation.point.delete",
        "Delete control point",
        "",
        None,
        "{id:uint,index:uint}",
        Step
    ),
    spec!(
        "automation.point.shape.set",
        "Curve type",
        "",
        None,
        "{id:uint,index:uint,shape:CurveShape,tension:int}",
        Coalesce
    ),
    spec!(
        "automation.delete",
        "Delete automation clip",
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

pub(super) fn invalid(id: &str, message: impl Into<String>) -> CmdError {
    CmdError::InvalidParams {
        command: id.to_owned(),
        message: message.into(),
    }
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
        Some("file") if id == "file.export_mid" => midi_file::run(session, id, params),
        Some("file") => file::run(session, id, params),
        Some("edit") => edit::run(session, id, params),
        Some("transport") => transport::run(session, id, params),
        Some("pattern") => pattern::run(session, id, params),
        Some("channel") => channel::run(session, id, params),
        Some("step") => step::run(session, id, params),
        Some("project") => project::run(session, id, params),
        Some("playlist") => playlist::run(session, id, params),
        Some("note")
            if matches!(
                id,
                "note.export_mid" | "note.import_mid" | "note.copy" | "note.cut" | "note.paste"
            ) =>
        {
            midi_file::run(session, id, params)
        }
        Some("note")
            if matches!(
                id,
                "note.flip"
                    | "note.strum"
                    | "note.limit"
                    | "note.stamp"
                    | "note.arpeggiate"
                    | "note.snap_scale"
            ) =>
        {
            note_tools::run(session, id, params)
        }
        Some("note") => note::run(session, id, params),
        Some("mixer") => mixer::run(session, id, params),
        Some("automation") => automation::run(session, id, params),
        Some("arrangement") | Some("marker") => arrangement::run(session, id, params),
        _ => Err(CmdError::UnknownCommand(id.to_owned())),
    }
}
