#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::process::Command;

#[test]
fn cli_creates_saves_and_reopens_a_project() {
    let path = std::env::temp_dir().join(format!("jet-cli-smoke-{}.jetproj", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_jet-cli"))
        .args([
            "run",
            "--cmd",
            r#"channel.add={"name":"Kick"}"#,
            "--cmd",
            r#"step.set={"pattern":1,"channel":1,"index":0,"on":true}"#,
            "--cmd",
            r#"playlist.track.add={"name":"Track 1"}"#,
            "--cmd",
            r#"playlist.clip.add={"track":1,"pattern":1,"start":0,"length":384}"#,
            "--save",
            path.to_str().expect("temp path is UTF-8"),
        ])
        .output()
        .expect("CLI process starts");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let reopened = Command::new(env!("CARGO_BIN_EXE_jet-cli"))
        .args(["inspect", path.to_str().expect("temp path is UTF-8")])
        .output()
        .expect("CLI inspect starts");
    assert!(
        reopened.status.success(),
        "{}",
        String::from_utf8_lossy(&reopened.stderr)
    );
    let project: serde_json::Value =
        serde_json::from_slice(&reopened.stdout).expect("valid inspect JSON");
    assert_eq!(project["channels"][0]["name"], "Kick");
    assert_eq!(
        project["arrangements"][0]["clips"].as_array().map(Vec::len),
        Some(1)
    );
    let _ = std::fs::remove_file(&path);
}
