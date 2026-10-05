#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use eframe::egui;


mod ui;

use ui::interpolation_dialog::InterpolationState;

pub struct SeisboxInterpApp {
    pub interp_state: InterpolationState,
}

impl Default for SeisboxInterpApp {
    fn default() -> Self {
        Self {
            interp_state: InterpolationState::default(),
        }
    }
}

impl eframe::App for SeisboxInterpApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Seisbox Advanced Interpolation");
            ui.separator();
            
            // Render the interpolation panel
            ui::interpolation_dialog::show_interp_panel(ui, &mut self.interp_state);
        });
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("Seisbox Interpolation"),
        ..Default::default()
    };
    eframe::run_native(
        "Seisbox Interpolation",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::light());
            Ok(Box::new(SeisboxInterpApp::default()))
        }),
    )
}
