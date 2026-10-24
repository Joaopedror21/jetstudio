#![forbid(unsafe_code)]

mod app;
mod audio_settings;
mod browser;
mod channel_rack;
mod channel_settings;
pub mod demo;
mod graph_editor;
mod icons;
mod mixer;
mod piano_roll;
mod playlist;
mod probe;
mod theme;
mod toolbar;
mod widgets;
mod workspace;

pub use app::JetApp;
pub use probe::{LayoutProbe, ProbeEntry};
pub use workspace::{WindowKind, Workspace, WsAction};
