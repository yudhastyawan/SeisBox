use eframe::egui;
use egui_plot::{Plot, PlotPoints, Points, Text, PlotPoint, PlotImage};
use std::sync::{Arc, Mutex};
use std::thread;

use crate::ui::inp_generator_dialog::{show_inp_generator_dialog, InpGeneratorState};

use crate::core::cfs_parser::{CoulombInput, BatchInput, open_input_file_cui, open_batch_file};
use crate::core::cfs_runner::{calculate_deformation, calculate_coulomb_grid, calculate_coulomb_grid_optimized, calculate_coulomb_batch, DeformationResult, CoulombResult, OptTarget};
use std::sync::mpsc::{channel, Receiver};

pub enum CfsThreadResult {
    Deformation(Vec<DeformationResult>),
    CoulombGrid(Vec<CoulombResult>),
    BatchReceiver(Vec<CoulombResult>),
    Optimized(Vec<(OptTarget, Vec<CoulombResult>)>),
}

#[derive(PartialEq, Clone)]
pub enum CfsMode {
    Deformation,
    CoulombGrid,
    BatchReceiver,
}

pub struct CfsDialogState {
    pub is_open: bool,
    pub input_path: String,
    pub batch_path: String,
    
    pub mode: CfsMode,
    
    pub receiver_strike: f64,
    pub receiver_dip: f64,
    pub receiver_rake: f64,
    
    // Grid Overrides
    pub override_grid: bool,
    pub min_x: f64,
    pub max_x: f64,
    pub min_y: f64,
    pub max_y: f64,
    pub x_inc: f64,
    pub y_inc: f64,
    
    pub min_lon: f64,
    pub max_lon: f64,
    pub min_lat: f64,
    pub max_lat: f64,
    pub lon_inc: f64,
    pub lat_inc: f64,
    
    pub zero_lon: f64,
    pub zero_lat: f64,
    
    pub depth: f64,
    pub is_depth_range: bool,
    pub min_depth: f64,
    pub max_depth: f64,
    pub depth_inc: f64,
    
    pub is_calculating: bool,
    pub calc_rx: Option<Receiver<CfsThreadResult>>,
    
    pub def_results: Option<Vec<DeformationResult>>,
    pub cfs_results: Option<Vec<CoulombResult>>,
    pub parsed_input: Option<CoulombInput>,
    pub calculation_msg: String,
    pub inp_generator: InpGeneratorState,
    
    // Optimization options
    pub opt_enabled: bool,
    pub opt_strike: bool,
    pub opt_dip: bool,
    pub opt_rake: bool,
    pub opt_strike_inc: f64,
    pub opt_dip_inc: f64,
    pub opt_rake_inc: f64,
    pub opt_target_shear: bool,
    pub opt_target_normal: bool,
    pub opt_target_coulomb: bool,
    
    // Multiple result sets for optimization (one per target)
    pub opt_results: Vec<(OptTarget, Vec<CoulombResult>)>,
}

impl Default for CfsDialogState {
    fn default() -> Self {
        Self {
            is_open: false,
            input_path: String::new(),
            batch_path: String::new(),
            mode: CfsMode::CoulombGrid,
            receiver_strike: 30.0,
            receiver_dip: 90.0,
            receiver_rake: 180.0,
            override_grid: false,
            min_x: -100.0,
            max_x: 100.0,
            min_y: -100.0,
            max_y: 100.0,
            x_inc: 5.0,
            y_inc: 5.0,
            min_lon: 0.0,
            max_lon: 0.0,
            min_lat: 0.0,
            max_lat: 0.0,
            lon_inc: 0.0,
            lat_inc: 0.0,
            zero_lon: 0.0,
            zero_lat: 0.0,
            depth: 7.5,
            is_depth_range: false,
            min_depth: 0.0,
            max_depth: 20.0,
            depth_inc: 5.0,
            is_calculating: false,
            calc_rx: None,
            def_results: None,
            cfs_results: None,
            parsed_input: None,
            calculation_msg: String::new(),
            inp_generator: InpGeneratorState::default(),
            opt_enabled: false,
            opt_strike: false,
            opt_dip: false,
            opt_rake: false,
            opt_strike_inc: 10.0,
            opt_dip_inc: 10.0,
            opt_rake_inc: 10.0,
            opt_target_shear: false,
            opt_target_normal: false,
            opt_target_coulomb: true,
            opt_results: Vec::new(),
        }
    }
}

