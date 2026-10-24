use std::path::PathBuf;

use egui::{Align2, Event, Key, Pos2, Rect, Stroke, StrokeKind, pos2, vec2};

use crate::theme;
use crate::widgets::{MenuItem, MenuResult, PaintCtx, framed, vgradient};

#[derive(Default)]
pub struct RackState {
    pub selected_channel: Option<u64>,
    /// Channels selected with the Channel Selectors [Manual: channelrack, "10. Channel Selector"];
    /// `selected_channel` (the target) is always one of them when set.
    pub selection: std::collections::BTreeSet<u64>,
    /// Right-button drag down the selectors in progress.
    select_drag: bool,
    /// Last selector click (channel, time), for double-clicks.
    last_selector_click: Option<(u64, f64)>,
    pub step_scroll: usize,
    /// Step paint gesture: `Some(true)` paints steps on, `Some(false)` off.
    pub painting: Option<bool>,
    /// Open channel context menu: channel id and where it was opened.
    pub menu: Option<(u64, Pos2)>,
    /// Inline rename: channel id and the text typed so far.
    pub rename: Option<(u64, String)>,
    /// Knob context menu: channel, parameter and where it opened.
    pub knob_menu: Option<(u64, KnobParam, Pos2)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KnobParam {
    Volume,
    Pan,
}

pub struct RackView {
    pub pattern_id: u64,
    pub steps_per_beat: usize,
    pub step_ticks: i64,
    pub channels: Vec<ChannelView>,
    /// A Browser file being dragged, if any.
    pub dragging_file: Option<PathBuf>,
    /// Global swing 0..=100 [Manual: channelrack, "3. Global Swing"].
    pub swing: u8,
    /// Explicit pattern length in steps; `None` is Auto.
    pub length_steps: Option<u16>,
    /// Length the pattern plays at, in steps (Auto resolved).
    pub effective_steps: u16,
    /// Controls with recorded event data in the selected pattern.
    pub event_targets: Vec<jet_model::AutoTarget>,
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
    pub is_sampler: bool,
    /// (start tick, length tick, key) of Piano roll notes; non-empty replaces the steps.
    pub notes: Vec<(i64, i64, u8)>,
    pub cut_itself: bool,
    /// The channel's sample could not be found: its button is red.
    pub missing: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RackMenu {
    PianoRoll,
    GraphEditor,
    Rename,
    LoadSample,
    CutItself,
    Clone,
    Delete,
    Fill(usize),
    ClearSteps,
    FreeMixer,
    MakeSampler,
    MakeSynth,
}

pub enum RackAction {
    AddChannel,
    SetStep {
        pattern: u64,
        channel: u64,
        index: usize,
        on: bool,
        gesture: bool,
    },
    SelectChannel(u64),
    /// Left-click on the Channel button: select it and open/close Channel Settings.
    OpenSettings(u64),
    SetMute {
        channel: u64,
        muted: bool,
    },
    Solo(u64),
    SetVolume {
        channel: u64,
        value: u16,
    },
    SetPan {
        channel: u64,
        value: i16,
    },
    SetMixer {
        channel: u64,
        value: u16,
    },
    EndGesture,
    Menu(u64, RackMenu),
    Rename(u64, String),
    DropSample {
        channel: Option<u64>,
        path: PathBuf,
    },
    SetSwing(u8),
    /// `None` returns the pattern to Auto length.
    SetPatternLength(Option<u16>),
    /// "Create automation clip" from a knob's right-click menu [Manual: playlist_automationclip].
    CreateAutomation(u64, KnobParam),
    /// "Turn into Automation Clip": the knob's recorded event data in the selected pattern.
    EventsToClip(u64, KnobParam),
}

/// Swing knob and pattern length display in the Channel rack title bar
/// [Manual: channelrack, "3. Global Swing", "4. Pattern Length"]. Positions are UNMEASURED.
pub fn paint_rack_header(
    paint: &mut PaintCtx<'_>,
    title: Rect,
    view: &RackView,
) -> Vec<RackAction> {
    let mut actions = Vec::new();
    let t = *paint.tokens;
    let painter = paint.painter();
    // Swing knob.
    let center = pos2(title.right() - 190.0, title.center().y);
    let knob = Rect::from_center_size(center, vec2(18.0, 18.0));
    let value = f32::from(view.swing) / 100.0;
    let response = paint.hit(
        "channel-rack.swing",
        knob,
        "knob",
        true,
        &format!("Swing: {}%", view.swing),
    );
    paint.knob(center, 7.0, value);
    painter.text(
        pos2(knob.left() - 4.0, title.center().y),
        Align2::RIGHT_CENTER,
        "SWING",
        theme::regular(11.0),
        t.title_text,
    );
    if response.dragged() {
        let next =
            ((value - response.drag_delta().y / KNOB_TRAVEL).clamp(0.0, 1.0) * 100.0).round() as u8;
        if next != view.swing {
            actions.push(RackAction::SetSwing(next));
        }
    }
    if response.drag_stopped() {
        actions.push(RackAction::EndGesture);
    }
    // Pattern length: drag vertically for steps, right-click for Auto.
    let lcd = Rect::from_min_max(
        pos2(title.right() - 160.0, title.top() + 2.0),
        pos2(title.right() - 78.0, title.bottom() - 2.0),
    );
    framed(&painter, lcd, 2.0, t.lcd_bg, t.lcd_edge);
    let label = match view.length_steps {
        Some(steps) => format!("{steps} steps"),
        None => format!("Auto ({})", view.effective_steps),
    };
    painter.text(
        lcd.center(),
        Align2::CENTER_CENTER,
        label,
        theme::regular(12.0),
        t.lcd_text,
    );
    let response = paint.hit(
        "channel-rack.pattern-length",
        lcd,
        "lcd",
        true,
        "Pattern length: drag to set steps (1-512), right-click for Auto",
    );
    if response.dragged()
        && let Some((origin, now)) = paint
            .ui
            .input(|i| i.pointer.press_origin().zip(i.pointer.hover_pos()))
    {
        let base = i32::from(view.length_steps.unwrap_or(view.effective_steps));
        let id = egui::Id::new("channel-rack.length-origin");
        let start = paint.ui.ctx().data_mut(|d| *d.get_temp_mut_or(id, base));
        let next = (start + ((origin.y - now.y) / 4.0).round() as i32).clamp(1, 512) as u16;
        if Some(next) != view.length_steps {
            actions.push(RackAction::SetPatternLength(Some(next)));
        }
    }
    if response.drag_stopped() {
        paint
            .ui
            .ctx()
            .data_mut(|d| d.remove::<i32>(egui::Id::new("channel-rack.length-origin")));
        actions.push(RackAction::EndGesture);
    }
    if response.secondary_clicked() {
        actions.push(RackAction::SetPatternLength(None));
    }
    actions
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
/// Pixels of vertical drag for a knob's full range (our choice; FL's drag rate is unmeasured).
const KNOB_TRAVEL: f32 = 200.0;
/// Pixels of vertical drag per mixer track step (our choice).
const MIXER_STEP_PX: f32 = 6.0;

fn menu_items(channel: &ChannelView) -> Vec<(RackMenu, &'static str, &'static str, bool)> {
    vec![
        (RackMenu::PianoRoll, "Piano roll", "", true),
        (RackMenu::GraphEditor, "Graph editor", "Ctrl+K", true),
        (RackMenu::Rename, "Rename...", "F2", true),
        (
            RackMenu::LoadSample,
            "Load sample...",
            "",
            channel.is_sampler,
        ),
        (
            RackMenu::CutItself,
            if channel.cut_itself {
                "Cut itself: on"
            } else {
                "Cut itself: off"
            },
            "",
            true,
        ),
        (RackMenu::Clone, "Clone", "Alt+C", true),
        (RackMenu::Delete, "Delete", "Alt+Del", true),
        (RackMenu::Fill(2), "Fill each 2 steps", "", true),
        (RackMenu::Fill(4), "Fill each 4 steps", "", true),
        (RackMenu::Fill(8), "Fill each 8 steps", "", true),
        (RackMenu::ClearSteps, "Clear steps", "", true),
        (
            RackMenu::FreeMixer,
            "Assign to free mixer track",
            "Ctrl+L",
            true,
        ),
        (
            RackMenu::MakeSampler,
            "Replace with Sampler",
            "",
            !channel.is_sampler,
        ),
        (
            RackMenu::MakeSynth,
            "Replace with Synth",
            "",
            channel.is_sampler,
        ),
    ]
}

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
    let (pointer, released, primary_down, secondary_down) = paint.ui.input(|i| {
        (
            i.pointer.hover_pos(),
            i.pointer.any_released(),
            i.pointer.primary_down(),
            i.pointer.secondary_down(),
        )
    });
    if !primary_down && !secondary_down && state.painting.take().is_some() {
        actions.push(RackAction::EndGesture);
    }
    let os_drops = paint.ui.input(|i| i.raw.dropped_files.clone());
    let mut dropped_on_row = false;

    for (row_index, channel) in view.channels.iter().enumerate() {
        let top = rect.top() + FIRST_ROW + row_index as f32 * row_h;
        if top > rect.bottom() {
            break;
        }
        let cy = top + row_h / 2.0;
        let x = |dx: f32| rect.left() + dx;
        let row_rect = Rect::from_min_max(pos2(rect.left(), top), pos2(rect.right(), top + row_h));

        // Sample drops: from the Browser (internal drag) or the OS onto this row
        // [Manual: chansettings_sampler, "drag and drop the sample from the Browser ... onto Channel Rack"].
        if pointer.is_some_and(|p| row_rect.contains(p)) {
            if released && let Some(path) = &view.dragging_file {
                actions.push(RackAction::DropSample {
                    channel: Some(channel.id),
                    path: path.clone(),
                });
                dropped_on_row = true;
            }
            for file in &os_drops {
                actions.push(RackAction::DropSample {
                    channel: Some(channel.id),
                    path: file.path().to_path_buf(),
                });
                dropped_on_row = true;
            }
            if view.dragging_file.is_some() {
                painter.rect_stroke(
                    row_rect,
                    0.0,
                    Stroke::new(1.0, t.select_on),
                    StrokeKind::Inside,
                );
            }
        }

        // Mute LED: click mutes; Ctrl/Alt+click or right-click solos
        // [Manual: channelrack, "Solo/Unsolo - (Ctrl+Left-Click), (Alt+Left-Click) OR (Right-Click)"].
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
            "Mute: click. Solo: Ctrl+click or right-click",
        );
        paint.led(led_rect.center(), LED_R + 1.0, !channel.muted);
        let modifiers = paint.ui.input(|i| i.modifiers);
        if response.clicked() {
            if modifiers.ctrl || modifiers.alt {
                actions.push(RackAction::Solo(channel.id));
            } else {
                actions.push(RackAction::SetMute {
                    channel: channel.id,
                    muted: !channel.muted,
                });
            }
        }
        if response.secondary_clicked() {
            actions.push(RackAction::Solo(channel.id));
        }

        // Pan and volume knobs: drag vertically [Manual: channelrack, "7 & 8. Pan & Volume"].
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
            let readout = if id == "pan" {
                match channel.pan {
                    0 => "Centered".to_owned(),
                    p if p < 0 => format!("{}% left", -p / 10),
                    p => format!("{}% right", p / 10),
                }
            } else {
                format!("{}%", channel.volume / 10)
            };
            let response = paint.hit(
                &format!("channel-rack.{id}.{}", channel.id),
                knob,
                "knob",
                true,
                &format!("{hint}: {readout}"),
            );
            paint.knob(knob.center(), KNOB_R, value);
            if response.secondary_clicked() {
                let param = if id == "pan" {
                    KnobParam::Pan
                } else {
                    KnobParam::Volume
                };
                state.knob_menu = pointer.map(|p| (channel.id, param, p));
            }
            if response.dragged() {
                let dy = -response.drag_delta().y / KNOB_TRAVEL;
                let next = (value + dy).clamp(0.0, 1.0);
                if id == "pan" {
                    let pan = (next * 2000.0 - 1000.0).round() as i16;
                    if pan != channel.pan {
                        actions.push(RackAction::SetPan {
                            channel: channel.id,
                            value: pan,
                        });
                    }
                } else {
                    let volume = (next * 1000.0).round() as u16;
                    if volume != channel.volume {
                        actions.push(RackAction::SetVolume {
                            channel: channel.id,
                            value: volume,
                        });
                    }
                }
            }
            if response.drag_stopped() {
                actions.push(RackAction::EndGesture);
            }
        }

