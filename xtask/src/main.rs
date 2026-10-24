//! Local quality gates for the JetStudio workspace.

use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

type TaskResult<T = ()> = Result<T, String>;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("xtask: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> TaskResult {
    let root = workspace_root()?;
    match env::args().nth(1).as_deref() {
        Some("ci") => ci(&root),
        Some("layers") => layers(&root),
        Some("assets") => assets(&root),
        Some("help") | Some("--help") | None => {
            println!("Usage: cargo xtask <ci|layers|assets>");
            Ok(())
        }
        Some(other) => Err(format!("unknown subcommand: {other}")),
    }
}

fn workspace_root() -> TaskResult<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "xtask manifest has no parent directory".to_owned())
}

fn ci(root: &Path) -> TaskResult {
    run_cargo(root, &["fmt", "--all", "--", "--check"])?;
    run_cargo(
        root,
        &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
    )?;
    run_cargo(root, &["test", "--workspace"])?;
    layers(root)?;
    assets(root)?;
    println!("xtask ci: all available M1.0 gates passed");
    Ok(())
}

fn run_cargo(root: &Path, args: &[&str]) -> TaskResult {
    println!("cargo {}", args.join(" "));
    let status = Command::new("cargo")
        .args(args)
        .current_dir(root)
        .status()
        .map_err(|error| format!("could not start cargo: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("cargo {} failed with {status}", args.join(" ")))
    }
}

fn layers(root: &Path) -> TaskResult {
    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(root)
        .output()
        .map_err(|error| format!("could not run cargo metadata: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let metadata: Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("invalid cargo metadata JSON: {error}"))?;
    validate_layers(&metadata)?;
    println!("layers: workspace dependencies respect the layer table");
    Ok(())
}

fn layer_for(name: &str) -> Option<u8> {
    match name {
        "jet-time" => Some(0),
        "jet-model" | "jet-format" => Some(1),
        "jet-seq" | "jet-dsp" | "jet-media" => Some(2),
        "jet-audio-graph" => Some(3),
        "jet-engine" | "jet-playback" => Some(4),
        "jet-control" => Some(5),
        "jet-ui" | "jet-fidelity" => Some(6),
        "jetstudio" | "jet-cli" => Some(7),
        "xtask" => Some(8),
        _ => None,
    }
}

fn same_layer_edge_allowed(from: &str, to: &str) -> bool {
    matches!(
        (from, to),
        ("jet-format", "jet-model")
            | ("jet-dsp", "jet-seq")
            | ("jet-media", "jet-dsp")
            | ("jet-playback", "jet-engine")
            | ("jet-fidelity", "jet-ui")
    )
}

fn forbidden_ui_dependency(name: &str) -> bool {
    matches!(name, "eframe" | "winit" | "wgpu" | "rfd" | "egui")
        || name.starts_with("egui-")
        || name.starts_with("egui_")
}

fn validate_layers(metadata: &Value) -> TaskResult {
    let member_ids = metadata["workspace_members"]
        .as_array()
        .ok_or_else(|| "metadata has no workspace_members array".to_owned())?
        .iter()
        .filter_map(Value::as_str)
        .collect::<HashSet<_>>();
    let packages = metadata["packages"]
        .as_array()
        .ok_or_else(|| "metadata has no packages array".to_owned())?;
    let mut members = HashMap::<&str, &Value>::new();
    for package in packages {
        let id = package["id"]
            .as_str()
            .ok_or_else(|| "package has no id".to_owned())?;
        if !member_ids.contains(id) {
            continue;
        }
        let name = package["name"]
            .as_str()
            .ok_or_else(|| format!("workspace package {id} has no name"))?;
        if layer_for(name).is_none() {
            return Err(format!("unregistered workspace package: {name}"));
        }
        members.insert(name, package);
    }
    if members.len() != member_ids.len() {
        return Err("some workspace members are missing from cargo metadata".to_owned());
    }
    for (&name, package) in &members {
        let level = layer_for(name).ok_or_else(|| format!("unregistered package: {name}"))?;
        let dependencies = package["dependencies"]
            .as_array()
            .ok_or_else(|| format!("package {name} has no dependencies array"))?;
        for dependency in dependencies {
            let dep_name = dependency["name"]
                .as_str()
                .ok_or_else(|| format!("package {name} has a dependency without name"))?;
            if dependency["kind"].as_str() == Some("dev") {
                continue;
            }
            if level < 6 && forbidden_ui_dependency(dep_name) {
                return Err(format!(
                    "UI dependency {dep_name} is forbidden below L6 ({name})"
                ));
            }
            if members.contains_key(dep_name) {
                let dep_level = layer_for(dep_name)
                    .ok_or_else(|| format!("unregistered dependency: {dep_name}"))?;
                if dep_level > level
                    || (dep_level == level && !same_layer_edge_allowed(name, dep_name))
                {
                    return Err(format!(
                        "ascending or unapproved same-layer dependency: {name} (L{level}) -> {dep_name} (L{dep_level})"
                    ));
                }
            } else if dep_name.starts_with("jet-") {
                return Err(format!(
                    "unregistered internal dependency: {name} -> {dep_name}"
                ));
            }
        }
    }
    Ok(())
}

fn assets(root: &Path) -> TaskResult {
    let dir = root.join("assets");
    if !dir.exists() {
        println!("assets: no assets/ directory");
        return Ok(());
    }
    let attribution = fs::read_to_string(root.join("ATTRIBUTION.md"))
        .map_err(|error| format!("could not read ATTRIBUTION.md: {error}"))?;
    let mut files = Vec::new();
    collect_assets(&dir, &mut files)?;
    for file in files {
        let relative = file
            .strip_prefix(root)
            .map_err(|error| format!("asset outside workspace: {error}"))?
            .to_string_lossy()
            .replace('\\', "/");
        if !attribution.contains(&relative) {
            return Err(format!("asset missing from ATTRIBUTION.md: {relative}"));
        }
    }
    println!("assets: every asset has an attribution entry");
    Ok(())
}

fn collect_assets(dir: &Path, files: &mut Vec<PathBuf>) -> TaskResult {
    let entries =
        fs::read_dir(dir).map_err(|error| format!("cannot read {}: {error}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("cannot read asset entry: {error}"))?;
        let kind = entry
            .file_type()
            .map_err(|error| format!("cannot inspect {}: {error}", entry.path().display()))?;
        if kind.is_symlink() {
            return Err(format!(
                "asset symlink is not allowed: {}",
                entry.path().display()
            ));
        }
        if kind.is_dir() {
            collect_assets(&entry.path(), files)?;
        } else if kind.is_file() {
            files.push(entry.path());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_layers;
    use serde_json::json;

    #[test]
    fn rejects_ascending_internal_dependency() {
        let metadata = json!({
            "workspace_members": ["time", "ui"],
            "packages": [
                {"id": "time", "name": "jet-time", "dependencies": [
                    {"name": "jet-ui", "kind": null}
                ]},
                {"id": "ui", "name": "jet-ui", "dependencies": []}
            ]
        });
        let result = validate_layers(&metadata);
        assert!(result.is_err_and(|message| message.contains("ascending")));
    }

    #[test]
    fn rejects_ui_dependency_below_l6() {
        let metadata = json!({
            "workspace_members": ["model"],
            "packages": [
                {"id": "model", "name": "jet-model", "dependencies": [
                    {"name": "eframe", "kind": null}
                ]}
            ]
        });
        let result = validate_layers(&metadata);
        assert!(result.is_err_and(|message| message.contains("forbidden below L6")));
    }

    #[test]
    fn allows_documented_format_to_model_edge() {
        let metadata = json!({
            "workspace_members": ["model", "format"],
            "packages": [
                {"id": "model", "name": "jet-model", "dependencies": []},
                {"id": "format", "name": "jet-format", "dependencies": [
                    {"name": "jet-model", "kind": null}
                ]}
            ]
        });
        assert!(validate_layers(&metadata).is_ok());
    }
}
