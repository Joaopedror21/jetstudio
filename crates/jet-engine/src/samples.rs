//! Decoded audio for project resources. Kept outside the undoable `Project`: resources are
//! append-only, so an undo never leaves a channel pointing at freed audio.

use jet_dsp::SampleData;
use jet_model::{Project, Resource, ResourceId, ResourceStatus};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Default, Clone)]
pub struct SamplePool {
    data: HashMap<ResourceId, Arc<SampleData>>,
    generation: u64,
    /// Audio dropped from the pool, freed by the host once nothing else holds it.
    retired: Vec<Arc<SampleData>>,
}

impl SamplePool {
    pub fn get(&self, id: ResourceId) -> Option<Arc<SampleData>> {
        self.data.get(&id).cloned()
    }

    pub fn insert(&mut self, id: ResourceId, data: Arc<SampleData>) {
        if let Some(old) = self.data.insert(id, data) {
            self.retired.push(old);
        }
        self.generation = self.generation.wrapping_add(1);
    }

    /// Changes whenever audio is added, so plans are rebuilt.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn clear(&mut self) {
        self.retired.extend(self.data.drain().map(|(_, data)| data));
        self.generation = self.generation.wrapping_add(1);
    }

    /// Audio removed from the pool. Voices may still read it, so the host keeps these until
    /// it holds the last reference: the audio thread must never free memory.
    pub fn take_retired(&mut self) -> Vec<Arc<SampleData>> {
        std::mem::take(&mut self.retired)
    }

    /// Decode every resource of `project`. A file not found where the project says is looked
    /// up by name in `search` folders ("FL Studio searches ... the extra search folders"
    /// [Manual: channelrack, "Red Channel buttons"]). Returns the ids that could not be loaded
    /// and the new location of each resource found in a search folder.
    pub fn load_project(
        &mut self,
        project: &Project,
        base: Option<&Path>,
        search: &[PathBuf],
    ) -> (Vec<ResourceId>, Vec<(ResourceId, PathBuf)>) {
        self.clear();
        let mut missing = Vec::new();
        let mut moved = Vec::new();
        for resource in &project.resources {
            let found = resolve(resource, base).or_else(|| {
                let name = Path::new(&resource.path_rel.replace('\\', "/"))
                    .file_name()?
                    .to_os_string();
                let path = find_by_name(search, &name)?;
                moved.push((resource.id, path.clone()));
                Some(path)
            });
            match found.and_then(|path| jet_media::decode_file(&path).ok()) {
                Some(data) => self.insert(resource.id, Arc::new(data)),
                None => {
                    moved.retain(|(id, _)| *id != resource.id);
                    missing.push(resource.id);
                }
            }
        }
        (missing, moved)
    }
}

/// The file a resource points to: relative to the project first, then the absolute hint.
pub fn resolve(resource: &Resource, base: Option<&Path>) -> Option<PathBuf> {
    if let Some(base) = base
        && !resource.path_rel.is_empty()
    {
        let candidate = base.join(&resource.path_rel);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    let direct = PathBuf::from(&resource.path_rel);
    if direct.is_absolute() && direct.is_file() {
        return Some(direct);
    }
    resource
        .path_abs_hint
        .as_deref()
        .map(PathBuf::from)
        .filter(|path| path.is_file())
}

/// The first file named `name` under `folders` (sub-folders included), visiting at most 50 000
/// entries so a huge folder cannot stall loading.
pub fn find_by_name(folders: &[PathBuf], name: &std::ffi::OsStr) -> Option<PathBuf> {
    let mut budget = 50_000_usize;
    let mut stack: Vec<PathBuf> = folders.iter().rev().cloned().collect();
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut subdirs = Vec::new();
        for entry in entries.flatten() {
            budget = budget.checked_sub(1)?;
            let path = entry.path();
            if path.is_dir() {
                subdirs.push(path);
            } else if path.file_name() == Some(name) {
                return Some(path);
            }
        }
        subdirs.sort();
        stack.extend(subdirs.into_iter().rev());
    }
    None
}

/// Path stored in `path_rel`: relative to `base` when the file is inside it.
pub fn relative_to(path: &Path, base: Option<&Path>) -> String {
    if let Some(base) = base
        && let Ok(stripped) = path.strip_prefix(base)
    {
        return stripped.to_string_lossy().replace('\\', "/");
    }
    path.to_string_lossy().into_owned()
}

pub fn new_resource(id: ResourceId, path: &Path, base: Option<&Path>) -> Resource {
    Resource {
        id,
        path_rel: relative_to(path, base),
        path_abs_hint: Some(path.to_string_lossy().into_owned()),
        hash: None,
        status: ResourceStatus::Available,
    }
}
