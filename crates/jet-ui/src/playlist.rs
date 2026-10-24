//! Playlist window body. Offsets are W pixels relative to the Playlist window's top-left corner
//! (W 500,84), measured in docs/UI_VISUAL_AUDIT.md §8.

use egui::{Align2, Id, PointerButton, Rect, Sense, Stroke, pos2, vec2};

use crate::icons;
use crate::theme;
use crate::widgets::{PaintCtx, hatch, vgradient};

pub struct PlaylistView {
    /// Milliseconds per tick at the project tempo (Audio Clip fades are in ms).
    pub ms_per_tick: f64,
    pub pattern_id: u64,
    pub arrangement_name: String,
    pub ticks_per_bar: i64,
    pub beats_per_bar: u32,
    /// The toolbar's Global snap.
    pub global_snap: crate::snap::SnapValue,
    /// Typing keyboard to piano is on: plain letters play notes instead of choosing tools.
    pub typing: bool,
    pub step_ticks: i64,
    pub position: Option<i64>,
    pub tracks: Vec<TrackView>,
    pub clips: Vec<ClipView>,
    pub patterns: Vec<PatternEntry>,
    /// A Browser file being dragged, if any (dropped on the grid it becomes an Audio Clip).
    pub dragging_file: Option<std::path::PathBuf>,
    /// (id, name) of every arrangement; the active one is `arrangement_id`.
    pub arrangements: Vec<(u64, String)>,
    pub arrangement_id: u64,
    /// Time markers of the active arrangement: (tick, name).
    pub markers: Vec<(i64, String, jet_model::MarkerAction)>,
    /// Timeline selection (start, end) in ticks.
    pub selection: Option<(i64, i64)>,
}

pub struct TrackView {
    pub id: u64,
    pub name: String,
    pub muted: bool,
}

pub struct ClipView {
    pub id: u64,
    pub track: u64,
    pub start: i64,
    pub length: i64,
    /// Content offset in ticks (Slip edits it).
    pub offset: i64,
    pub label: String,
    pub color: Option<u32>,
    pub muted: bool,
    /// For Automation Clips: the clip definition id, its control points and the clip offset.
    pub automation: Option<AutomationView>,
    /// For Audio Clips: waveform columns spanning the whole sample, its length in ticks and the
    /// clip offset.
    pub waveform: Option<ClipWaveform>,
    /// Audio Clip instance gain, fades and reverse.
    pub audio: Option<jet_model::AudioClipProps>,
}

/// (peak columns over the whole sample, sample length in ticks, clip offset in ticks).
pub type ClipWaveform = (Vec<(f32, f32)>, f64, i64);

#[derive(Clone)]
pub struct AutomationView {
    pub id: u64,
    pub points: Vec<jet_model::AutoPoint>,
    pub offset: i64,
    /// Min/Max/Time/LFO, for the preview drawn in the clip.
    pub settings: jet_model::AutoSettings,
    pub beat_ticks: i64,
}

/// Curve types in the order of the manual's Curve Type Menu [Manual: playlist_automationclip].
pub const CURVES: [(jet_model::CurveShape, &str); 10] = [
    (jet_model::CurveShape::SingleCurve, "Single curve"),
    (jet_model::CurveShape::DoubleCurve, "Double curve"),
    (jet_model::CurveShape::AltSingleCurve, "Alt single curve"),
    (jet_model::CurveShape::AltDoubleCurve, "Alt double curve"),
    (jet_model::CurveShape::Hold, "Hold"),
    (jet_model::CurveShape::Stairs, "Stairs"),
    (jet_model::CurveShape::SmoothStairs, "Smooth stairs"),
    (jet_model::CurveShape::Pulse, "Pulse"),
    (jet_model::CurveShape::Wave, "Wave"),
    (jet_model::CurveShape::HalfSine, "Half sine"),
];

pub struct PatternEntry {
    pub id: u64,
    pub name: String,
    /// Pattern length in ticks: the length of a newly placed clip.
    pub length: i64,
}

/// A clip gesture in progress; committed as one command when the mouse is released.
#[derive(Clone, Copy, Debug)]
enum ClipDrag {
    Move {
        id: u64,
        anchor: egui::Pos2,
        dt: i64,
        drow: i64,
    },
    Resize {
        id: u64,
        anchor: egui::Pos2,
        dl: i64,
    },
    /// Slip tool: the clip's content moves by `d` ticks (its offset decreases by `d`).
    Slip { id: u64, anchor: egui::Pos2, d: i64 },
}

/// Playlist tools [Manual: playlist, "Tool Bar icons"]: Draw (P), Paint (B), Delete (D), Mute (T),
/// Slip (S), Slice (C), Select (E), Play (Y).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlTool {
    #[default]
    Draw,
    Paint,
    Delete,
    Mute,
    Slip,
    Slice,
    Select,
    Play,
}

impl PlTool {
    const KEYS: [(PlTool, egui::Key); 8] = [
        (PlTool::Draw, egui::Key::P),
        (PlTool::Paint, egui::Key::B),
        (PlTool::Delete, egui::Key::D),
        (PlTool::Mute, egui::Key::T),
        (PlTool::Slip, egui::Key::S),
        (PlTool::Slice, egui::Key::C),
        (PlTool::Select, egui::Key::E),
        (PlTool::Play, egui::Key::Y),
    ];
}

/// A drag across the grid that is not on a clip.
#[derive(Clone, Copy, Debug)]
enum GridDrag {
    /// Select tool or Ctrl: a rectangle from `from` to `to`.
    Rect { from: egui::Pos2, to: egui::Pos2 },
    /// Slice tool: a vertical cut at `x` from `from_y` to `to_y`.
    Slice { x: f32, from_y: f32, to_y: f32 },
}

#[derive(Clone)]
pub struct PlaylistState {
    pub selected_track: Option<u64>,
    pub tool: PlTool,
    /// Selected clips (Select tool, Ctrl+drag, Shift+click).
    pub selection: std::collections::BTreeSet<u64>,
    /// The Playlist window has the keyboard (set by the host each frame).
    pub focused: bool,
    grid_drag: Option<GridDrag>,
    /// Delete/Mute sweep in progress: the mute value to set (None = delete) and the clips done.
    sweep: Option<(Option<bool>, std::collections::BTreeSet<u64>)>,
    /// Paint tool drag: (row, starts painted so far).
    painting: Option<(usize, Vec<i64>)>,
    /// Zoom and scroll before Shift+4 zoomed out, to switch back.
    zoom_before_out: Option<(f32, f32)>,
    drag: Option<ClipDrag>,
    /// Control point being dragged: (automation id, index, tick, value).
    point_drag: Option<(u64, usize, i64, u16)>,
    /// Control point menu: (automation id, point index, where it opened).
    point_menu: Option<(u64, usize, egui::Pos2)>,
    /// Tension handle drag: (automation id, point index, tension at the start, tension now).
    tension_drag: Option<(u64, usize, i16, i16)>,
    /// Audio Clip handle drag: (clip id, handle, value being set).
    audio_drag: Option<(u64, AudioHandle, i32)>,
    /// Audio Clip menu: (clip id, where it opened).
    audio_menu: Option<(u64, egui::Pos2)>,
    /// Arrangements menu opened from the title bar.
    arrangement_menu: Option<egui::Pos2>,
    /// "Merge with" options for a source arrangement: (source id, where it opened).
    merge_menu: Option<(u64, egui::Pos2)>,
    /// Ctrl+drag on the ruler in progress: (anchor tick, current tick).
    selecting: Option<(i64, i64)>,
    /// Marker menu: (marker index, where it opened).
    marker_menu: Option<(usize, egui::Pos2)>,
    /// Horizontal zoom as a multiple of the measured default bar width.
    pub zoom: f32,
    pub scroll_bars: f32,
    pub scroll_tracks: f32,
}

impl Default for PlaylistState {
    fn default() -> Self {
        Self {
            selected_track: None,
            tool: PlTool::Draw,
            selection: std::collections::BTreeSet::new(),
            focused: false,
            grid_drag: None,
            sweep: None,
            painting: None,
            zoom_before_out: None,
            drag: None,
            point_drag: None,
            point_menu: None,
            tension_drag: None,
            audio_drag: None,
            audio_menu: None,
            arrangement_menu: None,
            merge_menu: None,
            selecting: None,
            marker_menu: None,
            zoom: 1.0,
            scroll_bars: 0.0,
            scroll_tracks: 0.0,
        }
    }
}

pub enum PlaylistAction {
    AddTrack,
    SelectTrack(u64),
    SelectPattern(u64),
    /// Place the selected pattern on display row `row` (tracks are created up to that row).
    AddClipOnRow {
        row: usize,
        pattern: u64,
        start: i64,
        length: i64,
    },
    ResizeClip {
        id: u64,
        length: i64,
    },
    DeleteClip(u64),
    /// Several clips deleted as one undo step.
    DeleteClips(Vec<u64>),
    /// Several clips moved as one undo step: (id, display row, start).
    MoveClips(Vec<(u64, usize, i64)>),
    SliceClip {
        id: u64,
        at: i64,
    },
    SlipClip {
        id: u64,
        offset: i64,
    },
    /// Play tool: start playback (Song mode) from this tick.
    PlayFrom(i64),
    /// Double-click on an Automation Clip opens its settings.
    OpenAutomation(u64),
    MuteClip {
        id: u64,
        muted: bool,
    },
    MuteTrack {
        id: u64,
        muted: bool,
    },
    SoloTrack(u64),
    Seek(i64),
    /// An audio file dropped on display row `row` at tick `start`.
    DropAudio {
        row: usize,
        start: i64,
        path: std::path::PathBuf,
    },
    SelectArrangement(u64),
    AddArrangement,
    CloneArrangement,
    DeleteArrangement(u64),
    MergeArrangement {
        source: u64,
        position: &'static str,
        mode: &'static str,
    },
    AddMarker,
    DeleteMarker(usize),
    MarkerAction {
        index: usize,
        action: jet_model::MarkerAction,
    },
    SetSelection {
        start: i64,
        end: i64,
    },
    ClearSelection,
    /// Audio Clip instance settings: `None` fields stay as they are.
    ClipAudio {
        id: u64,
        gain_cdb: Option<i16>,
        fade_in_ms: Option<u32>,
        fade_out_ms: Option<u32>,
        reverse: Option<bool>,
    },
    AutoAddPoint {
        id: u64,
        at: i64,
        value: u16,
    },
    AutoMovePoint {
        id: u64,
        index: usize,
        at: i64,
        value: u16,
    },
    AutoDeletePoint {
        id: u64,
        index: usize,
    },
    AutoShape {
        id: u64,
        index: usize,
        shape: jet_model::CurveShape,
    },
    /// Tension of the segment ending at point `index`.
    AutoTension {
        id: u64,
        index: usize,
        tension: i16,
    },
}

