fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui_viewport(),
        ..Default::default()
    };
    eframe::run_native(
        "JetStudio",
        options,
        Box::new(|creation_context| {
            let mut app = jet_ui::JetApp::default();
            app.install_fonts(&creation_context.egui_ctx);
            Ok(Box::new(app))
        }),
    )
}

fn egui_viewport() -> eframe::egui::ViewportBuilder {
    eframe::egui::ViewportBuilder::default()
        .with_title("JetStudio")
        .with_decorations(false)
        .with_inner_size([1567.0, 1029.0]) // [W] size of the Windows reference capture
        .with_min_inner_size([800.0, 520.0])
}
