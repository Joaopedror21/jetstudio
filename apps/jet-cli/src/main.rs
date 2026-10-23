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
        "run" => run_commands(&args[1..]),
        _ => Err(usage()),
    }
}

fn run_commands(args: &[String]) -> Result<(), String> {
    let mut project_path: Option<PathBuf> = None;
    let mut save_path: Option<PathBuf> = None;
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
            _ => return Err(format!("unknown option: {option}")),
        }
        index += 1;
    }
    let mut session = if let Some(path) = &project_path {
        let project = jet_format::load(path).map_err(|e| e.to_string())?;
        Session::from_project(project).map_err(|e| e.to_string())?
    } else {
        Session::new_default()
    };
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
    print_json(
        &json!({ "outcomes": outcomes, "project": session.project(), "transport": session.transport_snapshot() }),
    )
}

fn print_json(value: &Value) -> Result<(), String> {
    let rendered = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    println!("{rendered}");
    Ok(())
}

fn usage() -> String {
    "usage: jet-cli inspect <project.jetproj> | commands | run [--project path] [--cmd 'id={json}']... [--save path]".into()
}
