//! Graph editor [Manual: channelrack, "6. Graph Editor"]: per-step Note, Velocity, Release,
//! Fine pitch, Pan, Mod X, Mod Y and Shift of the selected channel. "Edit bars - Left-click a
//! column and drag up/down"; "Hold Ctrl and adjust one column, the others will follow";
//! "Right-Click a value and drag to another, the values in between ... will be interpolated";
//! "Reset values - Alt/Opt+Click (or Click and drag across) bars". Geometry is ours (UNMEASURED).

use egui::{Align2, PointerButton, Pos2, Rect, Stroke, pos2, vec2};
use jet_model::{DEFAULT_VELOCITY, MAX_KEY, StepProps};

use crate::theme;
use crate::widgets::PaintCtx;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GraphProp {
    Note,
    #[default]
    Velocity,
    Release,
    FinePitch,
    Pan,
    ModX,
    ModY,
    Shift,
}

impl GraphProp {
    pub const ALL: [Self; 8] = [
        Self::Note,
        Self::Velocity,
        Self::Release,
        Self::FinePitch,
        Self::Pan,
        Self::ModX,
        Self::ModY,
        Self::Shift,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Note => "Note",
            Self::Velocity => "Velocity",
            Self::Release => "Release",
            Self::FinePitch => "Fine pitch",
            Self::Pan => "Pan",
            Self::ModX => "Mod X",
            Self::ModY => "Mod Y",
            Self::Shift => "Shift",
        }
    }

    /// Field name in `step.props.set`.
    pub fn field(self) -> &'static str {
        match self {
            Self::Note => "note",
            Self::Velocity => "velocity",
            Self::Release => "release",
            Self::FinePitch => "pitch",
            Self::Pan => "pan",
            Self::ModX => "mod_x",
            Self::ModY => "mod_y",
            Self::Shift => "shift",
        }
    }

    /// Value range. Note spans two octaves each side of the channel root (hypothesis H-CR-2:
    /// the manual does not give the Note tab's range); Shift stays below one step.
    pub fn range(self, view: &GraphView) -> (i32, i32) {
        match self {
            Self::Note => {
                let root = i32::from(view.root_key);
                ((root - 24).max(0), (root + 24).min(i32::from(MAX_KEY)))
            }
            Self::Velocity | Self::Release | Self::ModX | Self::ModY => (0, 127),
            // "Fine pitch - +/- 100 cents".
            Self::FinePitch => (-100, 100),
            Self::Pan => (-1000, 1000),
            Self::Shift => (0, (view.step_ticks - 1).max(0) as i32),
        }
    }

    pub fn default_value(self, view: &GraphView) -> i32 {
        let base = StepProps::default();
        match self {
            Self::Note => i32::from(view.root_key),
            Self::Velocity => i32::from(DEFAULT_VELOCITY),
            _ => self.get(&base),
        }
    }

    pub fn get(self, props: &StepProps) -> i32 {
        match self {
            Self::Note => i32::from(props.note),
            Self::Velocity => i32::from(props.velocity),
            Self::Release => i32::from(props.release),
            Self::FinePitch => i32::from(props.pitch),
            Self::Pan => i32::from(props.pan),
            Self::ModX => i32::from(props.mod_x),
            Self::ModY => i32::from(props.mod_y),
            Self::Shift => i32::from(props.shift),
        }
    }

    /// Bars of bipolar values grow from the center line.
    fn bipolar(self) -> bool {
        matches!(self, Self::FinePitch | Self::Pan)
    }

    fn display(self, value: i32, view: &GraphView) -> String {
        match self {
            Self::Note => crate::channel_settings::note_name(value.clamp(0, 131) as u8),
            Self::FinePitch => format!("{value:+} cents"),
            Self::Pan if value == 0 => "Centered".to_owned(),
            Self::Pan => format!(
                "{}% {}",
                value.abs() / 10,
                if value < 0 { "left" } else { "right" }
            ),
            Self::Shift => format!(
                "{}%",
                (value as f32 / view.step_ticks.max(1) as f32 * 100.0).round()
            ),
            Self::Velocity | Self::Release | Self::ModX | Self::ModY => {
                format!("{}%", (value as f32 / 1.27).round())
            }
        }
    }
}

pub struct GraphView {
    /// (id, name) of the edited channel, the selected one.
    pub channel: Option<(u64, String)>,
    pub pattern: u64,
    /// (on, properties) per step, as many as the Channel rack shows.
    pub steps: Vec<(bool, StepProps)>,
    pub step_ticks: i64,
    pub root_key: u8,
}

