use eframe::egui;
use egui_plot::{Plot, PlotPoints, Points, Text, PlotPoint, PlotImage};
use std::sync::{Arc, Mutex};
use std::thread;
use std::path::PathBuf;

use crate::ui::inp_generator_dialog::{show_inp_generator_panel, InpGeneratorState};

use crate::core::cfs_parser::{CoulombInput, BatchInput, open_input_file_cui, open_batch_file};
use crate::core::cfs_runner::{calculate_deformation, calculate_coulomb_grid, calculate_coulomb_batch, calculate_coulomb_grid_oof, calculate_coulomb_cross_section, DeformationResult, CoulombResult, OptTarget, CrossSectionResult};
use crate::core::cfs_math::{build_regional_stress_tensor, regional_tensor_to_voigt};
use crate::core::cfs_io::write_cross_section_csv;
use crate::core::cfs_io::{write_coulomb_csv, write_deformation_csv};
use std::sync::mpsc::{channel, Receiver};

pub enum CfsThreadResult {
    Deformation(Vec<DeformationResult>),
    CoulombGrid(Vec<CoulombResult>),
    BatchReceiver(Vec<CoulombResult>),
    CrossSection(Vec<CrossSectionResult>),
}

#[derive(PartialEq, Clone)]
pub enum CfsMode {
    Deformation,
    CoulombGrid,
    BatchReceiver,
    CrossSection,
}

#[derive(PartialEq, Clone, Copy)]
pub enum ActiveTab {
    Explorer,
    Process,
    InpGenerator,
}

#[derive(PartialEq, Clone, Copy)]
pub enum MapPlotStyle {
    Scatter,
    Imshow,
    Contourf,
}

#[derive(Clone)]
pub struct VisData {
    pub mode: CfsMode,
    pub def_results: Option<Vec<DeformationResult>>,
    pub cfs_results: Option<Vec<CoulombResult>>,
    pub cs_results: Option<Vec<CrossSectionResult>>,
        pub parsed_input: Option<CoulombInput>,
    pub cached_texture: Option<egui::TextureHandle>,
    pub cache_params: Option<(MapPlotStyle, f64, f64, usize, bool)>,
    pub cached_bounds: Option<([f64; 2], [f64; 2])>,
    pub selected_depth_filter: String,
}

#[derive(Clone)]
pub enum EditorTab {
    Visualization(String, Box<VisData>),
    Ascii(std::path::PathBuf, String, bool), // path, content, is_dirty
    
}

impl PartialEq for EditorTab {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Visualization(n1, _), Self::Visualization(n2, _)) => n1 == n2,
            (Self::Ascii(p1, _, _), Self::Ascii(p2, _, _)) => p1 == p2,
            _ => false,
        }
    }
}

pub struct CfsDialogState {
    pub is_open: bool,
    pub active_tab: ActiveTab,
    pub input_path: String,
    pub batch_path: String,
    pub explorer_dir: Option<PathBuf>,
    
    // Editor Tabs
    pub open_tabs: Vec<EditorTab>,
    pub active_tab_index: usize,
    
    pub mode: CfsMode,
    
    pub receiver_strike: f64,
    pub receiver_dip: f64,
    pub receiver_rake: f64,
    
    // Grid Overrides
    pub override_grid: bool,
    pub override_grid_lonlat: bool,
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
    pub validation_popup_msg: Option<String>,
    pub inp_generator: InpGeneratorState,
    
    pub use_source_mech: bool,
    
    // Cross Section
    pub cs_start_lon: f64,
    pub cs_finish_lon: f64,
    pub cs_start_lat: f64,
    pub cs_finish_lat: f64,
    pub cs_dist_inc: f64,
    pub cs_results: Option<Vec<CrossSectionResult>>,
    
    // OOF
    pub oof_enabled: bool,
    pub regional_mag: f64,
    pub regional_azimuth: f64,
    pub regional_plunge: f64,
    
    // Physical Overrides
    pub override_physics: bool,
    pub fric: f64,
    pub poisson: f64,
    pub young: f64,
    
    // Plotting Settings
    pub plot_vmin: String,
    pub plot_vmax: String,
    pub plot_title: String,
    pub plot_title_size: u32,
    pub plot_label_size: u32,
    pub plot_tick_size: u32,
    pub plot_cbar_label_size: u32,
    pub plot_cbar_tick_size: u32,
    pub plot_contour_steps: u32,
    pub plot_aspect_equal: bool,
    pub plot_fault_color: String,
        pub plot_fault_width: u32,
    pub plot_style: MapPlotStyle,
    pub plot_cs_track: bool,
    pub plot_use_lonlat: bool,
    pub tiff_export_component: String,
    pub overlay_cs_files: Vec<String>,
}

impl Default for CfsDialogState {
    fn default() -> Self {
        Self {
            is_open: false,
            active_tab: ActiveTab::Explorer,
            open_tabs: vec![],
            active_tab_index: 0,
            explorer_dir: None,
            input_path: String::new(),
            batch_path: String::new(),
            mode: CfsMode::CoulombGrid,
            receiver_strike: 30.0,
            receiver_dip: 90.0,
            receiver_rake: 180.0,
            override_grid: false,
            override_grid_lonlat: false,
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
            validation_popup_msg: None,
            inp_generator: InpGeneratorState::default(),
            use_source_mech: false,
            cs_start_lon: 0.0,
            cs_finish_lon: 0.0,
            cs_start_lat: 0.0,
            cs_finish_lat: 0.0,
            cs_dist_inc: 2.0,
            cs_results: None,
            oof_enabled: false,
            regional_mag: 100.0,
            regional_azimuth: 0.0,
            regional_plunge: 0.0,
            override_physics: false,
            fric: 0.4,
            poisson: 0.25,
            young: 800000.0,
            plot_vmin: String::new(),
            plot_vmax: String::new(),
            plot_title: "Coulomb Stress Change".to_string(),
            plot_title_size: 45,
            plot_label_size: 32,
            plot_tick_size: 15,
            plot_cbar_label_size: 25,
            plot_cbar_tick_size: 15,
            plot_contour_steps: 50,
            plot_aspect_equal: true,
            plot_fault_color: "black".to_string(),
            plot_fault_width: 4,
            plot_style: MapPlotStyle::Scatter,
            plot_cs_track: true,
            plot_use_lonlat: false,
            tiff_export_component: "coulomb".to_string(),
            overlay_cs_files: Vec::new(),
        }
    }
}

pub fn show_cfs_app(ctx: &egui::Context, state: &mut CfsDialogState) {
    let mut is_open = state.is_open;
    

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

    show_activity_bar(ctx, state);
    show_sidebar(ctx, state);
    show_central_view(ctx, state);

    // Process channel messages
    if let Some(rx) = &state.calc_rx {
        if let Ok(res) = rx.try_recv() {
            let mut vis_data = VisData {
                mode: state.mode.clone(),
                def_results: None,
                cfs_results: None,
                cs_results: None,
                parsed_input: state.parsed_input.clone(),
                cached_texture: None,
                cache_params: None,
                cached_bounds: None,
                selected_depth_filter: "Max".to_string(),
            };
            match res {
                CfsThreadResult::Deformation(r) => { vis_data.def_results = Some(r.clone()); vis_data.def_results = Some(r); },
                CfsThreadResult::CoulombGrid(r) | CfsThreadResult::BatchReceiver(r) => { vis_data.cfs_results = Some(r.clone()); vis_data.cfs_results = Some(r); },
                CfsThreadResult::CrossSection(r) => { vis_data.cs_results = Some(r.clone()); vis_data.cs_results = Some(r); },
            }
            
            let tab_name = "📊 Current Calc".to_string();
            let tab = EditorTab::Visualization(tab_name.clone(), Box::new(vis_data));
            
            if let Some(pos) = state.open_tabs.iter().position(|t| match t {
                EditorTab::Visualization(n, _) => n == &tab_name,
                _ => false,
            }) {
                state.open_tabs[pos] = tab;
                state.active_tab_index = pos;
            } else {
                state.open_tabs.push(tab);
                state.active_tab_index = state.open_tabs.len() - 1;
            }
            
            state.is_calculating = false;
            state.calc_rx = None;
        }
    }
    
    if let Some(msg) = state.validation_popup_msg.clone() {
        let mut is_open = true;
        egui::Window::new("Validation Result")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut is_open)
            .show(ctx, |ui| {
                ui.label(&msg);
                ui.add_space(10.0);
                if ui.button("OK").clicked() {
                    state.validation_popup_msg = None;
                }
            });
        if !is_open {
            state.validation_popup_msg = None;
        }
    }
    
    state.is_open = is_open;
}

