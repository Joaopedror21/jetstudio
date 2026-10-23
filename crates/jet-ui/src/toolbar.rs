use egui::{Align2, Event, FontId, Id, Key, Rect, Sense, Stroke, StrokeKind, pos2, vec2};
use serde_json::{Value, json};

use crate::icons;
use crate::widgets::PaintCtx;
use crate::workspace::{WindowKind, WsAction};

#[derive(Default)]
pub struct ToolbarState {
    pub open_menu: Option<usize>,
    pub tempo_edit: Option<String>,
    pub hidden: bool,
}

pub struct ToolbarView {
    pub song_mode: bool,
    pub playing: bool,
    pub paused: bool,
    pub tempo_milli: i64,
    pub position: String,
    pub pattern_id: u64,
    pub pattern_name: String,
}

pub enum ToolbarAction {
    Engine(&'static str, Value),
    Workspace(WsAction),
    OpenProject,
    SaveProject,
    SaveProjectAs,
    Minimize,
    Maximize,
    Quit,
}

const MENUS: [&str; 8] = [
    "FILE", "EDIT", "ADD", "PATTERNS", "VIEW", "OPTIONS", "TOOLS", "HELP",
];

pub fn paint_toolbar(
    paint: &mut PaintCtx<'_>,
    state: &mut ToolbarState,
    view: &ToolbarView,
    rect: Rect,
) -> Vec<ToolbarAction> {
    let mut actions = Vec::new();
    if state.hidden {
        return actions;
    }
    paint.panel(rect, true);
    let menu_h = 29.0; // UNMEASURED: reference profile not captured.
    let menu_row = Rect::from_min_size(rect.min, vec2(rect.width(), menu_h));
    paint
        .ui
        .painter()
        .rect_filled(menu_row, 0.0, paint.tokens.surface);
    let mut x = menu_row.left() + 7.0;
    for (index, name) in MENUS.iter().enumerate() {
        let width = if *name == "PATTERNS" || *name == "OPTIONS" {
            77.0
        } else {
            53.0
        };
        let button_rect =
            Rect::from_min_size(pos2(x, menu_row.top() + 3.0), vec2(width, menu_h - 6.0));
        if paint
            .button(
                &format!("toolbar.menu.{name}"),
                button_rect,
                name,
                state.open_menu == Some(index),
                true,
                &format!("Menu {name}"),
            )
            .clicked()
        {
            state.open_menu = if state.open_menu == Some(index) {
                None
            } else {
                Some(index)
            };
        }
        x += width + 2.0;
    }
    let title_rect = Rect::from_min_size(
        pos2(x + 4.0, menu_row.top()),
        vec2((menu_row.right() - x - 92.0).max(0.0), menu_h),
    );
    paint.ui.painter().text(
        pos2(title_rect.left() + 5.0, title_rect.center().y),
        Align2::LEFT_CENTER,
        "JetStudio · projeto local",
        FontId::proportional(paint.layout.font_regular),
        paint.tokens.muted,
    );
    let drag = paint
        .ui
        .interact(title_rect, Id::new("toolbar.title-drag"), Sense::drag());
    if drag.drag_started() {
        paint
            .ui
            .ctx()
            .send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
    paint
        .probe
        .insert("toolbar.title-drag", title_rect, "title", true, None);

    let window_x = rect.right() - 83.0;
    for (offset, text, action) in [
        (0.0, "_", ToolbarAction::Minimize),
        (27.0, "□", ToolbarAction::Maximize),
        (54.0, "×", ToolbarAction::Quit),
    ] {
        let control_rect = Rect::from_min_size(
            pos2(window_x + offset, menu_row.top() + 3.0),
            vec2(25.0, 22.0),
        );
        if paint
            .button(
                &format!("toolbar.window.{}", offset as i32),
                control_rect,
                text,
                false,
                true,
                text,
            )
            .clicked()
        {
            actions.push(action);
        }
    }

    let y = menu_row.bottom() + 7.0;
    let button_h = 24.0;
    let mut x = rect.left() + 8.0;
    let mode_rect = Rect::from_min_size(pos2(x, y), vec2(55.0, button_h));
    let mode_text = if view.song_mode { "SONG" } else { "PAT" };
    let mode_response = paint.button(
        "toolbar.mode",
        mode_rect,
        mode_text,
        view.song_mode,
        true,
        "Alternar PAT/SONG (L)",
    );
    if mode_response.clicked() {
        actions.push(ToolbarAction::Engine("transport.mode.toggle", json!({})));
    }
    if mode_response.secondary_clicked() {
        actions.push(ToolbarAction::Workspace(WsAction::Focus(
            if view.song_mode {
                WindowKind::Playlist
            } else {
                WindowKind::ChannelRack
            },
        )));
    }
    x += 60.0;
    for (id, hint, command) in [
        ("play", "Play/Stop (Space)", "transport.play_stop"),
        ("stop", "Stop", "transport.stop"),
    ] {
        let cell = Rect::from_min_size(pos2(x, y), vec2(28.0, button_h));
        let enabled = id != "stop" || view.playing || view.paused;
        let response = paint.button(
            &format!("toolbar.{id}"),
            cell,
            "",
            id == "play" && view.playing,
            enabled,
            hint,
        );
        if id == "play" {
            icons::play(paint.ui.painter(), cell.center(), paint.tokens.text);
        } else {
            icons::stop(paint.ui.painter(), cell.center(), paint.tokens.text);
        }
        if response.clicked() {
            actions.push(ToolbarAction::Engine(command, json!({})));
        }
        x += 30.0;
    }
    let record_rect = Rect::from_min_size(pos2(x, y), vec2(28.0, button_h));
    let _ = paint.button(
        "toolbar.record",
        record_rect,
        "",
        false,
        false,
        "Gravação chega com áudio/MIDI no M2/M6",
    );
    icons::record(
        paint.ui.painter(),
        record_rect.center(),
        paint.tokens.disabled,
    );
    x += 38.0;

    let tempo_rect = Rect::from_min_size(pos2(x, y), vec2(82.0, button_h));
    let tempo_text = state
        .tempo_edit
        .clone()
        .unwrap_or_else(|| format!("{:.3}", view.tempo_milli as f64 / 1000.0));
    let tempo_response = paint.button(
        "toolbar.tempo",
        tempo_rect,
        &tempo_text,
        state.tempo_edit.is_some(),
        true,
        "Tempo em BPM; clique e digite 10–522",
    );
    if tempo_response.clicked() {
        state.tempo_edit = Some(String::new());
    }
    if let Some(mut input) = state.tempo_edit.take() {
        let mut keep_editing = true;
        let events = paint.ui.input(|i| i.events.clone());
        for event in events {
            match event {
                Event::Text(text) => {
                    if text
                        .chars()
                        .all(|c| c.is_ascii_digit() || c == '.' || c == ',')
                    {
                        input.push_str(&text.replace(',', "."));
                    }
                }
                Event::Key {
                    key: Key::Backspace,
                    pressed: true,
                    ..
                } => {
                    input.pop();
                }
                Event::Key {
                    key: Key::Escape,
                    pressed: true,
                    ..
                } => keep_editing = false,
                Event::Key {
                    key: Key::Enter,
                    pressed: true,
                    ..
                } => {
                    if let Ok(bpm) = input.parse::<f64>() {
                        let milli = (bpm * 1000.0).round() as i64;
                        if (10_000..=522_000).contains(&milli) {
                            actions.push(ToolbarAction::Engine(
                                "transport.tempo.set",
                                json!({"milli_bpm": milli}),
                            ));
                        }
                    }
                    keep_editing = false;
                }
                _ => {}
            }
        }
        if keep_editing {
            state.tempo_edit = Some(input);
        }
    }
    x += 90.0;
    let position_rect = Rect::from_min_size(pos2(x, y), vec2(108.0, button_h));
    paint
        .ui
        .painter()
        .rect_filled(position_rect, 2.0, paint.tokens.surface);
    paint.ui.painter().rect_stroke(
        position_rect,
        2.0,
        Stroke::new(1.0, paint.tokens.border),
        StrokeKind::Inside,
    );
    paint.ui.painter().text(
        position_rect.center(),
        Align2::CENTER_CENTER,
        &view.position,
        FontId::proportional(paint.layout.font_large),
        paint.tokens.text,
    );
    paint.probe.insert(
        "toolbar.position",
        position_rect,
        "lcd",
        false,
        Some(view.position.clone()),
    );
    x += 118.0;

    let previous = Rect::from_min_size(pos2(x, y), vec2(25.0, button_h));
    if paint
        .button(
            "toolbar.pattern.prev",
            previous,
            "−",
            false,
            true,
            "Pattern anterior (-)",
        )
        .clicked()
    {
        actions.push(ToolbarAction::Engine("pattern.prev", json!({})));
    }
    x += 27.0;
    let pattern_rect = Rect::from_min_size(pos2(x, y), vec2(118.0, button_h));
    let pattern_title = format!("{} · {}", view.pattern_id, view.pattern_name);
    let _ = paint.button(
        "toolbar.pattern",
        pattern_rect,
        &pattern_title,
        false,
        false,
        "Pattern selecionado; renomear chega após editor de texto",
    );
    x += 120.0;
    let next = Rect::from_min_size(pos2(x, y), vec2(25.0, button_h));
    if paint
        .button(
            "toolbar.pattern.next",
            next,
            "+",
            false,
            true,
            "Próximo Pattern (+)",
        )
        .clicked()
    {
        actions.push(ToolbarAction::Engine("pattern.next", json!({})));
    }
    x += 33.0;
    for (name, kind, shortcut) in [
        ("Browser", None, "Alt+F8"),
        ("Rack", Some(WindowKind::ChannelRack), "F6"),
        ("Playlist", Some(WindowKind::Playlist), "F5"),
    ] {
        let width = if name == "Playlist" { 66.0 } else { 56.0 };
        let cell = Rect::from_min_size(pos2(x, y), vec2(width, button_h));
        if paint
            .button(
                &format!("toolbar.view.{name}"),
                cell,
                name,
                false,
                true,
                &format!("Mostrar/ocultar {name} ({shortcut})"),
            )
            .clicked()
        {
            actions.push(ToolbarAction::Workspace(
                kind.map_or(WsAction::ToggleBrowser, WsAction::Toggle),
            ));
        }
        x += width + 3.0;
    }
    if view.paused {
        paint.ui.painter().text(
            pos2(rect.left() + 8.0, rect.bottom() - 12.0),
            Align2::LEFT_CENTER,
            "Pausado · transporte sem áudio no M1",
            FontId::proportional(paint.layout.font_small),
            paint.tokens.muted,
        );
    }
    paint
        .probe
        .insert("toolbar.panel", rect, "toolbar", true, None);

    if let Some(index) = state.open_menu {
        paint_menu(paint, state, index, rect, &mut actions);
    }
    actions
}

fn paint_menu(
    paint: &mut PaintCtx<'_>,
    state: &mut ToolbarState,
    index: usize,
    toolbar_rect: Rect,
    actions: &mut Vec<ToolbarAction>,
) {
    let items: &[(&str, Option<ToolbarAction>)] = match index {
        0 => &[
            ("New", Some(ToolbarAction::Engine("file.new", json!({})))),
            ("Open...", Some(ToolbarAction::OpenProject)),
            ("Save", Some(ToolbarAction::SaveProject)),
            ("Save As...", Some(ToolbarAction::SaveProjectAs)),
        ],
        1 => &[
            (
                "Undo/Redo last edit",
                Some(ToolbarAction::Engine("edit.undo_toggle", json!({}))),
            ),
            (
                "Undo step-by-step",
                Some(ToolbarAction::Engine("edit.undo_step", json!({}))),
            ),
        ],
        4 => &[
            (
                "Playlist",
                Some(ToolbarAction::Workspace(WsAction::Toggle(
                    WindowKind::Playlist,
                ))),
            ),
            (
                "Channel rack",
                Some(ToolbarAction::Workspace(WsAction::Toggle(
                    WindowKind::ChannelRack,
                ))),
            ),
            (
                "Browser",
                Some(ToolbarAction::Workspace(WsAction::ToggleBrowser)),
            ),
        ],
        _ => &[("Ainda não implementado", None)],
    };
    let width = 214.0; // UNMEASURED: popup layout placeholder.
    let popup = Rect::from_min_size(
        pos2(
            toolbar_rect.left() + 7.0 + index as f32 * 55.0,
            toolbar_rect.top() + 29.0,
        ),
        vec2(width, items.len() as f32 * 25.0 + 6.0),
    );
    paint
        .ui
        .painter()
        .rect_filled(popup, 2.0, paint.tokens.raised);
    paint.ui.painter().rect_stroke(
        popup,
        2.0,
        Stroke::new(1.0, paint.tokens.border),
        StrokeKind::Inside,
    );
    for (row, (name, action)) in items.iter().enumerate() {
        let rect = Rect::from_min_size(
            pos2(popup.left() + 3.0, popup.top() + 3.0 + row as f32 * 25.0),
            vec2(width - 6.0, 24.0),
        );
        let response = paint.button(
            &format!("toolbar.menu-item.{index}.{row}"),
            rect,
            name,
            false,
            action.is_some(),
            if action.is_some() {
                name
            } else {
                "Funcionalidade ainda não implementada"
            },
        );
        if response.clicked()
            && let Some(action) = action
        {
            actions.push(match action {
                ToolbarAction::Engine(id, params) => ToolbarAction::Engine(id, params.clone()),
                ToolbarAction::Workspace(ws) => ToolbarAction::Workspace(*ws),
                ToolbarAction::OpenProject => ToolbarAction::OpenProject,
                ToolbarAction::SaveProject => ToolbarAction::SaveProject,
                ToolbarAction::SaveProjectAs => ToolbarAction::SaveProjectAs,
                ToolbarAction::Minimize => ToolbarAction::Minimize,
                ToolbarAction::Maximize => ToolbarAction::Maximize,
                ToolbarAction::Quit => ToolbarAction::Quit,
            });
            state.open_menu = None;
        }
    }
}
