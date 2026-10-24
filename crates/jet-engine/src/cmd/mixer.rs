//! Mixer commands [Manual: mixer]. Track 0 is the Master; 1..=500 are inserts.

use super::{invalid, parse};
use crate::{CmdError, Session};
use jet_model::{FxKind, FxSlot, MAX_FX_SLOTS, MAX_INSERTS, MixerInsert, MixerSend, Project};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TrackValue {
    track: u16,
    value: i32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TrackFlag {
    track: u16,
    value: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TrackEqParams {
    track: u16,
    band: usize,
    #[serde(default)]
    freq_hz: Option<u16>,
    #[serde(default)]
    gain_db_tenths: Option<i16>,
    #[serde(default)]
    q_milli: Option<u16>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TrackOnly {
    track: u16,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TrackName {
    track: u16,
    name: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SendParams {
    from: u16,
    to: u16,
    #[serde(default)]
    level: Option<u16>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FxAdd {
    track: u16,
    kind: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FxSlotParams {
    track: u16,
    slot: usize,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FxMove {
    track: u16,
    slot: usize,
    delta: i32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FxFlag {
    track: u16,
    slot: usize,
    value: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FxMix {
    track: u16,
    slot: usize,
    value: u16,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FxSet {
    track: u16,
    slot: usize,
    kind: FxKind,
}

fn track_mut(project: &mut Project, index: u16) -> Result<&mut MixerInsert, CmdError> {
    project
        .mixer
        .track_mut(index)
        .ok_or_else(|| CmdError::Failed("mixer track must be 0..=500".into()))
}

fn slot_mut(project: &mut Project, track: u16, slot: usize) -> Result<&mut FxSlot, CmdError> {
    track_mut(project, track)?
        .fx
        .get_mut(slot)
        .ok_or_else(|| CmdError::Failed("FX slot is empty".into()))
}

/// Keep FX-mix automation pointing at the same effect after slots change.
fn remap_fx_automation(project: &mut Project, track: u16, map: impl Fn(usize) -> Option<usize>) {
    use jet_model::{AutoTarget, ClipSource};
    let mut removed = Vec::new();
    for clip in &mut project.automations {
        if let AutoTarget::FxMix { track: t, slot } = clip.target
            && t == track
        {
            match map(usize::from(slot)) {
                Some(next) => {
                    clip.target = AutoTarget::FxMix {
                        track,
                        slot: next as u8,
                    }
                }
                None => removed.push(clip.id),
            }
        }
    }
    project.automations.retain(|a| !removed.contains(&a.id));
    for arrangement in &mut project.arrangements {
        arrangement
            .clips
            .retain(|c| !matches!(c.source, ClipSource::Automation(a) if removed.contains(&a)));
    }
}

fn ranged(id: &str, value: i32, low: i32, high: i32) -> Result<i32, CmdError> {
    if (low..=high).contains(&value) {
        Ok(value)
    } else {
        Err(invalid(id, format!("value must be {low}..={high}")))
    }
}

pub(super) fn run(session: &mut Session, id: &str, params: Value) -> Result<bool, CmdError> {
    match id {
        "mixer.track.volume.set" => {
            let p: TrackValue = parse(id, params)?;
            let v = ranged(id, p.value, 0, 1000)? as u16;
            session.edit_project(|project| {
                track_mut(project, p.track)?.volume = v;
                Ok(())
            })
        }
        "mixer.track.pan.set" => {
            let p: TrackValue = parse(id, params)?;
            let v = ranged(id, p.value, -1000, 1000)? as i16;
            session.edit_project(|project| {
                track_mut(project, p.track)?.pan = v;
                Ok(())
            })
        }
        "mixer.track.stereo_sep.set" => {
            let p: TrackValue = parse(id, params)?;
            let v = ranged(id, p.value, -1000, 1000)? as i16;
            session.edit_project(|project| {
                track_mut(project, p.track)?.stereo_sep = v;
                Ok(())
            })
        }
        // Integrated EQ band [Manual: mixer_trackprops, "Parametric EQ"]: frequency 20 Hz-20 kHz,
        // gain +-18 dB, bandwidth as Q 0.1-10 (ranges ours, H-MIX-3).
        "mixer.track.eq.set" => {
            let p: TrackEqParams = parse(id, params)?;
            if p.band > 2 {
                return Err(invalid(id, "band must be 0, 1 or 2"));
            }
            session.edit_project(|project| {
                let band = &mut track_mut(project, p.track)?.eq[p.band];
                if let Some(v) = p.freq_hz {
                    band.freq_hz = v.clamp(20, 20_000);
                }
                if let Some(v) = p.gain_db_tenths {
                    band.gain_db_tenths = v.clamp(-180, 180);
                }
                if let Some(v) = p.q_milli {
                    band.q_milli = v.clamp(100, 10_000);
                }
                Ok(())
            })
        }
        "mixer.track.mute.set" => {
            let p: TrackFlag = parse(id, params)?;
            session.edit_project(|project| {
                track_mut(project, p.track)?.mute = p.value;
                Ok(())
            })
        }
        "mixer.track.invert.set" => {
            let p: TrackFlag = parse(id, params)?;
            session.edit_project(|project| {
                track_mut(project, p.track)?.invert_phase = p.value;
                Ok(())
            })
        }
        "mixer.track.swap.set" => {
            let p: TrackFlag = parse(id, params)?;
            session.edit_project(|project| {
                track_mut(project, p.track)?.swap_lr = p.value;
                Ok(())
            })
        }
        "mixer.track.to_master.set" => {
            let p: TrackFlag = parse(id, params)?;
            if p.track == 0 {
                return Err(invalid(id, "the Master cannot send to itself"));
            }
            session.edit_project(|project| {
                track_mut(project, p.track)?.to_master = p.value;
                Ok(())
            })
        }
        "mixer.track.fx_enabled.set" => {
            let p: TrackFlag = parse(id, params)?;
            session.edit_project(|project| {
                track_mut(project, p.track)?.fx_enabled = p.value;
                Ok(())
            })
        }
        "mixer.track.rename" => {
            let p: TrackName = parse(id, params)?;
            if p.name.trim().is_empty() {
                return Err(invalid(id, "name cannot be empty"));
            }
            session.edit_project(|project| {
                track_mut(project, p.track)?.name = p.name;
                Ok(())
            })
        }
        // Solo mutes all other inserts; repeating it unmutes all [Manual: mixer, "Mute Switch"].
        "mixer.track.solo" => {
            let p: TrackOnly = parse(id, params)?;
            if p.track == 0 || usize::from(p.track) > MAX_INSERTS {
                return Err(invalid(id, "solo applies to inserts 1..=500"));
            }
            session.edit_project(|project| {
                // Materialize every insert that can carry audio so all of them get muted.
                let highest = project
                    .channels
                    .iter()
                    .map(|c| usize::from(c.mixer_target))
                    .chain(
                        project
                            .mixer
                            .inserts
                            .iter()
                            .flat_map(|t| t.sends.iter().map(|s| usize::from(s.to))),
                    )
                    .chain([project.mixer.inserts.len(), usize::from(p.track)])
                    .max()
                    .unwrap_or(0)
                    .min(MAX_INSERTS);
                track_mut(project, highest as u16)?;
                let soloed = project
                    .mixer
                    .inserts
                    .iter()
                    .enumerate()
                    .all(|(i, t)| t.mute == (i + 1 != usize::from(p.track)));
                for (i, insert) in project.mixer.inserts.iter_mut().enumerate() {
                    insert.mute = !soloed && i + 1 != usize::from(p.track);
                }
                Ok(())
            })
        }
        // Post-fader send; level 0 keeps a sidechain-only link [Manual: mixer, "Sidechain Sends"].
        "mixer.send.set" => {
            let p: SendParams = parse(id, params)?;
            let level = p.level.unwrap_or(jet_model::MIXER_UNITY);
            if level > 1000 || p.from == 0 || p.to == 0 || p.from == p.to {
                return Err(invalid(
                    id,
                    "sends connect two different inserts; level 0..=1000",
                ));
            }
            session.edit_project(|project| {
                if project.mixer.would_cycle(p.from, p.to) {
                    return Err(CmdError::Failed(
                        "this send would create a feedback loop".into(),
                    ));
                }
                track_mut(project, p.to)?;
                let from = track_mut(project, p.from)?;
                match from.sends.iter_mut().find(|send| send.to == p.to) {
                    Some(send) => send.level = level,
                    None => from.sends.push(MixerSend { to: p.to, level }),
                }
                Ok(())
            })
        }
        "mixer.send.remove" => {
            let p: SendParams = parse(id, params)?;
            session.edit_project(|project| {
                let from = track_mut(project, p.from)?;
                let before = from.sends.len();
                from.sends.retain(|send| send.to != p.to);
                if from.sends.len() == before {
                    return Err(CmdError::Failed("send does not exist".into()));
                }
                Ok(())
            })
        }
        "mixer.fx.add" => {
            let p: FxAdd = parse(id, params)?;
            let kind = FxKind::default_for(&p.kind)
                .ok_or_else(|| invalid(id, format!("unknown effect {:?}", p.kind)))?;
            session.edit_project(|project| {
                let track = track_mut(project, p.track)?;
                if track.fx.len() >= MAX_FX_SLOTS {
                    return Err(CmdError::Disabled("All 10 FX slots are in use".into()));
                }
                track.fx.push(FxSlot {
                    kind,
                    enabled: true,
                    mix: 1000,
                });
                Ok(())
            })
        }
        "mixer.fx.remove" => {
            let p: FxSlotParams = parse(id, params)?;
            session.edit_project(|project| {
                let track = track_mut(project, p.track)?;
                if p.slot >= track.fx.len() {
                    return Err(CmdError::Failed("FX slot is empty".into()));
                }
                track.fx.remove(p.slot);
                // Automation follows its slot: clips on the removed slot go, later slots shift.
                remap_fx_automation(project, p.track, |slot| match slot {
                    s if s == p.slot => None,
                    s if s > p.slot => Some(s - 1),
                    s => Some(s),
                });
                Ok(())
            })
        }
        "mixer.fx.move" => {
            let p: FxMove = parse(id, params)?;
            session.edit_project(|project| {
                let track = track_mut(project, p.track)?;
                if p.slot >= track.fx.len() {
                    return Err(CmdError::Failed("FX slot is empty".into()));
                }
                let target =
                    (p.slot as i64 + i64::from(p.delta)).clamp(0, track.fx.len() as i64 - 1);
                let slot = track.fx.remove(p.slot);
                track.fx.insert(target as usize, slot);
                let (from, to) = (p.slot, target as usize);
                remap_fx_automation(project, p.track, |slot| {
                    Some(if slot == from {
                        to
                    } else if from < to && slot > from && slot <= to {
                        slot - 1
                    } else if to < from && slot >= to && slot < from {
                        slot + 1
                    } else {
                        slot
                    })
                });
                Ok(())
            })
        }
        "mixer.fx.enabled.set" => {
            let p: FxFlag = parse(id, params)?;
            session.edit_project(|project| {
                slot_mut(project, p.track, p.slot)?.enabled = p.value;
                Ok(())
            })
        }
        "mixer.fx.mix.set" => {
            let p: FxMix = parse(id, params)?;
            if p.value > 1000 {
                return Err(invalid(id, "mix must be 0..=1000"));
            }
            session.edit_project(|project| {
                slot_mut(project, p.track, p.slot)?.mix = p.value;
                Ok(())
            })
        }
        "mixer.fx.set" => {
            let p: FxSet = parse(id, params)?;
            session.edit_project(|project| {
                let slot = slot_mut(project, p.track, p.slot)?;
                if slot.kind.name() != p.kind.name() {
                    return Err(CmdError::Failed(
                        "use mixer.fx.remove and mixer.fx.add to change the effect type".into(),
                    ));
                }
                slot.kind = p.kind;
                Ok(())
            })
        }
        _ => Err(CmdError::UnknownCommand(id.to_owned())),
    }
}
