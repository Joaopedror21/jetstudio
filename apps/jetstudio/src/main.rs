fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui_viewport(),
        ..Default::default()
    };
    eframe::run_native(
        "JetStudio",
        options,
        Box::new(|_creation_context| Ok(Box::new(jet_ui::JetApp::default()))),
    )
}

fn egui_viewport() -> eframe::egui::ViewportBuilder {
    eframe::egui::ViewportBuilder::default()
        .with_title("JetStudio")
        .with_decorations(false)
        .with_inner_size([1280.0, 800.0]) // UNMEASURED: usable starter window, not derived from FL Studio.
        .with_min_inner_size([800.0, 520.0])
}
