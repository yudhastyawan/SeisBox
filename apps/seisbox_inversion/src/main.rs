#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use eframe::egui;


mod ui;
pub mod core;
pub mod io;
pub mod hvf;

use ui::inversion_dialog::InversionState;

pub struct SeisboxInversionApp {
    pub inversion_state: InversionState,
}

impl Default for SeisboxInversionApp {
    fn default() -> Self {
        Self {
            inversion_state: InversionState::default(),
        }
    }
}

impl eframe::App for SeisboxInversionApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Render the inversion panel
        ui::inversion_dialog::show_inversion_panel(ctx, &mut self.inversion_state);
    }
}

use clap::Parser;

fn main() -> eframe::Result {
    // If the user provided arguments (beyond the program name), route to the CLI engine
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        // Run HVf CLI mode
        let cli_args = hvf::cli::Cli::parse();
        let config = match cli_args.into_config() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Error parsing arguments: {}", e);
                std::process::exit(1);
            }
        };

        if let Some(ref inv_method) = config.invert {
            if inv_method == "mcmc" {
                let config_path = config.mcmc_config.clone().expect("Missing --mcmc-config file");
                let cfg_str = std::fs::read_to_string(&config_path).expect("Failed to read mcmc config");
                let rjmcmc_cfg: core::rjmcmc::RjmcmcConfig = serde_json::from_str(&cfg_str).expect("Invalid RJ-MCMC config JSON");
                
                let (tx, rx) = std::sync::mpsc::channel();
                std::thread::spawn(move || {
                    core::rjmcmc::run_inversion(rjmcmc_cfg, tx);
                });
                
                while let Ok(msg) = rx.recv() {
                    if msg == "DONE" {
                        break;
                    }
                    println!("{}", msg);
                }
            } else {
                if let Err(e) = hvf::inversion::run_inversion(&config, None) {
                    eprintln!("Inversion Error: {}", e);
                    std::process::exit(1);
                }
            }
            return Ok(());
        }

        if config.method == "herak" {
            let earth_model = if let Some(ref path) = config.model_file {
                hvf::model::EarthModel::from_file(path, config.use_brocher, config.vp_expr.as_deref(), config.rho_expr.as_deref()).expect("Error reading model file")
            } else if let Some(ref path) = config.model_json {
                hvf::model::EarthModel::from_json(path).expect("Error reading JSON model")
            } else {
                eprintln!("No model file specified.");
                std::process::exit(1);
            };

            let freqs = hvf::compute::build_omega_vector(&config).unwrap_or_else(|e| {
                eprintln!("Error building frequencies: {}", e);
                std::process::exit(1);
            }).into_iter().map(|w| w / (2.0 * std::f64::consts::PI)).collect::<Vec<_>>();

            let hvsr = hvf::herak::compute_hvsr_herak(&earth_model, &freqs, &config);
            
            if config.output_hv {
                if let Some(ref out_file) = config.output_file {
                    use std::io::Write;
                    let mut file = std::fs::File::create(out_file).expect("Failed to create output file");
                    for i in 0..freqs.len() {
                        writeln!(file, "{:.6e}  {:.6e}", freqs[i], hvsr[i]).unwrap();
                    }
                } else if let Some(_ff) = &config.freq_file {
                    use std::io::Write;
                    let out_name = "HV.dat";
                    let mut file = std::fs::File::create(out_name).expect("Failed to create HV.dat");
                    for i in 0..freqs.len() {
                        writeln!(file, "{:.6e}  {:.6e}", freqs[i], hvsr[i]).unwrap();
                    }
                } else {
                    for i in 0..freqs.len() {
                        println!("{:.6e}  {:.6e}", freqs[i], hvsr[i]);
                    }
                }
            }
            
            if config.output_json {
                let out = hvf::output::HvsrOutput {
                    frequencies: freqs,
                    hv_ratio: hvsr,
                    rayleigh_phase: None,
                    love_phase: None,
                };
                let json_str = out.to_json();
                if let Some(ref out_file) = config.output_file {
                    std::fs::write(out_file, json_str).expect("Failed to write JSON output");
                } else {
                    println!("{}", json_str);
                }
            }
        } else {
            if let Err(e) = hvf::compute::run_hvsr(&config) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        
        return Ok(());
    }

    // Default to GUI mode
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("Seisbox Inversion"),
        ..Default::default()
    };
    eframe::run_native(
        "Seisbox Inversion",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::light());
            Ok(Box::new(SeisboxInversionApp::default()))
        }),
    )
}
