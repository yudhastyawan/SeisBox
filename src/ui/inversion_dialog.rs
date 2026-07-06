use eframe::egui;
use rfd::FileDialog;
use std::path::{PathBuf, Path};
use std::sync::mpsc::{channel, Receiver};
use std::thread;
use std::env;
use egui_plot::{Plot, Line, Points, Polygon, BarChart, Bar, PlotPoints};
use crate::core::rjmcmc::RjmcmcConfig;
use crate::core::rjmcmc_stats::{VisualizerData, load_and_process_data};
use crate::io::plotters_export::generate_rjmcmc_viz;

pub struct InversionState {
    pub show: bool,
    pub obs_file: String,
    pub hvf_path: String,
    pub output_file: String,
    
    pub n_iter: usize,
    pub burnin: usize,
    pub thin: usize,
    
    pub vs_min: f64,
    pub vs_max: f64,
    pub h_min: f64,
    pub h_max: f64,
    pub min_layers: usize,
    pub max_layers: usize,
    pub min_total_depth: f64,
    pub max_total_depth: f64,
    
    pub prob_asc_vs: f64,
    pub prob_asc_h: f64,
    
    pub use_avg_vs: bool,
    pub avg_vs_depth: f64,
    pub avg_vs_min: f64,
    pub avg_vs_max: f64,
    
    pub n_initial_search: usize,
    
    pub f0_min: f64,
    pub f0_max: f64,
    pub f0_weight: f64,
    pub a0_weight: f64,
    
    pub is_running: bool,
    pub log_output: String,
    pub rx: Option<Receiver<String>>,
    
    pub viz_data: Option<VisualizerData>,
    pub viz_error: String,
}

impl Default for InversionState {
    fn default() -> Self {
        // Auto-detect HVf executable based on OS and architecture
        let os = env::consts::OS;
        let arch = env::consts::ARCH;
        
        let binary_name = match (os, arch) {
            ("windows", _) => "HVf.exe",
            ("macos", "aarch64") => "HVf", // Current Apple Silicon binary
            ("macos", "x86_64") => "HVf_mac_x86",
            ("linux", _) => "HVf_linux",
            _ => "HVf"
        };
        
        // Try to find it in bundle, src/libexec (development) or libexec (production)
        let mut hvf_default_path = String::new();
        let dev_path = PathBuf::from(format!("src/libexec/{}", binary_name));
        let prod_path = PathBuf::from(format!("libexec/{}", binary_name));
        let bundle_path = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|parent| parent.join("libexec").join(binary_name)));
        
        if let Some(bp) = bundle_path.filter(|p| p.exists()) {
            hvf_default_path = bp.to_string_lossy().to_string();
        } else if dev_path.exists() {
            hvf_default_path = dev_path.to_string_lossy().to_string();
        } else if prod_path.exists() {
            hvf_default_path = prod_path.to_string_lossy().to_string();
        } else {
            // Fallback: just put the name, maybe it's in PATH
            hvf_default_path = binary_name.to_string();
        }
        
        Self {
            show: false,
            obs_file: String::new(),
            hvf_path: hvf_default_path,
            output_file: "output.jsonl".to_string(),
            n_iter: 100000,
            burnin: 20000,
            thin: 100,
            vs_min: 100.0,
            vs_max: 2000.0,
            h_min: 5.0,
            h_max: 100.0,
            min_layers: 3,
            max_layers: 10,
            min_total_depth: 10.0,
            max_total_depth: 300.0,
            prob_asc_vs: 0.8,
            prob_asc_h: 0.0,
            use_avg_vs: false,
            avg_vs_depth: 30.0,
            avg_vs_min: 150.0,
            avg_vs_max: 800.0,
            n_initial_search: 10,
            f0_min: 1.0,
            f0_max: 2.0,
            f0_weight: 0.0,
            a0_weight: 0.0,
            is_running: false,
            log_output: String::new(),
            rx: None,
            viz_data: None,
            viz_error: String::new(),
        }
    }
}