#[derive(Default)]
pub struct GraphState {
    pub prop: GraphProp,
    /// Column painted last in this drag, with its value (fast drags fill the columns between).
    last: Option<(usize, i32)>,
    /// Ctrl drag: the column held and every column's value when it started. Every column moves
    /// by the held column's change (hypothesis H-CR-3: "the others will follow" is not quantified).
    offset: Option<(usize, Vec<i32>)>,
    /// Right-button ramp: the first column and value.
    ramp: Option<(usize, i32)>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum GraphAction {
    /// Set `field` of each (step index, value) of the channel in the pattern.
    Set {
        pattern: u64,
        channel: u64,
        field: &'static str,
        values: Vec<(usize, i32)>,
    },
    EndGesture,
}

const TAB_H: f32 = 26.0;

pub fn paint_graph_editor(
    paint: &mut PaintCtx<'_>,
    state: &mut GraphState,
    view: &GraphView,
    rect: Rect,
) -> Vec<GraphAction> {
    let mut actions = Vec::new();
    let t = *paint.tokens;
    let painter = paint.painter().with_clip_rect(rect);
    painter.rect_filled(rect, 0.0, t.rack_bg);
    let Some((channel, name)) = view.channel.clone() else {
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            "Select a channel",
            theme::regular(15.0),
            t.title_text,
        );
        return actions;
    };

    // Property tabs.
    let mut x = rect.left() + 6.0;
    for prop in GraphProp::ALL {
        let w = 18.0 + 7.0 * prop.label().len() as f32;
        let tab = Rect::from_min_size(pos2(x, rect.top() + 4.0), vec2(w, TAB_H - 6.0));
        if paint.toggle(
            &format!("graph-editor.tab.{}", prop.field()),
            tab,
            prop.label(),
            state.prop == prop,
            &format!("Edit the {} of each step of {name}", prop.label()),
        ) {
            state.prop = prop;
        }
        x += w + 4.0;
    }

    let prop = state.prop;
    let (min, max) = prop.range(view);
    let span = (max - min).max(1) as f32;
    let area = Rect::from_min_max(
        pos2(rect.left() + 8.0, rect.top() + TAB_H + 6.0),
        pos2(rect.right() - 8.0, rect.bottom() - 8.0),
    );
    if area.width() < 8.0 || area.height() < 8.0 || view.steps.is_empty() {
        return actions;
    }
    painter.rect_filled(area, 2.0, theme::shade(t.rack_bg, 0.55));
    let count = view.steps.len();
    let column_w = area.width() / count as f32;
    let y_of =
        |value: i32| area.bottom() - ((value - min) as f32 / span).clamp(0.0, 1.0) * area.height();
    let baseline = if prop.bipolar() {
        y_of(0)
    } else {
        area.bottom()
    };
    if prop.bipolar() {
        painter.line_segment(
            [pos2(area.left(), baseline), pos2(area.right(), baseline)],
            Stroke::new(1.0, theme::shade(t.rack_bg, 0.9)),
        );
    }
    let values: Vec<i32> = view.steps.iter().map(|(_, p)| prop.get(p)).collect();
    for (i, ((on, _), value)) in view.steps.iter().zip(&values).enumerate() {
        let left = area.left() + i as f32 * column_w;
        let y = y_of(*value);
        let bar = Rect::from_min_max(
            pos2(left + 1.0, y.min(baseline)),
            pos2(
                left + column_w - 1.0,
                y.max(baseline).max(y.min(baseline) + 2.0),
            ),
        );
        // Steps that are off show dim, like FL's muted step data.
        painter.rect_filled(
            bar,
            1.0,
            if *on {
                t.step_on
            } else {
                theme::shade(t.step_on, 0.45)
            },
        );
        if i % 4 == 0 && i > 0 {
            painter.line_segment(
                [pos2(left, area.top()), pos2(left, area.bottom())],
                Stroke::new(1.0, theme::shade(t.rack_bg, 0.75)),
            );
        }
    }

