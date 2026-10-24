use egui::{Align2, Rect, Stroke, StrokeKind, pos2, vec2};

use crate::theme;
use crate::widgets::{PaintCtx, framed, vgradient};

#[derive(Default)]
pub struct RackState {
    pub selected_channel: Option<u64>,
    pub step_scroll: usize,
}

pub struct RackView {
    pub pattern_id: u64,
    pub steps_per_beat: usize,
    pub channels: Vec<ChannelView>,
}

pub struct ChannelView {
    pub id: u64,
    pub name: String,
    pub color: Option<u32>,
    pub muted: bool,
    pub volume: u16,
    pub pan: i16,
    pub mixer_target: u16,
    pub steps: Vec<bool>,
}

pub enum RackAction {
    AddChannel,
    SetStep {
        pattern: u64,
        channel: u64,
        index: usize,
        on: bool,
    },
    SelectChannel(u64),
    SetMute {
        channel: u64,
        muted: bool,
    },
}

/// Row columns (x from the rack content's left edge), derived from capture B scaled by 1/1.49
/// and capture A scaled by 1.79; see docs/UI_VISUAL_AUDIT.md §7.
const LED_X: f32 = 12.0;
const LED_R: f32 = 4.0;
const PAN_X: f32 = 29.5;
const VOL_X: f32 = 47.0;
const KNOB_R: f32 = 6.0;
const MIX_X0: f32 = 59.0;
const MIX_X1: f32 = 75.0;
const NAME_X0: f32 = 80.0;
const NAME_X1: f32 = 181.0;
const SELECT_X0: f32 = 186.0;
const SELECT_X1: f32 = 190.0;
const STEPS_X: f32 = 201.0;
const FIRST_ROW: f32 = 8.0; // UNMEASURED: top padding above the first row

