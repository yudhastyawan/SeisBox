#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use eframe::egui;

use seisbox_core::ui::spatial_map::MapData;

mod ui;
mod cli;
mod cli_runner;

use ui::spatial_dialog::IscState;
use clap::Parser;

pub struct SeisboxIscApp {
    pub isc_state: IscState,
    pub map_data: Option<MapData>,
}

impl Default for SeisboxIscApp {
    fn default() -> Self {
        Self {
            isc_state: IscState::default(),
            map_data: Some(MapData::new()),
        }
    }
}

impl eframe::App for SeisboxIscApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Seisbox ISC Catalog Search & Spatial Map");
            ui.separator();
            
            // Render the ISC panel
            ui::spatial_dialog::show_isc_panel(ui, &mut self.isc_state, &mut self.map_data);
        });
    }
}

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        // Run CLI Mode
        let cli_args = cli::Cli::parse();
        if let Err(e) = cli_runner::run_cli(cli_args) {
            eprintln!("CLI Error: {}", e);
            std::process::exit(1);
        }
        return Ok(());
    }

    // Run GUI Mode
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 900.0])
            .with_min_inner_size([1000.0, 700.0])
            .with_title("Seisbox ISC Catalog"),
        ..Default::default()
    };
    eframe::run_native(
        "Seisbox ISC",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::light());
            Ok(Box::new(SeisboxIscApp::default()))
        }),
    )
}
pub mod io;