fn show_activity_bar(ctx: &egui::Context, state: &mut CfsDialogState) {
    egui::SidePanel::left("cfs_activity_bar")
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
                
                ui.add_space(5.0);
                
                let mut inp_btn = egui::Button::new(egui::RichText::new("📝").size(20.0)).min_size(tab_size);
                if state.active_tab == ActiveTab::InpGenerator { inp_btn = inp_btn.fill(ui.visuals().selection.bg_fill); }
                if ui.add(inp_btn).on_hover_text("INP Generator").clicked() {
                    state.active_tab = ActiveTab::InpGenerator;
                }
            });
        });
}
fn show_sidebar(ctx: &egui::Context, state: &mut CfsDialogState) {
    egui::SidePanel::left("cfs_sidebar")
        .resizable(true)
        .min_width(280.0)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add_space(10.0);
                match state.active_tab {
                    ActiveTab::Explorer => show_explorer(ui, state),
                    ActiveTab::Process => show_process(ui, state),
                    ActiveTab::InpGenerator => show_inp_generator_tab(ui, state),
                }
            });
        });
}

fn show_selected_files_panel(ui: &mut egui::Ui, state: &mut CfsDialogState) {
    ui.group(|ui| {
        ui.label(egui::RichText::new("Selected Files:").strong());
        if !state.input_path.is_empty() {
            ui.horizontal(|ui| {
                ui.label(format!("INP: {}", std::path::Path::new(&state.input_path).file_name().unwrap_or_default().to_string_lossy()));
                if ui.button("Validate").clicked() {
                    if let Ok(input) = crate::core::cfs_parser::open_input_file_cui(&state.input_path) {
                        state.validation_popup_msg = Some(format!("Valid INP file!\nSegments: {}\nDepth: {}", input.el.len(), input.cdepth));
                    } else {
                        state.validation_popup_msg = Some("Invalid INP file.".to_string());
                    }
                }
                if ui.button("X").clicked() {
                    state.input_path.clear();
                }
            });
        }
        if !state.batch_path.is_empty() {
            ui.horizontal(|ui| {
                ui.label(format!("BATCH: {}", std::path::Path::new(&state.batch_path).file_name().unwrap_or_default().to_string_lossy()));
                if ui.button("X").clicked() {
                    state.batch_path.clear();
                }
            });
        }
        
        let mut to_remove = None;
        for (i, cs_file) in state.overlay_cs_files.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(format!("CS: {}", std::path::Path::new(cs_file).file_name().unwrap_or_default().to_string_lossy()));
                if ui.button("X").clicked() {
                    to_remove = Some(i);
                }
            });
        }
        if let Some(i) = to_remove {
            state.overlay_cs_files.remove(i);
        }
        
        if state.input_path.is_empty() && state.batch_path.is_empty() && state.overlay_cs_files.is_empty() {
            ui.label(egui::RichText::new("None").italics());
        }
    });
}

fn show_explorer(ui: &mut egui::Ui, state: &mut CfsDialogState) {
    show_selected_files_panel(ui, state);
    ui.separator();
    ui.heading("EXPLORER");
    ui.separator();
    

    
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("CURRENT DIRECTORY").strong());
        if ui.button("📂 Open Folder").clicked() {
            if let Some(path) = rfd::FileDialog::new().pick_folder() {
                state.explorer_dir = Some(path);
            }
        }
    });
    
    let current_dir = state.explorer_dir.clone().unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    ui.label(current_dir.display().to_string());
    ui.add_space(5.0);
    
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&current_dir) {
        for entry in entries.flatten() {
            files.push(entry.path());
        }
    }
    files.sort();
    
    for path in files {
        if path.is_dir() { continue; }
        
        let filename = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
        let is_inp = filename.ends_with(".inp") || filename.ends_with(".dat") || filename.ends_with(".txt") || filename.ends_with(".csv");
        
        let mut icon = "📄";
        if filename.ends_with(".inp") { icon = "⚙"; }
        else if filename.ends_with(".csv") { icon = "📊"; }
        else if filename.ends_with(".png") || filename.ends_with(".tif") { icon = "🖼"; }
        
        let response = ui.selectable_label(false, format!("{} {}", icon, filename));
        
        response.context_menu(|ui| {
            if is_inp {
                if ui.button("Set as Input (INP)").clicked() {
                    state.input_path = path.display().to_string();
                    if let Ok(input) = open_input_file_cui(&state.input_path) {
                        state.parsed_input = Some(input.clone());
                        state.receiver_strike = input.av_strike;
                        state.receiver_dip = input.av_dip;
                        state.receiver_rake = input.av_rake;
                        state.fric = input.fric;
                        state.poisson = input.pois;
                        state.young = input.young;
                        state.regional_mag = input.rstress[0];
                        state.regional_azimuth = input.rstress[1];
                        state.regional_plunge = input.rstress[2];
                        state.cs_start_lon = input.cross_section.start_x;
                        state.cs_finish_lon = input.cross_section.finish_x;
                        state.cs_start_lat = input.cross_section.start_y;
                        state.cs_finish_lat = input.cross_section.finish_y;
                        
                        // Set Grid Defaults
                        if let Some(&mx) = input.xvec.first() { state.min_x = mx; }
                        if let Some(&mx) = input.xvec.last() { state.max_x = mx; }
                        if input.xvec.len() > 1 { state.x_inc = input.xvec[1] - input.xvec[0]; }
                        
                        if let Some(&my) = input.yvec.first() { state.min_y = my; }
                        if let Some(&my) = input.yvec.last() { state.max_y = my; }
                        if input.yvec.len() > 1 { state.y_inc = input.yvec[1] - input.yvec[0]; }
                        
                        if input.map_info.min_lon != 0.0 || input.map_info.max_lon != 0.0 {
                            state.min_lon = input.map_info.min_lon;
                            state.max_lon = input.map_info.max_lon;
                            state.min_lat = input.map_info.min_lat;
                            state.max_lat = input.map_info.max_lat;
                            if input.xvec.len() > 1 {
                                state.lon_inc = (state.max_lon - state.min_lon) / (input.xvec.len() as f64 - 1.0);
                            }
                            if input.yvec.len() > 1 {
                                state.lat_inc = (state.max_lat - state.min_lat) / (input.yvec.len() as f64 - 1.0);
                            }
                        }
                    }
                    ui.close_menu();
                }
            }
            if filename.ends_with(".csv") || filename.ends_with(".txt") {
                if ui.button("Set as Batch Receiver").clicked() {
                    state.batch_path = path.display().to_string();
                    ui.close_menu();
                }
                if filename.ends_with(".csv") {
                    if ui.button("Add to CS Overlay Tracks").clicked() {
                        let p_str = path.display().to_string();
                        if !state.overlay_cs_files.contains(&p_str) {
                            state.overlay_cs_files.push(p_str);
                        }
                        ui.close_menu();
                    }
                    if ui.button("View Map/Cross Section").clicked() {
                        let mut vis_data = VisData {
                            mode: CfsMode::CoulombGrid,
                            def_results: None,
                            cfs_results: None,
                            cs_results: None,
                            parsed_input: state.parsed_input.clone(),
                            cached_texture: None,
                            cache_params: None,
                            cached_bounds: None,
                            selected_depth_filter: "Max".to_string(),
                        };
                        
                        if let Ok(results) = crate::core::cfs_io::read_cross_section_csv(&path) {
                            if !results.is_empty() && results[0].distance >= 0.0 {
                                vis_data.cs_results = Some(results);
                                vis_data.mode = CfsMode::CrossSection;
                            } else if let Ok(coulomb) = crate::core::cfs_io::read_coulomb_csv(&path) {
                                vis_data.cfs_results = Some(coulomb);
                                vis_data.mode = CfsMode::CoulombGrid;
                            }
                        } else if let Ok(coulomb) = crate::core::cfs_io::read_coulomb_csv(&path) {
                            vis_data.cfs_results = Some(coulomb);
                            vis_data.mode = CfsMode::CoulombGrid;
                        }
                        
                        let tab_name = format!("📊 {}", filename);
                        let tab = EditorTab::Visualization(tab_name.clone(), Box::new(vis_data));
                        
                        if let Some(pos) = state.open_tabs.iter().position(|t| match t {
                            EditorTab::Visualization(n, _) => n == &tab_name,
                            _ => false,
                        }) {
                            state.open_tabs[pos] = tab;
                            state.active_tab_index = pos;
                        } else {
                            state.open_tabs.push(tab);
                            state.active_tab_index = state.open_tabs.len() - 1;
                        }
                        ui.close_menu();
                    }
                }
            }
            
            if ui.button("View / Edit ASCII").clicked() {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    let tab = EditorTab::Ascii(path.clone(), content, false);
                    if !state.open_tabs.contains(&tab) {
                        state.open_tabs.push(tab);
                    }
                    state.active_tab_index = state.open_tabs.len() - 1;
                }
                ui.close_menu();
            }
            
            if filename.ends_with(".inp") {
                if ui.button("Visualize Faults").clicked() {
                    if let Ok(input) = crate::core::cfs_parser::open_input_file_cui(&path.display().to_string()) {
                        let mut vis_data = VisData {
                            mode: CfsMode::CoulombGrid, // Open in map view
                            parsed_input: Some(input),
                            cfs_results: None,
                            cs_results: None,
                            def_results: None,
                            cached_texture: None,
                            cache_params: None,
                            cached_bounds: None,
                            selected_depth_filter: "Max".to_string(),
                        };
                        let file_name = path.file_name().unwrap_or_default().to_string_lossy();
                        let tab_name = format!("Visual: {}", file_name);
                        
                        let mut exists = false;
                        for (i, t) in state.open_tabs.iter().enumerate() {
                            if let EditorTab::Visualization(n, _) = t {
                                if n == &tab_name {
                                    state.active_tab_index = i;
                                    exists = true;
                                    break;
                                }
                            }
                        }
                        
                        if !exists {
                            let tab = EditorTab::Visualization(tab_name.clone(), Box::new(vis_data));
                            state.open_tabs.push(tab);
                            state.active_tab_index = state.open_tabs.len() - 1;
                        }
                    }
                    ui.close_menu();
                }
            }
        });
    }
}

