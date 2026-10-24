//! Piano roll window [Manual: pianoroll]. Geometry is our own and UNMEASURED (no calibrated
//! capture of a Piano roll with notes exists yet); behaviour follows the manual and every edit
//! is a real engine command, committed when the gesture ends.

use std::collections::BTreeSet;

use egui::{Align2, Event, Key, Modifiers, Pos2, Rect, Stroke, StrokeKind, pos2, vec2};

use crate::snap::{GridTicks, SnapValue};
use crate::theme;
use crate::widgets::{PaintCtx, framed};

/// Two rows: edit tools and snap on top; file, tools menu, scale and stamp below.
const TOOLBAR_H: f32 = 54.0;
const ROW_H: f32 = 27.0;
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

#[derive(Clone, Debug)]
enum Drag {
    Move {
        anchor: Pos2,
        dt: i64,
        dkey: i16,
    },
    Resize {
        anchor: Pos2,
        dl: i64,
    },
    Rect {
        from: Pos2,
        to: Pos2,
    },
    Paint {
        last_cell: i64,
    },
    Erase,
    /// Event editor drag: the value, and with Shift the x span dragged across.
    Lane {
        value: i32,
        across: Option<(f32, f32)>,
    },
}

pub struct PianoRollState {
    pub scroll_ticks: f32,
    /// Highest key shown at the top edge.
    pub top_key: f32,
    pub px_per_tick: f32,
    pub key_h: f32,
    pub tool: Tool,
    pub snap: SnapValue,
    pub selection: BTreeSet<usize>,
    pub last_length: i64,
    drag: Option<Drag>,
    /// Notes to select once the engine has re-sorted the score: (start, key, length).
    reselect: Vec<(i64, u8, i64)>,
    pub held_key: Option<u8>,
    /// Channel shown last frame; a new target scrolls to its notes.
    shown_channel: Option<u64>,
    /// MIDI file button clicked: `Some(true)` import, `Some(false)` export.
    file_request: Option<bool>,
    /// Snap to scale: (root 0..11, index into `SCALES`) [Manual: pianoroll, "Snap to scale"].
    pub scale: Option<(u8, usize)>,
    /// Chord stamp shape (index into `CHORDS`) [Manual: pianoroll, "Chord Stamp"].
    pub stamp: Option<usize>,
    /// New notes become slide notes (S) or portamento notes (O) [Manual: pianoroll,
    /// "Slide & Portamento - These are flags that can be set per-note"].
    pub slide_mode: bool,
    pub porta_mode: bool,
    pub lane_target: LaneTarget,
    snap_menu: Option<egui::Pos2>,
    tools_menu: Option<egui::Pos2>,
}

fn in_scale(key: u8, scale: Option<(u8, usize)>) -> bool {
    match scale {
        Some((root, index)) => jet_engine::SCALES
            .get(index)
            .is_some_and(|(_, set)| set.contains(&((key + 12 - root % 12) % 12))),
        None => true,
    }
}

/// Nearest in-scale key (ties go down), for notes added while Snap to scale is on.
fn snap_key(key: u8, scale: Option<(u8, usize)>) -> u8 {
    (0..=6u8)
        .flat_map(|d| [key.checked_sub(d), key.checked_add(d)])
        .flatten()
        .find(|k| *k <= 131 && in_scale(*k, scale))
        .unwrap_or(key)
}

