use eframe::egui;
use egui_plot::{Plot, Line, PlotPoints, Polygon, VLine, Text, PlotPoint, Legend};
use std::sync::{Arc, Mutex};
use std::thread;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};

use crate::core::math_hvsr::{HvsrParams, HvsrResult, HvsrProgress, process_hvsr_pipeline, HvsrWindows};
use crate::core::math_hvtfa::HvtfaResult;
use crate::cli::CombineMethod;

#[derive(PartialEq, Clone, Copy)]
pub enum ActiveTab {
    Explorer,
    Process,
}

#[derive(Clone)]
pub struct VisData {
    pub hvsr_result: Option<HvsrResult>,
    pub hvtfa_result: Option<HvtfaResult>,
    pub hvsr_windows: Option<HvsrWindows>,
    pub z_comp_plot: Option<Vec<[f64; 2]>>,
    pub n_comp_plot: Option<Vec<[f64; 2]>>,
    pub e_comp_plot: Option<Vec<[f64; 2]>>,
    pub dt: f64,
    pub active_curve_view: usize,
    pub y_axis_log: bool,
}

#[derive(Clone)]
pub enum EditorTab {
    Seismogram(String, Box<VisData>),
    Windows(String, Box<VisData>),
    HvsrCurve(String, Box<VisData>),
}

impl PartialEq for EditorTab {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Seismogram(n1, _), Self::Seismogram(n2, _)) => n1 == n2,
            (Self::Windows(n1, _), Self::Windows(n2, _)) => n1 == n2,
            (Self::HvsrCurve(n1, _), Self::HvsrCurve(n2, _)) => n1 == n2,
            _ => false,
        }
    }
}

pub struct HvsrDialogState {
    pub is_open: bool,
    pub active_tab: ActiveTab,
    pub open_tabs: Vec<EditorTab>,
    pub active_tab_index: usize,
    
    // File Paths
    pub z_file: String,
    pub n_file: String,
    pub e_file: String,
    pub explorer_dir: Option<PathBuf>,
    
    // CLI Parameters Mapping
    pub params: HvsrParams,
    pub enable_hvtfa: bool,
    pub hvtfa_m: f64,
    
    // Threading
    pub is_processing: bool,
    pub progress_msg: String,
    pub progress_pct: f32,
    pub calc_rx: Option<Receiver<HvsrProgress>>,
    
    // Loaded components
    pub z_comp: Vec<f64>,
    pub n_comp: Vec<f64>,
    pub e_comp: Vec<f64>,
    pub z_comp_plot: Option<Vec<[f64; 2]>>,
    pub n_comp_plot: Option<Vec<[f64; 2]>>,
    pub e_comp_plot: Option<Vec<[f64; 2]>>,
    pub dt: f64,
    pub windows: Option<HvsrWindows>,
    pub selected_files: std::collections::HashSet<String>,
}

impl Default for HvsrDialogState {
    fn default() -> Self {
        Self {
            is_open: true,
            active_tab: ActiveTab::Explorer,
            open_tabs: Vec::new(),
            active_tab_index: 0,
            
            z_file: String::new(),
            n_file: String::new(),
            e_file: String::new(),
            explorer_dir: None,
            
            params: HvsrParams::default(),
            enable_hvtfa: false,
            hvtfa_m: 1.0,
            
            is_processing: false,
            progress_msg: "Ready.".to_string(),
            progress_pct: 0.0,
            calc_rx: None,
            
            z_comp: Vec::new(),
            n_comp: Vec::new(),
            e_comp: Vec::new(),
            z_comp_plot: None,
            n_comp_plot: None,
            e_comp_plot: None,
            dt: 0.01,
            windows: None,
            selected_files: std::collections::HashSet::new(),
        }
    }
}

pub fn show_hvsr_app(ctx: &egui::Context, state: &mut HvsrDialogState) {
    let is_open = state.is_open;

    show_activity_bar(ctx, state);
    show_sidebar(ctx, state);
    show_central_view(ctx, state);

    // Process channel messages
    if let Some(rx) = &state.calc_rx {
        while let Ok(msg) = rx.try_recv() {
            match msg {
                HvsrProgress::Progress(pct, s) => {
                    state.progress_msg = s;
                    state.progress_pct = pct;
                }
                HvsrProgress::Error(e) => {
                    state.progress_msg = format!("Error: {}", e);
                    state.progress_pct = 0.0;
                    state.is_processing = false;
                },
                HvsrProgress::Complete(res) => {
                    if let Some(w) = state.windows.as_mut() {
                        w.valid_windows_idx = res.final_valid_idx.clone();
                    }
                    
                    let vis_data = VisData {
                        hvsr_result: Some(res.clone()),
                        hvtfa_result: None,
                        hvsr_windows: state.windows.clone(),
                        z_comp_plot: state.z_comp_plot.clone(),
                        n_comp_plot: state.n_comp_plot.clone(),
                        e_comp_plot: state.e_comp_plot.clone(),
                        dt: state.dt,
                        active_curve_view: 0,
                        y_axis_log: false,
                    };
                    
                    let tab_name = "📈 HVSR Curve".to_string();
                    let tab = EditorTab::HvsrCurve(tab_name.clone(), Box::new(vis_data));
                    
                    if let Some(pos) = state.open_tabs.iter().position(|t| match t {
                        EditorTab::HvsrCurve(n, _) => n == &tab_name,
                        _ => false,
                    }) {
                        state.open_tabs[pos] = tab;
                        state.active_tab_index = pos;
                    } else {
                        state.open_tabs.push(tab);
                        state.active_tab_index = state.open_tabs.len() - 1;
                    }
                    
                    state.progress_msg = "Complete".to_string();
                    state.progress_pct = 1.0;
                    state.is_processing = false;
                }
                HvsrProgress::HvtfaComplete(res) => {
                    let vis_data = VisData {
                        hvsr_result: None,
                        hvtfa_result: Some(res),
                        hvsr_windows: state.windows.clone(),
                        z_comp_plot: state.z_comp_plot.clone(),
                        n_comp_plot: state.n_comp_plot.clone(),
                        e_comp_plot: state.e_comp_plot.clone(),
                        dt: state.dt,
                        active_curve_view: 0,
                        y_axis_log: false,
                    };
                    
                    let tab_name = "📊 HVTFA Curve".to_string();
                    let tab = EditorTab::HvsrCurve(tab_name.clone(), Box::new(vis_data));
                    
                    if let Some(pos) = state.open_tabs.iter().position(|t| match t {
                        EditorTab::HvsrCurve(n, _) => n == &tab_name,
                        _ => false,
                    }) {
                        state.open_tabs[pos] = tab;
                        state.active_tab_index = pos;
                    } else {
                        state.open_tabs.push(tab);
                        state.active_tab_index = state.open_tabs.len() - 1;
                    }
                    
                    state.progress_msg = "HVTFA Completed successfully".to_string();
                    state.progress_pct = 1.0;
                    state.is_processing = false;
                }
                    
                    // We shouldn't set calc_rx to None inside this loop since it might conflict with borrow checker 
                    // Actually, since we only set progress flags and process UI updates, it's fine. Wait, if rx is detached we might need to empty it.
                    // Instead of setting None here, we will handle it outside.
                }
            }
        }
        
    // Check if processing is false, but rx is still Some. If so, it means it completed or errored.
    if !state.is_processing && state.calc_rx.is_some() {
        state.calc_rx = None;
    }
    
    state.is_open = is_open;
}

