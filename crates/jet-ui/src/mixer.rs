//! Mixer window [Manual: mixer]. Geometry is our own and UNMEASURED; every control drives a
//! real engine command and the meters show the audio thread's peaks.

use egui::{Align2, Pos2, Rect, Stroke, StrokeKind, pos2, vec2};
use jet_model::{FxKind, MAX_FX_SLOTS};

use crate::theme;
use crate::widgets::{MenuItem, MenuResult, PaintCtx, framed, vgradient};

const STRIP_W: f32 = 62.0;
const INSPECTOR_W: f32 = 300.0;
const FADER_TRAVEL: f32 = 150.0;
const KNOB_TRAVEL: f32 = 200.0;

#[derive(Default)]
pub struct MixerState {
    pub selected: u16,
    pub scroll: f32,
    /// Displayed meter levels with decay, per mixer index.
    pub levels: Vec<(f32, f32)>,
    menu: Option<(MixerMenu, Pos2)>,
    pub selected_slot: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MixerMenu {
    AddFx(usize),
    Slot(usize),
    /// Right-click on a fader (`true`) or pan knob (`false`) of a track.
    Automate(u16, bool),
}

pub struct MixerTrackView {
    pub index: u16,
    pub name: String,
    pub volume: u16,
    pub pan: i16,
    pub mute: bool,
    pub to_master: bool,
    pub sends: Vec<(u16, u16)>,
    pub fx: Vec<(FxKind, bool, u16)>,
    pub fx_enabled: bool,
    /// Peak since the last frame (from the audio thread), linear.
    pub peak: (f32, f32),
}

pub struct MixerView {
    pub tracks: Vec<MixerTrackView>,
}

pub enum MixerAction {
    Volume {
        track: u16,
        value: u16,
    },
    Pan {
        track: u16,
        value: i16,
    },
    Mute {
        track: u16,
        value: bool,
    },
    Solo(u16),
    ToggleSend {
        from: u16,
        to: u16,
        on: bool,
    },
    ToMaster {
        track: u16,
        value: bool,
    },
    FxAdd {
        track: u16,
        kind: &'static str,
    },
    FxRemove {
        track: u16,
        slot: usize,
    },
    FxMove {
        track: u16,
        slot: usize,
        delta: i32,
    },
    FxEnabled {
        track: u16,
        slot: usize,
        value: bool,
    },
    FxMix {
        track: u16,
        slot: usize,
        value: u16,
    },
    FxSet {
        track: u16,
        slot: usize,
        kind: FxKind,
    },
    FxMaster {
        track: u16,
        value: bool,
    },
    EndGesture,
    /// "Create automation clip" for a fader (`true`) or pan (`false`).
    CreateAutomation {
        track: u16,
        fader: bool,
    },
}

fn db_label(gain: f64) -> String {
    let db = jet_dsp::gain_to_db(gain);
    if db.is_finite() {
        format!("{db:+.1} dB")
    } else {
        "-inf dB".to_owned()
    }
}

pub fn paint_mixer(
    paint: &mut PaintCtx<'_>,
    state: &mut MixerState,
    view: &MixerView,
    rect: Rect,
) -> Vec<MixerAction> {
    let mut actions = Vec::new();
    let t = *paint.tokens;
    let painter = paint.painter().with_clip_rect(rect);
    painter.rect_filled(rect, 0.0, t.rack_bg);
    let strips = Rect::from_min_max(rect.min, pos2(rect.right() - INSPECTOR_W, rect.bottom()));
    let inspector = Rect::from_min_max(pos2(strips.right(), rect.top()), rect.max);

    if state.levels.len() < view.tracks.len() {
        state.levels.resize(view.tracks.len(), (0.0, 0.0));
    }
    // Peak hold with a decay of about 20 dB per second at 30 frames per second (our choice).
    for (level, track) in state.levels.iter_mut().zip(&view.tracks) {
        level.0 = track.peak.0.max(level.0 * 0.85);
        level.1 = track.peak.1.max(level.1 * 0.85);
    }

    if paint.ui.rect_contains_pointer(strips) {
        let delta = paint.ui.input(|i| i.smooth_scroll_delta);
        state.scroll = (state.scroll - delta.y - delta.x).max(0.0);
    }
    let max_scroll = (view.tracks.len() as f32 * STRIP_W - strips.width()).max(0.0);
    state.scroll = state.scroll.min(max_scroll);
    let strip_painter = painter.with_clip_rect(strips);
    let selected = state.selected;
    let sends_from_selected: Vec<u16> = view
        .tracks
        .iter()
        .find(|tr| tr.index == selected)
        .map(|tr| tr.sends.iter().map(|send| send.0).collect())
        .unwrap_or_default();
    for (i, track) in view.tracks.iter().enumerate() {
        let x = strips.left() + i as f32 * STRIP_W - state.scroll;
        if x + STRIP_W < strips.left() || x > strips.right() {
            continue;
        }
        let strip =
            Rect::from_min_size(pos2(x, strips.top()), vec2(STRIP_W - 2.0, strips.height()));
        let level = state.levels.get(i).copied().unwrap_or_default();
        paint_strip(
            paint,
            &strip_painter,
            track,
            strip,
            (selected, &sends_from_selected),
            level,
            &mut actions,
            state,
        );
    }
    paint_inspector(paint, &painter, state, view, inspector, &mut actions);
    paint_menus(paint, state, view, &mut actions);
    actions
}

#[allow(clippy::too_many_arguments)]
fn paint_strip(
    paint: &mut PaintCtx<'_>,
    painter: &egui::Painter,
    track: &MixerTrackView,
    strip: Rect,
    (selected, sends_from_selected): (u16, &[u16]),
    level: (f32, f32),
    actions: &mut Vec<MixerAction>,
    state: &mut MixerState,
) {
    let t = *paint.tokens;
    let is_selected = track.index == selected;
    let face = if is_selected {
        theme::shade(t.rack_bg, 1.25)
    } else {
        theme::shade(t.rack_bg, 1.08)
    };
    framed(painter, strip, 2.0, face, theme::shade(t.rack_bg, 0.7));
    let id = format!("mixer.track.{}", track.index);

    // Name / select.
    let label = Rect::from_min_size(strip.min + vec2(2.0, 2.0), vec2(strip.width() - 4.0, 34.0));
    let response = paint.hit(
        &format!("{id}.label"),
        label,
        "track",
        true,
        &format!("{} (click to select)", track.name),
    );
    painter.text(
        pos2(label.center().x, label.top() + 9.0),
        Align2::CENTER_CENTER,
        if track.index == 0 {
            "M".to_owned()
        } else {
            track.index.to_string()
        },
        theme::medium(13.0),
        t.menu_text,
    );
    painter.with_clip_rect(label).text(
        pos2(label.center().x, label.top() + 25.0),
        Align2::CENTER_CENTER,
        &track.name,
        theme::regular(12.0),
        t.title_text,
    );
    if response.clicked() {
        state.selected = track.index;
        state.selected_slot = None;
    }

    // Pan knob.
    let knob_c = pos2(strip.center().x, strip.top() + 50.0);
    let knob_r = Rect::from_center_size(knob_c, vec2(18.0, 18.0));
    let value = (f32::from(track.pan) + 1000.0) / 2000.0;
    let response = paint.hit(
        &format!("{id}.pan"),
        knob_r,
        "knob",
        true,
        &format!("Pan {}", track.pan / 10),
    );
    paint.knob(knob_c, 7.5, value);
    if response.secondary_clicked() {
        state.menu = paint
            .ui
            .input(|i| i.pointer.hover_pos())
            .map(|p| (MixerMenu::Automate(track.index, false), p));
    }
    if response.dragged() {
        let next = (value - response.drag_delta().y / KNOB_TRAVEL).clamp(0.0, 1.0);
        let pan = (next * 2000.0 - 1000.0).round() as i16;
        if pan != track.pan {
            actions.push(MixerAction::Pan {
                track: track.index,
                value: pan,
            });
        }
    }
    if response.drag_stopped() {
        actions.push(MixerAction::EndGesture);
    }

    // Meters and fader.
    let body_top = strip.top() + 66.0;
    let body_bottom = strip.bottom() - 54.0;
    let meter = Rect::from_min_max(
        pos2(strip.left() + 6.0, body_top),
        pos2(strip.left() + 18.0, body_bottom),
    );
    framed(
        painter,
        meter,
        0.0,
        theme::shade(t.rack_bg, 0.45),
        theme::shade(t.rack_bg, 0.3),
    );
    for (side, value) in [(0, level.0), (1, level.1)] {
        let db = jet_dsp::gain_to_db(f64::from(value)).max(-60.0);
        let frac = ((db + 60.0) / 66.0).clamp(0.0, 1.0) as f32;
        let x0 = meter.left() + 1.0 + side as f32 * 5.0;
        let bar = Rect::from_min_max(
            pos2(x0, meter.bottom() - frac * meter.height()),
            pos2(x0 + 4.0, meter.bottom()),
        );
        // Overs above 0 dB turn orange [Manual: mixer_levelsandmixing, "Orange peaks (over 0 dB)"].
        let color = if value > 1.0 {
            egui::Color32::from_rgb(0xF2, 0x8B, 0x3D)
        } else {
            t.led_on
        };
        painter.rect_filled(bar, 0.0, color);
    }
    let rail = Rect::from_min_max(
        pos2(strip.center().x + 6.0, body_top),
        pos2(strip.center().x + 10.0, body_bottom),
    );
    painter.rect_filled(rail, 1.0, theme::shade(t.rack_bg, 0.4));
    let frac = f32::from(track.volume) / 1000.0;
    let handle_y = rail.bottom() - frac * rail.height();
    let handle = Rect::from_center_size(pos2(rail.center().x, handle_y), vec2(22.0, 10.0));
    let fader_hit = Rect::from_min_max(
        pos2(rail.left() - 12.0, body_top - 5.0),
        pos2(rail.right() + 12.0, body_bottom + 5.0),
    );
    let gain = jet_dsp::mixer_fader_gain(track.volume);
    let response = paint.hit(
        &format!("{id}.fader"),
        fader_hit,
        "fader",
        true,
        &format!("Volume {}", db_label(gain)),
    );
    vgradient(
        painter,
        handle,
        &[
            theme::shade(t.channel_default, 1.2),
            t.channel_default,
            theme::shade(t.channel_default, 0.8),
        ],
    );
    painter.rect_stroke(
        handle,
        2.0,
        Stroke::new(
            1.0,
            if is_selected {
                t.led_on
            } else {
                theme::shade(t.rack_bg, 0.5)
            },
        ),
        StrokeKind::Inside,
    );
    if response.secondary_clicked() {
        state.menu = paint
            .ui
            .input(|i| i.pointer.hover_pos())
            .map(|p| (MixerMenu::Automate(track.index, true), p));
    }
    if response.dragged() {
        let next = (frac - response.drag_delta().y / FADER_TRAVEL).clamp(0.0, 1.0);
        let volume = (next * 1000.0).round() as u16;
        if volume != track.volume {
            actions.push(MixerAction::Volume {
                track: track.index,
                value: volume,
            });
        }
        state.selected = track.index;
    }
    if response.drag_stopped() {
        actions.push(MixerAction::EndGesture);
    }

    // Mute LED: click mutes, right-click solos [Manual: mixer, "Mute Switch"].
    let led = pos2(strip.center().x, strip.bottom() - 42.0);
    let led_rect = Rect::from_center_size(led, vec2(14.0, 14.0));
    let response = paint.hit(
        &format!("{id}.mute"),
        led_rect,
        "toggle",
        true,
        "Mute: click. Solo: right-click",
    );
    paint.led(led, 5.0, !track.mute);
    if response.clicked() {
        actions.push(MixerAction::Mute {
            track: track.index,
            value: !track.mute,
        });
    }
    if response.secondary_clicked() && track.index != 0 {
        actions.push(MixerAction::Solo(track.index));
    }

    // Send switch: route the selected track to this one [Manual: mixer, "Send / Sidechain switch"].
    if track.index != 0 && track.index != selected && selected != 0 {
        let sw = Rect::from_center_size(
            pos2(strip.center().x, strip.bottom() - 18.0),
            vec2(30.0, 16.0),
        );
        let on = sends_from_selected.contains(&track.index);
        let response = paint.hit(
            &format!("{id}.send"),
            sw,
            "toggle",
            true,
            &format!("Send insert {selected} to {}", track.name),
        );
        if on {
            crate::widgets::lit_face(painter, sw, &t);
        } else {
            crate::widgets::button_face(painter, sw, &t, response.hovered(), false);
        }
        painter.text(
            sw.center(),
            Align2::CENTER_CENTER,
            "send",
            theme::regular(11.0),
            if on {
                theme::shade(t.lit_edge, 0.5)
            } else {
                t.menu_text
            },
        );
        if response.clicked() {
            actions.push(MixerAction::ToggleSend {
                from: selected,
                to: track.index,
                on: !on,
            });
        }
    }
}

/// Editable numeric parameters of an effect: (label, value, min, max).
pub fn fx_params(kind: &FxKind) -> Vec<(&'static str, i32, i32, i32)> {
    match kind {
        FxKind::Eq(bands) => bands
            .iter()
            .enumerate()
            .flat_map(|(i, b)| {
                let names = [
                    ("Low gain", "Low freq"),
                    ("Mid gain", "Mid freq"),
                    ("High gain", "High freq"),
                ];
                let (g, f) = names.get(i).copied().unwrap_or(("Gain", "Freq"));
                [
                    (g, i32::from(b.gain_db_tenths), -240, 240),
                    (f, i32::from(b.freq_hz), 20, 20_000),
                ]
            })
            .collect(),
        FxKind::Delay {
            time_beats_milli,
            feedback,
            mix,
            ..
        } => vec![
            ("Time (beats/1000)", *time_beats_milli as i32, 1, 4000),
            ("Feedback", i32::from(*feedback), 0, 950),
            ("Mix", i32::from(*mix), 0, 1000),
        ],
        FxKind::Reverb {
            room,
            damping,
            width,
            mix,
        } => vec![
            ("Room", i32::from(*room), 0, 1000),
            ("Damping", i32::from(*damping), 0, 1000),
            ("Width", i32::from(*width), 0, 1000),
            ("Mix", i32::from(*mix), 0, 1000),
        ],
        FxKind::Compressor {
            threshold_db_tenths,
            ratio_tenths,
            attack_ms,
            release_ms,
            makeup_db_tenths,
        } => vec![
            (
                "Threshold (0.1 dB)",
                i32::from(*threshold_db_tenths),
                -600,
                0,
            ),
            ("Ratio (x0.1)", i32::from(*ratio_tenths), 10, 1000),
            ("Attack ms", i32::from(*attack_ms), 0, 1000),
            ("Release ms", i32::from(*release_ms), 0, 5000),
            ("Makeup (0.1 dB)", i32::from(*makeup_db_tenths), 0, 360),
        ],
        FxKind::Limiter {
            ceiling_db_tenths,
            release_ms,
        } => vec![
            ("Ceiling (0.1 dB)", i32::from(*ceiling_db_tenths), -300, 0),
            ("Release ms", i32::from(*release_ms), 0, 5000),
        ],
        FxKind::Distortion { drive, mix } => vec![
            ("Drive", i32::from(*drive), 0, 1000),
            ("Mix", i32::from(*mix), 0, 1000),
        ],
        FxKind::Chorus {
            rate_milli_hz,
            depth,
            mix,
        } => vec![
            ("Rate (mHz)", *rate_milli_hz as i32, 10, 10_000),
            ("Depth", i32::from(*depth), 0, 1000),
            ("Mix", i32::from(*mix), 0, 1000),
        ],
        FxKind::Gate {
            threshold_db_tenths,
            release_ms,
        } => vec![
            (
                "Threshold (0.1 dB)",
                i32::from(*threshold_db_tenths),
                -900,
                0,
            ),
            ("Release ms", i32::from(*release_ms), 0, 5000),
        ],
        FxKind::Flanger {
            rate_milli_hz,
            depth,
            feedback,
            mix,
        }
        | FxKind::Phaser {
            rate_milli_hz,
            depth,
            feedback,
            mix,
        } => vec![
            ("Rate (mHz)", *rate_milli_hz as i32, 10, 10_000),
            ("Depth", i32::from(*depth), 0, 1000),
            ("Feedback", i32::from(*feedback), 0, 950),
            ("Mix", i32::from(*mix), 0, 1000),
        ],
        FxKind::Filter {
            mode,
            cutoff_hz,
            resonance,
        } => vec![
            ("Type (LP/HP/BP)", i32::from(*mode), 0, 2),
            ("Cutoff (Hz)", i32::from(*cutoff_hz), 20, 20_000),
            ("Resonance", i32::from(*resonance), 0, 1000),
        ],
    }
}

/// `kind` with parameter `index` (as listed by [`fx_params`]) set to `value`.
pub fn with_param(kind: &FxKind, index: usize, value: i32) -> FxKind {
    let mut next = kind.clone();
    let u16v = value.clamp(0, i32::from(u16::MAX)) as u16;
    let i16v = value.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16;
    match &mut next {
        FxKind::Eq(bands) => {
            if let Some(band) = bands.get_mut(index / 2) {
                if index.is_multiple_of(2) {
                    band.gain_db_tenths = i16v;
                } else {
                    band.freq_hz = u16v;
                }
            }
        }
        FxKind::Delay {
            time_beats_milli,
            feedback,
            mix,
            ..
        } => match index {
            0 => *time_beats_milli = value.max(1) as u32,
            1 => *feedback = u16v,
            _ => *mix = u16v,
        },
        FxKind::Reverb {
            room,
            damping,
            width,
            mix,
        } => match index {
            0 => *room = u16v,
            1 => *damping = u16v,
            2 => *width = u16v,
            _ => *mix = u16v,
        },
        FxKind::Compressor {
            threshold_db_tenths,
            ratio_tenths,
            attack_ms,
            release_ms,
            makeup_db_tenths,
        } => match index {
            0 => *threshold_db_tenths = i16v,
            1 => *ratio_tenths = u16v,
            2 => *attack_ms = u16v,
            3 => *release_ms = u16v,
            _ => *makeup_db_tenths = i16v,
        },
        FxKind::Limiter {
            ceiling_db_tenths,
            release_ms,
        } => match index {
            0 => *ceiling_db_tenths = i16v,
            _ => *release_ms = u16v,
        },
        FxKind::Distortion { drive, mix } => match index {
            0 => *drive = u16v,
            _ => *mix = u16v,
        },
        FxKind::Chorus {
            rate_milli_hz,
            depth,
            mix,
        } => match index {
            0 => *rate_milli_hz = value.max(10) as u32,
            1 => *depth = u16v,
            _ => *mix = u16v,
        },
        FxKind::Gate {
            threshold_db_tenths,
            release_ms,
        } => match index {
            0 => *threshold_db_tenths = i16v,
            _ => *release_ms = u16v,
        },
        FxKind::Flanger {
            rate_milli_hz,
            depth,
            feedback,
            mix,
        }
        | FxKind::Phaser {
            rate_milli_hz,
            depth,
            feedback,
            mix,
        } => match index {
            0 => *rate_milli_hz = value.max(10) as u32,
            1 => *depth = u16v,
            2 => *feedback = u16v.min(950),
            _ => *mix = u16v,
        },
        FxKind::Filter {
            mode,
            cutoff_hz,
            resonance,
        } => match index {
            0 => *mode = value.clamp(0, 2) as u8,
            1 => *cutoff_hz = u16v.clamp(20, 20_000),
            _ => *resonance = u16v,
        },
    }
    next
}

fn fx_label(kind: &FxKind) -> &'static str {
    match kind {
        FxKind::Eq(_) => "Parametric EQ",
        FxKind::Delay { .. } => "Delay",
        FxKind::Reverb { .. } => "Reverb",
        FxKind::Compressor { .. } => "Compressor",
        FxKind::Limiter { .. } => "Limiter",
        FxKind::Distortion { .. } => "Distortion",
        FxKind::Chorus { .. } => "Chorus",
        FxKind::Gate { .. } => "Noise gate",
        FxKind::Flanger { .. } => "Flanger",
        FxKind::Phaser { .. } => "Phaser",
        FxKind::Filter { .. } => "Filter",
    }
}