        // Mixer track selector: drag vertically [Manual: channelrack, "14. Mixer Track Routing"].
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
        let response = paint.hit(
            &format!("channel-rack.mixer.{}", channel.id),
            mix,
            "lcd",
            true,
            &if channel.mixer_target == 0 {
                "Mixer track: Master (drag to change)".to_owned()
            } else {
                format!("Mixer track: {} (drag to change)", channel.mixer_target)
            },
        );
        if response.dragged() {
            let total = paint
                .ui
                .input(|i| i.pointer.press_origin().zip(i.pointer.hover_pos()));
            if let Some((origin, now)) = total {
                let steps = ((origin.y - now.y) / MIXER_STEP_PX).round() as i32;
                let base = state_mixer_origin(paint, channel.id, channel.mixer_target);
                let next = (i32::from(base) + steps).clamp(0, 500) as u16;
                if next != channel.mixer_target {
                    actions.push(RackAction::SetMixer {
                        channel: channel.id,
                        value: next,
                    });
                }
            }
        }
        if response.drag_stopped() {
            forget_mixer_origin(paint, channel.id);
            actions.push(RackAction::EndGesture);
        }

        // Channel name button.
        let selected =
            state.selected_channel == Some(channel.id) || state.selection.contains(&channel.id);
        let name_rect =
            Rect::from_min_max(pos2(x(NAME_X0), cy - 10.0), pos2(x(NAME_X1), cy + 10.0));
        let name_id = format!("channel-rack.channel.{}", channel.id);
        let response = paint.hit(
            &name_id,
            name_rect,
            "channel",
            true,
            &if channel.missing {
                format!(
                    "Channel: {} - sample not found (add its folder to the Browser and reopen, or load it again)",
                    channel.name
                )
            } else {
                format!("Channel: {} (right-click for options)", channel.name)
            },
        );
        // "If you open a project and some, or all, Channel buttons are red, this means the
        // samples ... nominated for those Channels can't be found" [Manual: channelrack].
        let color = if channel.missing {
            egui::Color32::from_rgb(0xC8, 0x32, 0x32)
        } else {
            theme::model_color(channel.color, t.channel_default)
        };
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
        let renaming = state.rename.as_ref().filter(|(id, _)| *id == channel.id);
        let label = renaming.map_or_else(|| channel.name.clone(), |(_, text)| format!("{text}|"));
        name_painter.text(
            pos2(name_rect.left() + 6.0, cy),
            Align2::LEFT_CENTER,
            label,
            theme::medium(14.0),
            t.channel_text,
        );
        if response.clicked() {
            state.selected_channel = Some(channel.id);
            state.selection = [channel.id].into();
            actions.push(RackAction::OpenSettings(channel.id));
        }
        if response.secondary_clicked() {
            state.selected_channel = Some(channel.id);
            actions.push(RackAction::SelectChannel(channel.id));
            state.menu = pointer.map(|p| (channel.id, p));
        }

