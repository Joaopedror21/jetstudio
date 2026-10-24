use std::sync::Arc;
use std::time::Duration;

use eframe::egui;
use egui::{Event, Id, Key, Rect, Sense, pos2, vec2};
use jet_dsp::SampleData;
use jet_engine::{PlanKey, PlayMode, Session, TransportState};
use jet_model::{ChannelId, ChannelKind, ClipSource};
use jet_playback::{AudioOutput, OutputRequest};
use serde_json::{Value, json};

use crate::audio_settings::{AudioSettingsAction, AudioSettingsView, paint_audio_settings};
use crate::browser::{BrowserState, paint_browser};
use crate::channel_rack::{
    ChannelView, RackAction, RackMenu, RackState, RackView, paint_channel_rack,
};
use crate::channel_settings::{ChannelSettingsData, SettingsAction, paint_channel_settings};
use crate::icons;
use crate::mixer::{MixerAction, MixerState, MixerTrackView, MixerView, paint_mixer};
use crate::piano_roll::{NoteView, PianoAction, PianoRollState, PianoRollView, paint_piano_roll};
use crate::playlist::{
    ClipView, PatternEntry, PlaylistAction, PlaylistState, PlaylistView, TrackView, paint_playlist,
    paint_title_tools,
};
use crate::probe::LayoutProbe;
use crate::theme::{self, Layout, Tokens};
use crate::toolbar::{ToolbarAction, ToolbarState, ToolbarView, paint_toolbar};
use crate::widgets::{PaintCtx, WindowGlyph};
use crate::workspace::{ResizeEdge, WindowKind, Workspace, WsAction};

/// The audio device and what the UI thread keeps for it.
struct AudioHost {
    output: AudioOutput,
    last_key: Option<PlanKey>,
    /// Decoded audio the audio thread may still read; freed here once we hold the last
    /// reference, so the audio thread never frees memory.
    graveyard: Vec<Arc<SampleData>>,
    preview_hold: Option<Arc<SampleData>>,
    scope: Vec<f32>,
    cpu: f32,
    midi: Option<jet_playback::midi::MidiInputs>,
}

/// Typing keyboard to piano [Manual: menu_options, "Typing to piano keyboard (Ctrl+T)"].
/// Hypothesis H-KB-1: the manual shows no key map; we use the common two-row layout (Z row is
/// the lower octave, Q row the upper; the rows above hold the black keys).
const TYPING_KEYS: [(Key, u8); 29] = [
    (Key::Z, 0),
    (Key::S, 1),
    (Key::X, 2),
    (Key::D, 3),
    (Key::C, 4),
    (Key::V, 5),
    (Key::G, 6),
    (Key::B, 7),
    (Key::H, 8),
    (Key::N, 9),
    (Key::J, 10),
    (Key::M, 11),
    (Key::Comma, 12),
    (Key::Q, 12),
    (Key::Num2, 13),
    (Key::W, 14),
    (Key::Num3, 15),
    (Key::E, 16),
    (Key::R, 17),
    (Key::Num5, 18),
    (Key::T, 19),
    (Key::Num6, 20),
    (Key::Y, 21),
    (Key::Num7, 22),
    (Key::U, 23),
    (Key::I, 24),
    (Key::Num9, 25),
    (Key::O, 26),
    (Key::P, 28),
];

pub struct JetApp {
    session: Session,
    /// Device settings in use and the ones being edited in the F10 window.
    audio_request: OutputRequest,
    audio_pending: OutputRequest,
    device_list: Vec<String>,
    audio_error: Option<String>,
    /// Where the audio settings are kept (`None` in tests and screenshots).
    settings_file: Option<std::path::PathBuf>,
    /// True when the app may open audio hardware (never in tests or screenshots).
    hardware_audio: bool,
    /// Waveform columns per decoded resource, computed once (drawing only).
    waveforms: std::collections::HashMap<jet_model::ResourceId, Vec<(f32, f32)>>,
    audio: Option<AudioHost>,
    audio_status: Option<String>,
    piano: PianoRollState,
    mixer: MixerState,
    workspace: Option<Workspace>,
    browser: BrowserState,
    rack: RackState,
    playlist: PlaylistState,
    toolbar: ToolbarState,
    tokens: Tokens,
    layout: Layout,
    probe: LayoutProbe,
    hint: String,
    last_error: Option<String>,
    static_capture: bool,
    fonts_installed: bool,
    pending_show: Vec<WindowKind>,
    pending_focus: Option<WindowKind>,
    /// Last autosave check [Manual: envsettings_files, "Regularly (every 5 minutes)"].
    last_autosave: std::time::Instant,
    typing: bool,
    /// Octave of the Z row in FL naming (C5 = key 60); Alt+NumPad 2-6 changes it.
    typing_octave: u8,
    typing_held: Vec<(Key, u8)>,
}

impl Default for JetApp {
    fn default() -> Self {
        Self {
            session: Session::new_default(),
            audio_request: OutputRequest::default(),
            audio_pending: OutputRequest::default(),
            device_list: Vec::new(),
            audio_error: None,
            settings_file: None,
            hardware_audio: false,
            waveforms: std::collections::HashMap::new(),
            audio: None,
            audio_status: None,
            piano: PianoRollState::default(),
            mixer: MixerState::default(),
            workspace: None,
            browser: BrowserState::default(),
            rack: RackState::default(),
            playlist: PlaylistState::default(),
            toolbar: ToolbarState::default(),
            tokens: Tokens::default(),
            layout: Layout::default(),
            probe: LayoutProbe::default(),
            hint: String::new(),
            last_error: None,
            static_capture: false,
            fonts_installed: false,
            pending_show: Vec::new(),
            pending_focus: None,
            last_autosave: std::time::Instant::now(),
            typing: false,
            typing_octave: 4,
            typing_held: Vec::new(),
        }
    }
}

