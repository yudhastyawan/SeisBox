#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use eframe::egui;


mod ui;
mod core;
mod cli;
mod cli_runner;

use ui::fdsn_dialog::FdsnState;
use seisbox_core::ui::spatial_map::MapData;
use clap::Parser;

pub struct SeisboxFdsnApp {
    pub fdsn_state: FdsnState,
    pub map_data: Option<MapData>,
}

impl Default for SeisboxFdsnApp {
    fn default() -> Self {
        Self {
            fdsn_state: FdsnState::default(),
            map_data: Some(MapData::new()),
        }
    }
}

impl eframe::App for SeisboxFdsnApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ui::fdsn_dialog::show_fdsn_panel(ctx, &mut self.fdsn_state, &self.map_data);
    }
}

fn main() -> eframe::Result<()> {
    if std::env::args().len() > 1 {
        let args = cli::FdsnArgs::parse();
        if let Err(e) = cli_runner::run_cli(args) {
            eprintln!("CLI Error: {}", e);
            std::process::exit(1);
        }
        return Ok(());
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("Seisbox FDSN Downloader"),
        ..Default::default()
    };
    eframe::run_native(
        "Seisbox FDSN",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::light());
            Ok(Box::new(SeisboxFdsnApp::default()))
        }),
    )
}
