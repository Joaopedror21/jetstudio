use super::{empty, invalid, parse};
use crate::samples::relative_to;
use crate::{CmdError, History, Session};
use jet_audio_graph::Tail;
use jet_media::WavFormat;
use jet_model::{Project, ResourceStatus};
use jet_seq::Source;
use serde::Deserialize;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PathParams {
    path: PathBuf,
}

/// Export dialog options [Manual: fformats_save_export].
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExportParams {
    path: PathBuf,
    /// "song" or "pattern" ("Mode - Choose to render the whole Song or the currently selected Pattern").
    #[serde(default)]
    mode: Option<String>,
    /// 16, 24 or 32 (float).
    #[serde(default)]
    bits: Option<u16>,
    /// "leave" (default), "cut" or "wrap".
    #[serde(default)]
    tail: Option<String>,
    #[serde(default)]
    rate: Option<u32>,
}

fn reset_after_load(session: &mut Session, project: Project, path: Option<PathBuf>) {
    session.selected_pattern = project
        .patterns
        .first()
        .map(|p| p.id)
        .unwrap_or(jet_model::PatternId(1));
    session.selected_channel = None;
    let base = path
        .as_deref()
        .and_then(Path::parent)
        .map(Path::to_path_buf);
    let missing = session.samples.load_project(&project, base.as_deref());
    let mut project = project;
    for resource in &mut project.resources {
        resource.status = if missing.contains(&resource.id) {
            ResourceStatus::Missing
        } else {
            ResourceStatus::Available
        };
    }
    session.project = Arc::new(project);
    session.history = History::default();
    session.path = path;
    session.revision = session.revision.saturating_add(1);
    session
        .publisher
        .on_change(session.revision, Arc::clone(&session.project));
}

/// Store resource paths relative to the new project location.
fn rebase(project: &Project, target: &Path) -> Project {
    let base = target.parent();
    let mut next = project.clone();
    for resource in &mut next.resources {
        if let Some(abs) = resource.path_abs_hint.clone() {
            resource.path_rel = relative_to(Path::new(&abs), base);
        }
    }
    next
}

pub(super) fn run(session: &mut Session, id: &str, params: Value) -> Result<bool, CmdError> {
    match id {
        "file.new" => {
            empty(id, params)?;
            reset_after_load(session, Project::new(), None);
            Ok(true)
        }
        "file.open" => {
            let p: PathParams = parse(id, params)?;
            let project = jet_format::load(&p.path).map_err(|e| CmdError::Failed(e.to_string()))?;
            reset_after_load(session, project, Some(p.path));
            Ok(true)
        }
        "file.save" => {
            empty(id, params)?;
            let path = session
                .path
                .clone()
                .ok_or_else(|| CmdError::Disabled("Save As a project first".into()))?;
            let project = rebase(&session.project, &path);
            jet_format::save_atomic(&path, &project)
                .map_err(|e| CmdError::Failed(e.to_string()))?;
            Ok(false)
        }
        "file.save_as" => {
            let p: PathParams = parse(id, params)?;
            let project = rebase(&session.project, &p.path);
            jet_format::save_atomic(&p.path, &project)
                .map_err(|e| CmdError::Failed(e.to_string()))?;
            session.project = Arc::new(project);
            session.path = Some(p.path);
            Ok(false)
        }
        "file.export_wav" => {
            let p: ExportParams = parse(id, params)?;
            let format = match p.bits.unwrap_or(16) {
                16 => WavFormat::Int16,
                24 => WavFormat::Int24,
                32 => WavFormat::Float32,
                _ => return Err(invalid(id, "bits must be 16, 24 or 32")),
            };
            let tail = match p.tail.as_deref().unwrap_or("leave") {
                "leave" => Tail::LeaveRemainder,
                "cut" => Tail::Cut,
                "wrap" => Tail::Wrap,
                _ => return Err(invalid(id, "tail must be leave, cut or wrap")),
            };
            // Sample rates "between 22000 Hz ... and 192000 Hz are possible".
            let rate = p.rate.unwrap_or(44_100);
            if !(22_000..=192_000).contains(&rate) {
                return Err(invalid(id, "rate must be 22000..=192000"));
            }
            let source = match p.mode.as_deref().unwrap_or("song") {
                "song" => Source::Song,
                "pattern" => Source::Pattern(session.selected_pattern),
                _ => return Err(invalid(id, "mode must be song or pattern")),
            };
            let project = &session.project;
            let length = match source {
                Source::Song => {
                    if project.active().is_none_or(|a| a.clips.is_empty()) {
                        return Err(CmdError::Disabled("The Playlist is empty".into()));
                    }
                    jet_seq::song_length(project)
                }
                Source::Pattern(pattern) => project
                    .pattern(pattern)
                    .map(|found| jet_seq::pattern_length(project, found))
                    .unwrap_or(1),
            };
            let plan = jet_audio_graph::build_plan(project, source, rate, false, &|id| {
                session.samples.get(id)
            });
            let audio = jet_audio_graph::render_offline(plan, length, tail);
            jet_media::write_wav(&p.path, rate, 2, &audio, format)
                .map_err(|e| CmdError::Failed(e.to_string()))?;
            Ok(false)
        }
        _ => Err(CmdError::UnknownCommand(id.to_owned())),
    }
}
