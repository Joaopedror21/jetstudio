//! Channel Settings window [Manual: chansettings, chansettings_sampler, chansettings_misc].
//! Opened by left-clicking a Channel button. Layout is ours (UNMEASURED); every control is a
//! real engine command.

use egui::{Align2, Rect, Stroke, pos2, vec2};
use jet_model::{
    Envelope, FilterType, LfoShape, ModTarget, SampleLoop, SamplerSettings, SynthParams, Waveform,
};
use serde_json::{Value, json};

use crate::theme;
use crate::widgets::{PaintCtx, framed};

const ROW_H: f32 = 22.0;

pub struct ChannelSettingsData {
    pub id: u64,
    pub name: String,
    pub sample_name: Option<String>,
    /// Peak columns (min, max) of the loaded sample; empty without a sample.
    pub waveform: Vec<(f32, f32)>,
    pub is_sampler: bool,
    pub volume: u16,
    pub pan: i16,
    pub pitch: i16,
    pub root_key: u8,
    pub swing_mix: u16,
    pub cut: u8,
    pub cut_by: u8,
    pub mixer_target: u16,
    pub poly_max: u16,
    pub mono: bool,
    pub porta: bool,
    pub slide_ms: u16,
    pub sampler: SamplerSettings,
    pub synth: Option<SynthParams>,
}

pub enum SettingsAction {
    /// An engine command; `gesture` groups a drag into one undo step.
    Engine {
        id: &'static str,
        params: Value,
        gesture: Option<String>,
    },
    EndGesture,
    LoadSample(u64),
}

pub(crate) fn note_name(key: u8) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    format!("{}{}", NAMES[usize::from(key % 12)], key / 12)
}

/// Which Channel Settings page is shown, and the INS page's target tab.
pub struct ChannelSettingsState {
    pub ins: bool,
    pub target: ModTarget,
}

impl Default for ChannelSettingsState {
    fn default() -> Self {
        Self {
            ins: false,
            target: ModTarget::Volume,
        }
    }
}

