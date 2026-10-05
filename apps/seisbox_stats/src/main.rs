#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use eframe::egui;

mod ui;
mod core;
mod io;
pub mod cli;

use ui::bvor_dialog::{BVorDialog, BVorMode};
use ui::bvor_vis_dialog::BVorVisState;
use clap::Parser;

pub struct SeisboxStatsApp {
    pub bvor_dialog: BVorDialog,
    pub bvor_vis_dialog: BVorVisState,
}

impl Default for SeisboxStatsApp {
    fn default() -> Self {
        Self {
            bvor_dialog: BVorDialog::default(),
            bvor_vis_dialog: BVorVisState::default(),
        }
    }
}

impl eframe::App for SeisboxStatsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("SeisBox Statistical Seismology");
            ui.separator();
            
            // Render the config panel
            ui::bvor_dialog::show_bvor_panel(ui, self);
        });
        
        // Visualizer window (if open)
        if self.bvor_vis_dialog.is_open {
            ui::bvor_vis_dialog::show_bvor_vis_dialog(ctx, self);
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // If command line arguments are provided, run in CLI mode
    if std::env::args().len() > 1 {
        let args = cli::CliArgs::parse();
        cli::run_cli(args);
        return Ok(());
    }

    // Otherwise, launch the GUI
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("SeisBox Stats"),
        ..Default::default()
    };
    eframe::run_native(
        "SeisBox Stats",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::light());
            Ok(Box::new(SeisboxStatsApp::default()))
        }),
    ).map_err(|e| e.into())
}
