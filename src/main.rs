#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod config;
mod db;
mod history;
mod ui;

use app::PgClientApp;

fn main() -> anyhow::Result<()> {
    // Initialize tokio runtime on a separate thread pool for DB operations
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;

    let _guard = rt.enter();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("pgclient")
            .with_inner_size([1200.0, 750.0])
            .with_min_inner_size([800.0, 500.0])
            .with_icon(load_icon()),
        ..Default::default()
    };

    eframe::run_native(
        "pgclient",
        native_options,
        Box::new(|cc| Box::new(PgClientApp::new(cc)) as Box<dyn eframe::App>),
    )
    .map_err(|e| anyhow::anyhow!("eframe error: {e}"))
}

fn load_icon() -> egui::IconData {
    let bytes = include_bytes!("../assets/logo.png");
    // Resize to 256×256 — winit on Windows ignores icons larger than this for
    // the taskbar (ICON_BIG). The title bar uses the same data scaled down.
    let img = image::load_from_memory(bytes)
        .expect("Failed to decode logo.png")
        .resize_exact(256, 256, image::imageops::FilterType::Lanczos3)
        .into_rgba8();
    let (width, height) = img.dimensions();
    egui::IconData { rgba: img.into_raw(), width, height }
}