pub fn paint_channel_rack(
    paint: &mut PaintCtx<'_>,
    state: &mut RackState,
    view: &RackView,
    rect: Rect,
) -> Vec<RackAction> {
    let mut actions = Vec::new();
    let t = *paint.tokens;
    let l = *paint.layout;
    let painter = paint.painter().with_clip_rect(rect);
    painter.rect_filled(rect, 0.0, t.rack_bg);
    let row_h = l.rack_row_h;
    let visible_steps = ((rect.width() - STEPS_X - 8.0) / l.step_pitch)
        .floor()
        .max(0.0) as usize;
    let group = view.steps_per_beat.max(1);

    for (row_index, channel) in view.channels.iter().enumerate() {
        let top = rect.top() + FIRST_ROW + row_index as f32 * row_h;
        if top > rect.bottom() {
            break;
        }
        let cy = top + row_h / 2.0;
        let x = |dx: f32| rect.left() + dx;

        // Mute LED (green when the channel plays).
        let led_rect = Rect::from_center_size(
            pos2(x(LED_X), cy),
            vec2(LED_R * 2.0 + 4.0, LED_R * 2.0 + 4.0),
        );
        let mute_id = format!("channel-rack.mute.{}", channel.id);
        let response = paint.hit(
            &mute_id,
            led_rect,
            "toggle",
            true,
            if channel.muted {
                "Unmute channel"
            } else {
                "Mute channel"
            },
        );
        paint.led(led_rect.center(), LED_R + 1.0, !channel.muted);
        if response.clicked() {
            actions.push(RackAction::SetMute {
                channel: channel.id,
                muted: !channel.muted,
            });
        }

        // Pan and volume knobs show the model values (editing arrives with knob gestures).
        let pan = (f32::from(channel.pan) + 1000.0) / 2000.0;
        let vol = f32::from(channel.volume) / 1000.0;
        for (id, cx, value, hint) in [
            ("pan", PAN_X, pan, "Channel panning"),
            ("volume", VOL_X, vol, "Channel volume"),
        ] {
            let knob = Rect::from_center_size(
                pos2(x(cx), cy),
                vec2(KNOB_R * 2.0 + 2.0, KNOB_R * 2.0 + 2.0),
            );
            let _ = paint.hit(
                &format!("channel-rack.{id}.{}", channel.id),
                knob,
                "knob",
                false,
                hint,
            );
            paint.knob(knob.center(), KNOB_R, value);
        }

        // Mixer track number box.
        let mix = Rect::from_min_max(pos2(x(MIX_X0), cy - 9.0), pos2(x(MIX_X1), cy + 9.0));
        framed(&painter, mix, 2.0, t.mixbox, theme::shade(t.mixbox, 0.7));
        let target = if channel.mixer_target == 0 {
            "-".to_owned()
        } else {
            channel.mixer_target.to_string()
        };
        painter.text(
            pos2(mix.right() - 3.0, cy),
            Align2::RIGHT_CENTER,
            target,
            theme::regular(13.0),
            t.mixbox_text,
        );
        paint.probe.insert(
            format!("channel-rack.mixer.{}", channel.id),
            mix,
            "lcd",
            false,
            Some(channel.mixer_target.to_string()),
        );

        // Channel name button.
        let selected = state.selected_channel == Some(channel.id);
        let name_rect =
            Rect::from_min_max(pos2(x(NAME_X0), cy - 10.0), pos2(x(NAME_X1), cy + 10.0));
        let name_id = format!("channel-rack.channel.{}", channel.id);
        let response = paint.hit(
            &name_id,
            name_rect,
            "channel",
            true,
            &format!("Channel: {}", channel.name),
        );
        let color = theme::model_color(channel.color, t.channel_default);
        vgradient(
            &painter,
            name_rect,
            &[theme::shade(color, 1.1), color, theme::shade(color, 0.85)],
        );
        painter.rect_stroke(
            name_rect,
            2.0,
            Stroke::new(1.0, theme::shade(color, 0.6)),
            StrokeKind::Inside,
        );
        let name_painter = painter.with_clip_rect(name_rect.shrink(2.0).intersect(rect));
        name_painter.text(
            pos2(name_rect.left() + 6.0, cy),
            Align2::LEFT_CENTER,
            &channel.name,
            theme::medium(14.0),
            t.channel_text,
        );
        if response.clicked() {
            state.selected_channel = Some(channel.id);
            actions.push(RackAction::SelectChannel(channel.id));
        }

        // Selection bracket.
        let select =
            Rect::from_min_max(pos2(x(SELECT_X0), cy - 11.0), pos2(x(SELECT_X1), cy + 11.0));
        if selected {
            painter.rect_stroke(
                select.expand(1.0),
                1.0,
                Stroke::new(1.5, t.select_on),
                StrokeKind::Inside,
            );
        } else {
            painter.rect_filled(select, 1.0, t.select_off);
        }

        // Steps: groups of `steps_per_beat` alternate grey and red.
        for (step_index, on) in channel
            .steps
            .iter()
            .enumerate()
            .skip(state.step_scroll)
            .take(visible_steps)
        {
            let visible = (step_index - state.step_scroll) as f32;
            let cell = Rect::from_min_size(
                pos2(x(STEPS_X) + visible * l.step_pitch, cy - l.step_h / 2.0),
                vec2(l.step_w, l.step_h),
            );
            let alt = (step_index / group) % 2 == 1;
            let step_id = format!("channel-rack.step.{}.{}", channel.id, step_index);
            let response = paint.hit(
                &step_id,
                cell,
                "step",
                true,
                &format!("Step {}: left-click on, right-click off", step_index + 1),
            );
            paint
                .probe
                .insert(step_id.clone(), cell, "step", true, Some(on.to_string()));
            paint_step(&painter, &t, cell, *on, alt, response.hovered());
            if response.clicked() {
                actions.push(RackAction::SetStep {
                    pattern: view.pattern_id,
                    channel: channel.id,
                    index: step_index,
                    on: true,
                });
            }
            if response.secondary_clicked() {
                actions.push(RackAction::SetStep {
                    pattern: view.pattern_id,
                    channel: channel.id,
                    index: step_index,
                    on: false,
                });
            }
        }
    }

    // "+" under the last row adds a channel.
    let add_top = rect.top() + FIRST_ROW + view.channels.len() as f32 * row_h + 4.0;
    let add = Rect::from_min_size(
        pos2(rect.left() + NAME_X0, add_top),
        vec2(NAME_X1 - NAME_X0, 20.0),
    );
    if paint
        .tool_button(
            "channel-rack.add",
            add,
            false,
            true,
            "Add a channel",
            crate::icons::plus,
        )
        .clicked()
    {
        actions.push(RackAction::AddChannel);
    }
    actions
}

fn paint_step(
    painter: &egui::Painter,
    t: &theme::Tokens,
    cell: Rect,
    on: bool,
    alt: bool,
    hovered: bool,
) {
    painter.rect_filled(cell, 2.0, t.step_edge);
    let face = cell.shrink(1.0);
    let (top, mid, bottom) = match (on, alt) {
        (true, false) => (t.step_on_top, t.step_on, t.step_on_bottom),
        (true, true) => (
            theme::shade(t.step_alt_on, 1.08),
            t.step_alt_on,
            theme::shade(t.step_alt_on, 0.85),
        ),
        (false, false) => (t.step_off_top, t.step_off, t.step_off_bottom),
        (false, true) => (t.step_alt_top, t.step_alt, t.step_alt_bottom),
    };
    let k = if hovered { 1.1 } else { 1.0 }; // UNMEASURED: hover tint
    vgradient(
        painter,
        face,
        &[
            theme::shade(top, k),
            theme::shade(mid, k),
            theme::shade(bottom, k),
        ],
    );
    // The short notch near the top of each step [B 341,265].
    let notch = Rect::from_min_size(
        pos2(face.left() + 2.5, face.top() + 3.0),
        vec2(face.width() - 5.0, 1.0),
    );
    painter.rect_filled(
        notch,
        0.0,
        if on {
            theme::shade(mid, 0.55)
        } else {
            t.step_notch
        },
    );
}
