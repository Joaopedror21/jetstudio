//! Main toolbar. Every rectangle below is measured from capture W (x, y relative to the toolbar's
//! top-left; see docs/UI_VISUAL_AUDIT.md §5). Controls without an implementation are painted
//! disabled and say so in the hint bar; none of them shows invented data.

use egui::{Align2, Event, Key, Rect, pos2, vec2};
use serde_json::{Value, json};

use crate::icons;
use crate::theme::{self, Tokens};
use crate::widgets::{PaintCtx, WindowGlyph, framed, lit_face, vgradient};
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
    pub tempo_milli: i64,
    pub position: String,
    pub pattern_name: String,
    pub title: String,
    pub error: Option<String>,
    /// Status line shown when nothing is hovered (audio device, last export).
    pub status: Option<String>,
    /// DSP load of the audio callback in percent; `None` without an audio device.
    pub cpu: Option<f32>,
    /// Sounding voices; `None` without an audio device.
    pub voices: Option<u32>,
    /// Recent Master output samples for the oscilloscope (empty without a device).
    pub scope: Vec<f32>,
    pub typing_keyboard: bool,
    pub metronome: bool,
    /// Recording armed; `recording` once audio is being captured.
    pub armed: bool,
    pub recording: bool,
}

pub enum ToolbarAction {
    Engine(&'static str, Value),
    Workspace(WsAction),
    OpenProject,
    SaveProject,
    SaveProjectAs,
    ExportWav,
    ExportMidi,
    AddChannel(&'static str),
    ToggleTyping,
    ToggleRecord,
    Minimize,
    Maximize,
    Quit,
}

/// Menu words with their measured left edges and widths in W (glyph extents at y 14-25).
const MENUS: [(&str, f32, f32); 8] = [
    ("FILE", 9.0, 20.0),
    ("EDIT", 36.0, 22.0),
    ("ADD", 62.0, 21.0),
    ("PATTERNS", 91.0, 48.0),
    ("VIEW", 145.0, 26.0),
    ("OPTIONS", 178.0, 45.0),
    ("TOOLS", 227.0, 33.0),
    ("HELP", 267.0, 25.0),
];

fn r(origin: egui::Pos2, x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
    Rect::from_min_max(origin + vec2(x0, y0), origin + vec2(x1, y1))
}

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
    let t = *paint.tokens;
    let o = rect.min;
    let painter = paint.painter();

    // Background: two 40 px rows separated by 1 px lines [W y 0, 41, 81].
    painter.rect_filled(rect, 0.0, t.toolbar);
    for y in [0.0, 41.0, 81.0] {
        painter.rect_filled(r(o, 0.0, y, rect.width(), y + 1.0), 0.0, t.toolbar_line);
    }
    painter.rect_filled(r(o, 0.0, 82.0, rect.width(), 83.0), 0.0, t.desktop_edge);
    painter.rect_filled(
        r(o, 0.0, 83.0, rect.width(), 84.0),
        0.0,
        t.desktop_highlight,
    );

    // Title drag area: the empty toolbar right of the panels moves the window.
    let drag_rect = r(o, 1210.0, 1.0, (rect.width() - 70.0).max(1211.0), 81.0);
    let drag = paint.ui.interact(
        drag_rect,
        egui::Id::new("toolbar.title-drag"),
        egui::Sense::click_and_drag(),
    );
    if drag.drag_started() {
        paint
            .ui
            .ctx()
            .send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
    if drag.double_clicked() {
        actions.push(ToolbarAction::Maximize);
    }
    paint
        .probe
        .insert("toolbar.title-drag", drag_rect, "title", true, None);

    // --- Row 1 -------------------------------------------------------------------------------
    // Menu panel [W 4-296, 4-36].
    framed(
        &painter,
        r(o, 4.0, 4.0, 297.0, 37.0),
        0.0,
        t.menu_panel,
        t.menu_panel_edge,
    );
    for (index, (name, x, w)) in MENUS.iter().enumerate() {
        let hit = r(o, x - 3.0, 5.0, x + w + 3.0, 36.0);
        let response = paint.hit(
            &format!("toolbar.menu.{name}"),
            hit,
            "menu",
            true,
            &format!("{name} menu"),
        );
        let open = state.open_menu == Some(index);
        if open || response.hovered() {
            // UNMEASURED: menu-title hover/open highlight not captured.
            painter.rect_filled(
                hit,
                0.0,
                theme::shade(t.menu_panel, if open { 1.3 } else { 1.12 }),
            );
        }
        // Fit each word to its measured width: Barlow Condensed is ~6-20% wider than FL's face.
        let base = theme::medium(paint.layout.font_menu);
        let natural = painter
            .layout_no_wrap((*name).to_owned(), base.clone(), t.menu_text)
            .size()
            .x;
        let size = if natural > *w + 1.0 {
            paint.layout.font_menu * (*w + 1.0) / natural
        } else {
            paint.layout.font_menu
        };
        painter.text(
            o + vec2(*x - 0.5, 20.5),
            Align2::LEFT_CENTER,
            *name,
            theme::medium(size),
            t.menu_text,
        );
        if response.clicked() {
            state.open_menu = if open { None } else { Some(index) };
        }
        if response.hovered() && state.open_menu.is_some() && !open {
            state.open_menu = Some(index);
        }
    }

    // PAT/SONG switch [W 306-336, 4-36].
    let mode = r(o, 306.0, 4.0, 337.0, 37.0);
    let mode_response = paint.hit(
        "toolbar.mode",
        mode,
        "toggle",
        true,
        "Pattern/song mode (L) - right-click opens the window",
    );
    painter.rect_filled(mode, 3.0, t.btn_outer);
    let pat = r(o, 307.0, 5.0, 336.0, 20.0);
    let song = r(o, 307.0, 20.0, 336.0, 36.0);
    for (half, label, on) in [
        (pat, "PAT", !view.song_mode),
        (song, "SONG", view.song_mode),
    ] {
        if on {
            lit_face(&painter, half, &t);
        } else {
            painter.rect_filled(half, 2.0, t.song_off);
        }
        painter.text(
            half.center(),
            Align2::CENTER_CENTER,
            label,
            theme::medium(11.0),
            if on {
                theme::shade(t.lit_edge, 0.5)
            } else {
                theme::shade(t.song_off, 1.55)
            },
        );
    }
    if mode_response.clicked() {
        actions.push(ToolbarAction::Engine("transport.mode.toggle", json!({})));
    }
    if mode_response.secondary_clicked() {
        actions.push(ToolbarAction::Workspace(WsAction::Toggle(
            if view.song_mode {
                WindowKind::Playlist
            } else {
                WindowKind::ChannelRack
            },
        )));
    }

    // Play / Stop group [W 337-401, 4-36].
    let group = r(o, 337.0, 4.0, 401.0, 37.0);
    painter.rect_filled(group, 6.0, t.transport_shadow);
    for (id, x0, x1, hint, command) in [
        (
            "toolbar.play",
            337.0,
            370.0,
            "Play / pause (Space)",
            "transport.play_stop",
        ),
        (
            "toolbar.stop",
            370.0,
            401.0,
            "Stop (Space)",
            "transport.stop",
        ),
    ] {
        let cell = r(o, x0, 4.0, x1, 36.0);
        let response = paint.hit(id, cell, "button", true, hint);
        let face = cell.shrink2(vec2(1.0, 1.0));
        vgradient(
            &painter,
            face,
            &[
                t.transport_face_top,
                t.transport_face_top,
                t.transport_face_bottom,
            ],
        );
        painter.rect_stroke(
            face,
            0.0,
            egui::Stroke::new(1.0, t.transport_edge),
            egui::StrokeKind::Inside,
        );
        let glyph_rect = cell.translate(vec2(0.0, -1.0));
        let color = if response.hovered() {
            t.btn_icon
        } else {
            theme::shade(t.btn_icon, 0.92)
        };
        if id == "toolbar.play" {
            if view.playing {
                icons::pause(&painter, glyph_rect, t.lit_mid);
            } else {
                icons::play(&painter, glyph_rect, color);
            }
        } else {
            icons::stop(&painter, glyph_rect, color);
        }
        if response.clicked() {
            actions.push(ToolbarAction::Engine(command, json!({})));
        }
    }

    // Record [W circle centred at 419,20, r 13].
    let rec = r(o, 405.0, 7.0, 433.0, 34.0);
    let record = paint.hit(
        "toolbar.record",
        rec,
        "button",
        true,
        if view.recording {
            "Recording audio (stop to place the take)"
        } else if view.armed {
            "Record armed: press Play to record audio (R)"
        } else {
            "Record (R): arm audio recording from the default input"
        },
    );
    painter.circle_filled(rec.center(), 13.5, t.btn_outer);
    painter.circle_filled(rec.center(), 12.0, t.record_ring);
    painter.circle_filled(
        rec.center(),
        if view.armed { 7.5 } else { 4.5 },
        if view.armed {
            egui::Color32::from_rgb(0xF0, 0x40, 0x30)
        } else {
            t.record_dot
        },
    );
    if record.clicked() {
        actions.push(ToolbarAction::ToggleRecord);
    }

    // Tempo LCD [W 437-490, 8-33]: integer part large, decimals small.
    let tempo_rect = r(o, 437.0, 8.0, 491.0, 33.0);
    let tempo_response = paint.hit(
        "toolbar.tempo",
        tempo_rect,
        "lcd",
        true,
        "Tempo (BPM) - click and type 10-522, Enter to confirm",
    );
    paint.lcd(tempo_rect);
    let (int_part, frac_part) = match &state.tempo_edit {
        Some(edit) => (edit.clone(), String::new()),
        None => (
            format!("{}", view.tempo_milli / 1000),
            format!(".{:03}", view.tempo_milli % 1000),
        ),
    };
    let int_rect = painter.text(
        o + vec2(443.0, 21.0),
        Align2::LEFT_CENTER,
        &int_part,
        theme::regular(paint.layout.font_lcd),
        t.lcd_text,
    );
    painter.text(
        pos2(int_rect.right(), o.y + 24.0),
        Align2::LEFT_CENTER,
        &frac_part,
        theme::regular(11.0),
        t.lcd_text,
    );
    icons::spin(
        &painter,
        r(o, 480.0, 13.0, 488.0, 28.0),
        theme::shade(t.lcd_edge, 0.9),
    );
    if tempo_response.clicked() {
        state.tempo_edit = Some(String::new());
    }
    edit_tempo(paint, state, &mut actions);

    // Time panel [W 500-628, 4-36].
    let time = r(o, 500.0, 4.0, 629.0, 37.0);
    framed(&painter, time, 0.0, t.time_bg, t.time_edge);
    let (big, small) = view
        .position
        .rsplit_once(':')
        .map_or((view.position.clone(), String::new()), |(a, b)| {
            (a.to_owned(), format!(":{b}"))
        });
    let big_rect = painter.text(
        o + vec2(536.0, 21.0),
        Align2::LEFT_CENTER,
        &big,
        theme::light(paint.layout.font_clock),
        t.time_text,
    );
    painter.text(
        pos2(big_rect.right(), o.y + 25.0),
        Align2::LEFT_CENTER,
        &small,
        theme::light(27.0),
        t.time_text,
    );
    painter.text(
        o + vec2(623.0, 9.0),
        Align2::RIGHT_CENTER,
        "B:S:T",
        theme::medium(8.0),
        t.time_text,
    );
    paint.probe.insert(
        "toolbar.position",
        time,
        "lcd",
        false,
        Some(view.position.clone()),
    );

    // Recording/metronome buttons [W 636, 669, 702, 735; 30x33]: typing keyboard and
    // metronome work; countdown and loop recording arrive with recording.
    let small_font = theme::medium(13.0);
    for (i, hint) in [
        "Typing keyboard to piano keyboard (Ctrl+T)",
        "Countdown before recording - arrives with recording",
        "Blend/loop recording - arrives with recording",
        "Metronome (Ctrl+M)",
    ]
    .iter()
    .enumerate()
    {
        let x = 636.0 + 33.0 * i as f32;
        let cell = r(o, x, 4.0, x + 30.0, 37.0);
        let f = small_font.clone();
        let (enabled, lit) = match i {
            0 => (true, view.typing_keyboard),
            3 => (true, view.metronome),
            _ => (false, false),
        };
        let clicked = paint
            .tool_button(
                &format!("toolbar.rec-option.{i}"),
                cell,
                lit,
                enabled,
                hint,
                |p, rr, c| match i {
                    0 => icons::keys(p, rr, c),
                    1 => icons::countdown(p, rr, c, f),
                    2 => icons::loop_record(p, rr, c),
                    _ => icons::metronome(p, rr, c),
                },
            )
            .clicked();
        if clicked && i == 0 {
            actions.push(ToolbarAction::ToggleTyping);
        }
        if clicked && i == 3 {
            actions.push(ToolbarAction::Engine(
                "transport.metronome.toggle",
                json!({}),
            ));
        }
    }

    // Empty panels: song-info [W 768-858], oscilloscope [W 862-1030], CPU/memory [W 1035-1206].
    framed(
        &painter,
        r(o, 768.0, 4.0, 859.0, 37.0),
        0.0,
        t.panel_dark,
        t.panel_edge,
    );
    painter.text(
        o + vec2(813.0, 20.0),
        Align2::CENTER_CENTER,
        "---",
        theme::regular(17.0),
        t.hint_text,
    );
    let scope = r(o, 862.0, 4.0, 1031.0, 37.0);
    framed(&painter, scope, 0.0, t.panel_dark, t.panel_edge);
    framed(
        &painter,
        r(o, 950.0, 5.0, 1030.0, 26.0),
        0.0,
        t.panel_darker,
        t.panel_edge,
    );
    // Oscilloscope of the Master output; silence (or no device) is a flat line.
    let trace = r(o, 864.0, 6.0, 948.0, 35.0);
    if view.scope.len() >= 2 {
        let n = view.scope.len();
        let points: Vec<egui::Pos2> = view
            .scope
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let x = trace.left() + trace.width() * i as f32 / (n - 1) as f32;
                let y = trace.center().y - v.clamp(-1.0, 1.0) * trace.height() * 0.5;
                pos2(x, y)
            })
            .collect();
        painter.add(egui::Shape::line(
            points,
            egui::Stroke::new(1.2, t.time_text),
        ));
    } else {
        painter.line_segment(
            [o + vec2(864.0, 20.0), o + vec2(948.0, 20.0)],
            egui::Stroke::new(1.5, t.time_text),
        );
    }
    // Visibility toggle at the scope's lower right [W 1019, 31].
    painter.circle_stroke(
        o + vec2(1019.0, 31.0),
        3.5,
        egui::Stroke::new(1.5, theme::shade(t.panel_dark, 0.7)),
    );
    paint
        .probe
        .insert("toolbar.scope", scope, "meter", false, None);
    // CPU/polyphony [W 1035-1121] and memory [W 1121-1206] sub-panels, each split in two rows.
    // Values are only shown when they are real: no audio engine runs, so polyphony is 0 and the
    // CPU bar is empty; memory use is not measured yet and stays blank.
    let cpu = r(o, 1035.0, 4.0, 1207.0, 37.0);
    framed(&painter, cpu, 0.0, t.panel_darker, t.panel_edge);
    for (x0, x1) in [(1037.0, 1120.0), (1122.0, 1205.0)] {
        framed(
            &painter,
            r(o, x0, 6.0, x1, 21.0),
            0.0,
            t.panel_darker,
            t.panel_edge,
        );
        framed(
            &painter,
            r(o, x0, 21.0, x1, 35.0),
            0.0,
            t.panel_darker,
            t.panel_edge,
        );
    }
    // CPU bar = audio callback time / buffer time; polyphony = sounding voices.
    let cpu_frac = view.cpu.map_or(0.0, |c| (c / 100.0).clamp(0.0, 1.0));
    painter.rect_filled(
        r(o, 1040.0, 8.0, 1042.0 + 50.0 * cpu_frac, 19.0),
        0.0,
        t.time_text,
    );
    painter.text(
        o + vec2(1117.0, 13.5),
        Align2::RIGHT_CENTER,
        view.voices
            .map_or_else(|| "0".to_owned(), |v| v.to_string()),
        theme::regular(15.0),
        t.time_text,
    );
    if let Some(cpu) = view.cpu {
        painter.text(
            o + vec2(1117.0, 28.0),
            Align2::RIGHT_CENTER,
            format!("{cpu:.0}%"),
            theme::regular(12.0),
            t.time_text,
        );
    }
    paint.probe.insert("toolbar.cpu", cpu, "meter", false, None);

