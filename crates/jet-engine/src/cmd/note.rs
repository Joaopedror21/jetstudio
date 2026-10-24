//! Piano roll editing [Manual: pianoroll, pianoroll_menu]. Notes are addressed by their
//! index in the channel's score sorted by (start, key); an empty `indices` list means
//! "all notes", as the manual specifies for most tools ("if nothing is selected, applies to all").

use super::{invalid, parse};
use crate::{CmdError, Session};
use jet_model::{ChannelId, MAX_KEY, Note, PatternId, Project};
use jet_time::Tick;
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AddParams {
    pattern: u64,
    channel: u64,
    start: i64,
    length: i64,
    key: u8,
    #[serde(default)]
    velocity: Option<u8>,
    #[serde(default)]
    pan: Option<i16>,
    #[serde(default)]
    slide: Option<bool>,
    #[serde(default)]
    porta: Option<bool>,
    #[serde(default)]
    color: Option<u8>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Selection {
    pattern: u64,
    channel: u64,
    #[serde(default)]
    indices: Vec<usize>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MoveParams {
    pattern: u64,
    channel: u64,
    #[serde(default)]
    indices: Vec<usize>,
    #[serde(default)]
    dt: i64,
    #[serde(default)]
    dkey: i16,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LengthParams {
    pattern: u64,
    channel: u64,
    #[serde(default)]
    indices: Vec<usize>,
    #[serde(default)]
    delta: Option<i64>,
    #[serde(default)]
    length: Option<i64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GridParams {
    pattern: u64,
    channel: u64,
    #[serde(default)]
    indices: Vec<usize>,
    grid: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct QuantizeParams {
    pattern: u64,
    channel: u64,
    #[serde(default)]
    indices: Vec<usize>,
    grid: i64,
    #[serde(default)]
    starts_only: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TransposeParams {
    pattern: u64,
    channel: u64,
    #[serde(default)]
    indices: Vec<usize>,
    semitones: i16,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PropsParams {
    pattern: u64,
    channel: u64,
    #[serde(default)]
    indices: Vec<usize>,
    #[serde(default)]
    velocity: Option<u8>,
    #[serde(default)]
    pan: Option<i16>,
    #[serde(default)]
    release: Option<u8>,
    #[serde(default)]
    fine_pitch: Option<i16>,
    #[serde(default)]
    mod_x: Option<u8>,
    #[serde(default)]
    mod_y: Option<u8>,
    #[serde(default)]
    muted: Option<bool>,
    #[serde(default)]
    slide: Option<bool>,
    #[serde(default)]
    porta: Option<bool>,
    #[serde(default)]
    color: Option<u8>,
}

fn sort(notes: &mut [Note]) {
    notes.sort_by_key(|note| (note.start, note.key, note.length));
}

fn score_mut(
    project: &mut Project,
    pattern: u64,
    channel: u64,
) -> Result<&mut Vec<Note>, CmdError> {
    if project.channel(ChannelId(channel)).is_none() {
        return Err(CmdError::Failed("channel does not exist".into()));
    }
    let pattern = project
        .patterns
        .iter_mut()
        .find(|p| p.id == PatternId(pattern))
        .ok_or_else(|| CmdError::Failed("pattern does not exist".into()))?;
    Ok(pattern.notes.entry(ChannelId(channel)).or_default())
}

/// Resolve a selection to indices; empty means all.
fn chosen(notes: &[Note], indices: &[usize]) -> Result<Vec<usize>, CmdError> {
    if indices.is_empty() {
        return Ok((0..notes.len()).collect());
    }
    let mut out: Vec<usize> = indices.to_vec();
    out.sort_unstable();
    out.dedup();
    if out.last().is_some_and(|&i| i >= notes.len()) {
        return Err(CmdError::Failed("note index out of range".into()));
    }
    Ok(out)
}

fn round_to(value: i64, grid: i64) -> i64 {
    (value + grid / 2).div_euclid(grid) * grid
}

/// Edit one channel's score, then keep it sorted; an emptied score is removed so the
/// channel returns to its step sequence.
fn edit_score(
    session: &mut Session,
    pattern: u64,
    channel: u64,
    edit: impl FnOnce(&mut Vec<Note>) -> Result<(), CmdError>,
) -> Result<bool, CmdError> {
    session.edit_project(|project| {
        let notes = score_mut(project, pattern, channel)?;
        edit(notes)?;
        if notes
            .iter()
            .any(|n| n.start.0 < 0 || n.length.0 < 0 || n.key > MAX_KEY)
        {
            return Err(CmdError::Failed("edit moves notes out of range".into()));
        }
        sort(notes);
        if notes.is_empty()
            && let Some(p) = project
                .patterns
                .iter_mut()
                .find(|p| p.id == PatternId(pattern))
        {
            p.notes.remove(&ChannelId(channel));
        }
        Ok(())
    })
}

pub(super) fn run(session: &mut Session, id: &str, params: Value) -> Result<bool, CmdError> {
    match id {
        "note.add" => {
            let p: AddParams = parse(id, params)?;
            if p.start < 0 || p.length < 0 || p.key > MAX_KEY {
                return Err(invalid(id, "start/length must be >= 0 and key 0..=131"));
            }
            edit_score(session, p.pattern, p.channel, |notes| {
                let mut note = Note::new(Tick(p.start), Tick(p.length), p.key);
                if let Some(v) = p.velocity {
                    note.velocity = v;
                }
                if let Some(v) = p.pan {
                    note.pan = v;
                }
                note.slide = p.slide.unwrap_or(false);
                note.porta = p.porta.unwrap_or(false);
                if let Some(v) = p.color {
                    note.color = v;
                }
                notes.push(note);
                Ok(())
            })
        }
        // "Delete (Del) - Deletes selected Notes ... If no selection is made - deletes all".
        "note.delete" => {
            let p: Selection = parse(id, params)?;
            edit_score(session, p.pattern, p.channel, |notes| {
                let picked = chosen(notes, &p.indices)?;
                let mut index = 0;
                notes.retain(|_| {
                    let keep = picked.binary_search(&index).is_err();
                    index += 1;
                    keep
                });
                Ok(())
            })
        }
        "note.move" => {
            let p: MoveParams = parse(id, params)?;
            edit_score(session, p.pattern, p.channel, |notes| {
                for i in chosen(notes, &p.indices)? {
                    let note = &mut notes[i];
                    note.start = Tick(note.start.0 + p.dt);
                    let key = i16::from(note.key) + p.dkey;
                    if !(0..=i16::from(MAX_KEY)).contains(&key) {
                        return Err(CmdError::Failed("edit moves notes out of range".into()));
                    }
                    note.key = key as u8;
                }
                Ok(())
            })
        }
        "note.length.set" => {
            let p: LengthParams = parse(id, params)?;
            edit_score(session, p.pattern, p.channel, |notes| {
                for i in chosen(notes, &p.indices)? {
                    let note = &mut notes[i];
                    note.length = match (p.length, p.delta) {
                        (Some(length), None) => Tick(length),
                        (None, Some(delta)) => Tick((note.length.0 + delta).max(0)),
                        _ => {
                            return Err(CmdError::Failed(
                                "give exactly one of length or delta".into(),
                            ));
                        }
                    };
                }
                Ok(())
            })
        }
        "note.props.set" => {
            let p: PropsParams = parse(id, params)?;
            edit_score(session, p.pattern, p.channel, |notes| {
                for i in chosen(notes, &p.indices)? {
                    let note = &mut notes[i];
                    if let Some(v) = p.velocity {
                        note.velocity = v;
                    }
                    if let Some(v) = p.pan {
                        note.pan = v;
                    }
                    if let Some(v) = p.release {
                        note.release = v;
                    }
                    if let Some(v) = p.fine_pitch {
                        note.fine_pitch = v;
                    }
                    if let Some(v) = p.mod_x {
                        note.mod_x = v;
                    }
                    if let Some(v) = p.mod_y {
                        note.mod_y = v;
                    }
                    if let Some(v) = p.muted {
                        note.muted = v;
                    }
                    if let Some(v) = p.slide {
                        note.slide = v;
                    }
                    if let Some(v) = p.porta {
                        note.porta = v;
                    }
                    if let Some(v) = p.color {
                        note.color = v;
                    }
                }
                Ok(())
            })
        }
        // Shift+Up/Down (semitone), Ctrl+Up/Down (octave) [Manual: pianoroll_menu].
        "note.transpose" => {
            let p: TransposeParams = parse(id, params)?;
            edit_score(session, p.pattern, p.channel, |notes| {
                for i in chosen(notes, &p.indices)? {
                    let key = i16::from(notes[i].key) + p.semitones;
                    if !(0..=i16::from(MAX_KEY)).contains(&key) {
                        return Err(CmdError::Failed("edit moves notes out of range".into()));
                    }
                    notes[i].key = key as u8;
                }
                Ok(())
            })
        }
        // Quick quantize (start and length) / Shift+Q (start times only) to the snap grid.
        "note.quantize" => {
            let p: QuantizeParams = parse(id, params)?;
            if p.grid <= 0 {
                return Err(invalid(id, "grid must be > 0 ticks"));
            }
            edit_score(session, p.pattern, p.channel, |notes| {
                for i in chosen(notes, &p.indices)? {
                    let note = &mut notes[i];
                    let start = round_to(note.start.0, p.grid).max(0);
                    if !p.starts_only && note.length.0 > 0 {
                        let end = round_to(note.end().0, p.grid).max(start + p.grid);
                        note.length = Tick(end - start);
                    }
                    note.start = Tick(start);
                }
                Ok(())
            })
        }
        // Ctrl+B: duplicate the selection (or everything) right after itself; the repeat
        // interval is the selection span rounded up to the grid (Hypothesis H-PR-1).
        "note.duplicate" => {
            let p: GridParams = parse(id, params)?;
            if p.grid <= 0 {
                return Err(invalid(id, "grid must be > 0 ticks"));
            }
            edit_score(session, p.pattern, p.channel, |notes| {
                let picked = chosen(notes, &p.indices)?;
                let Some(first) = picked.iter().map(|&i| notes[i].start.0).min() else {
                    return Ok(());
                };
                let last = picked
                    .iter()
                    .map(|&i| notes[i].start.0 + notes[i].length.0.max(1))
                    .max()
                    .unwrap_or(first + 1);
                let span = (last - first + p.grid - 1) / p.grid * p.grid;
                let copies: Vec<Note> = picked
                    .iter()
                    .map(|&i| {
                        let mut note = notes[i].clone();
                        note.start = Tick(note.start.0 + span);
                        note
                    })
                    .collect();
                notes.extend(copies);
                Ok(())
            })
        }
        // Ctrl+U "Slices long Notes into many shorter, based on the Snap settings".
        "note.chop" => {
            let p: GridParams = parse(id, params)?;
            if p.grid <= 0 {
                return Err(invalid(id, "grid must be > 0 ticks"));
            }
            edit_score(session, p.pattern, p.channel, |notes| {
                let picked = chosen(notes, &p.indices)?;
                let mut pieces = Vec::new();
                for &i in picked.iter().rev() {
                    let note = notes.remove(i);
                    let mut start = note.start.0;
                    let end = note.end().0;
                    if note.length.0 <= p.grid {
                        pieces.push(note);
                        continue;
                    }
                    while start < end {
                        let mut piece = note.clone();
                        piece.start = Tick(start);
                        piece.length = Tick(p.grid.min(end - start));
                        pieces.push(piece);
                        start += p.grid;
                    }
                }
                notes.extend(pieces);
                Ok(())
            })
        }
        // Ctrl+G "Glue contiguous (touching) Notes together" (same key).
        "note.glue" => {
            let p: Selection = parse(id, params)?;
            edit_score(session, p.pattern, p.channel, |notes| {
                let picked = chosen(notes, &p.indices)?;
                let mut selected: Vec<Note> = picked.iter().map(|&i| notes[i].clone()).collect();
                for &i in picked.iter().rev() {
                    notes.remove(i);
                }
                selected.sort_by_key(|n| (n.key, n.start));
                let mut merged: Vec<Note> = Vec::new();
                for note in selected {
                    if let Some(last) = merged.last_mut()
                        && last.key == note.key
                        && last.end() == note.start
                    {
                        last.length = Tick(last.length.0 + note.length.0);
                        continue;
                    }
                    merged.push(note);
                }
                notes.extend(merged);
                Ok(())
            })
        }
        // Shift+D "Reduces selected Note lengths to the minimum size allowed by the Snap".
        "note.discard_lengths" => {
            let p: GridParams = parse(id, params)?;
            if p.grid < 0 {
                return Err(invalid(id, "grid must be >= 0 ticks"));
            }
            edit_score(session, p.pattern, p.channel, |notes| {
                for i in chosen(notes, &p.indices)? {
                    notes[i].length = Tick(p.grid);
                }
                Ok(())
            })
        }
        _ => Err(CmdError::UnknownCommand(id.to_owned())),
    }
}
