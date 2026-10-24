#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

//! Versioned JSON project files with same-directory atomic replacement, and MIDI files.

pub mod smf;

use jet_model::{ModelError, Project, ResourceStatus};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use thiserror::Error;

pub const FORMAT_VERSION: u32 = 1;
pub const MAX_PROJECT_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum FormatError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid project JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid project: {0}")]
    Model(#[from] ModelError),
    #[error("unsupported format version {0}")]
    UnsupportedVersion(u32),
    #[error("project file exceeds {MAX_PROJECT_BYTES} bytes")]
    FileTooLarge,
    #[error("project path must have a file name")]
    InvalidPath,
    #[error("could not reserve a temporary file beside the project")]
    TempCollision,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    format_version: u32,
    app_version: String,
    project: Project,
}

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TempCleanup(Option<PathBuf>);

impl TempCleanup {
    fn disarm(&mut self) {
        self.0 = None;
    }
}

impl Drop for TempCleanup {
    fn drop(&mut self) {
        if let Some(path) = self.0.take() {
            let _ = fs::remove_file(path);
        }
    }
}

/// Write, sync, then rename a temporary file in the target directory.
pub fn save_atomic(path: impl AsRef<Path>, project: &Project) -> Result<(), FormatError> {
    let path = path.as_ref();
    project.validate()?;
    let file_name = path.file_name().ok_or(FormatError::InvalidPath)?;
    let parent = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    fs::create_dir_all(parent)?;

    let envelope = Envelope {
        format_version: FORMAT_VERSION,
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
        project: project.clone(),
    };
    let data = serde_json::to_vec_pretty(&envelope)?;
    if data.len() as u64 > MAX_PROJECT_BYTES {
        return Err(FormatError::FileTooLarge);
    }

    let mut reserved = None;
    for _ in 0..16 {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let name = format!(
            ".{}.{}.{}.tmp",
            file_name.to_string_lossy(),
            std::process::id(),
            sequence
        );
        let temp_path = parent.join(name);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
        {
            Ok(file) => {
                reserved = Some((temp_path, file));
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(FormatError::Io(error)),
        }
    }
    let (temp_path, mut file) = reserved.ok_or(FormatError::TempCollision)?;
    let mut cleanup = TempCleanup(Some(temp_path.clone()));
    file.write_all(&data)?;
    file.flush()?;
    file.sync_all()?;
    drop(file);
    fs::rename(&temp_path, path)?;
    cleanup.disarm();
    Ok(())
}

/// Load and validate a project. A missing sample marks the resource missing
/// without discarding the document.
pub fn load(path: impl AsRef<Path>) -> Result<Project, FormatError> {
    let path = path.as_ref();
    let file = File::open(path)?;
    if file.metadata()?.len() > MAX_PROJECT_BYTES {
        return Err(FormatError::FileTooLarge);
    }
    let mut data = Vec::new();
    file.take(MAX_PROJECT_BYTES + 1).read_to_end(&mut data)?;
    if data.len() as u64 > MAX_PROJECT_BYTES {
        return Err(FormatError::FileTooLarge);
    }
    let envelope: Envelope = serde_json::from_slice(&data)?;
    if envelope.format_version != FORMAT_VERSION {
        return Err(FormatError::UnsupportedVersion(envelope.format_version));
    }
    let mut project = envelope.project;
    project.validate()?;
    let base = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    for resource in &mut project.resources {
        let relative = !resource.path_rel.is_empty() && base.join(&resource.path_rel).is_file();
        let absolute_hint = resource
            .path_abs_hint
            .as_deref()
            .is_some_and(|hint| Path::new(hint).is_file());
        resource.status = if relative || absolute_hint {
            ResourceStatus::Available
        } else {
            ResourceStatus::Missing
        };
    }
    Ok(project)
}

#[cfg(test)]
mod tests {
    use super::*;
    use jet_model::{Channel, ClipInstance, PlaylistTrack, Resource, ResourceId, StepSeq};
    use jet_time::Tick;

    fn test_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "jet-format-test-{}-{}-{name}.jetproj",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn save_load_and_replace_existing_file() {
        let path = test_path("replace");
        let mut project = Project::new();
        assert!(save_atomic(&path, &project).is_ok());
        let initial = load(&path).expect("first load");
        assert_eq!(initial, project);
        project.settings.swing = 25;
        assert!(save_atomic(&path, &project).is_ok());
        assert_eq!(load(&path).expect("second load"), project);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn populated_project_round_trips_with_shared_pattern_reference() {
        let path = test_path("populated");
        let mut project = Project::new();
        let channel = project.ids.channel();
        project
            .channels
            .push(Channel::sampler(channel, "Original sample"));
        let mut sequence = StepSeq::with_len(8).expect("valid sequence length");
        sequence.cells[2].on = true;
        project.patterns[0].steps.insert(channel, sequence);
        let track = project.ids.track();
        project.arrangements[0]
            .tracks
            .push(PlaylistTrack::new(track, "Percussion"));
        let pattern = project.patterns[0].id;
        for start in [Tick(0), Tick(96)] {
            let clip = project.ids.clip();
            project.arrangements[0].clips.push(ClipInstance::pattern(
                clip,
                track,
                pattern,
                start,
                Tick(96),
            ));
        }
        save_atomic(&path, &project).expect("save populated project");
        assert_eq!(load(&path).expect("load populated project"), project);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn truncated_json_fails_without_panic() {
        let path = test_path("truncated");
        fs::write(&path, b"{\"format_version\":1,\"project\":").expect("write fixture");
        assert!(matches!(load(&path), Err(FormatError::Json(_))));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn missing_resource_is_reported_but_project_loads() {
        let path = test_path("missing");
        let mut project = Project::new();
        project.resources.push(Resource {
            id: ResourceId(1),
            path_rel: "definitely-missing-sample.wav".to_owned(),
            path_abs_hint: None,
            hash: None,
            status: ResourceStatus::Available,
        });
        project.ids.next_resource = 2;
        save_atomic(&path, &project).expect("save fixture");
        let loaded = load(&path).expect("load fixture");
        assert_eq!(loaded.resources[0].status, ResourceStatus::Missing);
        let _ = fs::remove_file(path);
    }
}