// Clip edits follow the Global snap (the Playlist's local snap is Main); the Playlist always
// draws beat lines, so Line and Cell are one beat. Alt bypasses snap [Manual: playlist, "Holding
// the Alt key temporarily sets snap to 'none'"].

/// Title-bar tool icons [W x 510-740 relative to 500].
pub fn paint_title_tools(
    paint: &mut PaintCtx<'_>,
    state: &mut PlaylistState,
    title: Rect,
    view: &PlaylistView,
) {
    let t = *paint.tokens;
    let painter = paint.painter();
    type IconFn = fn(&egui::Painter, Rect, egui::Color32);
    let tools: [(f32, IconFn, &str, Option<PlTool>); 11] = [
        (13.0, icons::chevron_right, "Playlist options", None),
        (34.0, icons::magnet, "Snap - Main (the Global snap)", None),
        (66.0, icons::pencil, "Draw (P)", Some(PlTool::Draw)),
        (88.0, icons::brush, "Paint (B)", Some(PlTool::Paint)),
        (108.0, icons::delete, "Delete (D)", Some(PlTool::Delete)),
        (128.0, icons::mute, "Mute (T)", Some(PlTool::Mute)),
        (150.0, icons::slip, "Slip (S)", Some(PlTool::Slip)),
        (171.0, icons::slice, "Slice (C)", Some(PlTool::Slice)),
        (192.0, icons::select, "Select (E)", Some(PlTool::Select)),
        (
            213.0,
            icons::zoom,
            "Zoom - Ctrl+wheel zooms (the zoom tool is not implemented yet)",
            None,
        ),
        (
            235.0,
            icons::playback,
            "Play selected (Y)",
            Some(PlTool::Play),
        ),
    ];
    for (x, icon, hint, tool) in tools {
        let cell =
            Rect::from_center_size(pos2(title.left() + x, title.center().y), vec2(19.0, 20.0));
        let id = match tool {
            Some(tool) => format!("playlist.tool.{tool:?}").to_lowercase(),
            None => format!("playlist.tool.{x}"),
        };
        if paint.hit(&id, cell, "tool", tool.is_some(), hint).clicked()
            && let Some(tool) = tool
        {
            state.tool = tool;
        }
        // [W] The active tool (Draw in the capture) is drawn in the full title-icon colour.
        let color = if tool == Some(state.tool) {
            t.title_icon
        } else {
            theme::shade(t.title_icon, 0.82)
        };
        icon(&painter, cell, color);
    }
    icons::title_speaker(
        &painter,
        Rect::from_center_size(
            pos2(title.left() + 270.0, title.center().y),
            vec2(18.0, 18.0),
        ),
        t.title_icon,
    );
    // "Playlist - <arrangement> ▸ (none) ▸" with small triangles as separators [W 782-985].
    let font = theme::regular(paint.layout.font_title);
    let mut x = title.left() + 282.0;
    for (index, part) in [
        format!("Playlist - {}", view.arrangement_name),
        "(none)".to_owned(),
    ]
    .into_iter()
    .enumerate()
    {
        let end = painter.text(
            pos2(x, title.center().y),
            Align2::LEFT_CENTER,
            part,
            font.clone(),
            t.title_text,
        );
        // The arrangement name opens the Arrangements menu [Manual: playlist, "Arrangements"].
        if index == 0
            && paint
                .hit(
                    "playlist.arrangements",
                    end.expand2(vec2(2.0, 3.0)),
                    "menu",
                    true,
                    "Arrangements: switch, add, clone or delete",
                )
                .clicked()
        {
            state.arrangement_menu = Some(pos2(end.left(), title.bottom()));
        }
        icons::chevron_right(
            &painter,
            Rect::from_center_size(
                pos2(end.right() + 8.0, title.center().y + 1.0),
                vec2(8.0, 10.0),
            ),
            theme::shade(t.title_text, 0.55),
        );
        x = end.right() + 17.0;
    }
}