enum UiAction {
    Engine(&'static str, Value),
    Workspace(WsAction),
    Rack(RackAction),
    Playlist(PlaylistAction),
    Piano(PianoAction),
    Mixer(MixerAction),
    Settings(SettingsAction),
    AudioSettings(AudioSettingsAction),
    ToggleTyping,
    OpenProject,
    SaveProject,
    SaveProjectAs,
    ExportWav,
    ExportMidi,
    AddChannel(&'static str),
    Minimize,
    Maximize,
    Quit,
}

impl JetApp {
    /// App around an existing session (demo projects, tests, the screenshot tool).
    pub fn with_session(session: Session) -> Self {
        Self {
            session,
            ..Self::default()
        }
    }

    /// App with the default audio output device. Without a device the transport runs on the
    /// system clock, silently, and the hint bar says why.
    pub fn with_audio() -> Self {
        let mut app = Self::default();
        // Backups go to a projects folder in Documents, like FL's ...\FL Studio\Projects\Backup;
        // settings go to the per-user application data folder. (A folder named just
        // "JetStudio" in Documents could be a source checkout: Windows paths ignore case.)
        let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"));
        let projects = home.as_ref().map(|h| {
            std::path::PathBuf::from(h)
                .join("Documents")
                .join("JetStudio Projects")
        });
        app.session
            .set_backup_dir(projects.map(|d| d.join("Backup")));
        let config = std::env::var_os("APPDATA")
            .map(std::path::PathBuf::from)
            .or_else(|| home.map(|h| std::path::PathBuf::from(h).join(".config")));
        app.settings_file = config.map(|d| d.join("JetStudio").join("settings.json"));
        app.audio_request = app.load_audio_settings();
        app.audio_pending = app.audio_request.clone();
        app.hardware_audio = true;
        app.open_audio();
        app
    }

    fn load_audio_settings(&self) -> OutputRequest {
        let Some(value) = self
            .settings_file
            .as_ref()
            .and_then(|path| std::fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        else {
            return OutputRequest::default();
        };
        let audio = &value["audio"];
        OutputRequest {
            device: audio["device"].as_str().map(str::to_owned),
            rate: audio["rate"].as_u64().map(|r| r as u32),
            buffer_frames: audio["buffer_frames"].as_u64().map(|f| f as u32),
        }
    }

    fn save_audio_settings(&mut self) {
        let Some(path) = &self.settings_file else {
            return;
        };
        let value = json!({ "audio": {
            "device": self.audio_request.device,
            "rate": self.audio_request.rate,
            "buffer_frames": self.audio_request.buffer_frames,
        }});
        let written = path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(path, value.to_string()));
        if let Err(error) = written {
            self.last_error = Some(format!("Could not save the audio settings: {error}"));
        }
    }

    /// (Re)open the output device with `audio_request`. Audio the old stream may still hold
    /// moves to the new host's graveyard; the musical position carries over.
    fn open_audio(&mut self) {
        if !self.hardware_audio {
            return;
        }
        let graveyard = self
            .audio
            .take()
            .map(|host| {
                let mut graveyard = host.graveyard;
                graveyard.extend(host.preview_hold);
                // Dropping `host` stops its stream before the new one starts.
                graveyard
            })
            .unwrap_or_default();
        let session = &self.session;
        match AudioOutput::open(self.audio_request.clone(), |rate| session.build_plan(rate)) {
            Ok(mut output) => {
                self.session.set_clock(Box::new(output.clock()));
                let info = output.info().clone();
                let midi = output.open_midi_inputs().ok();
                self.audio_status = Some(format!(
                    "MIDI in: {} - {} Hz - {}",
                    midi.as_ref().map_or(0, |m| m.names.len()),
                    info.rate,
                    info.name
                ));
                self.audio_error = None;
                self.audio = Some(AudioHost {
                    output,
                    last_key: None,
                    graveyard,
                    preview_hold: None,
                    scope: vec![0.0; 256],
                    cpu: 0.0,
                    midi,
                });
            }
            Err(error) => {
                self.session
                    .set_clock(Box::new(jet_engine::WallClock::default()));
                self.audio_status = Some(format!("Audio output unavailable: {error}"));
                self.audio_error = Some(error.to_string());
            }
        }
    }

    fn audio_settings_view(&self) -> AudioSettingsView {
        AudioSettingsView {
            devices: self.device_list.clone(),
            pending: (
                self.audio_pending.device.clone(),
                self.audio_pending.rate,
                self.audio_pending.buffer_frames,
            ),
            running: match &self.audio {
                Some(host) => Ok(host.output.info().clone()),
                None => Err(self
                    .audio_error
                    .clone()
                    .unwrap_or_else(|| "not opened".to_owned())),
            },
            callbacks: self.audio.as_ref().map_or(0, |h| {
                h.output
                    .stats()
                    .callbacks
                    .load(std::sync::atomic::Ordering::Relaxed)
            }),
            late: self.audio.as_ref().map_or(0, |h| {
                h.output
                    .stats()
                    .overruns
                    .load(std::sync::atomic::Ordering::Relaxed)
            }),
            load: self.audio.as_ref().map_or(0.0, |h| h.cpu),
        }
    }

    fn run_audio_settings(&mut self, action: AudioSettingsAction) {
        match action {
            AudioSettingsAction::Device(device) => self.audio_pending.device = device,
            AudioSettingsAction::Rate(rate) => self.audio_pending.rate = rate,
            AudioSettingsAction::Buffer(frames) => self.audio_pending.buffer_frames = frames,
            AudioSettingsAction::Refresh if self.hardware_audio => {
                self.device_list = jet_playback::output_devices();
            }
            AudioSettingsAction::Refresh => {}
            AudioSettingsAction::Apply => {
                self.audio_request = self.audio_pending.clone();
                self.open_audio();
                self.save_audio_settings();
            }
        }
    }

    /// Mirror the session into the audio thread: plans, transport, panic, garbage.
    fn pump_audio(&mut self) {
        let Some(host) = &mut self.audio else {
            return;
        };
        host.output.collect();
        host.graveyard
            .extend(self.session.samples_mut().take_retired());
        host.graveyard.retain(|data| Arc::strong_count(data) > 1);
        let key = self.session.plan_key();
        let sync = self.session.transport_sync();
        if host.last_key != Some(key) {
            let plan = Box::new(self.session.build_plan(host.output.rate()));
            if host.output.send_plan(plan, sync).is_ok() {
                host.last_key = Some(key);
            }
        } else {
            host.output.sync(sync);
        }
        if self.session.take_panic() {
            host.output.panic();
        }
        // MIDI controllers play the selected channel [Manual: channelrack, "The selected
        // Channel also receives MIDI/Controller input"].
        if let Some(midi) = &host.midi {
            let target = self
                .rack
                .selected_channel
                .or(self.session.selected_channel().map(|c| c.0));
            midi.set_target(target);
        }
        let (load, _) = host.output.stats().take_load();
        host.cpu = host.cpu * 0.8 + load * 0.2;
        let meters = Arc::clone(host.output.meters());
        meters.read_scope(&mut host.scope);
    }

    fn selected_channel_id(&self) -> Option<u64> {
        self.rack
            .selected_channel
            .or(self.session.selected_channel().map(|c| c.0))
            .filter(|id| self.session.project().channel(ChannelId(*id)).is_some())
    }

    fn preview_file(&mut self, path: &std::path::Path) {
        let Some(host) = &mut self.audio else {
            return;
        };
        match jet_media::decode_file(path) {
            Ok(data) => {
                let data = Arc::new(data);
                host.output.preview(Arc::clone(&data));
                if let Some(old) = host.preview_hold.replace(data) {
                    host.graveyard.push(old);
                }
            }
            Err(error) => self.last_error = Some(error.to_string()),
        }
    }

    fn gesture(&mut self, key: &str, id: &str, params: Value) {
        match self.session.execute_gesture(key, id, params) {
            Ok(_) => self.last_error = None,
            Err(error) => self.last_error = Some(error.to_string()),
        }
    }

    fn run_rack(&mut self, action: RackAction) {
        match action {
            RackAction::AddChannel => {
                self.execute("channel.add", json!({"name": "Sampler"}));
                self.rack.selected_channel = self.session.selected_channel().map(|c| c.0);
            }
            RackAction::SetStep {
                pattern,
                channel,
                index,
                on,
                gesture,
            } => {
                let params =
                    json!({"pattern": pattern, "channel": channel, "index": index, "on": on});
                if gesture {
                    self.gesture(&format!("rack.paint.{channel}"), "step.set", params);
                } else {
                    self.execute("step.set", params);
                }
            }
            RackAction::SelectChannel(id) => {
                self.rack.selected_channel = Some(id);
                self.execute("channel.select", json!({"id": id}));
            }
            // "Left-click - Opens/Closes the Channel Settings window" [Manual: channelrack, §9].
            RackAction::OpenSettings(id) => {
                let was = self.selected_channel_id();
                self.rack.selected_channel = Some(id);
                self.execute("channel.select", json!({"id": id}));
                let visible = self
                    .workspace
                    .as_ref()
                    .and_then(|ws| ws.window(WindowKind::ChannelSettings))
                    .is_some_and(|w| w.visible);
                if !visible || was != Some(id) {
                    if visible {
                        self.pending_focus = Some(WindowKind::ChannelSettings);
                    } else {
                        self.pending_show.push(WindowKind::ChannelSettings);
                    }
                } else if let Some(ws) = &mut self.workspace {
                    let desktop = ws
                        .window(WindowKind::ChannelSettings)
                        .map_or(Rect::NOTHING, |w| w.rect);
                    let _ = ws.apply(
                        WsAction::Close(WindowKind::ChannelSettings),
                        desktop,
                        self.layout.snap_distance,
                    );
                }
            }
            RackAction::SetMute { channel, muted } => {
                self.execute("channel.mute.set", json!({"id": channel, "mute": muted}));
            }
            RackAction::Solo(id) => self.execute("channel.solo", json!({"id": id})),
            RackAction::SetVolume { channel, value } => self.gesture(
                &format!("rack.volume.{channel}"),
                "channel.volume.set",
                json!({"id": channel, "value": value}),
            ),
            RackAction::SetPan { channel, value } => self.gesture(
                &format!("rack.pan.{channel}"),
                "channel.pan.set",
                json!({"id": channel, "value": value}),
            ),
            RackAction::SetMixer { channel, value } => self.gesture(
                &format!("rack.mixer.{channel}"),
                "channel.mixer_target.set",
                json!({"id": channel, "value": value}),
            ),
            RackAction::EndGesture => self.session.end_gesture(),
            RackAction::Rename(id, name) => {
                self.execute("channel.rename", json!({"id": id, "name": name}));
            }
            RackAction::DropSample { channel, path } => {
                let id = match channel {
                    Some(id) => Some(id),
                    None => {
                        let name = path.file_stem().map_or_else(
                            || "Sampler".to_owned(),
                            |s| s.to_string_lossy().into_owned(),
                        );
                        self.execute("channel.add", json!({"name": name}));
                        self.session.selected_channel().map(|c| c.0)
                    }
                };
                if let Some(id) = id {
                    self.execute("channel.sample.load", json!({"id": id, "path": path}));
                    self.rack.selected_channel = Some(id);
                }
            }
            RackAction::Menu(id, item) => self.rack_menu(id, item),
            RackAction::CreateAutomation(channel, param) => {
                let target = match param {
                    crate::channel_rack::KnobParam::Volume => json!({"ChannelVolume": channel}),
                    crate::channel_rack::KnobParam::Pan => json!({"ChannelPan": channel}),
                };
                self.execute("automation.create", json!({ "target": target }));
            }
            RackAction::SetSwing(value) => {
                self.gesture("rack.swing", "project.swing.set", json!({"value": value}));
            }
            RackAction::SetPatternLength(steps) => {
                let id = self.session.selected_pattern().0;
                match steps {
                    Some(steps) => self.gesture(
                        "rack.pattern-length",
                        "pattern.length.set",
                        json!({"id": id, "steps": steps}),
                    ),
                    None => self.execute("pattern.length.auto", json!({"id": id})),
                }
            }
        }
    }

    fn rack_menu(&mut self, id: u64, item: RackMenu) {
        let pattern = self.session.selected_pattern().0;
        match item {
            RackMenu::PianoRoll => {
                self.rack.selected_channel = Some(id);
                self.execute("channel.select", json!({"id": id}));
                if let Some(ws) = &mut self.workspace
                    && !ws.window(WindowKind::PianoRoll).is_some_and(|w| w.visible)
                {
                    self.pending_show.push(WindowKind::PianoRoll);
                }
            }
            RackMenu::LoadSample => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Audio", &jet_media::SUPPORTED_EXTENSIONS)
                    .pick_file()
                {
                    self.execute("channel.sample.load", json!({"id": id, "path": path}));
                }
            }
            RackMenu::CutItself => {
                let project = self.session.project();
                let Some(channel) = project.channel(ChannelId(id)) else {
                    return;
                };
                if channel.cut != 0 && channel.cut == channel.cut_by {
                    self.execute("channel.cut.set", json!({"id": id, "cut": 0, "cut_by": 0}));
                } else {
                    // "setting the Cut/Cut By values to the same number" [Manual: chansettings_misc];
                    // we pick the lowest group no other channel uses.
                    let used: std::collections::BTreeSet<u8> = project
                        .channels
                        .iter()
                        .filter(|c| c.id != ChannelId(id))
                        .flat_map(|c| [c.cut, c.cut_by])
                        .collect();
                    if let Some(group) = (1..=u8::MAX).find(|g| !used.contains(g)) {
                        self.execute(
                            "channel.cut.set",
                            json!({"id": id, "cut": group, "cut_by": group}),
                        );
                    }
                }
            }
            RackMenu::Clone => self.execute("channel.clone", json!({"id": id})),
            RackMenu::Delete => self.execute("channel.remove", json!({"id": id})),
            RackMenu::Fill(every) => {
                self.execute(
                    "step.fill",
                    json!({"pattern": pattern, "channel": id, "every": every}),
                );
            }
            RackMenu::ClearSteps => {
                self.execute("step.clear", json!({"pattern": pattern, "channel": id}));
            }
            RackMenu::FreeMixer => self.execute("channel.mixer_target.free", json!({"id": id})),
            RackMenu::MakeSampler => {
                self.execute("channel.kind.set", json!({"id": id, "kind": "sampler"}));
            }
            RackMenu::MakeSynth => {
                self.execute("channel.kind.set", json!({"id": id, "kind": "synth"}));
            }
            RackMenu::Rename => {}
        }
    }

    fn run_piano(&mut self, action: PianoAction) {
        let Some(channel) = self.selected_channel_id() else {
            return;
        };
        let pattern = self.session.selected_pattern().0;
        let base = |extra: Value| {
            let mut params = json!({"pattern": pattern, "channel": channel});
            if let (Some(map), Value::Object(more)) = (params.as_object_mut(), extra) {
                map.extend(more);
            }
            params
        };
        match action {
            PianoAction::Add { start, length, key } => {
                self.execute(
                    "note.add",
                    base(json!({"start": start, "length": length, "key": key})),
                );
            }
            PianoAction::Delete(indices) => {
                self.execute("note.delete", base(json!({"indices": indices})))
            }
            PianoAction::Move { indices, dt, dkey } => {
                self.execute(
                    "note.move",
                    base(json!({"indices": indices, "dt": dt, "dkey": dkey})),
                );
            }
            PianoAction::Resize { indices, delta } => {
                self.execute(
                    "note.length.set",
                    base(json!({"indices": indices, "delta": delta})),
                );
            }
            PianoAction::Velocity { indices, value } => {
                self.execute(
                    "note.props.set",
                    base(json!({"indices": indices, "velocity": value})),
                );
            }
            PianoAction::Mute { indices, muted } => {
                self.execute(
                    "note.props.set",
                    base(json!({"indices": indices, "muted": muted})),
                );
            }
            PianoAction::Transpose { indices, semitones } => {
                self.execute(
                    "note.transpose",
                    base(json!({"indices": indices, "semitones": semitones})),
                );
            }
            PianoAction::Quantize {
                indices,
                grid,
                starts_only,
            } => self.execute(
                "note.quantize",
                base(json!({"indices": indices, "grid": grid, "starts_only": starts_only})),
            ),
            PianoAction::Duplicate { indices, grid } => {
                self.execute(
                    "note.duplicate",
                    base(json!({"indices": indices, "grid": grid})),
                );
            }
            PianoAction::Chop { indices, grid } => {
                self.execute("note.chop", base(json!({"indices": indices, "grid": grid})));
            }
            PianoAction::Glue(indices) => {
                self.execute("note.glue", base(json!({"indices": indices})))
            }
            PianoAction::DiscardLengths { indices, grid } => {
                self.execute(
                    "note.discard_lengths",
                    base(json!({"indices": indices, "grid": grid})),
                );
            }
            PianoAction::Copy(indices) => {
                self.execute("note.copy", base(json!({"indices": indices})))
            }
            PianoAction::Cut(indices) => {
                self.execute("note.cut", base(json!({"indices": indices})))
            }
            PianoAction::Paste { at } => self.execute("note.paste", base(json!({"at": at}))),
            PianoAction::ImportMidi => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("MIDI file", &["mid", "midi"])
                    .pick_file()
                {
                    self.execute("note.import_mid", base(json!({"path": path})));
                }
            }
            PianoAction::ExportMidi => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("MIDI file", &["mid"])
                    .set_file_name("Piano roll.mid")
                    .save_file()
                {
                    self.execute("note.export_mid", base(json!({"path": path})));
                }
            }
            PianoAction::KeyDown(key) => {
                if let Some(host) = &mut self.audio {
                    host.output
                        .note_on(ChannelId(channel), key, jet_model::DEFAULT_VELOCITY);
                }
            }
            PianoAction::KeyUp(key) => {
                if let Some(host) = &mut self.audio {
                    host.output.note_off(ChannelId(channel), key);
                }
            }
        }
    }

    fn run_mixer(&mut self, action: MixerAction) {
        match action {
            MixerAction::Volume { track, value } => self.gesture(
                &format!("mixer.volume.{track}"),
                "mixer.track.volume.set",
                json!({"track": track, "value": value}),
            ),
            MixerAction::Pan { track, value } => self.gesture(
                &format!("mixer.pan.{track}"),
                "mixer.track.pan.set",
                json!({"track": track, "value": value}),
            ),
            MixerAction::Mute { track, value } => {
                self.execute(
                    "mixer.track.mute.set",
                    json!({"track": track, "value": value}),
                );
            }
            MixerAction::Solo(track) => self.execute("mixer.track.solo", json!({"track": track})),
            MixerAction::ToggleSend { from, to, on } => {
                if on {
                    self.execute("mixer.send.set", json!({"from": from, "to": to}));
                } else {
                    self.execute("mixer.send.remove", json!({"from": from, "to": to}));
                }
            }
            MixerAction::ToMaster { track, value } => {
                self.execute(
                    "mixer.track.to_master.set",
                    json!({"track": track, "value": value}),
                );
            }
            MixerAction::FxAdd { track, kind } => {
                self.execute("mixer.fx.add", json!({"track": track, "kind": kind}));
            }
            MixerAction::FxRemove { track, slot } => {
                self.execute("mixer.fx.remove", json!({"track": track, "slot": slot}));
            }
            MixerAction::FxMove { track, slot, delta } => {
                self.execute(
                    "mixer.fx.move",
                    json!({"track": track, "slot": slot, "delta": delta}),
                );
            }
            MixerAction::FxEnabled { track, slot, value } => self.execute(
                "mixer.fx.enabled.set",
                json!({"track": track, "slot": slot, "value": value}),
            ),
            MixerAction::FxMix { track, slot, value } => self.gesture(
                &format!("mixer.fx-mix.{track}.{slot}"),
                "mixer.fx.mix.set",
                json!({"track": track, "slot": slot, "value": value}),
            ),
            MixerAction::FxSet { track, slot, kind } => self.gesture(
                &format!("mixer.fx.{track}.{slot}"),
                "mixer.fx.set",
                json!({"track": track, "slot": slot, "kind": kind}),
            ),
            MixerAction::FxMaster { track, value } => self.execute(
                "mixer.track.fx_enabled.set",
                json!({"track": track, "value": value}),
            ),
            MixerAction::EndGesture => self.session.end_gesture(),
            MixerAction::CreateAutomation { track, fader } => {
                let target = if fader {
                    json!({ "MixerVolume": track })
                } else {
                    json!({ "MixerPan": track })
                };
                self.execute("automation.create", json!({ "target": target }));
            }
        }
    }

    /// Typing keyboard: key presses become live notes on the selected channel.
    fn typing_keys(&mut self, ctx: &egui::Context) {
        let Some(channel) = self.selected_channel_id() else {
            return;
        };
        let events = ctx.input(|i| i.events.clone());
        for event in events {
            let Event::Key {
                key,
                pressed,
                repeat: false,
                modifiers,
                ..
            } = event
            else {
                continue;
            };
            // Alt+NumPad 2-6 changes the octave [Manual: basics_shortcuts].
            if modifiers.alt && pressed {
                let octave = match key {
                    Key::Num2 => Some(2),
                    Key::Num3 => Some(3),
                    Key::Num4 => Some(4),
                    Key::Num5 => Some(5),
                    Key::Num6 => Some(6),
                    _ => None,
                };
                if let Some(octave) = octave {
                    self.typing_octave = octave;
                    continue;
                }
            }
            if modifiers.any() {
                continue;
            }
            let Some(&(_, offset)) = TYPING_KEYS.iter().find(|(k, _)| *k == key) else {
                continue;
            };
            let note = (u32::from(self.typing_octave) * 12 + u32::from(offset)).min(131) as u8;
            if pressed {
                if !self.typing_held.iter().any(|(k, _)| *k == key) {
                    self.typing_held.push((key, note));
                    if let Some(host) = &mut self.audio {
                        host.output
                            .note_on(ChannelId(channel), note, jet_model::DEFAULT_VELOCITY);
                    }
                }
            } else if let Some(index) = self.typing_held.iter().position(|(k, _)| *k == key) {
                let (_, held) = self.typing_held.remove(index);
                if let Some(host) = &mut self.audio {
                    host.output.note_off(ChannelId(channel), held);
                }
            }
        }
    }

    fn release_typing_keys(&mut self) {
        let Some(channel) = self.selected_channel_id() else {
            self.typing_held.clear();
            return;
        };
        for (_, note) in std::mem::take(&mut self.typing_held) {
            if let Some(host) = &mut self.audio {
                host.output.note_off(ChannelId(channel), note);
            }
        }
    }

    fn export_wav(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Wave file", &["wav"])
            .set_file_name("Untitled.wav")
            .save_file()
        else {
            return;
        };
        let mode = if self.session.transport_snapshot().mode == PlayMode::Song {
            "song"
        } else {
            "pattern"
        };
        // "The output (Mixer) sample rate is set in the Audio Settings" [Manual: fformats_save_export].
        let rate = self
            .audio
            .as_ref()
            .map_or(44_100, |host| host.output.rate());
        self.execute(
            "file.export_wav",
            json!({"path": path, "mode": mode, "bits": 16, "tail": "leave", "rate": rate}),
        );
        if self.last_error.is_none() {
            self.audio_status = Some(format!("Exported {}", path.display()));
        }
    }

    /// Scrolls the Playlist so `bar` (zero-based) is the first visible bar.
    pub fn set_playlist_scroll_bars(&mut self, bar: f32) {
        self.playlist.scroll_bars = bar.max(0.0);
    }

    /// Deterministic frames for screenshots: no animation-driven repaints.
    pub fn set_static_capture(&mut self, on: bool) {
        self.static_capture = on;
    }

    /// Shows a window that a new project keeps closed (used by the screenshot tool).
    pub fn show_window_on_start(&mut self, kind: WindowKind) {
        self.pending_show.push(kind);
    }

    /// Installs the measured fonts. Call before the first frame (eframe creation context).
    pub fn install_fonts(&mut self, ctx: &egui::Context) {
        ctx.set_fonts(theme::fonts());
        self.fonts_installed = true;
    }

    pub fn probe(&self) -> &LayoutProbe {
        &self.probe
    }

    fn run_action(&mut self, action: UiAction, desktop: Rect, ctx: &egui::Context) {
        match action {
            UiAction::Engine(id, params) => self.execute(id, params),
            UiAction::Workspace(action) => {
                if let Some(workspace) = &mut self.workspace
                    && let Err(error) = workspace.apply(action, desktop, self.layout.snap_distance)
                {
                    self.last_error = Some(format!("Workspace: {error:?}"));
                }
            }
            UiAction::Rack(action) => self.run_rack(action),
            UiAction::Piano(action) => self.run_piano(action),
            UiAction::Mixer(action) => self.run_mixer(action),
            UiAction::AudioSettings(action) => self.run_audio_settings(action),
            UiAction::Settings(action) => match action {
                SettingsAction::Engine {
                    id,
                    params,
                    gesture,
                } => match gesture {
                    Some(key) => self.gesture(&key, id, params),
                    None => self.execute(id, params),
                },
                SettingsAction::EndGesture => self.session.end_gesture(),
                SettingsAction::LoadSample(id) => self.rack_menu(id, RackMenu::LoadSample),
            },
            UiAction::ExportWav => self.export_wav(),
            UiAction::ExportMidi => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("MIDI file", &["mid"])
                    .set_file_name("Untitled.mid")
                    .save_file()
                {
                    let mode = if self.session.transport_snapshot().mode == PlayMode::Song {
                        "song"
                    } else {
                        "pattern"
                    };
                    self.execute("file.export_mid", json!({"path": path, "mode": mode}));
                }
            }
            UiAction::ToggleTyping => {
                self.typing = !self.typing;
                if !self.typing {
                    self.release_typing_keys();
                }
            }
            UiAction::AddChannel(kind) => {
                let name = if kind == "synth" { "Synth" } else { "Sampler" };
                self.execute("channel.add", json!({"name": name, "kind": kind}));
                self.rack.selected_channel = self.session.selected_channel().map(|c| c.0);
                if let Some(ws) = &mut self.workspace
                    && !ws
                        .window(WindowKind::ChannelRack)
                        .is_some_and(|w| w.visible)
                {
                    self.pending_show.push(WindowKind::ChannelRack);
                }
            }
            UiAction::Playlist(action) => match action {
                PlaylistAction::AddTrack => self.add_track(),
                PlaylistAction::SelectTrack(id) => self.playlist.selected_track = Some(id),
                PlaylistAction::SelectPattern(id) => {
                    self.execute("pattern.select", json!({"id": id}));
                }
                PlaylistAction::MoveClip { id, row, start } => {
                    while self.track_count() <= row {
                        let before = self.track_count();
                        self.add_track();
                        if self.track_count() == before {
                            return;
                        }
                    }
                    if let Some(track) = self.track_id_at(row) {
                        self.execute(
                            "playlist.clip.move",
                            json!({"id": id, "track": track, "start": start}),
                        );
                    }
                }
                PlaylistAction::ResizeClip { id, length } => {
                    self.execute("playlist.clip.resize", json!({"id": id, "length": length}));
                }
                PlaylistAction::DeleteClip(id) => {
                    self.execute("playlist.clip.delete", json!({"id": id}));
                }
                PlaylistAction::MuteClip { id, muted } => {
                    self.execute("playlist.clip.mute.set", json!({"id": id, "muted": muted}));
                }
                PlaylistAction::MuteTrack { id, muted } => {
                    self.execute("playlist.track.mute.set", json!({"id": id, "mute": muted}));
                }
                PlaylistAction::SoloTrack(id) => {
                    self.execute("playlist.track.solo", json!({"id": id}))
                }
                PlaylistAction::DropAudio { row, start, path } => {
                    while self.track_count() <= row {
                        let before = self.track_count();
                        self.add_track();
                        if self.track_count() == before {
                            return;
                        }
                    }
                    if let Some(track) = self.track_id_at(row) {
                        self.execute(
                            "playlist.audio.add",
                            json!({"path": path, "track": track, "start": start}),
                        );
                    }
                }
                PlaylistAction::AutoAddPoint { id, at, value } => self.execute(
                    "automation.point.add",
                    json!({"id": id, "at": at, "value": value}),
                ),
                PlaylistAction::AutoMovePoint {
                    id,
                    index,
                    at,
                    value,
                } => self.execute(
                    "automation.point.move",
                    json!({"id": id, "index": index, "at": at, "value": value}),
                ),
                PlaylistAction::AutoDeletePoint { id, index } => {
                    self.execute("automation.point.delete", json!({"id": id, "index": index}))
                }
                PlaylistAction::AutoShape { id, index, shape } => {
                    let tension = self
                        .session
                        .project()
                        .automation(jet_model::AutomationId(id))
                        .and_then(|a| a.points.get(index))
                        .map_or(0, |p| p.tension);
                    self.execute(
                        "automation.point.shape.set",
                        json!({"id": id, "index": index, "shape": shape, "tension": tension}),
                    );
                }
                PlaylistAction::Seek(tick) => {
                    self.execute("transport.position.set", json!({"tick": tick}));
                }
                PlaylistAction::AddClipOnRow {
                    row,
                    pattern,
                    start,
                    length,
                } => {
                    // Display rows beyond the model's tracks are FL's empty "Track N" rows;
                    // placing a clip there first creates the real tracks up to that row.
                    while self.track_count() <= row {
                        let before = self.track_count();
                        self.add_track();
                        if self.track_count() == before {
                            return;
                        }
                    }
                    if let Some(track) = self.track_id_at(row) {
                        self.execute(
                            "playlist.clip.add",
                            json!({"track": track, "pattern": pattern, "start": start, "length": length}),
                        );
                    }
                }
            },
            UiAction::OpenProject => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("JetStudio project", &["jetproj"])
                    .pick_file()
                {
                    self.execute("file.open", json!({"path": path}));
                }
            }
            UiAction::SaveProject => {
                if self.session.path().is_some() {
                    self.execute("file.save", json!({}));
                } else {
                    self.save_project_as();
                }
            }
            UiAction::SaveProjectAs => self.save_project_as(),
            UiAction::Minimize => ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true)),
            UiAction::Maximize => {
                let is_maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!is_maximized));
            }
            UiAction::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
        }
    }

    fn add_track(&mut self) {
        let name = format!("Track {}", self.track_count() + 1);
        self.execute("playlist.track.add", json!({"name": name}));
    }

    fn active_arrangement(&self) -> Option<&jet_model::Arrangement> {
        let project = self.session.project();
        project
            .arrangements
            .iter()
            .find(|a| a.id == project.active_arrangement)
    }

    fn track_count(&self) -> usize {
        self.active_arrangement().map_or(0, |a| a.tracks.len())
    }

    fn track_id_at(&self, row: usize) -> Option<u64> {
        self.active_arrangement()
            .and_then(|a| a.tracks.get(row))
            .map(|t| t.id.0)
    }

    fn execute(&mut self, id: &str, params: Value) {
        if let Err(error) = self.session.execute(id, params) {
            self.last_error = Some(error.to_string());
        } else {
            self.last_error = None;
        }
    }

    fn save_project_as(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("JetStudio project", &["jetproj"])
            .set_file_name("Untitled.jetproj")
            .save_file()
        {
            self.execute("file.save_as", json!({"path": path}));
        }
    }

    fn key_actions(&mut self, ctx: &egui::Context) -> Vec<UiAction> {
        let mut actions = Vec::new();
        let pressed = |key: Key| ctx.input(|i| i.key_pressed(key));
        let modifiers = ctx.input(|i| i.modifiers);
        if pressed(Key::F11) && modifiers.ctrl {
            self.toolbar.hidden = !self.toolbar.hidden;
        }
        if pressed(Key::F5) {
            actions.push(UiAction::Workspace(WsAction::Toggle(WindowKind::Playlist)));
        }
        if pressed(Key::F6) {
            actions.push(UiAction::Workspace(WsAction::Toggle(
                WindowKind::ChannelRack,
            )));
        }
        if pressed(Key::F7) {
            actions.push(UiAction::Workspace(WsAction::Toggle(WindowKind::PianoRoll)));
        }
        if pressed(Key::F9) {
            actions.push(UiAction::Workspace(WsAction::Toggle(WindowKind::Mixer)));
        }
        // F10 opens the settings [Manual: envsettings_audio, "press F10 to open the settings panel"].
        if pressed(Key::F10) {
            if self.device_list.is_empty() && self.hardware_audio {
                self.device_list = jet_playback::output_devices();
            }
            actions.push(UiAction::Workspace(WsAction::Toggle(
                WindowKind::AudioSettings,
            )));
        }
        if pressed(Key::F8) && modifiers.alt {
            actions.push(UiAction::Workspace(WsAction::ToggleBrowser));
        }
        if pressed(Key::F12) {
            actions.push(UiAction::Workspace(WsAction::CloseAll));
        }
        if pressed(Key::Tab) {
            actions.push(UiAction::Workspace(WsAction::CycleNested));
        }
        if pressed(Key::H) && modifiers.ctrl && modifiers.shift {
            actions.push(UiAction::Workspace(WsAction::ArrangeDefault));
        }
        if self.toolbar.tempo_edit.is_some() || self.rack.rename.is_some() {
            return actions;
        }
        if pressed(Key::T) && modifiers.ctrl {
            actions.push(UiAction::ToggleTyping);
        }
        if pressed(Key::M) && modifiers.ctrl {
            actions.push(UiAction::Engine("transport.metronome.toggle", json!({})));
        }
        // "Single key shortcuts will only work when 'Typing keyboard to piano' is OFF"
        // [Manual: basics_shortcuts]; letter and number shortcuts yield to notes.
        let letters = !self.typing;
        if self.typing {
            self.typing_keys(ctx);
        }
        if pressed(Key::R) && modifiers.ctrl {
            actions.push(UiAction::ExportWav);
        }
        if pressed(Key::L)
            && modifiers.ctrl
            && let Some(id) = self.selected_channel_id()
        {
            actions.push(UiAction::Engine(
                "channel.mixer_target.free",
                json!({"id": id}),
            ));
        }
        if pressed(Key::O) && modifiers.ctrl {
            actions.push(UiAction::OpenProject);
        }
        if pressed(Key::S) && modifiers.ctrl {
            actions.push(if modifiers.shift {
                UiAction::SaveProjectAs
            } else {
                UiAction::SaveProject
            });
        }
        if pressed(Key::Enter) && !modifiers.any() {
            let maximized = self
                .workspace
                .as_ref()
                .and_then(|ws| ws.window(WindowKind::Playlist))
                .is_some_and(|w| w.maximized);
            actions.push(UiAction::Workspace(if maximized {
                WsAction::Restore(WindowKind::Playlist)
            } else {
                WsAction::Maximize(WindowKind::Playlist)
            }));
        }
        if letters && pressed(Key::L) && !modifiers.any() {
            actions.push(UiAction::Engine("transport.mode.toggle", json!({})));
        }
        if pressed(Key::Space) {
            actions.push(UiAction::Engine(
                if modifiers.ctrl {
                    "transport.pause"
                } else {
                    "transport.play_stop"
                },
                json!({}),
            ));
        }
        if pressed(Key::Z) && modifiers.ctrl {
            actions.push(UiAction::Engine(
                if modifiers.alt {
                    "edit.undo_step"
                } else {
                    "edit.undo_toggle"
                },
                json!({}),
            ));
        }
        if pressed(Key::F4) {
            actions.push(UiAction::Engine("pattern.next_empty", json!({})));
        }
        if letters && pressed(Key::Plus) && !modifiers.any() {
            actions.push(UiAction::Engine("pattern.next", json!({})));
        }
        if letters && pressed(Key::Minus) && !modifiers.any() {
            actions.push(UiAction::Engine("pattern.prev", json!({})));
        }
        if pressed(Key::Escape)
            && self.toolbar.open_menu.take().is_none()
            && let Some(kind) = self.workspace.as_ref().and_then(|ws| ws.focused)
        {
            actions.push(UiAction::Workspace(WsAction::Close(kind)));
        }
        actions
    }

    fn selected_pattern(&self) -> Option<&jet_model::Pattern> {
        let project = self.session.project();
        let selected = self.session.selected_pattern();
        project
            .patterns
            .iter()
            .find(|p| p.id == selected)
            .or_else(|| project.patterns.first())
    }

    fn pattern_name(&self) -> String {
        self.selected_pattern()
            .map_or_else(|| "Pattern".to_owned(), |p| p.name.clone())
    }

    fn toolbar_view(&self) -> ToolbarView {
        let snapshot = self.session.transport_snapshot();
        let project = self.session.project();
        let position = jet_time::format_bar_step_tick(
            snapshot.position,
            project.settings.ppq,
            project.settings.time_sig,
            project.settings.steps_per_beat,
        )
        .unwrap_or_else(|_| "--:--:--".to_owned());
        let title = self.session.path().and_then(|p| p.file_stem()).map_or_else(
            || "(untitled)".to_owned(),
            |s| s.to_string_lossy().into_owned(),
        );
        ToolbarView {
            song_mode: snapshot.mode == PlayMode::Song,
            playing: snapshot.state == TransportState::Playing,
            tempo_milli: i64::from(project.settings.tempo.0),
            position,
            pattern_name: self.pattern_name(),
            title,
            error: self.last_error.clone(),
            status: self.audio_status.clone(),
            cpu: self.audio.as_ref().map(|host| host.cpu),
            voices: self
                .audio
                .as_ref()
                .map(|host| host.output.meters().voices()),
            scope: self
                .audio
                .as_ref()
                .map_or_else(Vec::new, |host| host.scope.clone()),
            typing_keyboard: self.typing,
            metronome: self.session.metronome(),
        }
    }

    /// Peak columns for every decoded sample, computed once per resource.
    fn cache_waveforms(&mut self) {
        const COLUMNS: usize = 400;
        for resource in &self.session.project().resources {
            if self.waveforms.contains_key(&resource.id) {
                continue;
            }
            let Some(data) = self.session.samples().get(resource.id) else {
                continue;
            };
            let frames = data.frames().max(1);
            let columns = (0..COLUMNS)
                .map(|c| {
                    let a = c * frames / COLUMNS;
                    let b = ((c + 1) * frames / COLUMNS).max(a + 1).min(frames);
                    data.left[a.min(frames - 1)..b]
                        .iter()
                        .fold((0.0_f32, 0.0_f32), |(lo, hi), v| (lo.min(*v), hi.max(*v)))
                })
                .collect();
            self.waveforms.insert(resource.id, columns);
        }
    }

    fn settings_view(&self) -> Option<ChannelSettingsData> {
        let project = self.session.project();
        let channel = project.channel(ChannelId(self.selected_channel_id()?))?;
        let sample = channel.sample();
        Some(ChannelSettingsData {
            id: channel.id.0,
            name: channel.name.clone(),
            sample_name: sample
                .and_then(|id| project.resources.iter().find(|r| r.id == id))
                .and_then(|r| {
                    std::path::Path::new(&r.path_rel)
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                }),
            waveform: sample
                .and_then(|id| self.waveforms.get(&id).cloned())
                .unwrap_or_default(),
            is_sampler: matches!(channel.kind, ChannelKind::Sampler { .. }),
            volume: channel.volume,
            pan: channel.pan,
            pitch: channel.pitch,
            root_key: channel.root_key,
            swing_mix: channel.swing_mix,
            cut: channel.cut,
            cut_by: channel.cut_by,
            mixer_target: channel.mixer_target,
            poly_max: channel.polyphony.max,
            mono: channel.polyphony.mono,
            sampler: channel.sampler,
            synth: match &channel.kind {
                ChannelKind::Synth(params) => Some(params.clone()),
                ChannelKind::Sampler { .. } | ChannelKind::AudioClip { .. } => None,
            },
        })
    }

    fn piano_view(&self) -> PianoRollView {
        let project = self.session.project();
        let channel = self
            .selected_channel_id()
            .and_then(|id| project.channel(ChannelId(id)))
            .map(|c| (c.id.0, c.name.clone()));
        let notes = channel
            .as_ref()
            .and_then(|(id, _)| {
                self.selected_pattern()
                    .and_then(|p| p.notes.get(&ChannelId(*id)))
            })
            .map(|notes| {
                notes
                    .iter()
                    .map(|n| NoteView {
                        start: n.start.0,
                        length: n.length.0,
                        key: n.key,
                        velocity: n.velocity,
                        muted: n.muted,
                    })
                    .collect()
            })
            .unwrap_or_default();
        let snapshot = self.session.transport_snapshot();
        PianoRollView {
            channel,
            notes,
            step_ticks: project.settings.step_ticks().unwrap_or(24),
            beat_ticks: project.settings.beat_ticks().unwrap_or(96),
            bar_ticks: project.settings.bar_ticks().unwrap_or(384),
            typing: self.typing,
            playhead: (snapshot.mode == PlayMode::Pattern
                && snapshot.state != TransportState::Stopped)
                .then_some(snapshot.position.0),
        }
    }

    fn mixer_view(&self) -> MixerView {
        let project = self.session.project();
        let highest = project
            .channels
            .iter()
            .map(|c| c.mixer_target)
            .chain([project.mixer.inserts.len() as u16, self.mixer.selected])
            .max()
            .unwrap_or(0);
        // Show the used inserts plus a few free ones, at least ten (our choice; FL shows all 125+).
        let count = (highest + 4).clamp(10, jet_model::MAX_INSERTS as u16);
        let meters = self
            .audio
            .as_ref()
            .map(|host| Arc::clone(host.output.meters()));
        let tracks = (0..=count)
            .filter_map(|index| {
                let track = project.mixer.track(index)?;
                Some(MixerTrackView {
                    index,
                    name: track.name.clone(),
                    volume: track.volume,
                    pan: track.pan,
                    mute: track.mute,
                    to_master: track.to_master,
                    sends: track.sends.iter().map(|s| (s.to, s.level)).collect(),
                    fx: track
                        .fx
                        .iter()
                        .map(|slot| (slot.kind.clone(), slot.enabled, slot.mix))
                        .collect(),
                    fx_enabled: track.fx_enabled,
                    peak: meters
                        .as_ref()
                        .map_or((0.0, 0.0), |m| m.take(usize::from(index))),
                })
            })
            .collect();
        MixerView { tracks }
    }

    fn rack_view(&self) -> RackView {
        let project = self.session.project();
        let pattern = self.selected_pattern();
        let steps_per_beat = usize::from(project.settings.steps_per_beat);
        let default_steps = steps_per_beat * usize::from(project.settings.time_sig.num);
        let channels = project
            .channels
            .iter()
            .map(|channel| {
                let sequence = pattern.and_then(|p| p.steps.get(&channel.id));
                let length =
                    sequence.map_or(default_steps, |seq| seq.cells.len().max(default_steps));
                let steps = (0..length)
                    .map(|index| {
                        sequence
                            .and_then(|seq| seq.cells.get(index))
                            .is_some_and(|step| step.on)
                    })
                    .collect();
                let notes = pattern
                    .and_then(|p| p.notes.get(&channel.id))
                    .map(|notes| {
                        notes
                            .iter()
                            .map(|n| (n.start.0, n.length.0, n.key))
                            .collect()
                    })
                    .unwrap_or_default();
                ChannelView {
                    id: channel.id.0,
                    name: channel.name.clone(),
                    color: channel.color,
                    muted: channel.mute,
                    volume: channel.volume,
                    pan: channel.pan,
                    mixer_target: channel.mixer_target,
                    steps,
                    is_sampler: matches!(channel.kind, ChannelKind::Sampler { .. }),
                    notes,
                    cut_itself: channel.cut != 0 && channel.cut == channel.cut_by,
                }
            })
            .collect();
        RackView {
            pattern_id: self.session.selected_pattern().0,
            steps_per_beat,
            step_ticks: project.settings.step_ticks().unwrap_or(24),
            channels,
            dragging_file: self.browser.dragging.clone(),
            swing: project.settings.swing,
            length_steps: pattern.and_then(|p| match p.length {
                jet_model::PatternLength::Steps(steps) => Some(steps),
                jet_model::PatternLength::Auto => None,
            }),
            effective_steps: pattern.map_or(16, |p| {
                (jet_seq::pattern_length(project, p)
                    / project.settings.step_ticks().unwrap_or(24).max(1))
                .clamp(1, 512) as u16
            }),
        }
    }

    fn playlist_view(&self) -> PlaylistView {
        let project = self.session.project();
        let snapshot = self.session.transport_snapshot();
        let arrangement = self.active_arrangement();
        let tracks = arrangement.map_or_else(Vec::new, |a| {
            a.tracks
                .iter()
                .map(|track| TrackView {
                    id: track.id.0,
                    name: track.name.clone(),
                    muted: track.mute,
                })
                .collect()
        });
        let clips = arrangement.map_or_else(Vec::new, |a| {
            a.clips
                .iter()
                .map(|clip| {
                    let (label, color, automation) = match clip.source {
                        ClipSource::Pattern(pattern_id) => {
                            let pattern = project.patterns.iter().find(|p| p.id == pattern_id);
                            (
                                pattern.map_or_else(|| "Pattern".to_owned(), |p| p.name.clone()),
                                pattern.and_then(|p| p.color),
                                None,
                            )
                        }
                        ClipSource::Automation(id) => {
                            let auto = project.automation(id);
                            (
                                auto.map_or_else(|| "Automation".to_owned(), |a| a.name.clone()),
                                // Automation Clips get their own colour (ours; UNMEASURED).
                                Some(0x5A_7A8C),
                                auto.map(|a| crate::playlist::AutomationView {
                                    id: a.id.0,
                                    points: a.points.clone(),
                                    offset: clip.offset.0,
                                }),
                            )
                        }
                        ClipSource::Audio(channel) => {
                            let channel = project.channel(channel);
                            (
                                channel.map_or_else(|| "Audio".to_owned(), |c| c.name.clone()),
                                // Audio Clips get their own colour (ours; UNMEASURED).
                                channel.and_then(|c| c.color).or(Some(0x3F_7A5A)),
                                None,
                            )
                        }
                    };
                    let waveform = match clip.source {
                        ClipSource::Audio(channel) => project
                            .channel(channel)
                            .and_then(|c| c.sample())
                            .and_then(|id| {
                                let columns = self.waveforms.get(&id)?.clone();
                                let data = self.session.samples().get(id)?;
                                let seconds = data.frames() as f64 / f64::from(data.rate.max(1));
                                let ticks = seconds * f64::from(project.settings.tempo.0)
                                    / 60_000.0
                                    * f64::from(project.settings.ppq.0);
                                Some((columns, ticks, clip.offset.0))
                            }),
                        _ => None,
                    };
                    ClipView {
                        id: clip.id.0,
                        track: clip.track.0,
                        start: clip.start.0,
                        length: clip.length.0,
                        label,
                        color,
                        automation,
                        waveform,
                        muted: clip.muted,
                    }
                })
                .collect()
        });
        let ticks_per_bar =
            i64::from(project.settings.ppq.0) * 4 * i64::from(project.settings.time_sig.num)
                / i64::from(project.settings.time_sig.den);
        // In Song mode the position marker is always shown [Manual: playlist, "Play Position Marker"].
        let song_running = snapshot.mode == PlayMode::Song;
        PlaylistView {
            pattern_id: self.session.selected_pattern().0,
            arrangement_name: arrangement
                .map_or_else(|| "Arrangement".to_owned(), |a| a.name.clone()),
            ticks_per_bar,
            beats_per_bar: u32::from(project.settings.time_sig.num),
            position: song_running.then_some(snapshot.position.0),
            tracks,
            clips,
            patterns: project
                .patterns
                .iter()
                .map(|p| PatternEntry {
                    id: p.id.0,
                    name: p.name.clone(),
                    length: jet_seq::pattern_length(project, p),
                })
                .collect(),
            dragging_file: self.browser.dragging.clone(),
        }
    }
}

