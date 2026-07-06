mod app;
mod core;
mod io;
mod ui;

use app::QuakePickApp;

fn main() -> eframe::Result {
    let icon = load_icon();

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 800.0])
            .with_min_inner_size([800.0, 500.0])
            .with_title("SeisBox — Seismic Analysis Toolkit")
            .with_icon(std::sync::Arc::new(icon)),
        ..Default::default()
    };

    eframe::run_native(
        "SeisBox",
        options,
        Box::new(|_cc| Ok(Box::new(QuakePickApp::default()))),
    )
}

fn load_icon() -> eframe::egui::IconData {
    let image_bytes = include_bytes!("../assets/SeisBox.iconset/icon_256x256.png");
    let image = image::load_from_memory(image_bytes)
        .expect("Failed to load icon")
        .into_rgba8();
    let (width, height) = image.dimensions();
    let rgba = image.into_raw();
    eframe::egui::IconData {
        rgba,
        width,
        height,
    }
}