/// `rect` is the whole Playlist window (title bar included); the body starts at `title_h`.
pub fn paint_playlist(
    paint: &mut PaintCtx<'_>,
    state: &mut PlaylistState,
    view: &PlaylistView,
    rect: Rect,
) -> Vec<PlaylistAction> {
    let mut actions = Vec::new();
    let t = *paint.tokens;
    let l = *paint.layout;
    let o = rect.min;
    let at =
        |x0: f32, y0: f32, x1: f32, y1: f32| Rect::from_min_max(o + vec2(x0, y0), o + vec2(x1, y1));
    let painter = paint.painter().with_clip_rect(rect);
    let w = rect.width();
    let h = rect.height();
    let picker_r = l.picker_w;
    let header_l = picker_r + l.picker_scroll_w; // 160
    let header_r = header_l + l.track_header_w; // 261
    let grid_l = header_r + l.track_gutter_w; // 271
    let grid_r = w - l.vscroll_w;
    let bar_w = l.bar_w * state.zoom;

    // Body background and the upper band [W y 106-147].
    painter.rect_filled(at(0.0, l.title_h, w, h), 0.0, t.pl_title);
    painter.rect_filled(
        at(0.0, l.title_h, w, l.title_h + 1.0),
        0.0,
        t.pl_strip_shadow,
    );

    // Picker header icons [W 515-530 / 565 / 615 at y 130].
    icons::keys(&painter, at(14.0, 38.0, 34.0, 56.0), t.title_icon);
    icons::waveform(
        &painter,
        at(60.0, 38.0, 82.0, 56.0),
        theme::shade(t.title_icon, 0.45),
    );
    icons::automation(
        &painter,
        at(108.0, 38.0, 130.0, 56.0),
        theme::shade(t.title_icon, 0.45),
    );

    // Raised tab over the track headers with a slanted left edge [W 660-770, 108-147], holding
    // the clip-source icons [W 707/735/757, y 122] and the "+" / NOTE CHAN PAT row.
    painter.add(egui::Shape::convex_polygon(
        vec![
            pos2(o.x + header_l, o.y + l.tracks_top - 1.0),
            pos2(o.x + header_l + 18.0, o.y + l.strip_top + 2.0),
            pos2(o.x + header_l + 24.0, o.y + l.strip_top - 1.0),
            pos2(o.x + grid_l - 1.0, o.y + l.strip_top - 1.0),
            pos2(o.x + grid_l - 1.0, o.y + l.tracks_top - 1.0),
        ],
        t.track_highlight,
        Stroke::new(1.0, t.pl_strip_shadow),
    ));
    icons::waveform(
        &painter,
        at(header_l + 37.0, 26.0, header_l + 57.0, 42.0),
        theme::shade(t.title_icon, 0.45),
    );
    icons::automation(
        &painter,
        at(header_l + 64.0, 26.0, header_l + 86.0, 42.0),
        theme::shade(t.title_icon, 0.45),
    );
    icons::keys(
        &painter,
        at(header_l + 88.0, 26.0, header_l + 106.0, 42.0),
        t.title_icon,
    );

    // Track-header header: "+" and NOTE / CHAN / PAT [W 660-770, 128-145].
    let add_track = at(header_l + 10.0, 40.0, header_l + 28.0, 60.0);
    if paint
        .hit(
            "playlist.add-track",
            add_track,
            "button",
            true,
            "Add a playlist track",
        )
        .clicked()
    {
        actions.push(PlaylistAction::AddTrack);
    }
    icons::plus(&painter, add_track, t.title_icon);
    for (label, x, bright) in [
        ("NOTE", header_l + 37.0, false),
        ("CHAN", header_l + 63.0, false),
        ("PAT", header_l + 89.0, true),
    ] {
        painter.text(
            o + vec2(x, 53.0),
            Align2::CENTER_CENTER,
            label,
            theme::medium(l.font_small),
            if bright {
                t.hdr_label
            } else {
                theme::shade(t.hdr_label, 0.6)
            },
        );
    }

    // Overview strip [W 770-1545, 107-125] with scroll buttons.
    let strip = at(
        grid_l - 1.0,
        l.strip_top - 1.0,
        grid_r,
        l.strip_top + l.strip_h + 1.0,
    );
    painter.rect_filled(strip, 0.0, t.pl_strip_edge);
    painter.rect_filled(strip.shrink2(vec2(0.0, 1.0)), 0.0, t.pl_strip);
    for (x0, x1, left) in [
        (grid_l, grid_l + 20.0, true),
        (grid_r - 18.0, grid_r, false),
    ] {
        let b = at(x0, l.strip_top - 1.0, x1, l.strip_top + l.strip_h + 1.0);
        painter.rect_filled(b, 0.0, t.pl_strip_shadow);
        if left {
            icons::chevron_left_line(&painter, b, t.title_icon);
        } else {
            icons::chevron_right_line(&painter, b, t.title_icon);
        }
    }

    // Ruler [W 771-1546, 128-144] and its lower edge [W 145-146].
    let ruler = at(grid_l, l.ruler_top, grid_r, l.ruler_top + l.ruler_h);
    painter.rect_filled(at(grid_l, l.ruler_top, w, l.tracks_top), 0.0, t.ruler_bg);
    painter.rect_filled(
        at(grid_l, l.tracks_top - 3.0, w, l.tracks_top - 2.0),
        0.0,
        t.pl_strip_shadow,
    );
    painter.rect_filled(
        at(grid_l, l.tracks_top - 2.0, w, l.tracks_top - 1.0),
        0.0,
        t.ruler_edge,
    );

    // Picker panel [W 500-638] and its hatched scrollbar [W 639-659].
    let picker = at(0.0, l.tracks_top, picker_r, h);
    painter.rect_filled(picker, 0.0, t.picker_bg);
    hatch(
        &painter,
        at(picker_r, l.tracks_top, header_l, h),
        t.hatch_a,
        t.hatch_b,
    );
    // Scrollbar thumb button at the top [W 641-652, 150-158].
    let thumb = at(
        picker_r + 2.0,
        l.tracks_top + 1.0,
        header_l - 4.0,
        l.tracks_top + 12.0,
    );
    painter.rect_filled(thumb, 1.0, t.track_highlight);
    painter.rect_filled(
        Rect::from_center_size(thumb.center(), vec2(3.0, 4.0)),
        0.0,
        t.picker_text,
    );
    for (i, pattern) in view.patterns.iter().enumerate() {
        let item = at(
            8.0,
            l.tracks_top + 3.0 + 26.0 * i as f32,
            picker_r - 2.0,
            l.tracks_top + 28.0 + 26.0 * i as f32,
        );
        if item.top() > rect.bottom() {
            break;
        }
        let response = paint.hit(
            &format!("playlist.picker.{}", pattern.id),
            item,
            "pattern",
            true,
            &format!("Pattern: {}", pattern.name),
        );
        vgradient(
            &painter,
            item,
            &[t.picker_item_top, t.picker_item, t.picker_item],
        );
        if pattern.id == view.pattern_id {
            painter.add(egui::Shape::convex_polygon(
                vec![
                    pos2(o.x + 1.0, item.center().y - 6.0),
                    pos2(o.x + 7.0, item.center().y),
                    pos2(o.x + 1.0, item.center().y + 6.0),
                ],
                t.picker_marker,
                Stroke::NONE,
            ));
            icons::chevron_right(
                &painter,
                Rect::from_center_size(pos2(o.x + 10.5, item.center().y), vec2(5.0, 6.0)),
                t.picker_text,
            );
        }
        painter.text(
            pos2(item.left() + 9.0, item.center().y),
            Align2::LEFT_CENTER,
            &pattern.name,
            theme::medium(15.0),
            t.picker_text,
        );
        if response.clicked() {
            actions.push(PlaylistAction::SelectPattern(pattern.id));
        }
    }

    // Scroll/zoom input over the track area.
    let grid = at(grid_l, l.tracks_top, grid_r, h);
    let headers = at(header_l, l.tracks_top, header_r, h);
    if paint.ui.rect_contains_pointer(grid) || paint.ui.rect_contains_pointer(headers) {
        let (delta, ctrl, shift) = paint
            .ui
            .input(|i| (i.smooth_scroll_delta.y, i.modifiers.ctrl, i.modifiers.shift));
        if ctrl && delta != 0.0 {
            let anchor = paint
                .ui
                .input(|i| i.pointer.hover_pos())
                .map_or(grid.center().x, |p| p.x);
            zoom_at(
                state,
                l.bar_w,
                grid.left(),
                anchor,
                if delta > 0.0 { 1.1 } else { 0.9 },
            );
        } else if shift {
            state.scroll_bars = (state.scroll_bars - delta / bar_w).max(0.0);
        } else {
            state.scroll_tracks = (state.scroll_tracks - delta / l.track_h).max(0.0);
        }
    }
    // Zoom keys [Manual: playlist, "Zoom in/ Zoom out (Page up/Page down) - Zoom centers on the
    // Cursor position"; "Quick-zoom 1, 2 & 3 (Shift+1, 2, 3)"; "Zoom out (Shift+4) - Switches
    // between 'zoom out' and previous zoom level"; "On Selection (Shift+5)"]. The three quick
    // levels (2, 1 and 0.5 bars per default bar width) are ours (H-PL-3).
    if state.focused {
        let (page_up, page_down, digit, pointer) = paint.ui.input(|i| {
            let shift = i.modifiers.shift && !i.modifiers.ctrl;
            let digit = [
                egui::Key::Num1,
                egui::Key::Num2,
                egui::Key::Num3,
                egui::Key::Num4,
                egui::Key::Num5,
            ]
            .iter()
            .position(|k| shift && i.key_pressed(*k));
            (
                i.key_pressed(egui::Key::PageUp),
                i.key_pressed(egui::Key::PageDown),
                digit,
                i.pointer.hover_pos(),
            )
        });
        let anchor = pointer
            .filter(|p| grid.contains(*p))
            .map_or(grid.center().x, |p| p.x);
        if page_up {
            zoom_at(state, l.bar_w, grid.left(), anchor, 1.25);
        }
        if page_down {
            zoom_at(state, l.bar_w, grid.left(), anchor, 0.8);
        }
        let tpb = view.ticks_per_bar.max(1) as f32;
        let fit = |state: &mut PlaylistState, from: i64, to: i64| {
            let bars = ((to - from).max(1) as f32 / tpb).max(0.25);
            state.zoom = (grid.width() / bars / l.bar_w).clamp(0.25, 6.0);
            state.scroll_bars = (from as f32 / tpb).max(0.0);
        };
        match digit {
            Some(level @ 0..=2) => {
                let zoom = [2.0, 1.0, 0.5][level];
                zoom_at(state, l.bar_w, grid.left(), anchor, zoom / state.zoom);
            }
            Some(3) => match state.zoom_before_out.take() {
                Some((zoom, scroll)) => {
                    state.zoom = zoom;
                    state.scroll_bars = scroll;
                }
                None => {
                    state.zoom_before_out = Some((state.zoom, state.scroll_bars));
                    let end = view
                        .clips
                        .iter()
                        .map(|c| c.start + c.length)
                        .max()
                        .unwrap_or(view.ticks_per_bar);
                    fit(state, 0, end);
                }
            },
            Some(_) => {
                // The selected clips, else the timeline selection.
                let selected: Vec<&ClipView> = view
                    .clips
                    .iter()
                    .filter(|c| state.selection.contains(&c.id))
                    .collect();
                let range = if selected.is_empty() {
                    view.selection
                } else {
                    Some((
                        selected.iter().map(|c| c.start).min().unwrap_or(0),
                        selected
                            .iter()
                            .map(|c| c.start + c.length)
                            .max()
                            .unwrap_or(0),
                    ))
                };
                if let Some((from, to)) = range {
                    fit(state, from, to);
                }
            }
            None => {}
        }
    }
    let bar_w = l.bar_w * state.zoom;
    let pan = paint
        .ui
        .interact(grid, Id::new("playlist.pan"), Sense::click_and_drag());
    if pan.dragged_by(PointerButton::Middle) {
        let delta = paint.ui.input(|i| i.pointer.delta());
        state.scroll_bars = (state.scroll_bars - delta.x / bar_w).max(0.0);
        state.scroll_tracks = (state.scroll_tracks - delta.y / l.track_h).max(0.0);
    }

    // Grid background: 4-bar shading, beat lines, bar lines [W y 175].
    let grid_painter = painter.with_clip_rect(grid);
    let ruler_painter = painter.with_clip_rect(ruler);
    let beats = view.beats_per_bar.max(1) as usize;
    let first_bar = state.scroll_bars.floor() as usize;
    let bar_count = (grid.width() / bar_w).ceil() as usize + 2;
    let scroll_bars = state.scroll_bars;
    let bar_x = |bar: f32| grid.left() + 1.0 + (bar - scroll_bars) * bar_w;
    for bar in first_bar..first_bar.saturating_add(bar_count) {
        let x = bar_x(bar as f32);
        let dark = (bar / 4) % 2 == 1;
        grid_painter.rect_filled(
            Rect::from_min_max(pos2(x, grid.top()), pos2(x + bar_w, grid.bottom())),
            0.0,
            if dark { t.grid_b } else { t.grid_a },
        );
        for beat in 1..beats {
            let bx = (x + bar_w * beat as f32 / beats as f32).round();
            grid_painter.rect_filled(
                Rect::from_min_max(pos2(bx, grid.top()), pos2(bx + 1.0, grid.bottom())),
                0.0,
                if dark { t.beat_b } else { t.beat_a },
            );
        }
        let lx = x.round();
        grid_painter.rect_filled(
            Rect::from_min_max(pos2(lx, grid.top()), pos2(lx + 1.0, grid.bottom())),
            0.0,
            if dark || bar % 4 == 0 {
                t.bar_b
            } else {
                t.bar_a
            },
        );
        ruler_painter.text(
            pos2(lx + 3.0, ruler.center().y + 0.5),
            Align2::LEFT_CENTER,
            (bar + 1).to_string(),
            theme::regular(13.0),
            t.ruler_text,
        );
    }

    // Track rows: model tracks first, then placeholders named like FL's empty tracks.
    let first_row = state.scroll_tracks.floor() as usize;
    let rows_visible = (grid.height() / l.track_h).ceil() as usize + 1;
    let mut row_responses: Vec<(usize, Option<u64>, egui::Response)> = Vec::new();
    for row in first_row..first_row + rows_visible {
        let y = grid.top() + (row as f32 - state.scroll_tracks) * l.track_h;
        let track = view.tracks.get(row);
        let header =
            Rect::from_min_max(pos2(o.x + header_l, y), pos2(o.x + header_r, y + l.track_h));
        // Header face [W 660-760]: dark left edge, rounded right corners, lighter top band;
        // gutter [W 762-770].
        painter.rect_filled(
            Rect::from_min_max(
                pos2(header.right() - 8.0, y),
                pos2(o.x + grid_l, y + l.track_h),
            ),
            0.0,
            t.track_gutter,
        );
        painter.rect_filled(
            Rect::from_min_max(header.min, pos2(header.left() + 1.0, header.bottom())),
            0.0,
            t.track_edge,
        );
        let face = Rect::from_min_max(
            pos2(header.left() + 1.0, y + 1.0),
            pos2(header.right(), y + l.track_h - 1.0),
        );
        painter.rect_filled(
            face,
            egui::CornerRadius {
                nw: 0,
                ne: 7,
                sw: 0,
                se: 7,
            },
            t.track_face,
        );
        painter.rect_filled(
            Rect::from_min_max(face.min, pos2(face.right(), y + 10.0)),
            egui::CornerRadius {
                nw: 0,
                ne: 7,
                sw: 0,
                se: 0,
            },
            t.track_face_top,
        );
        painter.rect_filled(
            Rect::from_min_max(face.min, pos2(face.left() + 1.0, face.bottom())),
            0.0,
            t.track_highlight,
        );
        let name = track.map_or_else(|| format!("Track {}", row + 1), |tr| tr.name.clone());
        let muted = track.is_some_and(|tr| tr.muted);
        let probe_id = track.map_or_else(
            || format!("playlist.track.placeholder.{}", row + 1),
            |tr| format!("playlist.track.{}", tr.id),
        );
        let response = paint.hit(&probe_id, header, "track", true, &name);
        painter.text(
            pos2(header.left() + 3.0, y + 8.0),
            Align2::LEFT_CENTER,
            &name,
            theme::regular(15.0),
            t.track_text,
        );
        painter.text(
            pos2(header.left() + 22.0, y + 33.0),
            Align2::CENTER_CENTER,
            "...",
            theme::regular(12.0),
            theme::shade(t.track_text, 0.8),
        );
        let led_center = pos2(o.x + header_l + 90.0, y + 37.0);
        paint.led(led_center, 5.5, !muted);
        if let Some(tr) = track {
            // Track mute: click; solo: Ctrl+click or right-click [Manual: playlist, "Track Mute/Solo"].
            let led_rect = Rect::from_center_size(led_center, vec2(14.0, 14.0));
            let led = paint.hit(
                &format!("playlist.track-mute.{}", tr.id),
                led_rect,
                "toggle",
                true,
                "Mute: click. Solo: Ctrl+click or right-click",
            );
            let ctrl = paint.ui.input(|i| i.modifiers.ctrl);
            if led.clicked() && !ctrl {
                actions.push(PlaylistAction::MuteTrack {
                    id: tr.id,
                    muted: !tr.muted,
                });
            } else if led.secondary_clicked() || (led.clicked() && ctrl) {
                actions.push(PlaylistAction::SoloTrack(tr.id));
            }
        }
        if response.clicked()
            && let Some(tr) = track
        {
            state.selected_track = Some(tr.id);
            actions.push(PlaylistAction::SelectTrack(tr.id));
        }
        // Row separator [W y 193-195].
        for (dy, color) in [
            (45.0, t.row_line_edge),
            (46.0, t.row_line),
            (47.0, t.row_line_edge),
        ] {
            grid_painter.rect_filled(
                Rect::from_min_max(pos2(grid.left(), y + dy), pos2(grid.right(), y + dy + 1.0)),
                0.0,
                color,
            );
        }
        let row_rect = Rect::from_min_max(pos2(grid.left(), y), pos2(grid.right(), y + l.track_h));
        let row_response = paint.ui.interact(
            row_rect.intersect(grid),
            Id::new(("playlist.row", row)),
            Sense::click_and_drag(),
        );
        row_responses.push((row, track.map(|tr| tr.id), row_response.clone()));
        if row_response.hovered() {
            *paint.hint = match state.tool {
                PlTool::Draw => "Draw (P): click places the selected pattern; Ctrl+drag selects",
                PlTool::Paint => "Paint (B): click or drag to paint clips",
                PlTool::Delete => "Delete (D): click or drag across clips to delete them",
                PlTool::Mute => "Mute (T): click or drag across clips to mute them",
                PlTool::Slip => "Slip (S): drag a clip's content left or right",
                PlTool::Slice => "Slice (C): click a clip, or drag vertically, to slice",
                PlTool::Select => "Select (E): click or drag to select; Shift adds",
                PlTool::Play => "Play (Y): click a clip to play from there",
            }
            .to_owned();
        }
    }

    // Pattern clips. No clip is visible in W, so the clip look is derived from capture A.
    let tpb = view.ticks_per_bar.max(1);
    let beat = (tpb / i64::from(view.beats_per_bar.max(1))).max(1);
    let (alt, ctrl_down, shift_down) = paint
        .ui
        .input(|i| (i.modifiers.alt, i.modifiers.ctrl, i.modifiers.shift));
    let snap = if alt {
        1
    } else {
        view.global_snap.ticks(crate::snap::GridTicks {
            step: view.step_ticks.max(1),
            beat,
            bar: tpb,
            line: beat,
        })
    };
    let px_per_tick = bar_w / tpb as f32;
    for clip in &view.clips {
        let Some(row) = view.tracks.iter().position(|track| track.id == clip.track) else {
            continue;
        };
        if view.ticks_per_bar <= 0 {
            continue;
        }
        let (mut start, mut length, mut row_shown) = (clip.start, clip.length, row as i64);
        match state.drag {
            Some(ClipDrag::Move { dt, drow, .. }) if state.selection.contains(&clip.id) => {
                start = (start + dt).max(0);
                row_shown = (row_shown + drow).max(0);
            }
            Some(ClipDrag::Resize { id, dl, .. }) if id == clip.id => {
                length = (length + dl).max(snap);
            }
            _ => {}
        }
        let x0 = bar_x(start as f32 / view.ticks_per_bar as f32);
        let x1 = bar_x((start + length) as f32 / view.ticks_per_bar as f32);
        let y = grid.top() + (row_shown as f32 - state.scroll_tracks) * l.track_h;
        let clip_rect = Rect::from_min_max(pos2(x0 + 1.0, y + 1.0), pos2(x1, y + l.track_h - 3.0));
        if !clip_rect.intersects(grid) {
            continue;
        }
        let base = theme::model_color(clip.color, t.clip_default);
        // A muted clip is drawn faded.
        let color = if clip.muted {
            theme::shade(base, 0.55)
        } else {
            base
        };
        let selected = state.selection.contains(&clip.id);
        grid_painter.rect_filled(clip_rect, 1.0, theme::shade(color, 0.45));
        grid_painter.rect_filled(
            clip_rect.shrink(1.0),
            1.0,
            theme::shade(color, if selected { 0.95 } else { 0.78 }),
        );
        let label_band = Rect::from_min_max(
            clip_rect.min + vec2(1.0, 1.0),
            pos2(clip_rect.right() - 1.0, clip_rect.top() + 15.0),
        );
        grid_painter.rect_filled(label_band, 0.0, color);
        grid_painter
            .with_clip_rect(label_band.intersect(grid))
            .text(
                pos2(label_band.left() + 4.0, label_band.center().y),
                Align2::LEFT_CENTER,
                &clip.label,
                theme::medium(13.0),
                t.picker_text,
            );
        // Automation Clips are moved by their title bar because "the lower clip area is used to
        // edit Control Points" [Manual: playlist, "Resizing & Slip-editing Clips"].
        let slipped = match state.drag {
            Some(ClipDrag::Slip { id, d, .. }) if id == clip.id => d,
            _ => 0,
        };
        if let Some((columns, sample_ticks, offset)) = &clip.waveform {
            let offset = &(offset - slipped).max(0);
            let body =
                Rect::from_min_max(pos2(clip_rect.left(), label_band.bottom()), clip_rect.max)
                    .intersect(grid);
            if body.width() > 0.0 && *sample_ticks > 0.0 && !columns.is_empty() {
                let wave_painter = grid_painter.with_clip_rect(body);
                let cy = body.center().y;
                let half = body.height() * 0.45;
                let x_start = body.left().max(clip_rect.left());
                // "Show fade previews / gain previews - Shows the effect of the Fades and Gain
                // controls on the waveform" [Manual: playlist_audioclip].
                let props = clip.audio.unwrap_or_default();
                let ms_to_px =
                    |ms: u32| (f64::from(ms) / view.ms_per_tick.max(1e-9)) as f32 * px_per_tick;
                let (fade_in, fade_out) = (ms_to_px(props.fade_in_ms), ms_to_px(props.fade_out_ms));
                let preview = |x: f32| {
                    let mut k = props.gain() as f32;
                    let from_start = x - clip_rect.left();
                    let to_end = clip_rect.right() - x;
                    if fade_in > 0.0 && from_start < fade_in {
                        k *= (from_start / fade_in).max(0.0);
                    }
                    if fade_out > 0.0 && to_end < fade_out {
                        k *= (to_end / fade_out).max(0.0);
                    }
                    k
                };
                let mut x = x_start;
                while x < body.right() {
                    let tick = (x - (x0 + 1.0)) / px_per_tick + *offset as f32;
                    let index = (f64::from(tick) / sample_ticks * columns.len() as f64) as isize;
                    if let Some((lo, hi)) = usize::try_from(index).ok().and_then(|i| columns.get(i))
                    {
                        let k = preview(x);
                        wave_painter.line_segment(
                            [
                                pos2(x, cy - (hi * k).clamp(-1.0, 1.0) * half),
                                pos2(x, cy - (lo * k).clamp(-1.0, 1.0) * half),
                            ],
                            Stroke::new(1.0, t.picker_text),
                        );
                    }
                    x += 1.0;
                }
            }
        }
        if let Some(auto) = &clip.automation {
            let body =
                Rect::from_min_max(pos2(clip_rect.left(), label_band.bottom()), clip_rect.max);
            automation_body(
                paint,
                state,
                &grid_painter,
                auto,
                body.intersect(grid),
                x0 + 1.0,
                px_per_tick,
                snap,
                &mut actions,
            );
        }
        let handle = if clip.automation.is_some() {
            label_band
        } else {
            clip_rect
        };
        let response = paint.hit(
            &format!("playlist.clip.{}", clip.id),
            handle.intersect(grid),
            if clip.automation.is_some() {
                "automation-clip"
            } else {
                "pattern-clip"
            },
            true,
            &format!(
                "{}: {} - drag to move, drag the right edge to resize, right-click deletes, Ctrl+click mutes",
                if clip.automation.is_some() {
                    "Automation clip"
                } else {
                    "Pattern clip"
                },
                clip.label
            ),
        );
        // Gestures are measured from where the button went down, not from where egui's drag
        // threshold was crossed, so a drag of N bars moves the clip N bars.
        let pointer = paint
            .ui
            .input(|i| i.pointer.press_origin().or(i.pointer.interact_pos()));
        let on_edge = pointer.is_some_and(|p| p.x > clip_rect.right() - 6.0);
        let (shift, primary_down, hover) = paint.ui.input(|i| {
            (
                i.modifiers.shift,
                i.pointer.primary_down(),
                i.pointer.hover_pos(),
            )
        });
        let click_tick = |x: f32| {
            let raw = ((x - grid.left() - 1.0) / bar_w + state.scroll_bars) * tpb as f32;
            let tick = raw.max(0.0) as i64;
            (tick + snap / 2).div_euclid(snap) * snap
        };
        if response.clicked() {
            match state.tool {
                PlTool::Delete => actions.push(PlaylistAction::DeleteClip(clip.id)),
                PlTool::Mute => actions.push(PlaylistAction::MuteClip {
                    id: clip.id,
                    muted: !clip.muted,
                }),
                PlTool::Slice => {
                    if let Some(p) = response.interact_pointer_pos() {
                        actions.push(PlaylistAction::SliceClip {
                            id: clip.id,
                            at: click_tick(p.x),
                        });
                    }
                }
                // "Play selected (Y) - Click the clips you want to play. Click position will set
                // the start location."
                PlTool::Play => {
                    if let Some(p) = response.interact_pointer_pos() {
                        actions.push(PlaylistAction::PlayFrom(click_tick(p.x)));
                    }
                }
                PlTool::Draw | PlTool::Paint | PlTool::Select | PlTool::Slip => {
                    // Shift (Select tool) or Ctrl adds to / removes from the selection.
                    if shift || ctrl_down {
                        if !state.selection.remove(&clip.id) {
                            state.selection.insert(clip.id);
                        }
                    } else {
                        state.selection = [clip.id].into();
                    }
                }
            }
        }
        // Right-click deletes [Manual: playlist, "Draw mode ... Right-Click to delete Clips"].
        if response.secondary_clicked() {
            actions.push(PlaylistAction::DeleteClip(clip.id));
        }
        // "To Open - Double-click the Automation Clip in the Playlist" [Manual:
        // playlist_automationclip, "Automation Clip Settings"].
        if response.double_clicked()
            && let Some(auto) = &clip.automation
        {
            actions.push(PlaylistAction::OpenAutomation(auto.id));
        }
        // Delete and Mute tools: "Click-and-drag to delete Clips", "Left-click and drag to mute
        // multiple clips" - every clip the pointer crosses while the button is down.
        if response.drag_started_by(PointerButton::Primary)
            && matches!(state.tool, PlTool::Delete | PlTool::Mute)
        {
            let mute = (state.tool == PlTool::Mute).then_some(!clip.muted);
            state.sweep = Some((mute, std::collections::BTreeSet::new()));
        }
        if primary_down
            && hover.is_some_and(|p| clip_rect.contains(p))
            && let Some((mute, done)) = &mut state.sweep
            && done.insert(clip.id)
        {
            actions.push(match mute {
                Some(muted) => PlaylistAction::MuteClip {
                    id: clip.id,
                    muted: *muted,
                },
                None => PlaylistAction::DeleteClip(clip.id),
            });
        }
        if response.drag_started_by(PointerButton::Primary)
            && let Some(p) = pointer
            && !matches!(
                state.tool,
                PlTool::Delete | PlTool::Mute | PlTool::Slice | PlTool::Play
            )
        {
            if state.tool == PlTool::Slip {
                state.drag = Some(ClipDrag::Slip {
                    id: clip.id,
                    anchor: p,
                    d: 0,
                });
            } else if on_edge {
                state.selection = [clip.id].into();
                state.drag = Some(ClipDrag::Resize {
                    id: clip.id,
                    anchor: p,
                    dl: 0,
                });
            } else {
                // Dragging a selected clip moves the whole selection.
                if !state.selection.contains(&clip.id) {
                    state.selection = [clip.id].into();
                }
                state.drag = Some(ClipDrag::Move {
                    id: clip.id,
                    anchor: p,
                    dt: 0,
                    drow: 0,
                });
            }
        }
        if response.dragged()
            && let Some(p) = hover
        {
            match &mut state.drag {
                Some(ClipDrag::Move {
                    id,
                    anchor,
                    dt,
                    drow,
                }) if *id == clip.id => {
                    let raw = ((p.x - anchor.x) / px_per_tick).round() as i64;
                    *dt = (raw + snap / 2).div_euclid(snap) * snap;
                    *drow = ((p.y - anchor.y) / l.track_h).round() as i64;
                }
                Some(ClipDrag::Resize { id, anchor, dl }) if *id == clip.id => {
                    let raw = ((p.x - anchor.x) / px_per_tick).round() as i64;
                    *dl = (raw + snap / 2).div_euclid(snap) * snap;
                }
                Some(ClipDrag::Slip { id, anchor, d }) if *id == clip.id => {
                    let raw = ((p.x - anchor.x) / px_per_tick).round() as i64;
                    // The content cannot slide before its start (offset stays >= 0).
                    *d = ((raw + snap / 2).div_euclid(snap) * snap).min(clip.offset);
                }
                _ => {}
            }
        }
        if response.drag_stopped() {
            match state.drag.take() {
                Some(ClipDrag::Move { dt, drow, .. }) if dt != 0 || drow != 0 => {
                    let moves = view
                        .clips
                        .iter()
                        .filter(|c| state.selection.contains(&c.id))
                        .filter_map(|c| {
                            let row = view.tracks.iter().position(|tr| tr.id == c.track)?;
                            Some((
                                c.id,
                                (row as i64 + drow).max(0) as usize,
                                (c.start + dt).max(0),
                            ))
                        })
                        .collect();
                    actions.push(PlaylistAction::MoveClips(moves));
                }
                Some(ClipDrag::Resize { id, dl, .. }) if dl != 0 => {
                    actions.push(PlaylistAction::ResizeClip {
                        id,
                        length: (clip.length + dl).max(snap),
                    });
                }
                Some(ClipDrag::Slip { id, d, .. }) if d != 0 => {
                    actions.push(PlaylistAction::SlipClip {
                        id,
                        offset: (clip.offset - d).max(0),
                    });
                }
                _ => {}
            }
        }
        if let Some(audio) = clip.audio {
            audio_controls(
                paint,
                state,
                &grid_painter,
                clip,
                audio,
                clip_rect,
                label_band.bottom(),
                grid,
                px_per_tick,
                view.ms_per_tick,
                &mut actions,
            );
        }
    }

    // Gestures on the empty grid [Manual: playlist, "Tool Bar icons"].
    let tick_at_x = |x: f32| {
        (((x - grid.left() - 1.0) / bar_w + state.scroll_bars) * tpb as f32).max(0.0) as i64
    };
    let row_at_y = |y: f32| {
        ((y - grid.top()) / l.track_h + state.scroll_tracks)
            .floor()
            .max(0.0) as usize
    };
    let pattern_length = view
        .patterns
        .iter()
        .find(|p| p.id == view.pattern_id)
        .map_or(tpb, |p| p.length.max(1));
    let row_free = |row: usize, start: i64, length: i64| {
        view.tracks.get(row).is_none_or(|tr| {
            !view
                .clips
                .iter()
                .any(|c| c.track == tr.id && start < c.start + c.length && c.start < start + length)
        })
    };
    for (row, _track, response) in &row_responses {
        let selecting = state.tool == PlTool::Select || ctrl_down;
        if response.clicked()
            && let Some(p) = response.interact_pointer_pos()
        {
            let start = tick_at_x(p.x).div_euclid(snap) * snap;
            if selecting {
                state.selection.clear();
            } else if matches!(state.tool, PlTool::Draw | PlTool::Paint) && row_free(*row, start, 1)
            {
                // "Left-click to add the currently selected Clip", at the snap position.
                actions.push(PlaylistAction::AddClipOnRow {
                    row: *row,
                    pattern: view.pattern_id,
                    start,
                    length: pattern_length,
                });
            } else if state.tool == PlTool::Play {
                actions.push(PlaylistAction::PlayFrom(start));
            }
        }
        if response.drag_started_by(PointerButton::Primary)
            && let Some(origin) = paint.ui.input(|i| i.pointer.press_origin())
        {
            if selecting {
                state.grid_drag = Some(GridDrag::Rect {
                    from: origin,
                    to: origin,
                });
            } else if state.tool == PlTool::Slice {
                state.grid_drag = Some(GridDrag::Slice {
                    x: origin.x,
                    from_y: origin.y,
                    to_y: origin.y,
                });
            } else if state.tool == PlTool::Paint {
                state.painting = Some((*row, Vec::new()));
            } else if matches!(state.tool, PlTool::Delete | PlTool::Mute) {
                state.sweep = Some((
                    (state.tool == PlTool::Mute).then_some(true),
                    std::collections::BTreeSet::new(),
                ));
            }
        }
        if response.dragged()
            && let Some(p) = paint.ui.input(|i| i.pointer.hover_pos())
        {
            match &mut state.grid_drag {
                Some(GridDrag::Rect { to, .. }) => *to = p,
                Some(GridDrag::Slice { to_y, .. }) => *to_y = p.y,
                None => {}
            }
            // "Click-and-drag to paint multiple clips": one clip per free slot the pointer reaches.
            if let Some((paint_row, painted)) = &mut state.painting {
                let start = tick_at_x(p.x).div_euclid(pattern_length) * pattern_length;
                if !painted.contains(&start) && row_free(*paint_row, start, pattern_length) {
                    painted.push(start);
                    actions.push(PlaylistAction::AddClipOnRow {
                        row: *paint_row,
                        pattern: view.pattern_id,
                        start,
                        length: pattern_length,
                    });
                }
            }
        }
        if response.drag_stopped() {
            state.painting = None;
            match state.grid_drag.take() {
                Some(GridDrag::Rect { from, to }) => {
                    let area = Rect::from_two_pos(from, to);
                    let (t0, t1) = (tick_at_x(area.left()), tick_at_x(area.right()));
                    let (r0, r1) = (row_at_y(area.top()), row_at_y(area.bottom()));
                    if !(shift_down || ctrl_down) {
                        state.selection.clear();
                    }
                    for clip in &view.clips {
                        let Some(row) = view.tracks.iter().position(|tr| tr.id == clip.track)
                        else {
                            continue;
                        };
                        if (r0..=r1).contains(&row)
                            && clip.start < t1
                            && clip.start + clip.length > t0
                        {
                            state.selection.insert(clip.id);
                        }
                    }
                }
                // "Click and drag to make vertical slices through Clip".
                Some(GridDrag::Slice { x, from_y, to_y }) => {
                    let at = (tick_at_x(x) + snap / 2).div_euclid(snap) * snap;
                    let (r0, r1) = (row_at_y(from_y.min(to_y)), row_at_y(from_y.max(to_y)));
                    for clip in &view.clips {
                        let Some(row) = view.tracks.iter().position(|tr| tr.id == clip.track)
                        else {
                            continue;
                        };
                        if (r0..=r1).contains(&row)
                            && clip.start < at
                            && at < clip.start + clip.length
                        {
                            actions.push(PlaylistAction::SliceClip { id: clip.id, at });
                        }
                    }
                }
                None => {}
            }
        }
    }
    if !paint.ui.input(|i| i.pointer.primary_down()) {
        state.sweep = None;
    }
    match state.grid_drag {
        Some(GridDrag::Rect { from, to }) => {
            painter.with_clip_rect(grid).rect_stroke(
                Rect::from_two_pos(from, to),
                0.0,
                Stroke::new(1.0, t.select_on),
                egui::StrokeKind::Inside,
            );
        }
        Some(GridDrag::Slice { x, from_y, to_y }) => {
            painter.with_clip_rect(grid).line_segment(
                [pos2(x, from_y), pos2(x, to_y)],
                Stroke::new(1.5, t.playhead),
            );
        }
        None => {}
    }
    // Tool keys and Delete when the Playlist has the keyboard.
    if state.focused {
        let (pressed_tool, delete, plain) = paint.ui.input(|i| {
            (
                PlTool::KEYS
                    .iter()
                    .find(|(_, key)| i.key_pressed(*key))
                    .map(|(tool, _)| *tool),
                i.key_pressed(egui::Key::Delete),
                !i.modifiers.any(),
            )
        });
        if plain
            && !view.typing
            && let Some(tool) = pressed_tool
        {
            state.tool = tool;
        }
        if delete && !state.selection.is_empty() {
            actions.push(PlaylistAction::DeleteClips(
                std::mem::take(&mut state.selection).into_iter().collect(),
            ));
        }
    }

    // Clicking the bar ruler relocates the playback position [Manual: playlist,
    // "Relocate the playback position - Click on the upper timeline"]; Ctrl+drag makes a timeline
    // selection, Ctrl+click or double-click deletes it [Manual: playlist, "Timeline Selection"].
    let ruler_response =
        paint
            .ui
            .interact(ruler, Id::new("playlist.ruler"), Sense::click_and_drag());
    paint
        .probe
        .insert("playlist.ruler", ruler, "ruler", true, None);
    let tick_at = |x: f32| {
        (((x - grid.left() - 1.0) / bar_w + state.scroll_bars) * tpb as f32).max(0.0) as i64
    };
    let ctrl = paint.ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
    if ruler_response.double_clicked() || (ruler_response.clicked() && ctrl) {
        state.selecting = None;
        actions.push(PlaylistAction::ClearSelection);
    } else if ruler_response.drag_started() && ctrl {
        let origin = paint
            .ui
            .input(|i| i.pointer.press_origin())
            .map_or(0, |p| tick_at(p.x));
        state.selecting = Some((origin / beat * beat, origin / beat * beat));
    } else if let Some((from, _)) = state.selecting {
        if let Some(p) = paint.ui.input(|i| i.pointer.interact_pos()) {
            let to = (tick_at(p.x) + beat / 2) / beat * beat;
            state.selecting = Some((from, to));
        }
        if ruler_response.drag_stopped() {
            state.selecting = None;
            if let Some(p) = paint.ui.input(|i| i.pointer.interact_pos()) {
                let to = (tick_at(p.x) + beat / 2) / beat * beat;
                let (a, b) = (from.min(to), from.max(to));
                if b > a {
                    actions.push(PlaylistAction::SetSelection { start: a, end: b });
                }
            }
        }
    } else if (ruler_response.clicked() || ruler_response.dragged())
        && !ctrl
        && let Some(p) = paint.ui.input(|i| i.pointer.interact_pos())
    {
        actions.push(PlaylistAction::Seek((tick_at(p.x) / beat) * beat));
    }
    // The selection is shaded over the ruler and the grid.
    let shown_selection = state
        .selecting
        .map(|(a, b)| (a.min(b), a.max(b)))
        .or(view.selection);
    if let Some((a, b)) = shown_selection {
        let (x0, x1) = (
            bar_x(a as f32 / tpb as f32).max(grid.left()),
            bar_x(b as f32 / tpb as f32).min(grid.right()),
        );
        if x1 > x0 {
            let band = Rect::from_min_max(pos2(x0, ruler.top()), pos2(x1, grid.bottom()));
            painter.with_clip_rect(grid.union(ruler)).rect_filled(
                band,
                0.0,
                t.playhead.gamma_multiply(0.18),
            );
            painter.with_clip_rect(ruler).rect_filled(
                Rect::from_min_max(pos2(x0, ruler.top()), pos2(x1, ruler.top() + 4.0)),
                0.0,
                t.playhead,
            );
        }
    }

    // Time markers on the ruler: click seeks, right-click opens the marker menu; Alt+T adds one
    // at the playhead [Manual: playlist, "Time Markers"].
    for (index, (tick, name, action)) in view.markers.iter().enumerate() {
        let x = bar_x(*tick as f32 / tpb as f32);
        if x < ruler.left() || x > ruler.right() {
            continue;
        }
        let flag = Rect::from_min_size(pos2(x, ruler.top()), vec2(8.0, ruler.height()));
        let rp = painter.with_clip_rect(ruler);
        let points = match action {
            // "The 'Song loop' marker (note the down facing arrow)".
            jet_model::MarkerAction::Loop => vec![
                pos2(flag.left(), flag.top()),
                pos2(flag.right() + 2.0, flag.top()),
                pos2(flag.center().x + 1.0, flag.bottom()),
            ],
            // Ours: a right-pointing flag for the Start marker.
            jet_model::MarkerAction::Start => vec![
                pos2(flag.left(), flag.top()),
                pos2(flag.right() + 2.0, flag.center().y),
                pos2(flag.left(), flag.bottom()),
            ],
            // Ours: a left-pointing flag returns (marker loop), a narrow right wedge skips, a short
            // block pauses.
            jet_model::MarkerAction::MarkerLoop => vec![
                pos2(flag.right() + 2.0, flag.top()),
                pos2(flag.left(), flag.center().y),
                pos2(flag.right() + 2.0, flag.bottom()),
            ],
            jet_model::MarkerAction::Skip => vec![
                pos2(flag.left(), flag.top() + 3.0),
                pos2(flag.right() + 4.0, flag.center().y),
                pos2(flag.left(), flag.bottom() - 3.0),
            ],
            jet_model::MarkerAction::Pause => vec![
                pos2(flag.left(), flag.top() + 4.0),
                pos2(flag.right(), flag.top() + 4.0),
                pos2(flag.right(), flag.bottom() - 4.0),
                pos2(flag.left(), flag.bottom() - 4.0),
            ],
            jet_model::MarkerAction::None => vec![
                flag.left_top(),
                flag.right_top(),
                flag.right_bottom(),
                flag.left_bottom(),
            ],
        };
        rp.add(egui::Shape::convex_polygon(
            points,
            t.playhead,
            Stroke::NONE,
        ));
        painter.with_clip_rect(grid.union(ruler)).text(
            pos2(x + 10.0, ruler.center().y),
            Align2::LEFT_CENTER,
            name,
            theme::regular(12.0),
            t.ruler_text,
        );
        let response = paint.hit(
            &format!("playlist.marker.{index}"),
            flag.expand2(vec2(2.0, 0.0)),
            "marker",
            true,
            &format!("Marker {name} ({action:?}): click to jump, right-click for the marker menu"),
        );
        if response.clicked() {
            actions.push(PlaylistAction::Seek(*tick));
        }
        if response.secondary_clicked() {
            state.marker_menu = Some((index, pos2(flag.left(), ruler.bottom())));
        }
    }
    if paint
        .ui
        .input(|i| i.key_pressed(egui::Key::T) && i.modifiers.alt && !i.modifiers.ctrl)
        && paint.ui.rect_contains_pointer(rect)
    {
        actions.push(PlaylistAction::AddMarker);
    }

    // Audio files dropped on the grid become Audio Clips at the beat under the pointer.
    let (pointer, released) = paint
        .ui
        .input(|i| (i.pointer.hover_pos(), i.pointer.any_released()));
    if let Some(p) = pointer.filter(|p| grid.contains(*p)) {
        let mut dropped: Vec<std::path::PathBuf> = paint.ui.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .collect()
        });
        if released && let Some(path) = &view.dragging_file {
            dropped.push(path.clone());
        }
        let row = ((p.y - grid.top()) / l.track_h + state.scroll_tracks)
            .floor()
            .max(0.0) as usize;
        let tick =
            (((p.x - grid.left() - 1.0) / bar_w + state.scroll_bars) * tpb as f32).max(0.0) as i64;
        for path in dropped {
            actions.push(PlaylistAction::DropAudio {
                row,
                start: tick / beat * beat,
                path,
            });
        }
        if view.dragging_file.is_some() {
            *paint.hint = "Drop to create an Audio Clip".to_owned();
        }
    }

    // Song-position line while playing in SONG mode.
    if let Some(position) = view.position
        && view.ticks_per_bar > 0
    {
        let x = bar_x(position as f32 / view.ticks_per_bar as f32);
        if x >= grid.left() && x <= grid.right() {
            painter.line_segment(
                [pos2(x, ruler.top()), pos2(x, grid.bottom())],
                Stroke::new(1.5, t.playhead),
            );
        }
    }

    // Vertical scrollbar [W 1547-1566] with its top button [W 128-147].
    let vscroll = at(grid_r, l.ruler_top, w, h);
    hatch(&painter, vscroll, t.hatch_a, t.hatch_b);
    let up = at(grid_r, l.ruler_top, w, l.tracks_top);
    painter.rect_filled(up, 0.0, t.pl_strip_shadow);
    icons::chevron_up_line(&painter, up, t.title_icon);
    paint
        .probe
        .insert("playlist.grid", grid, "timeline", true, None);
    // Last, so the popups' items sit above every other control of the window.
    paint_point_menu(paint, state, &mut actions);
    paint_audio_menu(paint, state, view, &mut actions);
    paint_marker_menu(paint, state, view, &mut actions);
    paint_arrangement_menu(paint, state, view, &mut actions);
    paint_merge_menu(paint, state, &mut actions);
    actions
}

