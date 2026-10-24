//! MIDI file export/import and the note clipboard.

use super::{invalid, parse};
use crate::{CmdError, Session};
use jet_format::smf::{Smf, SmfNote, SmfTrack};
use jet_model::{ChannelId, MAX_KEY, Note, PatternId, Project};
use jet_seq::Source;
use jet_time::Tick;
use serde::Deserialize;
use serde_json::Value;
use std::path::PathBuf;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExportProject {
    path: PathBuf,
    #[serde(default)]
    mode: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ChannelFile {
    pattern: u64,
    channel: u64,
    path: PathBuf,
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
struct Paste {
    pattern: u64,
    channel: u64,
    /// Tick of the leftmost visible bar [Manual: pianoroll_menu, "Notes are pasted starting
    /// from the leftmost bar that is visible in the Piano roll window"].
    at: i64,
}

fn tempo_us(project: &Project) -> u32 {
    (60_000_000_000_u64 / u64::from(project.settings.tempo.0.max(1))) as u32
}

fn track_for(
    project: &Project,
    events: &[jet_seq::NoteEvent],
    gate_step: i64,
    channel: ChannelId,
    midi_channel: u8,
) -> SmfTrack {
    SmfTrack {
        name: project
            .channel(channel)
            .map_or_else(String::new, |c| c.name.clone()),
        notes: events
            .iter()
            .filter(|e| e.channel == channel && !e.audio)
            .map(|e| SmfNote {
                start: e.tick.max(0) as u64,
                length: if e.length > 0 { e.length } else { gate_step }.max(1) as u64,
                key: e.key.min(127),
                velocity: e.velocity.clamp(1, 127),
                channel: midi_channel,
            })
            .collect(),
    }
}

/// Selected notes, or all of them when the selection is empty.
fn picked(notes: &[Note], indices: &[usize]) -> Vec<Note> {
    if indices.is_empty() {
        notes.to_vec()
    } else {
        indices
            .iter()
            .filter_map(|&i| notes.get(i).cloned())
            .collect()
    }
}

pub(super) fn run(session: &mut Session, id: &str, params: Value) -> Result<bool, CmdError> {
    match id {
        // "MIDI - .mid saves Step Sequencer / Piano roll note data" [Manual: fformats_save_export].
        "file.export_mid" => {
            let p: ExportProject = parse(id, params)?;
            let project = &session.project;
            let source = match p.mode.as_deref().unwrap_or("song") {
                "song" => Source::Song,
                "pattern" => Source::Pattern(session.selected_pattern),
                _ => return Err(invalid(id, "mode must be song or pattern")),
            };
            let sequence = jet_seq::compile(project, source);
            let tracks = project
                .channels
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    track_for(
                        project,
                        &sequence.events,
                        sequence.step_ticks,
                        c.id,
                        (i % 16) as u8,
                    )
                })
                .filter(|t| !t.notes.is_empty())
                .collect();
            let smf = Smf {
                ppq: project.settings.ppq.0.min(u32::from(u16::MAX >> 1)) as u16,
                tempo_us: tempo_us(project),
                tracks,
            };
            jet_format::smf::write(&p.path, &smf).map_err(|e| CmdError::Failed(e.to_string()))?;
            Ok(false)
        }
        // Piano roll "Export as MIDI file..." [Manual: pianoroll_menu].
        "note.export_mid" => {
            let p: ChannelFile = parse(id, params)?;
            let project = &session.project;
            let pattern = project
                .pattern(PatternId(p.pattern))
                .ok_or_else(|| CmdError::Failed("pattern does not exist".into()))?;
            let mut events = Vec::new();
            jet_seq::pattern_events(project, pattern, &mut events);
            events.sort_by_key(|e| (e.tick, e.key));
            let step = project.settings.step_ticks().unwrap_or(24);
            let smf = Smf {
                ppq: project.settings.ppq.0.min(u32::from(u16::MAX >> 1)) as u16,
                tempo_us: tempo_us(project),
                tracks: vec![track_for(project, &events, step, ChannelId(p.channel), 0)],
            };
            jet_format::smf::write(&p.path, &smf).map_err(|e| CmdError::Failed(e.to_string()))?;
            Ok(false)
        }
        // "Importing MIDI from the Piano roll only adds Notes" [Manual: pianoroll_menu].
        "note.import_mid" => {
            let p: ChannelFile = parse(id, params)?;
            let smf =
                jet_format::smf::read(&p.path).map_err(|e| CmdError::Failed(e.to_string()))?;
            session.edit_project(|project| {
                let ppq = i128::from(project.settings.ppq.0);
                let scale = |t: u64| (i128::from(t) * ppq / i128::from(smf.ppq.max(1))) as i64;
                let imported: Vec<Note> = smf
                    .tracks
                    .iter()
                    .flat_map(|t| t.notes.iter())
                    .filter(|n| n.key <= MAX_KEY)
                    .map(|n| {
                        let mut note =
                            Note::new(Tick(scale(n.start)), Tick(scale(n.length)), n.key);
                        note.velocity = n.velocity.min(127);
                        note
                    })
                    .collect();
                if imported.is_empty() {
                    return Err(CmdError::Failed("the MIDI file has no notes".into()));
                }
                if project.channel(ChannelId(p.channel)).is_none() {
                    return Err(CmdError::Failed("channel does not exist".into()));
                }
                let pattern = project
                    .patterns
                    .iter_mut()
                    .find(|x| x.id == PatternId(p.pattern))
                    .ok_or_else(|| CmdError::Failed("pattern does not exist".into()))?;
                let notes = pattern.notes.entry(ChannelId(p.channel)).or_default();
                notes.extend(imported);
                notes.sort_by_key(|n| (n.start, n.key, n.length));
                Ok(())
            })
        }
        "note.copy" | "note.cut" => {
            let p: Selection = parse(id, params)?;
            let notes = session
                .project
                .pattern(PatternId(p.pattern))
                .and_then(|pattern| pattern.notes.get(&ChannelId(p.channel)))
                .map(|notes| picked(notes, &p.indices))
                .unwrap_or_default();
            if notes.is_empty() {
                return Err(CmdError::Disabled("No notes to copy".into()));
            }
            session.clipboard = notes;
            if id == "note.cut" {
                return super::note::run(
                    session,
                    "note.delete",
                    serde_json::json!({"pattern": p.pattern, "channel": p.channel, "indices": p.indices}),
                );
            }
            Ok(false)
        }
        "note.paste" => {
            let p: Paste = parse(id, params)?;
            if session.clipboard.is_empty() {
                return Err(CmdError::Disabled("The clipboard is empty".into()));
            }
            let clipboard = session.clipboard.clone();
            session.edit_project(|project| {
                // Keep the notes' place within their first bar; the bar lands at `at`.
                let bar = project.settings.bar_ticks().unwrap_or(384).max(1);
                let first = clipboard.iter().map(|n| n.start.0).min().unwrap_or(0);
                let shift = p.at.max(0) - first.div_euclid(bar) * bar;
                if project.channel(ChannelId(p.channel)).is_none() {
                    return Err(CmdError::Failed("channel does not exist".into()));
                }
                let pattern = project
                    .patterns
                    .iter_mut()
                    .find(|x| x.id == PatternId(p.pattern))
                    .ok_or_else(|| CmdError::Failed("pattern does not exist".into()))?;
                let notes = pattern.notes.entry(ChannelId(p.channel)).or_default();
                notes.extend(clipboard.into_iter().map(|mut n| {
                    n.start = Tick(n.start.0 + shift);
                    n
                }));
                notes.sort_by_key(|n| (n.start, n.key, n.length));
                Ok(())
            })
        }
        _ => Err(CmdError::UnknownCommand(id.to_owned())),
    }
}