pub fn render(ctx: &egui::Context, state: &mut InversionState) {
    if !state.show {
        return;
    }

    let mut is_open = state.show;
    
    egui::Window::new("RJ-MCMC HVSR Inversion")
        .open(&mut is_open)
        .default_width(1200.0)
        .default_height(800.0)
        .vscroll(true)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_max_width(350.0); // Make left column smaller
                    ui.heading("Input / Output Config");
            egui::Grid::new("inv_io_grid").num_columns(2).spacing([10.0, 10.0]).show(ui, |ui| {
                ui.label("Observation CSV:");
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut state.obs_file);
                    if ui.button("Browse").clicked() {
                        if let Some(path) = FileDialog::new().add_filter("CSV", &["csv"]).pick_file() {
                            state.obs_file = path.to_string_lossy().to_string();
                        }
                    }
                });
                ui.end_row();
                
                ui.label("HVF Executable:");
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut state.hvf_path);
                    if ui.button("Browse").clicked() {
                        if let Some(path) = FileDialog::new().pick_file() {
                            state.hvf_path = path.to_string_lossy().to_string();
                        }
                    }
                });
                ui.end_row();
                
                ui.label("Output File:");
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut state.output_file);
                    if ui.button("Browse").clicked() {
                        if let Some(path) = FileDialog::new().add_filter("JSONL", &["jsonl"]).save_file() {
                            state.output_file = path.to_string_lossy().to_string();
                        }
                    }
                });
                ui.end_row();
            });
            
            ui.separator();
            
            egui::CollapsingHeader::new("⚙ Inversion Parameters & Configuration")
                .default_open(true)
                .show(ui, |ui| {
                    ui.heading("MCMC Parameters");
            egui::Grid::new("inv_mcmc_grid").num_columns(2).spacing([10.0, 10.0]).show(ui, |ui| {
                ui.label("Total Iterations:");
                ui.add(egui::DragValue::new(&mut state.n_iter).speed(1000));
                ui.end_row();
                
                ui.label("Burn-in:");
                ui.add(egui::DragValue::new(&mut state.burnin).speed(1000));
                ui.end_row();
                
                ui.label("Thinning:");
                ui.add(egui::DragValue::new(&mut state.thin).speed(10));
                ui.end_row();
                
                ui.label("Initial Search Attempts:");
                ui.add(egui::DragValue::new(&mut state.n_initial_search).speed(1));
                ui.end_row();
            });
            
            ui.separator();
            ui.heading("Prior Boundaries");
            egui::Grid::new("inv_prior_grid").num_columns(2).spacing([10.0, 10.0]).show(ui, |ui| {
                ui.label("Vs Range (m/s):");
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut state.vs_min).speed(10.0).prefix("Min: "));
                    ui.add(egui::DragValue::new(&mut state.vs_max).speed(10.0).prefix("Max: "));
                });
                ui.end_row();
                
                ui.label("Thickness Range (m):");
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut state.h_min).speed(1.0).prefix("Min: "));
                    ui.add(egui::DragValue::new(&mut state.h_max).speed(1.0).prefix("Max: "));
                });
                ui.end_row();
                
                ui.label("Layer Count:");
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut state.min_layers).speed(1).prefix("Min: "));
                    ui.add(egui::DragValue::new(&mut state.max_layers).speed(1).prefix("Max: "));
                });
                ui.end_row();
                
                ui.label("Total Depth (m):");
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut state.min_total_depth).speed(10.0).prefix("Min: "));
                    ui.add(egui::DragValue::new(&mut state.max_total_depth).speed(10.0).prefix("Max: "));
                });
                ui.end_row();
            });
            
            ui.separator();
            ui.heading("Structural Probabilities");
            egui::Grid::new("inv_prob_grid").num_columns(2).spacing([10.0, 10.0]).show(ui, |ui| {
                ui.label("Prob Ascending Vs:");
                ui.add(egui::Slider::new(&mut state.prob_asc_vs, 0.0..=1.0));
                ui.end_row();
                
                ui.label("Prob Ascending Thickness:");
                ui.add(egui::Slider::new(&mut state.prob_asc_h, 0.0..=1.0));
                ui.end_row();
            });
            
            ui.separator();
            ui.heading("f0/A0 Target Misfit");
            egui::Grid::new("inv_f0_grid").num_columns(2).spacing([10.0, 10.0]).show(ui, |ui| {
                ui.label("Target f0 Range (Hz):");
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut state.f0_min).speed(0.1).prefix("Min: "));
                    ui.add(egui::DragValue::new(&mut state.f0_max).speed(0.1).prefix("Max: "));
                });
                ui.end_row();
                
                ui.label("Misfit Weights:");
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut state.f0_weight).speed(0.1).prefix("f0 Weight: "));
                    ui.add(egui::DragValue::new(&mut state.a0_weight).speed(0.1).prefix("A0 Weight: "));
                });
                ui.end_row();
            });
            
            ui.separator();
            ui.heading("Average Vs Constraints");
            ui.checkbox(&mut state.use_avg_vs, "Enable Time-Averaged Vs Constraint");
            if state.use_avg_vs {
                egui::Grid::new("inv_avgvs_grid").num_columns(2).spacing([10.0, 10.0]).show(ui, |ui| {
                    ui.label("Target Depth (m):");
                    ui.add(egui::DragValue::new(&mut state.avg_vs_depth).speed(1.0));
                    ui.end_row();
                    
                    ui.label("Avg Vs Range (m/s):");
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut state.avg_vs_min).speed(10.0).prefix("Min: "));
                        ui.add(egui::DragValue::new(&mut state.avg_vs_max).speed(10.0).prefix("Max: "));
                    });
                    ui.end_row();
                });
            }
            }); // End of Collapsing Header
            
            ui.add_space(10.0);
            
            if state.is_running {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Inversion is running... please wait.");
                });
                
                // Read messages from background thread
                if let Some(rx) = &state.rx {
                    while let Ok(msg) = rx.try_recv() {
                        state.log_output.push_str(&msg);
                        state.log_output.push('\n');
                        // Optional limit to avoid massive memory usage
                        if state.log_output.len() > 10000 {
                            let excess = state.log_output.len() - 10000;
                            state.log_output.drain(..excess);
                        }
                    }
                }
            } else {
                if ui.button("Generate Config & Run Inversion").clicked() {
                    if state.obs_file.is_empty() || state.hvf_path.is_empty() {
                        state.log_output = "Error: Observation file or HVF path is empty.".to_string();
                    } else {
                        state.log_output = format!("Starting Rust Native RJ-MCMC Inversion...\nOutput file: {}\n", state.output_file);
                        
                        let config = RjmcmcConfig {
                            obs_file: state.obs_file.clone(),
                            hvf_path: state.hvf_path.clone(),
                            output_file: state.output_file.clone(),
                            n_iter: state.n_iter,
                            burnin: state.burnin,
                            thin: state.thin,
                            vs_min: state.vs_min,
                            vs_max: state.vs_max,
                            h_min: state.h_min,
                            h_max: state.h_max,
                            min_layers: state.min_layers,
                            max_layers: state.max_layers,
                            min_total_depth: state.min_total_depth,
                            max_total_depth: state.max_total_depth,
                            prob_asc_vs: state.prob_asc_vs,
                            prob_asc_h: state.prob_asc_h,
                            use_avg_vs: state.use_avg_vs,
                            avg_vs_depth: state.avg_vs_depth,
                            avg_vs_min: state.avg_vs_min,
                            avg_vs_max: state.avg_vs_max,
                            n_initial_search: state.n_initial_search,
                            f0_min: state.f0_min,
                            f0_max: state.f0_max,
                            f0_weight: state.f0_weight,
                            a0_weight: state.a0_weight,
                        };
                        
                        state.is_running = true;
                        
                        let (tx, rx) = channel();
                        state.rx = Some(rx);
                        
                        thread::spawn(move || {
                            crate::core::rjmcmc::run_inversion(config, tx);
                        });
                    }
                }
            }
            
            // Check if process finished
            if state.is_running && state.log_output.ends_with("DONE\n") {
                state.is_running = false;
            }
            
            ui.add_space(10.0);
            ui.label("Terminal Log:");
            egui::ScrollArea::vertical().stick_to_bottom(true).max_height(150.0).show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut state.log_output.as_str())
                        .font(egui::TextStyle::Monospace)
                        .desired_width(f32::INFINITY)
                        .desired_rows(5)
                        .interactive(false)
                );
            });
            
            }); // End of left column
            
            ui.separator();
            
            ui.vertical(|ui| {
                ui.heading("Result Visualizer");
            
            ui.horizontal(|ui| {
                if ui.button("📂 Load & Visualize JSONL Output").clicked() {
                    if let Some(path) = FileDialog::new().add_filter("JSONL", &["jsonl"]).pick_file() {
                        let jsonl_path = path.to_string_lossy().to_string();
                        if state.obs_file.is_empty() {
                            state.viz_error = "Observation CSV file must be set in the Input Config above to load visualizer.".to_string();
                        } else {
                            match load_and_process_data(&jsonl_path, &state.obs_file) {
                                Ok(data) => {
                                    state.viz_data = Some(data);
                                    state.viz_error.clear();
                                }
                                Err(e) => {
                                    state.viz_error = e;
                                }
                            }
                        }
                    }
                }
                
                if ui.add_enabled(state.viz_data.is_some(), egui::Button::new("🖼 Save Image (.png)")).clicked() {
                    if let Some(path) = FileDialog::new().add_filter("PNG Image", &["png"]).save_file() {
                        if let Some(viz) = &state.viz_data {
                            match generate_rjmcmc_viz(viz, &path) {
                                Ok(_) => state.viz_error = "Image saved successfully.".to_string(),
                                Err(e) => state.viz_error = format!("Failed to save image: {}", e),
                            }
                        }
                    }
                }
            });
            
            if !state.viz_error.is_empty() {
                ui.colored_label(egui::Color32::RED, &state.viz_error);
            }
            
            if let Some(viz) = &state.viz_data {
                ui.add_space(10.0);
                
                // Color mapping utility function
                let cmap = |rmse: f64, min_rmse: f64, max_rmse: f64| -> egui::Color32 {
                    let norm = if max_rmse > min_rmse { (rmse - min_rmse) / (max_rmse - min_rmse) } else { 0.0 };
                    // Simulate Wistia_r: Orange (1.0) to Yellow (0.0) -> let's do Orange to Yellowish green
                    let r = 255;
                    let g = (165.0 + norm * (255.0 - 165.0)) as u8;
                    let b = (0.0 + norm * 100.0) as u8;
                    egui::Color32::from_rgb(r, g, b)
                };
                
                let min_rmse = viz.sorted_rmse.iter().copied().fold(f64::INFINITY, |a, b| a.min(b));
                let max_rmse = viz.sorted_rmse.iter().copied().fold(f64::NEG_INFINITY, |a, b| a.max(b));
                
                egui::Grid::new("viz_plots").num_columns(2).spacing([10.0, 10.0]).show(ui, |ui| {
                    
                    // --- Plot 1: HVSR Curves ---
                    ui.vertical(|ui| {
                        ui.label("Posterior HVSR Fits");
                        Plot::new("hvsr_plot")
                            .width(400.0).height(300.0)
                            .allow_drag(false)
                            .allow_zoom(false)
                            .allow_scroll(false)
                            .show(ui, |plot_ui| {
                                for m in &viz.samples {
                                    let pts: PlotPoints = viz.freq.iter().zip(&m.h_syn).map(|(&x, &y)| [x, y]).collect();
                                    plot_ui.line(Line::new(pts).color(cmap(m.rmse, min_rmse, max_rmse)).name("Syn"));
                                }
                                
                                // Observed
                                let obs_pts: PlotPoints = viz.freq.iter().zip(&viz.h_obs).map(|(&x, &y)| [x, y]).collect();
                                plot_ui.points(Points::new(obs_pts).color(egui::Color32::BLACK).radius(2.0).name("Observed"));
                            });
                    });
                    
                    // --- Plot 2: Vs(z) Profile ---
                    ui.vertical(|ui| {
                        ui.label(format!("Posterior Vs Profiles (Vs30 = {:.0} m/s)", viz.vs30_mean));
                        Plot::new("vsz_plot")
                            .width(400.0).height(300.0)
                            .allow_drag(false)
                            .allow_zoom(false)
                            .allow_scroll(false)
                            .show(ui, |plot_ui| {
                                // Draw samples
                                for m in &viz.samples {
                                    let mut pts = Vec::new();
                                    let mut z_sum = 0.0;
                                    pts.push([m.vs[0], 0.0]);
                                    for i in 0..m.h.len() {
                                        z_sum += m.h[i];
                                        pts.push([m.vs[i], -z_sum]); // inverted Y axis
                                        pts.push([m.vs[i+1], -z_sum]);
                                    }
                                    let last_z = z_sum + 20.0;
                                    pts.push([m.vs.last().copied().unwrap_or(0.0), -last_z]);
                                    
                                    plot_ui.line(Line::new(PlotPoints::new(pts)).color(cmap(m.rmse, min_rmse, max_rmse)));
                                }
                                
                                // Credible interval (P05 & P95 as step lines)
                                let mut p05_pts = Vec::new();
                                let mut p95_pts = Vec::new();
                                for (j, &zz) in viz.z_nodes.iter().enumerate() {
                                    p05_pts.push([viz.vs_p05[j], -zz]);
                                    p95_pts.push([viz.vs_p95[j], -zz]);
                                }
                                plot_ui.line(Line::new(PlotPoints::new(p05_pts)).color(egui::Color32::from_rgb(100, 149, 237)).width(1.0).name("P05"));
                                plot_ui.line(Line::new(PlotPoints::new(p95_pts)).color(egui::Color32::from_rgb(100, 149, 237)).width(1.0).name("P95"));
                                
                                // Median
                                let mut med_pts = Vec::new();
                                for (j, &zz) in viz.z_nodes.iter().enumerate() {
                                    med_pts.push([viz.vs_p50[j], -zz]);
                                }
                                plot_ui.line(Line::new(PlotPoints::new(med_pts)).color(egui::Color32::BLUE).width(2.0).name("Median"));
                                
                                // Best model
                                if let Some(m) = &viz.best_sample {
                                    let mut pts = Vec::new();
                                    let mut z_sum = 0.0;
                                    pts.push([m.vs[0], 0.0]);
                                    for i in 0..m.h.len() {
                                        z_sum += m.h[i];
                                        pts.push([m.vs[i], -z_sum]);
                                        pts.push([m.vs[i+1], -z_sum]);
                                    }
                                    let last_z = z_sum + 20.0;
                                    pts.push([m.vs.last().copied().unwrap_or(0.0), -last_z]);
                                    plot_ui.line(Line::new(PlotPoints::new(pts)).color(egui::Color32::BLACK).width(2.0).name("Best"));
                                }
                                
                                // Add markers (Vs30, H800, etc)
                                if !viz.vs30_mean.is_nan() {
                                    plot_ui.points(Points::new(PlotPoints::new(vec![[viz.vs30_mean, -30.0]])).color(egui::Color32::RED).radius(4.0).name("Vs30"));
                                }
                                if !viz.h800_mean.is_nan() {
                                    plot_ui.points(Points::new(PlotPoints::new(vec![[800.0, -viz.h800_mean]])).color(egui::Color32::DARK_GREEN).radius(4.0).name("H800"));
                                }
                                if !viz.z1_mean.is_nan() {
                                    plot_ui.points(Points::new(PlotPoints::new(vec![[1000.0, -viz.z1_mean]])).color(egui::Color32::from_rgb(0, 139, 139)).radius(4.0).name("Z1.0"));
                                }
                            });
                    });
                    
                    ui.end_row();
                    
                    // --- Plot 3: Histogram RMSE ---
                    ui.vertical(|ui| {
                        ui.label("RMSE Histogram");
                        let mut bins = vec![0; 30];
                        let bin_width = if max_rmse > min_rmse { (max_rmse - min_rmse) / 30.0 } else { 0.1 };
                        
                        for &r in &viz.sorted_rmse {
                            let mut idx = if bin_width > 0.0 { ((r - min_rmse) / bin_width) as usize } else { 0 };
                            if idx >= 30 { idx = 29; }
                            bins[idx] += 1;
                        }
                        
                        let mut bars = Vec::new();
                        for (i, &count) in bins.iter().enumerate() {
                            let x = min_rmse + (i as f64 + 0.5) * bin_width;
                            bars.push(Bar::new(x, count as f64).width(bin_width * 0.9));
                        }
                        
                        Plot::new("rmse_hist")
                            .width(400.0).height(200.0)
                            .allow_drag(false)
                            .allow_zoom(false)
                            .allow_scroll(false)
                            .show(ui, |plot_ui| {
                                plot_ui.bar_chart(BarChart::new(bars).color(egui::Color32::ORANGE));
                            });
                    });
                    
                    // --- Plot 4: RMSE vs Index & Layers vs Index ---
                    ui.vertical(|ui| {
                        ui.label("RMSE vs Index");
                        Plot::new("rmse_index")
                            .width(400.0).height(120.0)
                            .allow_drag(false)
                            .allow_zoom(false)
                            .allow_scroll(false)
                            .show(ui, |plot_ui| {
                                let mut pts = Vec::new();
                                for (i, &r) in viz.sorted_rmse.iter().enumerate() {
                                    pts.push([i as f64, r]);
                                }
                                plot_ui.line(Line::new(PlotPoints::new(pts)).color(egui::Color32::from_rgb(128, 0, 128)).name("RMSE"));
                            });
                            
                        ui.add_space(5.0);
                        ui.label("Number of Layers vs Index");
                        Plot::new("layers_index")
                            .width(400.0).height(120.0)
                            .allow_drag(false)
                            .allow_zoom(false)
                            .allow_scroll(false)
                            .show(ui, |plot_ui| {
                                let mut pts = Vec::new();
                                for (i, m) in viz.samples.iter().enumerate() {
                                    pts.push([i as f64, m.n_layers as f64]);
                                }
                                plot_ui.line(Line::new(PlotPoints::new(pts)).color(egui::Color32::ORANGE).name("Layers"));
                            });
                    });
                    
                    ui.end_row();
                });
                
                // Summary Stats text
                ui.group(|ui| {
                    ui.heading("Geotechnical Parameters Summary");
                    ui.label(format!("Vs30: {:.2} m/s (Range: {:.2} - {:.2})", viz.vs30_mean, viz.vs30_min, viz.vs30_max));
                    ui.label(format!("H800: {:.2} m (Range: {:.2} - {:.2})", viz.h800_mean, viz.h800_min, viz.h800_max));
                    ui.label(format!("Z1.0: {:.2} m (Range: {:.2} - {:.2})", viz.z1_mean, viz.z1_min, viz.z1_max));
                    ui.label(format!("Z2.5: {:.2} m (Range: {:.2} - {:.2})", viz.z2_5_mean, viz.z2_5_min, viz.z2_5_max));
                });
            }
            }); // End of right column
            }); // End of horizontal
        });
        
    state.show = is_open;
}
