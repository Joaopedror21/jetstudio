//! Automation Clip Settings [Manual: playlist_automationclip, "Settings", "Options Menu"]: Min,
//! Max, Time, the LFO (switch, Speed, Tension, Skew, Pulse width, Level, Multiply) and the Flip
//! vertically / Normalize levels tools. Opened by double-clicking an Automation Clip ("To Open -
//! Double-click the Automation Clip in the Playlist"). Layout is ours (UNMEASURED).

use egui::{Align2, Rect, pos2, vec2};
use jet_model::AutoSettings;

use crate::theme;
use crate::widgets::PaintCtx;

const ROW_H: f32 = 22.0;

pub struct AutomationSettingsView {
    pub name: String,
    pub settings: AutoSettings,
}

pub enum AutomationSettingsAction {
    /// New settings; part of a drag gesture keyed by the control.
    Set {
        settings: AutoSettings,
        gesture: Option<String>,
    },
    EndGesture,
    Flip,
    Normalize,
}

fn tension_name(tension: i16) -> &'static str {
    match tension {
        ..=-667 => "square",
        -666..=0 => "sine",
        1..=666 => "triangle",
        _ => "pulse",
    }
}

pub fn paint_automation_settings(
    paint: &mut PaintCtx<'_>,
    view: Option<&AutomationSettingsView>,
    rect: Rect,
) -> Vec<AutomationSettingsAction> {
    let mut actions = Vec::new();
    let t = *paint.tokens;
    let painter = paint.painter().with_clip_rect(rect);
    painter.rect_filled(rect, 0.0, t.rack_bg);
    let Some(v) = view else {
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            "Double-click an Automation Clip",
            theme::regular(15.0),
            t.title_text,
        );
        return actions;
    };
    let s = v.settings;
    let left = rect.left() + 10.0;
    let width = rect.width() - 20.0;
    let mut y = rect.top() + 8.0;
    let mut next_row = |gap: f32| {
        y += gap;
        let r = Rect::from_min_size(pos2(left, y), vec2(width, ROW_H));
        y += ROW_H + 4.0;
        r
    };
    let title = next_row(0.0);
    painter.text(
        pos2(title.left(), title.center().y),
        Align2::LEFT_CENTER,
        &v.name,
        theme::medium(15.0),
        t.menu_text,
    );
    #[allow(clippy::too_many_arguments)]
    fn row(
        paint: &mut PaintCtx<'_>,
        actions: &mut Vec<AutomationSettingsAction>,
        r: Rect,
        key: &str,
        label: &str,
        value: i32,
        min: i32,
        max: i32,
        display: String,
        make: &dyn Fn(i32) -> AutoSettings,
    ) {
        let e = paint.param_row(
            &format!("automation-settings.{key}"),
            r,
            label,
            value,
            min,
            max,
            &display,
        );
        if let Some(v) = e.value {
            let settings = make(v);
            if settings.valid() {
                actions.push(AutomationSettingsAction::Set {
                    settings,
                    gesture: Some(format!("automation-settings.{key}")),
                });
            }
        }
        if e.ended {
            actions.push(AutomationSettingsAction::EndGesture);
        }
    }
    row(
        paint,
        &mut actions,
        next_row(0.0),
        "min",
        "Min",
        i32::from(s.min),
        0,
        1000,
        format!("{}%", s.min / 10),
        &|x| AutoSettings { min: x as u16, ..s },
    );
    row(
        paint,
        &mut actions,
        next_row(0.0),
        "max",
        "Max",
        i32::from(s.max),
        0,
        1000,
        format!("{}%", s.max / 10),
        &|x| AutoSettings { max: x as u16, ..s },
    );
    row(
        paint,
        &mut actions,
        next_row(0.0),
        "time",
        "Time",
        i32::from(s.time_milli),
        100,
        16_000,
        format!("x{:.2}", f32::from(s.time_milli) / 1000.0),
        &|x| AutoSettings {
            time_milli: x as u16,
            ..s
        },
    );

    // LFO section.
    let header = next_row(6.0);
    painter.text(
        pos2(header.left(), header.center().y),
        Align2::LEFT_CENTER,
        "LFO",
        theme::medium(15.0),
        t.menu_text,
    );
    let lfo = s.lfo;
    let switch = Rect::from_min_size(pos2(header.left() + 60.0, header.top()), vec2(70.0, ROW_H));
    if paint.toggle(
        "automation-settings.lfo",
        switch,
        "LFO",
        lfo.on,
        "LFO Mode: the clip works as an LFO (the envelope is kept)",
    ) {
        let mut next = s;
        next.lfo.on = !lfo.on;
        actions.push(AutomationSettingsAction::Set {
            settings: next,
            gesture: None,
        });
    }
    let multiply = Rect::from_min_size(pos2(switch.right() + 6.0, header.top()), vec2(90.0, ROW_H));
    if paint.toggle(
        "automation-settings.multiply",
        multiply,
        "Multiply",
        lfo.multiply,
        "Multiply the envelope by the LFO (off: add them)",
    ) {
        let mut next = s;
        next.lfo.multiply = !lfo.multiply;
        actions.push(AutomationSettingsAction::Set {
            settings: next,
            gesture: None,
        });
    }
    let with = |f: &dyn Fn(&mut jet_model::AutoLfo)| {
        let mut next = s;
        f(&mut next.lfo);
        next
    };
    row(
        paint,
        &mut actions,
        next_row(0.0),
        "speed",
        "Speed",
        lfo.speed_milli as i32,
        10,
        16_000,
        format!("{:.2} per beat", lfo.speed_milli as f32 / 1000.0),
        &|x| with(&|l| l.speed_milli = x as u32),
    );
    row(
        paint,
        &mut actions,
        next_row(0.0),
        "tension",
        "Tension",
        i32::from(lfo.tension),
        -1000,
        1000,
        tension_name(lfo.tension).to_owned(),
        &|x| with(&|l| l.tension = x as i16),
    );
    row(
        paint,
        &mut actions,
        next_row(0.0),
        "skew",
        "Skew",
        i32::from(lfo.skew),
        -1000,
        1000,
        format!("{:+}%", lfo.skew / 10),
        &|x| with(&|l| l.skew = x as i16),
    );
    row(
        paint,
        &mut actions,
        next_row(0.0),
        "width",
        "Pulse width",
        i32::from(lfo.pulse_width),
        1,
        999,
        format!("{}%", lfo.pulse_width / 10),
        &|x| with(&|l| l.pulse_width = x as u16),
    );
    row(
        paint,
        &mut actions,
        next_row(0.0),
        "level",
        "Level",
        i32::from(lfo.level),
        -1000,
        1000,
        format!("{:+}%", lfo.level / 10),
        &|x| with(&|l| l.level = x as i16),
    );

    // Options menu tools.
    let r = next_row(6.0);
    let flip = Rect::from_min_size(r.min, vec2(130.0, ROW_H));
    if paint.toggle(
        "automation-settings.flip",
        flip,
        "Flip vertically",
        false,
        "Inverts the current envelope",
    ) {
        actions.push(AutomationSettingsAction::Flip);
    }
    let normalize = Rect::from_min_size(pos2(flip.right() + 6.0, r.top()), vec2(130.0, ROW_H));
    if paint.toggle(
        "automation-settings.normalize",
        normalize,
        "Normalize levels",
        false,
        "Scales the envelope so its levels reach 0 and 100%",
    ) {
        actions.push(AutomationSettingsAction::Normalize);
    }
    actions
}