    let response = paint.hit(
        "graph-editor.bars",
        area,
        "graph",
        true,
        "Drag: set bars - Ctrl+drag: move all - Right-drag: ramp - Alt+click: reset",
    );
    let column = |p: Pos2| {
        (((p.x - area.left()) / column_w).floor() as i64).clamp(0, count as i64 - 1) as usize
    };
    let value_at = |p: Pos2| {
        (min as f32 + ((area.bottom() - p.y) / area.height()).clamp(0.0, 1.0) * span).round() as i32
    };
    let (pointer, alt, ctrl) = paint.ui.input(|i| {
        (
            i.pointer.interact_pos(),
            i.modifiers.alt,
            i.modifiers.command || i.modifiers.ctrl,
        )
    });
    if let Some(p) = pointer
        && response.hovered()
    {
        let i = column(p);
        *paint.hint = format!(
            "Step {} {}: {}",
            i + 1,
            prop.label(),
            prop.display(values[i], view)
        );
    }
    let set = |values: Vec<(usize, i32)>| GraphAction::Set {
        pattern: view.pattern,
        channel,
        field: prop.field(),
        values,
    };

    // Left button: set, offset (Ctrl) or reset (Alt).
    let primary = response.is_pointer_button_down_on()
        && paint
            .ui
            .input(|i| i.pointer.button_down(PointerButton::Primary));
    if primary && let Some(p) = pointer {
        let i = column(p);
        if state.last.is_none() && ctrl && !alt {
            state.offset = Some((i, values.clone()));
        }
        if let Some((held, base)) = &state.offset {
            let delta = value_at(p) - base[*held];
            let changed: Vec<(usize, i32)> = base
                .iter()
                .enumerate()
                .map(|(j, v)| (j, (v + delta).clamp(min, max)))
                .filter(|(j, v)| values[*j] != *v)
                .collect();
            if !changed.is_empty() {
                actions.push(set(changed));
            }
            state.last = Some((i, 0));
        } else {
            let value = if alt {
                prop.default_value(view).clamp(min, max)
            } else {
                value_at(p)
            };
            let (from, from_value) = state.last.unwrap_or((i, value));
            let changed: Vec<(usize, i32)> = interpolate(from, from_value, i, value)
                .into_iter()
                .filter(|(j, v)| values[*j] != *v)
                .collect();
            if !changed.is_empty() {
                actions.push(set(changed));
            }
            state.last = Some((i, value));
        }
    } else if state.last.is_some() {
        state.last = None;
        state.offset = None;
        actions.push(GraphAction::EndGesture);
    }

    // Right button: ramp from the press to the release, previewed as a line.
    if response.drag_started_by(PointerButton::Secondary)
        && let Some(origin) = paint.ui.input(|i| i.pointer.press_origin())
    {
        state.ramp = Some((column(origin), value_at(origin)));
    }
    if let (Some((from, from_value)), Some(p)) = (state.ramp, pointer) {
        let to = column(p);
        let to_value = value_at(p);
        let center = |i: usize| area.left() + (i as f32 + 0.5) * column_w;
        painter.line_segment(
            [
                pos2(center(from), y_of(from_value)),
                pos2(center(to), y_of(to_value)),
            ],
            Stroke::new(2.0, t.playhead),
        );
        if response.drag_stopped_by(PointerButton::Secondary) {
            state.ramp = None;
            let changed: Vec<(usize, i32)> = interpolate(from, from_value, to, to_value)
                .into_iter()
                .filter(|(j, v)| values[*j] != *v)
                .collect();
            if !changed.is_empty() {
                actions.push(set(changed));
            }
            actions.push(GraphAction::EndGesture);
        }
    }
    if !paint
        .ui
        .input(|i| i.pointer.button_down(PointerButton::Secondary))
        && state.ramp.is_some()
        && !response.drag_stopped_by(PointerButton::Secondary)
    {
        state.ramp = None;
    }
    actions
}

/// Values for every column from `a` to `b` (either order), linear between the two ends.
fn interpolate(a: usize, va: i32, b: usize, vb: i32) -> Vec<(usize, i32)> {
    if a == b {
        return vec![(b, vb)];
    }
    let (lo, vlo, hi, vhi) = if a < b {
        (a, va, b, vb)
    } else {
        (b, vb, a, va)
    };
    (lo..=hi)
        .map(|i| {
            let f = (i - lo) as f32 / (hi - lo) as f32;
            (i, (vlo as f32 + (vhi - vlo) as f32 * f).round() as i32)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::interpolate;

    #[test]
    fn ramps_include_both_ends_in_either_direction() {
        assert_eq!(
            interpolate(0, 0, 4, 100),
            vec![(0, 0), (1, 25), (2, 50), (3, 75), (4, 100)]
        );
        assert_eq!(interpolate(2, 10, 0, 30), vec![(0, 30), (1, 20), (2, 10)]);
        assert_eq!(interpolate(3, 7, 3, 9), vec![(3, 9)]);
    }
}
