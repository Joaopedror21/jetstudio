use jet_model::Project;
use std::sync::Arc;

/// Snapshot history. M1 projects contain no decoded audio buffers.
#[derive(Clone, Default)]
pub struct History {
    past: Vec<Arc<Project>>,
    future: Vec<Arc<Project>>,
    last_was_undo: bool,
}

impl History {
    const LIMIT: usize = 100;

    pub fn record(&mut self, previous: Arc<Project>) {
        self.past.push(previous);
        if self.past.len() > Self::LIMIT {
            self.past.remove(0);
        }
        self.future.clear();
        self.last_was_undo = false;
    }

    pub fn undo(&mut self, current: Arc<Project>) -> Option<Arc<Project>> {
        let previous = self.past.pop()?;
        self.future.push(current);
        self.last_was_undo = true;
        Some(previous)
    }

    pub fn redo(&mut self, current: Arc<Project>) -> Option<Arc<Project>> {
        let next = self.future.pop()?;
        self.past.push(current);
        self.last_was_undo = false;
        Some(next)
    }

    pub fn toggle_last(&mut self, current: Arc<Project>) -> Option<Arc<Project>> {
        if self.last_was_undo {
            self.redo(current)
        } else {
            self.undo(current)
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }
}
