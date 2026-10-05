use eframe::egui;
use egui_plot::{Plot, PlotPoints, Points, Line, PlotPoint};
use std::path::PathBuf;
use seisbox_core::core::interpolation::{
    XYZPoint, VariogramModel, VariogramModelType, compute_empirical_variogram,
    interpolate_idw, interpolate_simple_kriging, interpolate_universal_kriging
};
use std::sync::mpsc::{channel, Sender, Receiver};

#[derive(PartialEq)]
pub enum InterpolationMethod {
    IDW,
    SimpleKriging,
    UniversalKriging,
}

pub struct InterpolationState {
    pub is_open: bool,
    pub input_path: String,
    pub raw_pts: Vec<XYZPoint>,
    
    // Column Mapping
    pub raw_csv_data: Vec<Vec<String>>,
    pub csv_headers: Vec<String>,
    pub col_x: usize,
    pub col_y: usize,
    pub col_z: usize,
    
    // Grid Setup
    pub x_min: f64,
    pub x_max: f64,
    pub y_min: f64,
    pub y_max: f64,
    pub grid_res_x: usize,
    pub grid_res_y: usize,
    
    // Interpolation Params
    pub method: InterpolationMethod,
    pub idw_power: f64,
    pub search_radius: f64,
    
    // Variogram Params
    pub vario_nugget: f64,
    pub vario_sill: f64,
    pub vario_range: f64,
    pub vario_model_type: VariogramModelType,
    pub vario_lags: usize,
    pub vario_max_dist: f64,
    pub emp_lags: Vec<f64>,
    pub emp_gamma: Vec<f64>,
    
    // Execution
    pub status_msg: String,
    pub is_calculating: bool,
    pub rx: Option<Receiver<Result<(Vec<f64>, Vec<f64>), String>>>,
    
    // Results
    pub grid_data: Option<Vec<f64>>,
    pub cv_predictions: Option<Vec<f64>>,
}

impl Default for InterpolationState {
    fn default() -> Self {
        Self {
            is_open: true,
            input_path: String::new(),
            raw_pts: Vec::new(),
            raw_csv_data: Vec::new(),
            csv_headers: Vec::new(),
            col_x: 0,
            col_y: 1,
            col_z: 2,
            x_min: 0.0, x_max: 100.0,
            y_min: 0.0, y_max: 100.0,
            grid_res_x: 100, grid_res_y: 100,
            method: InterpolationMethod::IDW,
            idw_power: 2.0,
            search_radius: 1000.0,
            vario_nugget: 0.0,
            vario_sill: 1.0,
            vario_range: 10.0,
            vario_model_type: VariogramModelType::Spherical,
            vario_lags: 20,
            vario_max_dist: 50.0,
            emp_lags: Vec::new(),
            emp_gamma: Vec::new(),
            status_msg: String::new(),
            is_calculating: false,
            rx: None,
            grid_data: None,
            cv_predictions: None,
        }
    }
}

