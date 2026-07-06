use eframe::egui;
use std::sync::{Arc, Mutex};
use std::thread;

use crate::core::bvor_runner::{run_bvor_ensemble, BVorConfig, BVorProgress};
use crate::app::QuakePickApp;

#[derive(Clone, PartialEq, Eq)]
pub enum BVorMode {
    Spatial,
    Temporal,
}

pub struct BVorDialog {
    pub mode: BVorMode,
    pub n_nodes_min: usize,
    pub n_nodes_max: usize,
    pub init_sobol: usize,
    pub init_uniform: usize,
    pub init_data: usize,
    pub init_kde: usize,
    pub init_kmeans: usize,
    pub grid_res: usize,
    pub min_obs: usize,
    
    pub num_threads: usize,
    pub max_threads: usize,
    
    pub input_csv: String,
    pub output_npz: String,
    
    pub is_running: bool,
    pub progress: Arc<Mutex<BVorProgress>>,
}

impl Default for BVorDialog {
    fn default() -> Self {
        Self {
            mode: BVorMode::Spatial,
            n_nodes_min: 2,
            n_nodes_max: 60,
            init_sobol: 30,
            init_uniform: 30,
            init_data: 30,
            init_kde: 30,
            init_kmeans: 1,
            grid_res: 200,
            min_obs: 5,
            
            num_threads: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4),
            max_threads: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4),
            
            input_csv: String::new(),
            output_npz: "bvalue.npz".to_string(),
            
            is_running: false,
            progress: Arc::new(Mutex::new(BVorProgress {
                total: 0,
                completed: 0,
                current_status: "Ready".to_string(),
                log_messages: Vec::new(),
            })),
        }
    }
}