fn show_process(ui: &mut egui::Ui, state: &mut CfsDialogState) {
    show_selected_files_panel(ui, state);
    ui.separator();
    ui.heading("PROCESS & RUN");
    ui.separator();
    if state.input_path.is_empty() {
        ui.add_space(5.0);
        ui.label(egui::RichText::new("⚠️ Please select an INP file in the Explorer tab first!").color(egui::Color32::RED).strong());
        ui.add_space(10.0);
    }
    
    ui.add_enabled_ui(!state.input_path.is_empty(), |ui| {
        // --- Mode Selection ---
        ui.label(egui::RichText::new("Calculation Mode").strong());
        ui.radio_value(&mut state.mode, CfsMode::Deformation, "Deformation");
        ui.radio_value(&mut state.mode, CfsMode::CoulombGrid, "Coulomb on Grid");
        ui.radio_value(&mut state.mode, CfsMode::CrossSection, "Cross Section");
        ui.add_enabled_ui(!state.batch_path.is_empty(), |ui| {
            ui.radio_value(&mut state.mode, CfsMode::BatchReceiver, "Batch Receiver");
        });
        ui.add_space(10.0);
    
    // --- Orientation ---
    if state.mode == CfsMode::CoulombGrid || state.mode == CfsMode::CrossSection {
        ui.label(egui::RichText::new("Receiver Orientation").strong());
        ui.checkbox(&mut state.use_source_mech, "Use Source Mechanism");
        ui.add_enabled_ui(!state.use_source_mech, |ui| {
            ui.add(egui::DragValue::new(&mut state.receiver_strike).speed(1.0).prefix("Strike: "));
            ui.add(egui::DragValue::new(&mut state.receiver_dip).speed(1.0).prefix("Dip: "));
            ui.add(egui::DragValue::new(&mut state.receiver_rake).speed(1.0).prefix("Rake: "));
        });
        ui.add_space(10.0);
    }
    
    // --- Physics ---
    ui.label(egui::RichText::new("Physical Parameters").strong());
    ui.checkbox(&mut state.override_physics, "Override Physics");
    if state.override_physics {
        ui.add(egui::DragValue::new(&mut state.fric).speed(0.01).prefix("Friction: "));
        ui.add(egui::DragValue::new(&mut state.poisson).speed(0.01).prefix("Poisson: "));
        ui.add(egui::DragValue::new(&mut state.young).speed(1000.0).prefix("Young's: "));
    }
    ui.add_space(10.0);
    
    // --- Grid / Depth Overrides ---
    if state.mode == CfsMode::CrossSection {
        ui.label(egui::RichText::new("Cross Section Profile (km from Zero Origin)").strong());
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut state.cs_start_lon).speed(0.1).prefix("Start X: "));
            ui.add(egui::DragValue::new(&mut state.cs_finish_lon).speed(0.1).prefix("End: "));
        });
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut state.cs_start_lat).speed(0.1).prefix("Start Y: "));
            ui.add(egui::DragValue::new(&mut state.cs_finish_lat).speed(0.1).prefix("End: "));
        });
        ui.add(egui::DragValue::new(&mut state.cs_dist_inc).speed(0.5).prefix("Dist Inc (km): "));
    } else {
        ui.checkbox(&mut state.override_grid, "Override Grid Params (km)");
        if state.override_grid {
            state.override_grid_lonlat = false;
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut state.min_x).speed(0.1).prefix("Min X: "));
                ui.add(egui::DragValue::new(&mut state.max_x).speed(0.1).prefix("Max: "));
            });
            ui.add(egui::DragValue::new(&mut state.x_inc).speed(0.1).prefix("X-Inc: "));
            
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut state.min_y).speed(0.1).prefix("Min Y: "));
                ui.add(egui::DragValue::new(&mut state.max_y).speed(0.1).prefix("Max: "));
            });
            ui.add(egui::DragValue::new(&mut state.y_inc).speed(0.1).prefix("Y-Inc: "));
        }
        
        ui.checkbox(&mut state.override_grid_lonlat, "Override Grid Params (Lon/Lat)");
        if state.override_grid_lonlat {
            state.override_grid = false;
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut state.min_lon).speed(0.01).prefix("Min Lon: "));
                ui.add(egui::DragValue::new(&mut state.max_lon).speed(0.01).prefix("Max: "));
            });
            ui.add(egui::DragValue::new(&mut state.lon_inc).speed(0.01).prefix("Lon-Inc: "));
            
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut state.min_lat).speed(0.01).prefix("Min Lat: "));
                ui.add(egui::DragValue::new(&mut state.max_lat).speed(0.01).prefix("Max: "));
            });
            ui.add(egui::DragValue::new(&mut state.lat_inc).speed(0.01).prefix("Lat-Inc: "));
        }
    }
    
    ui.add_space(5.0);
    ui.label(egui::RichText::new("Target Depth (km)").strong());
    ui.checkbox(&mut state.is_depth_range, "Depth Range");
    if state.is_depth_range {
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut state.min_depth).speed(0.1).prefix("Min: "));
            ui.add(egui::DragValue::new(&mut state.max_depth).speed(0.1).prefix("Max: "));
        });
        ui.add(egui::DragValue::new(&mut state.depth_inc).speed(0.1).prefix("Inc: "));
    } else {
        ui.add(egui::DragValue::new(&mut state.depth).speed(0.5).prefix("Depth: "));
    }
    ui.add_space(10.0);
    
    // --- OOF ---
    if state.mode == CfsMode::CoulombGrid || state.mode == CfsMode::CrossSection {
        ui.label(egui::RichText::new("Regional Stress (OOF)").strong());
        ui.checkbox(&mut state.oof_enabled, "Enable OOF");
        if state.oof_enabled {
            ui.add(egui::DragValue::new(&mut state.regional_mag).speed(1.0).prefix("Magnitude: "));
            ui.add(egui::DragValue::new(&mut state.regional_azimuth).speed(1.0).prefix("Azimuth: "));
            ui.add(egui::DragValue::new(&mut state.regional_plunge).speed(1.0).prefix("Plunge: "));
        }
        ui.add_space(10.0);
    }
    
    // --- Run Button ---
    if state.is_calculating {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label("Calculating...");
        });
    } else {
        if ui.button(egui::RichText::new("▶ RUN CALCULATION").size(14.0).strong()).clicked() {
            if let Some(mut input) = state.parsed_input.clone() {
                // Apply physics overrides
                if state.override_physics {
                    input.fric = state.fric;
                    input.pois = state.poisson;
                    input.young = state.young;
                }
            
                if state.mode != CfsMode::CrossSection && state.override_grid {
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
                }
                
                if state.mode != CfsMode::CrossSection && state.override_grid_lonlat {
                    let lon_inc = state.lon_inc.max(0.0001);
                    let lat_inc = state.lat_inc.max(0.0001);
                    
                    let earth_r = 6371.0;
                    let rad_conv = std::f64::consts::PI / 180.0;
                    let cos_lat = input.map_info.zero_lat.to_radians().cos();
                    
                    let mut xvec = Vec::new();
                    let mut lon = state.min_lon;
                    while lon <= state.max_lon + lon_inc * 0.5 {
                        let x = (lon - input.map_info.zero_lon) * (earth_r * cos_lat) * rad_conv;
                        xvec.push(x);
                        lon += lon_inc;
                    }
                    
                    let mut yvec = Vec::new();
                    let mut lat = state.min_lat;
                    while lat <= state.max_lat + lat_inc * 0.5 {
                        let y = (lat - input.map_info.zero_lat) * earth_r * rad_conv;
                        yvec.push(y);
                        lat += lat_inc;
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
                if state.is_depth_range {
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
                let rx_strike = if state.use_source_mech { input.av_strike } else { state.receiver_strike };
                let rx_dip = if state.use_source_mech { input.av_dip } else { state.receiver_dip };
                let rx_rake = if state.use_source_mech { input.av_rake } else { state.receiver_rake };
                let batch_path = state.batch_path.clone();
                
                let oof = state.oof_enabled;
                let regional_voigt = if oof {
                    let tensor = build_regional_stress_tensor(state.regional_mag, state.regional_azimuth, state.regional_plunge);
                    Some(regional_tensor_to_voigt(&tensor))
                } else {
                    None
                };
                
                // Cross section params
                let cs_start_lon = state.cs_start_lon;
                let cs_finish_lon = state.cs_finish_lon;
                let cs_start_lat = state.cs_start_lat;
                let cs_finish_lat = state.cs_finish_lat;
                let cs_dist_inc = state.cs_dist_inc;
                
                let (tx, rx) = channel();
                state.calc_rx = Some(rx);
                state.is_calculating = true;
                state.def_results = None;
                state.cfs_results = None;
                state.cs_results = None;
                state.calculation_msg = String::new();
                
                thread::spawn(move || {
                    match mode {
                        CfsMode::Deformation => {
                            let res = calculate_deformation(&input, &depths);
                            let _ = tx.send(CfsThreadResult::Deformation(res));
                        }
                        CfsMode::CoulombGrid => {
                            let res = if oof {
                                calculate_coulomb_grid_oof(&input, &depths, regional_voigt.as_ref().unwrap())
                            } else {
                                calculate_coulomb_grid(&input, &depths, rx_strike, rx_dip, rx_rake)
                            };
                            let _ = tx.send(CfsThreadResult::CoulombGrid(res));
                        }
                        CfsMode::CrossSection => {
                            let start_x_km = cs_start_lon;
                            let start_y_km = cs_start_lat;
                            let finish_x_km = cs_finish_lon;
                            let finish_y_km = cs_finish_lat;
                            
                            let res = calculate_coulomb_cross_section(
                                &input,
                                start_x_km, start_y_km,
                                finish_x_km, finish_y_km,
                                cs_dist_inc,
                                &depths,
                                rx_strike, rx_dip, rx_rake,
                                regional_voigt.as_ref()
                            );
                            let _ = tx.send(CfsThreadResult::CrossSection(res));
                        }
                        CfsMode::BatchReceiver => {
                            if let Ok(batch) = open_batch_file(&batch_path, &input.map_info) {
                                let res = calculate_coulomb_batch(&input, &batch);
                                let _ = tx.send(CfsThreadResult::BatchReceiver(res));
                            }
                        }
                    }
                });
            } else {
                state.calculation_msg = "Please set a valid INP file first.".to_string();
            }
        }
    }
    });
    
    ui.add_space(20.0);
    ui.label(egui::RichText::new("LOG STATUS").strong());
    ui.label(&state.calculation_msg);
}