        // Selection bracket: "Select/deselect target Channel - (Left-Click)"; "(Right-Click) ...
        // independent selection/deselection"; "Right-Click (hold) and drag down the Channel
        // Selectors to make multiple selections"; "Select all Channels - (Double-Click)".
        let select =
            Rect::from_min_max(pos2(x(SELECT_X0), cy - 11.0), pos2(x(SELECT_X1), cy + 11.0));
        let selector = paint.hit(
            &format!("channel-rack.selector.{}", channel.id),
            select,
            "selector",
            true,
            "Channel selector: click selects, right-click adds/removes, right-drag selects \
             several, double-click selects all",
        );
        // A second click on the same selector within egui's double-click delay.
        let now = paint.ui.input(|i| i.time);
        let double = selector.clicked()
            && state
                .last_selector_click
                .is_some_and(|(id, at)| id == channel.id && now - at <= 0.3);
        if selector.clicked() {
            state.last_selector_click = Some((channel.id, now));
        }
        if selector.double_clicked() || double {
            state.selection = view.channels.iter().map(|c| c.id).collect();
        } else if selector.clicked() {
            state.selected_channel = Some(channel.id);
            state.selection = [channel.id].into();
            actions.push(RackAction::SelectChannel(channel.id));
        } else if selector.secondary_clicked() && !state.selection.remove(&channel.id) {
            state.selection.insert(channel.id);
        }
        if selector.drag_started_by(egui::PointerButton::Secondary) {
            state.select_drag = true;
        }
        if state.select_drag
            && paint
                .ui
                .input(|i| i.pointer.hover_pos())
                .is_some_and(|p| p.y >= select.top() - 2.0 && p.y <= select.bottom() + 2.0)
        {
            state.selection.insert(channel.id);
        }
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