/// Envelope of an Automation Clip and its control-point gestures [Manual:
/// playlist_automationclip]: right-click adds a point, left-drag moves one, Alt+click deletes
/// one, right-click on a point opens Delete and the Curve Type Menu.
#[allow(clippy::too_many_arguments)]
fn automation_body(
    paint: &mut PaintCtx<'_>,
    state: &mut PlaylistState,
    painter: &egui::Painter,
    auto: &AutomationView,
    body: Rect,
    clip_left: f32,
    px_per_tick: f32,
    snap: i64,
    actions: &mut Vec<PlaylistAction>,
) {
    if body.width() <= 0.0 || body.height() <= 4.0 {
        return;
    }
    let t = *paint.tokens;
    let inner = body.shrink2(vec2(0.0, 3.0));
    let x_of = |at: i64| clip_left + (at - auto.offset) as f32 * px_per_tick;
    let y_of = |value: u16| inner.bottom() - f32::from(value) / 1000.0 * inner.height();
    let tick_at = |x: f32| ((x - clip_left) / px_per_tick).round() as i64 + auto.offset;
    let value_at =
        |y: f32| (((inner.bottom() - y) / inner.height()).clamp(0.0, 1.0) * 1000.0).round() as u16;
    // Points as shown (a dragged point follows the mouse).
    let mut points = auto.points.clone();
    if let Some((id, index, at, value)) = state.point_drag
        && id == auto.id
        && let Some(point) = points.get_mut(index)
    {
        point.at = jet_time::Tick(if index == 0 { 0 } else { at.max(0) });
        point.value = value;
        points.sort_by_key(|p| p.at);
    }
    // Tension drag preview.
    if let Some((id, index, _, tension)) = state.tension_drag
        && id == auto.id
        && let Some(point) = points.get_mut(index)
    {
        point.tension = tension;
    }
    // Envelope polyline, sampled every 2 px; with LFO mode or Min/Max/Time the clip previews the
    // output ("LFO Switch - Enables and shows a preview of the LFO ... inside clips").
    let clip_painter = painter.with_clip_rect(body);
    let output = |tick: i64| {
        let lane = jet_seq::automation::AutoLane {
            target: jet_model::AutoTarget::MixerVolume(0),
            segments: vec![jet_seq::automation::AutoSegment {
                start: 0,
                end: i64::MAX,
                offset: 0,
                automation: jet_model::AutomationId(auto.id),
            }],
            points: vec![(
                jet_model::AutomationId(auto.id),
                points.clone(),
                auto.settings,
            )],
            beat_ticks: auto.beat_ticks,
        };
        lane.value_at(tick)
    };
    let shaped = auto.settings != jet_model::AutoSettings::default();
    let mut line = Vec::new();
    let mut preview = Vec::new();
    let mut x = body.left();
    while x <= body.right() {
        let tick = tick_at(x);
        if let Some(v) = jet_seq::automation::value_at(&points, tick) {
            line.push(pos2(x, inner.bottom() - v as f32 * inner.height()));
        }
        if shaped && let Some(v) = output(tick.max(0)) {
            preview.push(pos2(x, inner.bottom() - v as f32 * inner.height()));
        }
        x += 2.0;
    }
    if shaped {
        clip_painter.add(egui::Shape::line(
            preview,
            Stroke::new(1.0, theme::shade(t.picker_text, 0.75)),
        ));
    }
    clip_painter.add(egui::Shape::line(line, Stroke::new(1.5, t.picker_text)));
    // Tension handles: a small circle in the middle of each segment [Manual:
    // playlist_automationclip, "Change envelope curve - Left-click a tension handle and move
    // up/down ... Right-Click the tension handle to reset curve tension"].
    let handles: Vec<(usize, egui::Pos2)> = (1..points.len())
        .map(|i| {
            let mid = (points[i - 1].at.0 + points[i].at.0) / 2;
            let v = jet_seq::automation::value_at(&points, mid).unwrap_or(0.0);
            (
                i,
                pos2(x_of(mid), inner.bottom() - v as f32 * inner.height()),
            )
        })
        .collect();
    for (_, at) in &handles {
        clip_painter.circle_filled(*at, 2.5, t.picker_text);
    }
    for point in &points {
        clip_painter.circle_stroke(
            pos2(x_of(point.at.0), y_of(point.value)),
            3.5,
            Stroke::new(1.5, t.picker_text),
        );
    }
    let response = paint.ui.interact(
        body,
        Id::new(("playlist.automation", auto.id, body.left() as i32)),
        Sense::click_and_drag(),
    );
    paint.probe.insert(
        format!("playlist.automation.{}", auto.id),
        body,
        "envelope",
        true,
        None,
    );
    let (pointer, origin, alt) = paint.ui.input(|i| {
        (
            i.pointer.hover_pos(),
            i.pointer.press_origin(),
            i.modifiers.alt,
        )
    });
    let near = |p: egui::Pos2| {
        auto.points
            .iter()
            .enumerate()
            .find(|(_, pt)| pos2(x_of(pt.at.0), y_of(pt.value)).distance(p) <= 6.0)
            .map(|(i, _)| i)
    };
    let near_handle = |p: egui::Pos2| {
        handles
            .iter()
            .find(|(_, at)| at.distance(p) <= 6.0)
            .map(|(i, _)| *i)
    };
    if response.hovered() {
        *paint.hint = "Right-click adds a point; drag a point to move it; Alt+click deletes; right-click a point for curve types".to_owned();
    }
    if response.secondary_clicked()
        && let Some(p) = pointer
    {
        match near(p) {
            Some(index) => state.point_menu = Some((auto.id, index, p)),
            None if near_handle(p).is_some() => {
                if let Some(index) = near_handle(p) {
                    actions.push(PlaylistAction::AutoTension {
                        id: auto.id,
                        index,
                        tension: 0,
                    });
                }
            }
            None => {
                let at = (tick_at(p.x) + snap / 2).div_euclid(snap.max(1)) * snap.max(1);
                actions.push(PlaylistAction::AutoAddPoint {
                    id: auto.id,
                    at: at.max(0),
                    value: value_at(p.y),
                });
            }
        }
    }
    if response.clicked()
        && alt
        && let Some(index) = pointer.and_then(near)
    {
        actions.push(PlaylistAction::AutoDeletePoint { id: auto.id, index });
    }
    if response.drag_started_by(PointerButton::Primary)
        && let Some(index) = origin.and_then(near)
    {
        let point = &auto.points[index];
        state.point_drag = Some((auto.id, index, point.at.0, point.value));
    } else if response.drag_started_by(PointerButton::Primary)
        && let Some(index) = origin.and_then(near_handle)
    {
        let tension = auto.points[index].tension;
        state.tension_drag = Some((auto.id, index, tension, tension));
    }
    // Up raises the curve's middle: positive tension for a rising segment, negative for a
    // falling one (8 units per pixel, ours).
    if response.dragged()
        && let (Some(p), Some(o), Some(drag)) = (pointer, origin, state.tension_drag.as_mut())
        && drag.0 == auto.id
    {
        let rising = auto.points[drag.1].value >= auto.points[drag.1 - 1].value;
        let delta = ((o.y - p.y) * 8.0) as i32 * if rising { -1 } else { 1 };
        drag.3 = (i32::from(drag.2) + delta).clamp(-1000, 1000) as i16;
        *paint.hint = format!("Tension: {:+.0}%", f32::from(drag.3) / 10.0);
    }
    if response.drag_stopped()
        && let Some((id, index, _, tension)) = state.tension_drag.take()
        && id == auto.id
    {
        actions.push(PlaylistAction::AutoTension { id, index, tension });
    }
    if response.dragged()
        && let (Some(p), Some(drag)) = (pointer, state.point_drag.as_mut())
        && drag.0 == auto.id
    {
        let at = tick_at(p.x);
        drag.2 = if alt {
            at
        } else {
            (at + snap / 2).div_euclid(snap.max(1)) * snap.max(1)
        };
        drag.3 = value_at(p.y);
        *paint.hint = format!("Value: {:.3}", f32::from(drag.3) / 1000.0);
    }
    if response.drag_stopped()
        && let Some((id, index, at, value)) = state.point_drag.take()
        && id == auto.id
    {
        actions.push(PlaylistAction::AutoMovePoint {
            id,
            index,
            at: at.max(0),
            value,
        });
    }
}

