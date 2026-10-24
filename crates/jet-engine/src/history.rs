use jet_model::Project;
use std::sync::Arc;

/// Snapshot history. Decoded audio lives in the `SamplePool`, outside these snapshots.
#[derive(Clone, Default)]
pub struct History {
    past: Vec<Arc<Project>>,
    future: Vec<Arc<Project>>,
    last_was_undo: bool,
    /// Key of the continuous gesture whose first edit opened the newest undo step.
    gesture: Option<String>,
}

impl History {
    const LIMIT: usize = 100;

    pub fn record(&mut self, previous: Arc<Project>) {
        self.gesture = None;
        self.push(previous);
    }

    /// Record an edit belonging to gesture `key`: only the first edit of an uninterrupted
    /// gesture opens a step, so a knob drag or a step paint undoes as one action
    /// [docs/FINAL_ARCHITECTURE.md §5.2].
    pub fn record_gesture(&mut self, previous: Arc<Project>, key: &str) {
        if self.gesture.as_deref() == Some(key) && !self.past.is_empty() {
            self.future.clear();
            return;
        }
        self.push(previous);
        self.gesture = Some(key.to_owned());
    }

    pub fn end_gesture(&mut self) {
        self.gesture = None;
    }

    fn push(&mut self, previous: Arc<Project>) {
        self.past.push(previous);
        if self.past.len() > Self::LIMIT {
            self.past.remove(0);
        }
        self.future.clear();
        self.last_was_undo = false;
    }

    pub fn undo(&mut self, current: Arc<Project>) -> Option<Arc<Project>> {
        self.gesture = None;
        let previous = self.past.pop()?;
        self.future.push(current);
        self.last_was_undo = true;
        Some(previous)
    }

    pub fn redo(&mut self, current: Arc<Project>) -> Option<Arc<Project>> {
        self.gesture = None;
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