pub fn paint_channel_settings(
    paint: &mut PaintCtx<'_>,
    state: &mut ChannelSettingsState,
    data: Option<&ChannelSettingsData>,
    rect: Rect,
) -> Vec<SettingsAction> {
    let mut actions = Vec::new();
    let t = *paint.tokens;
    let painter = paint.painter().with_clip_rect(rect);
    painter.rect_filled(rect, 0.0, t.rack_bg);
    let Some(c) = data else {
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            "Select a channel",
            theme::regular(15.0),
            t.title_text,
        );
        return actions;
    };
    // Page tabs [Manual: chansettings, the SMP (Sampler) and INS (Envelope / Instrument) tabs].
    let mut tab_x = rect.left() + 10.0;
    for (ins, label, hint) in [
        (false, "SMP", "Sampler, channel and polyphony settings"),
        (true, "INS", "Envelopes, LFOs and filter (Sampler channels)"),
    ] {
        let tab = Rect::from_min_size(pos2(tab_x, rect.top() + 6.0), vec2(60.0, 20.0));
        if paint.toggle(
            &format!("channel-settings.page.{}", label.to_lowercase()),
            tab,
            label,
            state.ins == ins,
            hint,
        ) {
            state.ins = ins;
        }
        tab_x += 64.0;
    }
    if state.ins {
        let content = Rect::from_min_max(pos2(rect.left(), rect.top() + 30.0), rect.max);
        paint_ins(paint, state, c, content, &mut actions);
        return actions;
    }
    let mut y = rect.top() + 32.0;
    let left = rect.left() + 10.0;
    let width = rect.width() - 20.0;
    let mut row = |h: f32| {
        let r = Rect::from_min_size(pos2(left, y), vec2(width, h));
        y += h + 4.0;
        r
    };
    let section = |painter: &egui::Painter, r: Rect, title: &str| {
        painter.text(
            pos2(r.left(), r.center().y),
            Align2::LEFT_CENTER,
            title,
            theme::medium(15.0),
            t.menu_text,
        );
    };

    // ---- Channel (MISC page controls that act on playback) ----
    section(&painter, row(ROW_H), &format!("{} - channel", c.name));
    let id = c.id;
    let mut edit = |paint: &mut PaintCtx<'_>,
                    r: Rect,
                    key: &str,
                    label: &str,
                    value: i32,
                    min: i32,
                    max: i32,
                    display: String,
                    command: &'static str,
                    make: &dyn Fn(i32) -> Value| {
        let e = paint.param_row(
            &format!("channel-settings.{key}"),
            r,
            label,
            value,
            min,
            max,
            &display,
        );
        if let Some(v) = e.value {
            actions.push(SettingsAction::Engine {
                id: command,
                params: make(v),
                gesture: Some(format!("settings.{key}.{id}")),
            });
        }
        if e.ended {
            actions.push(SettingsAction::EndGesture);
        }
    };
    edit(
        paint,
        row(ROW_H),
        "volume",
        "Volume",
        i32::from(c.volume),
        0,
        1000,
        format!("{}%", c.volume / 10),
        "channel.volume.set",
        &|v| json!({"id": id, "value": v}),
    );
    edit(
        paint,
        row(ROW_H),
        "pan",
        "Panning",
        i32::from(c.pan),
        -1000,
        1000,
        format!("{}", c.pan / 10),
        "channel.pan.set",
        &|v| json!({"id": id, "value": v}),
    );
    edit(
        paint,
        row(ROW_H),
        "pitch",
        "Pitch (cents)",
        i32::from(c.pitch),
        -1200,
        1200,
        format!("{:+}", c.pitch),
        "channel.pitch.set",
        &|v| json!({"id": id, "value": v}),
    );
    edit(
        paint,
        row(ROW_H),
        "root",
        "Root note",
        i32::from(c.root_key),
        0,
        131,
        note_name(c.root_key),
        "channel.root_key.set",
        &|v| json!({"id": id, "value": v}),
    );
    edit(
        paint,
        row(ROW_H),
        "swing",
        "Swing mix",
        i32::from(c.swing_mix),
        0,
        1000,
        format!("{}%", c.swing_mix / 10),
        "channel.swing_mix.set",
        &|v| json!({"id": id, "value": v}),
    );
    edit(
        paint,
        row(ROW_H),
        "mixer",
        "Mixer track",
        i32::from(c.mixer_target),
        0,
        125,
        if c.mixer_target == 0 {
            "Master".to_owned()
        } else {
            c.mixer_target.to_string()
        },
        "channel.mixer_target.set",
        &|v| json!({"id": id, "value": v}),
    );
    let (cut, cut_by) = (c.cut, c.cut_by);
    edit(
        paint,
        row(ROW_H),
        "cut",
        "Cut group",
        i32::from(cut),
        0,
        32,
        if cut == 0 {
            "--".to_owned()
        } else {
            cut.to_string()
        },
        "channel.cut.set",
        &|v| json!({"id": id, "cut": v, "cut_by": cut_by}),
    );
    edit(
        paint,
        row(ROW_H),
        "cut-by",
        "Cut by",
        i32::from(cut_by),
        0,
        32,
        if cut_by == 0 {
            "--".to_owned()
        } else {
            cut_by.to_string()
        },
        "channel.cut.set",
        &|v| json!({"id": id, "cut": cut, "cut_by": v}),
    );
    let mono = c.mono;
    edit(
        paint,
        row(ROW_H),
        "poly",
        "Max polyphony",
        i32::from(c.poly_max),
        0,
        32,
        if c.poly_max == 0 {
            "---".to_owned()
        } else {
            c.poly_max.to_string()
        },
        "channel.polyphony.set",
        &|v| json!({"id": id, "max": v, "mono": mono}),
    );
    let (max, mono) = (c.poly_max, c.mono);
    edit(
        paint,
        row(ROW_H),
        "slide",
        "Slide time",
        i32::from(c.slide_ms),
        0,
        i32::from(jet_model::MAX_SLIDE_MS),
        format!("{} ms", c.slide_ms),
        "channel.polyphony.set",
        &|v| json!({"id": id, "max": max, "mono": mono, "slide_ms": v}),
    );
    let r = row(ROW_H);
    if paint.toggle(
        "channel-settings.mono",
        Rect::from_min_size(r.min, vec2(70.0, r.height())),
        "Mono",
        c.mono,
        "Mono: one note at a time; overlapping notes slide one to another",
    ) {
        actions.push(SettingsAction::Engine {
            id: "channel.polyphony.set",
            params: json!({"id": id, "max": c.poly_max, "mono": !c.mono}),
            gesture: None,
        });
    }
    if paint.toggle(
        "channel-settings.porta",
        Rect::from_min_size(r.min + vec2(76.0, 0.0), vec2(70.0, r.height())),
        "Porta",
        c.porta,
        "Porta: every note slides in pitch from the previous one",
    ) {
        actions.push(SettingsAction::Engine {
            id: "channel.polyphony.set",
            params: json!({"id": id, "max": c.poly_max, "mono": c.mono, "porta": !c.porta}),
            gesture: None,
        });
    }

    // ---- Sampler ----
    if c.is_sampler {
        y += 6.0;
        let header = Rect::from_min_size(pos2(left, y), vec2(width, ROW_H));
        y += ROW_H + 4.0;
        section(&painter, header, "Sampler");
        let load = Rect::from_min_size(
            pos2(header.right() - 150.0, header.top()),
            vec2(150.0, ROW_H),
        );
        if paint.toggle(
            "channel-settings.load",
            load,
            "Load sample...",
            false,
            "Open an audio file",
        ) {
            actions.push(SettingsAction::LoadSample(id));
        }
        let wave = Rect::from_min_size(pos2(left, y), vec2(width, 80.0));
        y += 84.0;
        framed(
            &painter,
            wave,
            2.0,
            theme::shade(t.rack_bg, 0.45),
            theme::shade(t.rack_bg, 0.3),
        );
        if c.waveform.is_empty() {
            painter.text(
                wave.center(),
                Align2::CENTER_CENTER,
                "No sample - drop one on the channel or use Load sample",
                theme::regular(13.0),
                t.hint_text,
            );
        } else {
            let n = c.waveform.len() as f32;
            for (i, (lo, hi)) in c.waveform.iter().enumerate() {
                let x = wave.left() + wave.width() * i as f32 / n;
                let cy = wave.center().y;
                let half = wave.height() * 0.48;
                painter.line_segment(
                    [
                        pos2(x, cy - hi.clamp(-1.0, 1.0) * half),
                        pos2(x, cy - lo.clamp(-1.0, 1.0) * half),
                    ],
                    Stroke::new(1.0, t.step_on),
                );
            }
            let marker = |frac: u16| wave.left() + wave.width() * f32::from(frac) / 1000.0;
            painter.line_segment(
                [
                    pos2(marker(c.sampler.start), wave.top()),
                    pos2(marker(c.sampler.start), wave.bottom()),
                ],
                Stroke::new(1.5, egui::Color32::from_rgb(0xE0, 0x40, 0x40)),
            );
            if c.sampler.looping != SampleLoop::Off {
                for frac in [c.sampler.loop_start, c.sampler.loop_end] {
                    painter.line_segment(
                        [
                            pos2(marker(frac), wave.top()),
                            pos2(marker(frac), wave.bottom()),
                        ],
                        Stroke::new(1.0, t.select_on),
                    );
                }
            }
        }
        if let Some(name) = &c.sample_name {
            painter.text(
                pos2(wave.left() + 4.0, wave.top() + 9.0),
                Align2::LEFT_CENTER,
                name,
                theme::regular(12.0),
                t.title_text,
            );
        }
        let s = c.sampler;
        let set = move |settings: SamplerSettings| json!({"id": id, "settings": settings});
        let mut sampler_row =
            |paint: &mut PaintCtx<'_>,
             key: &str,
             label: &str,
             value: u16,
             change: &dyn Fn(u16) -> SamplerSettings| {
                let r = Rect::from_min_size(pos2(left, y), vec2(width, ROW_H));
                y += ROW_H + 4.0;
                let e = paint.param_row(
                    &format!("channel-settings.{key}"),
                    r,
                    label,
                    i32::from(value),
                    0,
                    1000,
                    &format!("{:.1}%", f32::from(value) / 10.0),
                );
                if let Some(v) = e.value {
                    let next = change(v as u16);
                    if next.valid() {
                        actions.push(SettingsAction::Engine {
                            id: "channel.sampler.set",
                            params: set(next),
                            gesture: Some(format!("settings.{key}.{id}")),
                        });
                    }
                }
                if e.ended {
                    actions.push(SettingsAction::EndGesture);
                }
            };
        sampler_row(paint, "start", "Start offset", s.start, &|v| {
            SamplerSettings { start: v, ..s }
        });
        sampler_row(paint, "loop-start", "Loop start", s.loop_start, &|v| {
            SamplerSettings { loop_start: v, ..s }
        });
        sampler_row(paint, "loop-end", "Loop end", s.loop_end, &|v| {
            SamplerSettings { loop_end: v, ..s }
        });
        let r = Rect::from_min_size(pos2(left, y), vec2(width, ROW_H));
        y += ROW_H + 4.0;
        let mut x = r.left();
        for (key, label, mode) in [
            ("loop-off", "No loop", SampleLoop::Off),
            ("loop-fwd", "Loop", SampleLoop::Forward),
            ("loop-pp", "Ping pong", SampleLoop::PingPong),
        ] {
            let b = Rect::from_min_size(pos2(x, r.top()), vec2(80.0, r.height()));
            if paint.toggle(
                &format!("channel-settings.{key}"),
                b,
                label,
                s.looping == mode,
                "Loop mode [Manual: Use loop points / Ping pong loop]",
            ) && s.looping != mode
            {
                actions.push(SettingsAction::Engine {
                    id: "channel.sampler.set",
                    params: set(SamplerSettings { looping: mode, ..s }),
                    gesture: None,
                });
            }
            x += 84.0;
        }
        let b = Rect::from_min_size(pos2(x + 8.0, r.top()), vec2(80.0, r.height()));
        if paint.toggle(
            "channel-settings.reverse",
            b,
            "Reverse",
            s.reverse,
            "Play the sample backwards",
        ) {
            actions.push(SettingsAction::Engine {
                id: "channel.sampler.set",
                params: set(SamplerSettings {
                    reverse: !s.reverse,
                    ..s
                }),
                gesture: None,
            });
        }
        // Volume envelope [Manual: chansettings_sampler, "ADSR & Filtering"].
        let r = Rect::from_min_size(pos2(left, y), vec2(width, ROW_H));
        y += ROW_H + 4.0;
        let b = Rect::from_min_size(r.min, vec2(150.0, r.height()));
        if paint.toggle(
            "channel-settings.envelope",
            b,
            "Volume envelope",
            s.envelope.is_some(),
            "Without it the sample plays to its end after note-off",
        ) {
            let envelope = if s.envelope.is_some() {
                None
            } else {
                Some(Envelope::default())
            };
            actions.push(SettingsAction::Engine {
                id: "channel.sampler.set",
                params: set(SamplerSettings { envelope, ..s }),
                gesture: None,
            });
        }
        if let Some(env) = s.envelope {
            let mut env_row = |paint: &mut PaintCtx<'_>,
                               key: &str,
                               label: &str,
                               value: u16,
                               max: i32,
                               change: &dyn Fn(u16) -> Envelope,
                               unit: &str| {
                let r = Rect::from_min_size(pos2(left, y), vec2(width, ROW_H));
                y += ROW_H + 4.0;
                let e = paint.param_row(
                    &format!("channel-settings.{key}"),
                    r,
                    label,
                    i32::from(value),
                    0,
                    max,
                    &format!("{value} {unit}"),
                );
                if let Some(v) = e.value {
                    actions.push(SettingsAction::Engine {
                        id: "channel.sampler.set",
                        params: set(SamplerSettings {
                            envelope: Some(change(v as u16)),
                            ..s
                        }),
                        gesture: Some(format!("settings.{key}.{id}")),
                    });
                }
                if e.ended {
                    actions.push(SettingsAction::EndGesture);
                }
            };
            env_row(
                paint,
                "attack",
                "Attack",
                env.attack_ms,
                2000,
                &|v| Envelope {
                    attack_ms: v,
                    ..env
                },
                "ms",
            );
            env_row(
                paint,
                "decay",
                "Decay",
                env.decay_ms,
                2000,
                &|v| Envelope { decay_ms: v, ..env },
                "ms",
            );
            env_row(
                paint,
                "sustain",
                "Sustain",
                env.sustain,
                1000,
                &|v| Envelope { sustain: v, ..env },
                "/1000",
            );
            env_row(
                paint,
                "release",
                "Release",
                env.release_ms,
                4000,
                &|v| Envelope {
                    release_ms: v,
                    ..env
                },
                "ms",
            );
        }
    }

    // ---- Synth ----
    if let Some(p) = &c.synth {
        y += 6.0;
        let header = Rect::from_min_size(pos2(left, y), vec2(width, ROW_H));
        y += ROW_H + 4.0;
        section(&painter, header, "Synth");
        let r = Rect::from_min_size(pos2(left, y), vec2(width, ROW_H));
        y += ROW_H + 4.0;
        let mut x = r.left();
        for (key, label, wave) in [
            ("saw", "Saw", Waveform::Saw),
            ("square", "Square", Waveform::Square),
            ("tri", "Triangle", Waveform::Triangle),
            ("sine", "Sine", Waveform::Sine),
            ("noise", "Noise", Waveform::Noise),
        ] {
            let b = Rect::from_min_size(pos2(x, r.top()), vec2(64.0, r.height()));
            if paint.toggle(
                &format!("channel-settings.wave.{key}"),
                b,
                label,
                p.waveform == wave,
                "Oscillator waveform",
            ) && p.waveform != wave
            {
                actions.push(SettingsAction::Engine { id: "channel.synth.set", params: json!({"id": id, "params": SynthParams { waveform: wave, ..p.clone() }}), gesture: None });
            }
            x += 68.0;
        }
        let rows: [(&str, &str, i32, i32, i32); 7] = [
            ("attack", "Attack (ms)", i32::from(p.attack_ms), 0, 2000),
            ("decay", "Decay (ms)", i32::from(p.decay_ms), 0, 2000),
            ("sustain", "Sustain", i32::from(p.sustain), 0, 1000),
            ("release", "Release (ms)", i32::from(p.release_ms), 0, 4000),
            ("cutoff", "Cutoff (Hz)", i32::from(p.cutoff_hz), 20, 20_000),
            ("resonance", "Resonance", i32::from(p.resonance), 0, 1000),
            (
                "detune",
                "Detune (cents)",
                i32::from(p.detune_cents),
                -100,
                100,
            ),
        ];
        for (index, (key, label, value, min, max)) in rows.into_iter().enumerate() {
            let r = Rect::from_min_size(pos2(left, y), vec2(width, ROW_H));
            y += ROW_H + 4.0;
            let e = paint.param_row(
                &format!("channel-settings.synth.{key}"),
                r,
                label,
                value,
                min,
                max,
                &value.to_string(),
            );
            if let Some(v) = e.value {
                let mut next = p.clone();
                match index {
                    0 => next.attack_ms = v as u16,
                    1 => next.decay_ms = v as u16,
                    2 => next.sustain = v as u16,
                    3 => next.release_ms = v as u16,
                    4 => next.cutoff_hz = v as u16,
                    5 => next.resonance = v as u16,
                    _ => next.detune_cents = v as i16,
                }
                actions.push(SettingsAction::Engine {
                    id: "channel.synth.set",
                    params: json!({"id": id, "params": next}),
                    gesture: Some(format!("settings.synth.{key}.{id}")),
                });
            }
            if e.ended {
                actions.push(SettingsAction::EndGesture);
            }
        }
    }
    let _ = y;
    actions
}