fn show_activity_bar(ctx: &egui::Context, state: &mut HvsrDialogState) {
    egui::SidePanel::left("hvsr_activity_bar")
        .resizable(false)
        .exact_width(45.0)
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(10.0);
                
                let tab_size = egui::vec2(35.0, 35.0);
                
                let mut files_btn = egui::Button::new(egui::RichText::new("📁").size(20.0)).min_size(tab_size);
                if state.active_tab == ActiveTab::Explorer { files_btn = files_btn.fill(ui.visuals().selection.bg_fill); }
                if ui.add(files_btn).on_hover_text("Explorer").clicked() {
                    state.active_tab = ActiveTab::Explorer;
                }
                
                ui.add_space(5.0);
                
                let mut param_btn = egui::Button::new(egui::RichText::new("⚙").size(20.0)).min_size(tab_size);
                if state.active_tab == ActiveTab::Process { param_btn = param_btn.fill(ui.visuals().selection.bg_fill); }
                if ui.add(param_btn).on_hover_text("Process & Run").clicked() {
                    state.active_tab = ActiveTab::Process;
                }
            });
        });
}

fn show_sidebar(ctx: &egui::Context, state: &mut HvsrDialogState) {
    egui::SidePanel::left("hvsr_sidebar")
        .resizable(true)
        .min_width(280.0)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add_space(10.0);
                match state.active_tab {
                    ActiveTab::Explorer => show_explorer(ui, state),
                    ActiveTab::Process => show_process(ui, state),
                }
            });
        });
}

fn show_selected_files_panel(ui: &mut egui::Ui, state: &mut HvsrDialogState) {
    ui.group(|ui| {
        ui.label(egui::RichText::new("Selected Files:").strong());
        
        let mut has_any = false;
        if !state.z_file.is_empty() {
            ui.horizontal(|ui| {
                ui.label(format!("Z: {}", std::path::Path::new(&state.z_file).file_name().unwrap_or_default().to_string_lossy()));
                if ui.button("X").clicked() {
                    state.z_file.clear();
                    state.z_comp.clear();
                    state.z_comp_plot = None;
                }
            });
            has_any = true;
        }
        if !state.n_file.is_empty() {
            ui.horizontal(|ui| {
                ui.label(format!("N: {}", std::path::Path::new(&state.n_file).file_name().unwrap_or_default().to_string_lossy()));
                if ui.button("X").clicked() {
                    state.n_file.clear();
                    state.n_comp.clear();
                    state.n_comp_plot = None;
                }
            });
            has_any = true;
        }
        if !state.e_file.is_empty() {
            ui.horizontal(|ui| {
                ui.label(format!("E: {}", std::path::Path::new(&state.e_file).file_name().unwrap_or_default().to_string_lossy()));
                if ui.button("X").clicked() {
                    state.e_file.clear();
                    state.e_comp.clear();
                    state.e_comp_plot = None;
                }
            });
            has_any = true;
        }
        
        if !has_any {
            ui.label(egui::RichText::new("None").italics());
        }
    });
}

fn show_explorer(ui: &mut egui::Ui, state: &mut HvsrDialogState) {
    show_selected_files_panel(ui, state);
    ui.separator();
    ui.heading("EXPLORER");
    ui.separator();
    
    ui.horizontal(|ui| {
        if ui.button("📂 Open Folder").clicked() {
            if let Some(path) = rfd::FileDialog::new().pick_folder() {
                state.explorer_dir = Some(path);
                state.selected_files.clear();
            }
        }
        if !state.selected_files.is_empty() {
            if ui.button("▶ Load Selected Files").clicked() {
                state.z_file.clear();
                state.n_file.clear();
                state.e_file.clear();
                state.z_comp.clear();
                state.n_comp.clear();
                state.e_comp.clear();
                state.z_comp_plot = None;
                state.n_comp_plot = None;
                state.e_comp_plot = None;
                
                for path_str in &state.selected_files {
                    let path = std::path::PathBuf::from(path_str);
                    if let Ok(seismograms) = seisbox_core::core::parser::parse_seismic_file(&path) {
                        for seis in &seismograms {
                            let ch = &seis.channel.to_uppercase();
                            let filename = path.file_name().unwrap_or_default().to_string_lossy().to_uppercase();
                            
                            let mut data = seis.amplitude.clone();
                            crate::core::math_hvsr::detrend_signal(&mut data);
                            
                            state.dt = 1.0 / seis.sample_rate;
                            let plot_data = seisbox_core::ui::plot::decimate_for_plot(&seis.time, &data);
                            
                            let is_z = ch.ends_with('Z') || filename.ends_with("Z.SAC") || filename.contains("_Z");
                            let is_n = ch.ends_with('N') || ch.ends_with('1') || filename.ends_with("N.SAC") || filename.ends_with("1.SAC") || filename.contains("_N");
                            let is_e = ch.ends_with('E') || ch.ends_with('2') || filename.ends_with("E.SAC") || filename.ends_with("2.SAC") || filename.contains("_E");
                            
                            if is_z {
                                state.z_file = path_str.clone();
                                state.z_comp = data;
                                state.z_comp_plot = plot_data;
                            } else if is_n {
                                state.n_file = path_str.clone();
                                state.n_comp = data;
                                state.n_comp_plot = plot_data;
                            } else if is_e {
                                state.e_file = path_str.clone();
                                state.e_comp = data;
                                state.e_comp_plot = plot_data;
                            } else if seismograms.len() == 1 {
                                // Fallback for single component if we couldn't detect
                                if state.z_comp.is_empty() {
                                    state.z_file = path_str.clone();
                                    state.z_comp = data;
                                    state.z_comp_plot = plot_data;
                                } else if state.n_comp.is_empty() {
                                    state.n_file = path_str.clone();
                                    state.n_comp = data;
                                    state.n_comp_plot = plot_data;
                                } else if state.e_comp.is_empty() {
                                    state.e_file = path_str.clone();
                                    state.e_comp = data;
                                    state.e_comp_plot = plot_data;
                                }
                            }
                        }
                    }
                }
                
                // If data is successfully loaded, initialize the Seismogram tab
                if !state.z_comp.is_empty() || !state.n_comp.is_empty() || !state.e_comp.is_empty() {
                    let vis_data = VisData {
                        hvsr_result: None,
                        hvtfa_result: None,
                        hvsr_windows: None,
                        z_comp_plot: state.z_comp_plot.clone(),
                        n_comp_plot: state.n_comp_plot.clone(),
                        e_comp_plot: state.e_comp_plot.clone(),
                        dt: state.dt,
                        active_curve_view: 0,
                        y_axis_log: false,
                    };
                    
                    let tab_name = "📈 Seismogram".to_string();
                    let tab = EditorTab::Seismogram(tab_name.clone(), Box::new(vis_data));
                    
                    if let Some(pos) = state.open_tabs.iter().position(|t| match t {
                        EditorTab::Seismogram(n, _) => n == &tab_name,
                        _ => false,
                    }) {
                        state.open_tabs[pos] = tab;
                        state.active_tab_index = pos;
                    } else {
                        state.open_tabs.push(tab);
                        state.active_tab_index = state.open_tabs.len() - 1;
                    }
                }
            }
        }
    });
    
    let current_dir = state.explorer_dir.clone().unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")));
    ui.label(current_dir.display().to_string());
    ui.separator();
    
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&current_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                    let e = ext.to_lowercase();
                    if e == "sac" || e == "mseed" || e == "miniseed" {
                        files.push(path);
                    }
                }
            }
        }
    }
    
    files.sort();
    
    egui::ScrollArea::vertical().id_salt("hvsr_explorer_scroll").show(ui, |ui| {
        if files.is_empty() {
            ui.label(egui::RichText::new("No SAC/MSEED files found in directory.").italics());
        } else {
            for file in files {
                let filename = file.file_name().unwrap_or_default().to_string_lossy().to_string();
                let path_str = file.display().to_string();
                
                let is_selected = state.selected_files.contains(&path_str);
                
                let label = if is_selected {
                    egui::RichText::new(format!("📄 {}", filename)).strong()
                } else {
                    egui::RichText::new(format!("📄 {}", filename))
                };
                
                if ui.selectable_label(is_selected, label).clicked() {
                    if is_selected {
                        state.selected_files.remove(&path_str);
                    } else {
                        state.selected_files.insert(path_str);
                    }
                }
            }
        }
    });
}