impl Default for PianoRollState {
    fn default() -> Self {
        Self {
            scroll_ticks: 0.0,
            top_key: 84.0,
            px_per_tick: 0.5,
            key_h: 12.0,
            tool: Tool::Draw,
            snap: SnapValue::Main,
            selection: BTreeSet::new(),
            last_length: 0,
            drag: None,
            reselect: Vec::new(),
            held_key: None,
            shown_channel: None,
            file_request: None,
            scale: None,
            stamp: None,
            slide_mode: false,
            porta_mode: false,
            lane_target: LaneTarget::Velocity,
            snap_menu: None,
            tools_menu: None,
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
    pub slide: bool,
    pub porta: bool,
    pub pan: i16,
    pub release: u8,
    pub fine_pitch: i16,
    pub mod_x: u8,
    pub mod_y: u8,
}

pub struct PianoRollView {
    pub channel: Option<(u64, String)>,
    pub notes: Vec<NoteView>,
    pub step_ticks: i64,
    pub beat_ticks: i64,
    pub bar_ticks: i64,
    /// The toolbar's Global snap, used when the local snap is Main.
    pub global_snap: SnapValue,
    pub playhead: Option<i64>,
    /// Typing keyboard to piano is on: plain letter keys play notes instead of tools.
    pub typing: bool,
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
    /// Set a note property (`note.props.set` field) on notes.
    Prop {
        indices: Vec<usize>,
        field: &'static str,
        value: i32,
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
    Copy(Vec<usize>),
    Cut(Vec<usize>),
    /// Paste at the leftmost visible bar [Manual: pianoroll_menu, "Paste (Ctrl+V)"].
    Paste {
        at: i64,
    },
    ImportMidi,
    ExportMidi,
    /// A tool from the Tools menu applied to the selection (or all notes).
    Tool {
        name: &'static str,
        indices: Vec<usize>,
    },
    SnapScale {
        indices: Vec<usize>,
        root: u8,
        scale: &'static str,
    },
    Stamp {
        start: i64,
        length: i64,
        root: u8,
        chord: &'static str,
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
    let grid_ticks = snap_ticks(state, view);
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
        // "Slides are shown as horizontal bars with a small triangle drawn in the right side";
        // portamento notes carry a flag (ours: a small triangle at the left edge, pointing in).
        let mark = theme::shade(base, 0.45);
        if note.slide && r.width() > 6.0 {
            let h = r.height() * 0.8;
            note_painter.add(egui::Shape::convex_polygon(
                vec![
                    pos2(r.right() - 2.0, r.center().y),
                    pos2(r.right() - 2.0 - h * 0.7, r.center().y - h / 2.0),
                    pos2(r.right() - 2.0 - h * 0.7, r.center().y + h / 2.0),
                ],
                mark,
                Stroke::NONE,
            ));
        }
        if note.porta && r.width() > 6.0 {
            let h = r.height() * 0.6;
            note_painter.add(egui::Shape::convex_polygon(
                vec![
                    pos2(r.left() + 2.0, r.top() + 2.0),
                    pos2(r.left() + 2.0 + h, r.top() + 2.0),
                    pos2(r.left() + 2.0, r.top() + 2.0 + h),
                ],
                mark,
                Stroke::NONE,
            ));
        }
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
    let press_pos = paint.ui.input(|i| i.pointer.press_origin()).or(pointer);
    if press && let Some(p) = press_pos {
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
                        let key = snap_key(key, state.scale);
                        if let Some(chord) = state
                            .stamp
                            .and_then(|i| jet_engine::CHORDS.get(i).map(|(name, _)| name))
                        {
                            actions.push(PianoAction::Stamp {
                                start,
                                length,
                                root: key,
                                chord,
                            });
                        } else {
                            actions.push(PianoAction::Add { start, length, key });
                            state.reselect = vec![(start, key, length)];
                        }
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
    match state.file_request.take() {
        Some(true) => actions.push(PianoAction::ImportMidi),
        Some(false) => actions.push(PianoAction::ExportMidi),
        None => {}
    }
    paint_tools_menu(paint, state, &mut actions);
    paint_snap_menu(paint, state);
    actions
}

fn paint_tools_menu(
    paint: &mut PaintCtx<'_>,
    state: &mut PianoRollState,
    actions: &mut Vec<PianoAction>,
) {
    let Some(origin) = state.tools_menu else {
        return;
    };
    let entries: [(&str, &'static str); 7] = [
        ("Flip horizontally", "flip-h"),
        ("Flip vertically", "flip-v"),
        ("Strum (down)", "strum"),
        ("Arpeggiate up (1/2 beat)", "arp-up"),
        ("Arpeggiate down (1/2 beat)", "arp-down"),
        ("Limit to C4 - B5", "limit"),
        ("Snap notes to scale", "snap-scale"),
    ];
    let items: Vec<crate::widgets::MenuItem<'_>> = entries
        .iter()
        .map(|(label, name)| crate::widgets::MenuItem {
            label,
            shortcut: "",
            enabled: *name != "snap-scale" || state.scale.is_some(),
        })
        .collect();
    match paint.popup_menu("piano-roll.tools-menu", origin, &items) {
        crate::widgets::MenuResult::Open => {}
        crate::widgets::MenuResult::Dismissed => state.tools_menu = None,
        crate::widgets::MenuResult::Chosen(i) => {
            state.tools_menu = None;
            let indices = targets(state);
            let Some((_, name)) = entries.get(i) else {
                return;
            };
            if *name == "snap-scale" {
                if let Some((root, scale)) = state.scale {
                    actions.push(PianoAction::SnapScale {
                        indices,
                        root,
                        scale: jet_engine::SCALES[scale].0,
                    });
                }
            } else {
                actions.push(PianoAction::Tool { name, indices });
            }
        }
    }
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
        let r = Rect::from_min_size(pos2(x, toolbar.top() + 4.0), vec2(52.0, ROW_H - 6.0));
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
    let snap = Rect::from_min_size(pos2(x + 8.0, toolbar.top() + 4.0), vec2(90.0, ROW_H - 6.0));
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
            &format!(
                "Snap: {} (Main follows the Global snap, now {}); Alt bypasses snap",
                state.snap.label(),
                view.global_snap.label()
            ),
        )
        .clicked()
    {
        state.snap_menu = Some(snap.left_bottom());
    }
    // Second row: MIDI file import/export [Manual: pianoroll_menu, "File"].
    let row2 = toolbar.top() + ROW_H;
    let mut bx = toolbar.left() + 6.0;
    for (key, label, hint) in [
        ("import", "Import MIDI", "Import MIDI file... (adds notes)"),
        ("export", "Export MIDI", "Export as MIDI file..."),
    ] {
        let b = Rect::from_min_size(pos2(bx, row2 + 3.0), vec2(84.0, ROW_H - 6.0));
        if paint.toggle(&format!("piano-roll.{key}"), b, label, false, hint) {
            state.file_request = Some(key == "import");
        }
        bx += 88.0;
    }
    // Tools menu, Snap to scale and Chord stamp selectors [Manual: pianoroll, "Toolbar icons"].
    let tools = Rect::from_min_size(pos2(bx, row2 + 3.0), vec2(60.0, ROW_H - 6.0));
    if paint.toggle(
        "piano-roll.tools",
        tools,
        "Tools",
        false,
        "Piano roll tools (flip, strum, arpeggiate, limit, snap to scale)",
    ) {
        state.tools_menu = Some(pos2(tools.left(), tools.bottom()));
    }
    bx += 64.0;
    const ROOTS: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let scale_label = state.scale.map_or_else(
        || "Scale: off".to_owned(),
        |(root, i)| {
            format!(
                "{} {}",
                ROOTS[usize::from(root % 12)],
                jet_engine::SCALES[i].0
            )
        },
    );
    let scale_button = Rect::from_min_size(pos2(bx, row2 + 3.0), vec2(150.0, ROW_H - 6.0));
    let scale_response = paint.hit(
        "piano-roll.scale",
        scale_button,
        "toggle",
        true,
        "Snap to scale: click to change the scale, right-click to change the root",
    );
    if state.scale.is_some() {
        crate::widgets::lit_face(painter, scale_button, &t);
    } else {
        crate::widgets::button_face(painter, scale_button, &t, scale_response.hovered(), false);
    }
    painter.text(
        scale_button.center(),
        Align2::CENTER_CENTER,
        &scale_label,
        theme::regular(12.0),
        if state.scale.is_some() {
            theme::shade(t.lit_edge, 0.5)
        } else {
            t.menu_text
        },
    );
    if scale_response.clicked() {
        state.scale = match state.scale {
            None => Some((0, 0)),
            Some((_, i)) if i + 1 >= jet_engine::SCALES.len() => None,
            Some((root, i)) => Some((root, i + 1)),
        };
    }
    if scale_response.secondary_clicked()
        && let Some((root, i)) = state.scale
    {
        state.scale = Some(((root + 1) % 12, i));
    }
    bx += 154.0;
    let stamp_label = state
        .stamp
        .and_then(|i| jet_engine::CHORDS.get(i).map(|(name, _)| name))
        .map_or_else(|| "Stamp: off".to_owned(), |c| format!("Stamp: {c}"));
    let stamp_button = Rect::from_min_size(pos2(bx, row2 + 3.0), vec2(130.0, ROW_H - 6.0));
    if paint.toggle(
        "piano-roll.stamp",
        stamp_button,
        &stamp_label,
        state.stamp.is_some(),
        "Chord stamp: clicking the grid places this chord",
    ) {
        state.stamp = match state.stamp {
            None => Some(0),
            Some(i) if i + 1 >= jet_engine::CHORDS.len() => None,
            Some(i) => Some(i + 1),
        };
    }
    bx += 134.0;
    let slide_button = Rect::from_min_size(pos2(bx, row2 + 3.0), vec2(56.0, ROW_H - 6.0));
    if paint.toggle(
        "piano-roll.slide",
        slide_button,
        "Slide",
        state.slide_mode,
        "Slide toggle (S): new notes are slide notes, which bend the notes they overlap",
    ) {
        state.slide_mode = !state.slide_mode;
    }
    bx += 60.0;
    let porta_button = Rect::from_min_size(pos2(bx, row2 + 3.0), vec2(56.0, ROW_H - 6.0));
    if paint.toggle(
        "piano-roll.porta",
        porta_button,
        "Porta",
        state.porta_mode,
        "Portamento toggle (O): new notes slide in pitch from the previous note",
    ) {
        state.porta_mode = !state.porta_mode;
    }
    let channel = view
        .channel
        .as_ref()
        .map_or_else(|| "(none)".to_owned(), |(_, name)| name.clone());
    painter.text(
        pos2(snap.right() + 12.0, toolbar.top() + ROW_H / 2.0),
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
        // With Snap to scale on, in-scale rows are highlighted [Manual: pianoroll, "the background
        // grid of the Piano roll will show notes of that scale highlighted in a lighter color"].
        let base = if is_black(key as u8) {
            t.grid_b
        } else {
            t.grid_a
        };
        let fill = if state.scale.is_some() && in_scale(key as u8, state.scale) {
            theme::shade(base, 1.18)
        } else {
            base
        };
        gp.rect_filled(row, 0.0, fill);
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

/// Note property shown in the event editor [Manual: pianoroll, "Note Properties & Event
/// Editor": "velocity, pan, release (where supported), Mod X & Y (where supported), pitch"].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LaneTarget {
    #[default]
    Velocity,
    Pan,
    Release,
    FinePitch,
    ModX,
    ModY,
}

impl LaneTarget {
    pub const ALL: [Self; 6] = [
        Self::Velocity,
        Self::Pan,
        Self::Release,
        Self::FinePitch,
        Self::ModX,
        Self::ModY,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Velocity => "Velocity",
            Self::Pan => "Pan",
            Self::Release => "Release",
            Self::FinePitch => "Fine pitch",
            Self::ModX => "Mod X",
            Self::ModY => "Mod Y",
        }
    }

    /// Field name in `note.props.set`.
    pub fn field(self) -> &'static str {
        match self {
            Self::Velocity => "velocity",
            Self::Pan => "pan",
            Self::Release => "release",
            Self::FinePitch => "fine_pitch",
            Self::ModX => "mod_x",
            Self::ModY => "mod_y",
        }
    }

    fn range(self) -> (i32, i32) {
        match self {
            Self::Pan => (-1000, 1000),
            Self::FinePitch => (-1200, 1200),
            Self::Velocity | Self::Release | Self::ModX | Self::ModY => (0, 127),
        }
    }

    /// "To reset a property, use (Alt/Opt+Left-Click)": the value a new note gets.
    fn default_value(self) -> i32 {
        match self {
            Self::Velocity => i32::from(jet_model::DEFAULT_VELOCITY),
            _ => 0,
        }
    }

    fn get(self, note: &NoteView) -> i32 {
        match self {
            Self::Velocity => i32::from(note.velocity),
            Self::Pan => i32::from(note.pan),
            Self::Release => i32::from(note.release),
            Self::FinePitch => i32::from(note.fine_pitch),
            Self::ModX => i32::from(note.mod_x),
            Self::ModY => i32::from(note.mod_y),
        }
    }

    fn bipolar(self) -> bool {
        matches!(self, Self::Pan | Self::FinePitch)
    }

    fn cycle(self, step: isize) -> Self {
        let i = Self::ALL.iter().position(|t| *t == self).unwrap_or(0) as isize;
        Self::ALL[(i + step).rem_euclid(Self::ALL.len() as isize) as usize]
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
    let target = state.lane_target;
    painter.rect_filled(lane, 0.0, t.ruler_bg);
    let lp = painter.with_clip_rect(lane);

    // Target control: click for the next target, right-click for the previous one, mouse wheel
    // over it or Shift+F cycles [Manual: pianoroll, "Selecting Targets"].
    let selector = Rect::from_min_size(
        pos2(lane.left() - KEYS_W + 2.0, lane.top() + 2.0),
        vec2(KEYS_W - 4.0, 18.0),
    );
    let selector_response = paint.hit(
        "piano-roll.lane-target",
        selector,
        "menu",
        true,
        "Event editor target: click or mouse wheel to change (Shift+F)",
    );
    painter.text(
        pos2(selector.left() + 2.0, selector.center().y),
        Align2::LEFT_CENTER,
        format!("{} >", target.label()),
        theme::regular(12.0),
        t.ruler_text,
    );
    if selector_response.clicked() {
        state.lane_target = target.cycle(1);
    }
    if selector_response.secondary_clicked() {
        state.lane_target = target.cycle(-1);
    }
    if selector_response.hovered() {
        // One target per wheel notch (MouseWheel events, not the smoothed scroll).
        let wheel: f32 = paint.ui.input(|i| {
            i.events
                .iter()
                .map(|e| match e {
                    Event::MouseWheel { delta, .. } => delta.y,
                    _ => 0.0,
                })
                .sum()
        });
        if wheel != 0.0 {
            state.lane_target = target.cycle(if wheel < 0.0 { 1 } else { -1 });
        }
    }

    let response = paint.ui.interact(
        lane,
        egui::Id::new("piano-roll.velocity"),
        egui::Sense::click_and_drag(),
    );
    paint
        .probe
        .insert("piano-roll.velocity", lane, "event-editor", true, None);
    let (min, max) = target.range();
    let span = (max - min) as f32;
    let inner = lane.height() - 8.0;
    let y_of = |value: i32| lane.bottom() - 4.0 - (value - min) as f32 / span * inner;
    let value_at = |y: f32| {
        (min as f32 + ((lane.bottom() - 4.0 - y) / inner).clamp(0.0, 1.0) * span).round() as i32
    };
    let (pointer, shift, alt) = paint
        .ui
        .input(|i| (i.pointer.hover_pos(), i.modifiers.shift, i.modifiers.alt));
    let near = pointer.filter(|p| lane.contains(*p)).and_then(|p| {
        view.notes
            .iter()
            .enumerate()
            .min_by_key(|(_, n)| (geo.x_of(state, n.start) - p.x).abs() as i64)
            .filter(|(_, n)| (geo.x_of(state, n.start) - p.x).abs() < 8.0)
            .map(|(i, _)| i)
    });
    if (response.dragged() || response.clicked())
        && let Some(p) = pointer
    {
        let origin = paint.ui.input(|i| i.pointer.press_origin()).unwrap_or(p);
        state.drag = Some(if alt {
            Drag::Lane {
                value: target.default_value(),
                across: None,
            }
        } else if shift {
            // "To set each note's property to the same value, use (Shift+Left-Click) and drag
            // across the area that should be affected": the value is where the drag started.
            Drag::Lane {
                value: value_at(origin.y),
                across: Some((origin.x.min(p.x), origin.x.max(p.x))),
            }
        } else {
            Drag::Lane {
                value: value_at(p.y),
                across: None,
            }
        });
    }
    let (live, across) = match state.drag {
        Some(Drag::Lane { value, across }) => (Some(value), across),
        _ => (None, None),
    };
    let in_span = |x: f32| across.is_some_and(|(a, b)| x >= a - 4.0 && x <= b + 4.0);
    let affected = |i: usize, x: f32| {
        live.is_some()
            && if across.is_some() {
                in_span(x)
            } else if state.selection.is_empty() {
                Some(i) == near
            } else {
                state.selection.contains(&i)
            }
    };
    let baseline = if target.bipolar() {
        y_of(0)
    } else {
        lane.bottom() - 4.0
    };
    if target.bipolar() {
        lp.line_segment(
            [pos2(lane.left(), baseline), pos2(lane.right(), baseline)],
            Stroke::new(1.0, theme::shade(t.ruler_bg, 1.4)),
        );
    }
    let mut changed = Vec::new();
    for (i, note) in view.notes.iter().enumerate() {
        let x = geo.x_of(state, note.start);
        let value = match live {
            Some(value) if affected(i, x) => {
                changed.push(i);
                value
            }
            _ => target.get(note),
        };
        let top = y_of(value);
        // "Note properties - are displayed as vertical lines with a small circle at the top. The
        // right-facing tail allows you to see the independent values of notes with the same
        // start time."
        lp.line_segment(
            [pos2(x, baseline), pos2(x, top)],
            Stroke::new(2.0, t.step_on),
        );
        lp.circle_filled(pos2(x, top), 3.0, t.step_on);
        let tail = (geo.x_of(state, note.start + note.length.max(1)) - x).clamp(0.0, 40.0);
        lp.line_segment(
            [pos2(x, top), pos2(x + tail, top)],
            Stroke::new(1.0, t.step_on),
        );
    }
    if let Some(p) = pointer.filter(|p| lane.contains(*p)) {
        let value = live.unwrap_or_else(|| value_at(p.y));
        *paint.hint = format!("{}: {}", target.label(), display_lane(target, value));
    }
    if (response.drag_stopped() || response.clicked())
        && let Some(Drag::Lane { value, .. }) = state.drag.take()
        && !changed.is_empty()
    {
        actions.push(PianoAction::Prop {
            indices: changed,
            field: target.field(),
            value,
        });
    }
}

fn display_lane(target: LaneTarget, value: i32) -> String {
    match target {
        LaneTarget::Pan if value == 0 => "Centered".to_owned(),
        LaneTarget::Pan => format!(
            "{}% {}",
            value.abs() / 10,
            if value < 0 { "left" } else { "right" }
        ),
        LaneTarget::FinePitch => format!("{value:+} cents"),
        _ => format!("{}%", (value as f32 / 1.27).round()),
    }
}

fn keyboard(
    paint: &mut PaintCtx<'_>,
    state: &mut PianoRollState,
    view: &PianoRollView,
    actions: &mut Vec<PianoAction>,
) {
    let events = paint.ui.input(|i| i.events.clone());
    let grid = snap_ticks(state, view);
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
            Key::C if only(m(true, false, false)) => actions.push(PianoAction::Copy(sel)),
            Key::X if only(m(true, false, false)) => {
                actions.push(PianoAction::Cut(sel));
                state.selection.clear();
            }
            Key::V if only(m(true, false, false)) => {
                let bar = view.bar_ticks.max(1);
                let first_visible = (state.scroll_ticks as i64 + bar - 1) / bar * bar;
                actions.push(PianoAction::Paste { at: first_visible });
            }
            Key::D if only(m(false, true, false)) => {
                actions.push(PianoAction::DiscardLengths { indices: sel, grid })
            }
            // "Portamento Toggle ( O )", "Slide Toggle ( S )" [Manual: pianoroll].
            Key::S if plain && !view.typing => state.slide_mode = !state.slide_mode,
            Key::F if only(m(false, true, false)) => {
                state.lane_target = state.lane_target.cycle(1);
            }
            Key::O if plain && !view.typing => state.porta_mode = !state.porta_mode,
            _ if plain && !view.typing => {
                if let Some((tool, ..)) = Tool::ALL.iter().find(|(_, _, _, k)| *k == key) {
                    state.tool = *tool;
                }
            }
            _ => {}
        }
    }
}

/// Grid size of the effective snap. The Piano roll draws step lines when a step is at least
/// 4 px wide, else beat lines: that is the "Line"/"Cell" resolution at this zoom.
fn snap_ticks(state: &PianoRollState, view: &PianoRollView) -> i64 {
    let line = if state.px_per_tick * view.step_ticks as f32 >= 4.0 {
        view.step_ticks
    } else {
        view.beat_ticks
    };
    state.snap.resolve(view.global_snap).ticks(GridTicks {
        step: view.step_ticks,
        beat: view.beat_ticks,
        bar: view.bar_ticks,
        line,
    })
}

fn paint_snap_menu(paint: &mut PaintCtx<'_>, state: &mut PianoRollState) {
    let Some(origin) = state.snap_menu else {
        return;
    };
    let options = SnapValue::local_options();
    let labels: Vec<String> = options.iter().map(|o| o.label()).collect();
    let items: Vec<crate::widgets::MenuItem<'_>> = labels
        .iter()
        .zip(&options)
        .map(|(label, option)| crate::widgets::MenuItem {
            label,
            shortcut: "",
            enabled: *option != state.snap,
        })
        .collect();
    match paint.popup_menu("piano-roll.snap-menu", origin, &items) {
        crate::widgets::MenuResult::Open => {}
        crate::widgets::MenuResult::Dismissed => state.snap_menu = None,
        crate::widgets::MenuResult::Chosen(i) => {
            state.snap_menu = None;
            if let Some(option) = options.get(i) {
                state.snap = *option;
            }
        }
    }
}