impl JetApp {
    /// Draw one frame against egui's root UI; also used by headless interaction tests.
    pub fn ui_pass(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if !self.fonts_installed {
            // Named font families only exist from the next pass on; painting with them now
            // would panic inside epaint, so this pass only installs them.
            self.install_fonts(&ctx);
            ctx.request_repaint();
            return;
        }
        self.pump_audio();
        self.probe.begin_frame();
        self.hint.clear();
        // Paint edge to edge: the reference has no margin around the FL window.
        let available = ctx.content_rect();
        ui.set_clip_rect(available);
        let toolbar_h = if self.toolbar.hidden {
            0.0
        } else {
            self.layout.toolbar_h
        };
        let toolbar_rect = Rect::from_min_size(available.min, vec2(available.width(), toolbar_h));
        let base_desktop = Rect::from_min_max(
            pos2(available.left(), available.top() + toolbar_h),
            available.max,
        );
        let side = self.layout.browser_w + self.layout.splitter_w;
        if self.workspace.is_none() {
            let initial = Rect::from_min_max(
                pos2(base_desktop.left() + side, base_desktop.top()),
                base_desktop.max,
            );
            self.workspace = Some(Workspace::new(initial, self.layout.browser_w));
        }
        let browser_visible = self.workspace.as_ref().is_some_and(|ws| ws.browser_visible);
        let browser_rect = Rect::from_min_size(
            base_desktop.min,
            vec2(self.layout.browser_w, base_desktop.height()),
        );
        let desktop = Rect::from_min_max(
            pos2(
                base_desktop.left() + if browser_visible { side } else { 0.0 },
                base_desktop.top(),
            ),
            base_desktop.max,
        );
        for kind in std::mem::take(&mut self.pending_show) {
            if let Some(workspace) = &mut self.workspace {
                let _ = workspace.apply(WsAction::Toggle(kind), desktop, self.layout.snap_distance);
            }
        }
        if let Some(kind) = self.pending_focus.take()
            && let Some(workspace) = &mut self.workspace
        {
            let _ = workspace.apply(WsAction::Focus(kind), desktop, self.layout.snap_distance);
        }
        self.cache_waveforms();
        let settings_view = self.settings_view();
        let audio_settings_view = self.audio_settings_view();
        let toolbar_view = self.toolbar_view();
        let rack_view = self.rack_view();
        let playlist_view = self.playlist_view();
        let piano_view = self.piano_view();
        let mixer_view = self.mixer_view();
        let windows = self
            .workspace
            .as_ref()
            .map_or_else(Vec::new, |ws| ws.windows.clone());
        let mut actions = self.key_actions(&ctx);
        let focused = self.workspace.as_ref().and_then(|ws| ws.focused);

        {
            ui.painter()
                .rect_filled(available, 0.0, self.tokens.desktop);
            let mut paint = PaintCtx {
                ui,
                tokens: &self.tokens,
                layout: &self.layout,
                probe: &mut self.probe,
                hint: &mut self.hint,
            };
            if browser_visible {
                paint_browser(&mut paint, &mut self.browser, browser_rect);
                paint_splitter(
                    &mut paint,
                    Rect::from_min_max(
                        pos2(browser_rect.right(), base_desktop.top()),
                        pos2(desktop.left(), base_desktop.bottom()),
                    ),
                );
            }
            if let Some(pointer) = paint.ui.input(|i| {
                if i.pointer.any_pressed() {
                    i.pointer.interact_pos()
                } else {
                    None
                }
            }) && let Some(win) = windows
                .iter()
                .rev()
                .find(|win| win.visible && effective_rect(win, desktop).contains(pointer))
                && focused != Some(win.kind)
            {
                actions.push(UiAction::Workspace(WsAction::Focus(win.kind)));
            }
            let mut surfaces = Surfaces {
                rack_state: &mut self.rack,
                rack_view: &rack_view,
                playlist_state: &mut self.playlist,
                playlist_view: &playlist_view,
                piano_state: &mut self.piano,
                piano_view: &piano_view,
                mixer_state: &mut self.mixer,
                mixer_view: &mixer_view,
                settings_view: settings_view.as_ref(),
                audio_settings_view: &audio_settings_view,
                focused,
            };
            for win in windows.iter().filter(|win| win.visible) {
                paint_window(
                    &mut paint,
                    win,
                    effective_rect(win, desktop),
                    &mut surfaces,
                    &mut actions,
                );
            }
            if !self.toolbar.hidden {
                actions.extend(
                    paint_toolbar(&mut paint, &mut self.toolbar, &toolbar_view, toolbar_rect)
                        .into_iter()
                        .map(UiAction::from),
                );
            }
        }

        // A Browser drag paints its label at the pointer and ends when the button is released.
        if let Some(path) = &self.browser.dragging {
            if let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) {
                let painter = ctx.layer_painter(egui::LayerId::new(
                    egui::Order::Tooltip,
                    Id::new("browser.drag"),
                ));
                painter.text(
                    pos + vec2(14.0, 4.0),
                    egui::Align2::LEFT_CENTER,
                    path.file_name()
                        .map_or_else(String::new, |n| n.to_string_lossy().into_owned()),
                    theme::regular(14.0),
                    self.tokens.menu_text,
                );
            }
            if !ctx.input(|i| i.pointer.any_down()) {
                self.browser.dragging = None;
            }
        }
        if let Some(path) = self.browser.preview.take() {
            self.preview_file(&path);
        }
        for action in actions {
            self.run_action(action, desktop, &ctx);
        }
        self.pump_audio();
        // Autosave every 5 minutes "UNLESS FL Studio is playing, then the autosave is postponed
        // until the project is stopped" [Manual: envsettings_files, "Regularly"].
        if self.last_autosave.elapsed() >= Duration::from_secs(300) && !toolbar_view.playing {
            self.last_autosave = std::time::Instant::now();
            if self.session.needs_backup()
                && let Err(error) = self.session.execute("file.autosave", json!({}))
            {
                self.last_error = Some(error.to_string());
            }
        }
        if (toolbar_view.playing || self.audio.is_some()) && !self.static_capture {
            ctx.request_repaint_after(Duration::from_millis(30));
        }
    }
}