pub fn show_interp_panel(ui: &mut egui::Ui, state: &mut InterpolationState) {
    if !state.is_open {
        return;
    }
    
    let mut is_open = state.is_open;
    
    egui::ScrollArea::both()
        .vscroll(true)
        .show(ui, |ui| {
            // Check for background thread results
            if let Some(rx) = &state.rx {
                if let Ok(res) = rx.try_recv() {
                    state.is_calculating = false;
                    match res {
                        Ok((grid, cv)) => {
                            state.grid_data = Some(grid);
                            state.cv_predictions = Some(cv);
                            state.status_msg = "Interpolation complete.".to_string();
                        }
                        Err(e) => {
                            state.status_msg = format!("Error: {}", e);
                        }
                    }
                    state.rx = None;
                }
            }
            
            // 1. Input Section
            ui.heading("1. Input Data (XYZ CSV)");
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut state.input_path);
                if ui.button("Browse").clicked() {
                    if let Some(path) = rfd::FileDialog::new().add_filter("CSV", &["csv", "txt"]).pick_file() {
                        state.input_path = path.to_string_lossy().to_string();
                        load_csv(state);
                    }
                }
                if ui.button("Load").clicked() {
                    load_csv(state);
                }
            });
            
            if !state.csv_headers.is_empty() {
                ui.horizontal(|ui| {
                    let headers = &state.csv_headers;
                    
                    let mut changed = false;
                    ui.label("X:");
                    changed |= egui::ComboBox::from_id_source("col_x").selected_text(headers.get(state.col_x).unwrap_or(&state.col_x.to_string())).show_ui(ui, |ui| {
                        let mut c = false;
                        for (i, h) in headers.iter().enumerate() { c |= ui.selectable_value(&mut state.col_x, i, h).changed(); }
                        c
                    }).inner.unwrap_or(false);
                    
                    ui.label("Y:");
                    changed |= egui::ComboBox::from_id_source("col_y").selected_text(headers.get(state.col_y).unwrap_or(&state.col_y.to_string())).show_ui(ui, |ui| {
                        let mut c = false;
                        for (i, h) in headers.iter().enumerate() { c |= ui.selectable_value(&mut state.col_y, i, h).changed(); }
                        c
                    }).inner.unwrap_or(false);
                    
                    ui.label("Z:");
                    changed |= egui::ComboBox::from_id_source("col_z").selected_text(headers.get(state.col_z).unwrap_or(&state.col_z.to_string())).show_ui(ui, |ui| {
                        let mut c = false;
                        for (i, h) in headers.iter().enumerate() { c |= ui.selectable_value(&mut state.col_z, i, h).changed(); }
                        c
                    }).inner.unwrap_or(false);
                    
                    if changed {
                        update_raw_pts(state);
                    }
                });
            }
            
            ui.label(format!("Loaded {} points.", state.raw_pts.len()));
            
            ui.separator();
            
            // 2. Settings
            ui.heading("2. Grid Parameters");
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut state.x_min).prefix("X Min: "));
                ui.add(egui::DragValue::new(&mut state.x_max).prefix("X Max: "));
                ui.add(egui::DragValue::new(&mut state.grid_res_x).prefix("Cols: "));
            });
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut state.y_min).prefix("Y Min: "));
                ui.add(egui::DragValue::new(&mut state.y_max).prefix("Y Max: "));
                ui.add(egui::DragValue::new(&mut state.grid_res_y).prefix("Rows: "));
            });
            
            ui.separator();
            ui.heading("3. Interpolation Method");
            ui.horizontal(|ui| {
                ui.radio_value(&mut state.method, InterpolationMethod::IDW, "IDW");
                ui.radio_value(&mut state.method, InterpolationMethod::SimpleKriging, "Simple Kriging");
                ui.radio_value(&mut state.method, InterpolationMethod::UniversalKriging, "Universal Kriging");
            });
            
            if state.method == InterpolationMethod::IDW {
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut state.idw_power).prefix("Power (p): ").speed(0.1));
                    ui.add(egui::DragValue::new(&mut state.search_radius).prefix("Search Radius: "));
                });
            } else {
                // Kriging Variogram setup
                ui.horizontal(|ui| {
                    ui.label("Variogram Model:");
                    egui::ComboBox::from_id_source("varmod")
                        .selected_text(format!("{:?}", state.vario_model_type))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut state.vario_model_type, VariogramModelType::Spherical, "Spherical");
                            ui.selectable_value(&mut state.vario_model_type, VariogramModelType::Exponential, "Exponential");
                            ui.selectable_value(&mut state.vario_model_type, VariogramModelType::Gaussian, "Gaussian");
                            ui.selectable_value(&mut state.vario_model_type, VariogramModelType::Linear, "Linear");
                        });
                    ui.add(egui::DragValue::new(&mut state.vario_nugget).prefix("Nugget: ").speed(0.01));
                    ui.add(egui::DragValue::new(&mut state.vario_sill).prefix("Sill: ").speed(0.01));
                    ui.add(egui::DragValue::new(&mut state.vario_range).prefix("Range: ").speed(1.0));
                });
                
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut state.vario_lags).prefix("Lags: "));
                    ui.add(egui::DragValue::new(&mut state.vario_max_dist).prefix("Max Dist: "));
                    if ui.button("Compute Empirical Variogram").clicked() {
                        let (lags, gamma, _) = compute_empirical_variogram(&state.raw_pts, state.vario_lags, state.vario_max_dist);
                        state.emp_lags = lags;
                        state.emp_gamma = gamma;
                    }
                });
                
                // Plot Variogram
                let plot = Plot::new("variogram_plot").height(200.0).view_aspect(2.0);
                plot.show(ui, |plot_ui| {
                    // Empirical
                    if !state.emp_lags.is_empty() {
                        let mut pts = Vec::new();
                        for i in 0..state.emp_lags.len() {
                            pts.push([state.emp_lags[i], state.emp_gamma[i]]);
                        }
                        plot_ui.points(Points::new(pts).color(egui::Color32::RED).radius(4.0).name("Empirical"));
                    }
                    
                    // Theoretical
                    let mut theo_pts = Vec::new();
                    let model = VariogramModel::new(state.vario_model_type, state.vario_nugget, state.vario_sill, state.vario_range);
                    let steps = 100;
                    for i in 0..=steps {
                        let h = (i as f64 / steps as f64) * state.vario_max_dist;
                        theo_pts.push([h, model.compute_semivariance(h)]);
                    }
                    plot_ui.line(Line::new(theo_pts).color(egui::Color32::BLUE).name("Theoretical"));
                });
            }
            
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Run Interpolation").clicked() && !state.is_calculating {
                    run_interpolation(state);
                }
                
                if state.raw_pts.len() == state.grid_res_x * state.grid_res_y && state.raw_pts.len() > 0 {
                    if ui.button("Export Directly to TIFF (Skip Interpolation)").clicked() {
                        if let Some(path) = rfd::FileDialog::new().set_file_name("raw_grid.tif").save_file() {
                            let w = state.grid_res_x as u32;
                            let h = state.grid_res_y as u32;
                            let mut f32_data = vec![0.0f32; (w * h) as usize];
                            
                            // Map raw points directly assuming they are ordered properly (row by row or col by col).
                            // A safer approach: map each point to the closest (ix, iy) bin.
                            for p in &state.raw_pts {
                                let ix = ((p.x - state.x_min) / (state.x_max - state.x_min) * (w as f64 - 1.0)).round() as u32;
                                let iy = ((p.y - state.y_min) / (state.y_max - state.y_min) * (h as f64 - 1.0)).round() as u32;
                                if ix < w && iy < h {
                                    let tiff_y = h - 1 - iy;
                                    f32_data[(tiff_y * w + ix) as usize] = p.z as f32;
                                }
                            }
                            
                            if let Err(e) = seisbox_core::io::tiff_export::export_grid_to_tiff(
                                &path, &f32_data, w, h, state.x_min, state.x_max, state.y_min, state.y_max
                            ) {
                                state.status_msg = format!("Direct TIFF Error: {}", e);
                            } else {
                                state.status_msg = "Direct TIFF exported.".to_string();
                            }
                        }
                    }
                }
                
                ui.label(&state.status_msg);
            });
            
            // Post-Analysis
            if state.grid_data.is_some() && state.cv_predictions.is_some() {
                ui.separator();
                ui.heading("4. Analysis & Export");
                
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label("Cross Validation (Obs vs Calc)");
                        let plot = Plot::new("cv_plot").height(200.0).width(300.0).data_aspect(1.0);
                        plot.show(ui, |plot_ui| {
                            let mut pts = Vec::new();
                            let mut min_val = f64::MAX;
                            let mut max_val = f64::MIN;
                            if let Some(cv) = &state.cv_predictions {
                                for (i, p) in state.raw_pts.iter().enumerate() {
                                    let c = cv[i];
                                    let o = p.z;
                                    if !c.is_nan() {
                                        pts.push([o, c]);
                                        min_val = min_val.min(o).min(c);
                                        max_val = max_val.max(o).max(c);
                                    }
                                }
                                plot_ui.points(Points::new(pts).radius(2.0).color(egui::Color32::DARK_GREEN));
                                plot_ui.line(Line::new(vec![[min_val, min_val], [max_val, max_val]]).color(egui::Color32::GRAY));
                            }
                        });
                    });
                    
                    ui.vertical(|ui| {
                        if ui.button("Export to TIFF").clicked() {
                            if let Some(path) = rfd::FileDialog::new().set_file_name("grid.tif").save_file() {
                                let w = state.grid_res_x as u32;
                                let h = state.grid_res_y as u32;
                                let mut f32_data = vec![0.0f32; (w * h) as usize];
                                
                                if let Some(gd) = &state.grid_data {
                                    for (i, &v) in gd.iter().enumerate() {
                                        // gd is generated as x outer, y inner. We need to flip for TIFF standard (y outer, top-down)
                                        let ix = i / state.grid_res_y;
                                        let iy = i % state.grid_res_y;
                                        let tiff_y = state.grid_res_y - 1 - iy;
                                        let tiff_x = ix;
                                        f32_data[tiff_y * state.grid_res_x + tiff_x] = v as f32;
                                    }
                                    if let Err(e) = seisbox_core::io::tiff_export::export_grid_to_tiff(
                                        &path, &f32_data, w, h, state.x_min, state.x_max, state.y_min, state.y_max
                                    ) {
                                        state.status_msg = format!("TIFF Error: {}", e);
                                    } else {
                                        state.status_msg = "TIFF exported.".to_string();
                                    }
                                }
                            }
                        }
                        
                        if ui.button("Export to Grid CSV").clicked() {
                            if let Some(path) = rfd::FileDialog::new().set_file_name("grid.csv").save_file() {
                                if let Some(gd) = &state.grid_data {
                                    if let Ok(mut wtr) = csv::Writer::from_path(path) {
                                        let _ = wtr.write_record(&["X", "Y", "Z"]);
                                        for (i, &v) in gd.iter().enumerate() {
                                            let ix = i / state.grid_res_y;
                                            let iy = i % state.grid_res_y;
                                            let x = state.x_min + (ix as f64) * (state.x_max - state.x_min) / ((state.grid_res_x - 1).max(1) as f64);
                                            let y = state.y_min + (iy as f64) * (state.y_max - state.y_min) / ((state.grid_res_y - 1).max(1) as f64);
                                            let _ = wtr.write_record(&[x.to_string(), y.to_string(), v.to_string()]);
                                        }
                                        state.status_msg = "CSV exported.".to_string();
                                    }
                                }
                            }
                        }
                    });
                });
            }
        });
        
    state.is_open = is_open;
}

