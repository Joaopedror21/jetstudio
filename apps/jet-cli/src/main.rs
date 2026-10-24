#![forbid(unsafe_code)]

use jet_engine::Session;
use serde_json::{Value, json};
use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    match run(env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("jet-cli: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: Vec<String>) -> Result<(), String> {
    let Some(action) = args.first().map(String::as_str) else {
        return Err(usage());
    };
    match action {
        "inspect" => {
            if args.len() != 2 {
                return Err(usage());
            }
            let project = jet_format::load(&args[1]).map_err(|e| e.to_string())?;
            print_json(&serde_json::to_value(project).map_err(|e| e.to_string())?)
        }
        "commands" => print_json(
            &serde_json::to_value(jet_engine::command_specs()).map_err(|e| e.to_string())?,
        ),
        "run" => run_commands(&args[1..], false),
        "play" => run_commands(&args[1..], true),
        _ => Err(usage()),
    }
}

fn run_commands(args: &[String], play: bool) -> Result<(), String> {
    let mut project_path: Option<PathBuf> = None;
    let mut save_path: Option<PathBuf> = None;
    let mut seconds = 2.0_f64;
    let mut request = jet_playback::OutputRequest::default();
    let mut commands = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let option = args[index].as_str();
        index += 1;
        let Some(value) = args.get(index) else {
            return Err(format!("missing value after {option}"));
        };
        match option {
            "--project" => project_path = Some(PathBuf::from(value)),
            "--save" => save_path = Some(PathBuf::from(value)),
            "--cmd" => commands.push(value.clone()),
            "--device" if play => request.device = Some(value.clone()),
            "--rate" if play => {
                request.rate = Some(
                    value
                        .parse()
                        .map_err(|_| format!("invalid --rate {value}"))?,
                );
            }
            "--buffer" if play => {
                request.buffer_frames = Some(
                    value
                        .parse()
                        .map_err(|_| format!("invalid --buffer {value}"))?,
                );
            }
            "--seconds" if play => {
                seconds = value
                    .parse::<f64>()
                    .ok()
                    .filter(|s| (0.1..=600.0).contains(s))
                    .ok_or_else(|| format!("invalid --seconds {value}"))?;
            }
            _ => return Err(format!("unknown option: {option}")),
        }
        index += 1;
    }
    let mut session = Session::new_default();
    if let Some(path) = &project_path {
        // file.open also decodes the project's samples.
        session
            .execute("file.open", json!({ "path": path }))
            .map_err(|e| e.to_string())?;
    }
    let mut outcomes = Vec::new();
    for command in commands {
        let Some((id, json_params)) = command.split_once('=') else {
            return Err(format!("command must be id={{json}}: {command}"));
        };
        let params: Value = serde_json::from_str(json_params).map_err(|e| format!("{id}: {e}"))?;
        let outcome = session.execute(id, params).map_err(|e| e.to_string())?;
        outcomes.push(outcome);
    }
    if let Some(path) = save_path {
        jet_format::save_atomic(&path, session.project()).map_err(|e| e.to_string())?;
    }
    if play {
        return play_session(session, seconds, request);
    }
    print_json(
        &json!({ "outcomes": outcomes, "project": session.project(), "transport": session.transport_snapshot() }),
    )
}

/// Play the session on the default output device and report what the callback measured.
fn play_session(
    mut session: Session,
    seconds: f64,
    request: jet_playback::OutputRequest,
) -> Result<(), String> {
    use jet_playback::AudioOutput;
    use std::sync::atomic::Ordering;
    let output = {
        let session = &session;
        AudioOutput::open(request, |rate| session.build_plan(rate))
    };
    let mut output = output.map_err(|e| e.to_string())?;
    session.set_clock(Box::new(output.clock()));
    session
        .execute("transport.play_stop", json!({}))
        .map_err(|e| e.to_string())?;
    let plan = Box::new(session.build_plan(output.rate()));
    if output.send_plan(plan, session.transport_sync()).is_err() {
        return Err("audio queue full".into());
    }
    let started = std::time::Instant::now();
    let mut master_peak = 0.0_f32;
    let mut peak_load = 0.0_f32;
    let mut max_voices = 0;
    while started.elapsed().as_secs_f64() < seconds {
        std::thread::sleep(std::time::Duration::from_millis(20));
        output.collect();
        let (l, r) = output.meters().take(0);
        master_peak = master_peak.max(l).max(r);
        let (_, peak) = output.stats().take_load();
        peak_load = peak_load.max(peak);
        max_voices = max_voices.max(output.meters().voices());
    }
    session
        .execute("transport.stop", json!({}))
        .map_err(|e| e.to_string())?;
    output.sync(session.transport_sync());
    std::thread::sleep(std::time::Duration::from_millis(100));
    let stats = output.stats();
    let info = output.info();
    print_json(&json!({
        "device": info.name,
        "rate": info.rate,
        "channels": info.channels,
        "format": info.format,
        "buffer_requested_or_default": info.buffer,
        "fell_back_to_default": info.fallback,
        "devices": jet_playback::output_devices(),
        "callbacks": stats.callbacks.load(Ordering::Relaxed),
        "last_buffer_frames": stats.last_buffer_frames.load(Ordering::Relaxed),
        "overruns": stats.overruns.load(Ordering::Relaxed),
        "stream_error": stats.failed.load(Ordering::Relaxed),
        "peak_dsp_load_percent": peak_load,
        "max_voices": max_voices,
        "master_peak": master_peak,
        "master_peak_db": 20.0 * f64::from(master_peak.max(1e-9)).log10(),
        "position_ticks_at_stop": session.transport_snapshot().position.0,
    }))
}

fn print_json(value: &Value) -> Result<(), String> {
    let rendered = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    println!("{rendered}");
    Ok(())
}

fn usage() -> String {
    "usage: jet-cli inspect <project.jetproj> | commands | run [--project path] [--cmd 'id={json}']... [--save path] | play [same options] [--seconds n]".into()
}
