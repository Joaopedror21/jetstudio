//! Piano roll window [Manual: pianoroll]. Geometry is our own and UNMEASURED (no calibrated
//! capture of a Piano roll with notes exists yet); behaviour follows the manual and every edit
//! is a real engine command, committed when the gesture ends.

use std::collections::BTreeSet;

use egui::{Align2, Event, Key, Modifiers, Pos2, Rect, Stroke, StrokeKind, pos2, vec2};

use crate::theme;
use crate::widgets::{PaintCtx, framed};

const TOOLBAR_H: f32 = 28.0;
const RULER_H: f32 = 18.0;
const KEYS_W: f32 = 64.0;
const LANE_H: f32 = 72.0;
const EDGE_GRAB: f32 = 5.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Draw,
    Paint,
    Delete,
    Mute,
    Select,
}

impl Tool {
    const ALL: [(Tool, &'static str, &'static str, Key); 5] = [
        (Tool::Draw, "Draw", "Draw (P)", Key::P),
        (Tool::Paint, "Paint", "Paint (B)", Key::B),
        (Tool::Delete, "Delete", "Delete (D)", Key::D),
        (Tool::Mute, "Mute", "Mute (T)", Key::T),
        (Tool::Select, "Select", "Select (E)", Key::E),
    ];
}

/// Snap choices (a subset of FL's snap menu [Manual: pianoroll_menu, "Snap"]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Snap {
    None,
    QuarterStep,
    HalfStep,
    Step,
    Beat,
    Bar,
}

impl Snap {
    const ORDER: [Snap; 6] = [
        Snap::None,
        Snap::QuarterStep,
        Snap::HalfStep,
        Snap::Step,
        Snap::Beat,
        Snap::Bar,
    ];

    fn label(self) -> &'static str {
        match self {
            Snap::None => "(none)",
            Snap::QuarterStep => "1/4 step",
            Snap::HalfStep => "1/2 step",
            Snap::Step => "Step",
            Snap::Beat => "Beat",
            Snap::Bar => "Bar",
        }
    }

    pub fn ticks(self, view: &PianoRollView) -> i64 {
        match self {
            Snap::None => 1,
            Snap::QuarterStep => (view.step_ticks / 4).max(1),
            Snap::HalfStep => (view.step_ticks / 2).max(1),
            Snap::Step => view.step_ticks.max(1),
            Snap::Beat => view.beat_ticks.max(1),
            Snap::Bar => view.bar_ticks.max(1),
        }
    }

    fn next(self) -> Snap {
        let i = Self::ORDER.iter().position(|s| *s == self).unwrap_or(0);
        Self::ORDER[(i + 1) % Self::ORDER.len()]
    }
}

#[derive(Clone, Debug)]
enum Drag {
    Move { anchor: Pos2, dt: i64, dkey: i16 },
    Resize { anchor: Pos2, dl: i64 },
    Rect { from: Pos2, to: Pos2 },
    Paint { last_cell: i64 },
    Erase,
    Velocity { value: u8 },
}

pub struct PianoRollState {
    pub scroll_ticks: f32,
    /// Highest key shown at the top edge.
    pub top_key: f32,
    pub px_per_tick: f32,
    pub key_h: f32,
    pub tool: Tool,
    pub snap: Snap,
    pub selection: BTreeSet<usize>,
    pub last_length: i64,
    drag: Option<Drag>,
    /// Notes to select once the engine has re-sorted the score: (start, key, length).
    reselect: Vec<(i64, u8, i64)>,
    pub held_key: Option<u8>,
    /// Channel shown last frame; a new target scrolls to its notes.
    shown_channel: Option<u64>,
}