pub fn show_cfs_dialog(ctx: &egui::Context, state: &mut CfsDialogState) {
    let mut is_open = state.is_open;
    
    show_inp_generator_dialog(ctx, &mut state.inp_generator);
    
    // Auto load if generated
    if let Some(path) = state.inp_generator.generated_path.take() {
        state.input_path = path;
        if let Ok(input) = open_input_file_cui(&state.input_path) {
            state.parsed_input = Some(input.clone());
            if input.xvec.len() > 1 {
                state.min_x = *input.xvec.first().unwrap();
                state.max_x = *input.xvec.last().unwrap();
                state.x_inc = input.xvec[1] - input.xvec[0];
            }
            if input.yvec.len() > 1 {
                state.min_y = *input.yvec.first().unwrap();
                state.max_y = *input.yvec.last().unwrap();
                state.y_inc = input.yvec[1] - input.yvec[0];
            }
            state.depth = input.cdepth;
            state.zero_lon = input.map_info.zero_lon;
            state.zero_lat = input.map_info.zero_lat;
            state.min_lon = input.map_info.min_lon;
            state.max_lon = input.map_info.max_lon;
            state.min_lat = input.map_info.min_lat;
            state.max_lat = input.map_info.max_lat;
            
            state.receiver_strike = input.av_strike;
            state.receiver_dip = input.av_dip;
            state.receiver_rake = input.av_rake;
            
            let earth_r = 6371.0;
            let km_per_deg_lat = std::f64::consts::PI / 180.0 * earth_r;
            let km_per_deg_lon = km_per_deg_lat * state.zero_lat.to_radians().cos();
            state.lon_inc = state.x_inc / km_per_deg_lon;
            state.lat_inc = state.y_inc / km_per_deg_lat;
        }
    }
    
    egui::Window::new("Coulomb Stress Change Analysis")
        .open(&mut is_open)
        .resizable(true)
        .default_size([800.0, 600.0])
        .show(ctx, |ui| {
            
            ui.horizontal(|ui| {
                ui.label("Source File (.inp / .dat / .txt):");
                if ui.button("Browse...").clicked() {
                    if let Some(path) = rfd::FileDialog::new().add_filter("Source Files", &["inp", "dat", "txt", "csv"]).pick_file() {
                        state.input_path = path.display().to_string();
                        // Try parsing immediately
                        if let Ok(input) = open_input_file_cui(&state.input_path) {
                            state.parsed_input = Some(input.clone());
                            
                            state.receiver_strike = input.av_strike;
                            state.receiver_dip = input.av_dip;
                            state.receiver_rake = input.av_rake;
                            
                            if input.xvec.len() > 1 {
                                state.min_x = *input.xvec.first().unwrap();
                                state.max_x = *input.xvec.last().unwrap();
                                state.x_inc = input.xvec[1] - input.xvec[0];
                            }
                            if input.yvec.len() > 1 {
                                state.min_y = *input.yvec.first().unwrap();
                                state.max_y = *input.yvec.last().unwrap();
                                state.y_inc = input.yvec[1] - input.yvec[0];
                            }
                            state.depth = input.cdepth;
                            state.zero_lon = input.map_info.zero_lon;
                            state.zero_lat = input.map_info.zero_lat;
                            state.min_lon = input.map_info.min_lon;
                            state.max_lon = input.map_info.max_lon;
                            state.min_lat = input.map_info.min_lat;
                            state.max_lat = input.map_info.max_lat;
                            
                            let earth_r = 6371.0;
                            let km_per_deg_lat = std::f64::consts::PI / 180.0 * earth_r;
                            let km_per_deg_lon = km_per_deg_lat * state.zero_lat.to_radians().cos();
                            state.lon_inc = state.x_inc / km_per_deg_lon;
                            state.lat_inc = state.y_inc / km_per_deg_lat;
                        }
                    }
                }
                if ui.button("Create New...").clicked() {
                    state.inp_generator.is_open = true;
                }
                ui.text_edit_singleline(&mut state.input_path);
            });
            
            ui.horizontal(|ui| {
                ui.radio_value(&mut state.mode, CfsMode::Deformation, "Deformation");
                ui.radio_value(&mut state.mode, CfsMode::CoulombGrid, "Coulomb on Grid");
                ui.radio_value(&mut state.mode, CfsMode::BatchReceiver, "Batch Receiver");
            });
            
            if state.mode == CfsMode::CoulombGrid {
                ui.horizontal(|ui| {
                    ui.label("Receiver:");
                    ui.add(egui::DragValue::new(&mut state.receiver_strike).speed(1.0).prefix("Strike: "));
                    ui.add(egui::DragValue::new(&mut state.receiver_dip).speed(1.0).prefix("Dip: "));
                    ui.add(egui::DragValue::new(&mut state.receiver_rake).speed(1.0).prefix("Rake: "));
                });
                
                ui.checkbox(&mut state.opt_enabled, "🔍 Optimize Receiver Geometry");
                if state.opt_enabled {
                    ui.group(|ui| {
                        ui.label("Parameters to optimize (others fixed from above):");
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut state.opt_strike, "Strike (0°–359°)");
                            if state.opt_strike { ui.add(egui::DragValue::new(&mut state.opt_strike_inc).speed(1.0).prefix("Inc: ")); }
                            
                            ui.checkbox(&mut state.opt_dip, "Dip (0°–90°)");
                            if state.opt_dip { ui.add(egui::DragValue::new(&mut state.opt_dip_inc).speed(1.0).prefix("Inc: ")); }
                            
                            ui.checkbox(&mut state.opt_rake, "Rake (-180°–180°)");
                            if state.opt_rake { ui.add(egui::DragValue::new(&mut state.opt_rake_inc).speed(1.0).prefix("Inc: ")); }
                        });
                        ui.label("Maximize:");
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut state.opt_target_shear, "Shear");
                            ui.checkbox(&mut state.opt_target_normal, "Normal");
                            ui.checkbox(&mut state.opt_target_coulomb, "Coulomb");
                        });
                    });
                }
            }
            
            if state.mode == CfsMode::BatchReceiver {
                ui.horizontal(|ui| {
                    ui.label("Batch File:");
                    if ui.button("Browse...").clicked() {
                        if let Some(path) = rfd::FileDialog::new().add_filter("Batch Files", &["dat", "txt", "csv"]).pick_file() {
                            state.batch_path = path.display().to_string();
                        }
                    }
                    ui.text_edit_singleline(&mut state.batch_path);
                });
            }
            
            ui.separator();
            ui.checkbox(&mut state.override_grid, "Override Grid Parameters");
            if state.override_grid {
                let earth_r = 6371.0;
                let km_per_deg_lat = std::f64::consts::PI / 180.0 * earth_r;
                let km_per_deg_lon = km_per_deg_lat * state.zero_lat.to_radians().cos();
            
                let mut changed_xy = false;
                let mut changed_lonlat = false;
            
                ui.group(|ui| {
                    ui.label("Grid Parameters (X / Y in km)");
                    ui.horizontal(|ui| {
                        if ui.add(egui::DragValue::new(&mut state.min_x).speed(0.1).min_decimals(4).max_decimals(7).prefix("Min X: ")).changed() { changed_xy = true; }
                        if ui.add(egui::DragValue::new(&mut state.max_x).speed(0.1).min_decimals(4).max_decimals(7).prefix("Max X: ")).changed() { changed_xy = true; }
                        if ui.add(egui::DragValue::new(&mut state.x_inc).speed(0.1).min_decimals(4).max_decimals(7).prefix("X-Inc: ")).changed() { changed_xy = true; }
                    });
                    ui.horizontal(|ui| {
                        if ui.add(egui::DragValue::new(&mut state.min_y).speed(0.1).min_decimals(4).max_decimals(7).prefix("Min Y: ")).changed() { changed_xy = true; }
                        if ui.add(egui::DragValue::new(&mut state.max_y).speed(0.1).min_decimals(4).max_decimals(7).prefix("Max Y: ")).changed() { changed_xy = true; }
                        if ui.add(egui::DragValue::new(&mut state.y_inc).speed(0.1).min_decimals(4).max_decimals(7).prefix("Y-Inc: ")).changed() { changed_xy = true; }
                    });
                });
            
                ui.group(|ui| {
                    ui.label("Map Info (Longitude / Latitude in degrees)");
                    ui.horizontal(|ui| {
                        if ui.add(egui::DragValue::new(&mut state.min_lon).speed(0.001).min_decimals(4).max_decimals(7).prefix("Min Lon: ")).changed() { changed_lonlat = true; }
                        if ui.add(egui::DragValue::new(&mut state.max_lon).speed(0.001).min_decimals(4).max_decimals(7).prefix("Max Lon: ")).changed() { changed_lonlat = true; }
                        if ui.add(egui::DragValue::new(&mut state.lon_inc).speed(0.001).min_decimals(4).max_decimals(7).prefix("Lon-Inc: ")).changed() { changed_lonlat = true; }
                    });
                    ui.horizontal(|ui| {
                        if ui.add(egui::DragValue::new(&mut state.min_lat).speed(0.001).min_decimals(4).max_decimals(7).prefix("Min Lat: ")).changed() { changed_lonlat = true; }
                        if ui.add(egui::DragValue::new(&mut state.max_lat).speed(0.001).min_decimals(4).max_decimals(7).prefix("Max Lat: ")).changed() { changed_lonlat = true; }
                        if ui.add(egui::DragValue::new(&mut state.lat_inc).speed(0.001).min_decimals(4).max_decimals(7).prefix("Lat-Inc: ")).changed() { changed_lonlat = true; }
                    });
                });
                
                ui.horizontal(|ui| {
                    ui.label("Target Depth (km):");
                    ui.checkbox(&mut state.is_depth_range, "Use Depth Range");
                });
                
                if state.is_depth_range {
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut state.min_depth).speed(0.1).prefix("Min: "));
                        ui.add(egui::DragValue::new(&mut state.max_depth).speed(0.1).prefix("Max: "));
                        ui.add(egui::DragValue::new(&mut state.depth_inc).speed(0.1).prefix("Inc: "));
                    });
                } else {
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut state.depth).speed(0.5).prefix("Depth: "));
                    });
                }
            
                if changed_xy {
                    state.min_lon = state.zero_lon + (state.min_x / km_per_deg_lon);
                    state.max_lon = state.zero_lon + (state.max_x / km_per_deg_lon);
                    state.min_lat = state.zero_lat + (state.min_y / km_per_deg_lat);
                    state.max_lat = state.zero_lat + (state.max_y / km_per_deg_lat);
                    state.lon_inc = state.x_inc / km_per_deg_lon;
                    state.lat_inc = state.y_inc / km_per_deg_lat;
                } else if changed_lonlat {
                    state.min_x = (state.min_lon - state.zero_lon) * km_per_deg_lon;
                    state.max_x = (state.max_lon - state.zero_lon) * km_per_deg_lon;
                    state.min_y = (state.min_lat - state.zero_lat) * km_per_deg_lat;
                    state.max_y = (state.max_lat - state.zero_lat) * km_per_deg_lat;
                    state.x_inc = state.lon_inc * km_per_deg_lon;
                    state.y_inc = state.lat_inc * km_per_deg_lat;
                }
            }
            
            ui.separator();
            
            if state.is_calculating {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Calculating... This may take a moment using all CPU threads.");
                });
                
                // Process channel messages
                if let Some(rx) = &state.calc_rx {
                    if let Ok(res) = rx.try_recv() {
                        match res {
                            CfsThreadResult::Deformation(r) => state.def_results = Some(r),
                            CfsThreadResult::CoulombGrid(r) | CfsThreadResult::BatchReceiver(r) => state.cfs_results = Some(r),
                            CfsThreadResult::Optimized(r) => state.opt_results = r,
                        }
                        state.is_calculating = false;
                        state.calc_rx = None;
                    }
                }
            } else {
                if ui.button("▶ Run Calculation").clicked() {
                    if let Some(mut input) = state.parsed_input.clone() {
                        if state.override_grid {
                            let xin = state.x_inc.max(0.001);
                            let yin = state.y_inc.max(0.001);
                            
                            let mut xvec = Vec::new();
                            let mut x = state.min_x;
                            while x <= state.max_x + xin * 0.5 {
                                xvec.push(x);
                                x += xin;
                            }
                            
                            let mut yvec = Vec::new();
                            let mut y = state.min_y;
                            while y <= state.max_y + yin * 0.5 {
                                yvec.push(y);
                                y += yin;
                            }
                            input.xvec = xvec;
                            input.yvec = yvec;
                            input.cdepth = state.depth;
                            input.map_info.min_lon = state.min_lon;
                            input.map_info.max_lon = state.max_lon;
                            input.map_info.min_lat = state.min_lat;
                            input.map_info.max_lat = state.max_lat;
                        }
                        let mut depths = Vec::new();
                        if state.override_grid && state.is_depth_range {
                            let mut d = state.min_depth;
                            let inc = state.depth_inc.max(0.001);
                            while d <= state.max_depth + inc * 0.5 {
                                depths.push(d);
                                d += inc;
                            }
                        } else {
                            depths.push(state.depth);
                        }
                        
                        let mode = state.mode.clone();
                        let rx_strike = state.receiver_strike;
                        let rx_dip = state.receiver_dip;
                        let rx_rake = state.receiver_rake;
                        let batch_path = state.batch_path.clone();
                        
                        let (tx, rx) = channel();
                        state.calc_rx = Some(rx);
                        state.is_calculating = true;
                        state.def_results = None;
                        state.cfs_results = None;
                        state.opt_results.clear();
                        
                        let opt_enabled = state.opt_enabled;
                        let opt_strike = state.opt_strike;
                        let opt_dip = state.opt_dip;
                        let opt_rake = state.opt_rake;
                        let opt_s_inc = state.opt_strike_inc;
                        let opt_d_inc = state.opt_dip_inc;
                        let opt_r_inc = state.opt_rake_inc;
                        let targets: Vec<OptTarget> = [
                            (state.opt_target_shear, OptTarget::Shear),
                            (state.opt_target_normal, OptTarget::Normal),
                            (state.opt_target_coulomb, OptTarget::Coulomb),
                        ].iter().filter(|(enabled, _)| *enabled).map(|(_, t)| *t).collect();
                        
                        thread::spawn(move || {
                            match mode {
                                CfsMode::Deformation => {
                                    let res = calculate_deformation(&input, &depths);
                                    let _ = tx.send(CfsThreadResult::Deformation(res));
                                }
                                CfsMode::CoulombGrid => {
                                    if opt_enabled && (opt_strike || opt_dip || opt_rake) {
                                        let mut opt_res = Vec::new();
                                        for target in targets {
                                            let res = calculate_coulomb_grid_optimized(
                                                &input, &depths,
                                                rx_strike, rx_dip, rx_rake,
                                                opt_strike, opt_dip, opt_rake,
                                                opt_s_inc, opt_d_inc, opt_r_inc,
                                                target,
                                            );
                                            opt_res.push((target, res));
                                        }
                                        let _ = tx.send(CfsThreadResult::Optimized(opt_res));
                                    } else {
                                        let res = calculate_coulomb_grid(&input, &depths, rx_strike, rx_dip, rx_rake);
                                        let _ = tx.send(CfsThreadResult::CoulombGrid(res));
                                    }
                                }
                                CfsMode::BatchReceiver => {
                                    if let Ok(batch) = open_batch_file(&batch_path, &input.map_info) {
                                        let res = calculate_coulomb_batch(&input, &batch);
                                        let _ = tx.send(CfsThreadResult::BatchReceiver(res));
                                    }
                                }
                            }
                        });
                    }
                }
            }
            
            ui.separator();
            
            // Show some output stats
            if let Some(res) = &state.cfs_results {
                ui.label(format!("Calculated {} points for Coulomb Stress.", res.len()));
                if ui.button("Save Coulomb Results (.csv)").clicked() {
                    if let Some(path) = rfd::FileDialog::new().set_file_name("coulomb_out.csv").save_file() {
                        write_coulomb_csv(&path, res);
                    }
                }
                
                // Simple scatter plot of Coulomb
                if state.mode == CfsMode::CoulombGrid {
                    let plot = Plot::new("cfs_plot")
                        .data_aspect(1.0)
                        .show_axes([false, false]);
                    
                    plot.show(ui, |plot_ui| {
                        // Plot source faults as lines
                        if let Some(input) = &state.parsed_input {
                            for el in &input.el {
                                let pts = vec![
                                    [el[0], el[1]],
                                    [el[2], el[3]],
                                ];
                                plot_ui.line(egui_plot::Line::new(pts).color(egui::Color32::BLACK).width(2.0));
                            }
                        }
                    });
                }
            }
            
            // Show optimization results
            if !state.opt_results.is_empty() {
                for (target, res) in &state.opt_results {
                    ui.label(format!("Optimized {:?}: {} points", target, res.len()));
                }
                if ui.button("Save Optimized Results (.csv)").clicked() {
                    if let Some(path) = rfd::FileDialog::new().set_file_name("coulomb_opt.csv").save_file() {
                        let path_str = path.display().to_string();
                        let (stem, ext) = if let Some(dot_pos) = path_str.rfind('.') {
                            (&path_str[..dot_pos], &path_str[dot_pos..])
                        } else {
                            (path_str.as_str(), ".csv")
                        };
                        
                        if state.opt_results.len() == 1 {
                            // Single target -> single file, no suffix
                            write_coulomb_csv(&path, &state.opt_results[0].1);
                        } else {
                            // Multiple targets -> one file per target with suffix
                            for (target, res) in &state.opt_results {
                                let out_path = format!("{}{}{}", stem, target.suffix(), ext);
                                write_coulomb_csv(&std::path::PathBuf::from(&out_path), res);
                            }
                        }
                    }
                }
                
                // Plot the first optimization result
                if state.mode == CfsMode::CoulombGrid {
                    let plot = Plot::new("cfs_opt_plot")
                        .data_aspect(1.0)
                        .show_axes([false, false]);
                    
                    plot.show(ui, |plot_ui| {
                        if let Some(input) = &state.parsed_input {
                            for el in &input.el {
                                let pts = vec![
                                    [el[0], el[1]],
                                    [el[2], el[3]],
                                ];
                                plot_ui.line(egui_plot::Line::new(pts).color(egui::Color32::BLACK).width(2.0));
                            }
                        }
                    });
                }
            }
            
            if let Some(res) = &state.def_results {
                ui.label(format!("Calculated {} points for Deformation.", res.len()));
                if ui.button("Save Deformation Results (.csv)").clicked() {
                    if let Some(path) = rfd::FileDialog::new().set_file_name("def_out.csv").save_file() {
                        let mut wtr = csv::Writer::from_path(path).unwrap();
                        wtr.write_record(&["x", "y", "z", "ux", "uy", "uz", "sxx", "syy", "szz", "syz", "sxz", "sxy"]).unwrap();
                        for r in res {
                            wtr.write_record(&[
                                r.x.to_string(), r.y.to_string(), r.z.to_string(),
                                r.ux.to_string(), r.uy.to_string(), r.uz.to_string(),
                                r.sxx.to_string(), r.syy.to_string(), r.szz.to_string(),
                                r.syz.to_string(), r.sxz.to_string(), r.sxy.to_string()
                            ]).unwrap();
                        }
                        wtr.flush().unwrap();
                    }
                }
            }
        });
        
    state.is_open = is_open;
}

fn write_coulomb_csv(path: &std::path::Path, results: &[CoulombResult]) {
    let mut wtr = csv::Writer::from_path(path).unwrap();
    wtr.write_record(&[
        "X_km", "Y_km", "Z_km", "Lon", "Lat",
        "Strike", "Dip", "Rake",
        "ux_m", "uy_m", "uz_m",
        "sxx_bar", "syy_bar", "szz_bar",
        "syz_bar", "sxz_bar", "sxy_bar",
        "Shear_bar", "Normal_bar", "Coulomb_bar"
    ]).unwrap();
    for r in results {
        wtr.write_record(&[
            r.x.to_string(), r.y.to_string(), r.z.to_string(),
            r.lon.to_string(), r.lat.to_string(),
            r.strike.to_string(), r.dip.to_string(), r.rake.to_string(),
            r.ux.to_string(), r.uy.to_string(), r.uz.to_string(),
            r.sxx.to_string(), r.syy.to_string(), r.szz.to_string(),
            r.syz.to_string(), r.sxz.to_string(), r.sxy.to_string(),
            r.shear.to_string(), r.normal.to_string(), r.coulomb.to_string()
        ]).unwrap();
    }
    wtr.flush().unwrap();
}