    // Window controls [W 1502-1560, 12-28].
    let wx = rect.width() - 65.0;
    for (i, (glyph, hint, action)) in [
        (WindowGlyph::Minimize, "Minimize", ToolbarAction::Minimize),
        (WindowGlyph::Restore, "Restore", ToolbarAction::Maximize),
        (WindowGlyph::Close, "Close", ToolbarAction::Quit),
    ]
    .into_iter()
    .enumerate()
    {
        let x = wx + 20.0 * i as f32;
        let cell = r(o, x, 11.0, x + 19.0, 29.0);
        if paint
            .window_button(&format!("toolbar.window.{i}"), cell, glyph, true, hint)
            .clicked()
        {
            actions.push(action);
        }
    }

    // --- Row 2 -------------------------------------------------------------------------------
    // Hint panel [W 4-296, 44-78]: title line and hint/value line.
    let hint_rect = r(o, 4.0, 44.0, 297.0, 79.0);
    painter.rect_filled(hint_rect, 0.0, t.hint_panel);
    icons::cloud(
        &painter,
        r(o, 8.0, 48.0, 24.0, 62.0),
        theme::shade(t.hint_text, 0.8),
    );
    painter.text(
        o + vec2(30.0, 54.0),
        Align2::LEFT_CENTER,
        &view.title,
        theme::regular(14.0),
        t.hint_text,
    );
    // Errors win; otherwise the hovered control's hint, then the engine status line.
    let hint_text = match (&view.error, paint.hint.is_empty(), &view.status) {
        (Some(e), _, _) => format!("Error: {e}"),
        (None, false, _) => paint.hint.clone(),
        (None, true, Some(status)) => status.clone(),
        (None, true, None) => String::new(),
    };
    painter.text(
        o + vec2(9.0, 70.0),
        Align2::LEFT_CENTER,
        &hint_text,
        theme::regular(13.0),
        t.hint_text,
    );
    paint.probe.insert(
        "toolbar.hint",
        hint_rect,
        "hint",
        true,
        Some(paint.hint.clone()),
    );

