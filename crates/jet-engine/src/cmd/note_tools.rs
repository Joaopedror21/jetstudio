//! Piano roll tools [Manual: pianoroll_menu "Tools", pianoroll_strum, pianoroll_flip,
//! pianoroll_limit; pianoroll "Chord Stamp", "Snap to scale"]. Each tool works on the selection,
//! or on the whole score when nothing is selected.

use super::{invalid, parse};
use crate::{CmdError, Session};
use jet_model::{ChannelId, MAX_KEY, Note, PatternId};
use jet_time::Tick;
use serde::Deserialize;
use serde_json::Value;

/// Scale interval sets (standard music theory; the manual lists scales by name only).
pub const SCALES: [(&str, &[u8]); 12] = [
    ("major", &[0, 2, 4, 5, 7, 9, 11]),
    ("minor", &[0, 2, 3, 5, 7, 8, 10]),
    ("harmonic-minor", &[0, 2, 3, 5, 7, 8, 11]),
    ("dorian", &[0, 2, 3, 5, 7, 9, 10]),
    ("phrygian", &[0, 1, 3, 5, 7, 8, 10]),
    ("lydian", &[0, 2, 4, 6, 7, 9, 11]),
    ("mixolydian", &[0, 2, 4, 5, 7, 9, 10]),
    ("locrian", &[0, 1, 3, 5, 6, 8, 10]),
    ("major-pentatonic", &[0, 2, 4, 7, 9]),
    ("minor-pentatonic", &[0, 3, 5, 7, 10]),
    ("blues", &[0, 3, 5, 6, 7, 10]),
    ("chromatic", &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]),
];

/// Chord shapes for the Chord Stamp (intervals above the clicked root).
pub const CHORDS: [(&str, &[u8]); 10] = [
    ("major", &[0, 4, 7]),
    ("minor", &[0, 3, 7]),
    ("diminished", &[0, 3, 6]),
    ("augmented", &[0, 4, 8]),
    ("sus2", &[0, 2, 7]),
    ("sus4", &[0, 5, 7]),
    ("major7", &[0, 4, 7, 11]),
    ("minor7", &[0, 3, 7, 10]),
    ("dominant7", &[0, 4, 7, 10]),
    ("power", &[0, 7, 12]),
];

pub fn scale(name: &str) -> Option<&'static [u8]> {
    SCALES.iter().find(|(n, _)| *n == name).map(|(_, s)| *s)
}

