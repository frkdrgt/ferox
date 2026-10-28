#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ai;
mod app;
mod config;
mod db;
mod dbeaver;
mod history;
mod i18n;
mod logger;
mod secrets;
mod snippets;
mod ui;
mod update;

use app::PgClientApp;

fn main() -> anyhow::Result<()> {
    // Panic hook + startup log — must be first.
    logger::setup();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("ferox")
            .with_inner_size([1200.0, 750.0])
            .with_min_inner_size([800.0, 500.0])
            .with_icon(load_icon()),
        ..Default::default()
    };

    let result = eframe::run_native(
        "ferox",
        native_options,
        Box::new(|cc| Ok(Box::new(PgClientApp::new(cc)) as Box<dyn eframe::App>)),
    );

    match &result {
        Ok(_) => logger::write_entry("INFO", "=== Ferox exited normally ==="),
        Err(e) => logger::error(&format!("eframe fatal error: {e}")),
    }

    result.map_err(|e| anyhow::anyhow!("eframe error: {e}"))
}

fn load_icon() -> egui::IconData {
    // Pre-rendered at 256×256 (winit on Windows ignores larger taskbar icons),
    // so startup only decodes — no resize. The title bar scales it down.
    let bytes = include_bytes!("../assets/icon-256.png");
    let img = image::load_from_memory(bytes)
        .expect("Failed to decode icon-256.png")
        .into_rgba8();
    let (width, height) = img.dimensions();
    egui::IconData { rgba: img.into_raw(), width, height }
}