    // Window shortcut buttons [W 304 + 33k, 45-77].
    type IconFn = fn(&egui::Painter, Rect, egui::Color32);
    let shortcuts: [(&str, Option<WsAction>, &str, IconFn); 7] = [
        (
            "browser",
            Some(WsAction::ToggleBrowser),
            "Browser (Alt+F8)",
            icons::browser,
        ),
        (
            "channel-rack",
            Some(WsAction::Toggle(WindowKind::ChannelRack)),
            "Channel rack (F6)",
            icons::channel_rack,
        ),
        (
            "piano-roll",
            Some(WsAction::Toggle(WindowKind::PianoRoll)),
            "Piano roll (F7)",
            icons::piano_roll,
        ),
        (
            "playlist",
            Some(WsAction::Toggle(WindowKind::Playlist)),
            "Playlist (F5)",
            icons::playlist,
        ),
        (
            "mixer",
            Some(WsAction::Toggle(WindowKind::Mixer)),
            "Mixer (F9)",
            icons::mixer,
        ),
        (
            "plugin-picker",
            None,
            "Plugin picker (F8) - arrives with plugin hosting",
            icons::arrow_right,
        ),
        (
            "audio-input",
            None,
            "Audio input - arrives with recording",
            icons::mic,
        ),
    ];
    for (i, (name, action, hint, icon)) in shortcuts.into_iter().enumerate() {
        let x = 304.0 + 33.0 * i as f32;
        let cell = r(o, x, 45.0, x + 30.0, 78.0);
        if paint
            .tool_button(
                &format!("toolbar.view.{name}"),
                cell,
                false,
                action.is_some(),
                hint,
                icon,
            )
            .clicked()
            && let Some(action) = action
        {
            actions.push(ToolbarAction::Workspace(action));
        }
    }