fn show_process(ui: &mut egui::Ui, state: &mut HvsrDialogState) {
    show_selected_files_panel(ui, state);
    ui.separator();
    ui.heading("PROCESS & RUN");
    ui.separator();
    
    egui::ScrollArea::vertical().id_salt("hvsr_process_scroll").show(ui, |ui| {
        egui::CollapsingHeader::new("Windowing & Tapering").default_open(true).show(ui, |ui| {
            egui::Grid::new("hvsr_win_params").num_columns(2).show(ui, |ui| {
                ui.label("Window Length (s)");
                ui.add(egui::DragValue::new(&mut state.params.window_len_s).speed(1.0).range(10.0..=120.0));
                ui.end_row();
                
                ui.label("Overlap (%)");
                ui.add(egui::DragValue::new(&mut state.params.overlap_pct).speed(5.0).range(0.0..=90.0));
                ui.end_row();
            });
        });
        ui.add_space(5.0);

        egui::CollapsingHeader::new("Anti-Trigger (STA/LTA)").default_open(true).show(ui, |ui| {
            egui::Grid::new("hvsr_sta_params").num_columns(2).show(ui, |ui| {
                ui.label("STA Length (s)");
                ui.add(egui::DragValue::new(&mut state.params.sta_len_s).speed(0.1).range(0.1..=5.0));
                ui.end_row();
                
                ui.label("LTA Length (s)");
                ui.add(egui::DragValue::new(&mut state.params.lta_len_s).speed(1.0).range(10.0..=100.0));
                ui.end_row();
                
                ui.label("Threshold T1 (Lower)");
                ui.add(egui::DragValue::new(&mut state.params.t1).speed(0.05).range(0.0..=1.0));
                ui.end_row();
                
                ui.label("Threshold T2 (Upper)");
                ui.add(egui::DragValue::new(&mut state.params.t2).speed(0.1).range(1.5..=10.0));
                ui.end_row();
            });
        });
        ui.add_space(5.0);

        egui::CollapsingHeader::new("Smoothing & Frequency").default_open(true).show(ui, |ui| {
            egui::Grid::new("hvsr_smooth_params").num_columns(2).show(ui, |ui| {
                ui.label("Konno-Ohmachi (b)");
                ui.add(egui::DragValue::new(&mut state.params.b_value).speed(1.0).range(10.0..=100.0));
                ui.end_row();
                
                ui.label("Min Frequency (Hz)");
                ui.add(egui::DragValue::new(&mut state.params.freq_min).speed(0.1).range(0.1..=10.0));
                ui.end_row();
                
                ui.label("Max Frequency (Hz)");
                ui.add(egui::DragValue::new(&mut state.params.freq_max).speed(1.0).range(5.0..=50.0));
                ui.end_row();
                
                ui.label("Sample Count");
                ui.add(egui::DragValue::new(&mut state.params.freq_count).speed(10.0).range(50..=1000));
                ui.end_row();
                
                ui.label("Horiz Combine");
                egui::ComboBox::from_id_salt("horiz_combine")
                    .selected_text(match state.params.combine_method {
                        crate::core::math_hvsr::HorizontalCombineMethod::Geometric => "Geometric Mean",
                        crate::core::math_hvsr::HorizontalCombineMethod::Quadratic => "Quadratic Mean",
                        crate::core::math_hvsr::HorizontalCombineMethod::Arithmetic => "Arithmetic Mean",
                        crate::core::math_hvsr::HorizontalCombineMethod::Maximum => "Maximum",
                        crate::core::math_hvsr::HorizontalCombineMethod::Dfa => "DFA (Total Energy)",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut state.params.combine_method, crate::core::math_hvsr::HorizontalCombineMethod::Geometric, "Geometric Mean");
                        ui.selectable_value(&mut state.params.combine_method, crate::core::math_hvsr::HorizontalCombineMethod::Quadratic, "Quadratic Mean");
                        ui.selectable_value(&mut state.params.combine_method, crate::core::math_hvsr::HorizontalCombineMethod::Arithmetic, "Arithmetic Mean");
                        ui.selectable_value(&mut state.params.combine_method, crate::core::math_hvsr::HorizontalCombineMethod::Maximum, "Maximum");
                        ui.selectable_value(&mut state.params.combine_method, crate::core::math_hvsr::HorizontalCombineMethod::Dfa, "DFA (Total Energy)");
                    });
                ui.end_row();
            });
        });
        ui.add_space(5.0);
        
        egui::CollapsingHeader::new("f0 Filter (Iterative)").default_open(true).show(ui, |ui| {
            ui.checkbox(&mut state.params.enable_f0_filter, "Enable Iterative f0 Filter");
            if state.params.enable_f0_filter {
                egui::Grid::new("hvsr_f0_params").num_columns(2).show(ui, |ui| {
                    ui.label("Multiplier (n)");
                    ui.add(egui::DragValue::new(&mut state.params.f0_filter_n).speed(0.1).range(0.5..=5.0))
                        .on_hover_text("Number of standard deviations (e.g., 2.0)");
                    ui.end_row();
                    
                    ui.label("Max Iterations");
                    ui.add(egui::DragValue::new(&mut state.params.f0_filter_max_iter).speed(1).range(1..=200))
                        .on_hover_text("Maximum number of iterations for f0 filtering");
                    ui.end_row();
                });
            }
        });
        ui.add_space(5.0);
        
        egui::CollapsingHeader::new("HVTFA (Continuous Wavelet)").default_open(true).show(ui, |ui| {
            ui.checkbox(&mut state.enable_hvtfa, "Enable HVTFA");
            if state.enable_hvtfa {
                egui::Grid::new("hvsr_hvtfa_params").num_columns(2).show(ui, |ui| {
                    ui.label("Wavelet Width (m)");
                    ui.add(egui::DragValue::new(&mut state.hvtfa_m).speed(0.1).range(0.5..=5.0));
                    ui.end_row();
                });
            }
        });
        
        ui.add_space(20.0);
        
        if ui.button(egui::RichText::new("1. Preview Windows (STA/LTA)").size(16.0).strong()).clicked() {
            if !state.z_comp.is_empty() && !state.n_comp.is_empty() && !state.e_comp.is_empty() {
                match crate::core::math_hvsr::compute_windows(&state.z_comp, &state.n_comp, &state.e_comp, state.dt, &state.params) {
                    Ok(windows) => {
                        state.windows = Some(windows.clone());
                        state.progress_msg = "Windows computed".to_string();
                        
                        let vis_data = VisData {
                            hvsr_result: None,
                            hvtfa_result: None,
                            hvsr_windows: Some(windows),
                            z_comp_plot: state.z_comp_plot.clone(),
                            n_comp_plot: state.n_comp_plot.clone(),
                            e_comp_plot: state.e_comp_plot.clone(),
                            dt: state.dt,
                            active_curve_view: 0,
                        y_axis_log: false,
                        };
                        
                        let tab_name = "✂ Windows".to_string();
                        let tab = EditorTab::Windows(tab_name.clone(), Box::new(vis_data));
                        
                        if let Some(pos) = state.open_tabs.iter().position(|t| match t {
                            EditorTab::Windows(n, _) => n == &tab_name,
                            _ => false,
                        }) {
                            state.open_tabs[pos] = tab;
                            state.active_tab_index = pos;
                        } else {
                            state.open_tabs.push(tab);
                            state.active_tab_index = state.open_tabs.len() - 1;
                        }
                    }
                    Err(e) => {
                        state.progress_msg = format!("Error computing windows: {}", e);
                    }
                }
            } else {
                state.progress_msg = "Error: Missing components".to_string();
            }
        }
        
        ui.add_space(10.0);
        
        if ui.add_enabled(!state.is_processing, egui::Button::new(egui::RichText::new("2. Calculate HVSR").size(16.0).strong())).clicked() {
            if state.z_comp.is_empty() || state.n_comp.is_empty() || state.e_comp.is_empty() {
                state.progress_msg = "Error: Missing Z, N, E components".to_string();
            } else if state.windows.is_none() && !state.enable_hvtfa {
                state.progress_msg = "Error: Please preview windows first (for Standard HVSR)".to_string();
            } else {
                let (tx, rx) = std::sync::mpsc::channel();
                state.calc_rx = Some(rx);
                state.is_processing = true;
                state.progress_pct = 0.0;
                state.progress_msg = "Starting...".to_string();
                
                let z_c = state.z_comp.clone();
                let n_c = state.n_comp.clone();
                let e_c = state.e_comp.clone();
                let dt_c = state.dt;
                
                if state.enable_hvtfa {
                    let freq_min = state.params.freq_min;
                    let freq_max = state.params.freq_max;
                    let freq_count = state.params.freq_count;
                    let hvtfa_m = state.hvtfa_m;
                    
                    let (ptx, prx) = std::sync::mpsc::channel();
                    let tx_clone = tx.clone();
                    std::thread::spawn(move || {
                        while let Ok((pct, msg)) = prx.recv() {
                            let _ = tx_clone.send(HvsrProgress::Progress(pct, msg));
                        }
                    });

                    std::thread::spawn(move || {
                        let res = crate::core::math_hvtfa::run_hvtfa(
                            &z_c, &n_c, &e_c, 1.0 / dt_c,
                            freq_min, freq_max, freq_count, hvtfa_m,
                            Some(ptx)
                        );
                        let _ = tx.send(HvsrProgress::HvtfaComplete(res));
                    });
                } else {
                    let p_c = state.params.clone();
                    let w_c = state.windows.as_ref().unwrap().clone();
                    
                    std::thread::spawn(move || {
                        process_hvsr_pipeline(z_c, n_c, e_c, dt_c, p_c, w_c, tx);
                    });
                }
            }
        }
        
        ui.add_space(20.0);
        
        ui.vertical_centered_justified(|ui| {
            if ui.add_enabled(!state.is_processing, egui::Button::new(egui::RichText::new("3. Save Output (CSV)").size(16.0).strong())).clicked() {
                let mut result_found = false;
                if state.active_tab_index < state.open_tabs.len() {
                    if let EditorTab::HvsrCurve(_, vis_data) = &state.open_tabs[state.active_tab_index] {
                        if let Some(res) = &vis_data.hvsr_result {
                            result_found = true;
                            if let Some(path) = rfd::FileDialog::new()
                                .set_title("Save HVSR Result (CSV)")
                                .add_filter("CSV", &["csv"])
                                .save_file() 
                            {
                                let final_stats = if let Some(f0) = &res.f0_stats { f0 } else { &res.sta_lta_stats };
                                let mut peak_amp = -1.0;
                                let mut mean_peak_f0 = 0.0;
                                for (i, &val) in final_stats.mean_hvsr.iter().enumerate() {
                                    if val > peak_amp {
                                        peak_amp = val;
                                        mean_peak_f0 = res.freq[i];
                                    }
                                }
                                
                                let mut csv_content = String::new();
                                
                                // Data Summary
                                let z_name = std::path::Path::new(&state.z_file).file_name().unwrap_or_default().to_string_lossy();
                                let n_name = std::path::Path::new(&state.n_file).file_name().unwrap_or_default().to_string_lossy();
                                let e_name = std::path::Path::new(&state.e_file).file_name().unwrap_or_default().to_string_lossy();
                                let duration = state.z_comp.len() as f64 * state.dt;
                                let fs = if state.dt > 0.0 { 1.0 / state.dt } else { 0.0 };
                                let total_win = res.all_hvsr.len();
                                let acc_win = final_stats.valid_indices.len();
                                let rej_win = total_win - acc_win;
                                
                                csv_content.push_str("# --- SIGNAL DATA & WINDOWS ---\n");
                                csv_content.push_str(&format!("# Z Component File\t{}\n", z_name));
                                csv_content.push_str(&format!("# N Component File\t{}\n", n_name));
                                csv_content.push_str(&format!("# E Component File\t{}\n", e_name));
                                csv_content.push_str(&format!("# Sampling Rate (Hz)\t{:.2}\n", fs));
                                csv_content.push_str(&format!("# Signal Duration (s)\t{:.2}\n", duration));
                                csv_content.push_str(&format!("# Total Windows\t{}\n", total_win));
                                csv_content.push_str(&format!("# Accepted Windows\t{}\n", acc_win));
                                csv_content.push_str(&format!("# Rejected Windows\t{}\n", rej_win));
                                csv_content.push_str("# -----------------------------\n");
                                
                                // Print Parameters
                                csv_content.push_str("# --- PROCESSING PARAMETERS ---\n");
                                csv_content.push_str(&format!("# Window Length (s)\t{:.2}\n", state.params.window_len_s));
                                csv_content.push_str(&format!("# Overlap (%)\t{:.2}\n", state.params.overlap_pct));
                                csv_content.push_str(&format!("# STA Length (s)\t{:.2}\n", state.params.sta_len_s));
                                csv_content.push_str(&format!("# LTA Length (s)\t{:.2}\n", state.params.lta_len_s));
                                csv_content.push_str(&format!("# STA/LTA Threshold\t{:.2} - {:.2}\n", state.params.t1, state.params.t2));
                                csv_content.push_str(&format!("# Konno-Ohmachi b-value\t{:.2}\n", state.params.b_value));
                                csv_content.push_str(&format!("# Frequency Range (Hz)\t{:.4} - {:.4}\n", state.params.freq_min, state.params.freq_max));
                                csv_content.push_str(&format!("# Frequency Points\t{}\n", state.params.freq_count));
                                csv_content.push_str(&format!("# Combine Method\t{:?}\n", state.params.combine_method));
                                csv_content.push_str(&format!("# Enable f0 Filter\t{}\n", state.params.enable_f0_filter));
                                if state.params.enable_f0_filter {
                                    csv_content.push_str(&format!("# f0 Filter Bounds (StdDev)\t{:.2}\n", state.params.f0_filter_n));
                                    csv_content.push_str(&format!("# f0 Filter Max Iterations\t{}\n", state.params.f0_filter_max_iter));
                                }
                                csv_content.push_str("# -----------------------------\n");
                                
                                csv_content.push_str(&format!("# f0 from average\t{:.4} +/- {:.4}\n", final_stats.f0_mean, final_stats.f0_std));
                                csv_content.push_str(&format!("# Mean Curve Peak f0\t{:.4}\n", mean_peak_f0));
                                csv_content.push_str(&format!("# Mean Curve Peak A0\t{:.4}\n", peak_amp));
                                
                                if let Some(sesame) = &res.sesame_result {
                                    csv_content.push_str(&format!("# SESAME Reliable\t{}\n", sesame.is_reliable));
                                    csv_content.push_str(&format!("# SESAME Clear Peak\t{}\n", sesame.is_clear_peak));
                                    csv_content.push_str(&format!("# SESAME Rel_c1\t{}\n", sesame.reliability_c1));
                                    csv_content.push_str(&format!("# SESAME Rel_c2\t{}\n", sesame.reliability_c2));
                                    csv_content.push_str(&format!("# SESAME Rel_c3\t{}\n", sesame.reliability_c3));
                                    csv_content.push_str(&format!("# SESAME Clr_c1\t{}\n", sesame.clear_peak_c1));
                                    csv_content.push_str(&format!("# SESAME Clr_c2\t{}\n", sesame.clear_peak_c2));
                                    csv_content.push_str(&format!("# SESAME Clr_c3\t{}\n", sesame.clear_peak_c3));
                                    csv_content.push_str(&format!("# SESAME Clr_c4\t{}\n", sesame.clear_peak_c4));
                                    csv_content.push_str(&format!("# SESAME Clr_c5\t{}\n", sesame.clear_peak_c5));
                                    csv_content.push_str(&format!("# SESAME Clr_c6\t{}\n", sesame.clear_peak_c6));
                                }
                                
                                csv_content.push_str("Frequency,Mean_HV,Std_Dev_Plus,Std_Dev_Minus\n");
                                
                                for i in 0..res.freq.len() {
                                    csv_content.push_str(&format!("{:.4},{:.4},{:.4},{:.4}\n",
                                        res.freq[i], final_stats.mean_hvsr[i], final_stats.std_plus[i], final_stats.std_minus[i]));
                                }
                                let _ = std::fs::write(&path, csv_content);
                                state.progress_msg = format!("Saved to {}", path.file_name().unwrap_or_default().to_string_lossy());
                            }
                        } else if let Some(hvtfa) = &vis_data.hvtfa_result {
                            result_found = true;
                            if let Some(path) = rfd::FileDialog::new()
                                .set_title("Save HVTFA Result (CSV)")
                                .add_filter("CSV", &["csv"])
                                .save_file() 
                            {
                                let mut csv_content = String::new();
                                csv_content.push_str("Frequency,HV_Mode,HV_Std_Log\n");
                                for p in &hvtfa.picked_curve {
                                    csv_content.push_str(&format!("{:.4},{:.4},{:.4}\n", p.freq, p.hv_mode, p.hv_std_log));
                                }
                                let _ = std::fs::write(&path, csv_content);
                                state.progress_msg = format!("Saved to {}", path.file_name().unwrap_or_default().to_string_lossy());
                            }
                        }
                    }
                }
                if !result_found {
                    state.progress_msg = "Error: No HVSR result available to save.".to_string();
                }
            }
            
            if ui.add_enabled(!state.is_processing, egui::Button::new(egui::RichText::new("📁 Save Windows (CSV)").size(16.0).strong())).clicked() {
                let mut result_found = false;
                if state.active_tab_index < state.open_tabs.len() {
                    if let EditorTab::HvsrCurve(_, vis_data) = &state.open_tabs[state.active_tab_index] {
                        if let Some(res) = &vis_data.hvsr_result {
                            result_found = true;
                            if let Some(path) = rfd::FileDialog::new()
                                .set_title("Save Windows Data (CSV)")
                                .add_filter("CSV", &["csv"])
                                .save_file() 
                            {
                                let final_stats = if let Some(f0) = &res.f0_stats { f0 } else { &res.sta_lta_stats };
                                let mut csv_content = String::new();
                                
                                // Header: Frequency, Main Curves..., Window_0_Status, Window_1_Status, ...
                                csv_content.push_str("Frequency,Main_Curve_Raw,Main_Curve_STA_LTA");
                                if res.f0_stats.is_some() {
                                    csv_content.push_str(",Main_Curve_F0");
                                }
                                for j in 0..res.all_hvsr.len() {
                                    let status = if final_stats.valid_indices.contains(&j) { "Accepted" } else { "Rejected" };
                                    csv_content.push_str(&format!(",Window_{}_{}", j + 1, status));
                                }
                                csv_content.push('\n');
                                
                                // Data rows
                                for i in 0..res.freq.len() {
                                    csv_content.push_str(&format!("{:.4}", res.freq[i]));
                                    csv_content.push_str(&format!(",{:.4}", res.raw_stats.mean_hvsr[i]));
                                    csv_content.push_str(&format!(",{:.4}", res.sta_lta_stats.mean_hvsr[i]));
                                    if let Some(f0_stats) = &res.f0_stats {
                                        csv_content.push_str(&format!(",{:.4}", f0_stats.mean_hvsr[i]));
                                    }
                                    
                                    for j in 0..res.all_hvsr.len() {
                                        csv_content.push_str(&format!(",{:.4}", res.all_hvsr[j][i]));
                                    }
                                    csv_content.push('\n');
                                }
                                
                                let _ = std::fs::write(&path, csv_content);
                                state.progress_msg = format!("Windows saved to {}", path.file_name().unwrap_or_default().to_string_lossy());
                            }
                        }
                    }
                }
                if !result_found {
                    state.progress_msg = "Error: No HVSR result available to save windows.".to_string();
                }
            }

            if ui.add_enabled(!state.is_processing, egui::Button::new(egui::RichText::new("🖼 Save Figure (PNG)").size(16.0).strong())).clicked() {
                let mut result_found = false;
                if state.active_tab_index < state.open_tabs.len() {
                    if let EditorTab::HvsrCurve(_, vis_data) = &state.open_tabs[state.active_tab_index] {
                        if let Some(res) = &vis_data.hvsr_result {
                            result_found = true;
                            if let Some(path) = rfd::FileDialog::new()
                                .set_title("Save HVSR Curve Figure (PNG)")
                                .add_filter("PNG Image", &["png"])
                                .save_file() 
                            {
                                let final_stats = if let Some(f0) = &res.f0_stats { f0 } else { &res.sta_lta_stats };
                                let is_dfa = false; // We could infer this from params if needed, but for now we assume standard HVSR
                                
                                match crate::io::plotters_export::export_hvsr_curve(
                                    &path,
                                    &res.freq,
                                    final_stats,
                                    &res.all_hvsr,
                                    &final_stats.valid_indices,
                                    is_dfa,
                                ) {
                                    Ok(_) => {
                                        state.progress_msg = format!("Figure saved to {}", path.file_name().unwrap_or_default().to_string_lossy());
                                    }
                                    Err(e) => {
                                        state.progress_msg = format!("Error saving figure: {}", e);
                                    }
                                }
                            }
                        } else if let Some(hvtfa) = &vis_data.hvtfa_result {
                            result_found = true;
                            if let Some(path) = rfd::FileDialog::new()
                                .set_title("Save HVTFA Scatter Figure (PNG)")
                                .add_filter("PNG Image", &["png"])
                                .save_file() 
                            {
                                let mut f_min = 20.0;
                                let mut f_max = 0.1;
                                for p in &hvtfa.scatter_points {
                                    if p.freq < f_min { f_min = p.freq; }
                                    if p.freq > f_max { f_max = p.freq; }
                                }
                                if f_min >= f_max {
                                    f_min = 0.1;
                                    f_max = 20.0;
                                }
                                
                                match crate::io::plotters_export::export_hvtfa_plot(
                                    &path,
                                    f_min,
                                    f_max,
                                    &hvtfa.scatter_points,
                                    &hvtfa.picked_curve,
                                ) {
                                    Ok(_) => {
                                        state.progress_msg = format!("Figure saved to {}", path.file_name().unwrap_or_default().to_string_lossy());
                                    }
                                    Err(e) => {
                                        state.progress_msg = format!("Error saving figure: {}", e);
                                    }
                                }
                            }
                        }
                    } else if let EditorTab::Windows(_, _) = &state.open_tabs[state.active_tab_index] {
                        // User is in Windows tab, but we don't have easy access to the full data in state right here
                        // We would need the full z, n, e data to export windows plot.
                        // Let's just fall back to saying no result to save.
                        state.progress_msg = "Save Figure currently only supports HVSR / HVTFA Curve tabs.".to_string();
                        result_found = true;
                    }
                }
                if !result_found {
                    state.progress_msg = "Error: No plot available to save.".to_string();
                }
            }
        });
        
        ui.add_space(20.0);
        
        if state.is_processing {
            ui.vertical(|ui| {
                ui.label(egui::RichText::new(&state.progress_msg).strong());
                ui.add(egui::ProgressBar::new(state.progress_pct).show_percentage());
            });
        } else {
            if !state.progress_msg.is_empty() {
                ui.label(egui::RichText::new(&state.progress_msg).strong());
            }
        }
    });
}

