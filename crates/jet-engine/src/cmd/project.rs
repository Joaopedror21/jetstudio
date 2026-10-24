use super::{invalid, parse};
use crate::{CmdError, Session};
use jet_model::PanLaw;
use jet_time::{Ppq, TimeSig};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Unsigned {
    value: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Law {
    value: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Signature {
    num: u8,
    den: u8,
}

pub(super) fn run(session: &mut Session, id: &str, params: Value) -> Result<bool, CmdError> {
    match id {
        "project.swing.set" => {
            let p: Unsigned = parse(id, params)?;
            if p.value > 100 {
                return Err(invalid(id, "swing must be 0..=100"));
            }
            session.edit_project(|project| {
                project.settings.swing = p.value as u8;
                Ok(())
            })
        }
        "project.ppq.set" => {
            let p: Unsigned = parse(id, params)?;
            let ppq = Ppq::new(p.value).map_err(|e| invalid(id, e.to_string()))?;
            session.edit_project(|project| {
                project
                    .set_ppq(ppq)
                    .map_err(|e| CmdError::Failed(e.to_string()))
            })
        }
        // [Manual: songsettings_settings, "Panning law"].
        "project.pan_law.set" => {
            let p: Law = parse(id, params)?;
            let law = match p.value.as_str() {
                "circular" => PanLaw::Circular,
                "triangular" => PanLaw::Triangular,
                _ => return Err(invalid(id, "pan law must be circular or triangular")),
            };
            session.edit_project(|project| {
                project.settings.pan_law = law;
                Ok(())
            })
        }
        // "Set as time signature" [Manual: songsettings_settings, "Time Settings"].
        "project.time_sig.set" => {
            let p: Signature = parse(id, params)?;
            let sig = TimeSig::new(p.num, p.den).map_err(|e| invalid(id, e.to_string()))?;
            session.edit_project(|project| {
                project.settings.time_sig = sig;
                if project.settings.step_ticks().is_none() {
                    return Err(CmdError::Failed(
                        "the project PPQ cannot represent this time signature's steps".into(),
                    ));
                }
                Ok(())
            })
        }
        _ => Err(CmdError::UnknownCommand(id.to_owned())),
    }
}