impl Default for PianoRollState {
    fn default() -> Self {
        Self {
            scroll_ticks: 0.0,
            top_key: 84.0,
            px_per_tick: 0.5,
            key_h: 12.0,
            tool: Tool::Draw,
            snap: Snap::Step,
            selection: BTreeSet::new(),
            last_length: 0,
            drag: None,
            reselect: Vec::new(),
            held_key: None,
            shown_channel: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoteView {
    pub start: i64,
    pub length: i64,
    pub key: u8,
    pub velocity: u8,
    pub muted: bool,
}

pub struct PianoRollView {
    pub channel: Option<(u64, String)>,
    pub notes: Vec<NoteView>,
    pub step_ticks: i64,
    pub beat_ticks: i64,
    pub bar_ticks: i64,
    pub playhead: Option<i64>,
}

pub enum PianoAction {
    Add {
        start: i64,
        length: i64,
        key: u8,
    },
    Delete(Vec<usize>),
    Move {
        indices: Vec<usize>,
        dt: i64,
        dkey: i16,
    },
    Resize {
        indices: Vec<usize>,
        delta: i64,
    },
    Velocity {
        indices: Vec<usize>,
        value: u8,
    },
    Mute {
        indices: Vec<usize>,
        muted: bool,
    },
    Transpose {
        indices: Vec<usize>,
        semitones: i16,
    },
    Quantize {
        indices: Vec<usize>,
        grid: i64,
        starts_only: bool,
    },
    Duplicate {
        indices: Vec<usize>,
        grid: i64,
    },
    Chop {
        indices: Vec<usize>,
        grid: i64,
    },
    Glue(Vec<usize>),
    DiscardLengths {
        indices: Vec<usize>,
        grid: i64,
    },
    KeyDown(u8),
    KeyUp(u8),
}

fn is_black(key: u8) -> bool {
    matches!(key % 12, 1 | 3 | 6 | 8 | 10)
}

fn note_name(key: u8) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    // FL names middle C (60) "C5" [Manual: chansettings_misc, "The default Root Key is C5"].
    format!("{}{}", NAMES[usize::from(key % 12)], key / 12)
}

struct Geometry {
    grid: Rect,
    lane: Rect,
    keys: Rect,
    ruler: Rect,
}

impl Geometry {
    fn x_of(&self, state: &PianoRollState, tick: i64) -> f32 {
        self.grid.left() + (tick as f32 - state.scroll_ticks) * state.px_per_tick
    }
    fn tick_at(&self, state: &PianoRollState, x: f32) -> i64 {
        ((x - self.grid.left()) / state.px_per_tick + state.scroll_ticks).floor() as i64
    }
    fn y_of(&self, state: &PianoRollState, key: u8) -> f32 {
        self.grid.top() + (state.top_key - f32::from(key)) * state.key_h
    }
    fn key_at(&self, state: &PianoRollState, y: f32) -> i32 {
        (state.top_key - ((y - self.grid.top()) / state.key_h).floor()) as i32
    }
}

fn snap_floor(tick: i64, grid: i64) -> i64 {
    tick.div_euclid(grid) * grid
}

fn snap_round(tick: i64, grid: i64) -> i64 {
    (tick + grid / 2).div_euclid(grid) * grid
}

/// Indices the operation applies to: the selection, or every note when nothing is selected
/// ("if nothing is selected, applies to all") [Manual: pianoroll_menu].
fn targets(state: &PianoRollState) -> Vec<usize> {
    state.selection.iter().copied().collect()
}

pub fn paint_piano_roll(
    paint: &mut PaintCtx<'_>,
    state: &mut PianoRollState,
    view: &PianoRollView,
    rect: Rect,
    focused: bool,
) -> Vec<PianoAction> {
    let mut actions = Vec::new();
    let t = *paint.tokens;
    let painter = paint.painter().with_clip_rect(rect);
    painter.rect_filled(rect, 0.0, t.grid_a);

    // Re-select moved/added notes after the engine re-sorted the score.
    if !state.reselect.is_empty() {
        state.selection = view
            .notes
            .iter()
            .enumerate()
            .filter(|(_, n)| state.reselect.contains(&(n.start, n.key, n.length)))
            .map(|(i, _)| i)
            .collect();
        state.reselect.clear();
    }
    state.selection.retain(|&i| i < view.notes.len());

    let toolbar = Rect::from_min_size(rect.min, vec2(rect.width(), TOOLBAR_H));
    let ruler = Rect::from_min_max(
        pos2(rect.left() + KEYS_W, toolbar.bottom()),
        pos2(rect.right(), toolbar.bottom() + RULER_H),
    );
    let lane = Rect::from_min_max(pos2(rect.left() + KEYS_W, rect.bottom() - LANE_H), rect.max);
    let grid = Rect::from_min_max(
        pos2(ruler.left(), ruler.bottom()),
        pos2(rect.right(), lane.top() - 2.0),
    );
    let keys = Rect::from_min_max(
        pos2(rect.left(), grid.top()),
        pos2(grid.left(), grid.bottom()),
    );
    let geo = Geometry {
        grid,
        lane,
        keys,
        ruler,
    };

    paint_toolbar(paint, &painter, state, view, toolbar);

    let Some((channel_id, _)) = &view.channel else {
        painter.text(
            grid.center(),
            Align2::CENTER_CENTER,
            "Select a channel in the Channel rack to edit its Piano roll",
            theme::regular(16.0),
            t.title_text,
        );
        return actions;
    };
    // Changing the target channel brings its score into view (our behaviour; FL's
    // "Auto locate" selects the first channel with notes [Manual: pianoroll]).
    if state.shown_channel != Some(*channel_id) {
        state.shown_channel = Some(*channel_id);
        state.selection.clear();
        let visible_keys = (grid.height() / state.key_h).max(1.0);
        state.top_key = match view.notes.iter().map(|n| n.key).max() {
            Some(high) => {
                let low = view.notes.iter().map(|n| n.key).min().unwrap_or(high);
                let center = (f32::from(high) + f32::from(low)) / 2.0;
                (center + visible_keys / 2.0).clamp(visible_keys.min(131.0), 131.0)
            }
            None => (72.0 + visible_keys / 2.0).min(131.0),
        };
        state.scroll_ticks = view
            .notes
            .iter()
            .map(|n| n.start)
            .min()
            .map_or(0.0, |start| (start - start % view.bar_ticks.max(1)) as f32);
    }

    // Scrolling and zoom: wheel scrolls keys, Shift+wheel scrolls time, Ctrl+wheel zooms.
    if paint
        .ui
        .rect_contains_pointer(Rect::from_min_max(keys.min, lane.max))
    {
        let (delta, modifiers) = paint.ui.input(|i| (i.smooth_scroll_delta, i.modifiers));
        if modifiers.ctrl {
            let factor = (1.0 + delta.y * 0.002).clamp(0.5, 2.0);
            state.px_per_tick = (state.px_per_tick * factor).clamp(0.05, 8.0);
        } else if modifiers.shift {
            state.scroll_ticks =
                (state.scroll_ticks - (delta.y + delta.x) / state.px_per_tick).max(0.0);
        } else {
            state.top_key = (state.top_key + delta.y / state.key_h).clamp(20.0, 131.0);
            state.scroll_ticks = (state.scroll_ticks - delta.x / state.px_per_tick).max(0.0);
        }
    }

    paint_grid(&painter, &t, state, view, &geo);
    paint_keyboard(paint, &painter, state, &geo, &mut actions);
    paint_ruler(&painter, &t, state, view, &geo);

    // Notes.
    let grid_ticks = state.snap.ticks(view);
    let (pointer, modifiers) = paint.ui.input(|i| (i.pointer.hover_pos(), i.modifiers));
    let grid_response = paint.ui.interact(
        grid,
        egui::Id::new("piano-roll.grid"),
        egui::Sense::click_and_drag(),
    );
    paint
        .probe
        .insert("piano-roll.grid", grid, "grid", true, None);
    let hovered_note = pointer.filter(|p| grid.contains(*p)).and_then(|p| {
        view.notes.iter().enumerate().rev().find_map(|(i, n)| {
            let r = note_rect(&geo, state, n);
            r.contains(p).then_some(i)
        })
    });
    let on_edge = hovered_note.zip(pointer).is_some_and(|(i, p)| {
        let r = note_rect(&geo, state, &view.notes[i]);
        p.x > r.right() - EDGE_GRAB
    });
    if hovered_note.is_some() {
        *paint.hint = if on_edge {
            "Drag to resize the note".to_owned()
        } else {
            "Drag to move; right-click deletes; double-click selects".to_owned()
        };
    }

    let (offset_dt, offset_dkey, offset_dl) = match &state.drag {
        Some(Drag::Move { dt, dkey, .. }) => (*dt, *dkey, 0),
        Some(Drag::Resize { dl, .. }) => (0, 0, *dl),
        _ => (0, 0, 0),
    };
    let note_painter = painter.with_clip_rect(grid);
    for (index, note) in view.notes.iter().enumerate() {
        let selected = state.selection.contains(&index);
        let mut shown = *note;
        if selected {
            shown.start += offset_dt;
            shown.key = (i16::from(shown.key) + offset_dkey).clamp(0, 131) as u8;
            shown.length = (shown.length + offset_dl).max(0);
        }
        let r = note_rect(&geo, state, &shown);
        let base = if note.muted { t.step_off } else { t.step_on };
        let fill = if selected {
            theme::shade(base, 1.25)
        } else {
            base
        };
        note_painter.rect_filled(r, 2.0, fill);
        note_painter.rect_stroke(
            r,
            2.0,
            Stroke::new(
                1.0,
                if selected {
                    t.select_on
                } else {
                    theme::shade(base, 0.55)
                },
            ),
            StrokeKind::Inside,
        );
        if r.width() > 26.0 && r.height() >= 10.0 {
            note_painter.text(
                pos2(r.left() + 3.0, r.center().y),
                Align2::LEFT_CENTER,
                note_name(shown.key),
                theme::regular(11.0),
                theme::shade(t.grid_a, 0.6),
            );
        }
    }
    if let Some(Drag::Rect { from, to }) = &state.drag {
        let r = Rect::from_two_pos(*from, *to);
        painter.rect_stroke(r, 0.0, Stroke::new(1.0, t.select_on), StrokeKind::Inside);
    }

    // Gestures.
    let press = grid_response.drag_started()
        || grid_response.clicked()
        || grid_response.secondary_clicked();
    let primary =
        grid_response.drag_started_by(egui::PointerButton::Primary) || grid_response.clicked();
    let secondary = grid_response.drag_started_by(egui::PointerButton::Secondary)
        || grid_response.secondary_clicked();
    if press && let Some(p) = pointer {
        let tick = geo.tick_at(state, p.x).max(0);
        let key = geo.key_at(state, p.y).clamp(0, 131) as u8;
        let selecting = state.tool == Tool::Select || (modifiers.ctrl && primary);
        if secondary
            && matches!(
                state.tool,
                Tool::Draw | Tool::Paint | Tool::Mute | Tool::Select
            )
        {
            // Right-click deletes; right-drag deletes many [Manual: pianoroll, "Draw (P)"].
            if let Some(i) = hovered_note {
                actions.push(PianoAction::Delete(vec![i]));
                state.selection.clear();
            } else if !modifiers.ctrl {
                state.selection.clear();
            }
            if grid_response.drag_started() {
                state.drag = Some(Drag::Erase);
            }
        } else if selecting && primary {
            if let Some(i) = hovered_note.filter(|_| grid_response.clicked()) {
                if modifiers.shift && !state.selection.insert(i) {
                    state.selection.remove(&i);
                } else if !modifiers.shift {
                    state.selection = [i].into();
                }
            } else if grid_response.drag_started() {
                if !modifiers.shift {
                    state.selection.clear();
                }
                state.drag = Some(Drag::Rect { from: p, to: p });
            } else {
                state.selection.clear();
            }
        } else if primary {
            match state.tool {
                Tool::Delete => {
                    if let Some(i) = hovered_note {
                        actions.push(PianoAction::Delete(vec![i]));
                        state.selection.clear();
                    }
                    if grid_response.drag_started() {
                        state.drag = Some(Drag::Erase);
                    }
                }
                Tool::Mute => {
                    if let Some(i) = hovered_note {
                        actions.push(PianoAction::Mute {
                            indices: vec![i],
                            muted: !view.notes[i].muted,
                        });
                    }
                }
                Tool::Draw | Tool::Paint | Tool::Select => {
                    if let Some(i) = hovered_note {
                        // Touching a note sets the default length [Manual: pianoroll, "equal to the last touched note"].
                        state.last_length = view.notes[i].length;
                        if !state.selection.contains(&i) {
                            state.selection = [i].into();
                        }
                        if grid_response.drag_started() {
                            state.drag = Some(if on_edge {
                                Drag::Resize { anchor: p, dl: 0 }
                            } else {
                                Drag::Move {
                                    anchor: p,
                                    dt: 0,
                                    dkey: 0,
                                }
                            });
                        }
                    } else {
                        let alt = modifiers.alt; // Alt bypasses snap.
                        let g = if alt { 1 } else { grid_ticks };
                        let start = snap_floor(tick, g);
                        let length = if state.last_length > 0 {
                            state.last_length
                        } else {
                            view.step_ticks.max(1)
                        };
                        actions.push(PianoAction::Add { start, length, key });
                        state.reselect = vec![(start, key, length)];
                        actions.push(PianoAction::KeyDown(key));
                        actions.push(PianoAction::KeyUp(key));
                        if state.tool == Tool::Paint && grid_response.drag_started() {
                            state.drag = Some(Drag::Paint {
                                last_cell: start / g,
                            });
                        }
                    }
                }
            }
        }
    }

    if grid_response.dragged()
        && let Some(p) = pointer
    {
        let pointer_tick = geo.tick_at(state, p.x).max(0);
        let pointer_key = geo.key_at(state, p.y).clamp(0, 131) as u8;
        let alt = modifiers.alt;
        let g = if alt { 1 } else { grid_ticks };
        match &mut state.drag {
            Some(Drag::Move { anchor, dt, dkey }) => {
                let raw = ((p.x - anchor.x) / state.px_per_tick).round() as i64;
                *dt = if modifiers.ctrl {
                    0
                } else {
                    snap_round(raw, g)
                };
                *dkey = if modifiers.shift {
                    0
                } else {
                    ((anchor.y - p.y) / state.key_h).round() as i16
                };
            }
            Some(Drag::Resize { anchor, dl }) => {
                let raw = ((p.x - anchor.x) / state.px_per_tick).round() as i64;
                *dl = snap_round(raw, g);
            }
            Some(Drag::Rect { to, .. }) => *to = p,
            Some(Drag::Paint { last_cell }) => {
                let cell = pointer_tick / g;
                if cell != *last_cell {
                    *last_cell = cell;
                    let key = pointer_key;
                    let length = if state.last_length > 0 {
                        state.last_length
                    } else {
                        view.step_ticks.max(1)
                    };
                    actions.push(PianoAction::Add {
                        start: cell * g,
                        length,
                        key,
                    });
                }
            }
            Some(Drag::Erase) => {
                if let Some(i) = hovered_note {
                    actions.push(PianoAction::Delete(vec![i]));
                    state.selection.clear();
                }
            }
            _ => {}
        }
    }

    if grid_response.drag_stopped() {
        match state.drag.take() {
            Some(Drag::Move { dt, dkey, .. }) if dt != 0 || dkey != 0 => {
                let indices = targets(state);
                state.reselect = indices
                    .iter()
                    .map(|&i| {
                        let n = view.notes[i];
                        (
                            n.start + dt,
                            (i16::from(n.key) + dkey).clamp(0, 131) as u8,
                            n.length,
                        )
                    })
                    .collect();
                actions.push(PianoAction::Move { indices, dt, dkey });
            }
            Some(Drag::Resize { dl, .. }) if dl != 0 => {
                let indices = targets(state);
                state.reselect = indices
                    .iter()
                    .map(|&i| {
                        let n = view.notes[i];
                        (n.start, n.key, (n.length + dl).max(0))
                    })
                    .collect();
                if let Some(&first) = indices.first() {
                    state.last_length = (view.notes[first].length + dl).max(0);
                }
                actions.push(PianoAction::Resize { indices, delta: dl });
            }
            Some(Drag::Rect { from, to }) => {
                let area = Rect::from_two_pos(from, to);
                for (i, n) in view.notes.iter().enumerate() {
                    if note_rect(&geo, state, n).intersects(area) {
                        state.selection.insert(i);
                    }
                }
            }
            _ => {}
        }
    }

    paint_velocity_lane(paint, &painter, state, view, &geo, &mut actions);

    if let Some(tick) = view.playhead {
        let x = geo.x_of(state, tick);
        if x >= grid.left() && x <= grid.right() {
            painter.line_segment(
                [pos2(x, ruler.top()), pos2(x, lane.bottom())],
                Stroke::new(1.0, t.select_on),
            );
        }
    }

    if focused {
        keyboard(paint, state, view, &mut actions);
    }
    actions
}

fn note_rect(geo: &Geometry, state: &PianoRollState, note: &NoteView) -> Rect {
    let x0 = geo.x_of(state, note.start);
    let w = (note.length.max(1) as f32 * state.px_per_tick).max(4.0);
    let y = geo.y_of(state, note.key);
    Rect::from_min_size(pos2(x0, y + 1.0), vec2(w, state.key_h - 1.0))
}

fn paint_toolbar(
    paint: &mut PaintCtx<'_>,
    painter: &egui::Painter,
    state: &mut PianoRollState,
    view: &PianoRollView,
    toolbar: Rect,
) {
    let t = *paint.tokens;
    painter.rect_filled(toolbar, 0.0, t.toolbar);
    let mut x = toolbar.left() + 6.0;
    for (tool, label, hint, _) in Tool::ALL {
        let r = Rect::from_min_size(
            pos2(x, toolbar.top() + 4.0),
            vec2(52.0, toolbar.height() - 8.0),
        );
        let response = paint.hit(&format!("piano-roll.tool.{label}"), r, "tool", true, hint);
        if state.tool == tool {
            crate::widgets::lit_face(painter, r, &t);
        } else {
            crate::widgets::button_face(painter, r, &t, response.hovered(), false);
        }
        painter.text(
            r.center(),
            Align2::CENTER_CENTER,
            label,
            theme::regular(13.0),
            if state.tool == tool {
                theme::shade(t.lit_edge, 0.5)
            } else {
                t.menu_text
            },
        );
        if response.clicked() {
            state.tool = tool;
        }
        x += 56.0;
    }
    let snap = Rect::from_min_size(
        pos2(x + 8.0, toolbar.top() + 4.0),
        vec2(90.0, toolbar.height() - 8.0),
    );
    framed(painter, snap, 2.0, t.lcd_bg, t.lcd_edge);
    painter.text(
        snap.center(),
        Align2::CENTER_CENTER,
        format!("Snap: {}", state.snap.label()),
        theme::regular(13.0),
        t.lcd_text,
    );
    if paint
        .hit(
            "piano-roll.snap",
            snap,
            "lcd",
            true,
            "Snap (click to change; Alt bypasses snap)",
        )
        .clicked()
    {
        state.snap = state.snap.next();
    }
    let channel = view
        .channel
        .as_ref()
        .map_or_else(|| "(none)".to_owned(), |(_, name)| name.clone());
    painter.text(
        pos2(snap.right() + 12.0, toolbar.center().y),
        Align2::LEFT_CENTER,
        format!("Target: {channel}"),
        theme::regular(14.0),
        t.menu_text,
    );
}

fn paint_grid(
    painter: &egui::Painter,
    t: &theme::Tokens,
    state: &PianoRollState,
    view: &PianoRollView,
    geo: &Geometry,
) {
    let grid = geo.grid;
    let gp = painter.with_clip_rect(grid);
    // Key rows: black-key rows darker.
    let first = geo.key_at(state, grid.bottom()).max(0);
    let last = geo.key_at(state, grid.top()).min(131);
    for key in first..=last {
        let y = geo.y_of(state, key as u8);
        let row = Rect::from_min_max(pos2(grid.left(), y), pos2(grid.right(), y + state.key_h));
        gp.rect_filled(
            row,
            0.0,
            if is_black(key as u8) {
                t.grid_b
            } else {
                t.grid_a
            },
        );
        if key % 12 == 0 {
            gp.line_segment(
                [
                    pos2(grid.left(), y + state.key_h),
                    pos2(grid.right(), y + state.key_h),
                ],
                Stroke::new(1.0, t.bar_a),
            );
        }
    }
    // Vertical lines: steps, beats, bars.
    let start = geo.tick_at(state, grid.left()).max(0);
    let end = geo.tick_at(state, grid.right());
    let step = view.step_ticks.max(1);
    let mut tick = start / step * step;
    while tick <= end {
        let x = geo.x_of(state, tick);
        let color = if tick % view.bar_ticks.max(1) == 0 {
            t.bar_a
        } else if tick % view.beat_ticks.max(1) == 0 {
            t.beat_a
        } else {
            theme::shade(t.grid_b, 0.92)
        };
        if state.px_per_tick * step as f32 >= 4.0 || tick % view.beat_ticks.max(1) == 0 {
            gp.line_segment(
                [pos2(x, grid.top()), pos2(x, grid.bottom())],
                Stroke::new(1.0, color),
            );
        }
        tick += step;
    }
}

fn paint_ruler(
    painter: &egui::Painter,
    t: &theme::Tokens,
    state: &PianoRollState,
    view: &PianoRollView,
    geo: &Geometry,
) {
    let ruler = geo.ruler;
    painter.rect_filled(ruler, 0.0, t.ruler_bg);
    let rp = painter.with_clip_rect(ruler);
    let bar = view.bar_ticks.max(1);
    let first = (geo.tick_at(state, ruler.left()).max(0) / bar) * bar;
    let mut tick = first;
    while geo.x_of(state, tick) <= ruler.right() {
        let x = geo.x_of(state, tick);
        rp.text(
            pos2(x + 3.0, ruler.center().y),
            Align2::LEFT_CENTER,
            (tick / bar + 1).to_string(),
            theme::regular(13.0),
            t.ruler_text,
        );
        tick += bar;
    }
}

fn paint_keyboard(
    paint: &mut PaintCtx<'_>,
    painter: &egui::Painter,
    state: &mut PianoRollState,
    geo: &Geometry,
    actions: &mut Vec<PianoAction>,
) {
    let keys = geo.keys;
    let kp = painter.with_clip_rect(keys);
    let first = geo.key_at(state, keys.bottom()).max(0);
    let last = geo.key_at(state, keys.top()).min(131);
    for key in first..=last {
        let key = key as u8;
        let y = geo.y_of(state, key);
        let r = Rect::from_min_max(pos2(keys.left(), y), pos2(keys.right(), y + state.key_h));
        let held = state.held_key == Some(key);
        let fill = match (is_black(key), held) {
            (_, true) => egui::Color32::from_rgb(0xF2, 0x8B, 0x3D),
            (true, false) => egui::Color32::from_gray(0x2A),
            (false, false) => egui::Color32::from_gray(0xE6),
        };
        let r = if is_black(key) {
            Rect::from_min_max(r.min, pos2(keys.right() - 18.0, r.max.y))
        } else {
            r
        };
        kp.rect_filled(r.shrink2(vec2(0.0, 0.5)), 1.0, fill);
        if key.is_multiple_of(12) {
            kp.text(
                pos2(keys.right() - 3.0, r.center().y),
                Align2::RIGHT_CENTER,
                note_name(key),
                theme::regular(10.0),
                egui::Color32::from_gray(0x40),
            );
        }
    }
    // Preview keyboard: press to hear the key [Manual: pianoroll, "Preview Keyboard"].
    let response = paint.ui.interact(
        keys,
        egui::Id::new("piano-roll.keys"),
        egui::Sense::click_and_drag(),
    );
    paint
        .probe
        .insert("piano-roll.keys", keys, "keyboard", true, None);
    let pointer = paint.ui.input(|i| i.pointer.hover_pos());
    let down = paint.ui.input(|i| i.pointer.primary_down());
    if response.is_pointer_button_down_on()
        && down
        && let Some(p) = pointer
    {
        let key = geo.key_at(state, p.y).clamp(0, 131) as u8;
        if state.held_key != Some(key) {
            if let Some(old) = state.held_key.take() {
                actions.push(PianoAction::KeyUp(old));
            }
            state.held_key = Some(key);
            actions.push(PianoAction::KeyDown(key));
        }
    } else if let Some(old) = state.held_key.take() {
        actions.push(PianoAction::KeyUp(old));
    }
}

fn paint_velocity_lane(
    paint: &mut PaintCtx<'_>,
    painter: &egui::Painter,
    state: &mut PianoRollState,
    view: &PianoRollView,
    geo: &Geometry,
    actions: &mut Vec<PianoAction>,
) {
    let t = *paint.tokens;
    let lane = geo.lane;
    painter.rect_filled(lane, 0.0, t.ruler_bg);
    let lp = painter.with_clip_rect(lane);
    lp.text(
        pos2(lane.left() - KEYS_W + 4.0, lane.top() + 10.0),
        Align2::LEFT_CENTER,
        "Velocity",
        theme::regular(12.0),
        t.ruler_text,
    );
    let response = paint.ui.interact(
        lane,
        egui::Id::new("piano-roll.velocity"),
        egui::Sense::click_and_drag(),
    );
    paint
        .probe
        .insert("piano-roll.velocity", lane, "event-editor", true, None);
    let pointer = paint.ui.input(|i| i.pointer.hover_pos());
    let value_at = |y: f32| {
        (((lane.bottom() - 4.0 - y) / (lane.height() - 8.0)) * 127.0)
            .round()
            .clamp(0.0, 127.0) as u8
    };
    if (response.dragged() || response.clicked())
        && let Some(p) = pointer
    {
        state.drag = Some(Drag::Velocity {
            value: value_at(p.y),
        });
    }
    let live = match state.drag {
        Some(Drag::Velocity { value }) => Some(value),
        _ => None,
    };
    let near = pointer.filter(|p| lane.contains(*p)).and_then(|p| {
        view.notes
            .iter()
            .enumerate()
            .min_by_key(|(_, n)| (geo.x_of(state, n.start) - p.x).abs() as i64)
            .filter(|(_, n)| (geo.x_of(state, n.start) - p.x).abs() < 8.0)
            .map(|(i, _)| i)
    });
    for (i, note) in view.notes.iter().enumerate() {
        let x = geo.x_of(state, note.start);
        let affected = live.is_some()
            && if state.selection.is_empty() {
                Some(i) == near
            } else {
                state.selection.contains(&i)
            };
        let velocity = if affected {
            live.unwrap_or(note.velocity)
        } else {
            note.velocity
        };
        let h = f32::from(velocity) / 127.0 * (lane.height() - 8.0);
        let top = lane.bottom() - 4.0 - h;
        lp.line_segment(
            [pos2(x, lane.bottom() - 4.0), pos2(x, top)],
            Stroke::new(2.0, t.step_on),
        );
        lp.circle_filled(pos2(x, top), 3.0, t.step_on);
    }
    if (response.drag_stopped() || response.clicked())
        && let Some(Drag::Velocity { value }) = state.drag.take()
    {
        let indices: Vec<usize> = if state.selection.is_empty() {
            near.into_iter().collect()
        } else {
            targets(state)
        };
        if !indices.is_empty() {
            actions.push(PianoAction::Velocity { indices, value });
        }
    }
}

fn keyboard(
    paint: &mut PaintCtx<'_>,
    state: &mut PianoRollState,
    view: &PianoRollView,
    actions: &mut Vec<PianoAction>,
) {
    let events = paint.ui.input(|i| i.events.clone());
    let grid = state.snap.ticks(view);
    for event in events {
        let Event::Key {
            key,
            pressed: true,
            modifiers,
            ..
        } = event
        else {
            continue;
        };
        let sel = targets(state);
        let m = |ctrl: bool, shift: bool, alt: bool| Modifiers {
            ctrl,
            shift,
            alt,
            ..Default::default()
        };
        let plain = modifiers.is_none();
        let only = |want: Modifiers| {
            modifiers.ctrl == want.ctrl
                && modifiers.shift == want.shift
                && modifiers.alt == want.alt
        };
        match key {
            Key::Delete => {
                // "If no selection is made - deletes all Notes" [Manual: pianoroll_menu, "Delete"].
                actions.push(PianoAction::Delete(sel));
                state.selection.clear();
            }
            Key::A if only(m(true, false, false)) => {
                state.selection = (0..view.notes.len()).collect()
            }
            Key::ArrowUp | Key::ArrowDown
                if only(m(false, true, false)) || only(m(true, false, false)) =>
            {
                let octave = modifiers.ctrl;
                let sign = if key == Key::ArrowUp { 1 } else { -1 };
                let semitones = sign * if octave { 12 } else { 1 };
                state.reselect = sel
                    .iter()
                    .map(|&i| {
                        let n = view.notes[i];
                        (
                            n.start,
                            (i16::from(n.key) + semitones).clamp(0, 131) as u8,
                            n.length,
                        )
                    })
                    .collect();
                actions.push(PianoAction::Transpose {
                    indices: sel,
                    semitones,
                });
            }
            Key::B if only(m(true, false, false)) => {
                actions.push(PianoAction::Duplicate { indices: sel, grid })
            }
            Key::Q if only(m(true, false, false)) => actions.push(PianoAction::Quantize {
                indices: sel,
                grid,
                starts_only: false,
            }),
            Key::Q if only(m(false, true, false)) => actions.push(PianoAction::Quantize {
                indices: sel,
                grid,
                starts_only: true,
            }),
            Key::U if only(m(true, false, false)) => {
                actions.push(PianoAction::Chop { indices: sel, grid })
            }
            Key::G if only(m(true, false, false)) => actions.push(PianoAction::Glue(sel)),
            Key::D if only(m(false, true, false)) => {
                actions.push(PianoAction::DiscardLengths { indices: sel, grid })
            }
            _ if plain => {
                if let Some((tool, ..)) = Tool::ALL.iter().find(|(_, _, _, k)| *k == key) {
                    state.tool = *tool;
                }
            }
            _ => {}
        }
    }
}
