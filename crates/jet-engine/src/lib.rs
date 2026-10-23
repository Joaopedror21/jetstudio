//! Headless session, commands, history and silent transport for M1.
#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod cmd;
mod history;
mod transport;

pub use cmd::{CommandSpec, DisabledReason, command_specs};
pub use history::History;
pub use transport::{Clock, ManualClock, PlayMode, TransportSnapshot, TransportState, WallClock};

use jet_model::{PatternId, Project};
use jet_time::Tick;
use serde::Serialize;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;

/// A command response suitable for the CLI and control channel.
#[derive(Debug, Clone, Serialize)]
pub struct Outcome {
    pub command: String,
    pub revision: u64,
    pub changed: bool,
}

#[derive(Debug, Error)]
pub enum CmdError {
    #[error("unknown command: {0}")]
    UnknownCommand(String),
    #[error("invalid parameters for {command}: {message}")]
    InvalidParams { command: String, message: String },
    #[error("command disabled: {0}")]
    Disabled(String),
    #[error("command failed: {0}")]
    Failed(String),
    #[error("command panicked; the project was preserved")]
    Panicked,
}

pub trait SnapshotPublisher: Send {
    fn on_change(&mut self, revision: u64, project: Arc<Project>);
}

#[derive(Default)]
pub struct NullPublisher;

impl SnapshotPublisher for NullPublisher {
    fn on_change(&mut self, _revision: u64, _project: Arc<Project>) {}
}

pub struct Session {
    project: Arc<Project>,
    history: History,
    transport: transport::Transport,
    selected_pattern: PatternId,
    selected_channel: Option<jet_model::ChannelId>,
    path: Option<PathBuf>,
    revision: u64,
    publisher: Box<dyn SnapshotPublisher>,
}

impl Session {
    pub fn new_default() -> Self {
        Self::with_clock(Box::new(WallClock::default()))
    }

    pub fn with_clock(clock: Box<dyn Clock>) -> Self {
        let project = Project::new();
        let selected_pattern = project
            .patterns
            .first()
            .map(|p| p.id)
            .unwrap_or(PatternId(1));
        Self {
            project: Arc::new(project),
            history: History::default(),
            transport: transport::Transport::new(clock),
            selected_pattern,
            selected_channel: None,
            path: None,
            revision: 0,
            publisher: Box::new(NullPublisher),
        }
    }

    pub fn from_project(project: Project) -> Result<Self, CmdError> {
        project
            .validate()
            .map_err(|e| CmdError::Failed(e.to_string()))?;
        let selected_pattern = project
            .patterns
            .first()
            .map(|p| p.id)
            .unwrap_or(PatternId(1));
        Ok(Self {
            project: Arc::new(project),
            history: History::default(),
            transport: transport::Transport::new(Box::new(WallClock::default())),
            selected_pattern,
            selected_channel: None,
            path: None,
            revision: 0,
            publisher: Box::new(NullPublisher),
        })
    }

    pub fn project(&self) -> &Project {
        &self.project
    }
    pub fn selected_pattern(&self) -> PatternId {
        self.selected_pattern
    }
    pub fn selected_channel(&self) -> Option<jet_model::ChannelId> {
        self.selected_channel
    }
    pub fn path(&self) -> Option<&std::path::Path> {
        self.path.as_deref()
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn transport_snapshot(&self) -> TransportSnapshot {
        self.transport
            .snapshot(self.project.settings.ppq, self.project.settings.tempo)
    }

    pub fn set_publisher(&mut self, publisher: Box<dyn SnapshotPublisher>) {
        self.publisher = publisher;
    }

    pub fn execute(&mut self, id: &str, params: Value) -> Result<Outcome, CmdError> {
        let old_project = Arc::clone(&self.project);
        let old_selection = self.selected_pattern;
        let old_channel = self.selected_channel;
        let old_path = self.path.clone();
        let old_history = self.history.clone();
        let old_revision = self.revision;
        let old_transport = self.transport.checkpoint();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            cmd::dispatch(self, id, params)
        }));
        let changed = match result {
            Ok(Ok(changed)) => changed,
            Ok(Err(error)) => {
                self.project = old_project;
                self.selected_pattern = old_selection;
                self.selected_channel = old_channel;
                self.path = old_path;
                self.history = old_history;
                self.revision = old_revision;
                self.transport.restore(old_transport);
                return Err(error);
            }
            Err(_) => {
                self.project = old_project;
                self.selected_pattern = old_selection;
                self.selected_channel = old_channel;
                self.path = old_path;
                self.history = old_history;
                self.revision = old_revision;
                self.transport.restore(old_transport);
                return Err(CmdError::Panicked);
            }
        };
        Ok(Outcome {
            command: id.to_owned(),
            revision: self.revision,
            changed,
        })
    }

    fn edit_project(
        &mut self,
        edit: impl FnOnce(&mut Project) -> Result<(), CmdError>,
    ) -> Result<bool, CmdError> {
        let mut next = (*self.project).clone();
        edit(&mut next)?;
        next.validate()
            .map_err(|e| CmdError::Failed(e.to_string()))?;
        if next == *self.project {
            return Ok(false);
        }
        self.history.record(Arc::clone(&self.project));
        self.project = Arc::new(next);
        self.revision = self.revision.saturating_add(1);
        self.publisher
            .on_change(self.revision, Arc::clone(&self.project));
        Ok(true)
    }

    fn swap_project(&mut self, project: Arc<Project>) {
        self.project = project;
        if !self
            .project
            .patterns
            .iter()
            .any(|pattern| pattern.id == self.selected_pattern)
            && let Some(pattern) = self.project.patterns.first()
        {
            self.selected_pattern = pattern.id;
        }
        if self
            .selected_channel
            .is_some_and(|id| !self.project.channels.iter().any(|channel| channel.id == id))
        {
            self.selected_channel = None;
        }
        self.revision = self.revision.saturating_add(1);
        self.publisher
            .on_change(self.revision, Arc::clone(&self.project));
    }

    pub fn seek(&mut self, tick: Tick) {
        self.transport.seek(tick);
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new_default()
    }
}