fn paint_inspector(
    paint: &mut PaintCtx<'_>,
    painter: &egui::Painter,
    state: &mut MixerState,
    view: &MixerView,
    rect: Rect,
    actions: &mut Vec<MixerAction>,
) {
    let t = *paint.tokens;
    painter.rect_filled(rect, 0.0, theme::shade(t.rack_bg, 0.8));
    let Some(track) = view.tracks.iter().find(|tr| tr.index == state.selected) else {
        return;
    };
    let mut y = rect.top() + 8.0;
    painter.text(
        pos2(rect.left() + 10.0, y + 8.0),
        Align2::LEFT_CENTER,
        format!("{} - effects", track.name),
        theme::medium(15.0),
        t.menu_text,
    );
    y += 24.0;
    // FX master switch and Master routing.
    let master_sw = Rect::from_min_size(pos2(rect.left() + 10.0, y), vec2(110.0, 20.0));
    let r = paint.hit(
        "mixer.fx-master",
        master_sw,
        "toggle",
        true,
        "Enable/disable all effects on this track",
    );
    if track.fx_enabled {
        crate::widgets::lit_face(painter, master_sw, &t)
    } else {
        crate::widgets::button_face(painter, master_sw, &t, r.hovered(), false)
    }
    painter.text(
        master_sw.center(),
        Align2::CENTER_CENTER,
        "FX on/off",
        theme::regular(13.0),
        if track.fx_enabled {
            theme::shade(t.lit_edge, 0.5)
        } else {
            t.menu_text
        },
    );
    if r.clicked() {
        actions.push(MixerAction::FxMaster {
            track: track.index,
            value: !track.fx_enabled,
        });
    }
    if track.index != 0 {
        let to_master = Rect::from_min_size(pos2(master_sw.right() + 8.0, y), vec2(150.0, 20.0));
        let r = paint.hit(
            "mixer.to-master",
            to_master,
            "toggle",
            true,
            "Send this track to the Master",
        );
        if track.to_master {
            crate::widgets::lit_face(painter, to_master, &t)
        } else {
            crate::widgets::button_face(painter, to_master, &t, r.hovered(), false)
        }
        painter.text(
            to_master.center(),
            Align2::CENTER_CENTER,
            "Route to Master",
            theme::regular(13.0),
            if track.to_master {
                theme::shade(t.lit_edge, 0.5)
            } else {
                t.menu_text
            },
        );
        if r.clicked() {
            actions.push(MixerAction::ToMaster {
                track: track.index,
                value: !track.to_master,
            });
        }
    }
    y += 30.0;
    // Ten FX slots [Manual: mixer, "Each Track has 10 effects slots"].
    for slot in 0..MAX_FX_SLOTS {
        let row = Rect::from_min_size(pos2(rect.left() + 10.0, y), vec2(rect.width() - 20.0, 22.0));
        let filled = track.fx.get(slot);
        let selected = state.selected_slot == Some(slot) && filled.is_some();
        framed(
            painter,
            row,
            2.0,
            if selected {
                theme::shade(t.rack_bg, 1.3)
            } else {
                theme::shade(t.rack_bg, 0.6)
            },
            theme::shade(t.rack_bg, 0.4),
        );
        let hint = if filled.is_some() {
            "Click to edit; right-click for options"
        } else {
            "Click to add an effect"
        };
        let r = paint.hit(&format!("mixer.slot.{slot}"), row, "fx-slot", true, hint);
        match filled {
            Some((kind, enabled, _)) => {
                painter.circle_filled(
                    pos2(row.left() + 10.0, row.center().y),
                    4.0,
                    if *enabled { t.led_on } else { t.led_off },
                );
                painter.text(
                    pos2(row.left() + 22.0, row.center().y),
                    Align2::LEFT_CENTER,
                    fx_label(kind),
                    theme::regular(14.0),
                    t.menu_text,
                );
                if r.clicked() {
                    state.selected_slot = Some(slot);
                }
                if r.secondary_clicked() {
                    state.menu = paint
                        .ui
                        .input(|i| i.pointer.hover_pos())
                        .map(|p| (MixerMenu::Slot(slot), p));
                }
            }
            None => {
                painter.text(
                    pos2(row.left() + 22.0, row.center().y),
                    Align2::LEFT_CENTER,
                    format!("Slot {}", slot + 1),
                    theme::regular(13.0),
                    t.hint_text,
                );
                if r.clicked() && slot == track.fx.len() {
                    state.menu = paint
                        .ui
                        .input(|i| i.pointer.hover_pos())
                        .map(|p| (MixerMenu::AddFx(slot), p));
                }
            }
        }
        y += 25.0;
    }
    // Parameters of the selected effect.
    let Some(slot) = state.selected_slot else {
        return;
    };
    let Some((kind, _, mix)) = track.fx.get(slot) else {
        return;
    };
    y += 6.0;
    painter.text(
        pos2(rect.left() + 10.0, y + 8.0),
        Align2::LEFT_CENTER,
        format!("{} parameters", fx_label(kind)),
        theme::medium(14.0),
        t.menu_text,
    );
    y += 22.0;
    let mut rows: Vec<(&'static str, i32, i32, i32)> = vec![("Slot mix", i32::from(*mix), 0, 1000)];
    rows.extend(fx_params(kind));
    for (index, (label, value, min, max)) in rows.into_iter().enumerate() {
        let row = Rect::from_min_size(pos2(rect.left() + 10.0, y), vec2(rect.width() - 20.0, 20.0));
        let bar = Rect::from_min_max(
            pos2(row.left() + 130.0, row.top() + 4.0),
            pos2(row.right(), row.bottom() - 4.0),
        );
        painter.text(
            pos2(row.left(), row.center().y),
            Align2::LEFT_CENTER,
            label,
            theme::regular(13.0),
            t.title_text,
        );
        framed(
            painter,
            bar,
            2.0,
            theme::shade(t.rack_bg, 0.5),
            theme::shade(t.rack_bg, 0.35),
        );
        let frac = (value - min) as f32 / (max - min).max(1) as f32;
        painter.rect_filled(
            Rect::from_min_max(bar.min, pos2(bar.left() + frac * bar.width(), bar.bottom())),
            2.0,
            t.step_on,
        );
        painter.text(
            bar.center(),
            Align2::CENTER_CENTER,
            value.to_string(),
            theme::regular(12.0),
            t.menu_text,
        );
        let r = paint.hit(
            &format!("mixer.param.{index}"),
            bar,
            "slider",
            true,
            "Drag horizontally to change",
        );
        if r.dragged()
            && let Some(p) = paint.ui.input(|i| i.pointer.hover_pos())
        {
            let f = ((p.x - bar.left()) / bar.width()).clamp(0.0, 1.0);
            let next = min + ((max - min) as f32 * f).round() as i32;
            if next != value {
                if index == 0 {
                    actions.push(MixerAction::FxMix {
                        track: track.index,
                        slot,
                        value: next as u16,
                    });
                } else {
                    actions.push(MixerAction::FxSet {
                        track: track.index,
                        slot,
                        kind: with_param(kind, index - 1, next),
                    });
                }
            }
        }
        if r.drag_stopped() {
            actions.push(MixerAction::EndGesture);
        }
        y += 22.0;
    }
}

fn paint_menus(
    paint: &mut PaintCtx<'_>,
    state: &mut MixerState,
    view: &MixerView,
    actions: &mut Vec<MixerAction>,
) {
    let Some((menu, origin)) = state.menu else {
        return;
    };
    let track = state.selected;
    match menu {
        MixerMenu::AddFx(_) => {
            let names: Vec<(&'static str, &'static str)> = FxKind::NAMES
                .iter()
                .filter_map(|n| FxKind::default_for(n).map(|k| (*n, fx_label(&k))))
                .collect();
            let items: Vec<MenuItem<'_>> = names
                .iter()
                .map(|(_, label)| MenuItem {
                    label,
                    shortcut: "",
                    enabled: true,
                })
                .collect();
            match paint.popup_menu("mixer.add-fx", origin, &items) {
                MenuResult::Open => {}
                MenuResult::Dismissed => state.menu = None,
                MenuResult::Chosen(i) => {
                    state.menu = None;
                    if let Some((name, _)) = names.get(i) {
                        actions.push(MixerAction::FxAdd { track, kind: name });
                    }
                }
            }
        }
        MixerMenu::Automate(index, fader) => {
            let items = [MenuItem {
                label: "Create automation clip",
                shortcut: "",
                enabled: true,
            }];
            match paint.popup_menu("mixer.automate-menu", origin, &items) {
                MenuResult::Open => {}
                MenuResult::Dismissed => state.menu = None,
                MenuResult::Chosen(_) => {
                    state.menu = None;
                    actions.push(MixerAction::CreateAutomation {
                        track: index,
                        fader,
                    });
                }
            }
        }
        MixerMenu::Slot(slot) => {
            let enabled = view
                .tracks
                .iter()
                .find(|tr| tr.index == track)
                .and_then(|tr| tr.fx.get(slot))
                .is_some_and(|f| f.1);
            let items = [
                MenuItem {
                    label: if enabled { "Disable" } else { "Enable" },
                    shortcut: "",
                    enabled: true,
                },
                MenuItem {
                    label: "Move up",
                    shortcut: "",
                    enabled: slot > 0,
                },
                MenuItem {
                    label: "Move down",
                    shortcut: "",
                    enabled: true,
                },
                MenuItem {
                    label: "Delete",
                    shortcut: "",
                    enabled: true,
                },
            ];
            match paint.popup_menu("mixer.slot-menu", origin, &items) {
                MenuResult::Open => {}
                MenuResult::Dismissed => state.menu = None,
                MenuResult::Chosen(i) => {
                    state.menu = None;
                    actions.push(match i {
                        0 => MixerAction::FxEnabled {
                            track,
                            slot,
                            value: !enabled,
                        },
                        1 => MixerAction::FxMove {
                            track,
                            slot,
                            delta: -1,
                        },
                        2 => MixerAction::FxMove {
                            track,
                            slot,
                            delta: 1,
                        },
                        _ => {
                            state.selected_slot = None;
                            MixerAction::FxRemove { track, slot }
                        }
                    });
                }
            }
        }
    }
}