    // Snap selector [W magnet 537-551, field 555-617, 48-74].
    icons::magnet(
        &painter,
        r(o, 535.0, 52.0, 553.0, 70.0),
        theme::shade(t.btn_icon, 0.55),
    );
    let snap = r(o, 555.0, 48.0, 618.0, 75.0);
    let _ = paint.hit(
        "toolbar.snap",
        snap,
        "selector",
        false,
        "Global snap - Line (other values arrive with the editors)",
    );
    framed(&painter, snap, 2.0, t.field_dark, t.field_dark_edge);
    painter.text(
        o + vec2(560.0, 61.0),
        Align2::LEFT_CENTER,
        "Line",
        theme::regular(14.0),
        t.field_text,
    );
    icons::chevron_right(
        &painter,
        r(o, 605.0, 55.0, 615.0, 67.0),
        theme::shade(t.field_text, 0.6),
    );

    // Pattern selector [W 627-643 button, 644-746 LCD, 747-763 "+"].
    let prev = r(o, 627.0, 48.0, 644.0, 75.0);
    if paint
        .tool_button(
            "toolbar.pattern.prev",
            prev,
            false,
            true,
            "Pattern list (-/+ to step)",
            icons::chevron_right,
        )
        .clicked()
    {
        actions.push(ToolbarAction::Engine("pattern.prev", json!({})));
    }
    let pattern = r(o, 644.0, 48.0, 747.0, 75.0);
    let _ = paint.hit(
        "toolbar.pattern",
        pattern,
        "selector",
        true,
        "Selected pattern",
    );
    paint.lcd(pattern);
    painter.text(
        o + vec2(734.0, 61.0),
        Align2::RIGHT_CENTER,
        &view.pattern_name,
        theme::regular(paint.layout.font_lcd),
        t.lcd_text,
    );
    icons::spin(
        &painter,
        r(o, 735.0, 53.0, 745.0, 70.0),
        theme::shade(t.lcd_edge, 0.9),
    );
    let next = r(o, 747.0, 48.0, 764.0, 75.0);
    if paint
        .tool_button(
            "toolbar.pattern.next",
            next,
            false,
            true,
            "Next empty pattern (F4)",
            icons::plus,
        )
        .clicked()
    {
        actions.push(ToolbarAction::Engine("pattern.next_empty", json!({})));
    }