fn paint_point_menu(
    paint: &mut PaintCtx<'_>,
    state: &mut PlaylistState,
    actions: &mut Vec<PlaylistAction>,
) {
    let Some((id, index, origin)) = state.point_menu else {
        return;
    };
    let mut items = vec![crate::widgets::MenuItem {
        label: "Delete",
        shortcut: "",
        enabled: true,
    }];
    items.extend(CURVES.iter().map(|(_, label)| crate::widgets::MenuItem {
        label,
        shortcut: "",
        enabled: index > 0,
    }));
    match paint.popup_menu("playlist.point-menu", origin, &items) {
        crate::widgets::MenuResult::Open => {}
        crate::widgets::MenuResult::Dismissed => state.point_menu = None,
        crate::widgets::MenuResult::Chosen(choice) => {
            state.point_menu = None;
            if choice == 0 {
                actions.push(PlaylistAction::AutoDeletePoint { id, index });
            } else if let Some((shape, _)) = CURVES.get(choice - 1) {
                actions.push(PlaylistAction::AutoShape {
                    id,
                    index,
                    shape: *shape,
                });
            }
        }
    }
}

fn paint_arrangement_menu(
    paint: &mut PaintCtx<'_>,
    state: &mut PlaylistState,
    view: &PlaylistView,
    actions: &mut Vec<PlaylistAction>,
) {
    let Some(origin) = state.arrangement_menu else {
        return;
    };
    let labels: Vec<String> = view
        .arrangements
        .iter()
        .map(|(id, name)| {
            if *id == view.arrangement_id {
                format!("> {name}")
            } else {
                name.clone()
            }
        })
        .chain([
            "Add one".to_owned(),
            "Clone".to_owned(),
            "Delete".to_owned(),
        ])
        .chain(
            view.arrangements
                .iter()
                .filter(|(id, _)| *id != view.arrangement_id)
                .map(|(_, name)| format!("Merge with {name}...")),
        )
        .collect();
    let count = view.arrangements.len();
    let items: Vec<crate::widgets::MenuItem<'_>> = labels
        .iter()
        .enumerate()
        .map(|(i, label)| crate::widgets::MenuItem {
            label,
            shortcut: "",
            enabled: !(i == count + 2 && count <= 1),
        })
        .collect();
    match paint.popup_menu("playlist.arrangement-menu", origin, &items) {
        crate::widgets::MenuResult::Open => {}
        crate::widgets::MenuResult::Dismissed => state.arrangement_menu = None,
        crate::widgets::MenuResult::Chosen(i) => {
            state.arrangement_menu = None;
            if let Some((id, _)) = view.arrangements.get(i) {
                actions.push(PlaylistAction::SelectArrangement(*id));
            } else if i == count {
                actions.push(PlaylistAction::AddArrangement);
            } else if i == count + 1 {
                actions.push(PlaylistAction::CloneArrangement);
            } else if i == count + 2 {
                actions.push(PlaylistAction::DeleteArrangement(view.arrangement_id));
            } else if let Some((source, _)) = view
                .arrangements
                .iter()
                .filter(|(id, _)| *id != view.arrangement_id)
                .nth(i - count - 3)
            {
                state.merge_menu = Some((*source, origin));
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AudioHandle {
    FadeIn,
    FadeOut,
    Gain,
}

/// Audio Clip volume envelope controls [Manual: playlist_audioclip, "Volume Envelopes &
/// Controls"]: "Fade handles are the triangles on the upper edge of the Clip. Drag horizontally";
/// "Gain controls are the semi-circles on the lower edge of Clips ... Click and drag vertically
/// for +36 dB to -inf"; "Reset - (Alt/Opt+Click)". Right-click on a handle also resets it (the
/// manual's right-click menus offer "Reset"). Sizes and the 0.25 dB per pixel drag rate are ours
/// (UNMEASURED).
#[allow(clippy::too_many_arguments)]
fn audio_controls(
    paint: &mut PaintCtx<'_>,
    state: &mut PlaylistState,
    painter: &egui::Painter,
    clip: &ClipView,
    audio: jet_model::AudioClipProps,
    clip_rect: Rect,
    body_top: f32,
    grid: Rect,
    px_per_tick: f32,
    ms_per_tick: f64,
    actions: &mut Vec<PlaylistAction>,
) {
    let t = *paint.tokens;
    let ms_to_px = |ms: u32| (f64::from(ms) / ms_per_tick.max(1e-9)) as f32 * px_per_tick;
    let px_to_ms = |px: f32| (f64::from(px.max(0.0) / px_per_tick) * ms_per_tick).round() as i32;
    let live = |handle: AudioHandle, value: i32| match state.audio_drag {
        Some((id, h, v)) if id == clip.id && h == handle => v,
        _ => value,
    };
    let fade_in = live(AudioHandle::FadeIn, audio.fade_in_ms as i32).max(0) as u32;
    let fade_out = live(AudioHandle::FadeOut, audio.fade_out_ms as i32).max(0) as u32;
    let gain = live(AudioHandle::Gain, i32::from(audio.gain_cdb));
    let width = clip_rect.width();
    let x_in = clip_rect.left() + ms_to_px(fade_in).min(width);
    let x_out = clip_rect.right() - ms_to_px(fade_out).min(width);

    // Envelope line over the waveform.
    let gp = painter.with_clip_rect(clip_rect.intersect(grid));
    let (top, bottom) = (body_top + 2.0, clip_rect.bottom() - 2.0);
    let line = Stroke::new(1.0, theme::shade(t.picker_text, 1.2));
    if fade_in > 0 {
        gp.line_segment([pos2(clip_rect.left(), bottom), pos2(x_in, top)], line);
    }
    if fade_out > 0 {
        gp.line_segment([pos2(x_out, top), pos2(clip_rect.right(), bottom)], line);
    }

    let mut handle = |paint: &mut PaintCtx<'_>, which: AudioHandle, rect: Rect, hint: &str| {
        let id = match which {
            AudioHandle::FadeIn => format!("playlist.clip.{}.fade-in", clip.id),
            AudioHandle::FadeOut => format!("playlist.clip.{}.fade-out", clip.id),
            AudioHandle::Gain => format!("playlist.clip.{}.gain", clip.id),
        };
        let response = paint.hit(&id, rect.intersect(grid), "handle", true, hint);
        let alt = paint.ui.input(|i| i.modifiers.alt);
        let reset = |actions: &mut Vec<PlaylistAction>| {
            actions.push(PlaylistAction::ClipAudio {
                id: clip.id,
                gain_cdb: (which == AudioHandle::Gain).then_some(0),
                fade_in_ms: (which == AudioHandle::FadeIn).then_some(0),
                fade_out_ms: (which == AudioHandle::FadeOut).then_some(0),
                reverse: None,
            });
        };
        if (response.clicked() && alt) || response.secondary_clicked() {
            reset(actions);
            return;
        }
        if response.dragged()
            && let Some(p) = paint.ui.input(|i| i.pointer.interact_pos())
        {
            let value = match which {
                AudioHandle::FadeIn => px_to_ms(p.x - clip_rect.left()),
                AudioHandle::FadeOut => px_to_ms(clip_rect.right() - p.x),
                AudioHandle::Gain => {
                    let origin = paint.ui.input(|i| i.pointer.press_origin()).unwrap_or(p);
                    let base = if audio.gain_cdb == jet_model::AudioClipProps::GAIN_SILENT {
                        i32::from(jet_model::AudioClipProps::GAIN_MIN_CDB)
                    } else {
                        i32::from(audio.gain_cdb)
                    };
                    // 0.25 dB per pixel, up is louder; below -96 dB snaps to -inf.
                    (base + ((origin.y - p.y) * 25.0) as i32).min(3600)
                }
            };
            state.audio_drag = Some((clip.id, which, value));
        }
        if response.drag_stopped()
            && let Some((id, h, value)) = state.audio_drag.take()
            && id == clip.id
            && h == which
        {
            let ms = value.clamp(0, 600_000) as u32;
            actions.push(PlaylistAction::ClipAudio {
                id,
                gain_cdb: (which == AudioHandle::Gain)
                    .then(|| value.clamp(i32::from(i16::MIN), 3600) as i16),
                fade_in_ms: (which == AudioHandle::FadeIn).then_some(ms),
                fade_out_ms: (which == AudioHandle::FadeOut).then_some(ms),
                reverse: None,
            });
        }
    };

    // Fade handles: triangles on the upper edge.
    let tri = |x: f32, left: bool| {
        let (a, b) = if left { (x, x + 8.0) } else { (x - 8.0, x) };
        vec![
            pos2(a, body_top),
            pos2(b, body_top),
            pos2(if left { a } else { b }, body_top + 7.0),
        ]
    };
    let mark = t.picker_text;
    gp.add(egui::Shape::convex_polygon(
        tri(x_in, true),
        mark,
        Stroke::NONE,
    ));
    gp.add(egui::Shape::convex_polygon(
        tri(x_out, false),
        mark,
        Stroke::NONE,
    ));
    let fmt_ms = |ms: u32| format!("{ms} ms");
    handle(
        paint,
        AudioHandle::FadeIn,
        Rect::from_min_size(pos2(x_in - 2.0, body_top), vec2(11.0, 9.0)),
        &format!(
            "Fade in: {} - drag, Alt+click or right-click resets",
            fmt_ms(fade_in)
        ),
    );
    handle(
        paint,
        AudioHandle::FadeOut,
        Rect::from_min_size(pos2(x_out - 9.0, body_top), vec2(11.0, 9.0)),
        &format!(
            "Fade out: {} - drag, Alt+click or right-click resets",
            fmt_ms(fade_out)
        ),
    );
    // Gain: a semi-circle on the lower edge.
    let center = pos2(clip_rect.center().x, clip_rect.bottom());
    gp.circle_filled(center, 5.0, mark);
    let db = if gain <= i32::from(jet_model::AudioClipProps::GAIN_MIN_CDB) {
        "-inf dB".to_owned()
    } else {
        format!("{:+.1} dB", f64::from(gain) / 100.0)
    };
    handle(
        paint,
        AudioHandle::Gain,
        Rect::from_center_size(center - vec2(0.0, 3.0), vec2(12.0, 8.0)),
        &format!("Clip gain: {db} - drag up/down, Alt+click or right-click resets"),
    );

    // Clip menu: the small triangle at the right of the title.
    let menu = Rect::from_min_size(
        pos2(clip_rect.right() - 13.0, clip_rect.top() + 2.0),
        vec2(11.0, 12.0),
    );
    gp.add(egui::Shape::convex_polygon(
        vec![
            pos2(menu.left() + 1.0, menu.top() + 4.0),
            pos2(menu.right() - 1.0, menu.top() + 4.0),
            pos2(menu.center().x, menu.bottom() - 3.0),
        ],
        t.picker_text,
        Stroke::NONE,
    ));
    if paint
        .hit(
            &format!("playlist.clip.{}.menu", clip.id),
            menu.intersect(grid),
            "menu",
            true,
            "Audio Clip menu: reverse, reset fades, reset gain",
        )
        .clicked()
    {
        state.audio_menu = Some((clip.id, menu.left_bottom()));
    }
}

fn paint_audio_menu(
    paint: &mut PaintCtx<'_>,
    state: &mut PlaylistState,
    view: &PlaylistView,
    actions: &mut Vec<PlaylistAction>,
) {
    let Some((id, origin)) = state.audio_menu else {
        return;
    };
    let Some(audio) = view.clips.iter().find(|c| c.id == id).and_then(|c| c.audio) else {
        state.audio_menu = None;
        return;
    };
    let items = [
        crate::widgets::MenuItem {
            label: if audio.reverse {
                "Reverse: on"
            } else {
                "Reverse: off"
            },
            shortcut: "",
            enabled: true,
        },
        crate::widgets::MenuItem {
            label: "Reset fades",
            shortcut: "",
            enabled: audio.fade_in_ms > 0 || audio.fade_out_ms > 0,
        },
        crate::widgets::MenuItem {
            label: "Reset gain",
            shortcut: "",
            enabled: audio.gain_cdb != 0,
        },
    ];
    match paint.popup_menu("playlist.audio-menu", origin, &items) {
        crate::widgets::MenuResult::Open => {}
        crate::widgets::MenuResult::Dismissed => state.audio_menu = None,
        crate::widgets::MenuResult::Chosen(choice) => {
            state.audio_menu = None;
            actions.push(PlaylistAction::ClipAudio {
                id,
                gain_cdb: (choice == 2).then_some(0),
                fade_in_ms: (choice == 1).then_some(0),
                fade_out_ms: (choice == 1).then_some(0),
                reverse: (choice == 0).then_some(!audio.reverse),
            });
        }
    }
}

/// Marker right-click menu [Manual: playlist, "Marker/lane right-click menu": "Delete", "Action -
/// Set the Markers purpose"].
fn paint_marker_menu(
    paint: &mut PaintCtx<'_>,
    state: &mut PlaylistState,
    view: &PlaylistView,
    actions: &mut Vec<PlaylistAction>,
) {
    let Some((index, origin)) = state.marker_menu else {
        return;
    };
    let Some((_, _, current)) = view.markers.get(index) else {
        state.marker_menu = None;
        return;
    };
    const ACTIONS: [(jet_model::MarkerAction, &str); 6] = [
        (jet_model::MarkerAction::None, "Action: None"),
        (jet_model::MarkerAction::Start, "Action: Start"),
        (jet_model::MarkerAction::Loop, "Action: Song loop"),
        (jet_model::MarkerAction::MarkerLoop, "Action: Marker loop"),
        (jet_model::MarkerAction::Skip, "Action: Marker skip"),
        (jet_model::MarkerAction::Pause, "Action: Marker pause"),
    ];
    let mut items = vec![crate::widgets::MenuItem {
        label: "Delete",
        shortcut: "",
        enabled: true,
    }];
    items.extend(
        ACTIONS
            .iter()
            .map(|(action, label)| crate::widgets::MenuItem {
                label,
                shortcut: "",
                enabled: action != current,
            }),
    );
    match paint.popup_menu("playlist.marker-menu", origin, &items) {
        crate::widgets::MenuResult::Open => {}
        crate::widgets::MenuResult::Dismissed => state.marker_menu = None,
        crate::widgets::MenuResult::Chosen(choice) => {
            state.marker_menu = None;
            if choice == 0 {
                actions.push(PlaylistAction::DeleteMarker(index));
            } else if let Some((action, _)) = ACTIONS.get(choice - 1) {
                actions.push(PlaylistAction::MarkerAction {
                    index,
                    action: *action,
                });
            }
        }
    }
}

/// Zoom by `factor` keeping the time under `anchor_x` in place.
fn zoom_at(state: &mut PlaylistState, base_bar_w: f32, grid_left: f32, anchor_x: f32, factor: f32) {
    let old_w = base_bar_w * state.zoom;
    let bars_at_anchor = (anchor_x - grid_left) / old_w + state.scroll_bars;
    state.zoom = (state.zoom * factor).clamp(0.25, 6.0);
    let new_w = base_bar_w * state.zoom;
    state.scroll_bars = (bars_at_anchor - (anchor_x - grid_left) / new_w).max(0.0);
}

/// "Merge with" options: Position (Start, Current selection, End) x Mode (Insert, Merge, Replace)
/// [Manual: playlist, "Arrangements"].
fn paint_merge_menu(
    paint: &mut PaintCtx<'_>,
    state: &mut PlaylistState,
    actions: &mut Vec<PlaylistAction>,
) {
    let Some((source, origin)) = state.merge_menu else {
        return;
    };
    const OPTIONS: [(&str, &str, &str); 9] = [
        ("Insert at start", "start", "insert"),
        ("Insert at selection", "current", "insert"),
        ("Append at end", "end", "insert"),
        ("Merge at start", "start", "merge"),
        ("Merge at selection", "current", "merge"),
        ("Merge at end", "end", "merge"),
        ("Replace from start", "start", "replace"),
        ("Replace from selection", "current", "replace"),
        ("Replace at end", "end", "replace"),
    ];
    let items: Vec<crate::widgets::MenuItem<'_>> = OPTIONS
        .iter()
        .map(|(label, ..)| crate::widgets::MenuItem {
            label,
            shortcut: "",
            enabled: true,
        })
        .collect();
    match paint.popup_menu("playlist.merge-menu", origin, &items) {
        crate::widgets::MenuResult::Open => {}
        crate::widgets::MenuResult::Dismissed => state.merge_menu = None,
        crate::widgets::MenuResult::Chosen(i) => {
            state.merge_menu = None;
            if let Some((_, position, mode)) = OPTIONS.get(i) {
                actions.push(PlaylistAction::MergeArrangement {
                    source,
                    position,
                    mode,
                });
            }
        }
    }
}