/// INS page [Manual: chansettings_ins]: the filter (type, Mod X cutoff, Mod Y resonance), then
/// for the chosen target (Pan, Volume, Mod X, Mod Y, Pitch) its envelope and its LFO.
fn paint_ins(
    paint: &mut PaintCtx<'_>,
    state: &mut ChannelSettingsState,
    c: &ChannelSettingsData,
    rect: Rect,
    actions: &mut Vec<SettingsAction>,
) {
    let t = *paint.tokens;
    let painter = paint.painter().with_clip_rect(rect);
    let left = rect.left() + 10.0;
    let width = rect.width() - 20.0;
    let mut y = rect.top() + 4.0;
    if !c.is_sampler {
        painter.text(
            pos2(left, y + 10.0),
            Align2::LEFT_TOP,
            "The INS page is for Sampler channels.",
            theme::regular(14.0),
            t.hint_text,
        );
        return;
    }
    let id = c.id;
    let s = c.sampler;
    let send =
        |actions: &mut Vec<SettingsAction>, next: SamplerSettings, gesture: Option<String>| {
            if next.valid() {
                actions.push(SettingsAction::Engine {
                    id: "channel.sampler.set",
                    params: json!({"id": id, "settings": next}),
                    gesture,
                });
            }
        };
    let section = |painter: &egui::Painter, y: f32, title: &str| {
        painter.text(
            pos2(left, y + ROW_H / 2.0),
            Align2::LEFT_CENTER,
            title,
            theme::medium(15.0),
            t.menu_text,
        );
    };
    // A labelled value row; `make` builds the new settings from the value.
    #[allow(clippy::too_many_arguments)]
    fn value_row(
        paint: &mut PaintCtx<'_>,
        actions: &mut Vec<SettingsAction>,
        r: Rect,
        key: &str,
        label: &str,
        value: i32,
        min: i32,
        max: i32,
        display: String,
        id: u64,
        make: &dyn Fn(i32) -> SamplerSettings,
    ) {
        let e = paint.param_row(
            &format!("channel-settings.ins.{key}"),
            r,
            label,
            value,
            min,
            max,
            &display,
        );
        if let Some(v) = e.value {
            let next = make(v);
            if next.valid() {
                actions.push(SettingsAction::Engine {
                    id: "channel.sampler.set",
                    params: json!({"id": id, "settings": next}),
                    gesture: Some(format!("settings.ins.{key}.{id}")),
                });
            }
        }
        if e.ended {
            actions.push(SettingsAction::EndGesture);
        }
    }
    let mut next_row = |gap: f32| {
        y += gap;
        let r = Rect::from_min_size(pos2(left, y), vec2(width, ROW_H));
        y += ROW_H + 4.0;
        r
    };

    // ---- Filter ----
    let header = next_row(0.0);
    section(&painter, header.top(), "Filter");
    const TYPES: [(FilterType, &str); 8] = [
        (FilterType::FastLp, "Fast LP"),
        (FilterType::Lp, "LP"),
        (FilterType::Bp, "BP"),
        (FilterType::Hp, "HP"),
        (FilterType::Bs, "BS"),
        (FilterType::Lp2, "LPx2"),
        (FilterType::SvfLp, "SVF LP"),
        (FilterType::SvfLp2, "SVF LPx2"),
    ];
    for chunk in TYPES.chunks(4) {
        let r = next_row(0.0);
        let w = (r.width() - 12.0) / 4.0;
        for (i, (kind, label)) in chunk.iter().enumerate() {
            let b = Rect::from_min_size(
                pos2(r.left() + i as f32 * (w + 4.0), r.top()),
                vec2(w, r.height()),
            );
            if paint.toggle(
                &format!("channel-settings.ins.filter.{label}"),
                b,
                label,
                s.filter.kind == *kind,
                "Filter type",
            ) && s.filter.kind != *kind
            {
                let mut next = s;
                next.filter.kind = *kind;
                send(actions, next, None);
            }
        }
    }
    let cutoff_hz = 20.0 * 1000f32.powf(f32::from(s.filter.mod_x) / 1000.0);
    value_row(
        paint,
        actions,
        next_row(0.0),
        "mod-x",
        "Mod X (cutoff)",
        i32::from(s.filter.mod_x),
        0,
        1000,
        if s.filter.mod_x == 1000 && s.filter.mod_y == 0 {
            "100% (bypass)".to_owned()
        } else {
            format!("{:.0} Hz", cutoff_hz)
        },
        id,
        &|v| {
            let mut next = s;
            next.filter.mod_x = v as u16;
            next
        },
    );
    value_row(
        paint,
        actions,
        next_row(0.0),
        "mod-y",
        "Mod Y (resonance)",
        i32::from(s.filter.mod_y),
        0,
        1000,
        format!("{}%", s.filter.mod_y / 10),
        id,
        &|v| {
            let mut next = s;
            next.filter.mod_y = v as u16;
            next
        },
    );

    // ---- Target tabs ----
    let r = next_row(4.0);
    let w = (r.width() - 16.0) / 5.0;
    for (i, target) in ModTarget::ALL.iter().enumerate() {
        let label = match target {
            ModTarget::Pan => "Pan",
            ModTarget::Volume => "Volume",
            ModTarget::ModX => "Mod X",
            ModTarget::ModY => "Mod Y",
            ModTarget::Pitch => "Pitch",
        };
        let b = Rect::from_min_size(
            pos2(r.left() + i as f32 * (w + 4.0), r.top()),
            vec2(w, r.height()),
        );
        if paint.toggle(
            &format!(
                "channel-settings.ins.target.{}",
                label.to_lowercase().replace(' ', "-")
            ),
            b,
            label,
            state.target == *target,
            "Envelope and LFO target",
        ) {
            state.target = *target;
        }
    }
    let target = state.target;

    // ---- Envelope ----
    let header = next_row(0.0);
    section(&painter, header.top(), "Envelope");
    match target.env_index() {
        None => {
            painter.text(
                pos2(left + 90.0, header.center().y),
                Align2::LEFT_CENTER,
                if target == ModTarget::Pan {
                    "pan does not support an envelope"
                } else {
                    "the volume envelope is on the SMP page"
                },
                theme::regular(13.0),
                t.hint_text,
            );
        }
        Some(e) => {
            let env = s.mod_env[e];
            let b =
                Rect::from_min_size(pos2(header.right() - 80.0, header.top()), vec2(80.0, ROW_H));
            if paint.toggle(
                "channel-settings.ins.env-on",
                b,
                "On",
                env.on,
                "The envelope applies only when it is on",
            ) {
                let mut next = s;
                next.mod_env[e].on = !env.on;
                send(actions, next, None);
            }
            let times: [(&str, &str, u16); 5] = [
                ("delay", "Delay", env.delay_ms),
                ("attack", "Attack", env.attack_ms),
                ("hold", "Hold", env.hold_ms),
                ("decay", "Decay", env.decay_ms),
                ("release", "Release", env.release_ms),
            ];
            for (field, label, value) in times {
                value_row(
                    paint,
                    actions,
                    next_row(0.0),
                    &format!("env-{field}"),
                    label,
                    i32::from(value),
                    0,
                    5000,
                    format!("{value} ms"),
                    id,
                    &|v| {
                        let mut next = s;
                        let target = &mut next.mod_env[e];
                        let v = v as u16;
                        match field {
                            "delay" => target.delay_ms = v,
                            "attack" => target.attack_ms = v,
                            "hold" => target.hold_ms = v,
                            "decay" => target.decay_ms = v,
                            _ => target.release_ms = v,
                        }
                        next
                    },
                );
            }
            value_row(
                paint,
                actions,
                next_row(0.0),
                "env-sustain",
                "Sustain",
                i32::from(env.sustain),
                0,
                1000,
                format!("{}%", env.sustain / 10),
                id,
                &|v| {
                    let mut next = s;
                    next.mod_env[e].sustain = v as u16;
                    next
                },
            );
            value_row(
                paint,
                actions,
                next_row(0.0),
                "env-amount",
                "Amount",
                i32::from(env.amount),
                -1000,
                1000,
                format!("{:+}%", env.amount / 10),
                id,
                &|v| {
                    let mut next = s;
                    next.mod_env[e].amount = v as i16;
                    next
                },
            );
        }
    }

    // ---- LFO ----
    let header = next_row(4.0);
    section(&painter, header.top(), "LFO");
    let l = target.lfo_index();
    let lfo = s.lfo[l];
    let mut bx = left + 60.0;
    for (shape, label) in [
        (LfoShape::Sine, "Sine"),
        (LfoShape::Triangle, "Triangle"),
        (LfoShape::Square, "Square"),
    ] {
        let b = Rect::from_min_size(pos2(bx, header.top()), vec2(70.0, ROW_H));
        if paint.toggle(
            &format!("channel-settings.ins.lfo-{}", label.to_lowercase()),
            b,
            label,
            lfo.shape == shape,
            "LFO shape",
        ) && lfo.shape != shape
        {
            let mut next = s;
            next.lfo[l].shape = shape;
            send(actions, next, None);
        }
        bx += 74.0;
    }
    for (field, label, value) in [
        ("delay", "Delay", lfo.delay_ms),
        ("attack", "Attack", lfo.attack_ms),
    ] {
        value_row(
            paint,
            actions,
            next_row(0.0),
            &format!("lfo-{field}"),
            label,
            i32::from(value),
            0,
            5000,
            format!("{value} ms"),
            id,
            &|v| {
                let mut next = s;
                if field == "delay" {
                    next.lfo[l].delay_ms = v as u16;
                } else {
                    next.lfo[l].attack_ms = v as u16;
                }
                next
            },
        );
    }
    value_row(
        paint,
        actions,
        next_row(0.0),
        "lfo-amount",
        "Amount",
        i32::from(lfo.amount),
        -1000,
        1000,
        if lfo.amount == 0 {
            "off".to_owned()
        } else {
            format!("{:+}%", lfo.amount / 10)
        },
        id,
        &|v| {
            let mut next = s;
            next.lfo[l].amount = v as i16;
            next
        },
    );
    value_row(
        paint,
        actions,
        next_row(0.0),
        "lfo-speed",
        "Speed",
        lfo.speed_milli as i32,
        10,
        20_000,
        format!(
            "{:.2} {}",
            lfo.speed_milli as f32 / 1000.0,
            if lfo.tempo { "per beat" } else { "Hz" }
        ),
        id,
        &|v| {
            let mut next = s;
            next.lfo[l].speed_milli = v.max(10) as u32;
            next
        },
    );
    let r = next_row(0.0);
    for (i, (key, label, on, hint)) in [
        ("lfo-tempo", "Tempo", lfo.tempo, "Speed in cycles per beat"),
        (
            "lfo-global",
            "Global",
            lfo.global,
            "Free running: the LFO does not restart with each note",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let b = Rect::from_min_size(
            pos2(r.left() + i as f32 * 84.0, r.top()),
            vec2(80.0, r.height()),
        );
        if paint.toggle(&format!("channel-settings.ins.{key}"), b, label, on, hint) {
            let mut next = s;
            if key == "lfo-tempo" {
                next.lfo[l].tempo = !on;
            } else {
                next.lfo[l].global = !on;
            }
            send(actions, next, None);
        }
    }
}