    // Touch modifier buttons [W 771 + 33k, 45-77] - not implemented.
    let modifier_font = theme::regular(15.0);
    for (i, label) in ["Shift", "Ctrl", "Alt"].iter().enumerate() {
        let x = 771.0 + 33.0 * i as f32;
        let f = modifier_font.clone();
        let _ = paint.tool_button(
            &format!("toolbar.touch.{label}"),
            r(o, x, 45.0, x + 30.0, 78.0),
            false,
            false,
            &format!("{label} (touch) - not implemented yet"),
            move |p, rr, c| {
                p.text(rr.center(), Align2::CENTER_CENTER, *label, f, c);
            },
        );
    }
    let help_font = theme::medium(13.0);
    let _ = paint.tool_button(
        "toolbar.help",
        r(o, 870.0, 45.0, 900.0, 78.0),
        false,
        false,
        "Help - not implemented yet",
        move |p, rr, c| {
            icons::help(p, rr, c, help_font);
        },
    );
    let _ = paint.tool_button(
        "toolbar.shop",
        r(o, 903.0, 45.0, 933.0, 78.0),
        false,
        false,
        "Online store - out of scope",
        icons::cart,
    );

    // News panel [W 938-1104, 44-78]: online content is out of scope, so it stays empty.
    framed(
        &painter,
        r(o, 938.0, 44.0, 1105.0, 79.0),
        0.0,
        t.time_bg,
        t.time_edge,
    );

