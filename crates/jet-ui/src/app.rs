use std::time::Duration;

use eframe::egui;
use egui::{Id, Key, Rect, Sense, pos2, vec2};
use jet_engine::{PlayMode, Session, TransportState};
use jet_model::ClipSource;
use serde_json::{Value, json};

use crate::browser::{BrowserState, paint_browser};
use crate::channel_rack::{ChannelView, RackAction, RackState, RackView, paint_channel_rack};
use crate::icons;
use crate::playlist::{
    ClipView, PatternEntry, PlaylistAction, PlaylistState, PlaylistView, TrackView, paint_playlist,
    paint_title_tools,
};
use crate::probe::LayoutProbe;
use crate::theme::{self, Layout, Tokens};
use crate::toolbar::{ToolbarAction, ToolbarState, ToolbarView, paint_toolbar};
use crate::widgets::{PaintCtx, WindowGlyph};
use crate::workspace::{ResizeEdge, WindowKind, Workspace, WsAction};

pub struct JetApp {
    session: Session,
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
    pending_show: Option<WindowKind>,
}

impl Default for JetApp {
    fn default() -> Self {
        Self {
            session: Session::new_default(),
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
            pending_show: None,
        }
    }
}

enum UiAction {
    Engine(&'static str, Value),
    Workspace(WsAction),
    Rack(RackAction),
    Playlist(PlaylistAction),
    OpenProject,
    SaveProject,
    SaveProjectAs,
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
        self.pending_show = Some(kind);
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
            UiAction::Rack(action) => match action {
                RackAction::AddChannel => {
                    let name = format!("Channel {}", self.session.project().ids.next_channel);
                    self.execute("channel.add", json!({"name": name}));
                }
                RackAction::SetStep {
                    pattern,
                    channel,
                    index,
                    on,
                } => {
                    self.execute(
                        "step.set",
                        json!({"pattern": pattern, "channel": channel, "index": index, "on": on}),
                    );
                }
                RackAction::SelectChannel(id) => self.rack.selected_channel = Some(id),
                RackAction::SetMute { channel, muted } => {
                    self.execute("channel.mute.set", json!({"id": channel, "mute": muted}));
                }
            },
            UiAction::Playlist(action) => match action {
                PlaylistAction::AddTrack => self.add_track(),
                PlaylistAction::SelectTrack(id) => self.playlist.selected_track = Some(id),
                PlaylistAction::SelectPattern(id) => {
                    self.execute("pattern.select", json!({"id": id}));
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
        if self.toolbar.tempo_edit.is_some() {
            return actions;
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
        if pressed(Key::L) && !modifiers.any() {
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
        if pressed(Key::Plus) && !modifiers.any() {
            actions.push(UiAction::Engine("pattern.next", json!({})));
        }
        if pressed(Key::Minus) && !modifiers.any() {
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
        }
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
                ChannelView {
                    id: channel.id.0,
                    name: channel.name.clone(),
                    color: channel.color,
                    muted: channel.mute,
                    volume: channel.volume,
                    pan: channel.pan,
                    mixer_target: channel.mixer_target,
                    steps,
                }
            })
            .collect();
        RackView {
            pattern_id: self.session.selected_pattern().0,
            steps_per_beat,
            channels,
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
                    let ClipSource::Pattern(pattern_id) = clip.source;
                    let pattern = project.patterns.iter().find(|p| p.id == pattern_id);
                    ClipView {
                        id: clip.id.0,
                        track: clip.track.0,
                        start: clip.start.0,
                        length: clip.length.0,
                        label: pattern.map_or_else(|| "Pattern".to_owned(), |p| p.name.clone()),
                        color: pattern.and_then(|p| p.color),
                    }
                })
                .collect()
        });
        let ticks_per_bar =
            i64::from(project.settings.ppq.0) * 4 * i64::from(project.settings.time_sig.num)
                / i64::from(project.settings.time_sig.den);
        let song_running =
            snapshot.mode == PlayMode::Song && snapshot.state != TransportState::Stopped;
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
                })
                .collect(),
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
            let mut workspace = Workspace::new(initial, self.layout.browser_w);
            if let Some(kind) = self.pending_show.take() {
                let _ = workspace.apply(WsAction::Toggle(kind), initial, self.layout.snap_distance);
            }
            self.workspace = Some(workspace);
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
        let toolbar_view = self.toolbar_view();
        let rack_view = self.rack_view();
        let playlist_view = self.playlist_view();
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

        for action in actions {
            self.run_action(action, desktop, &ctx);
        }
        if toolbar_view.playing && !self.static_capture {
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
        WindowKind::ChannelRack => 22.0,
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
}
