//! Channel Settings window [Manual: chansettings, chansettings_sampler, chansettings_misc].
//! Opened by left-clicking a Channel button. Layout is ours (UNMEASURED); every control is a
//! real engine command.

use egui::{Align2, Rect, Stroke, pos2, vec2};
use jet_model::{Envelope, SampleLoop, SamplerSettings, SynthParams, Waveform};
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

fn note_name(key: u8) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    format!("{}{}", NAMES[usize::from(key % 12)], key / 12)
}

pub fn paint_channel_settings(
    paint: &mut PaintCtx<'_>,
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
    let mut y = rect.top() + 8.0;
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
    let r = row(ROW_H);
    if paint.toggle(
        "channel-settings.mono",
        Rect::from_min_size(r.min, vec2(70.0, r.height())),
        "Mono",
        c.mono,
        "Mono: one note at a time",
    ) {
        actions.push(SettingsAction::Engine {
            id: "channel.polyphony.set",
            params: json!({"id": id, "max": c.poly_max, "mono": !c.mono}),
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
