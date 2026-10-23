use std::time::Duration;

use eframe::egui;
use egui::{Align2, FontId, Id, Key, Rect, Sense, Stroke, StrokeKind, pos2, vec2};
use jet_engine::{PlayMode, Session, TransportState};
use jet_model::ClipSource;
use serde_json::{Value, json};

use crate::browser::{BrowserState, paint_browser};
use crate::channel_rack::{ChannelView, RackAction, RackState, RackView, paint_channel_rack};
use crate::playlist::{
    ClipView, PlaylistAction, PlaylistState, PlaylistView, TrackView, paint_playlist,
};
use crate::probe::LayoutProbe;
use crate::theme::{Layout, Tokens};
use crate::toolbar::{ToolbarAction, ToolbarState, ToolbarView, paint_toolbar};
use crate::widgets::PaintCtx;
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
            UiAction::Rack(action) => {
                match action {
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
                        self.execute("step.set", json!({"pattern": pattern, "channel": channel, "index": index, "on": on}));
                    }
                    RackAction::SelectChannel(id) => self.rack.selected_channel = Some(id),
                    RackAction::SetMute { channel, muted } => {
                        self.execute("channel.mute.set", json!({"id": channel, "mute": muted}));
                    }
                }
            }
            UiAction::Playlist(action) => match action {
                PlaylistAction::AddTrack => {
                    let name = format!("Track {}", self.session.project().ids.next_track);
                    self.execute("playlist.track.add", json!({"name": name}));
                }
                PlaylistAction::SelectTrack(id) => self.playlist.selected_track = Some(id),
                PlaylistAction::AddClip {
                    track,
                    pattern,
                    start,
                    length,
                } => {
                    self.execute("playlist.clip.add", json!({"track": track, "pattern": pattern, "start": start, "length": length}));
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
        let input = ctx.input(|i| {
            (
                i.modifiers,
                i.key_pressed(Key::F5),
                i.key_pressed(Key::F6),
                i.key_pressed(Key::F8),
                i.key_pressed(Key::F12),
                i.key_pressed(Key::F11),
                i.key_pressed(Key::Tab),
                i.key_pressed(Key::Enter),
                i.key_pressed(Key::Escape),
                i.key_pressed(Key::H),
                i.key_pressed(Key::O),
                i.key_pressed(Key::S),
                i.key_pressed(Key::L),
                i.key_pressed(Key::Space),
                i.key_pressed(Key::Z),
                i.key_pressed(Key::F4),
                i.key_pressed(Key::Plus),
                i.key_pressed(Key::Minus),
            )
        });
        let (
            modifiers,
            f5,
            f6,
            f8,
            f12,
            f11,
            tab,
            enter,
            escape,
            h,
            o,
            s,
            l,
            space,
            z,
            f4,
            plus,
            minus,
        ) = input;
        if f11 && modifiers.ctrl {
            self.toolbar.hidden = !self.toolbar.hidden;
        }
        if f5 {
            actions.push(UiAction::Workspace(WsAction::Toggle(WindowKind::Playlist)));
        }
        if f6 {
            actions.push(UiAction::Workspace(WsAction::Toggle(
                WindowKind::ChannelRack,
            )));
        }
        if f8 && modifiers.alt {
            actions.push(UiAction::Workspace(WsAction::ToggleBrowser));
        }
        if f12 {
            actions.push(UiAction::Workspace(WsAction::CloseAll));
        }
        if tab {
            actions.push(UiAction::Workspace(WsAction::CycleNested));
        }
        if h && modifiers.ctrl && modifiers.shift {
            actions.push(UiAction::Workspace(WsAction::ArrangeDefault));
        }
        if self.toolbar.tempo_edit.is_none() {
            if o && modifiers.ctrl {
                actions.push(UiAction::OpenProject);
            }
            if s && modifiers.ctrl {
                actions.push(if modifiers.shift {
                    UiAction::SaveProjectAs
                } else {
                    UiAction::SaveProject
                });
            }
            if enter && !modifiers.any() {
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
            if l && !modifiers.any() {
                actions.push(UiAction::Engine("transport.mode.toggle", json!({})));
            }
            if space {
                actions.push(UiAction::Engine(
                    if modifiers.ctrl {
                        "transport.pause"
                    } else {
                        "transport.play_stop"
                    },
                    json!({}),
                ));
            }
            if z && modifiers.ctrl {
                actions.push(UiAction::Engine(
                    if modifiers.alt {
                        "edit.undo_step"
                    } else {
                        "edit.undo_toggle"
                    },
                    json!({}),
                ));
            }
            if f4 {
                actions.push(UiAction::Engine("pattern.next_empty", json!({})));
            }
            if plus && !modifiers.any() {
                actions.push(UiAction::Engine("pattern.next", json!({})));
            }
            if minus && !modifiers.any() {
                actions.push(UiAction::Engine("pattern.prev", json!({})));
            }
            if escape
                && self.toolbar.open_menu.take().is_none()
                && let Some(kind) = self.workspace.as_ref().and_then(|ws| ws.focused)
            {
                actions.push(UiAction::Workspace(WsAction::Close(kind)));
            }
        }
        actions
    }

    fn toolbar_view(&self) -> ToolbarView {
        let snapshot = self.session.transport_snapshot();
        let project = self.session.project();
        let selected = self.session.selected_pattern();
        let pattern = project
            .patterns
            .iter()
            .find(|p| p.id == selected)
            .or_else(|| project.patterns.first());
        let position = jet_time::format_bar_step_tick(
            snapshot.position,
            project.settings.ppq,
            project.settings.time_sig,
            project.settings.steps_per_beat,
        )
        .unwrap_or_else(|_| "--:--:--".to_owned());
        ToolbarView {
            song_mode: snapshot.mode == PlayMode::Song,
            playing: snapshot.state == TransportState::Playing,
            paused: snapshot.state == TransportState::Paused,
            tempo_milli: i64::from(project.settings.tempo.0),
            position,
            pattern_id: selected.0,
            pattern_name: pattern.map_or_else(|| "Pattern".to_owned(), |p| p.name.clone()),
        }
    }

    fn rack_view(&self) -> RackView {
        let project = self.session.project();
        let selected = self.session.selected_pattern();
        let pattern = project
            .patterns
            .iter()
            .find(|p| p.id == selected)
            .or_else(|| project.patterns.first());
        let default_steps = usize::from(project.settings.steps_per_beat)
            * usize::from(project.settings.time_sig.num);
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
                    muted: channel.mute,
                    steps,
                }
            })
            .collect();
        RackView {
            pattern_id: selected.0,
            pattern_name: pattern.map_or_else(|| "Pattern".to_owned(), |p| p.name.clone()),
            channels,
        }
    }

    fn playlist_view(&self) -> PlaylistView {
        let project = self.session.project();
        let selected = self.session.selected_pattern();
        let arrangement = project
            .arrangements
            .iter()
            .find(|a| a.id == project.active_arrangement);
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
                    let label = project
                        .patterns
                        .iter()
                        .find(|p| p.id == pattern_id)
                        .map_or_else(|| "Pattern".to_owned(), |p| p.name.clone());
                    ClipView {
                        id: clip.id.0,
                        track: clip.track.0,
                        start: clip.start.0,
                        length: clip.length.0,
                        label,
                    }
                })
                .collect()
        });
        let ticks_per_bar =
            i64::from(project.settings.ppq.0) * 4 * i64::from(project.settings.time_sig.num)
                / i64::from(project.settings.time_sig.den);
        PlaylistView {
            pattern_id: selected.0,
            pattern_name: project
                .patterns
                .iter()
                .find(|p| p.id == selected)
                .map_or_else(|| "Pattern".to_owned(), |p| p.name.clone()),
            ticks_per_bar,
            tracks,
            clips,
        }
    }
}