pub fn show_bvor_dialog(ctx: &egui::Context, state: &mut QuakePickApp) {
    let mut open = state.bvor_dialog_open;
    let mut is_running = state.bvor_dialog.is_running;

    egui::Window::new("B-Value Voronoi Analysis")
        .open(&mut open)
        .default_size([700.0, 500.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Left Column: Config
                ui.vertical(|ui| {
                    ui.set_width(300.0);
                    ui.heading("Configuration");
                    ui.separator();
                    
                    ui.horizontal(|ui| {
                        ui.label("Mode:");
                        ui.radio_value(&mut state.bvor_dialog.mode, BVorMode::Spatial, "Spatial");
                        ui.radio_value(&mut state.bvor_dialog.mode, BVorMode::Temporal, "Temporal");
                    });
                    
                    ui.add_space(5.0);
                    ui.label("Number of Voronoi Nodes (Nnodes):");
                    ui.horizontal(|ui| {
                        ui.label("Min:");
                        ui.add(egui::DragValue::new(&mut state.bvor_dialog.n_nodes_min).range(2..=200));
                        ui.label("Max:");
                        ui.add(egui::DragValue::new(&mut state.bvor_dialog.n_nodes_max).range(2..=200));
                    });
                    
                    ui.add_space(5.0);
                    ui.label("Initializations (Realizations):");
                    egui::Grid::new("bvor_init_grid").num_columns(2).show(ui, |ui| {
                        ui.label("Sobol:"); ui.add(egui::DragValue::new(&mut state.bvor_dialog.init_sobol)); ui.end_row();
                        ui.label("Uniform:"); ui.add(egui::DragValue::new(&mut state.bvor_dialog.init_uniform)); ui.end_row();
                        ui.label("Data:"); ui.add(egui::DragValue::new(&mut state.bvor_dialog.init_data)); ui.end_row();
                        ui.label("KDE:"); ui.add(egui::DragValue::new(&mut state.bvor_dialog.init_kde)); ui.end_row();
                        ui.label("K-Means:"); ui.add(egui::DragValue::new(&mut state.bvor_dialog.init_kmeans)); ui.end_row();
                    });
                    
                    ui.add_space(5.0);
                    ui.horizontal(|ui| {
                        ui.label("Grid Resolution:");
                        ui.add(egui::DragValue::new(&mut state.bvor_dialog.grid_res).range(10..=1000));
                    });
                    
                    ui.horizontal(|ui| {
                        ui.label("Min Obs/Cell:");
                        ui.add(egui::DragValue::new(&mut state.bvor_dialog.min_obs).range(3..=100));
                    });
                    
                    ui.add_space(5.0);
                    ui.horizontal(|ui| {
                        ui.label("CPU Cores:");
                        ui.add(egui::DragValue::new(&mut state.bvor_dialog.num_threads).range(1..=state.bvor_dialog.max_threads));
                        ui.label(format!("(Max: {})", state.bvor_dialog.max_threads));
                    });
                    
                    ui.add_space(5.0);
                    ui.label("Input Catalog (.csv):");
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut state.bvor_dialog.input_csv);
                        if ui.button("Browse").clicked() {
                            if let Some(path) = rfd::FileDialog::new().add_filter("CSV", &["csv"]).pick_file() {
                                state.bvor_dialog.input_csv = path.display().to_string();
                            }
                        }
                    });
                    ui.label(egui::RichText::new("Leave empty to use active Spatial Map events.").italics().small());
                    
                    ui.add_space(5.0);
                    ui.label("Output File (.npz):");
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut state.bvor_dialog.output_npz);
                        if ui.button("Browse").clicked() {
                            if let Some(path) = rfd::FileDialog::new().add_filter("NPZ", &["npz"]).save_file() {
                                state.bvor_dialog.output_npz = path.display().to_string();
                            }
                        }
                    });
                    
                    ui.add_space(20.0);
                    
                    if is_running {
                        ui.add_enabled(false, egui::Button::new("Running..."));
                    } else {
                        ui.horizontal(|ui| {
                            let mut run_clicked = false;
                            if ui.button("Run B-Value Analysis").clicked() {
                                run_clicked = true;
                            }
                            
                            if ui.button("Visualize Results").clicked() {
                                state.bvor_vis_dialog.is_open = true;
                            }
                            
                            if run_clicked {
                                let config = BVorConfig {
                                    mode: if state.bvor_dialog.mode == BVorMode::Spatial { "spatial".to_string() } else { "temporal".to_string() },
                                    n_nodes_range: state.bvor_dialog.n_nodes_min..=state.bvor_dialog.n_nodes_max,
                                    init_methods: vec![
                                        ("sobol".to_string(), state.bvor_dialog.init_sobol),
                                        ("uniform".to_string(), state.bvor_dialog.init_uniform),
                                        ("data".to_string(), state.bvor_dialog.init_data),
                                        ("kde".to_string(), state.bvor_dialog.init_kde),
                                        ("kmeans".to_string(), state.bvor_dialog.init_kmeans),
                                    ],
                                    grid_res: state.bvor_dialog.grid_res,
                                    min_obs: state.bvor_dialog.min_obs,
                                    num_threads: state.bvor_dialog.num_threads,
                                };
                                
                                let output_path = state.bvor_dialog.output_npz.clone();
                                let input_csv = state.bvor_dialog.input_csv.clone();
                                let is_spatial = state.bvor_dialog.mode == BVorMode::Spatial;
                                
                                // Try to read from CSV if provided, else use app state
                                let (mut x, mut y, mut m) = (Vec::new(), Vec::new(), Vec::new());
                                
                                let mut parse_error = None;
                                if !input_csv.is_empty() {
                                    match csv::Reader::from_path(&input_csv) {
                                    Ok(mut rdr) => {
                                        let headers = rdr.headers().cloned().unwrap_or_default();
                                        let h_lon = headers.iter().position(|h| h.to_lowercase().contains("lon"));
                                        let h_lat = headers.iter().position(|h| h.to_lowercase().contains("lat"));
                                        let h_mag = headers.iter().position(|h| h.to_lowercase().contains("mag"));
                                        let h_dep = headers.iter().position(|h| h.to_lowercase().contains("dep"));
                                        let h_time = headers.iter().position(|h| h.to_lowercase().contains("time") || h.to_lowercase().contains("date"));
                                        
                                        if let (Some(ilon), Some(ilat), Some(imag), Some(idep), Some(itime)) = (h_lon, h_lat, h_mag, h_dep, h_time) {
                                            let mut t_raw = Vec::new();
                                            for result in rdr.records() {
                                                if let Ok(record) = result {
                                                    if let (Ok(lon), Ok(lat), Ok(mag), Ok(dep)) = (
                                                        record[ilon].parse::<f64>(),
                                                        record[ilat].parse::<f64>(),
                                                        record[imag].parse::<f64>(),
                                                        record[idep].parse::<f64>(),
                                                    ) {
                                                        // Simple timestamp parse or just assume index based for temporal if time format is complex
                                                        // For now, if we can parse time string, great. Otherwise use row index as proxy for time.
                                                        let timestamp = if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&record[itime]) {
                                                            dt.timestamp() as f64
                                                        } else {
                                                            x.len() as f64
                                                        };
                                                        
                                                        t_raw.push((lon, lat, mag, dep, timestamp));
                                                    }
                                                }
                                            }
                                            
                                            let min_t = t_raw.iter().map(|v| v.4).fold(f64::INFINITY, f64::min);
                                            let max_t = t_raw.iter().map(|v| v.4).fold(f64::NEG_INFINITY, f64::max);
                                            
                                            for v in t_raw {
                                                if is_spatial {
                                                    x.push(v.0);
                                                    y.push(v.1);
                                                } else {
                                                    let t_norm = if max_t > min_t { (v.4 - min_t) / (max_t - min_t) } else { 0.0 };
                                                    x.push(t_norm);
                                                    y.push(v.3); // depth
                                                }
                                                m.push(v.2);
                                            }
                                        } else {
                                            parse_error = Some("CSV must contain columns for lon, lat, mag, depth, and time.".to_string());
                                        }
                                    },
                                    Err(e) => {
                                        parse_error = Some(format!("Failed to open CSV: {}", e));
                                    }
                                }
                            } else {
                                // Use app state
                                let mut min_t = f64::INFINITY;
                                let mut max_t = f64::NEG_INFINITY;
                                for eq in &state.isc_events {
                                    let t = eq.timestamp;
                                    if t < min_t { min_t = t; }
                                    if t > max_t { max_t = t; }
                                }
                                
                                for eq in &state.isc_events {
                                    if is_spatial {
                                        x.push(eq.lon);
                                        y.push(eq.lat);
                                    } else {
                                        let t = eq.timestamp;
                                        let t_norm = if max_t > min_t { (t - min_t) / (max_t - min_t) } else { 0.0 };
                                        x.push(t_norm);
                                        y.push(eq.depth_km);
                                    }
                                    m.push(eq.mag);
                                }
                            }
                            
                            let progress = state.bvor_dialog.progress.clone();
                            if let Some(err) = parse_error {
                                let mut p = progress.lock().unwrap();
                                p.current_status = err;
                                state.bvor_dialog.is_running = false;
                                return;
                            }
                            
                            if x.is_empty() {
                                let mut p = progress.lock().unwrap();
                                p.current_status = "Error: No data available to analyze.".to_string();
                                state.bvor_dialog.is_running = false;
                                return;
                            }
                            
                            let progress = state.bvor_dialog.progress.clone();
                            state.bvor_dialog.is_running = true;
                            
                            thread::spawn(move || {
                                if let Err(e) = run_bvor_ensemble(config, x, y, m, progress.clone(), &output_path) {
                                    let mut p = progress.lock().unwrap();
                                    p.current_status = format!("Error: {}", e);
                                }
                            });
                        }
                        });
                    }
                });
                
                ui.separator();
                
                // Right Column: Progress
                ui.vertical(|ui| {
                    ui.heading("Execution Progress");
                    ui.separator();
                    
                    let p = state.bvor_dialog.progress.lock().unwrap();
                    ui.label(format!("Status: {}", p.current_status));
                    
                    if p.total > 0 {
                        let fraction = p.completed as f32 / p.total as f32;
                        ui.add(egui::ProgressBar::new(fraction).text(format!("{}/{}", p.completed, p.total)));
                    } else {
                        ui.add(egui::ProgressBar::new(0.0).text("0/0"));
                    }
                    
                    ui.add_space(10.0);
                    ui.label("Execution Logs:");
                    egui::ScrollArea::vertical().max_height(300.0).stick_to_bottom(true).show(ui, |ui| {
                        for msg in &p.log_messages {
                            ui.label(egui::RichText::new(msg).monospace().size(11.0));
                        }
                    });
                    
                    if !is_running && p.total > 0 && p.completed == p.total {
                        ui.add_space(10.0);
                        ui.label(egui::RichText::new("Analysis Complete! Output saved to bvalue.npz").color(egui::Color32::GREEN));
                    }
                });
            });
        });

    // Check if finished
    if is_running {
        let p = state.bvor_dialog.progress.lock().unwrap();
        if p.total > 0 && p.completed >= p.total {
            state.bvor_dialog.is_running = false;
        } else if p.current_status.starts_with("Error") {
            state.bvor_dialog.is_running = false;
        }
        // Force repaint to animate progress bar
        ctx.request_repaint();
    }

    state.bvor_dialog_open = open;
}