fn show_central_view(ctx: &egui::Context, state: &mut HvsrDialogState) {
    egui::CentralPanel::default().show(ctx, |ui| {
        if state.open_tabs.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label(egui::RichText::new("No active tabs.

Select files from the Explorer or run Process.").italics());
            });
            return;
        }

        ui.horizontal(|ui| {
            ui.style_mut().spacing.item_spacing = egui::vec2(2.0, 0.0);
            
            let mut tab_to_close = None;
            
            for (idx, tab) in state.open_tabs.iter().enumerate() {
                let (title, is_dirty) = match tab {
                    EditorTab::Seismogram(name, _) => (name.clone(), false),
                    EditorTab::Windows(name, _) => (name.clone(), false),
                    EditorTab::HvsrCurve(name, _) => (name.clone(), false),
                };
                
                let is_active = state.active_tab_index == idx;
                
                let tab_fill = if is_active {
                    ui.visuals().selection.bg_fill
                } else {
                    ui.visuals().widgets.noninteractive.bg_fill
                };
                
                let mut title_text = egui::RichText::new(title).size(14.0);
                if is_dirty { title_text = title_text.strong(); }
                if is_active { title_text = title_text.color(ui.visuals().selection.stroke.color); }
                
                egui::Frame::NONE
                    .fill(tab_fill)
                    .corner_radius(4.0_f32)
                    .inner_margin(8.0_f32)
                    .show(ui, |ui| {
                        if ui.selectable_label(is_active, title_text).clicked() {
                            state.active_tab_index = idx;
                        }
                        ui.add_space(4.0);
                        if ui.button(egui::RichText::new("x").size(12.0)).clicked() {
                            tab_to_close = Some(idx);
                        }
                    });
            }
            
            if let Some(idx) = tab_to_close {
                state.open_tabs.remove(idx);
                if state.active_tab_index >= state.open_tabs.len() && !state.open_tabs.is_empty() {
                    state.active_tab_index = state.open_tabs.len() - 1;
                }
            }
        });

        ui.separator();

        if state.active_tab_index < state.open_tabs.len() {
            let active_tab = &mut state.open_tabs[state.active_tab_index];
            match active_tab {
                EditorTab::Seismogram(_name, vis_data) => {
                    show_seismogram_tab(ui, vis_data);
                }
                EditorTab::Windows(_name, vis_data) => {
                    let mut new_windows = None;
                    show_windows_tab(ui, vis_data, &mut new_windows);
                    if let Some(nw) = new_windows {
                        state.windows = Some(nw);
                    }
                }
                EditorTab::HvsrCurve(_name, vis_data) => {
                    show_hvsr_curve_tab(ui, vis_data);
                }
            }
        }
    });
}