pub fn in_scale(key: u8, root: u8, intervals: &[u8]) -> bool {
    intervals.contains(&((key + 12 - root % 12) % 12))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Flip {
    pattern: u64,
    channel: u64,
    #[serde(default)]
    indices: Vec<usize>,
    #[serde(default)]
    horizontal: bool,
    #[serde(default)]
    vertical: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Strum {
    pattern: u64,
    channel: u64,
    #[serde(default)]
    indices: Vec<usize>,
    /// Ticks between successive chord notes; positive strums low to high.
    time: i64,
    /// Velocity change per note; positive decreases.
    #[serde(default)]
    velocity: i16,
    #[serde(default)]
    preserve_end: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Limit {
    pattern: u64,
    channel: u64,
    #[serde(default)]
    indices: Vec<usize>,
    low: u8,
    high: u8,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Stamp {
    pattern: u64,
    channel: u64,
    start: i64,
    length: i64,
    root: u8,
    chord: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Arp {
    pattern: u64,
    channel: u64,
    #[serde(default)]
    indices: Vec<usize>,
    step: i64,
    /// "up", "down" or "updown".
    direction: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SnapScale {
    pattern: u64,
    channel: u64,
    #[serde(default)]
    indices: Vec<usize>,
    root: u8,
    scale: String,
}

/// Split a score into (picked, untouched) by index; empty `indices` picks all.
fn split(notes: &[Note], indices: &[usize]) -> (Vec<Note>, Vec<Note>) {
    if indices.is_empty() {
        return (notes.to_vec(), Vec::new());
    }
    let mut picked = Vec::new();
    let mut rest = Vec::new();
    for (i, note) in notes.iter().enumerate() {
        if indices.contains(&i) {
            picked.push(note.clone());
        } else {
            rest.push(note.clone());
        }
    }
    (picked, rest)
}

fn edit(
    session: &mut Session,
    pattern: u64,
    channel: u64,
    indices: &[usize],
    tool: impl FnOnce(Vec<Note>) -> Result<Vec<Note>, CmdError>,
) -> Result<bool, CmdError> {
    session.edit_project(|project| {
        if project.channel(ChannelId(channel)).is_none() {
            return Err(CmdError::Failed("channel does not exist".into()));
        }
        let pattern = project
            .patterns
            .iter_mut()
            .find(|p| p.id == PatternId(pattern))
            .ok_or_else(|| CmdError::Failed("pattern does not exist".into()))?;
        let notes = pattern.notes.entry(ChannelId(channel)).or_default();
        let (picked, mut rest) = split(notes, indices);
        let changed = tool(picked)?;
        if changed
            .iter()
            .any(|n| n.start.0 < 0 || n.length.0 < 0 || n.key > MAX_KEY)
        {
            return Err(CmdError::Failed("the tool moves notes out of range".into()));
        }
        rest.extend(changed);
        rest.sort_by_key(|n| (n.start, n.key, n.length));
        *notes = rest;
        Ok(())
    })
}

/// Chords: notes sharing a start time [Manual: pianoroll_strum, "any overlapping notes, that
/// share the same start time"].
fn chords(mut notes: Vec<Note>) -> Vec<Vec<Note>> {
    notes.sort_by_key(|n| (n.start, n.key));
    let mut groups: Vec<Vec<Note>> = Vec::new();
    for note in notes {
        match groups.last_mut() {
            Some(group) if group[0].start == note.start => group.push(note),
            _ => groups.push(vec![note]),
        }
    }
    groups
}

pub(super) fn run(session: &mut Session, id: &str, params: Value) -> Result<bool, CmdError> {
    match id {
        "note.flip" => {
            let p: Flip = parse(id, params)?;
            if !p.horizontal && !p.vertical {
                return Err(invalid(id, "choose horizontal and/or vertical"));
            }
            edit(session, p.pattern, p.channel, &p.indices, |mut notes| {
                let (Some(low), Some(high)) = (
                    notes.iter().map(|n| n.key).min(),
                    notes.iter().map(|n| n.key).max(),
                ) else {
                    return Ok(notes);
                };
                let first = notes.iter().map(|n| n.start.0).min().unwrap_or(0);
                let last = notes.iter().map(|n| n.end().0).max().unwrap_or(0);
                for note in &mut notes {
                    if p.vertical {
                        note.key = low + high - note.key;
                    }
                    if p.horizontal {
                        // A perfect mirror in time [Manual: pianoroll_flip, "Flip Horizontally"].
                        note.start = Tick(first + last - note.end().0);
                    }
                }
                Ok(notes)
            })
        }
        "note.strum" => {
            let p: Strum = parse(id, params)?;
            edit(session, p.pattern, p.channel, &p.indices, |notes| {
                let mut out = Vec::new();
                for mut chord in chords(notes) {
                    if p.time < 0 {
                        chord.reverse(); // high to low
                    }
                    for (i, mut note) in chord.into_iter().enumerate() {
                        let shift = p.time.abs() * i as i64;
                        note.start = Tick(note.start.0 + shift);
                        if p.preserve_end {
                            note.length = Tick((note.length.0 - shift).max(0));
                        }
                        let velocity = i32::from(note.velocity) - i32::from(p.velocity) * i as i32;
                        note.velocity = velocity.clamp(1, 127) as u8;
                        out.push(note);
                    }
                }
                Ok(out)
            })
        }
        // Transpose by octaves into [low, high] [Manual: pianoroll_limit].
        "note.limit" => {
            let p: Limit = parse(id, params)?;
            if p.high > MAX_KEY || p.high < p.low.saturating_add(11) {
                return Err(invalid(id, "the range must span at least an octave"));
            }
            edit(session, p.pattern, p.channel, &p.indices, |mut notes| {
                for note in &mut notes {
                    let mut key = i16::from(note.key);
                    while key > i16::from(p.high) {
                        key -= 12;
                    }
                    while key < i16::from(p.low) {
                        key += 12;
                    }
                    note.key = key as u8;
                }
                Ok(notes)
            })
        }
        "note.stamp" => {
            let p: Stamp = parse(id, params)?;
            let shape = CHORDS
                .iter()
                .find(|(n, _)| *n == p.chord)
                .map(|(_, s)| *s)
                .ok_or_else(|| invalid(id, format!("unknown chord {:?}", p.chord)))?;
            if p.start < 0 || p.length <= 0 {
                return Err(invalid(id, "start must be >= 0 and length > 0"));
            }
            edit(session, p.pattern, p.channel, &[usize::MAX], |_| {
                Ok(shape
                    .iter()
                    .map(|i| Note::new(Tick(p.start), Tick(p.length), p.root.saturating_add(*i)))
                    .collect())
            })
        }
        // Hypothesis H-PR-3: the Arpeggiator dialog page is unavailable; we turn each chord into
        // a run of `step`-long notes over the chord's length.
        "note.arpeggiate" => {
            let p: Arp = parse(id, params)?;
            if p.step <= 0 || !matches!(p.direction.as_str(), "up" | "down" | "updown") {
                return Err(invalid(
                    id,
                    "step must be > 0 and direction up, down or updown",
                ));
            }
            edit(session, p.pattern, p.channel, &p.indices, |notes| {
                let mut out = Vec::new();
                for chord in chords(notes) {
                    let start = chord[0].start.0;
                    let end = chord
                        .iter()
                        .map(|n| n.end().0)
                        .max()
                        .unwrap_or(start + p.step);
                    let mut order: Vec<Note> = chord.clone();
                    match p.direction.as_str() {
                        "down" => order.reverse(),
                        "updown" if order.len() > 2 => {
                            let mut back: Vec<Note> = order[1..order.len() - 1].to_vec();
                            back.reverse();
                            order.extend(back);
                        }
                        _ => {}
                    }
                    let mut t = start;
                    let mut i = 0;
                    while t < end {
                        let mut note = order[i % order.len()].clone();
                        note.start = Tick(t);
                        note.length = Tick(p.step.min(end - t));
                        out.push(note);
                        t += p.step;
                        i += 1;
                    }
                }
                Ok(out)
            })
        }
        // "Snap PR notes to scale - Forces edited Piano roll notes to the nearest valid note"
        // [Manual: pianoroll]; ties go down (our choice).
        "note.snap_scale" => {
            let p: SnapScale = parse(id, params)?;
            let intervals = scale(&p.scale).ok_or_else(|| invalid(id, "unknown scale"))?;
            let root = p.root % 12;
            edit(session, p.pattern, p.channel, &p.indices, |mut notes| {
                for note in &mut notes {
                    let key = note.key;
                    let candidates =
                        (0..=6u8).flat_map(|d| [key.checked_sub(d), key.checked_add(d)]);
                    if let Some(found) = candidates
                        .flatten()
                        .find(|k| *k <= MAX_KEY && in_scale(*k, root, intervals))
                    {
                        note.key = found;
                    }
                }
                Ok(notes)
            })
        }
        _ => Err(CmdError::UnknownCommand(id.to_owned())),
    }
}