fn show_inp_generator_tab(ui: &mut egui::Ui, state: &mut CfsDialogState) {
    show_selected_files_panel(ui, state);
    ui.separator();
    ui.heading("📝 INP GENERATOR");
    ui.separator();
    ui.label("Generate or append fault parameters to an INP file.");
    ui.add_space(10.0);
    
    if state.input_path.is_empty() {
        state.inp_generator.append_path = None;
    } else {
        state.inp_generator.append_path = Some(state.input_path.clone());
    }
    
    crate::ui::inp_generator_dialog::show_inp_generator_panel(ui, &mut state.inp_generator);
}

fn show_central_view(ctx: &egui::Context, state: &mut CfsDialogState) {
    egui::CentralPanel::default().show(ctx, |ui| {
        // Render Tab Bar
        ui.horizontal(|ui| {
            let mut close_idx = None;
            for (idx, tab) in state.open_tabs.iter().enumerate() {
                let is_active = state.active_tab_index == idx;
                
                let (title, is_dirty) = match tab {
                    EditorTab::Visualization(name, _) => (name.clone(), false),
                    EditorTab::Ascii(path, _, dirty) => (path.file_name().unwrap_or_default().to_string_lossy().to_string(), *dirty),
                };
                
                let is_vis = false; // allow closing all tabs now!
                
                // We use a custom horizontal layout for the tab to include a close button
                let response = ui.selectable_value(&mut state.active_tab_index, idx, title);
                
                if !is_vis {
                    let close_char = if is_dirty { " ⏺" } else { " ✕" };
                    if ui.button(close_char).on_hover_text("Close Tab").clicked() {
                        close_idx = Some(idx);
                    }
                    ui.add_space(5.0);
                }
                
                // Add right click context menu just in case
                if !is_vis {
                    response.context_menu(|ui| {
                        if ui.button("Close Tab").clicked() {
                            close_idx = Some(idx);
                            ui.close_menu();
                        }
                    });
                }
            }
            
            if let Some(idx) = close_idx {
                state.open_tabs.remove(idx);
                if state.active_tab_index >= state.open_tabs.len() {
                    state.active_tab_index = state.open_tabs.len().saturating_sub(1);
                }
            }
        });
        ui.separator();
        
        if state.open_tabs.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label(egui::RichText::new("No editors open").color(egui::Color32::DARK_GRAY).size(24.0));
            });
            return;
        }
        
        let active_idx = state.active_tab_index;
        
        // Render Active Tab Content
        // We pull the tab out to avoid borrow checker errors since show_visualization_tab needs state
        let mut active_tab = state.open_tabs.remove(active_idx);
        match &mut active_tab {
            EditorTab::Visualization(name, vis_data) => {
                show_visualization_tab(ui, state, vis_data);
            },
            EditorTab::Ascii(path, content, is_dirty) => {
                ui.horizontal(|ui| {
                    ui.label(path.display().to_string());
                    if ui.button("💾 Save").clicked() {
                        if let Err(e) = std::fs::write(&path, &*content) {
                            state.calculation_msg = format!("Failed to save: {}", e);
                        } else {
                            *is_dirty = false;
                            state.calculation_msg = "File saved successfully.".to_string();
                        }
                    }
                });
                ui.separator();
                
                egui::ScrollArea::both().show(ui, |ui| {
                    if ui.add_sized(ui.available_size(), egui::TextEdit::multiline(content).code_editor()).changed() {
                        *is_dirty = true;
                    }
                });
            },
        }
        state.open_tabs.insert(active_idx, active_tab);
    });
}

