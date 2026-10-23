use egui::{Align2, FontId, Id, Rect, Sense, Stroke, StrokeKind, pos2, vec2};

use crate::widgets::PaintCtx;

#[derive(Default)]
pub struct RackState {
    pub selected_channel: Option<u64>,
    pub step_scroll: usize,
}

pub struct RackView {
    pub pattern_id: u64,
    pub pattern_name: String,
    pub channels: Vec<ChannelView>,
}

pub struct ChannelView {
    pub id: u64,
    pub name: String,
    pub muted: bool,
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

pub fn paint_channel_rack(
    paint: &mut PaintCtx<'_>,
    state: &mut RackState,
    view: &RackView,
    rect: Rect,
) -> Vec<RackAction> {
    let mut actions = Vec::new();
    paint.panel(rect, false);
    let header = Rect::from_min_size(rect.min, vec2(rect.width(), paint.layout.window_title_h));
    paint.panel(header, true);
    paint.label(header, &format!("Pattern: {}", view.pattern_name), false);
    let add = Rect::from_min_size(
        pos2(rect.right() - 86.0, rect.top() + 3.0),
        vec2(80.0, 21.0),
    );
    if paint
        .button(
            "channel-rack.add",
            add,
            "+ Channel",
            false,
            true,
            "Adicionar canal vazio ao projeto",
        )
        .clicked()
    {
        actions.push(RackAction::AddChannel);
    }

    // UNMEASURED: channel controls and row geometry await R0 fixtures.
    let row_top = header.bottom() + 4.0;
    let fixed_w = 178.0;
    let visible_steps = ((rect.width() - fixed_w - 20.0) / paint.layout.step_w)
        .floor()
        .max(0.0) as usize;
    let row_painter = paint.ui.painter().with_clip_rect(rect);
    for (row_index, channel) in view.channels.iter().enumerate() {
        let y = row_top + row_index as f32 * paint.layout.row_h;
        if y > rect.bottom() {
            break;
        }
        let row = Rect::from_min_size(
            pos2(rect.left() + 3.0, y),
            vec2(rect.width() - 6.0, paint.layout.row_h - 1.0),
        );
        let selected = state.selected_channel == Some(channel.id);
        row_painter.rect_filled(
            row,
            0.0,
            if selected {
                paint.tokens.raised
            } else {
                paint.tokens.surface
            },
        );

        let mute_rect =
            Rect::from_min_size(pos2(row.left() + 5.0, row.top() + 5.0), vec2(14.0, 14.0));
        let mute_id = format!("channel-rack.mute.{}", channel.id);
        let mute_response = paint
            .ui
            .interact(mute_rect, Id::new(&mute_id), Sense::click());
        row_painter.circle_filled(
            mute_rect.center(),
            5.0,
            if channel.muted {
                paint.tokens.disabled
            } else {
                paint.tokens.active
            },
        );
        paint.probe.insert(
            mute_id,
            mute_rect,
            "toggle",
            true,
            Some((!channel.muted).to_string()),
        );
        if mute_response.clicked() {
            actions.push(RackAction::SetMute {
                channel: channel.id,
                muted: !channel.muted,
            });
        }
        if mute_response.hovered() {
            *paint.hint = if channel.muted {
                "Ativar canal"
            } else {
                "Silenciar canal"
            }
            .to_owned();
        }

        let name_rect = Rect::from_min_size(
            pos2(row.left() + 24.0, row.top() + 2.0),
            vec2(fixed_w - 28.0, row.height() - 4.0),
        );
        let name_id = format!("channel-rack.channel.{}", channel.id);
        let name_response = paint
            .ui
            .interact(name_rect, Id::new(&name_id), Sense::click());
        row_painter.rect_filled(
            name_rect,
            2.0,
            if selected {
                paint.tokens.active
            } else {
                paint.tokens.control
            },
        );
        row_painter.rect_stroke(
            name_rect,
            2.0,
            Stroke::new(1.0, paint.tokens.border),
            StrokeKind::Inside,
        );
        row_painter.text(
            pos2(name_rect.left() + 6.0, name_rect.center().y),
            Align2::LEFT_CENTER,
            &channel.name,
            FontId::proportional(paint.layout.font_regular),
            paint.tokens.text,
        );
        paint.probe.insert(
            name_id,
            name_rect,
            "channel",
            true,
            Some(channel.name.clone()),
        );
        if name_response.clicked() {
            state.selected_channel = Some(channel.id);
            actions.push(RackAction::SelectChannel(channel.id));
        }
        if name_response.hovered() {
            *paint.hint = format!("Canal {}", channel.name);
        }

        let step_start = row.left() + fixed_w;
        for (step_index, on) in channel
            .steps
            .iter()
            .enumerate()
            .skip(state.step_scroll)
            .take(visible_steps)
        {
            let visible_index = step_index - state.step_scroll;
            let x = step_start + visible_index as f32 * paint.layout.step_w;
            let cell = Rect::from_min_size(
                pos2(x, row.top() + 2.0),
                vec2(paint.layout.step_w - 2.0, row.height() - 4.0),
            );
            let step_id = format!("channel-rack.step.{}.{}", channel.id, step_index);
            let response = paint.ui.interact(cell, Id::new(&step_id), Sense::click());
            row_painter.rect_filled(
                cell,
                2.0,
                if *on {
                    paint.tokens.active
                } else if response.hovered() {
                    paint.tokens.raised
                } else {
                    paint.tokens.control
                },
            );
            row_painter.rect_stroke(
                cell,
                2.0,
                Stroke::new(1.0, paint.tokens.border),
                StrokeKind::Inside,
            );
            paint
                .probe
                .insert(step_id, cell, "step", true, Some(on.to_string()));
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
            if response.hovered() {
                *paint.hint = format!(
                    "Passo {}: clique esquerdo liga; direito desliga",
                    step_index + 1
                );
            }
        }
    }

    if view.channels.is_empty() {
        row_painter.text(
            pos2(rect.left() + 10.0, row_top + 18.0),
            Align2::LEFT_CENTER,
            "Nenhum canal. Use + Channel.",
            FontId::proportional(paint.layout.font_regular),
            paint.tokens.muted,
        );
    }
    actions
}