fn show_seismogram_tab(ui: &mut egui::Ui, vis_data: &mut VisData) {
    let mut plot_configs = Vec::new();
    if vis_data.z_comp_plot.is_some() { plot_configs.push(("Z Component", &vis_data.z_comp_plot, egui::Color32::from_rgba_unmultiplied(100, 100, 100, 150))); }
    if vis_data.n_comp_plot.is_some() { plot_configs.push(("N Component", &vis_data.n_comp_plot, egui::Color32::from_rgba_unmultiplied(200, 50, 50, 100))); }
    if vis_data.e_comp_plot.is_some() { plot_configs.push(("E Component", &vis_data.e_comp_plot, egui::Color32::from_rgba_unmultiplied(50, 50, 200, 100))); }
    
    if plot_configs.is_empty() { return; }
    
    let plot_count = plot_configs.len() as f32;
    let overhead = 45.0; // Heading + spacing + margins
    let h2 = (ui.available_height() - (overhead * plot_count) - 30.0) / plot_count;
    
    egui::ScrollArea::vertical().id_salt("seismogram_scroll").show(ui, |ui| {
        for (i, (title, comp, color)) in plot_configs.iter().enumerate() {
            ui.heading(*title);
            Plot::new(format!("seismogram_plot_{}", i))
                .height(h2.max(150.0))
                .y_axis_formatter(|mark, _| format!("{:e}", mark.value))
                .allow_zoom(true)
                .allow_drag(true)
                .show(ui, |plot_ui| {
                    if let Some(cached) = comp {
                        plot_ui.line(Line::new(PlotPoints::new(cached.clone())).color(*color));
                    }
                });
            ui.add_space(5.0);
        }
    });
}