impl JetApp {
    /// Draw one frame against egui's root UI; also used by headless interaction tests.
    pub fn ui_pass(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.probe.begin_frame();
        self.hint.clear();
        let available = ui.max_rect();
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
        if self.workspace.is_none() {
            let initial_desktop = Rect::from_min_max(
                pos2(
                    base_desktop.left() + self.layout.browser_w,
                    base_desktop.top(),
                ),
                base_desktop.max,
            );
            self.workspace = Some(Workspace::new(initial_desktop, self.layout.browser_w));
        }
        let browser_visible = self.workspace.as_ref().is_some_and(|ws| ws.browser_visible);
        let browser_w = self
            .workspace
            .as_ref()
            .map_or(self.layout.browser_w, |ws| ws.browser_width);
        let browser_rect = Rect::from_min_size(
            base_desktop.min,
            vec2(
                if browser_visible { browser_w } else { 0.0 },
                base_desktop.height(),
            ),
        );
        let desktop = Rect::from_min_max(
            pos2(
                base_desktop.left() + browser_rect.width(),
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
                .find(|win| win.visible && win.rect.contains(pointer))
                && focused != Some(win.kind)
            {
                actions.push(UiAction::Workspace(WsAction::Focus(win.kind)));
            }
            for win in windows.iter().filter(|win| win.visible) {
                paint_window(
                    &mut paint,
                    win,
                    &mut self.rack,
                    &rack_view,
                    &mut self.playlist,
                    &playlist_view,
                    &mut actions,
                );
            }
            if !self.toolbar.hidden {
                actions.extend(
                    paint_toolbar(&mut paint, &mut self.toolbar, &toolbar_view, toolbar_rect)
                        .into_iter()
                        .map(UiAction::from),
                );
                let hint_rect = Rect::from_min_size(
                    pos2(toolbar_rect.left() + 8.0, toolbar_rect.bottom() - 23.0),
                    vec2(toolbar_rect.width() - 16.0, 18.0),
                );
                let hint_text = if let Some(error) = &self.last_error {
                    format!("Erro: {error}")
                } else if paint.hint.is_empty() {
                    "M1: transporte silencioso; áudio, plugins e gravação pendentes".to_owned()
                } else {
                    paint.hint.clone()
                };
                paint.ui.painter().text(
                    pos2(hint_rect.left(), hint_rect.center().y),
                    Align2::LEFT_CENTER,
                    hint_text,
                    FontId::proportional(self.layout.font_small),
                    self.tokens.muted,
                );
                paint.probe.insert(
                    "toolbar.hint",
                    hint_rect,
                    "hint",
                    true,
                    Some(paint.hint.clone()),
                );
            }
        }

        for action in actions {
            self.run_action(action, desktop, &ctx);
        }
        if toolbar_view.playing {
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

fn paint_window(
    paint: &mut PaintCtx<'_>,
    win: &crate::workspace::WinState,
    rack_state: &mut RackState,
    rack_view: &RackView,
    playlist_state: &mut PlaylistState,
    playlist_view: &PlaylistView,
    actions: &mut Vec<UiAction>,
) {
    let rect = win.rect;
    paint.panel(rect, false);
    let title = Rect::from_min_size(rect.min, vec2(rect.width(), paint.layout.window_title_h));
    paint.panel(title, true);
    let title_drag = Rect::from_min_max(title.min, pos2(title.right() - 60.0, title.bottom()));
    let response = paint.ui.interact(
        title_drag,
        Id::new(("window.title", win.kind.id())),
        Sense::click_and_drag(),
    );
    paint.ui.painter().text(
        pos2(title.left() + 8.0, title.center().y),
        Align2::LEFT_CENTER,
        win.kind.title(),
        FontId::proportional(paint.layout.font_regular),
        paint.tokens.text,
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
    let max_rect = Rect::from_min_size(
        pos2(title.right() - 56.0, title.top() + 3.0),
        vec2(24.0, title.height() - 6.0),
    );
    if paint
        .button(
            &format!("{}.maximize", win.kind.id()),
            max_rect,
            if win.maximized { "◧" } else { "□" },
            false,
            true,
            "Maximizar/restaurar janela",
        )
        .clicked()
    {
        actions.push(UiAction::Workspace(if win.maximized {
            WsAction::Restore(win.kind)
        } else {
            WsAction::Maximize(win.kind)
        }));
    }
    let close_rect = Rect::from_min_size(
        pos2(title.right() - 29.0, title.top() + 3.0),
        vec2(24.0, title.height() - 6.0),
    );
    if paint
        .button(
            &format!("{}.close", win.kind.id()),
            close_rect,
            "×",
            false,
            true,
            "Fechar janela",
        )
        .clicked()
    {
        actions.push(UiAction::Workspace(WsAction::Close(win.kind)));
    }
    let content = Rect::from_min_max(
        pos2(rect.left() + 2.0, title.bottom() + 1.0),
        pos2(rect.right() - 2.0, rect.bottom() - 2.0),
    );
    match win.kind {
        WindowKind::ChannelRack => {
            actions.extend(
                paint_channel_rack(paint, rack_state, rack_view, content)
                    .into_iter()
                    .map(UiAction::Rack),
            );
        }
        WindowKind::Playlist => {
            actions.extend(
                paint_playlist(paint, playlist_state, playlist_view, content)
                    .into_iter()
                    .map(UiAction::Playlist),
            );
        }
    }
    if !win.maximized {
        let grip = Rect::from_min_size(
            pos2(rect.right() - 13.0, rect.bottom() - 13.0),
            vec2(12.0, 12.0),
        );
        let response = paint.ui.interact(
            grip,
            Id::new(("window.resize", win.kind.id())),
            Sense::drag(),
        );
        paint.ui.painter().line_segment(
            [grip.left_bottom(), grip.right_top()],
            Stroke::new(1.0, paint.tokens.border),
        );
        paint.ui.painter().rect_stroke(
            grip,
            0.0,
            Stroke::new(1.0, paint.tokens.border),
            StrokeKind::Inside,
        );
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

    #[test]
    fn root_frame_registers_the_main_surfaces() {
        let ctx = egui::Context::default();
        let mut app = JetApp::default();
        run_frame(&mut app, &ctx, Vec::new());
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
    fn pointer_click_adds_a_real_channel_and_transport_starts_and_stops() {
        let ctx = egui::Context::default();
        let mut app = JetApp::default();
        run_frame(&mut app, &ctx, Vec::new());
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
    fn keyboard_toggles_playlist_and_tempo_editor_commits_value() {
        let ctx = egui::Context::default();
        let mut app = JetApp::default();
        run_frame(&mut app, &ctx, Vec::new());
        run_frame(
            &mut app,
            &ctx,
            vec![key(Key::F5, true, Modifiers::default())],
        );
        assert_eq!(
            app.workspace.as_ref().unwrap().focused,
            Some(WindowKind::Playlist)
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
        assert!(
            !app.workspace
                .as_ref()
                .unwrap()
                .window(WindowKind::Playlist)
                .unwrap()
                .visible
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
