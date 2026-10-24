//! Playlist window body. Offsets are W pixels relative to the Playlist window's top-left corner
//! (W 500,84), measured in docs/UI_VISUAL_AUDIT.md §8.

use egui::{Align2, Id, PointerButton, Rect, Sense, Stroke, pos2, vec2};

use crate::icons;
use crate::theme;
use crate::widgets::{PaintCtx, hatch, vgradient};

pub struct PlaylistView {
    pub pattern_id: u64,
    pub arrangement_name: String,
    pub ticks_per_bar: i64,
    pub beats_per_bar: u32,
    pub position: Option<i64>,
    pub tracks: Vec<TrackView>,
    pub clips: Vec<ClipView>,
    pub patterns: Vec<PatternEntry>,
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
    pub label: String,
    pub color: Option<u32>,
}

pub struct PatternEntry {
    pub id: u64,
    pub name: String,
}

#[derive(Clone)]
pub struct PlaylistState {
    pub selected_track: Option<u64>,
    pub selected_clip: Option<u64>,
    /// Horizontal zoom as a multiple of the measured default bar width.
    pub zoom: f32,
    pub scroll_bars: f32,
    pub scroll_tracks: f32,
}

impl Default for PlaylistState {
    fn default() -> Self {
        Self {
            selected_track: None,
            selected_clip: None,
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
}

/// Title-bar tool icons [W x 510-740 relative to 500].
pub fn paint_title_tools(paint: &mut PaintCtx<'_>, title: Rect, view: &PlaylistView) {
    let t = *paint.tokens;
    let painter = paint.painter();
    type IconFn = fn(&egui::Painter, Rect, egui::Color32);
    let tools: [(f32, IconFn, &str, bool); 11] = [
        (13.0, icons::chevron_right, "Playlist options", false),
        (34.0, icons::magnet, "Snap - Main", false),
        (66.0, icons::pencil, "Draw tool", true),
        (
            88.0,
            icons::brush,
            "Paint tool - not implemented yet",
            false,
        ),
        (
            108.0,
            icons::delete,
            "Delete tool - not implemented yet",
            false,
        ),
        (128.0, icons::mute, "Mute tool - not implemented yet", false),
        (150.0, icons::slip, "Slip tool - not implemented yet", false),
        (
            171.0,
            icons::slice,
            "Slice tool - not implemented yet",
            false,
        ),
        (
            192.0,
            icons::select,
            "Select tool - not implemented yet",
            false,
        ),
        (213.0, icons::zoom, "Zoom tool - not implemented yet", false),
        (
            235.0,
            icons::playback,
            "Playback tool - not implemented yet",
            false,
        ),
    ];
    for (x, icon, hint, active) in tools {
        let cell =
            Rect::from_center_size(pos2(title.left() + x, title.center().y), vec2(19.0, 20.0));
        let _ = paint.hit(&format!("playlist.tool.{x}"), cell, "tool", false, hint);
        let color = if active {
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
    for part in [
        format!("Playlist - {}", view.arrangement_name),
        "(none)".to_owned(),
    ] {
        let end = painter.text(
            pos2(x, title.center().y),
            Align2::LEFT_CENTER,
            part,
            font.clone(),
            t.title_text,
        );
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
            state.zoom = (state.zoom * if delta > 0.0 { 1.1 } else { 0.9 }).clamp(0.25, 6.0);
        } else if shift {
            state.scroll_bars = (state.scroll_bars - delta / bar_w).max(0.0);
        } else {
            state.scroll_tracks = (state.scroll_tracks - delta / l.track_h).max(0.0);
        }
    }
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
    let bar_x = |bar: f32| grid.left() + 1.0 + (bar - state.scroll_bars) * bar_w;
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
        paint.led(pos2(o.x + header_l + 90.0, y + 37.0), 5.5, !muted);
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
            Sense::click(),
        );
        if row_response.clicked()
            && let Some(pointer) = row_response.interact_pointer_pos()
            && view.ticks_per_bar > 0
        {
            let bar = ((pointer.x - grid.left() - 1.0) / bar_w + state.scroll_bars)
                .floor()
                .max(0.0);
            let start = bar as i64 * view.ticks_per_bar;
            let occupied = track.is_some_and(|tr| {
                view.clips
                    .iter()
                    .any(|c| c.track == tr.id && start >= c.start && start < c.start + c.length)
            });
            if !occupied {
                actions.push(PlaylistAction::AddClipOnRow {
                    row,
                    pattern: view.pattern_id,
                    start,
                    length: view.ticks_per_bar,
                });
            }
        }
        if row_response.hovered() {
            *paint.hint = "Click to place the selected pattern; Ctrl+wheel zooms".to_owned();
        }
    }

    // Pattern clips. No clip is visible in W, so the clip look is derived from capture A.
    for clip in &view.clips {
        let Some(row) = view.tracks.iter().position(|track| track.id == clip.track) else {
            continue;
        };
        if view.ticks_per_bar <= 0 {
            continue;
        }
        let x0 = bar_x(clip.start as f32 / view.ticks_per_bar as f32);
        let x1 = bar_x((clip.start + clip.length) as f32 / view.ticks_per_bar as f32);
        let y = grid.top() + (row as f32 - state.scroll_tracks) * l.track_h;
        let clip_rect = Rect::from_min_max(pos2(x0 + 1.0, y + 1.0), pos2(x1, y + l.track_h - 3.0));
        if !clip_rect.intersects(grid) {
            continue;
        }
        let color = theme::model_color(clip.color, t.clip_default);
        let selected = state.selected_clip == Some(clip.id);
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
        let response = paint.hit(
            &format!("playlist.clip.{}", clip.id),
            clip_rect.intersect(grid),
            "pattern-clip",
            true,
            &format!("Pattern clip: {}", clip.label),
        );
        if response.clicked() {
            state.selected_clip = Some(clip.id);
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
    actions
}