fn show_visualization_tab(ui: &mut egui::Ui, state: &mut CfsDialogState, vis_data: &mut VisData) {
    egui::SidePanel::right("plot_settings_panel")
        .resizable(true)
        .min_width(200.0)
        .show_inside(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading("Plot Settings");
                ui.separator();
                
                ui.label(egui::RichText::new("Global Settings (GUI & Export)").strong().color(ui.visuals().warn_fg_color));
                ui.add_space(5.0);
                
                ui.label("Data Range (Vmin/Vmax):");
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut state.plot_vmin).desired_width(60.0));
                    ui.label("Min");
                    ui.add(egui::TextEdit::singleline(&mut state.plot_vmax).desired_width(60.0));
                    ui.label("Max");
                });
                
                ui.add_space(5.0);
                ui.label("Styling:");
                ui.add(egui::DragValue::new(&mut state.plot_contour_steps).prefix("Contour Steps: "));
                
                ui.horizontal(|ui| {
                    ui.label("Map Plot Style:");
                    egui::ComboBox::from_id_salt("plot_style_combo")
                        .selected_text(match state.plot_style {
                            MapPlotStyle::Scatter => "Scatter Points",
                            MapPlotStyle::Imshow => "Imshow (Grid)",
                            MapPlotStyle::Contourf => "Contourf (Smooth)",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut state.plot_style, MapPlotStyle::Scatter, "Scatter Points");
                            ui.selectable_value(&mut state.plot_style, MapPlotStyle::Imshow, "Imshow (Grid)");
                            ui.selectable_value(&mut state.plot_style, MapPlotStyle::Contourf, "Contourf (Smooth)");
                        });
                });
                ui.checkbox(&mut state.plot_aspect_equal, "Equal Aspect (1:1)");
                ui.checkbox(&mut state.plot_use_lonlat, "Use Lon/Lat Grid for Map View");
                
                ui.add_space(5.0);
                ui.label("Fault Lines:");
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("fault_color_combo")
                        .selected_text(&state.plot_fault_color)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut state.plot_fault_color, "black".to_string(), "Black");
                            ui.selectable_value(&mut state.plot_fault_color, "white".to_string(), "White");
                            ui.selectable_value(&mut state.plot_fault_color, "red".to_string(), "Red");
                            ui.selectable_value(&mut state.plot_fault_color, "blue".to_string(), "Blue");
                            ui.selectable_value(&mut state.plot_fault_color, "green".to_string(), "Green");
                            ui.selectable_value(&mut state.plot_fault_color, "yellow".to_string(), "Yellow");
                        });
                    ui.label("Color");
                });
                ui.add(egui::DragValue::new(&mut state.plot_fault_width).prefix("Width: "));
                
                ui.add_space(15.0);
                ui.label(egui::RichText::new("Image Export Configuration").strong().color(ui.visuals().warn_fg_color));
                ui.add_space(5.0);
                ui.label("Labels & Fonts:");
                ui.text_edit_singleline(&mut state.plot_title);
                ui.add(egui::DragValue::new(&mut state.plot_title_size).prefix("Title Size: "));
                ui.add(egui::DragValue::new(&mut state.plot_label_size).prefix("Label Size: "));
                ui.add(egui::DragValue::new(&mut state.plot_tick_size).prefix("Tick Size: "));
                ui.add(egui::DragValue::new(&mut state.plot_cbar_label_size).prefix("Cbar Label Size: "));
                ui.add(egui::DragValue::new(&mut state.plot_cbar_tick_size).prefix("Cbar Tick Size: "));
                
                if vis_data.mode == CfsMode::CoulombGrid {
                    ui.add_space(20.0);
                    ui.label(egui::RichText::new("Export Options").strong());
                    if ui.button("Save Coulomb Results (.csv)").clicked() {
                        if let Some(res) = &vis_data.cfs_results {
                            if let Some(path) = rfd::FileDialog::new().set_file_name("coulomb_out.csv").save_file() {
                                crate::core::cfs_io::write_coulomb_csv(&path, res);
                                state.calculation_msg = "CSV Exported.".to_string();
                            }
                        }
                    }
                    ui.horizontal(|ui| {
                        ui.label("TIFF Component:");
                        egui::ComboBox::from_id_salt("tiff_comp_combo")
                            .selected_text(&state.tiff_export_component)
                            .show_ui(ui, |ui| {
                                let comps = ["coulomb", "ux", "uy", "uz", "sxx", "syy", "szz", "sxy", "sxz", "syz", "shear", "normal"];
                                for c in comps {
                                    ui.selectable_value(&mut state.tiff_export_component, c.to_string(), c);
                                }
                            });
                    });
                    
                    if ui.button("Export to TIFF").clicked() {
                        // Keep TIFF export
                        if let Some(res) = &vis_data.cfs_results {
                            let comp = state.tiff_export_component.clone();
                            if let Some(path) = rfd::FileDialog::new().set_file_name(format!("{}_grid", comp)).save_file() {
                                if let Some(input) = &vis_data.parsed_input {
                                    let w = input.xvec.len();
                                    let h = input.yvec.len();
                                    if w > 0 && h > 0 && res.len() == w * h {
                                        let mut tiff_data = vec![0.0_f32; w * h];
                                        for (i, r) in res.iter().enumerate() {
                                            let ix = i / h;
                                            let iy = i % h;
                                            let tiff_y = h - 1 - iy;
                                            let tiff_x = ix;
                                            
                                            let val = match comp.as_str() {
                                                "ux" => r.ux,
                                                "uy" => r.uy,
                                                "uz" => r.uz,
                                                "sxx" => r.sxx,
                                                "syy" => r.syy,
                                                "szz" => r.szz,
                                                "sxy" => r.sxy,
                                                "sxz" => r.sxz,
                                                "syz" => r.syz,
                                                "shear" => r.shear,
                                                "normal" => r.normal,
                                                _ => r.coulomb,
                                            };
                                            tiff_data[tiff_y * w + tiff_x] = val as f32;
                                        }
                                        let mut x_min = *input.xvec.first().unwrap_or(&0.0);
                                        let mut x_max = *input.xvec.last().unwrap_or(&1.0);
                                        let mut y_min = *input.yvec.first().unwrap_or(&0.0);
                                        let mut y_max = *input.yvec.last().unwrap_or(&1.0);
                                        if input.map_info.min_lon != 0.0 || input.map_info.max_lon != 0.0 {
                                            x_min = input.map_info.min_lon;
                                            x_max = input.map_info.max_lon;
                                            y_min = input.map_info.min_lat;
                                            y_max = input.map_info.max_lat;
                                        }
                                        let base_path = path.to_string_lossy().to_string();
                                        let tif_path = std::path::PathBuf::from(format!("{}_{}.tif", base_path, comp));
                                        if let Err(e) = seisbox_core::io::tiff_export::export_grid_to_tiff(&tif_path, &tiff_data, w as u32, h as u32, x_min, x_max, y_min, y_max) {
                                            state.calculation_msg = format!("TIFF export failed: {}", e);
                                        } else {
                                            state.calculation_msg = "TIFF exported successfully.".to_string();
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if ui.button("Export Image").clicked() {
                        let path = rfd::FileDialog::new()
                            .set_title("Export Image")
                            .add_filter("PNG Image", &["png"])
                            .save_file();
                        if let Some(p) = path {
                            let temp_csv = std::env::temp_dir().join("temp_coulomb_export.csv");
                            if let Some(res) = &vis_data.cfs_results {
                                crate::core::cfs_io::write_coulomb_csv(&temp_csv, res);
                            }
                            
                            let mut start_lons = vec![];
                            let mut start_lats = vec![];
                            let mut finish_lons = vec![];
                            let mut finish_lats = vec![];
                            
                            if state.cs_start_lon != 0.0 || state.cs_start_lat != 0.0 || state.cs_finish_lon != 0.0 || state.cs_finish_lat != 0.0 {
                                start_lons.push(state.cs_start_lon);
                                start_lats.push(state.cs_start_lat);
                                finish_lons.push(state.cs_finish_lon);
                                finish_lats.push(state.cs_finish_lat);
                            }
                            
                            for cs_file in &state.overlay_cs_files {
                                if let Ok(results) = crate::core::cfs_io::read_cross_section_csv(std::path::Path::new(cs_file)) {
                                    if !results.is_empty() {
                                        start_lons.push(results.first().unwrap().cfs.lon);
                                        start_lats.push(results.first().unwrap().cfs.lat);
                                        finish_lons.push(results.last().unwrap().cfs.lon);
                                        finish_lats.push(results.last().unwrap().cfs.lat);
                                    }
                                }
                            }
                            
                            let has_tracks = !start_lons.is_empty();
                            
                            let mut config = crate::plot::cfs_plotter::PlotConfig {
                                csv_path: temp_csv.to_string_lossy().to_string(),
                                out_path: p.to_string_lossy().to_string(),
                                aspect_equal: state.plot_aspect_equal,
                                use_contourf: state.plot_style == MapPlotStyle::Contourf,
                                vmin: state.plot_vmin.parse().ok(),
                                vmax: state.plot_vmax.parse().ok(),
                                title: Some(state.plot_title.clone()),
                                width: 1200,
                                height: 1000,
                                title_size: state.plot_title_size,
                                label_size: state.plot_label_size,
                                tick_size: state.plot_tick_size,
                                x_labels: 5,
                                y_labels: 5,
                                cbar_label: None,
                                cbar_label_size: Some(state.plot_cbar_label_size),
                                cbar_tick_size: Some(state.plot_cbar_tick_size),
                                cbar_y_labels: None,
                                cbar_extend: None,
                                contour_steps: state.plot_contour_steps,
                                upsample_res: 200,
                                fault_color: Some(state.plot_fault_color.clone()),
                                fault_width: Some(state.plot_fault_width),
                                fault_style: None,
                                plot_cs_track: has_tracks,
                                cs_track_color: None,
                                cs_track_width: None,
                                cs_track_style: None,
                                plot_inp: if !state.input_path.is_empty() { Some(std::path::PathBuf::from(&state.input_path)) } else { None },
                                cs_start_lon: if has_tracks { Some(start_lons) } else { None },
                                cs_start_lat: if has_tracks { Some(start_lats) } else { None },
                                cs_finish_lon: if has_tracks { Some(finish_lons) } else { None },
                                cs_finish_lat: if has_tracks { Some(finish_lats) } else { None },
                            };
                            
                            if state.plot_style == MapPlotStyle::Imshow {
                                config.use_contourf = false;
                            }
                            
                            let _ = crate::plot::cfs_plotter::plot_cfs_csv(&config);
                        }
                    }
                
                }
                if vis_data.mode == CfsMode::CrossSection {
                    ui.add_space(20.0);
                    if ui.button("Save Cross Section (.csv)").clicked() {
                        if let Some(res) = &vis_data.cs_results {
                            if let Some(path) = rfd::FileDialog::new().set_file_name("cross_section.csv").save_file() {
                                crate::core::cfs_io::write_cross_section_csv(&path, res);
                                state.calculation_msg = "Cross Section CSV Exported.".to_string();
                            }
                        }
                    }
                    if ui.button("Export Cross Section Image").clicked() {
                        let path = rfd::FileDialog::new()
                            .set_title("Export Image")
                            .add_filter("PNG Image", &["png"])
                            .save_file();
                        if let Some(p) = path {
                            let temp_csv = std::env::temp_dir().join("temp_cs_export.csv");
                            if let Some(res) = &vis_data.cs_results {
                                crate::core::cfs_io::write_cross_section_csv(&temp_csv, res);
                            }
                            
                            let config = crate::plot::cfs_plotter::PlotConfig {
                                csv_path: temp_csv.to_string_lossy().to_string(),
                                out_path: p.to_string_lossy().to_string(),
                                aspect_equal: false,
                                use_contourf: state.plot_style == MapPlotStyle::Contourf,
                                vmin: state.plot_vmin.parse().ok(),
                                vmax: state.plot_vmax.parse().ok(),
                                title: Some(state.plot_title.clone()),
                                width: 1200,
                                height: 800,
                                title_size: state.plot_title_size,
                                label_size: state.plot_label_size,
                                tick_size: state.plot_tick_size,
                                x_labels: 5,
                                y_labels: 5,
                                cbar_label: None,
                                cbar_label_size: Some(state.plot_cbar_label_size),
                                cbar_tick_size: Some(state.plot_cbar_tick_size),
                                cbar_y_labels: None,
                                cbar_extend: None,
                                contour_steps: state.plot_contour_steps,
                                upsample_res: 200,
                                fault_color: Some(state.plot_fault_color.clone()),
                                fault_width: Some(state.plot_fault_width),
                                fault_style: None,
                                plot_cs_track: false,
                                cs_track_color: None,
                                cs_track_width: None,
                                cs_track_style: None,
                                plot_inp: if !state.input_path.is_empty() { Some(std::path::PathBuf::from(&state.input_path)) } else { None },
                                cs_start_lon: Some(vec![state.cs_start_lon]),
                                cs_start_lat: Some(vec![state.cs_start_lat]),
                                cs_finish_lon: Some(vec![state.cs_finish_lon]),
                                cs_finish_lat: Some(vec![state.cs_finish_lat]),
                            };
                            
                            let _ = crate::plot::cfs_plotter::plot_cfs_csv(&config);
                        }
                    }
                }
                
            });
        });

    egui::CentralPanel::default().show_inside(ui, |ui| {
        // Output Stats
        if let Some(res) = &vis_data.cfs_results {
            ui.label(format!("Calculated {} points for Coulomb Stress.", res.len()));
        }
        if let Some(res) = &vis_data.def_results {
            ui.label(format!("Calculated {} points for Deformation.", res.len()));
        }
        if let Some(res) = &vis_data.cs_results {
            ui.label(format!("Calculated {} points for Cross Section.", res.len()));
        }

        let is_cross_section = vis_data.mode == CfsMode::CrossSection;
        let aspect = if !state.plot_aspect_equal { 0.0 } else { 1.0 };
        let is_lonlat = state.plot_use_lonlat && !is_cross_section;
        let plot_id = format!("cfs_main_plot_{}_{}_{}", is_cross_section, is_lonlat, state.plot_aspect_equal);
        
        if !is_cross_section {
            if let Some(res) = &vis_data.cfs_results {
                let mut unique_depths: Vec<f64> = Vec::new();
                let mut has_998 = false;
                let mut has_999 = false;
                for r in res {
                    if r.z == 998.0 { has_998 = true; }
                    else if r.z == 999.0 { has_999 = true; }
                    else if !unique_depths.contains(&r.z) {
                        unique_depths.push(r.z);
                    }
                }
                unique_depths.sort_by(|a, b| a.partial_cmp(b).unwrap());
                
                if unique_depths.len() > 1 || has_998 || has_999 {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Display Depth:").strong());
                        let mut selected = vis_data.selected_depth_filter.clone();
                        let display_text = if selected == "998.0" || (selected == "Max" && has_998) {
                            "Max (Absolute Coulomb)"
                        } else if selected == "999.0" || (selected == "Max" && has_999) {
                            "Max (Component-wise)"
                        } else {
                            &selected
                        };
                        egui::ComboBox::from_id_salt("depth_filter_combo")
                            .selected_text(display_text)
                            .show_ui(ui, |ui| {
                                if has_998 {
                                    ui.selectable_value(&mut selected, "998.0".to_string(), "Max (Absolute Coulomb)");
                                }
                                if has_999 {
                                    ui.selectable_value(&mut selected, "999.0".to_string(), "Max (Component-wise)");
                                }
                                for d in unique_depths {
                                    let s = format!("{:.2}", d);
                                    ui.selectable_value(&mut selected, s.clone(), s);
                                }
                            });
                        if selected != vis_data.selected_depth_filter {
                            if selected == "Max" {
                                vis_data.selected_depth_filter = if has_998 { "998.0".to_string() } else if has_999 { "999.0".to_string() } else { selected.clone() };
                            } else {
                                vis_data.selected_depth_filter = selected;
                            }
                            vis_data.cached_texture = None;
                        }
                    });
                    ui.add_space(5.0);
                }
            }
        }

        let mut do_zoom_in = false;
        let mut do_zoom_out = false;
        ui.horizontal(|ui| {
            if ui.button("➕ Zoom In").clicked() {
                do_zoom_in = true;
            }
            if ui.button("➖ Zoom Out").clicked() {
                do_zoom_out = true;
            }
            ui.label(egui::RichText::new("💡 Tips: You can also use mouse scroll wheel on the plot to zoom.").italics().weak());
        });

        let mut plot = Plot::new(plot_id)
            .data_aspect(aspect)
            .show_axes([true, true])
            .legend(egui_plot::Legend::default());
        if is_cross_section {
            plot = plot.y_axis_formatter(|mark, _range| format!("{:.1}", mark.value.abs()));
        }
            
        let ctx = ui.ctx().clone();
        let plot_height = ui.available_height();
        ui.horizontal(|ui| {
            let plot_resp = ui.allocate_ui(egui::vec2((ui.available_width() - 60.0).max(10.0), ui.available_height()), |ui| {
                let plot = plot.height(plot_height);
                plot.show(ui, |plot_ui| {
                    let mut plot_data = None;
                    let mut is_lonlat = false;
                    
                    if is_cross_section {
                        if let Some(res) = &vis_data.cs_results {
                            plot_data = Some(res.iter().map(|r| [r.distance, r.cfs.z, r.cfs.coulomb]).collect::<Vec<_>>());
                        }
                    } else {
                        if let Some(res) = &vis_data.cfs_results {
                            is_lonlat = state.plot_use_lonlat;
                            
                            // "Max" might still be in old state, map it to 998.0 or 999.0
                            let target_depth: Option<f64> = if vis_data.selected_depth_filter == "Max" {
                                None
                            } else {
                                vis_data.selected_depth_filter.parse().ok()
                            };
                            
                            if let Some(td) = target_depth {
                                let mut filtered = Vec::new();
                                for r in res {
                                    if (r.z - td).abs() < 1e-4 {
                                        let vx = if is_lonlat { r.lon } else { r.x };
                                        let vy = if is_lonlat { r.lat } else { r.y };
                                        filtered.push([vx, vy, r.coulomb]);
                                    }
                                }
                                plot_data = Some(filtered);
                            } else {
                                plot_data = Some(res.iter().map(|r| [if is_lonlat { r.lon } else { r.x }, if is_lonlat { r.lat } else { r.y }, r.coulomb]).collect());
                            }
                        }
                    }
                    
                    if let Some(res_pts) = plot_data {
                        let vmin: f64 = state.plot_vmin.parse().unwrap_or(-1.0);
                        let vmax: f64 = state.plot_vmax.parse().unwrap_or(1.0);
                        let steps = state.plot_contour_steps.max(2) as usize;
                        
                        if state.plot_style == MapPlotStyle::Scatter {
                            let mut buckets: Vec<Vec<[f64; 2]>> = vec![Vec::new(); steps];
                            
                            for r in &res_pts {
                                let mut t = (r[2] - vmin) / (vmax - vmin);
                                t = t.clamp(0.0, 1.0);
                                let mut b = (t * (steps as f64 - 1.0)).round() as usize;
                                b = b.clamp(0, steps - 1);
                                buckets[b].push([r[0], r[1]]);
                            }
                            
                            let radius = 4.0_f32;
                            for b in 0..steps {
                                if buckets[b].is_empty() { continue; }
                                let t = b as f64 / (steps as f64 - 1.0);
                                
                                let color = if t < 0.5 {
                                    let frac = t * 2.0;
                                    egui::Color32::from_rgb(
                                        (59.0 + frac * (255.0 - 59.0)) as u8,
                                        (76.0 + frac * (255.0 - 76.0)) as u8,
                                        (192.0 + frac * (255.0 - 192.0)) as u8,
                                    )
                                } else {
                                    let frac = (t - 0.5) * 2.0;
                                    egui::Color32::from_rgb(
                                        (255.0 - frac * (255.0 - 180.0)) as u8,
                                        (255.0 - frac * (255.0 - 4.0)) as u8,
                                        (255.0 - frac * (255.0 - 38.0)) as u8,
                                    )
                                };
                                
                                plot_ui.points(Points::new(buckets[b].clone()).radius(radius).color(color));
                            }
                        } else {
                            let style_param = state.plot_style.clone();
                            let is_cache_valid = vis_data.cached_texture.is_some() && 
                                                 vis_data.cache_params == Some((style_param, vmin, vmax, steps, is_lonlat));
                                                 
                            if !is_cache_valid {
                                let mut xs: Vec<f64> = res_pts.iter().map(|r| r[0]).collect();
                                let mut ys: Vec<f64> = res_pts.iter().map(|r| r[1]).collect();
                                xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
                                ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
                                xs.dedup_by(|a, b| (*a - *b).abs() < 1e-4);
                                ys.dedup_by(|a, b| (*a - *b).abs() < 1e-4);
                                let nx = xs.len();
                                let ny = ys.len();
                                
                                if nx >= 2 && ny >= 2 {
                                    let min_x = xs[0];
                                    let max_x = xs[nx - 1];
                                    let min_y = ys[0];
                                    let max_y = ys[ny - 1];
                                    
                                    let mut matrix = vec![0.0_f64; nx * ny];
                                    let mut count = vec![0_u32; nx * ny];
                                    
                                    for r in &res_pts {
                                        let xi = xs.binary_search_by(|v| v.partial_cmp(&r[0]).unwrap()).unwrap_or_else(|e| e).min(nx - 1);
                                        let yi = ys.binary_search_by(|v| v.partial_cmp(&r[1]).unwrap()).unwrap_or_else(|e| e).min(ny - 1);
                                        matrix[yi * nx + xi] += r[2];
                                        count[yi * nx + xi] += 1;
                                    }
                                    
                                    for i in 0..(nx * ny) {
                                        if count[i] > 0 { matrix[i] /= count[i] as f64; }
                                    }
                                    
                                    let scale = if state.plot_style == MapPlotStyle::Contourf { 10 } else { 1 };
                                    let snx = (nx - 1) * scale + 1;
                                    let sny = (ny - 1) * scale + 1;
                                    
                                    let mut pixels = vec![egui::Color32::TRANSPARENT; snx * sny];
                                    
                                    for sy in 0..sny {
                                        let row = sny - 1 - sy;
                                        for sx in 0..snx {
                                            let fx = sx as f64 / scale as f64;
                                            let fy = sy as f64 / scale as f64;
                                            
                                            let x0 = fx.floor() as usize;
                                            let x1 = (x0 + 1).min(nx - 1);
                                            let y0 = fy.floor() as usize;
                                            let y1 = (y0 + 1).min(ny - 1);
                                            
                                            let tx = fx - x0 as f64;
                                            let ty = fy - y0 as f64;
                                            
                                            if count[y0 * nx + x0] == 0 || count[y0 * nx + x1] == 0 ||
                                               count[y1 * nx + x0] == 0 || count[y1 * nx + x1] == 0 {
                                                continue;
                                            }
                                            
                                            let v00 = matrix[y0 * nx + x0];
                                            let v10 = matrix[y0 * nx + x1];
                                            let v01 = matrix[y1 * nx + x0];
                                            let v11 = matrix[y1 * nx + x1];
                                            
                                            let val = v00 * (1.0 - tx) * (1.0 - ty) +
                                                      v10 * tx * (1.0 - ty) +
                                                      v01 * (1.0 - tx) * ty +
                                                      v11 * tx * ty;
                                            
                                            let mut t = (val - vmin) / (vmax - vmin);
                                            t = t.clamp(0.0, 1.0);
                                            
                                            let b = (t * (steps as f64 - 1.0)).round() as usize;
                                            let b = b.clamp(0, steps - 1);
                                            let t_discrete = b as f64 / (steps as f64 - 1.0);
                                            
                                            let color = if t_discrete < 0.5 {
                                                let frac = t_discrete * 2.0;
                                                egui::Color32::from_rgb(
                                                    (59.0 + frac * (255.0 - 59.0)) as u8,
                                                    (76.0 + frac * (255.0 - 76.0)) as u8,
                                                    (192.0 + frac * (255.0 - 192.0)) as u8,
                                                )
                                            } else {
                                                let frac = (t_discrete - 0.5) * 2.0;
                                                egui::Color32::from_rgb(
                                                    (255.0 - frac * (255.0 - 180.0)) as u8,
                                                    (255.0 - frac * (255.0 - 4.0)) as u8,
                                                    (255.0 - frac * (255.0 - 38.0)) as u8,
                                                )
                                            };
                                            pixels[row * snx + sx] = color;
                                        }
                                    }
                                    
                                    let image = egui::ColorImage {
                                        size: [snx, sny],
                                        pixels,
                                    };
                                    
                                    let filter = egui::TextureOptions::NEAREST;
                                    
                                    let handle = ctx.load_texture(
                                        "coulomb_heatmap",
                                        image,
                                        filter
                                    );
                                    
                                    vis_data.cached_texture = Some(handle);
                                    vis_data.cache_params = Some((state.plot_style.clone(), vmin, vmax, steps, is_lonlat));
                                    vis_data.cached_bounds = Some(([min_x, min_y], [max_x, max_y]));
                                }
                            }
                            
                            if let (Some(tex), Some(bounds)) = (&vis_data.cached_texture, &vis_data.cached_bounds) {
                                let center_x = (bounds.0[0] + bounds.1[0]) / 2.0;
                                let center_y = (bounds.0[1] + bounds.1[1]) / 2.0;
                                let width = bounds.1[0] - bounds.0[0];
                                let height = bounds.1[1] - bounds.0[1];
                                
                                let plot_img = egui_plot::PlotImage::new(
                                    tex,
                                    egui_plot::PlotPoint::new(center_x, center_y),
                                    egui::vec2(width as f32, height as f32),
                                );
                                plot_ui.image(plot_img);
                            }
                        }
                    }
                    
                    if let Some(input) = &vis_data.parsed_input {
                        let f_width = state.plot_fault_width as f32;
                        let f_color_str = state.plot_fault_color.to_lowercase();
                        let f_color = match f_color_str.as_str() {
                            "red" => egui::Color32::RED,
                            "blue" => egui::Color32::BLUE,
                            "green" => egui::Color32::GREEN,
                            "white" => egui::Color32::WHITE,
                            _ => egui::Color32::BLACK,
                        };
                        
                        if is_cross_section {
                            let x1_km = state.cs_start_lon; 
                            let y1_km = state.cs_start_lat;
                            let x2_km = state.cs_finish_lon;
                            let y2_km = state.cs_finish_lat;
                            
                            let cs_dx = x2_km - x1_km;
                            let cs_dy = y2_km - y1_km;
                            let cs_len = (cs_dx * cs_dx + cs_dy * cs_dy).sqrt();
                            
                            if cs_len > 0.0 {
                                for el in &input.el {
                                    let xs = el[0];
                                    let ys = el[1];
                                    let xf = el[2];
                                    let yf = el[3];
                                    let dip = el[6];
                                    let top = el[7]; 
                                    let bottom = el[8];
                                    
                                    let dx = xf - xs;
                                    let dy = yf - ys;
                                    let len = (dx * dx + dy * dy).sqrt();
                                    
                                    let nx = if len > 0.0 { dy / len } else { 0.0 };
                                    let ny = if len > 0.0 { -dx / len } else { 0.0 };
                                    
                                    let h = bottom - top;
                                    let mut w_h = 0.0;
                                    if dip != 90.0 && dip != 0.0 {
                                        w_h = h / dip.to_radians().tan();
                                    }
                                    
                                    let bx1 = xs + nx * w_h;
                                    let by1 = ys + ny * w_h;
                                    let bx2 = xf + nx * w_h;
                                    let by2 = yf + ny * w_h;
                                    
                                    // Intersection test
                                    let ccw = |a: (f64, f64), b: (f64, f64), c: (f64, f64)| {
                                        (c.1 - a.1) * (b.0 - a.0) > (b.1 - a.1) * (c.0 - a.0)
                                    };
                                    let intersect = |p1: (f64, f64), p2: (f64, f64), p3: (f64, f64), p4: (f64, f64)| {
                                        ccw(p1, p3, p4) != ccw(p2, p3, p4) && ccw(p1, p2, p3) != ccw(p1, p2, p4)
                                    };
                                    
                                    let cs_p1 = (x1_km, y1_km);
                                    let cs_p2 = (x2_km, y2_km);
                                    let e1 = intersect(cs_p1, cs_p2, (xs, ys), (xf, yf));
                                    let e2 = intersect(cs_p1, cs_p2, (xf, yf), (bx2, by2));
                                    let e3 = intersect(cs_p1, cs_p2, (bx2, by2), (bx1, by1));
                                    let e4 = intersect(cs_p1, cs_p2, (bx1, by1), (xs, ys));
                                    
                                    if !e1 && !e2 && !e3 && !e4 {
                                        continue;
                                    }
                                    
                                    let tc_x = (xs + xf) / 2.0;
                                    let tc_y = (ys + yf) / 2.0;
                                    let bc_x = (bx1 + bx2) / 2.0;
                                    let bc_y = (by1 + by2) / 2.0;
                                    
                                    let proj_tc = ((tc_x - x1_km) * cs_dx + (tc_y - y1_km) * cs_dy) / cs_len;
                                    let proj_bc = ((bc_x - x1_km) * cs_dx + (bc_y - y1_km) * cs_dy) / cs_len;
                                    
                                    let pts = vec![
                                        [proj_tc, -top],
                                        [proj_bc, -bottom],
                                    ];
                                    plot_ui.line(egui_plot::Line::new(pts).color(f_color).width(f_width));
                                }
                            }
                        } else {
                            let deg_to_km = 6371.0 * std::f64::consts::PI / 180.0;
                            let to_lon = |val: f64| { if is_lonlat { (val / deg_to_km) / input.map_info.zero_lat.to_radians().cos() + input.map_info.zero_lon } else { val } };
                            let to_lat = |val: f64| { if is_lonlat { (val / deg_to_km) + input.map_info.zero_lat } else { val } };
                            
                            for el in &input.el {
                                let xs = el[0];
                                let ys = el[1];
                                let xf = el[2];
                                let yf = el[3];
                                let dip = el[6];
                                let top = el[7];
                                let bottom = el[8];
                                
                                let dx = xf - xs;
                                let dy = yf - ys;
                                let len = (dx * dx + dy * dy).sqrt();
                                
                                let nx = if len > 0.0 { dy / len } else { 0.0 };
                                let ny = if len > 0.0 { -dx / len } else { 0.0 };
                                
                                let h = bottom - top;
                                let mut w_h = 0.0;
                                if dip != 90.0 && dip != 0.0 {
                                    w_h = h / dip.to_radians().tan();
                                }
                                
                                let bx1 = xs + nx * w_h;
                                let by1 = ys + ny * w_h;
                                let bx2 = xf + nx * w_h;
                                let by2 = yf + ny * w_h;
                                
                                let xs_ll = to_lon(xs);
                                let ys_ll = to_lat(ys);
                                let xf_ll = to_lon(xf);
                                let yf_ll = to_lat(yf);
                                let bx1_ll = to_lon(bx1);
                                let by1_ll = to_lat(by1);
                                let bx2_ll = to_lon(bx2);
                                let by2_ll = to_lat(by2);
                                
                                let peri_width = (f_width / 2.0).max(1.0);
                                let peri_pts = vec![
                                    [xf_ll, yf_ll], [bx2_ll, by2_ll], [bx1_ll, by1_ll], [xs_ll, ys_ll], [xf_ll, yf_ll]
                                ];
                                plot_ui.line(egui_plot::Line::new(peri_pts).color(f_color).width(peri_width));
                                plot_ui.line(egui_plot::Line::new(vec![[xs_ll, ys_ll], [xf_ll, yf_ll]]).color(f_color).width(f_width));
                            }
                            
                            // Plot Cross Section Overlay Tracks
                            let mut cs_colors = vec![
                                egui::Color32::from_rgb(255, 100, 100),
                                egui::Color32::from_rgb(100, 255, 100),
                                egui::Color32::from_rgb(100, 100, 255),
                                egui::Color32::from_rgb(255, 255, 100),
                                egui::Color32::from_rgb(255, 100, 255),
                                egui::Color32::from_rgb(100, 255, 255),
                            ];
                            
                            for (i, cs_file) in state.overlay_cs_files.iter().enumerate() {
                                if let Ok(results) = crate::core::cfs_io::read_cross_section_csv(std::path::Path::new(cs_file)) {
                                    let mut track_pts = Vec::new();
                                    for r in results {
                                        let pt = if is_lonlat {
                                            [r.cfs.lon, r.cfs.lat]
                                        } else {
                                            let x = (r.cfs.lon - input.map_info.zero_lon) * input.map_info.zero_lat.to_radians().cos() * deg_to_km;
                                            let y = (r.cfs.lat - input.map_info.zero_lat) * deg_to_km;
                                            [x, y]
                                        };
                                        if track_pts.is_empty() {
                                            track_pts.push(pt);
                                        } else {
                                            let last = track_pts.last().unwrap();
                                            if (pt[0] - last[0]).abs() > 1e-6 || (pt[1] - last[1]).abs() > 1e-6 {
                                                track_pts.push(pt);
                                            }
                                        }
                                    }
                                    
                                    if !track_pts.is_empty() {
                                        let name = std::path::Path::new(cs_file).file_stem().unwrap_or_default().to_string_lossy().to_string();
                                        let color = cs_colors[i % cs_colors.len()];
                                        plot_ui.line(egui_plot::Line::new(track_pts.clone()).color(color).width(3.0).name(name.clone()));
                                        
                                        // Label at the start
                                        let start_pt = track_pts[0];
                                        plot_ui.text(egui_plot::Text::new(egui_plot::PlotPoint::new(start_pt[0], start_pt[1]), name)
                                            .color(color)
                                            .anchor(egui::Align2::LEFT_BOTTOM));
                                    }
                                }
                            }
                        }
                    }
                    
                    if do_zoom_in {
                        let bounds = plot_ui.plot_bounds();
                        let cx = (bounds.min()[0] + bounds.max()[0]) / 2.0;
                        let cy = (bounds.min()[1] + bounds.max()[1]) / 2.0;
                        let hw = (bounds.max()[0] - bounds.min()[0]) / 2.0 * 0.75;
                        let hh = (bounds.max()[1] - bounds.min()[1]) / 2.0 * 0.75;
                        plot_ui.set_plot_bounds(egui_plot::PlotBounds::from_min_max([cx - hw, cy - hh], [cx + hw, cy + hh]));
                    }
                    if do_zoom_out {
                        let bounds = plot_ui.plot_bounds();
                        let cx = (bounds.min()[0] + bounds.max()[0]) / 2.0;
                        let cy = (bounds.min()[1] + bounds.max()[1]) / 2.0;
                        let hw = (bounds.max()[0] - bounds.min()[0]) / 2.0 * 1.33;
                        let hh = (bounds.max()[1] - bounds.min()[1]) / 2.0 * 1.33;
                        plot_ui.set_plot_bounds(egui_plot::PlotBounds::from_min_max([cx - hw, cy - hh], [cx + hw, cy + hh]));
                    }
                });
            });
            
            ui.vertical(|ui| {
                ui.add_space(20.0);
                let vmin: f64 = state.plot_vmin.parse().unwrap_or(-1.0);
                let vmax: f64 = state.plot_vmax.parse().unwrap_or(1.0);
                ui.label(format!("{:.2}", vmax));
                
                let bar_rect = ui.allocate_space(egui::vec2(20.0, (ui.available_height() - 40.0).max(10.0))).1;
                let steps = 50;
                let painter = ui.painter();
                for i in 0..steps {
                    let t1 = i as f32 / steps as f32;
                    let t2 = (i + 1) as f32 / steps as f32;
                    
                    let y1 = bar_rect.max.y - t1 * bar_rect.height();
                    let y2 = bar_rect.max.y - t2 * bar_rect.height();
                    
                    let t_val = t1 as f64;
                    let color = if t_val < 0.5 {
                        let frac = t_val * 2.0;
                        egui::Color32::from_rgb(
                            (59.0 + frac * (255.0 - 59.0)) as u8,
                            (76.0 + frac * (255.0 - 76.0)) as u8,
                            (192.0 + frac * (255.0 - 192.0)) as u8,
                        )
                    } else {
                        let frac = (t_val - 0.5) * 2.0;
                        egui::Color32::from_rgb(
                            (255.0 - frac * (255.0 - 180.0)) as u8,
                            (255.0 - frac * (255.0 - 4.0)) as u8,
                            (255.0 - frac * (255.0 - 38.0)) as u8,
                        )
                    };
                    
                    painter.rect_filled(
                        egui::Rect::from_min_max(
                            egui::pos2(bar_rect.min.x, y2),
                            egui::pos2(bar_rect.max.x, y1),
                        ),
                        0.0,
                        color,
                    );
                }
                
                ui.label(format!("{:.2}", vmin));
            });
        });
    });
}
