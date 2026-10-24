//! Render the real JetStudio UI offscreen (wgpu, no window) and save a PNG.
//!
//! `cargo run -p jet-ui --example shot -- out.png [--size WxH] [--ppp N] [--demo] [--show rack,piano,mixer] [--cmd 'id={json}']...`
//!
//! The default is a new empty project at 1567x1029, the size and state of the Windows master
//! reference capture W, so the output can be compared pixel for pixel with it
//! (`tools/compare/compare.py`). `--demo` loads the demo project and opens the Channel Rack.

use egui_kittest::Harness;
use jet_ui::JetApp;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let out = args
        .iter()
        .find(|a| !a.starts_with("--") && a.ends_with(".png"))
        .cloned()
        .unwrap_or_else(|| "jetstudio.png".to_owned());
    let value = |flag: &str| {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let (w, h) = value("--size")
        .and_then(|s| {
            let (a, b) = s.split_once('x')?;
            Some((a.parse::<f32>().ok()?, b.parse::<f32>().ok()?))
        })
        .unwrap_or((1567.0, 1029.0));
    let ppp = value("--ppp")
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(1.0);
    // Default: a new empty project, the state of the Windows reference capture W.
    // `--cmd 'id={json}'` (repeatable) runs engine commands on the project before the capture.
    let commands: Vec<(String, serde_json::Value)> = args
        .iter()
        .enumerate()
        .filter(|(_, a)| *a == "--cmd")
        .filter_map(|(i, _)| args.get(i + 1))
        .filter_map(|c| {
            let (id, json) = c.split_once('=')?;
            Some((id.to_owned(), serde_json::from_str(json).ok()?))
        })
        .collect();
    let demo = args.iter().any(|a| a == "--demo");
    let mut session = if demo {
        jet_ui::demo::demo_session()
    } else {
        jet_engine::Session::default()
    };
    for (id, params) in commands {
        if let Err(error) = session.execute(&id, params) {
            eprintln!("{id}: {error}");
        }
    }
    let mut app = JetApp::with_session(session);
    if demo {
        app.set_playlist_scroll_bars(16.0);
        app.show_window_on_start(jet_ui::WindowKind::ChannelRack);
    }
    // `--show rack,piano,mixer` opens extra windows (in that order, last in front).
    if let Some(list) = value("--show") {
        for name in list.split(',') {
            let kind = match name {
                "rack" => Some(jet_ui::WindowKind::ChannelRack),
                "piano" => Some(jet_ui::WindowKind::PianoRoll),
                "mixer" => Some(jet_ui::WindowKind::Mixer),
                "settings" => Some(jet_ui::WindowKind::ChannelSettings),
                "graph" => Some(jet_ui::WindowKind::GraphEditor),
                _ => None,
            };
            if let Some(kind) = kind {
                app.show_window_on_start(kind);
            }
        }
    }
    // `--ins` shows the Channel Settings INS page (Pitch tab).
    if args.iter().any(|a| a == "--ins") {
        app.show_ins_page(jet_model::ModTarget::Pitch);
    }
    app.set_static_capture(true);
    let mut harness = Harness::builder()
        .with_size(egui::vec2(w, h))
        .with_pixels_per_point(ppp)
        .wgpu()
        .build_ui_state(|ui, app: &mut JetApp| app.ui_pass(ui), app);
    for _ in 0..6 {
        harness.step();
    }
    match harness.render() {
        Ok(image) => match image.save(&out) {
            Ok(()) => println!("wrote {out} ({w}x{h} @ {ppp}x)"),
            Err(error) => eprintln!("{out}: {error}"),
        },
        Err(error) => eprintln!("render failed: {error}"),
    }
}