fn show_windows_tab(ui: &mut egui::Ui, vis_data: &mut VisData, new_windows: &mut Option<HvsrWindows>) {
    let mut plot_configs = Vec::new();
    if vis_data.z_comp_plot.is_some() { plot_configs.push(("Z Component", &vis_data.z_comp_plot, egui::Color32::from_rgba_unmultiplied(100, 100, 100, 150))); }
    if vis_data.n_comp_plot.is_some() { plot_configs.push(("N Component", &vis_data.n_comp_plot, egui::Color32::from_rgba_unmultiplied(200, 50, 50, 100))); }
    if vis_data.e_comp_plot.is_some() { plot_configs.push(("E Component", &vis_data.e_comp_plot, egui::Color32::from_rgba_unmultiplied(50, 50, 200, 100))); }
    
    if plot_configs.is_empty() { return; }
    
    let plot_count = plot_configs.len() as f32;
    let overhead = 50.0; // 25 for heading + 25 for padding and margins
    let h2 = (ui.available_height() - (overhead * plot_count) - 30.0) / plot_count;
    
    let mut clicked_x = None;
    
    egui::ScrollArea::vertical().id_salt("windows_scroll").show(ui, |ui| {
        for (i, (title, comp, color)) in plot_configs.iter().enumerate() {
            ui.heading(*title);
            
            let mut max_amp = 0.0_f64;
            if let Some(cached) = comp {
                for &[_, a] in cached { if a.abs() > max_amp { max_amp = a.abs(); } }
            }
            if max_amp == 0.0 { max_amp = 1.0; }
            let y_bound = 1.2;
            
            let plot_response = Plot::new(format!("windows_plot_{}", i))
                .height(h2.max(150.0))
                .y_axis_formatter(|mark, _| format!("{:.2}", mark.value))
                .allow_zoom(true)
                .allow_drag(true)
                .show(ui, |plot_ui| {
                    if let Some(cached) = comp {
                        let normalized: Vec<[f64; 2]> = cached.iter().map(|&[t, a]| [t, a / max_amp]).collect();
                        plot_ui.line(Line::new(PlotPoints::new(normalized)).color(*color));
                    }
                    
                    if let Some(windows) = &vis_data.hvsr_windows {
                        let win_len = windows.window_len_s;
                        for i in 0..windows.window_starts_s.len() {
                            let start = windows.window_starts_s[i];
                            let end = start + win_len;
                            
                            let poly_pts = vec![
                                [start, -y_bound],
                                [end, -y_bound],
                                [end, y_bound],
                                [start, y_bound],
                            ];
                            
                            let is_valid = windows.valid_windows_idx[i];
                            let p_color = if is_valid {
                                egui::Color32::from_rgba_unmultiplied(0, 255, 0, 40)
                            } else {
                                egui::Color32::from_rgba_unmultiplied(255, 0, 0, 20)
                            };
                            
                            plot_ui.polygon(Polygon::new(PlotPoints::new(poly_pts)).fill_color(p_color));
                        }
                    }
                });
                
            if plot_response.response.clicked() {
                if let Some(pointer) = plot_response.response.hover_pos() {
                    let plot_pointer = plot_response.transform.value_from_position(pointer);
                    clicked_x = Some(plot_pointer.x);
                }
            }
            ui.add_space(5.0);
        }
    });
    
    if let Some(x) = clicked_x {
        if let Some(windows) = &mut vis_data.hvsr_windows {
            let win_len = windows.window_len_s;
            for i in 0..windows.window_starts_s.len() {
                let start = windows.window_starts_s[i];
                let end = start + win_len;
                if x >= start && x <= end {
                    windows.valid_windows_idx[i] = !windows.valid_windows_idx[i];
                }
            }
            *new_windows = Some(windows.clone());
        }
    }
}

