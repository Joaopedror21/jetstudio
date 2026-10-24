//! Headless session, commands, history, transport and the audio render seam.
#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod cmd;
mod history;
mod samples;
mod transport;

pub use cmd::note_tools::{CHORDS, SCALES};
pub use cmd::{CommandSpec, DisabledReason, command_specs};
pub use history::History;
pub use samples::SamplePool;
pub use transport::{Clock, ManualClock, PlayMode, TransportSnapshot, TransportState, WallClock};

use jet_audio_graph::{RenderPlan, TransportSync};
use jet_model::{ChannelId, PatternId, Project};
use jet_seq::Source;
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

/// What the render plan depends on; a change means the plan must be rebuilt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlanKey {
    pub revision: u64,
    pub source: Source,
    pub samples: u64,
    pub metronome: bool,
    pub selection: Option<(i64, i64)>,
}

pub struct Session {
    project: Arc<Project>,
    history: History,
    transport: transport::Transport,
    selected_pattern: PatternId,
    selected_channel: Option<ChannelId>,
    path: Option<PathBuf>,
    revision: u64,
    publisher: Box<dyn SnapshotPublisher>,
    samples: SamplePool,
    panic_requested: bool,
    /// Gesture key of the command being executed, if it is part of a continuous gesture.
    gesture: Option<String>,
    /// Metronome switch: a global option, not saved in the project [Manual: menu_options].
    metronome: bool,
    /// Note clipboard (Ctrl+C/X/V in the Piano roll); not part of the project or history.
    clipboard: Vec<jet_model::Note>,
    /// Folder for autosave backups [Manual: envsettings_files, "Backup (Autosave)"].
    backup_dir: Option<PathBuf>,
    /// Revision last written to the backup folder.
    backed_up: Option<u64>,
    /// Playlist timeline selection (start, end) in ticks: "The Timeline Selections will play in
    /// loop mode or render as a single section" [Manual: playlist]. Not saved, not undoable.
    time_selection: Option<(i64, i64)>,
    /// "Remember seek time - After pressing stop or (Space), Play will start from the selected
    /// position (click on the Playlist time-line to select)" [Manual: toolbar_panels]. A global
    /// option, not saved in the project.
    remember_seek: bool,
    /// Last position set with `transport.position.set`.
    seek_point: i64,
    /// Folders searched for samples a project cannot find (the Browser folders).
    search_folders: Vec<PathBuf>,
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
            samples: SamplePool::default(),
            panic_requested: false,
            gesture: None,
            metronome: false,
            clipboard: Vec::new(),
            backup_dir: None,
            backed_up: None,
            time_selection: None,
            remember_seek: false,
            seek_point: 0,
            search_folders: Vec::new(),
        }
    }

    pub fn from_project(project: Project) -> Result<Self, CmdError> {
        project
            .validate()
            .map_err(|e| CmdError::Failed(e.to_string()))?;
        let mut session = Self::new_default();
        session.selected_pattern = project
            .patterns
            .first()
            .map(|p| p.id)
            .unwrap_or(PatternId(1));
        session.project = Arc::new(project);
        Ok(session)
    }

    pub fn project(&self) -> &Project {
        &self.project
    }
    pub fn project_arc(&self) -> Arc<Project> {
        Arc::clone(&self.project)
    }
    pub fn selected_pattern(&self) -> PatternId {
        self.selected_pattern
    }
    pub fn selected_channel(&self) -> Option<ChannelId> {
        self.selected_channel
    }
    pub fn path(&self) -> Option<&std::path::Path> {
        self.path.as_deref()
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn samples(&self) -> &SamplePool {
        &self.samples
    }
    pub fn samples_mut(&mut self) -> &mut SamplePool {
        &mut self.samples
    }

    /// Material the transport plays now [Manual: toolbar_panels, "PAT/SONG"].
    pub fn play_source(&self) -> Source {
        match self.transport.mode() {
            PlayMode::Pattern => Source::Pattern(self.selected_pattern),
            PlayMode::Song => Source::Song,
        }
    }

    /// Folders searched by name for samples a loaded project cannot find.
    pub fn set_search_folders(&mut self, folders: Vec<PathBuf>) {
        self.search_folders = folders;
    }

    pub fn remember_seek(&self) -> bool {
        self.remember_seek
    }

    pub fn time_selection(&self) -> Option<(i64, i64)> {
        self.time_selection
    }

    /// (loop start, loop end) of the playing material: a Song-mode timeline selection, else the
    /// Song loop marker to the song end, else the whole material.
    pub fn loop_range(&self) -> (i64, i64) {
        let end = self.loop_length();
        match self.play_source() {
            Source::Song => match self.time_selection {
                Some(range) => range,
                None => (
                    jet_seq::loop_marker(&self.project)
                        .filter(|at| *at < end)
                        .unwrap_or(0),
                    end,
                ),
            },
            Source::Pattern(_) => (0, end),
        }
    }

    /// Loop length of the playing material in ticks.
    pub fn loop_length(&self) -> i64 {
        match self.play_source() {
            Source::Pattern(id) => self
                .project
                .pattern(id)
                .map(|pattern| jet_seq::pattern_length(&self.project, pattern))
                .unwrap_or(1),
            Source::Song => jet_seq::song_length(&self.project),
        }
        .max(1)
    }

    /// Transport position wrapped into the playing material, as the renderer loops it.
    pub fn transport_snapshot(&self) -> TransportSnapshot {
        let mut snapshot = self
            .transport
            .snapshot(self.project.settings.ppq, self.project.settings.tempo);
        let (start, end) = self.loop_range();
        let wrap = jet_seq::Sequence {
            length: end,
            loop_start: start,
            ..jet_seq::Sequence::default()
        };
        snapshot.position = Tick(wrap.wrap(snapshot.position.0));
        snapshot
    }

    /// Transport state for the renderer; both use the same frame counter and formula.
    /// Unwrapped transport tick at a clock frame while playing (see `Transport::tick_at_frame`).
    pub fn tick_at_frame(&self, frame: u64) -> Option<i64> {
        self.transport.tick_at_frame(
            frame,
            self.project.settings.ppq,
            self.project.settings.tempo,
        )
    }

    /// The transport clock's current frame.
    pub fn now_frame(&self) -> u64 {
        self.transport.now_frame()
    }

    pub fn transport_sync(&self) -> TransportSync {
        let (playing, anchor_tick, anchor_frame) = self.transport.anchor();
        TransportSync {
            playing,
            anchor_tick: anchor_tick.0,
            anchor_frame,
        }
    }

    /// Swap the transport clock, e.g. to the audio device's frame counter.
    pub fn set_clock(&mut self, clock: Box<dyn Clock>) {
        self.transport.set_clock(
            clock,
            self.project.settings.ppq,
            self.project.settings.tempo,
        );
    }

    pub fn plan_key(&self) -> PlanKey {
        PlanKey {
            revision: self.revision,
            source: self.play_source(),
            samples: self.samples.generation(),
            metronome: self.metronome,
            selection: self.time_selection,
        }
    }

    /// Compile the current project for live playback at `rate`.
    pub fn build_plan(&self, rate: u32) -> RenderPlan {
        let mut plan =
            jet_audio_graph::build_plan(&self.project, self.play_source(), rate, true, &|id| {
                self.samples.get(id)
            });
        plan.metronome = self.metronome;
        if matches!(self.play_source(), Source::Song) {
            let (start, end) = self.loop_range();
            plan.sequence.loop_start = start;
            plan.sequence.length = end.max(1);
        }
        plan
    }

    /// Where Play starts from a stop: the timeline selection start when the position is outside
    /// it ("only the selected portion of the Playlist will play"), else a Start marker when the
    /// position is at 0 ("Playback and time = 0 starts from the set point").
    pub(crate) fn play_start(&self, position: i64) -> Option<i64> {
        if !matches!(self.play_source(), Source::Song) {
            return None;
        }
        if let Some((start, end)) = self.time_selection {
            return (position < start || position >= end).then_some(start);
        }
        if position == 0 {
            return jet_seq::marker_at(&self.project, jet_model::MarkerAction::Start);
        }
        None
    }

    pub fn metronome(&self) -> bool {
        self.metronome
    }

    /// Where `file.autosave` writes backups; hosts point it at the user data folder.
    pub fn set_backup_dir(&mut self, dir: Option<PathBuf>) {
        self.backup_dir = dir;
    }

    /// True when the project changed since the last backup.
    pub fn needs_backup(&self) -> bool {
        self.backed_up != Some(self.revision) && self.revision > 0
    }

    fn backup_path(&self) -> Option<PathBuf> {
        let name = self
            .path
            .as_deref()
            .and_then(std::path::Path::file_stem)
            .map_or_else(
                || "Untitled".to_owned(),
                |s| s.to_string_lossy().into_owned(),
            );
        Some(
            self.backup_dir
                .as_ref()?
                .join(format!("{name} (autosave).jetproj")),
        )
    }

    pub fn set_publisher(&mut self, publisher: Box<dyn SnapshotPublisher>) {
        self.publisher = publisher;
    }

    /// Execute `id` as part of the continuous gesture `key` (a knob drag, a step paint):
    /// consecutive edits with the same key undo as one step until [`Session::end_gesture`].
    pub fn execute_gesture(
        &mut self,
        key: &str,
        id: &str,
        params: Value,
    ) -> Result<Outcome, CmdError> {
        self.gesture = Some(key.to_owned());
        let result = self.execute(id, params);
        self.gesture = None;
        result
    }

    pub fn end_gesture(&mut self) {
        self.history.end_gesture();
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
        match self.gesture.as_deref() {
            Some(key) => self.history.record_gesture(Arc::clone(&self.project), key),
            None => self.history.record(Arc::clone(&self.project)),
        }
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

    /// True once after `transport.panic`; the audio host then stops every voice.
    pub fn take_panic(&mut self) -> bool {
        std::mem::take(&mut self.panic_requested)
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