impl eframe::App for JetApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.ui_pass(ui);
    }
}

impl From<ToolbarAction> for UiAction {
    fn from(action: ToolbarAction) -> Self {
        match action {
            ToolbarAction::Engine(id, params) => Self::Engine(id, params),
            ToolbarAction::Workspace(action) => Self::Workspace(action),
            ToolbarAction::OpenProject => Self::OpenProject,
            ToolbarAction::SaveProject => Self::SaveProject,
            ToolbarAction::SaveProjectAs => Self::SaveProjectAs,
            ToolbarAction::ExportWav => Self::ExportWav,
            ToolbarAction::ExportMidi => Self::ExportMidi,
            ToolbarAction::AddChannel(kind) => Self::AddChannel(kind),
            ToolbarAction::ToggleTyping => Self::ToggleTyping,
            ToolbarAction::Minimize => Self::Minimize,
            ToolbarAction::Maximize => Self::Maximize,
            ToolbarAction::Quit => Self::Quit,
        }
    }
}

struct Surfaces<'a> {
    rack_state: &'a mut RackState,
    rack_view: &'a RackView,
    playlist_state: &'a mut PlaylistState,
    playlist_view: &'a PlaylistView,
    piano_state: &'a mut PianoRollState,
    piano_view: &'a PianoRollView,
    mixer_state: &'a mut MixerState,
    mixer_view: &'a MixerView,
    settings_view: Option<&'a ChannelSettingsData>,
    audio_settings_view: &'a AudioSettingsView,
    focused: Option<WindowKind>,
}