fn show_hvsr_curve_tab(ui: &mut egui::Ui, vis_data: &mut VisData) {
    if let Some(res) = &vis_data.hvsr_result {
        let mut view_options = vec![
            "Stage 1: Raw (All Windows)".to_string(),
            "Stage 2: STA/LTA Filtered".to_string(),
        ];
        if res.f0_stats.is_some() {
            view_options.push("Stage 3: f0 Filtered".to_string());
        }
        if res.sesame_result.is_some() {
            view_options.push("SESAME (2004) Peak Evaluation".to_string());
        }
        
        if vis_data.active_curve_view >= view_options.len() {
            vis_data.active_curve_view = view_options.len().saturating_sub(1);
        }
        
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Select View:").strong());
            egui::ComboBox::from_id_source("hvsr_view_combo")
                .selected_text(&view_options[vis_data.active_curve_view])
                .show_ui(ui, |ui| {
                    for (i, option) in view_options.iter().enumerate() {
                        ui.selectable_value(&mut vis_data.active_curve_view, i, option);
                    }
                });
        });
        ui.separator();
        
        let is_sesame = vis_data.active_curve_view == view_options.len() - 1 && res.sesame_result.is_some();
        if is_sesame {
            if let Some(sesame) = &res.sesame_result {
                ui.heading("SESAME (2004) Peak Evaluation");
                ui.vertical(|ui| {
                    ui.group(|ui| {
                        ui.label(egui::RichText::new("Reliability Criteria").strong());
                        ui.label(format!("Criteria i   (f0 > 10/Lw): {}", if sesame.reliability_c1 { "✅ Pass" } else { "❌ Fail" }));
                        ui.label(format!("Criteria ii  (nc > 200): {}", if sesame.reliability_c2 { "✅ Pass" } else { "❌ Fail" }));
                        ui.label(format!("Criteria iii (σA(f) limit): {}", if sesame.reliability_c3 { "✅ Pass" } else { "❌ Fail" }));
                        ui.add_space(5.0);
                        let rel_text = if sesame.is_reliable { egui::RichText::new("RELIABLE").color(egui::Color32::from_rgb(0, 180, 0)).strong() } else { egui::RichText::new("NOT RELIABLE").color(egui::Color32::RED).strong() };
                        ui.label(rel_text);
                    });
                    
                    ui.group(|ui| {
                        ui.label(egui::RichText::new("Clarity Criteria").strong());
                        ui.label(format!("Criteria i   (f in [f0/4, f0] | A < A0/2): {}", if sesame.clear_peak_c1 { "✅ Pass" } else { "❌ Fail" }));
                        ui.label(format!("Criteria ii  (f in [f0, 4f0] | A < A0/2): {}", if sesame.clear_peak_c2 { "✅ Pass" } else { "❌ Fail" }));
                        ui.label(format!("Criteria iii (A0 > 2.0): {}", if sesame.clear_peak_c3 { "✅ Pass" } else { "❌ Fail" }));
                        ui.label(format!("Criteria iv  (f+, f- limits): {}", if sesame.clear_peak_c4 { "✅ Pass" } else { "❌ Fail" }));
                        ui.label(format!("Criteria v   (σf < ε(f0)): {}", if sesame.clear_peak_c5 { "✅ Pass" } else { "❌ Fail" }));
                        ui.label(format!("Criteria vi  (σA(f0) < θ(f0)): {}", if sesame.clear_peak_c6 { "✅ Pass" } else { "❌ Fail" }));
                        ui.add_space(5.0);
                        let clr_text = if sesame.is_clear_peak { egui::RichText::new("CLEAR PEAK (≥ 5/6 Pass)").color(egui::Color32::from_rgb(0, 180, 0)).strong() } else { egui::RichText::new("NOT CLEAR PEAK").color(egui::Color32::RED).strong() };
                        ui.label(clr_text);
                    });
                });
            }
            return;
        }
        
        let (title, stats) = match vis_data.active_curve_view {
            0 => ("Stage 1: Raw (All Windows)", &res.raw_stats),
            1 => ("Stage 2: STA/LTA Filtered", &res.sta_lta_stats),
            2 if res.f0_stats.is_some() => ("Stage 3: f0 Filtered", res.f0_stats.as_ref().unwrap()),
            _ => ("Stage 1: Raw (All Windows)", &res.raw_stats),
        };
        
        ui.horizontal(|ui| {
            ui.heading(title);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.checkbox(&mut vis_data.y_axis_log, "Logarithmic Y-Axis");
            });
        });
        ui.label(format!("Windows used: {}", stats.valid_indices.len()));
        
        let h2 = ui.available_height() - 20.0;
        let window_color = ui.visuals().text_color().linear_multiply(0.35);
        let rejected_color = egui::Color32::from_rgb(200, 50, 50).linear_multiply(0.15);
        
        let y_log = vis_data.y_axis_log;
        let mut plot = Plot::new(format!("hvsr_freq_plot_{}", vis_data.active_curve_view))
            .height(h2.max(250.0))
            .x_axis_formatter(|mark, _| format!("{:.2} Hz", 10_f64.powf(mark.value)))
            .allow_scroll(false)
            .allow_zoom(false)
            .allow_drag(true);
            
        if y_log {
            // using log_grid_spacer(10) generates marks for powers of 10 and intermediate values (2, 3, etc.)
            // The formatter will take the log10 value and format 10^x.
            plot = plot.y_axis_formatter(|mark, _| {
                let v = 10_f64.powf(mark.value);
                if v >= 10.0 {
                    format!("{:.0}", v)
                } else if v >= 1.0 {
                    format!("{:.1}", v)
                } else {
                    format!("{:.2}", v)
                }
            }).y_grid_spacer(egui_plot::log_grid_spacer(10));
        } else {
            plot = plot.y_axis_formatter(|mark, _| format!("{:.1}", mark.value));
        }
        
        plot.show(ui, |plot_ui| {
                if !res.freq.is_empty() && !stats.valid_indices.is_empty() {
                    let mut mean_pts = Vec::new();
                    let mut upper_pts = Vec::new();
                    let mut lower_pts = Vec::new();
                    
                    let mut max_hvsr = -1.0;
                    let mut peak_freq_log = 0.0;
                    let mut peak_freq = 0.0;
                    
                    for j in 0..res.freq.len() {
                        let f = res.freq[j];
                        if f >= 0.1 && f <= res.freq.last().copied().unwrap_or(50.0) {
                            let x = f.log10(); 
                            let mut h = stats.mean_hvsr[j];
                            let mut p = stats.std_plus[j];
                            let mut m = stats.std_minus[j];
                            
                            // Track real linear max_hvsr
                            if stats.mean_hvsr[j] > max_hvsr {
                                max_hvsr = stats.mean_hvsr[j];
                                peak_freq_log = x;
                                peak_freq = f;
                            }
                            
                            if y_log {
                                h = h.max(1e-6).log10();
                                p = p.max(1e-6).log10();
                                m = m.max(1e-6).log10();
                            }
                            
                            mean_pts.push([x, h]);
                            upper_pts.push([x, p]);
                            lower_pts.push([x, m]);
                        }
                    }
                    
                    for window_idx in 0..res.all_hvsr.len() {
                        if !stats.valid_indices.contains(&window_idx) {
                            let hvsr_curve = &res.all_hvsr[window_idx];
                            let mut win_pts = Vec::new();
                            for j in 0..res.freq.len() {
                                let f = res.freq[j];
                                if f >= 0.1 && f <= res.freq.last().copied().unwrap_or(50.0) {
                                    let x = f.log10();
                                    let mut h = hvsr_curve[j];
                                    if y_log { h = h.max(1e-6).log10(); }
                                    win_pts.push([x, h]);
                                }
                            }
                            if !win_pts.is_empty() {
                                plot_ui.line(Line::new(PlotPoints::new(win_pts))
                                    .color(rejected_color)
                                    .width(1.0_f32));
                            }
                        }
                    }
                    
                    for &window_idx in &stats.valid_indices {
                        let hvsr_curve = &res.all_hvsr[window_idx];
                        let mut win_pts = Vec::new();
                        
                        let mut win_max = -1.0;
                        let mut win_max_log = 0.0;
                        
                        for j in 0..res.freq.len() {
                            let f = res.freq[j];
                            if f >= 0.1 && f <= res.freq.last().copied().unwrap_or(50.0) {
                                let x = f.log10();
                                let mut h = hvsr_curve[j];
                                if h > win_max {
                                    win_max = h;
                                    win_max_log = x;
                                }
                                if y_log { h = h.max(1e-6).log10(); }
                                win_pts.push([x, h]);
                            }
                        }
                        
                        if !win_pts.is_empty() {
                            plot_ui.line(Line::new(PlotPoints::new(win_pts.clone()))
                                .color(window_color)
                                .width(1.0_f32));
                                
                            if win_max > 0.0 {
                                let mut win_max_y = win_max;
                                if y_log { win_max_y = win_max_y.max(1e-6).log10(); }
                                plot_ui.points(egui_plot::Points::new(PlotPoints::new(vec![[win_max_log, win_max_y]]))
                                    .shape(egui_plot::MarkerShape::Circle)
                                    .radius(3.0_f32)
                                    .color(window_color));
                            }
                        }
                    }
                    
                    if !mean_pts.is_empty() {
                        plot_ui.line(Line::new(PlotPoints::new(upper_pts))
                            .color(egui::Color32::from_rgb(100, 100, 255))
                            .style(egui_plot::LineStyle::Dashed { length: 4.0 })
                            .width(1.5_f32));
                            
                        plot_ui.line(Line::new(PlotPoints::new(lower_pts))
                            .color(egui::Color32::from_rgb(100, 100, 255))
                            .style(egui_plot::LineStyle::Dashed { length: 4.0 })
                            .width(1.5_f32));
                            
                        plot_ui.line(Line::new(PlotPoints::new(mean_pts))
                            .color(egui::Color32::BLUE)
                            .width(2.5_f32));
                            
                        if max_hvsr > 0.0 {
                            let mut max_hvsr_y = max_hvsr;
                            if y_log { max_hvsr_y = max_hvsr_y.max(1e-6).log10(); }
                            
                            plot_ui.points(egui_plot::Points::new(PlotPoints::new(vec![[peak_freq_log, max_hvsr_y]]))
                                .shape(egui_plot::MarkerShape::Diamond)
                                .radius(6.0_f32)
                                .color(egui::Color32::RED));
                                
                            plot_ui.vline(VLine::new(peak_freq_log)
                                .color(egui::Color32::RED)
                                .style(egui_plot::LineStyle::Dashed { length: 5.0 }));
                            
                            let mut text_y = if y_log { max_hvsr_y + 0.15 } else { max_hvsr_y + 0.5 };
                            plot_ui.text(Text::new(
                                PlotPoint::new(peak_freq_log, text_y),
                                format!("f0 = {:.2} Hz, A0 = {:.2}", peak_freq, max_hvsr)
                            ).color(egui::Color32::RED));
                        }
                        
                        if stats.f0_mean > 0.0 {
                            let f0_log = stats.f0_mean.log10();
                            let f0_plus = (stats.f0_mean + stats.f0_std).log10();
                            let f0_minus = (stats.f0_mean - stats.f0_std).max(0.001).log10();
                            
                            plot_ui.vline(VLine::new(f0_log)
                                .color(egui::Color32::from_rgb(0, 180, 0))
                                .width(2.0_f32));
                                
                            plot_ui.vline(VLine::new(f0_plus)
                                .color(egui::Color32::from_rgb(0, 180, 0))
                                .style(egui_plot::LineStyle::Dashed { length: 4.0 })
                                .width(1.0_f32));
                                
                            plot_ui.vline(VLine::new(f0_minus)
                                .color(egui::Color32::from_rgb(0, 180, 0))
                                .style(egui_plot::LineStyle::Dashed { length: 4.0 })
                                .width(1.0_f32));
                                
                            let max_hvsr_y = if y_log { max_hvsr.max(1e-6).log10() } else { max_hvsr };
                            let text_y2 = if y_log { max_hvsr_y + 0.25 } else { max_hvsr_y + 1.2 };
                            
                            plot_ui.text(Text::new(
                                PlotPoint::new(f0_log, text_y2),
                                format!("μ(f0) = {:.2} ± {:.2} Hz", stats.f0_mean, stats.f0_std)
                            ).color(egui::Color32::from_rgb(0, 180, 0)));
                        }
                    }
                }
            });

    } else if let Some(hvtfa) = &vis_data.hvtfa_result {
        let h2 = ui.available_height() - 80.0;
        
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("HVTFA Result");
            
            ui.label("Scatter Plot (Freq vs HV) overlaid with Picked Mode Curve");
            Plot::new("hvtfa_plot")
                .height(h2.max(400.0))
                .x_axis_formatter(|mark, _| format!("{:.2} Hz", 10_f64.powf(mark.value)))
                .y_axis_formatter(|mark, _| format!("{:.1}", mark.value))
                .allow_scroll(false)
                .allow_zoom(false)
                .allow_drag(true)
                .show(ui, |plot_ui| {
                    // Plot Scatter points (Freq vs HV_pos)
                    if !hvtfa.scatter_points.is_empty() {
                        // Subsample if too many points to prevent UI lag
                        let step = (hvtfa.scatter_points.len() / 5000).max(1);
                        let mut scatter_coords = Vec::new();
                        for (i, p) in hvtfa.scatter_points.iter().enumerate() {
                            if i % step == 0 && p.freq >= 0.1 && p.freq <= 50.0 {
                                scatter_coords.push([p.freq.log10(), p.hv_pos]);
                            }
                        }
                        
                        plot_ui.points(egui_plot::Points::new(PlotPoints::new(scatter_coords))
                            .color(egui::Color32::from_black_alpha(20))
                            .radius(1.5_f32)
                        );
                    }
                    
                    // Plot Picked Curve (Freq vs Mode)
                    if !hvtfa.picked_curve.is_empty() {
                        let mut mode_pts = Vec::new();
                        let mut max_mode = -1.0;
                        let mut peak_freq_log = 0.0;
                        let mut peak_freq = 0.0;
                        
                        for p in &hvtfa.picked_curve {
                            if p.freq >= 0.1 && p.freq <= 50.0 {
                                let x = p.freq.log10();
                                mode_pts.push([x, p.hv_mode]);
                                
                                if p.hv_mode > max_mode {
                                    max_mode = p.hv_mode;
                                    peak_freq_log = x;
                                    peak_freq = p.freq;
                                }
                            }
                        }
                        
                        if !mode_pts.is_empty() {
                            plot_ui.line(Line::new(PlotPoints::new(mode_pts))
                                .color(egui::Color32::from_rgb(0, 180, 0))
                                .width(3.0_f32));
                                
                            plot_ui.points(egui_plot::Points::new(PlotPoints::new(vec![[peak_freq_log, max_mode]]))
                                .shape(egui_plot::MarkerShape::Diamond)
                                .radius(8.0_f32)
                                .color(egui::Color32::RED));
                                
                            plot_ui.vline(VLine::new(peak_freq_log)
                                .color(egui::Color32::RED)
                                .style(egui_plot::LineStyle::Dashed { length: 5.0 }));
                            
                            plot_ui.text(Text::new(
                                PlotPoint::new(peak_freq_log, max_mode + 0.5),
                                format!("f0 = {:.2} Hz, A0 = {:.2}", peak_freq, max_mode)
                            ).color(egui::Color32::RED));
                        }
                    }
                });
        });
    }
}