        if !channel.notes.is_empty() {
            if paint_piano_preview(
                paint,
                &painter,
                view,
                channel,
                rect,
                cy,
                visible_steps,
                state,
            ) {
                actions.push(RackAction::Menu(channel.id, RackMenu::PianoRoll));
            }
            continue;
        }

        // Steps: groups of `steps_per_beat` alternate grey and red. Left-click/drag paints
        // on, right-click/drag paints off [Manual: channelrack, "Activating & deactivating steps"].
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
            let set = |on: bool, gesture: bool| RackAction::SetStep {
                pattern: view.pattern_id,
                channel: channel.id,
                index: step_index,
                on,
                gesture,
            };
            if response.clicked() {
                actions.push(set(true, false));
            } else if response.secondary_clicked() {
                actions.push(set(false, false));
            } else if response.drag_started() {
                let on = response.dragged_by(egui::PointerButton::Primary);
                state.painting = Some(on);
                actions.push(set(on, true));
            }
            if let Some(paint_on) = state.painting
                && pointer.is_some_and(|p| cell.contains(p))
                && *on != paint_on
                && !response.drag_started()
            {
                actions.push(set(paint_on, true));
            }
        }
    }

    // Drop on the empty rack area creates a Sampler channel holding the sample.
    if !dropped_on_row && pointer.is_some_and(|p| rect.contains(p)) {
        if released && let Some(path) = &view.dragging_file {
            actions.push(RackAction::DropSample {
                channel: None,
                path: path.clone(),
            });
        }
        for file in &os_drops {
            actions.push(RackAction::DropSample {
                channel: None,
                path: file.path().to_path_buf(),
            });
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

    edit_name(paint, state, &mut actions);
    paint_menu(paint, state, view, &mut actions);
    if let Some((channel, param, origin)) = state.knob_menu {
        let target = match param {
            KnobParam::Volume => {
                jet_model::AutoTarget::ChannelVolume(jet_model::ChannelId(channel))
            }
            KnobParam::Pan => jet_model::AutoTarget::ChannelPan(jet_model::ChannelId(channel)),
        };
        let items = [
            MenuItem {
                label: "Create automation clip",
                shortcut: "",
                enabled: true,
            },
            MenuItem {
                label: "Turn recorded events into automation clip",
                shortcut: "",
                enabled: view.event_targets.contains(&target),
            },
        ];
        match paint.popup_menu("channel-rack.knob-menu", origin, &items) {
            MenuResult::Open => {}
            MenuResult::Dismissed => state.knob_menu = None,
            MenuResult::Chosen(choice) => {
                state.knob_menu = None;
                actions.push(if choice == 0 {
                    RackAction::CreateAutomation(channel, param)
                } else {
                    RackAction::EventsToClip(channel, param)
                });
            }
        }
    }
    if !paint.ui.input(|i| i.pointer.secondary_down()) {
        state.select_drag = false;
    }
    actions
}

fn mixer_origin_id(channel: u64) -> egui::Id {
    egui::Id::new(("channel-rack.mixer-origin", channel))
}

/// Mixer target when the drag began, so the value follows the total drag distance.
fn state_mixer_origin(paint: &PaintCtx<'_>, channel: u64, current: u16) -> u16 {
    let id = mixer_origin_id(channel);
    paint.ui.ctx().data_mut(|d| *d.get_temp_mut_or(id, current))
}

fn forget_mixer_origin(paint: &PaintCtx<'_>, channel: u64) {
    paint
        .ui
        .ctx()
        .data_mut(|d| d.remove::<u16>(mixer_origin_id(channel)));
}

fn edit_name(paint: &mut PaintCtx<'_>, state: &mut RackState, actions: &mut Vec<RackAction>) {
    let Some((id, mut text)) = state.rename.take() else {
        return;
    };
    let mut keep = true;
    for event in paint.ui.input(|i| i.events.clone()) {
        match event {
            Event::Text(typed) => text.push_str(&typed),
            Event::Key {
                key: Key::Backspace,
                pressed: true,
                ..
            } => {
                text.pop();
            }
            Event::Key {
                key: Key::Escape,
                pressed: true,
                ..
            } => keep = false,
            Event::Key {
                key: Key::Enter,
                pressed: true,
                ..
            } => {
                if !text.trim().is_empty() {
                    actions.push(RackAction::Rename(id, text.trim().to_owned()));
                }
                keep = false;
            }
            _ => {}
        }
    }
    if keep {
        state.rename = Some((id, text));
    }
}

fn paint_menu(
    paint: &mut PaintCtx<'_>,
    state: &mut RackState,
    view: &RackView,
    actions: &mut Vec<RackAction>,
) {
    let Some((channel_id, origin)) = state.menu else {
        return;
    };
    let Some(channel) = view.channels.iter().find(|c| c.id == channel_id) else {
        state.menu = None;
        return;
    };
    let entries = menu_items(channel);
    let items: Vec<MenuItem<'_>> = entries
        .iter()
        .map(|(_, label, shortcut, enabled)| MenuItem {
            label,
            shortcut,
            enabled: *enabled,
        })
        .collect();
    match paint.popup_menu("channel-rack.menu", origin, &items) {
        MenuResult::Open => {}
        MenuResult::Dismissed => state.menu = None,
        MenuResult::Chosen(index) => {
            state.menu = None;
            if let Some((choice, ..)) = entries.get(index) {
                if *choice == RackMenu::Rename {
                    state.rename = Some((channel.id, channel.name.clone()));
                } else {
                    actions.push(RackAction::Menu(channel.id, *choice));
                }
            }
        }
    }
}

/// Mini preview of the Piano roll score in place of the steps; "You can't edit notes in the
/// mini-preview" [Manual: channelrack, "11. Piano roll Preview"]. Clicking it opens the Piano roll.
#[allow(clippy::too_many_arguments)]
fn paint_piano_preview(
    paint: &mut PaintCtx<'_>,
    painter: &egui::Painter,
    view: &RackView,
    channel: &ChannelView,
    rect: Rect,
    cy: f32,
    visible_steps: usize,
    state: &RackState,
) -> bool {
    let t = *paint.tokens;
    let l = *paint.layout;
    let left = rect.left() + STEPS_X;
    let area = Rect::from_min_size(
        pos2(left, cy - l.step_h / 2.0),
        vec2(
            visible_steps as f32 * l.step_pitch - (l.step_pitch - l.step_w),
            l.step_h,
        ),
    );
    painter.rect_filled(area, 2.0, t.step_off_bottom);
    let ticks_per_px = view.step_ticks.max(1) as f32 / l.step_pitch;
    let offset = state.step_scroll as f32 * l.step_pitch;
    let (low, high) = channel
        .notes
        .iter()
        .fold((u8::MAX, 0u8), |(lo, hi), n| (lo.min(n.2), hi.max(n.2)));
    let span = f32::from(high.saturating_sub(low).max(1));
    for &(start, length, key) in &channel.notes {
        let x0 = area.left() + start as f32 / ticks_per_px - offset;
        let w = (length.max(view.step_ticks / 4) as f32 / ticks_per_px).max(2.0);
        let y = area.bottom() - 3.0 - (f32::from(key - low) / span) * (area.height() - 6.0);
        let note = Rect::from_min_size(pos2(x0, y - 1.0), vec2(w, 2.0)).intersect(area);
        if note.width() > 0.0 {
            painter.rect_filled(note, 0.0, t.step_on);
        }
    }
    let response = paint.hit(
        &format!("channel-rack.preview.{}", channel.id),
        area,
        "preview",
        true,
        "Piano roll preview: click to open the Piano roll",
    );
    response.clicked()
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