/// A maximized window always fills the current desktop (it follows the Browser being toggled).
fn effective_rect(win: &crate::workspace::WinState, desktop: Rect) -> Rect {
    if win.maximized { desktop } else { win.rect }
}

/// Browser/desktop splitter [W x 490-499]: dark edges around a 6 px face.
fn paint_splitter(paint: &mut PaintCtx<'_>, rect: Rect) {
    let t = *paint.tokens;
    let painter = paint.painter();
    painter.rect_filled(rect, 0.0, t.splitter);
    for x in [
        rect.left(),
        rect.left() + 1.0,
        rect.right() - 2.0,
        rect.right() - 1.0,
    ] {
        painter.rect_filled(
            Rect::from_min_max(pos2(x, rect.top()), pos2(x + 1.0, rect.bottom())),
            0.0,
            t.splitter_edge,
        );
    }
}

fn paint_window(
    paint: &mut PaintCtx<'_>,
    win: &crate::workspace::WinState,
    rect: Rect,
    surfaces: &mut Surfaces<'_>,
    actions: &mut Vec<UiAction>,
) {
    let t = *paint.tokens;
    let title_h = paint.layout.title_h;
    let painter = paint.painter();
    painter.rect_filled(rect, 0.0, t.title_bg);
    let title = Rect::from_min_size(rect.min, vec2(rect.width(), title_h));

    // Drag area: the title bar minus the tool icons and the window buttons.
    let drag_left = match win.kind {
        WindowKind::Playlist => 250.0,
        WindowKind::ChannelRack
        | WindowKind::PianoRoll
        | WindowKind::Mixer
        | WindowKind::ChannelSettings
        | WindowKind::AudioSettings => 22.0,
    };
    let title_drag = Rect::from_min_max(
        pos2(title.left() + drag_left, title.top()),
        pos2(title.right() - 62.0, title.bottom()),
    );
    let response = paint.ui.interact(
        title_drag,
        Id::new(("window.title", win.kind.id())),
        Sense::click_and_drag(),
    );
    paint.probe.insert(
        format!("{}.titlebar", win.kind.id()),
        title_drag,
        "titlebar",
        true,
        Some(win.kind.title().to_owned()),
    );
    if response.dragged() {
        let delta = paint.ui.input(|i| i.pointer.delta());
        let snap = !paint.ui.input(|i| i.modifiers.alt);
        actions.push(UiAction::Workspace(WsAction::Move {
            kind: win.kind,
            to: win.rect.min + delta,
            snap,
        }));
    }
    if response.clicked() {
        actions.push(UiAction::Workspace(WsAction::Focus(win.kind)));
    }
    if response.double_clicked() {
        actions.push(UiAction::Workspace(if win.maximized {
            WsAction::Restore(win.kind)
        } else {
            WsAction::Maximize(win.kind)
        }));
    }

    match win.kind {
        WindowKind::Playlist => paint_title_tools(paint, title, surfaces.playlist_view),
        WindowKind::ChannelRack => {
            icons::chevron_right(
                &painter,
                Rect::from_center_size(
                    pos2(title.left() + 12.0, title.center().y),
                    vec2(10.0, 12.0),
                ),
                t.title_icon,
            );
            icons::title_speaker(
                &painter,
                Rect::from_center_size(
                    pos2(title.left() + 32.0, title.center().y),
                    vec2(18.0, 18.0),
                ),
                t.title_icon,
            );
            painter.text(
                pos2(title.left() + 44.0, title.center().y),
                egui::Align2::LEFT_CENTER,
                "Channel rack",
                theme::regular(paint.layout.font_title),
                t.title_text,
            );
            actions.extend(
                crate::channel_rack::paint_rack_header(paint, title, surfaces.rack_view)
                    .into_iter()
                    .map(UiAction::Rack),
            );
        }
        WindowKind::PianoRoll
        | WindowKind::Mixer
        | WindowKind::ChannelSettings
        | WindowKind::AudioSettings => {
            let label = if win.kind == WindowKind::AudioSettings {
                "Settings - Audio".to_owned()
            } else if win.kind == WindowKind::ChannelSettings {
                surfaces
                    .settings_view
                    .map_or_else(|| "Channel settings".to_owned(), |c| c.name.clone())
            } else if win.kind == WindowKind::PianoRoll {
                let target = surfaces
                    .piano_view
                    .channel
                    .as_ref()
                    .map_or_else(String::new, |(_, name)| format!(" - {name}"));
                format!("Piano roll{target}")
            } else {
                "Mixer".to_owned()
            };
            painter.text(
                pos2(title.left() + 22.0, title.center().y),
                egui::Align2::LEFT_CENTER,
                label,
                theme::regular(paint.layout.font_title),
                t.title_text,
            );
        }
    }

    // Window buttons [W Playlist: minimize 1510-1520, restore 1528-1541, close 1545-1556].
    let max_glyph = if win.maximized {
        WindowGlyph::Restore
    } else {
        WindowGlyph::Maximize
    };
    for (glyph, name, from_right, hint) in [
        (WindowGlyph::Minimize, "minimize", 53.0, "Minimize window"),
        (max_glyph, "maximize", 35.0, "Maximize/restore window"),
        (WindowGlyph::Close, "close", 17.0, "Close window"),
    ] {
        let cell = Rect::from_center_size(
            pos2(title.right() - from_right, title.center().y),
            vec2(17.0, 18.0),
        );
        if paint
            .window_button(
                &format!("{}.{name}", win.kind.id()),
                cell,
                glyph,
                false,
                hint,
            )
            .clicked()
        {
            actions.push(UiAction::Workspace(match name {
                "maximize" if win.maximized => WsAction::Restore(win.kind),
                "maximize" => WsAction::Maximize(win.kind),
                // UNMEASURED: FL's in-desktop minimize state is not modelled yet; it hides.
                _ => WsAction::Close(win.kind),
            }));
        }
    }

    match win.kind {
        WindowKind::ChannelRack => {
            let content = Rect::from_min_max(pos2(rect.left(), title.bottom()), rect.max);
            actions.extend(
                paint_channel_rack(paint, surfaces.rack_state, surfaces.rack_view, content)
                    .into_iter()
                    .map(UiAction::Rack),
            );
        }
        WindowKind::Playlist => {
            actions.extend(
                paint_playlist(paint, surfaces.playlist_state, surfaces.playlist_view, rect)
                    .into_iter()
                    .map(UiAction::Playlist),
            );
        }
        WindowKind::PianoRoll => {
            let content = Rect::from_min_max(pos2(rect.left(), title.bottom()), rect.max);
            let focused = surfaces.focused == Some(WindowKind::PianoRoll);
            actions.extend(
                paint_piano_roll(
                    paint,
                    surfaces.piano_state,
                    surfaces.piano_view,
                    content,
                    focused,
                )
                .into_iter()
                .map(UiAction::Piano),
            );
        }
        WindowKind::Mixer => {
            let content = Rect::from_min_max(pos2(rect.left(), title.bottom()), rect.max);
            actions.extend(
                paint_mixer(paint, surfaces.mixer_state, surfaces.mixer_view, content)
                    .into_iter()
                    .map(UiAction::Mixer),
            );
        }
        WindowKind::AudioSettings => {
            let content = Rect::from_min_max(pos2(rect.left(), title.bottom()), rect.max);
            actions.extend(
                paint_audio_settings(paint, surfaces.audio_settings_view, content)
                    .into_iter()
                    .map(UiAction::AudioSettings),
            );
        }
        WindowKind::ChannelSettings => {
            let content = Rect::from_min_max(pos2(rect.left(), title.bottom()), rect.max);
            actions.extend(
                paint_channel_settings(paint, surfaces.settings_view, content)
                    .into_iter()
                    .map(UiAction::Settings),
            );
        }
    }

    if !win.maximized {
        let painter = paint.painter();
        painter.rect_stroke(
            rect,
            0.0,
            egui::Stroke::new(1.0, t.desktop_edge),
            egui::StrokeKind::Inside,
        );
        let grip = Rect::from_min_size(
            pos2(rect.right() - 13.0, rect.bottom() - 13.0),
            vec2(12.0, 12.0),
        );
        let response = paint.ui.interact(
            grip,
            Id::new(("window.resize", win.kind.id())),
            Sense::drag(),
        );
        for i in 0..3 {
            let d = 3.0 + 3.0 * i as f32;
            painter.line_segment(
                [
                    pos2(grip.right() - d, grip.bottom() - 1.0),
                    pos2(grip.right() - 1.0, grip.bottom() - d),
                ],
                egui::Stroke::new(1.0, t.title_icon.gamma_multiply(0.5)),
            );
        }
        paint.probe.insert(
            format!("{}.resize", win.kind.id()),
            grip,
            "resize-grip",
            true,
            None,
        );
        if response.dragged() {
            let delta = paint.ui.input(|i| i.pointer.delta());
            actions.push(UiAction::Workspace(WsAction::Resize {
                kind: win.kind,
                edge: ResizeEdge::BottomRight,
                delta,
            }));
        }
    }
    paint.probe.insert(
        format!("{}.window", win.kind.id()),
        rect,
        "window",
        true,
        None,
    );
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use egui::{Event, Modifiers, PointerButton, RawInput};
    use jet_model::ChannelId;

    fn run_frame(app: &mut JetApp, ctx: &egui::Context, events: Vec<Event>) {
        if !app.fonts_installed {
            app.install_fonts(ctx);
        }
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 800.0))),
            events,
            ..RawInput::default()
        };
        ctx.run_ui(input, |ui| app.ui_pass(ui))
            .drop_without_applying_deltas();
    }

    fn click_button(app: &mut JetApp, ctx: &egui::Context, rect: Rect, button: PointerButton) {
        let pos = rect.center();
        run_frame(app, ctx, vec![Event::PointerMoved(pos)]);
        run_frame(
            app,
            ctx,
            vec![Event::PointerButton {
                pos,
                button,
                pressed: true,
                modifiers: Modifiers::default(),
            }],
        );
        run_frame(
            app,
            ctx,
            vec![Event::PointerButton {
                pos,
                button,
                pressed: false,
                modifiers: Modifiers::default(),
            }],
        );
    }

    fn click(app: &mut JetApp, ctx: &egui::Context, rect: Rect) {
        click_button(app, ctx, rect, PointerButton::Primary);
    }

    fn key(key: Key, pressed: bool, modifiers: Modifiers) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers,
        }
    }

    /// F6 opens the Channel Rack, which a new project keeps closed (as in the reference).
    fn open_rack(app: &mut JetApp, ctx: &egui::Context) {
        run_frame(app, ctx, vec![key(Key::F6, true, Modifiers::default())]);
        run_frame(app, ctx, vec![key(Key::F6, false, Modifiers::default())]);
        run_frame(app, ctx, Vec::new());
    }

    #[test]
    fn root_frame_registers_the_main_surfaces() {
        let ctx = egui::Context::default();
        let mut app = JetApp::default();
        run_frame(&mut app, &ctx, Vec::new());
        open_rack(&mut app, &ctx);
        for id in [
            "toolbar.play",
            "browser.add-folder",
            "channel-rack.add",
            "playlist.add-track",
        ] {
            assert!(app.probe().entries().contains_key(id), "missing {id}");
        }
    }

    #[test]
    fn new_project_shows_maximized_playlist_and_closed_rack() {
        let ctx = egui::Context::default();
        let mut app = JetApp::default();
        run_frame(&mut app, &ctx, Vec::new());
        assert!(app.probe().entries().contains_key("playlist.window"));
        assert!(!app.probe().entries().contains_key("channel-rack.window"));
    }

    #[test]
    fn pointer_click_adds_a_real_channel_and_transport_starts_and_stops() {
        let ctx = egui::Context::default();
        let mut app = JetApp::default();
        run_frame(&mut app, &ctx, Vec::new());
        open_rack(&mut app, &ctx);
        let add = app.probe().entries()["channel-rack.add"].rect;
        click(&mut app, &ctx, add);
        assert_eq!(app.session.project().channels.len(), 1);

        let play = app.probe().entries()["toolbar.play"].rect;
        click(&mut app, &ctx, play);
        assert_eq!(
            app.session.transport_snapshot().state,
            TransportState::Playing
        );
        let stop = app.probe().entries()["toolbar.stop"].rect;
        click(&mut app, &ctx, stop);
        assert_eq!(
            app.session.transport_snapshot().state,
            TransportState::Stopped
        );
    }

    #[test]
    fn step_buttons_and_playlist_grid_change_the_project() {
        let ctx = egui::Context::default();
        let mut app = JetApp::default();
        run_frame(&mut app, &ctx, Vec::new());
        open_rack(&mut app, &ctx);
        let add_channel = app.probe().entries()["channel-rack.add"].rect;
        click(&mut app, &ctx, add_channel);
        run_frame(&mut app, &ctx, Vec::new());
        let step = app.probe().entries()["channel-rack.step.1.0"].rect;
        click(&mut app, &ctx, step);
        assert!(app.session.project().patterns[0].steps[&jet_model::ChannelId(1)].cells[0].on);
        click_button(&mut app, &ctx, step, PointerButton::Secondary);
        assert!(!app.session.project().patterns[0].steps[&jet_model::ChannelId(1)].cells[0].on);

        let add_track = app.probe().entries()["playlist.add-track"].rect;
        click(&mut app, &ctx, add_track);
        assert_eq!(app.session.project().arrangements[0].tracks.len(), 1);
        run_frame(&mut app, &ctx, Vec::new());
        let grid = app.probe().entries()["playlist.grid"].rect;
        let track = app.probe().entries()["playlist.track.1"].rect;
        let clip_cell =
            Rect::from_center_size(pos2(grid.left() + 20.0, track.center().y), vec2(8.0, 8.0));
        click(&mut app, &ctx, clip_cell);
        assert_eq!(app.session.project().arrangements[0].clips.len(), 1);
    }

    #[test]
    fn clicking_an_empty_placeholder_row_creates_tracks_and_a_clip() {
        let ctx = egui::Context::default();
        let mut app = JetApp::default();
        run_frame(&mut app, &ctx, Vec::new());
        let grid = app.probe().entries()["playlist.grid"].rect;
        let row3 = app.probe().entries()["playlist.track.placeholder.3"].rect;
        click(
            &mut app,
            &ctx,
            Rect::from_center_size(pos2(grid.left() + 20.0, row3.center().y), vec2(8.0, 8.0)),
        );
        let arrangement = &app.session.project().arrangements[0];
        assert_eq!(arrangement.tracks.len(), 3);
        assert_eq!(arrangement.clips.len(), 1);
        assert_eq!(arrangement.clips[0].track, arrangement.tracks[2].id);
    }

    #[test]
    fn keyboard_toggles_playlist_and_tempo_editor_commits_value() {
        let ctx = egui::Context::default();
        let mut app = JetApp::default();
        run_frame(&mut app, &ctx, Vec::new());
        // The Playlist starts visible and focused, so F5 hides it and F5 again shows it.
        run_frame(
            &mut app,
            &ctx,
            vec![key(Key::F5, true, Modifiers::default())],
        );
        assert!(
            !app.workspace
                .as_ref()
                .unwrap()
                .window(WindowKind::Playlist)
                .unwrap()
                .visible
        );
        run_frame(
            &mut app,
            &ctx,
            vec![key(Key::F5, false, Modifiers::default())],
        );
        run_frame(
            &mut app,
            &ctx,
            vec![key(Key::F5, true, Modifiers::default())],
        );
        assert_eq!(
            app.workspace.as_ref().unwrap().focused,
            Some(WindowKind::Playlist)
        );

        let tempo = app.probe().entries()["toolbar.tempo"].rect;
        click(&mut app, &ctx, tempo);
        run_frame(
            &mut app,
            &ctx,
            vec![
                Event::Text("140".to_owned()),
                key(Key::Enter, true, Modifiers::default()),
            ],
        );
        assert_eq!(app.session.project().settings.tempo.0, 140_000);
    }

    fn drag(app: &mut JetApp, ctx: &egui::Context, from: egui::Pos2, to: egui::Pos2) {
        run_frame(app, ctx, vec![Event::PointerMoved(from)]);
        run_frame(
            app,
            ctx,
            vec![Event::PointerButton {
                pos: from,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::default(),
            }],
        );
        for step in 1..=10 {
            let t = step as f32 / 10.0;
            run_frame(app, ctx, vec![Event::PointerMoved(from + (to - from) * t)]);
        }
        run_frame(
            app,
            ctx,
            vec![Event::PointerButton {
                pos: to,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::default(),
            }],
        );
        run_frame(app, ctx, Vec::new());
    }

    fn toggle(app: &mut JetApp, ctx: &egui::Context, key_code: Key) {
        run_frame(app, ctx, vec![key(key_code, true, Modifiers::default())]);
        run_frame(app, ctx, vec![key(key_code, false, Modifiers::default())]);
        run_frame(app, ctx, Vec::new());
    }

    #[test]
    fn dragging_a_volume_knob_changes_volume_in_one_undo_step() {
        let ctx = egui::Context::default();
        let mut app = JetApp::default();
        run_frame(&mut app, &ctx, Vec::new());
        open_rack(&mut app, &ctx);
        let add = app.probe().entries()["channel-rack.add"].rect;
        click(&mut app, &ctx, add);
        run_frame(&mut app, &ctx, Vec::new());
        let knob = app.probe().entries()["channel-rack.volume.1"].rect;
        drag(
            &mut app,
            &ctx,
            knob.center(),
            knob.center() + vec2(0.0, 60.0),
        );
        let volume = app.session.project().channels[0].volume;
        assert!(volume < jet_model::DEFAULT_CHANNEL_VOLUME, "{volume}");
        app.execute("edit.undo_step", json!({}));
        assert_eq!(
            app.session.project().channels[0].volume,
            jet_model::DEFAULT_CHANNEL_VOLUME,
            "the whole drag is one undo step"
        );
    }

    #[test]
    fn context_menu_fill_each_four_steps_sets_real_steps() {
        let ctx = egui::Context::default();
        let mut app = JetApp::default();
        run_frame(&mut app, &ctx, Vec::new());
        open_rack(&mut app, &ctx);
        let add = app.probe().entries()["channel-rack.add"].rect;
        click(&mut app, &ctx, add);
        run_frame(&mut app, &ctx, Vec::new());
        let name = app.probe().entries()["channel-rack.channel.1"].rect;
        click_button(&mut app, &ctx, name, PointerButton::Secondary);
        run_frame(&mut app, &ctx, Vec::new());
        let item = app
            .probe()
            .entries()
            .iter()
            .find(|(id, e)| {
                id.starts_with("channel-rack.menu.item")
                    && e.value.as_deref() == Some("Fill each 4 steps")
            })
            .map(|(_, e)| e.rect)
            .expect("menu item");
        click(&mut app, &ctx, item);
        let cells = &app.session.project().patterns[0].steps[&ChannelId(1)].cells;
        let on: Vec<usize> = cells
            .iter()
            .enumerate()
            .filter(|(_, c)| c.on)
            .map(|(i, _)| i)
            .collect();
        assert_eq!(on, vec![0, 4, 8, 12]);
    }

    #[test]
    fn piano_roll_click_adds_a_note_and_mixer_fader_moves_the_master() {
        let ctx = egui::Context::default();
        let mut app = JetApp::default();
        run_frame(&mut app, &ctx, Vec::new());
        app.execute("channel.add", json!({"name": "Lead", "kind": "synth"}));
        toggle(&mut app, &ctx, Key::F7);
        let grid = app.probe().entries()["piano-roll.grid"].rect;
        click(
            &mut app,
            &ctx,
            Rect::from_center_size(grid.left_top() + vec2(30.0, 40.0), vec2(2.0, 2.0)),
        );
        let notes = &app.session.project().patterns[0].notes[&ChannelId(1)];
        assert_eq!(notes.len(), 1);
        // 30 px at the default 0.5 px/tick is tick 60; Step snap (24 ticks) floors it to 48.
        assert_eq!(notes[0].start.0, 48, "snapped to the step grid");

        toggle(&mut app, &ctx, Key::F9);
        let fader = app.probe().entries()["mixer.track.0.fader"].rect;
        let before = app.session.project().mixer.master.volume;
        drag(
            &mut app,
            &ctx,
            fader.center(),
            fader.center() + vec2(0.0, 40.0),
        );
        assert!(app.session.project().mixer.master.volume < before);
    }

    #[test]
    fn playlist_clips_move_and_delete_with_the_mouse() {
        let ctx = egui::Context::default();
        let mut app = JetApp::default();
        run_frame(&mut app, &ctx, Vec::new());
        let grid = app.probe().entries()["playlist.grid"].rect;
        let row1 = app.probe().entries()["playlist.track.placeholder.1"].rect;
        click(
            &mut app,
            &ctx,
            Rect::from_center_size(pos2(grid.left() + 20.0, row1.center().y), vec2(4.0, 4.0)),
        );
        run_frame(&mut app, &ctx, Vec::new());
        let clip = app.probe().entries()["playlist.clip.1"].rect;
        let bar_w = app.layout.bar_w;
        // Drag the clip two bars to the right (from its middle, away from the resize edge).
        let from = pos2(clip.left() + 8.0, clip.center().y);
        drag(&mut app, &ctx, from, from + vec2(bar_w * 2.0, 0.0));
        let ticks_per_bar = 384;
        assert_eq!(
            app.session.project().arrangements[0].clips[0].start.0,
            2 * ticks_per_bar
        );
        let clip = app.probe().entries()["playlist.clip.1"].rect;
        click_button(&mut app, &ctx, clip, PointerButton::Secondary);
        assert!(app.session.project().arrangements[0].clips.is_empty());
    }

    #[test]
    fn rack_header_sets_swing_and_pattern_length() {
        let ctx = egui::Context::default();
        let mut app = JetApp::default();
        run_frame(&mut app, &ctx, Vec::new());
        open_rack(&mut app, &ctx);
        let swing = app.probe().entries()["channel-rack.swing"].rect;
        drag(
            &mut app,
            &ctx,
            swing.center(),
            swing.center() - vec2(0.0, 100.0),
        );
        assert_eq!(
            app.session.project().settings.swing,
            50,
            "half the knob travel"
        );
        let length = app.probe().entries()["channel-rack.pattern-length"].rect;
        drag(
            &mut app,
            &ctx,
            length.center(),
            length.center() - vec2(0.0, 64.0),
        );
        assert_eq!(
            app.session.project().patterns[0].length,
            jet_model::PatternLength::Steps(32),
            "16 steps + 64 px / 4 px per step"
        );
        click_button(&mut app, &ctx, length, PointerButton::Secondary);
        assert_eq!(
            app.session.project().patterns[0].length,
            jet_model::PatternLength::Auto
        );
    }

    #[test]
    fn knob_menu_creates_an_automation_clip_whose_envelope_takes_points() {
        let ctx = egui::Context::default();
        let mut app = JetApp::default();
        run_frame(&mut app, &ctx, Vec::new());
        open_rack(&mut app, &ctx);
        let add = app.probe().entries()["channel-rack.add"].rect;
        click(&mut app, &ctx, add);
        run_frame(&mut app, &ctx, Vec::new());
        let knob = app.probe().entries()["channel-rack.volume.1"].rect;
        click_button(&mut app, &ctx, knob, PointerButton::Secondary);
        run_frame(&mut app, &ctx, Vec::new());
        let item = app.probe().entries()["channel-rack.knob-menu.item.0"].rect;
        click(&mut app, &ctx, item);
        assert_eq!(app.session.project().automations.len(), 1);
        let arrangement = &app.session.project().arrangements[0];
        assert_eq!(arrangement.clips.len(), 1);
        assert!(matches!(
            arrangement.clips[0].source,
            ClipSource::Automation(_)
        ));
        // Close the rack so the Playlist envelope is reachable, then right-click inside it.
        toggle(&mut app, &ctx, Key::F6);
        run_frame(&mut app, &ctx, Vec::new());
        let body = app.probe().entries()["playlist.automation.1"].rect;
        let spot = Rect::from_center_size(
            pos2(body.left() + body.width() * 0.3, body.top() + 6.0),
            vec2(2.0, 2.0),
        );
        click_button(&mut app, &ctx, spot, PointerButton::Secondary);
        let points = &app.session.project().automations[0].points;
        assert_eq!(points.len(), 3, "a point was added between the two ends");
        assert!(
            points[1].value > 800,
            "near the top of the clip: {}",
            points[1].value
        );
    }

    #[test]
    fn channel_button_opens_settings_whose_controls_edit_the_channel() {
        let ctx = egui::Context::default();
        let mut app = JetApp::default();
        run_frame(&mut app, &ctx, Vec::new());
        open_rack(&mut app, &ctx);
        let add = app.probe().entries()["channel-rack.add"].rect;
        click(&mut app, &ctx, add);
        run_frame(&mut app, &ctx, Vec::new());
        let button = app.probe().entries()["channel-rack.channel.1"].rect;
        click(&mut app, &ctx, button);
        run_frame(&mut app, &ctx, Vec::new());
        assert!(
            app.probe()
                .entries()
                .contains_key("channel-settings.window")
        );
        // Max polyphony slider: clicking near its right end sets a high value.
        let poly = app.probe().entries()["channel-settings.poly"].rect;
        click(
            &mut app,
            &ctx,
            Rect::from_center_size(pos2(poly.right() - 2.0, poly.center().y), vec2(1.0, 1.0)),
        );
        assert!(app.session.project().channels[0].polyphony.max >= 30);
        let mono = app.probe().entries()["channel-settings.mono"].rect;
        click(&mut app, &ctx, mono);
        assert!(app.session.project().channels[0].polyphony.mono);
        // Volume envelope toggle on the Sampler.
        let envelope = app.probe().entries()["channel-settings.envelope"].rect;
        click(&mut app, &ctx, envelope);
        assert!(app.session.project().channels[0].sampler.envelope.is_some());
        // Clicking the Channel button again closes the window.
        let button = app.probe().entries()["channel-rack.channel.1"].rect;
        click(&mut app, &ctx, button);
        run_frame(&mut app, &ctx, Vec::new());
        assert!(
            !app.probe()
                .entries()
                .contains_key("channel-settings.window")
        );
    }

    #[test]
    fn typing_keyboard_plays_notes_and_blocks_letter_shortcuts() {
        let ctx = egui::Context::default();
        let mut app = JetApp::default();
        run_frame(&mut app, &ctx, Vec::new());
        app.execute("channel.add", json!({"name": "Lead", "kind": "synth"}));
        let ctrl = Modifiers {
            ctrl: true,
            command: true,
            ..Modifiers::default()
        };
        // egui tracks held modifiers through ModifiersChanged events, not the key event alone.
        let with_ctrl = |app: &mut JetApp, mut events: Vec<Event>| {
            events.insert(0, Event::ModifiersChanged(ctrl));
            run_frame(app, &ctx, events);
            run_frame(
                app,
                &ctx,
                vec![Event::ModifiersChanged(Modifiers::default())],
            );
        };
        with_ctrl(&mut app, vec![key(Key::T, true, ctrl)]);
        with_ctrl(&mut app, vec![key(Key::T, false, ctrl)]);
        run_frame(&mut app, &ctx, Vec::new());
        assert!(app.typing);
        run_frame(
            &mut app,
            &ctx,
            vec![key(Key::Z, true, Modifiers::default())],
        );
        assert_eq!(app.typing_held, vec![(Key::Z, 48)], "Z is C4 (FL naming)");
        run_frame(
            &mut app,
            &ctx,
            vec![key(Key::Z, false, Modifiers::default())],
        );
        assert!(app.typing_held.is_empty(), "release ends the note");
        let mode = app.session.transport_snapshot().mode;
        run_frame(
            &mut app,
            &ctx,
            vec![key(Key::L, true, Modifiers::default())],
        );
        assert_eq!(
            app.session.transport_snapshot().mode,
            mode,
            "L plays a note, not PAT/SONG"
        );
        with_ctrl(&mut app, vec![key(Key::M, true, ctrl)]);
        assert!(app.session.metronome(), "Ctrl+M toggles the metronome");
    }

    #[test]
    fn dropping_a_browser_file_on_the_playlist_creates_an_audio_clip() {
        let path = std::env::temp_dir().join(format!("jet-ui-drop-{}.wav", std::process::id()));
        jet_media::write_wav(&path, 48_000, 1, &[0.5; 4_800], jet_media::WavFormat::Int16).unwrap();
        let ctx = egui::Context::default();
        let mut app = JetApp::default();
        run_frame(&mut app, &ctx, Vec::new());
        let grid = app.probe().entries()["playlist.grid"].rect;
        let spot = grid.left_top() + vec2(grid.width() * 0.25, 70.0);
        run_frame(&mut app, &ctx, vec![Event::PointerMoved(spot)]);
        // The Browser drag is live while the button is held; the release frame drops it.
        app.browser.dragging = Some(path.clone());
        run_frame(
            &mut app,
            &ctx,
            vec![Event::PointerButton {
                pos: spot,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::default(),
            }],
        );
        let project = app.session.project();
        let clips = &project.arrangements[0].clips;
        assert_eq!(clips.len(), 1, "{:?}", app.last_error);
        assert!(matches!(clips[0].source, ClipSource::Audio(_)));
        assert_eq!(
            project.arrangements[0].tracks.len(),
            2,
            "dropped on the second row"
        );
        assert_eq!(clips[0].start.0 % 96, 0, "snapped to a beat");
        assert!(app.browser.dragging.is_none());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn f10_opens_audio_settings_and_apply_keeps_the_choice() {
        let ctx = egui::Context::default();
        let mut app = JetApp::default();
        run_frame(&mut app, &ctx, Vec::new());
        toggle(&mut app, &ctx, Key::F10);
        assert!(app.probe().entries().contains_key("audio-settings.window"));
        // Rate choice index 2 is 48 000 Hz (0 = device default).
        let rate = app.probe().entries()["audio-settings.rate.2"].rect;
        click(&mut app, &ctx, rate);
        let buffer = app.probe().entries()["audio-settings.buffer.3"].rect;
        click(&mut app, &ctx, buffer);
        assert_eq!(app.audio_pending.rate, Some(48_000));
        assert_eq!(app.audio_pending.buffer_frames, Some(512));
        assert_eq!(app.audio_request.rate, None, "nothing changes before Apply");
        let apply = app.probe().entries()["audio-settings.apply"].rect;
        click(&mut app, &ctx, apply);
        assert_eq!(app.audio_request.rate, Some(48_000));
        assert!(app.audio.is_none(), "tests never open audio hardware");
    }
}
