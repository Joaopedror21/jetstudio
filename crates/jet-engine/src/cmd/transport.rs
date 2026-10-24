use super::{empty, parse};
use crate::{CmdError, Session};
use jet_time::{Tempo, Tick};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TempoParams {
    milli_bpm: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PositionParams {
    tick: i64,
}

pub(super) fn run(session: &mut Session, id: &str, params: Value) -> Result<bool, CmdError> {
    match id {
        "transport.play_stop" => {
            empty(id, params)?;
            session
                .transport
                .play_stop(session.project.settings.ppq, session.project.settings.tempo);
            Ok(true)
        }
        "transport.stop" => {
            empty(id, params)?;
            session.transport.stop();
            Ok(true)
        }
        "transport.pause" => {
            empty(id, params)?;
            session
                .transport
                .pause(session.project.settings.ppq, session.project.settings.tempo);
            Ok(true)
        }
        "transport.metronome.toggle" => {
            empty(id, params)?;
            session.metronome = !session.metronome;
            Ok(true)
        }
        "transport.panic" => {
            empty(id, params)?;
            session.panic_requested = true;
            Ok(true)
        }
        "transport.mode.toggle" => {
            empty(id, params)?;
            session.transport.toggle_mode();
            Ok(true)
        }
        "transport.tempo.set" => {
            let p: TempoParams = parse(id, params)?;
            let tempo = Tempo::new(p.milli_bpm).map_err(|e| CmdError::Failed(e.to_string()))?;
            let position = session.transport_snapshot().position;
            let changed = session.edit_project(|project| {
                project.settings.tempo = tempo;
                Ok(())
            })?;
            session.transport.seek(position);
            Ok(changed)
        }
        "transport.position.set" => {
            let p: PositionParams = parse(id, params)?;
            session.transport.seek(Tick(p.tick.max(0)));
            Ok(true)
        }
        _ => Err(CmdError::UnknownCommand(id.to_owned())),
    }
}