fn load_csv(state: &mut InterpolationState) {
    if let Ok(content) = std::fs::read_to_string(&state.input_path) {
        state.raw_csv_data.clear();
        state.csv_headers.clear();
        
        let mut first_row = true;
        let mut max_cols = 0;
        
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            
            let parts: Vec<String> = line.split(|c: char| c == ',' || c.is_whitespace()).filter(|s| !s.is_empty()).map(|s| s.to_string()).collect();
            if parts.is_empty() {
                continue;
            }
            
            max_cols = max_cols.max(parts.len());
            
            if first_row {
                first_row = false;
                // Check if first row is header
                if parts[0].parse::<f64>().is_err() {
                    state.csv_headers = parts.clone();
                    continue;
                } else {
                    for i in 0..parts.len() {
                        state.csv_headers.push(format!("Column {}", i));
                    }
                }
            }
            
            state.raw_csv_data.push(parts);
        }
        
        // Ensure defaults are valid
        if state.col_x >= max_cols { state.col_x = 0; }
        if state.col_y >= max_cols { state.col_y = 1.min(max_cols.saturating_sub(1)); }
        if state.col_z >= max_cols { state.col_z = 2.min(max_cols.saturating_sub(1)); }
        
        // Ensure headers cover max_cols
        while state.csv_headers.len() < max_cols {
            state.csv_headers.push(format!("Column {}", state.csv_headers.len()));
        }
        
        update_raw_pts(state);
    }
}

