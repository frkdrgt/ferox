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
    // Placeholder icon — replace with actual icon bytes in production
    egui::IconData {
        rgba: vec![0u8; 32 * 32 * 4],
        width: 32,
        height: 32,
    }
}
