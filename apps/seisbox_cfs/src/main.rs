#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use eframe::egui;


mod ui;
pub mod core;
pub mod cli;
pub mod plot;

use ui::cfs_dialog::CfsDialogState;
use clap::Parser;

pub struct SeisboxCfsApp {
    pub cfs_dialog_state: CfsDialogState,
}

impl Default for SeisboxCfsApp {
    fn default() -> Self {
        Self {
            cfs_dialog_state: CfsDialogState::default(),
        }
    }
}

impl eframe::App for SeisboxCfsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Render the CFS UI using the new VS Code style layout
        ui::cfs_dialog::show_cfs_app(ctx, &mut self.cfs_dialog_state);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // If command line arguments are provided (other than the executable path), run in CLI mode
    if std::env::args().len() > 1 {
        let args = cli::CliArgs::parse();
        cli::parse_and_run(args);
        return Ok(());
    }

    // Otherwise, launch the GUI
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("Seisbox CFS"),
        ..Default::default()
    };
    eframe::run_native(
        "Seisbox CFS",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::light());
            Ok(Box::new(SeisboxCfsApp::default()))
        }),
    ).map_err(|e| e.into())
}