fn update_raw_pts(state: &mut InterpolationState) {
    state.raw_pts.clear();
    for row in &state.raw_csv_data {
        if let (Some(x_str), Some(y_str), Some(z_str)) = (row.get(state.col_x), row.get(state.col_y), row.get(state.col_z)) {
            if let (Ok(x), Ok(y), Ok(z)) = (x_str.parse::<f64>(), y_str.parse::<f64>(), z_str.parse::<f64>()) {
                state.raw_pts.push(XYZPoint{x, y, z});
            }
        }
    }
    
    // Auto-calculate bounds and grid properties
    if !state.raw_pts.is_empty() {
        state.x_min = state.raw_pts.iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
        state.x_max = state.raw_pts.iter().map(|p| p.x).fold(f64::NEG_INFINITY, f64::max);
        state.y_min = state.raw_pts.iter().map(|p| p.y).fold(f64::INFINITY, f64::min);
        state.y_max = state.raw_pts.iter().map(|p| p.y).fold(f64::NEG_INFINITY, f64::max);
        
        // Try to auto-detect grid resolution by checking unique X and Y values
        let mut unique_xs: Vec<f64> = state.raw_pts.iter().map(|p| (p.x * 10000.0).round() / 10000.0).collect();
        let mut unique_ys: Vec<f64> = state.raw_pts.iter().map(|p| (p.y * 10000.0).round() / 10000.0).collect();
        unique_xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        unique_ys.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        unique_xs.dedup();
        unique_ys.dedup();
        
        if unique_xs.len() * unique_ys.len() == state.raw_pts.len() {
            state.grid_res_x = unique_xs.len();
            state.grid_res_y = unique_ys.len();
            state.status_msg = format!("Loaded {} points (Detected Grid: {}x{})", state.raw_pts.len(), state.grid_res_x, state.grid_res_y);
        } else {
            state.status_msg = format!("Loaded {} unstructured points", state.raw_pts.len());
        }
        
        // Default variogram max dist
        state.vario_max_dist = ((state.x_max - state.x_min).powi(2) + (state.y_max - state.y_min).powi(2)).sqrt() * 0.5;
    } else {
        state.status_msg = "No valid data parsed with selected columns.".to_string();
    }
}

