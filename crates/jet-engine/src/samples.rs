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
}

impl SamplePool {
    pub fn get(&self, id: ResourceId) -> Option<Arc<SampleData>> {
        self.data.get(&id).cloned()
    }

    pub fn insert(&mut self, id: ResourceId, data: Arc<SampleData>) {
        self.data.insert(id, data);
        self.generation = self.generation.wrapping_add(1);
    }

    /// Changes whenever audio is added, so plans are rebuilt.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn clear(&mut self) {
        self.data.clear();
        self.generation = self.generation.wrapping_add(1);
    }

    /// Decode every resource of `project`; returns the ids that could not be loaded.
    pub fn load_project(&mut self, project: &Project, base: Option<&Path>) -> Vec<ResourceId> {
        self.clear();
        let mut missing = Vec::new();
        for resource in &project.resources {
            match resolve(resource, base).and_then(|path| jet_media::decode_file(&path).ok()) {
                Some(data) => self.insert(resource.id, Arc::new(data)),
                None => missing.push(resource.id),
            }
        }
        missing
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