    if let Some(index) = state.open_menu {
        paint_menu(paint, state, index, rect, &mut actions);
    }
    paint
        .probe
        .insert("toolbar.panel", rect, "toolbar", true, None);
    actions
}

fn edit_tempo(
    paint: &mut PaintCtx<'_>,
    state: &mut ToolbarState,
    actions: &mut Vec<ToolbarAction>,
) {
    let Some(mut input) = state.tempo_edit.take() else {
        return;
    };
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

fn paint_menu(
    paint: &mut PaintCtx<'_>,
    state: &mut ToolbarState,
    index: usize,
    toolbar_rect: Rect,
    actions: &mut Vec<ToolbarAction>,
) {
    let items: &[(&str, &str, Option<ToolbarAction>)] = match index {
        0 => &[
            (
                "New",
                "",
                Some(ToolbarAction::Engine("file.new", json!({}))),
            ),
            ("Open...", "Ctrl+O", Some(ToolbarAction::OpenProject)),
            ("Save", "Ctrl+S", Some(ToolbarAction::SaveProject)),
            (
                "Save as...",
                "Ctrl+Shift+S",
                Some(ToolbarAction::SaveProjectAs),
            ),
            (
                "Export > Wave file...",
                "Ctrl+R",
                Some(ToolbarAction::ExportWav),
            ),
            (
                "Export > MIDI file...",
                "Shift+Ctrl+M",
                Some(ToolbarAction::ExportMidi),
            ),
            (
                "Revert to last autosave",
                "",
                Some(ToolbarAction::Engine("file.revert_autosave", json!({}))),
            ),
        ],
        2 => &[
            (
                "Channel > Sampler",
                "",
                Some(ToolbarAction::AddChannel("sampler")),
            ),
            (
                "Channel > Synth",
                "",
                Some(ToolbarAction::AddChannel("synth")),
            ),
        ],
        1 => &[
            (
                "Undo",
                "Ctrl+Z",
                Some(ToolbarAction::Engine("edit.undo_toggle", json!({}))),
            ),
            (
                "Undo step",
                "Ctrl+Alt+Z",
                Some(ToolbarAction::Engine("edit.undo_step", json!({}))),
            ),
        ],
        4 => &[
            (
                "Playlist",
                "F5",
                Some(ToolbarAction::Workspace(WsAction::Toggle(
                    WindowKind::Playlist,
                ))),
            ),
            (
                "Channel rack",
                "F6",
                Some(ToolbarAction::Workspace(WsAction::Toggle(
                    WindowKind::ChannelRack,
                ))),
            ),
            (
                "Browser",
                "Alt+F8",
                Some(ToolbarAction::Workspace(WsAction::ToggleBrowser)),
            ),
            (
                "Piano roll",
                "F7",
                Some(ToolbarAction::Workspace(WsAction::Toggle(
                    WindowKind::PianoRoll,
                ))),
            ),
            (
                "Mixer",
                "F9",
                Some(ToolbarAction::Workspace(WsAction::Toggle(
                    WindowKind::Mixer,
                ))),
            ),
        ],
        5 => &[
            (
                "Audio settings",
                "F10",
                Some(ToolbarAction::Workspace(WsAction::Toggle(
                    WindowKind::AudioSettings,
                ))),
            ),
            (
                "Typing keyboard to piano",
                "Ctrl+T",
                Some(ToolbarAction::ToggleTyping),
            ),
            (
                "Metronome",
                "Ctrl+M",
                Some(ToolbarAction::Engine(
                    "transport.metronome.toggle",
                    json!({}),
                )),
            ),
        ],
        _ => &[("Not implemented yet", "", None)],
    };
    let t: Tokens = *paint.tokens;
    let row_h = 22.0; // UNMEASURED: no open menu captured yet (R0 scenario shell/toolbar).
    let width = 230.0; // UNMEASURED
    let x = MENUS.get(index).map_or(9.0, |m| m.1) - 4.0;
    let popup = Rect::from_min_size(
        pos2(toolbar_rect.left() + x, toolbar_rect.top() + 37.0),
        vec2(width, items.len() as f32 * row_h + 6.0),
    );
    let painter = paint.painter().with_layer_id(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("menu"),
    ));
    framed(&painter, popup, 0.0, t.hint_panel, t.btn_outer);
    for (row, (name, shortcut, action)) in items.iter().enumerate() {
        let item = Rect::from_min_size(
            pos2(popup.left() + 3.0, popup.top() + 3.0 + row as f32 * row_h),
            vec2(width - 6.0, row_h),
        );
        let id = format!("toolbar.menu-item.{index}.{row}");
        let response = paint.hit(
            &id,
            item,
            "menu-item",
            action.is_some(),
            if action.is_some() {
                name
            } else {
                "Not implemented yet"
            },
        );
        if response.hovered() && action.is_some() {
            painter.rect_filled(item, 0.0, theme::shade(t.menu_panel, 1.15));
        }
        let color = if action.is_some() {
            t.menu_text
        } else {
            t.disabled_glyph
        };
        painter.text(
            pos2(item.left() + 22.0, item.center().y),
            Align2::LEFT_CENTER,
            *name,
            theme::regular(15.0),
            color,
        );
        painter.text(
            pos2(item.right() - 8.0, item.center().y),
            Align2::RIGHT_CENTER,
            *shortcut,
            theme::regular(13.0),
            t.hint_text,
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
                ToolbarAction::ExportWav => ToolbarAction::ExportWav,
                ToolbarAction::ExportMidi => ToolbarAction::ExportMidi,
                ToolbarAction::AddChannel(kind) => ToolbarAction::AddChannel(kind),
                ToolbarAction::ToggleTyping => ToolbarAction::ToggleTyping,
                ToolbarAction::ToggleRecord => ToolbarAction::ToggleRecord,
                ToolbarAction::Minimize => ToolbarAction::Minimize,
                ToolbarAction::Maximize => ToolbarAction::Maximize,
                ToolbarAction::Quit => ToolbarAction::Quit,
            });
            state.open_menu = None;
        }
    }
}