fn run_interpolation(state: &mut InterpolationState) {
    state.is_calculating = true;
    state.status_msg = "Calculating...".to_string();
    
    let (tx, rx) = channel();
    state.rx = Some(rx);
    
    // Copy parameters for thread
    let pts = state.raw_pts.clone();
    let method = match state.method {
        InterpolationMethod::IDW => 0,
        InterpolationMethod::SimpleKriging => 1,
        InterpolationMethod::UniversalKriging => 2,
    };
    let power = state.idw_power;
    let radius = state.search_radius;
    
    let model = VariogramModel::new(state.vario_model_type, state.vario_nugget, state.vario_sill, state.vario_range);
    
    let w = state.grid_res_x;
    let h = state.grid_res_y;
    let xmin = state.x_min;
    let xmax = state.x_max;
    let ymin = state.y_min;
    let ymax = state.y_max;
    
    std::thread::spawn(move || {
        // Generate targets
        let mut targets = Vec::with_capacity(w * h);
        for ix in 0..w {
            let x = xmin + (ix as f64) * (xmax - xmin) / ((w - 1).max(1) as f64);
            for iy in 0..h {
                let y = ymin + (iy as f64) * (ymax - ymin) / ((h - 1).max(1) as f64);
                targets.push((x, y));
            }
        }
        
        let cv_targets: Vec<(f64, f64)> = pts.iter().map(|p| (p.x, p.y)).collect();
        
        let (grid, cv) = match method {
            0 => {
                let grid = interpolate_idw(&pts, &targets, power, radius);
                let cv = interpolate_idw(&pts, &cv_targets, power, radius);
                (Ok(grid), cv)
            },
            1 => {
                let mean_z = pts.iter().map(|p| p.z).sum::<f64>() / (pts.len().max(1) as f64);
                let grid = interpolate_simple_kriging(&pts, &targets, &model, mean_z);
                let cv = interpolate_simple_kriging(&pts, &cv_targets, &model, mean_z).unwrap_or_default();
                (grid, cv)
            },
            _ => {
                let grid = interpolate_universal_kriging(&pts, &targets, &model);
                let cv = interpolate_universal_kriging(&pts, &cv_targets, &model).unwrap_or_default();
                (grid, cv)
            }
        };
        
        match grid {
            Ok(g) => { let _ = tx.send(Ok((g, cv))); },
            Err(e) => { let _ = tx.send(Err(e)); }
        }
    });
}
