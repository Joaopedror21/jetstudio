use egui::{Align2, FontId, Id, PointerButton, Rect, Sense, Stroke, StrokeKind, pos2, vec2};

use crate::widgets::PaintCtx;

pub struct PlaylistView {
    pub pattern_id: u64,
    pub pattern_name: String,
    pub ticks_per_bar: i64,
    pub tracks: Vec<TrackView>,
    pub clips: Vec<ClipView>,
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
}

#[derive(Clone)]
pub struct PlaylistState {
    pub selected_track: Option<u64>,
    pub selected_clip: Option<u64>,
    pub pixels_per_bar: f32,
    pub scroll_bars: f32,
    pub scroll_tracks: f32,
}

impl Default for PlaylistState {
    fn default() -> Self {
        // UNMEASURED: initial viewport scale and scroll settings.
        Self {
            selected_track: None,
            selected_clip: None,
            pixels_per_bar: 84.0,
            scroll_bars: 0.0,
            scroll_tracks: 0.0,
        }
    }
}

pub enum PlaylistAction {
    AddTrack,
    SelectTrack(u64),
    AddClip {
        track: u64,
        pattern: u64,
        start: i64,
        length: i64,
    },
}

pub fn paint_playlist(
    paint: &mut PaintCtx<'_>,
    state: &mut PlaylistState,
    view: &PlaylistView,
    rect: Rect,
) -> Vec<PlaylistAction> {
    let mut actions = Vec::new();
    paint.panel(rect, false);
    let header_h = paint.layout.window_title_h;
    let header = Rect::from_min_size(rect.min, vec2(rect.width(), header_h));
    paint.panel(header, true);
    paint.label(header, &format!("Playlist · {}", view.pattern_name), false);
    let add_track_rect = Rect::from_min_size(
        pos2(rect.right() - 86.0, rect.top() + 3.0),
        vec2(80.0, 21.0),
    );
    if paint
        .button(
            "playlist.add-track",
            add_track_rect,
            "+ Track",
            false,
            true,
            "Adicionar faixa à Playlist",
        )
        .clicked()
    {
        actions.push(PlaylistAction::AddTrack);
    }

    // UNMEASURED: track header and ruler dimensions until a calibrated capture exists.
    let track_header_w = 142.0;
    let ruler_h = 24.0;
    let row_h = paint.layout.row_h + 9.0;
    let ruler = Rect::from_min_max(
        pos2(rect.left() + track_header_w, header.bottom()),
        pos2(rect.right(), header.bottom() + ruler_h),
    );
    let grid = Rect::from_min_max(
        pos2(ruler.left(), ruler.bottom()),
        pos2(rect.right(), rect.bottom()),
    );
    let track_names = Rect::from_min_max(
        pos2(rect.left(), grid.top()),
        pos2(ruler.left(), grid.bottom()),
    );
    let pointer_in_grid = paint.ui.rect_contains_pointer(grid);
    if pointer_in_grid || paint.ui.rect_contains_pointer(track_names) {
        let (delta, ctrl, shift) = paint
            .ui
            .input(|i| (i.smooth_scroll_delta.y, i.modifiers.ctrl, i.modifiers.shift));
        if ctrl {
            let next = state.pixels_per_bar
                * if delta > 0.0 {
                    1.1
                } else if delta < 0.0 {
                    0.9
                } else {
                    1.0
                };
            state.pixels_per_bar = next.clamp(24.0, 320.0);
        } else if shift {
            state.scroll_bars = (state.scroll_bars - delta / state.pixels_per_bar).max(0.0);
        } else {
            state.scroll_tracks = (state.scroll_tracks - delta / row_h).max(0.0);
        }
    }
    let pan_response = paint
        .ui
        .interact(grid, Id::new("playlist.pan"), Sense::click_and_drag());
    if pan_response.dragged_by(PointerButton::Middle) {
        let delta = paint.ui.input(|i| i.pointer.delta());
        state.scroll_bars = (state.scroll_bars - delta.x / state.pixels_per_bar).max(0.0);
        state.scroll_tracks = (state.scroll_tracks - delta.y / row_h).max(0.0);
    }

    paint
        .ui
        .painter()
        .rect_filled(ruler, 0.0, paint.tokens.raised);
    paint
        .ui
        .painter()
        .rect_filled(track_names, 0.0, paint.tokens.surface);
    paint
        .ui
        .painter()
        .rect_filled(grid, 0.0, paint.tokens.desktop);
    let grid_painter = paint.ui.painter().with_clip_rect(grid);
    let ruler_painter = paint.ui.painter().with_clip_rect(ruler);
    let first_bar = state.scroll_bars.floor() as usize;
    let bar_count = (grid.width() / state.pixels_per_bar).ceil() as usize + 2;
    for bar in first_bar..first_bar.saturating_add(bar_count) {
        let x = grid.left() + (bar as f32 - state.scroll_bars) * state.pixels_per_bar;
        grid_painter.line_segment(
            [pos2(x, grid.top()), pos2(x, grid.bottom())],
            Stroke::new(1.0, paint.tokens.grid),
        );
        ruler_painter.text(
            pos2(x + 4.0, ruler.center().y),
            Align2::LEFT_CENTER,
            (bar + 1).to_string(),
            FontId::proportional(paint.layout.font_small),
            paint.tokens.muted,
        );
    }

    for (index, track) in view.tracks.iter().enumerate() {
        let y = grid.top() + (index as f32 - state.scroll_tracks) * row_h;
        if y + row_h < grid.top() || y > grid.bottom() {
            continue;
        }
        let name_rect = Rect::from_min_size(
            pos2(track_names.left() + 2.0, y),
            vec2(track_names.width() - 4.0, row_h - 1.0),
        );
        let selected = state.selected_track == Some(track.id);
        let response = paint.ui.interact(
            name_rect,
            Id::new(("playlist.track", track.id)),
            Sense::click(),
        );
        paint.ui.painter().rect_filled(
            name_rect,
            0.0,
            if selected {
                paint.tokens.active
            } else {
                paint.tokens.raised
            },
        );
        paint.ui.painter().rect_stroke(
            name_rect,
            0.0,
            Stroke::new(1.0, paint.tokens.border),
            StrokeKind::Inside,
        );
        paint.ui.painter().text(
            pos2(name_rect.left() + 6.0, name_rect.center().y),
            Align2::LEFT_CENTER,
            &track.name,
            FontId::proportional(paint.layout.font_regular),
            if track.muted {
                paint.tokens.disabled
            } else {
                paint.tokens.text
            },
        );
        paint.probe.insert(
            format!("playlist.track.{}", track.id),
            name_rect,
            "track",
            true,
            Some(track.name.clone()),
        );
        if response.clicked() {
            state.selected_track = Some(track.id);
            actions.push(PlaylistAction::SelectTrack(track.id));
        }
        grid_painter.line_segment(
            [pos2(grid.left(), y + row_h), pos2(grid.right(), y + row_h)],
            Stroke::new(1.0, paint.tokens.grid),
        );

        let track_grid = Rect::from_min_size(pos2(grid.left(), y), vec2(grid.width(), row_h));
        let track_response = paint.ui.interact(
            track_grid,
            Id::new(("playlist.grid", track.id)),
            Sense::click(),
        );
        if track_response.clicked()
            && let Some(pointer) = track_response.interact_pointer_pos()
            && view.ticks_per_bar > 0
        {
            let bar = ((pointer.x - grid.left()) / state.pixels_per_bar + state.scroll_bars)
                .floor()
                .max(0.0);
            let start = bar as i64 * view.ticks_per_bar;
            let occupied = view.clips.iter().any(|clip| {
                clip.track == track.id && start >= clip.start && start < clip.start + clip.length
            });
            if !occupied {
                actions.push(PlaylistAction::AddClip {
                    track: track.id,
                    pattern: view.pattern_id,
                    start,
                    length: view.ticks_per_bar,
                });
            }
        }
        if track_response.hovered() {
            *paint.hint =
                "Clique numa área vazia para colocar o Pattern selecionado; Ctrl+roda ajusta zoom"
                    .to_owned();
        }
    }

    for clip in &view.clips {
        let Some(track_index) = view.tracks.iter().position(|track| track.id == clip.track) else {
            continue;
        };
        if view.ticks_per_bar <= 0 {
            continue;
        }
        let start_bar = clip.start as f32 / view.ticks_per_bar as f32;
        let length_bars = clip.length as f32 / view.ticks_per_bar as f32;
        let x = grid.left() + (start_bar - state.scroll_bars) * state.pixels_per_bar;
        let y = grid.top() + (track_index as f32 - state.scroll_tracks) * row_h;
        let clip_rect = Rect::from_min_size(
            pos2(x + 1.0, y + 2.0),
            vec2(
                (length_bars * state.pixels_per_bar - 2.0).max(3.0),
                row_h - 4.0,
            ),
        );
        if !clip_rect.intersects(grid) {
            continue;
        }
        let response = paint.ui.interact(
            clip_rect,
            Id::new(("playlist.clip", clip.id)),
            Sense::click(),
        );
        grid_painter.rect_filled(
            clip_rect,
            2.0,
            if state.selected_clip == Some(clip.id) {
                paint.tokens.active
            } else {
                paint.tokens.control
            },
        );
        grid_painter.rect_stroke(
            clip_rect,
            2.0,
            Stroke::new(1.0, paint.tokens.border),
            StrokeKind::Inside,
        );
        grid_painter.text(
            pos2(clip_rect.left() + 5.0, clip_rect.top() + 10.0),
            Align2::LEFT_CENTER,
            &clip.label,
            FontId::proportional(paint.layout.font_small),
            paint.tokens.text,
        );
        paint.probe.insert(
            format!("playlist.clip.{}", clip.id),
            clip_rect,
            "pattern-clip",
            true,
            Some(clip.label.clone()),
        );
        if response.clicked() {
            state.selected_clip = Some(clip.id);
        }
        if response.hovered() {
            *paint.hint = format!("Pattern Clip: {}", clip.label);
        }
    }

    if view.tracks.is_empty() {
        paint.ui.painter().text(
            pos2(rect.left() + 8.0, grid.top() + 18.0),
            Align2::LEFT_CENTER,
            "Nenhuma faixa. Use + Track.",
            FontId::proportional(paint.layout.font_regular),
            paint.tokens.muted,
        );
    }
    paint
        .probe
        .insert("playlist.grid", grid, "timeline", true, None);
    actions
}
