//! Audio settings (F10) [Manual: envsettings_audio]: device, sample rate, buffer length and
//! the status of the running stream. Layout is ours (UNMEASURED).

use egui::{Align2, Rect, pos2, vec2};

use crate::theme;
use crate::widgets::PaintCtx;

/// Rates offered; the manual recommends 44 100 Hz "where possible".
pub const RATES: [u32; 5] = [44_100, 48_000, 88_200, 96_000, 192_000];
/// Buffer lengths in frames (our choice of steps).
pub const BUFFERS: [u32; 6] = [128, 256, 512, 1024, 2048, 4096];

pub struct AudioSettingsView {
    pub devices: Vec<String>,
    /// Selection to apply (device, rate, buffer); `None` = device default.
    pub pending: (Option<String>, Option<u32>, Option<u32>),
    /// Running stream: (name, rate, channels, format, buffer, fallback) or the open error.
    pub running: Result<jet_playback::DeviceInfo, String>,
    pub callbacks: u64,
    pub late: u64,
    pub load: f32,
}

pub enum AudioSettingsAction {
    Device(Option<String>),
    Rate(Option<u32>),
    Buffer(Option<u32>),
    Apply,
    Refresh,
}

pub fn paint_audio_settings(
    paint: &mut PaintCtx<'_>,
    view: &AudioSettingsView,
    rect: Rect,
) -> Vec<AudioSettingsAction> {
    let mut actions = Vec::new();
    let t = *paint.tokens;
    let painter = paint.painter().with_clip_rect(rect);
    painter.rect_filled(rect, 0.0, t.rack_bg);
    let left = rect.left() + 10.0;
    let width = rect.width() - 20.0;
    let mut y = rect.top() + 8.0;
    let heading = |painter: &egui::Painter, y: f32, text: &str| {
        painter.text(
            pos2(left, y + 10.0),
            Align2::LEFT_CENTER,
            text,
            theme::medium(15.0),
            t.menu_text,
        );
    };

    // Device [Manual: "Device - ... select the audio device"].
    heading(&painter, y, "Output device");
    let refresh = Rect::from_min_size(pos2(left + width - 80.0, y), vec2(80.0, 20.0));
    if paint.toggle(
        "audio-settings.refresh",
        refresh,
        "Refresh",
        false,
        "List the devices again",
    ) {
        actions.push(AudioSettingsAction::Refresh);
    }
    y += 26.0;
    let mut rows: Vec<(Option<String>, String)> = vec![(None, "System default".to_owned())];
    rows.extend(view.devices.iter().map(|d| (Some(d.clone()), d.clone())));
    for (i, (value, label)) in rows.iter().enumerate() {
        let r = Rect::from_min_size(pos2(left, y), vec2(width, 22.0));
        if paint.toggle(
            &format!("audio-settings.device.{i}"),
            r,
            label,
            view.pending.0 == *value,
            "Select the output device",
        ) {
            actions.push(AudioSettingsAction::Device(value.clone()));
        }
        y += 25.0;
    }

    // Sample rate [Manual: "Sample Rate - Sets the sample play-back rate used by the mixer"].
    y += 8.0;
    heading(&painter, y, "Sample rate");
    y += 26.0;
    let mut x = left;
    let choices = std::iter::once(None).chain(RATES.iter().copied().map(Some));
    for (i, rate) in choices.enumerate() {
        let label = rate.map_or_else(|| "Default".to_owned(), |r| format!("{r}"));
        let b = Rect::from_min_size(pos2(x, y), vec2(70.0, 22.0));
        if paint.toggle(
            &format!("audio-settings.rate.{i}"),
            b,
            &label,
            view.pending.1 == rate,
            "Sample rate in Hz",
        ) {
            actions.push(AudioSettingsAction::Rate(rate));
        }
        x += 74.0;
    }
    y += 30.0;

    // Buffer length [Manual: "Longer buffers lower CPU load and reduce audio glitches. However
    // with longer buffers the delay ... is at least equal to the buffer length"].
    heading(&painter, y, "Buffer length");
    y += 26.0;
    let mut x = left;
    let rate_for_latency = view
        .pending
        .1
        .or(view.running.as_ref().ok().map(|r| r.rate))
        .unwrap_or(48_000);
    let choices = std::iter::once(None).chain(BUFFERS.iter().copied().map(Some));
    for (i, frames) in choices.enumerate() {
        let label = frames.map_or_else(
            || "Default".to_owned(),
            |f| {
                format!(
                    "{f} ({:.1} ms)",
                    f as f32 * 1000.0 / rate_for_latency as f32
                )
            },
        );
        let b = Rect::from_min_size(pos2(x, y), vec2(112.0, 22.0));
        if paint.toggle(
            &format!("audio-settings.buffer.{i}"),
            b,
            &label,
            view.pending.2 == frames,
            "Buffer length in samples (latency)",
        ) {
            actions.push(AudioSettingsAction::Buffer(frames));
        }
        x += 116.0;
        if x + 112.0 > rect.right() {
            x = left;
            y += 26.0;
        }
    }
    y += 34.0;
    let apply = Rect::from_min_size(pos2(left, y), vec2(140.0, 24.0));
    if paint.toggle(
        "audio-settings.apply",
        apply,
        "Apply",
        false,
        "Reopen the audio device with these settings",
    ) {
        actions.push(AudioSettingsAction::Apply);
    }
    y += 36.0;

    // Status [Manual: "Status - Shows the status of the selected audio device"; "Underruns"].
    heading(&painter, y, "Status");
    y += 24.0;
    let lines: Vec<String> = match &view.running {
        Ok(info) => {
            let (name, rate, channels, format, buffer, fallback) = (
                &info.name,
                &info.rate,
                &info.channels,
                &info.format,
                &info.buffer,
                &info.fallback,
            );
            let mut lines = vec![
                format!("Device: {name}"),
                format!("{rate} Hz, {channels} channels, {format}"),
                match buffer {
                    Some(frames) => format!(
                        "Buffer: {frames} samples ({:.1} ms)",
                        *frames as f32 * 1000.0 / *rate as f32
                    ),
                    None => "Buffer: device default".to_owned(),
                },
                format!(
                    "Callbacks: {}  Late callbacks: {}  DSP load: {:.1}%",
                    view.callbacks, view.late, view.load
                ),
            ];
            if *fallback {
                lines.push(
                    "The requested rate/buffer was refused; using the device default.".to_owned(),
                );
            }
            lines
        }
        Err(error) => vec![format!("No audio output: {error}")],
    };
    for line in lines {
        painter.text(
            pos2(left, y + 9.0),
            Align2::LEFT_CENTER,
            line,
            theme::regular(13.0),
            t.title_text,
        );
        y += 20.0;
    }
    actions
}
