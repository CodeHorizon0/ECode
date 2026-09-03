mod app;
mod bindings;
mod config;
mod editor;
mod fs;
mod workspace;

use std::time::Instant;

use app::CodeApp;

fn main() -> eframe::Result<()> {
    let startup_time = Instant::now();
    println!("[startup] application started");

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 760.0])
            .with_min_inner_size([800.0, 500.0]),
        ..Default::default()
    };

    eframe::run_native(
        "ECode",
        options,
        Box::new(move |cc| {
            Box::new(CodeApp::new(cc, startup_time))
        }),
    )
}
