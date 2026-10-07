use eframe::egui;
use egui_plot::{Plot, Line, Points, Polygon, BarChart, Bar, PlotPoints};
use rfd::FileDialog;
use std::path::{PathBuf, Path};
use std::sync::mpsc::{channel, Receiver};
use std::thread;
use std::env;

use crate::core::rjmcmc::RjmcmcConfig;
use crate::core::rjmcmc_stats::{VisualizerData, load_and_process_data};
use crate::io::plotters_export::generate_rjmcmc_viz;
use crate::hvf::inversion::InversionResult;

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum DataSource {
    File,
    Manual,
}

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum OptionalDataSource {
    None,
    File,
    Manual,
}

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum RunMode {
    Inversion,
    ForwardModeling,
}

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum InversionMethod {
    RjMcmc,
    Pso,
    LevenbergMarquardt,
    Occam,
    SimulatedAnnealing,
}

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum ForwardEngine {
    Herak,
    Dfa,
    Ellipticity,
    EllipticityLove,
}

#[derive(PartialEq, Clone, Copy)]
pub enum ActiveTab {
    Explorer,
    Process,
    Extractor,
}

#[derive(Clone)]
pub enum EditorTab {
    Visualization(String, VisualizerData),
    DeterministicCurve(String, crate::hvf::output::HvsrOutput),
    Ascii(PathBuf, String, bool),
    ObsCurve(String, Vec<f64>, Vec<f64>),
    PsoResult(String, PsoVizData),
}

impl PartialEq for EditorTab {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Visualization(n1, _), Self::Visualization(n2, _)) => n1 == n2,
            (Self::DeterministicCurve(n1, _), Self::DeterministicCurve(n2, _)) => n1 == n2,
            (Self::Ascii(p1, _, _), Self::Ascii(p2, _, _)) => p1 == p2,
            (Self::ObsCurve(n1, _, _), Self::ObsCurve(n2, _, _)) => n1 == n2,
            (Self::PsoResult(n1, _), Self::PsoResult(n2, _)) => n1 == n2,
            _ => false,
        }
    }
}

/// Visualization data for PSO/LM/Occam inversion results.
#[derive(Clone)]
pub struct PsoVizData {
    pub obs_freqs: Vec<f64>,
    pub obs_hvs: Vec<f64>,
    pub result: InversionResult,
}

pub struct InversionState {
    pub active_tab: ActiveTab,
    pub explorer_dir: Option<PathBuf>,
    pub open_tabs: Vec<EditorTab>,
    pub active_tab_index: usize,

    // Run Configuration
    pub run_mode: RunMode,
    pub inv_method: InversionMethod,
    pub fwd_engine: ForwardEngine,

    pub bounds_source: DataSource,
    pub bounds_manual_nlayers: usize,
    pub bounds_manual_h: Vec<[f64; 2]>,
    pub bounds_manual_vs: Vec<[f64; 2]>,
    
    pub initial_model_source: OptionalDataSource,
    pub initial_model_manual: Vec<crate::hvf::model::Layer>,

    // Common Files (selected from explorer)
    pub obs_file: String,
    pub bounds_file: String,
    pub initial_model_file: String,
    pub earth_model_file: String,
    pub freq_file: String,
    pub output_file: String,
    pub hvf_path: String,

    // RJ-MCMC Params
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

    // PSO Params
    pub pso_pop: usize,
    pub pso_iter: usize,
    pub pso_c1: f64,
    pub pso_c2: f64,
    pub pso_w: f64,

    // SA Params
    pub sa_t_initial: f64,
    pub sa_t_final: f64,
    pub sa_cooling_rate: f64,

    // LM / Occam Params
    pub lm_lambda: f64,
    pub occam_alpha: f64,

    // Constraints
    pub enforce_increasing_vs: bool,
    pub enforce_increasing_h: bool,
    pub constraint_mode: crate::hvf::constraints::ConstraintMode,

    // Extractor
    pub extractor_input_files: Vec<String>,
    pub extractor_best_model: bool,
    pub extractor_geotech: bool,
    pub extractor_rmse: bool,
    pub extractor_format: String,

    // Forward Modeling Params
    pub fmin: f64,
    pub fmax: f64,
    pub nf: usize,
    pub logsam: bool,
    pub use_brocher: bool,
    pub qp: f64,
    pub qs: f64,
    pub nmr: usize,
    pub nml: usize,

    // Additional CLI params now in GUI
    pub love_alpha: f64,
    pub kq: f64,
    pub fref: f64,
    pub nks: usize,
    pub apsv: f64,
    pub ash: f64,
    pub vp_expr: String,
    pub rho_expr: String,

    pub is_running: bool,
    pub log_output: String,
    pub rx: Option<Receiver<String>>,
    pub last_browse_time: f64,
}

impl Default for InversionState {
    fn default() -> Self {
        let os = env::consts::OS;
        let arch = env::consts::ARCH;
        
        let binary_name = match (os, arch) {
            ("windows", _) => "HVf.exe",
            ("macos", "aarch64") => "HVf",
            ("macos", "x86_64") => "HVf_mac_x86",
            ("linux", _) => "HVf_linux",
            _ => "HVf"
        };
        
        let dev_path = PathBuf::from(format!("src/libexec/{}", binary_name));
        let prod_path = PathBuf::from(format!("libexec/{}", binary_name));
        let bundle_path = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|parent| parent.join("libexec").join(binary_name)));
        
        let hvf_default_path = if let Some(bp) = bundle_path.filter(|p| p.exists()) {
            bp.to_string_lossy().to_string()
        } else if dev_path.exists() {
            dev_path.to_string_lossy().to_string()
        } else if prod_path.exists() {
            prod_path.to_string_lossy().to_string()
        } else {
            binary_name.to_string()
        };
        
        Self {
            active_tab: ActiveTab::Explorer,
            explorer_dir: std::env::current_dir().ok(),
            open_tabs: vec![],
            active_tab_index: 0,
            
            run_mode: RunMode::Inversion,
            inv_method: InversionMethod::RjMcmc,
            fwd_engine: ForwardEngine::Herak,
            
            bounds_source: DataSource::File,
            bounds_manual_nlayers: 3,
            bounds_manual_h: vec![[5.0, 30.0], [10.0, 50.0]],
            bounds_manual_vs: vec![[100.0, 300.0], [300.0, 600.0], [500.0, 900.0]],
            
            initial_model_source: OptionalDataSource::None,
            initial_model_manual: vec![
                crate::hvf::model::Layer { thickness: 15.0, vp: 600.0, vs: 200.0, density: 1800.0, qp: None, qs: None },
                crate::hvf::model::Layer { thickness: 30.0, vp: 1200.0, vs: 450.0, density: 1900.0, qp: None, qs: None },
                crate::hvf::model::Layer { thickness: 0.0, vp: 2000.0, vs: 700.0, density: 2000.0, qp: None, qs: None },
            ],

            obs_file: String::new(),
            bounds_file: String::new(),
            initial_model_file: String::new(),
            earth_model_file: String::new(),
            freq_file: String::new(),
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

            pso_pop: 50,
            pso_iter: 100,
            pso_c1: 2.0,
            pso_c2: 2.0,
            pso_w: 0.9,

            sa_t_initial: 10.0,
            sa_t_final: 1e-4,
            sa_cooling_rate: 0.95,

            lm_lambda: 1.0,
            occam_alpha: 0.1,

            enforce_increasing_vs: false,
            enforce_increasing_h: false,
            constraint_mode: crate::hvf::constraints::ConstraintMode::Repair,

            fmin: 0.5,
            fmax: 20.0,
            nf: 100,
            logsam: false,
            use_brocher: true,
            qp: 100.0,
            qs: 50.0,
            nmr: 5,
            nml: 5,

            love_alpha: 0.5,
            kq: 0.25,
            fref: 1.0,
            nks: 200,
            apsv: 0.005,
            ash: 0.01,
            vp_expr: String::new(),
            rho_expr: String::new(),

            is_running: false,
            log_output: String::new(),
            rx: None,
            last_browse_time: 0.0,
            
            extractor_input_files: Vec::new(),
            extractor_best_model: true,
            extractor_geotech: true,
            extractor_rmse: true,
            extractor_format: "csv".to_string(),
        }
    }
}

pub fn show_inversion_panel(ctx: &egui::Context, state: &mut InversionState) {
    // Poll messages from background thread
    poll_messages(state);

    // Activity Bar (far left)
    egui::SidePanel::left("inv_activity_bar")
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
                
                let mut extr_btn = egui::Button::new(egui::RichText::new("📥").size(20.0)).min_size(tab_size);
                if state.active_tab == ActiveTab::Extractor { extr_btn = extr_btn.fill(ui.visuals().selection.bg_fill); }
                if ui.add(extr_btn).on_hover_text("Data Extractor").clicked() {
                    state.active_tab = ActiveTab::Extractor;
                }
            });
        });

    // Main Sidebar
    egui::SidePanel::left("left_panel")
        .resizable(true)
        .default_width(350.0)
        .width_range(250.0..=600.0)
        .show(ctx, |ui| {
            show_left_panel(ui, state);
        });

    show_central_view(ctx, state);

    // Request repaint while running to keep polling
    if state.is_running {
        ctx.request_repaint();
    }
}

fn poll_messages(state: &mut InversionState) {
    if let Some(ref rx) = state.rx {
        while let Ok(msg) = rx.try_recv() {
            if msg == "DONE" {
                state.is_running = false;
                state.log_output.push_str("\n--- Computation Finished ---\n");
                
                // Try to load the result file and create a visualization tab
                if !state.output_file.is_empty() {
                    let mut out_path = PathBuf::from(&state.output_file);
                    if !out_path.is_absolute() {
                        if let Some(ref edir) = state.explorer_dir {
                            out_path = edir.join(out_path);
                        }
                    }
                    if out_path.exists() {
                        if let Ok(content) = std::fs::read_to_string(&out_path) {
                            // Try to parse as InversionResult (PSO/LM/Occam)
                            if let Ok(result) = serde_json::from_str::<InversionResult>(&content) {
                                // Read observation data
                                if !state.obs_file.is_empty() {
                                    let mut obs_path = PathBuf::from(&state.obs_file);
                                    if !obs_path.is_absolute() {
                                        if let Some(ref edir) = state.explorer_dir {
                                            obs_path = edir.join(obs_path);
                                        }
                                    }
                                    if let Ok((obs_f, obs_h)) = crate::hvf::inversion::read_obs_data(&obs_path, -1.0, -1.0) {
                                        let viz = PsoVizData {
                                            obs_freqs: obs_f,
                                            obs_hvs: obs_h,
                                            result,
                                        };
                                        let tab_name = out_path.file_name().unwrap_or_default().to_string_lossy().to_string();
                                        let tab = EditorTab::PsoResult(format!("📊 {}", tab_name), viz);
                                        // Replace if exists, otherwise add
                                        if let Some(pos) = state.open_tabs.iter().position(|t| match t {
                                            EditorTab::PsoResult(n, _) => n == &format!("📊 {}", tab_name),
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
                        }
                    }
                }
            } else {
                state.log_output.push_str(&msg);
                state.log_output.push('\n');
            }
        }
    }
}

fn show_left_panel(ui: &mut egui::Ui, state: &mut InversionState) {
    ui.add_space(10.0);
    match state.active_tab {
        ActiveTab::Explorer => {
            ui.heading("Explorer");
            ui.separator();
            egui::ScrollArea::vertical().id_salt("left_scroll").show(ui, |ui| {
                show_explorer_tab(ui, state);
            });
        }
        ActiveTab::Process => {
            ui.heading("Process & Run");
            ui.separator();
            egui::ScrollArea::vertical().id_salt("left_scroll").show(ui, |ui| {
                show_process_tab(ui, state);
            });
        }
        ActiveTab::Extractor => {
            ui.heading("Data Extractor");
            ui.separator();
            egui::ScrollArea::vertical().id_salt("left_scroll").show(ui, |ui| {
                show_extractor_tab(ui, state);
            });
        }
    }
}

fn show_extractor_tab(ui: &mut egui::Ui, state: &mut InversionState) {
    ui.label("Extract information from .json and .jsonl output files into .csv or .txt.");
    ui.add_space(10.0);

    // 1. Input Selection
    ui.group(|ui| {
        ui.heading("Input Files");
        ui.add_space(5.0);
        
        if ui.button("Select File(s)...").clicked() {
            if let Some(paths) = rfd::FileDialog::new()
                .add_filter("Inversion Results", &["json", "jsonl"])
                .pick_files()
            {
                for p in paths {
                    let s = p.to_string_lossy().to_string();
                    if !state.extractor_input_files.contains(&s) {
                        state.extractor_input_files.push(s);
                    }
                }
            }
        }
        
        ui.add_space(5.0);
        if !state.extractor_input_files.is_empty() {
            ui.label(format!("{} files selected:", state.extractor_input_files.len()));
            let mut to_remove = None;
            egui::ScrollArea::vertical().id_salt("extr_files").max_height(100.0).show(ui, |ui| {
                for (i, file) in state.extractor_input_files.iter().enumerate() {
                    ui.horizontal(|ui| {
                        if ui.button("❌").clicked() {
                            to_remove = Some(i);
                        }
                        let name = PathBuf::from(file).file_name().unwrap_or_default().to_string_lossy().to_string();
                        ui.label(name);
                    });
                }
            });
            if let Some(idx) = to_remove {
                state.extractor_input_files.remove(idx);
            }
            if ui.button("Clear All").clicked() {
                state.extractor_input_files.clear();
            }
        } else {
            ui.label(egui::RichText::new("No files selected.").italics().weak());
        }
    });
    
    ui.add_space(10.0);

    // 2. Parameters to Extract
    ui.group(|ui| {
        ui.heading("Parameters to Extract");
        ui.add_space(5.0);
        
        ui.checkbox(&mut state.extractor_best_model, "Best Model Layers (H, Vs, Vp, Rho)");
        ui.checkbox(&mut state.extractor_geotech, "Geotechnical Parameters (Vs30, Z800)");
        ui.checkbox(&mut state.extractor_rmse, "Inversion RMSE / Cost");
    });
    
    ui.add_space(10.0);
    
    // 3. Format Selection
    ui.group(|ui| {
        ui.heading("Output Format");
        ui.horizontal(|ui| {
            ui.radio_value(&mut state.extractor_format, "csv".to_string(), "CSV");
            ui.radio_value(&mut state.extractor_format, "txt".to_string(), "TXT");
        });
    });
    
    ui.add_space(10.0);
    
    // 4. Execution
    if ui.button("📥 Extract & Save Data").clicked() {
        if state.extractor_input_files.is_empty() {
            state.log_output = "Error: Please select at least one input file.".to_string();
        } else {
            if let Some(save_path) = rfd::FileDialog::new()
                .add_filter("Data", &[&state.extractor_format])
                .save_file()
            {
                match run_extraction(state, &save_path) {
                    Ok(_) => {
                        state.log_output = format!("Extraction successful. Saved to {:?}", save_path);
                    }
                    Err(e) => {
                        state.log_output = format!("Extraction error: {}", e);
                    }
                }
            }
        }
    }
}

fn show_explorer_tab(ui: &mut egui::Ui, state: &mut InversionState) {
    // Status bar: selected input file
    if !state.obs_file.is_empty() {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("📌 Input:").strong());
                let basename = PathBuf::from(&state.obs_file);
                let name = basename.file_name().unwrap_or_default().to_string_lossy();
                ui.label(egui::RichText::new(name.to_string()).color(egui::Color32::from_rgb(0, 120, 215)));
                if ui.small_button("❌").on_hover_text("Deselect input file").clicked() {
                    state.obs_file.clear();
                }
            });
        });
        ui.add_space(3.0);
    }
    
    ui.horizontal(|ui| {
        if ui.button("Open Folder...").clicked() {
            if let Some(path) = rfd::FileDialog::new().pick_folder() {
                state.explorer_dir = Some(path);
            }
        }
        if let Some(dir) = &state.explorer_dir {
            ui.label(dir.display().to_string());
        }
    });
    ui.separator();

    if let Some(dir) = &state.explorer_dir {
        let dir_clone = dir.clone();
        render_directory(ui, &dir_clone, state);
    } else {
        ui.label("No folder selected.");
    }
}

fn render_directory(ui: &mut egui::Ui, path: &Path, state: &mut InversionState) {
    if let Ok(entries) = std::fs::read_dir(path) {
        let mut paths: Vec<_> = entries.filter_map(|e| e.ok()).map(|e| e.path()).collect();
        paths.sort_by(|a, b| {
            let a_is_dir = a.is_dir();
            let b_is_dir = b.is_dir();
            if a_is_dir && !b_is_dir { std::cmp::Ordering::Less }
            else if !a_is_dir && b_is_dir { std::cmp::Ordering::Greater }
            else { a.file_name().cmp(&b.file_name()) }
        });

        for p in paths {
            let name = p.file_name().unwrap_or_default().to_string_lossy().into_owned();
            if name.starts_with('.') { continue; }
            
            if p.is_dir() {
                egui::collapsing_header::CollapsingState::load_with_default_open(
                    ui.ctx(),
                    ui.id().with(p.display().to_string()),
                    false,
                )
                .show_header(ui, |ui| {
                    ui.label(format!("📁 {}", name));
                })
                .body(|ui| {
                    render_directory(ui, &p, state);
                });
            } else {
                let is_jsonl = name.ends_with(".jsonl");
                let is_obs = name.ends_with(".csv") || name.ends_with(".hv") || name.ends_with(".txt");
                let is_selected = state.obs_file == p.to_string_lossy();
                let icon = if is_jsonl { "📊" } else if is_selected { "📌" } else { "📄" };
                
                let label = ui.selectable_label(is_selected, format!("{} {}", icon, name));
                
                if label.double_clicked() {
                    open_file(&p, state);
                }
                
                // Right-click context menu
                label.context_menu(|ui| {
                    if ui.button("📂 Open File").clicked() {
                        open_file(&p, state);
                        ui.close_menu();
                    }
                    
                    if is_obs {
                        ui.separator();
                        if ui.button("📌 Select as Input File").clicked() {
                            state.obs_file = p.to_string_lossy().to_string();
                            // Parse f0 defaults from header
                            parse_obs_header(state);
                            ui.close_menu();
                        }
                        if ui.button("📈 View HVSR Curve").clicked() {
                            open_obs_curve(&p, state);
                            ui.close_menu();
                        }
                    }
                    
                    let is_json = name.ends_with(".json");
                    if is_jsonl || is_json {
                        ui.separator();
                        if ui.button("📊 View Inversion Results").clicked() {
                            open_file(&p, state);
                            ui.close_menu();
                        }
                    }
                });
            }
        }
    }
}

fn parse_obs_header(state: &mut InversionState) {
    if let Ok(content) = std::fs::read_to_string(&state.obs_file) {
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with('#') {
                let lower = line.to_lowercase();
                if lower.contains("f0 from average") || lower.contains("median curve peak frequency") {
                    let parts: Vec<&str> = line.split(|c: char| c == '\t' || c == ',').collect();
                    if parts.len() >= 2 {
                        if let Ok(val) = parts[1].trim().parse::<f64>() {
                            state.f0_min = (val * 0.8 * 100.0).round() / 100.0;
                            state.f0_max = (val * 1.2 * 100.0).round() / 100.0;
                            state.f0_weight = 1.0;
                        }
                    }
                }
            }
        }
    }
}

fn open_obs_curve(path: &Path, state: &mut InversionState) {
    if let Ok((freqs, hvs)) = crate::hvf::inversion::read_obs_data(path, -1.0, -1.0) {
        let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
        let tab = EditorTab::ObsCurve(format!("📈 {}", name), freqs, hvs);
        if let Some(pos) = state.open_tabs.iter().position(|t| match t {
            EditorTab::ObsCurve(n, _, _) => n == &format!("📈 {}", name),
            _ => false,
        }) {
            state.active_tab_index = pos;
        } else {
            state.open_tabs.push(tab);
            state.active_tab_index = state.open_tabs.len() - 1;
        }
    }
}

fn open_file(path: &Path, state: &mut InversionState) {
    let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
    
    // First, try parsing as a single JSON object (for PSO, SA, Occam, LM)
    if name.ends_with(".json") || name.ends_with(".jsonl") {
        if let Ok(content) = std::fs::read_to_string(path) {
            match serde_json::from_str::<InversionResult>(&content) {
                Ok(result) => {
                    let mut obs_f = Vec::new();
                    let mut obs_h = Vec::new();
                if !state.obs_file.is_empty() {
                    if let Ok((f, h)) = crate::hvf::inversion::read_obs_data(std::path::Path::new(&state.obs_file), -1.0, -1.0) {
                        obs_f = f;
                        obs_h = h;
                    }
                }
                let viz = PsoVizData { obs_freqs: obs_f, obs_hvs: obs_h, result };
                let tab = EditorTab::PsoResult(format!("📊 {}", name), viz);
                if let Some(pos) = state.open_tabs.iter().position(|t| match t {
                    EditorTab::PsoResult(n, _) => n == &format!("📊 {}", name),
                    _ => false,
                }) {
                    state.active_tab_index = pos;
                } else {
                    state.open_tabs.push(tab);
                    state.active_tab_index = state.open_tabs.len() - 1;
                }
                return;
            }
            Err(e) => {
                eprintln!("Failed to parse InversionResult from {}: {}", name, e);
            }
        }
            
        // Try deterministic output
            if let Ok(hvsr_out) = serde_json::from_str::<crate::hvf::output::HvsrOutput>(&content) {
                let tab = EditorTab::DeterministicCurve(name.clone(), hvsr_out);
                if let Some(pos) = state.open_tabs.iter().position(|t| match t {
                    EditorTab::DeterministicCurve(n, _) => n == &name,
                    _ => false,
                }) {
                    state.active_tab_index = pos;
                } else {
                    state.open_tabs.push(tab);
                    state.active_tab_index = state.open_tabs.len() - 1;
                }
                return;
            }
        }
    }
    
    // Second, if it wasn't a standard JSON result and ends with .jsonl, load as RJMCMC JSONL
    if name.ends_with(".jsonl") {
        match load_and_process_data(&path.to_string_lossy().to_string(), &state.obs_file) {
            Ok(viz) => {
                let tab = EditorTab::Visualization(name.clone(), viz);
                if let Some(pos) = state.open_tabs.iter().position(|t| match t {
                    EditorTab::Visualization(n, _) => n == &name,
                    _ => false,
                }) {
                    state.active_tab_index = pos;
                } else {
                    state.open_tabs.push(tab);
                    state.active_tab_index = state.open_tabs.len() - 1;
                }
            }
            Err(e) => {
                state.log_output = format!("Error loading JSONL: {}", e);
            }
        }
    } else {
        // Fallback to ASCII
        if let Ok(content) = std::fs::read_to_string(path) {
            let tab = EditorTab::Ascii(path.to_path_buf(), content, false);
            if let Some(pos) = state.open_tabs.iter().position(|t| match t {
                EditorTab::Ascii(p, _, _) => p == path,
                _ => false,
            }) {
                state.active_tab_index = pos;
            } else {
                state.open_tabs.push(tab);
                state.active_tab_index = state.open_tabs.len() - 1;
            }
        }
    }
}

fn show_process_tab(ui: &mut egui::Ui, state: &mut InversionState) {
    // Status: Selected input file
    ui.group(|ui| {
        ui.label(egui::RichText::new("📌 Selected Input File").strong());
        if state.obs_file.is_empty() {
            ui.label(egui::RichText::new("No file selected. Right-click a file in Explorer → 'Select as Input File'.").italics().weak());
        } else {
            let basename = PathBuf::from(&state.obs_file);
            let name = basename.file_name().unwrap_or_default().to_string_lossy();
            ui.label(egui::RichText::new(name.to_string()).color(egui::Color32::from_rgb(0, 120, 215)));
        }
    });
    ui.add_space(5.0);
    
    ui.heading("Run Mode & Method");
    egui::Grid::new("inv_mode_grid").num_columns(2).spacing([10.0, 10.0]).show(ui, |ui| {
        ui.label("Operation Mode:");
        ui.horizontal(|ui| {
            ui.radio_value(&mut state.run_mode, RunMode::Inversion, "Inversion");
            ui.radio_value(&mut state.run_mode, RunMode::ForwardModeling, "Forward Modeling");
        });
        ui.end_row();

        if state.run_mode == RunMode::Inversion {
            ui.label("Inversion Method:");
            egui::ComboBox::from_id_salt("inv_method_combo")
                .selected_text(match state.inv_method {
                    InversionMethod::RjMcmc => "Transdimensional RJ-MCMC",
                    InversionMethod::Pso => "Particle Swarm Optimization (PSO)",
                    InversionMethod::SimulatedAnnealing => "Simulated Annealing (SA)",
                    InversionMethod::LevenbergMarquardt => "Levenberg-Marquardt",
                    InversionMethod::Occam => "Occam's Inversion",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut state.inv_method, InversionMethod::RjMcmc, "Transdimensional RJ-MCMC");
                    ui.selectable_value(&mut state.inv_method, InversionMethod::Pso, "Particle Swarm Optimization (PSO)");
                    ui.selectable_value(&mut state.inv_method, InversionMethod::SimulatedAnnealing, "Simulated Annealing (SA)");
                    ui.selectable_value(&mut state.inv_method, InversionMethod::LevenbergMarquardt, "Levenberg-Marquardt");
                    ui.selectable_value(&mut state.inv_method, InversionMethod::Occam, "Occam's Inversion");
                });
            ui.end_row();
        }
        
        ui.label("Forward Engine:");
        egui::ComboBox::from_id_salt("fwd_engine_combo")
            .selected_text(match state.fwd_engine {
                ForwardEngine::Herak => "Herak (Body Wave Approach)",
                ForwardEngine::Dfa => "DFA (Diffused Field Approach)",
                ForwardEngine::Ellipticity => "Ellipticity (Rayleigh only)",
                ForwardEngine::EllipticityLove => "Ellipticity (Rayleigh + Love)",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut state.fwd_engine, ForwardEngine::Herak, "Herak (Body Wave Approach)");
                ui.selectable_value(&mut state.fwd_engine, ForwardEngine::Dfa, "DFA (Diffused Field Approach)");
                ui.selectable_value(&mut state.fwd_engine, ForwardEngine::Ellipticity, "Ellipticity (Rayleigh only)");
                ui.selectable_value(&mut state.fwd_engine, ForwardEngine::EllipticityLove, "Ellipticity (Rayleigh + Love)");
            });
        ui.end_row();
    });
    
    ui.separator();
    
    // Input/Output - only show browse for files NOT covered by explorer selection
    ui.heading("Input / Output Config");
    egui::Grid::new("inv_io_grid").num_columns(2).spacing([10.0, 10.0]).show(ui, |ui| {
        if state.run_mode == RunMode::Inversion {
            if state.inv_method != InversionMethod::RjMcmc {
                ui.label("Bounds Source:");
                ui.horizontal(|ui| {
                    ui.radio_value(&mut state.bounds_source, DataSource::File, "From File");
                    ui.radio_value(&mut state.bounds_source, DataSource::Manual, "Manual (Grid)");
                });
                ui.end_row();

                if state.bounds_source == DataSource::File {
                    ui.label("Bounds File (.json):");
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut state.bounds_file);
                        let current_time = ui.input(|i| i.time);
                        if ui.button("Browse##bounds").clicked() && current_time - state.last_browse_time > 1.0 {
                            state.last_browse_time = current_time;
                            if let Some(path) = FileDialog::new().add_filter("JSON", &["json"]).pick_file() {
                                state.bounds_file = path.to_string_lossy().to_string();
                            }
                        }
                    });
                    ui.end_row();
                }

                ui.label("Initial Model Source:");
                ui.horizontal(|ui| {
                    ui.radio_value(&mut state.initial_model_source, OptionalDataSource::None, "None (Use Bounds Center)");
                    ui.radio_value(&mut state.initial_model_source, OptionalDataSource::File, "From File");
                    ui.radio_value(&mut state.initial_model_source, OptionalDataSource::Manual, "Manual (Grid)");
                });
                ui.end_row();

                if state.initial_model_source == OptionalDataSource::File {
                    ui.label("Initial Model (.txt / .json):");
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut state.initial_model_file);
                        let current_time = ui.input(|i| i.time);
                        if ui.button("Browse##init").clicked() && current_time - state.last_browse_time > 1.0 {
                            state.last_browse_time = current_time;
                            if let Some(path) = FileDialog::new().add_filter("Model", &["txt", "mod", "json"]).pick_file() {
                                state.initial_model_file = path.to_string_lossy().to_string();
                            }
                        }
                    });
                    ui.end_row();
                }
            }
        } else {
            ui.label("Earth Model File (.json / .txt):");
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut state.earth_model_file);
                let current_time = ui.input(|i| i.time);
                if ui.button("Browse##earth").clicked() && current_time - state.last_browse_time > 1.0 {
                    state.last_browse_time = current_time;
                    if let Some(path) = FileDialog::new().add_filter("Model", &["json", "txt"]).pick_file() {
                        state.earth_model_file = path.to_string_lossy().to_string();
                    }
                }
            });
            ui.end_row();

            ui.label("Frequency File (.txt) [Optional]:");
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut state.freq_file);
                let current_time = ui.input(|i| i.time);
                if ui.button("Browse##freq").clicked() && current_time - state.last_browse_time > 1.0 {
                    state.last_browse_time = current_time;
                    if let Some(path) = FileDialog::new().add_filter("Text", &["txt"]).pick_file() {
                        state.freq_file = path.to_string_lossy().to_string();
                    }
                }
            });
            ui.end_row();
        }

        ui.label("Output File:");
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut state.output_file);
            let current_time = ui.input(|i| i.time);
            if ui.button("Browse...").clicked() && current_time - state.last_browse_time > 1.0 {
                state.last_browse_time = current_time;
                let mut dialog = FileDialog::new().add_filter("JSON", &["json", "jsonl"]);
                if let Some(ref edir) = state.explorer_dir {
                    dialog = dialog.set_directory(edir);
                }
                if let Some(path) = dialog.save_file() {
                    state.output_file = path.to_string_lossy().to_string();
                }
            }
        });
        ui.end_row();
    });
    
    ui.separator();
    
    if state.run_mode == RunMode::Inversion && state.inv_method != InversionMethod::RjMcmc {
        if state.bounds_source == DataSource::Manual {
            egui::CollapsingHeader::new("⚙ Manual Bounds Editor")
                .default_open(true)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Number of Layers:");
                        if ui.add(egui::DragValue::new(&mut state.bounds_manual_nlayers).clamp_range(2..=20)).changed() {
                            // New layers inherit the deepest layer's bounds so manual bounds stay monotonic.
                            let last_h = state.bounds_manual_h.last().copied().unwrap_or([10.0, 50.0]);
                            let last_vs = state.bounds_manual_vs.last().copied().unwrap_or([200.0, 500.0]);
                            state.bounds_manual_h.resize(state.bounds_manual_nlayers - 1, last_h);
                            state.bounds_manual_vs.resize(state.bounds_manual_nlayers, last_vs);
                        }
                    });
                    ui.add_space(5.0);
                    egui::Grid::new("manual_bounds_grid").striped(true).show(ui, |ui| {
                        ui.label("Layer");
                        ui.label("Thickness Min");
                        ui.label("Thickness Max");
                        ui.label("Vs Min");
                        ui.label("Vs Max");
                        ui.end_row();

                        for i in 0..state.bounds_manual_nlayers {
                            ui.label(format!("{}", i + 1));
                            if i < state.bounds_manual_nlayers - 1 {
                                ui.add(egui::DragValue::new(&mut state.bounds_manual_h[i][0]).speed(1.0));
                                ui.add(egui::DragValue::new(&mut state.bounds_manual_h[i][1]).speed(1.0));
                            } else {
                                ui.label("∞");
                                ui.label("∞");
                            }
                            ui.add(egui::DragValue::new(&mut state.bounds_manual_vs[i][0]).speed(10.0));
                            ui.add(egui::DragValue::new(&mut state.bounds_manual_vs[i][1]).speed(10.0));
                            ui.end_row();
                        }
                    });
                    ui.add_space(5.0);
                    if ui.button("💾 Save Bounds to File...").clicked() {
                        if let Some(path) = FileDialog::new().add_filter("JSON", &["json"]).set_file_name("my_bounds.json").save_file() {
                            let bounds_data = crate::hvf::inversion::BoundsData {
                                nlayers: state.bounds_manual_nlayers,
                                h_bounds: state.bounds_manual_h.clone(),
                                vs_bounds: state.bounds_manual_vs.clone(),
                            };
                            if let Ok(json_str) = serde_json::to_string_pretty(&bounds_data) {
                                let _ = std::fs::write(path, json_str);
                            }
                        }
                    }
                });
            ui.separator();
        }
        
        if state.initial_model_source == OptionalDataSource::Manual {
            egui::CollapsingHeader::new("⚙ Manual Initial Model Editor")
                .default_open(true)
                .show(ui, |ui| {
                    let mut nlayers = state.initial_model_manual.len();
                    ui.horizontal(|ui| {
                        ui.label("Number of Layers:");
                        if ui.add(egui::DragValue::new(&mut nlayers).clamp_range(2..=20)).changed() {
                            state.initial_model_manual.resize(nlayers, crate::hvf::model::Layer {
                                thickness: 10.0, vp: 1000.0, vs: 500.0, density: 1800.0, qp: None, qs: None
                            });
                        }
                    });
                    ui.add_space(5.0);
                    egui::Grid::new("manual_init_model_grid").striped(true).show(ui, |ui| {
                        ui.label("Layer");
                        ui.label("Thickness");
                        ui.label("Vs");
                        ui.label("Vp");
                        ui.label("Density");
                        ui.end_row();

                        for i in 0..nlayers {
                            ui.label(format!("{}", i + 1));
                            if i < nlayers - 1 {
                                ui.add(egui::DragValue::new(&mut state.initial_model_manual[i].thickness).speed(1.0));
                            } else {
                                state.initial_model_manual[i].thickness = 0.0;
                                ui.label("∞");
                            }
                            ui.add(egui::DragValue::new(&mut state.initial_model_manual[i].vs).speed(10.0));
                            ui.add(egui::DragValue::new(&mut state.initial_model_manual[i].vp).speed(10.0));
                            ui.add(egui::DragValue::new(&mut state.initial_model_manual[i].density).speed(10.0));
                            ui.end_row();
                        }
                    });
                    ui.add_space(5.0);
                    if ui.button("💾 Save Model to File...").clicked() {
                        if let Some(path) = FileDialog::new().add_filter("Text", &["txt"]).set_file_name("my_model.txt").save_file() {
                            let mut content = String::new();
                            content.push_str(&format!("{}\n", state.initial_model_manual.len()));
                            for layer in &state.initial_model_manual {
                                content.push_str(&format!("{} {} {} {}\n", layer.thickness, layer.vp, layer.vs, layer.density));
                            }
                            let _ = std::fs::write(path, content);
                        }
                    }
                });
            ui.separator();
        }
    }
    
    if state.run_mode == RunMode::Inversion && state.inv_method == InversionMethod::RjMcmc {
        egui::CollapsingHeader::new("⚙ RJ-MCMC Parameters & Configuration")
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
                    
                    ui.label("Total Depth (m):");
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut state.min_total_depth).speed(10.0).prefix("Min: "));
                        ui.add(egui::DragValue::new(&mut state.max_total_depth).speed(10.0).prefix("Max: "));
                    });
                    ui.end_row();
                    
                    ui.label("Layers:");
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut state.min_layers).speed(1).prefix("Min: "));
                        ui.add(egui::DragValue::new(&mut state.max_layers).speed(1).prefix("Max: "));
                    });
                    ui.end_row();
                    
                    ui.label("f0 Range (Hz):");
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut state.f0_min).speed(0.1).prefix("Min: "));
                        ui.add(egui::DragValue::new(&mut state.f0_max).speed(0.1).prefix("Max: "));
                    });
                    ui.end_row();
                });
                
                ui.separator();
                ui.heading("Advanced Priors & Weights");
                egui::Grid::new("inv_adv_grid").num_columns(2).spacing([10.0, 10.0]).show(ui, |ui| {
                    ui.label("Prob Ascending Vs:");
                    ui.add(egui::Slider::new(&mut state.prob_asc_vs, 0.0..=1.0));
                    ui.end_row();
                    
                    ui.label("Prob Ascending Thick:");
                    ui.add(egui::Slider::new(&mut state.prob_asc_h, 0.0..=1.0));
                    ui.end_row();
                    
                    ui.label("f0 Likelihood Weight:");
                    ui.add(egui::Slider::new(&mut state.f0_weight, 0.0..=5.0));
                    ui.end_row();
                    
                    ui.label("Amplitude Weight:");
                    ui.add(egui::Slider::new(&mut state.a0_weight, 0.0..=5.0));
                    ui.end_row();
                    
                    ui.label("Constrain Avg Vs:");
                    ui.checkbox(&mut state.use_avg_vs, "Enable");
                    ui.end_row();
                    
                    if state.use_avg_vs {
                        ui.label("Avg Vs Settings:");
                        ui.horizontal(|ui| {
                            ui.add(egui::DragValue::new(&mut state.avg_vs_depth).speed(1.0).prefix("Depth: "));
                            ui.add(egui::DragValue::new(&mut state.avg_vs_min).speed(10.0).prefix("Min: "));
                            ui.add(egui::DragValue::new(&mut state.avg_vs_max).speed(10.0).prefix("Max: "));
                        });
                        ui.end_row();
                    }
                });
            });
    }

    if state.run_mode == RunMode::Inversion && state.inv_method == InversionMethod::Pso {
        egui::CollapsingHeader::new("⚙ PSO Configuration")
            .default_open(true)
            .show(ui, |ui| {
                egui::Grid::new("pso_grid").num_columns(2).spacing([10.0, 10.0]).show(ui, |ui| {
                    ui.label("Population Size:");
                    ui.add(egui::DragValue::new(&mut state.pso_pop).speed(1));
                    ui.end_row();
                    
                    ui.label("Max Iterations:");
                    ui.add(egui::DragValue::new(&mut state.pso_iter).speed(10));
                    ui.end_row();

                    ui.label("Cognitive (c1):");
                    ui.add(egui::DragValue::new(&mut state.pso_c1).speed(0.1));
                    ui.end_row();

                    ui.label("Social (c2):");
                    ui.add(egui::DragValue::new(&mut state.pso_c2).speed(0.1));
                    ui.end_row();

                    ui.label("Inertia Weight (w):");
                    ui.add(egui::DragValue::new(&mut state.pso_w).speed(0.1));
                    ui.end_row();
                });
            });
    }

    if state.run_mode == RunMode::Inversion && state.inv_method == InversionMethod::SimulatedAnnealing {
        egui::CollapsingHeader::new("⚙ Simulated Annealing Configuration")
            .default_open(true)
            .show(ui, |ui| {
                egui::Grid::new("sa_grid").num_columns(2).spacing([10.0, 10.0]).show(ui, |ui| {
                    ui.label("Max Iterations:");
                    ui.add(egui::DragValue::new(&mut state.pso_iter).speed(10));
                    ui.end_row();

                    ui.label("Initial Temperature:");
                    ui.add(egui::DragValue::new(&mut state.sa_t_initial).speed(1.0));
                    ui.end_row();

                    ui.label("Final Temperature:");
                    ui.add(egui::DragValue::new(&mut state.sa_t_final).speed(0.001));
                    ui.end_row();

                    ui.label("Cooling Rate:");
                    ui.add(egui::DragValue::new(&mut state.sa_cooling_rate).speed(0.01).range(0.5..=0.999));
                    ui.end_row();
                });
            });
    }

    if state.run_mode == RunMode::Inversion && state.inv_method == InversionMethod::LevenbergMarquardt {
        egui::CollapsingHeader::new("⚙ LM Configuration")
            .default_open(true)
            .show(ui, |ui| {
                egui::Grid::new("lm_grid").num_columns(2).spacing([10.0, 10.0]).show(ui, |ui| {
                    ui.label("Damping Lambda:");
                    ui.add(egui::DragValue::new(&mut state.lm_lambda).speed(0.1));
                    ui.end_row();
                });
            });
    }

    if state.run_mode == RunMode::Inversion && state.inv_method == InversionMethod::Occam {
        egui::CollapsingHeader::new("⚙ Occam's Configuration")
            .default_open(true)
            .show(ui, |ui| {
                egui::Grid::new("occam_grid").num_columns(2).spacing([10.0, 10.0]).show(ui, |ui| {
                    ui.label("Smoothing Alpha:");
                    ui.add(egui::DragValue::new(&mut state.occam_alpha).speed(0.01));
                    ui.end_row();
                });
            });
    }

    if state.run_mode == RunMode::Inversion && state.inv_method != InversionMethod::RjMcmc {
        egui::CollapsingHeader::new("⚙ Physical Constraints")
            .default_open(true)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut state.enforce_increasing_vs, "Vs non-decreasing with depth");
                });
                ui.horizontal(|ui| {
                    ui.checkbox(&mut state.enforce_increasing_h, "Thickness non-decreasing with depth");
                });

                let any_active = state.enforce_increasing_vs || state.enforce_increasing_h;
                ui.add_enabled_ui(any_active, |ui| {
                    ui.horizontal(|ui| {
                        use crate::hvf::constraints::ConstraintMode;
                        ui.label("Mode:");
                        egui::ComboBox::from_id_salt("constraint_mode_combo")
                            .selected_text(state.constraint_mode.label())
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut state.constraint_mode, ConstraintMode::Repair, ConstraintMode::Repair.label())
                                    .on_hover_text("Models are sorted (PSO/SA) or isotonically projected (LM/Occam) and clamped before evaluation. The repaired model is the one stored.");
                                ui.selectable_value(&mut state.constraint_mode, ConstraintMode::Penalty, ConstraintMode::Penalty.label())
                                    .on_hover_text("Legacy: violating models get a cost of 1e6 and are effectively rejected.");
                            });
                    });

                    if state.constraint_mode == crate::hvf::constraints::ConstraintMode::Repair
                        && matches!(state.inv_method, InversionMethod::LevenbergMarquardt | InversionMethod::Occam)
                    {
                        ui.label(egui::RichText::new("ℹ LM/Occam use isotonic projection (PAVA) instead of sorting.").weak());
                    }
                });

                if any_active && state.bounds_source == DataSource::Manual {
                    let vs_bad = state.enforce_increasing_vs && !crate::hvf::constraints::bounds_are_monotonic(&state.bounds_manual_vs);
                    let h_bad = state.enforce_increasing_h && !crate::hvf::constraints::bounds_are_monotonic(&state.bounds_manual_h);
                    if vs_bad || h_bad {
                        let which = match (vs_bad, h_bad) {
                            (true, true) => "Vs and Thickness",
                            (true, false) => "Vs",
                            _ => "Thickness",
                        };
                        ui.colored_label(
                            egui::Color32::from_rgb(230, 170, 60),
                            format!("⚠ {} bounds are not monotonic with depth. Feasibility cannot be guaranteed; a graded penalty will be applied.", which),
                        );
                    }
                }
            });
    }

    if state.run_mode == RunMode::ForwardModeling || state.inv_method != InversionMethod::RjMcmc {
        egui::CollapsingHeader::new("⚙ Forward / Medium Settings")
            .default_open(true)
            .show(ui, |ui| {
                egui::Grid::new("fwd_grid").num_columns(2).spacing([10.0, 10.0]).show(ui, |ui| {
                    ui.label("Freq Range (Hz):");
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut state.fmin).speed(0.1).prefix("Min: "));
                        ui.add(egui::DragValue::new(&mut state.fmax).speed(0.1).prefix("Max: "));
                    });
                    ui.end_row();

                    if state.run_mode == RunMode::ForwardModeling {
                        ui.label("Number of Freqs (nf):");
                        ui.add(egui::DragValue::new(&mut state.nf).speed(1));
                        ui.end_row();
                        
                        ui.label("Log Freq Sampling:");
                        ui.checkbox(&mut state.logsam, "Enable");
                        ui.end_row();
                    }

                    if state.fwd_engine == ForwardEngine::Dfa || state.fwd_engine == ForwardEngine::Ellipticity || state.fwd_engine == ForwardEngine::EllipticityLove {
                        ui.label("Modes (Rayleigh / Love):");
                        ui.horizontal(|ui| {
                            ui.add(egui::DragValue::new(&mut state.nmr).speed(1).prefix("nmr: "));
                            if state.fwd_engine == ForwardEngine::Dfa || state.fwd_engine == ForwardEngine::EllipticityLove {
                                ui.add(egui::DragValue::new(&mut state.nml).speed(1).prefix("nml: "));
                            }
                        });
                        ui.end_row();
                    }
                    
                    if state.fwd_engine != ForwardEngine::Dfa {
                        ui.label("Quality Factors (Qp / Qs):");
                        ui.horizontal(|ui| {
                            ui.add(egui::DragValue::new(&mut state.qp).speed(10.0).prefix("Qp: "));
                            ui.add(egui::DragValue::new(&mut state.qs).speed(10.0).prefix("Qs: "));
                        });
                        ui.end_row();
                    }

                    if state.fwd_engine == ForwardEngine::EllipticityLove {
                        ui.label("Love Alpha:");
                        ui.add(egui::DragValue::new(&mut state.love_alpha).speed(0.05).range(0.0..=1.0));
                        ui.end_row();
                    }
                    
                    if state.fwd_engine == ForwardEngine::Herak {
                        ui.label("Q Freq Dep (kq):");
                        ui.add(egui::DragValue::new(&mut state.kq).speed(0.05));
                        ui.end_row();
                        
                        ui.label("Ref Freq (fref):");
                        ui.add(egui::DragValue::new(&mut state.fref).speed(0.1));
                        ui.end_row();
                    }
                    
                    if state.fwd_engine == ForwardEngine::Dfa {
                        ui.label("Body Wave Wavenumber (nks):");
                        ui.add(egui::DragValue::new(&mut state.nks).speed(10));
                        ui.end_row();
                        
                        ui.label("P-SV Atten (apsv):");
                        ui.add(egui::DragValue::new(&mut state.apsv).speed(0.001));
                        ui.end_row();
                        
                        ui.label("SH Atten (ash):");
                        ui.add(egui::DragValue::new(&mut state.ash).speed(0.001));
                        ui.end_row();
                    }

                    ui.label("Use Brocher's Eq:");
                    ui.checkbox(&mut state.use_brocher, "Enable (empirical Vp-Vs-Rho)");
                    ui.end_row();

                    if !state.use_brocher {
                        ui.label("Vp Expression:");
                        ui.text_edit_singleline(&mut state.vp_expr);
                        ui.end_row();
                        
                        ui.label("Rho Expression:");
                        ui.text_edit_singleline(&mut state.rho_expr);
                        ui.end_row();
                    }
                });
            });
    }

    ui.add_space(20.0);
    
    if state.is_running {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label("Computation in progress...");
        });
    } else {
        if ui.button("▶ Run Computation").clicked() {
            if state.run_mode == RunMode::Inversion && state.inv_method == InversionMethod::RjMcmc {
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
                        fmin: state.fmin,
                        fmax: state.fmax,
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
                        vp_expr: None,
                        rho_expr: None,
                        method: match state.fwd_engine {
                            ForwardEngine::Herak => "herak".to_string(),
                            ForwardEngine::Dfa => "dfa".to_string(),
                            ForwardEngine::Ellipticity => "ellipticity".to_string(),
                            ForwardEngine::EllipticityLove => "ellipticity-love".to_string(),
                        },
                        love_alpha: 0.5,
                        plot_dir: None,
                    };
                    
                    let (tx, rx) = channel();
                    state.rx = Some(rx);
                    state.is_running = true;
                    
                    thread::spawn(move || {
                        crate::core::rjmcmc::run_inversion(config, tx);
                    });
                }
            } else {
                // Other inversion / forward modeling methods
                state.log_output = format!("Starting CLI Engine...\nOutput file: {}\n", state.output_file);
                
                // Write temporary manual files if needed
                let mut tmp_bounds = None;
                let mut tmp_initial = None;

                if state.run_mode == RunMode::Inversion && state.inv_method != InversionMethod::RjMcmc {
                    if state.bounds_source == DataSource::Manual {
                        let bounds_data = crate::hvf::inversion::BoundsData {
                            nlayers: state.bounds_manual_nlayers,
                            h_bounds: state.bounds_manual_h.clone(),
                            vs_bounds: state.bounds_manual_vs.clone(),
                        };
                        let path = std::env::temp_dir().join("seisbox_manual_bounds.json");
                        if let Ok(json_str) = serde_json::to_string(&bounds_data) {
                            if std::fs::write(&path, json_str).is_ok() {
                                tmp_bounds = Some(path);
                            }
                        }
                    }

                    if state.initial_model_source == OptionalDataSource::Manual {
                        let path = std::env::temp_dir().join("seisbox_manual_init.txt");
                        let mut content = String::new();
                        content.push_str(&format!("{}\n", state.initial_model_manual.len()));
                        for layer in &state.initial_model_manual {
                            content.push_str(&format!("{} {} {} {}\n", layer.thickness, layer.vp, layer.vs, layer.density));
                        }
                        if std::fs::write(&path, content).is_ok() {
                            tmp_initial = Some(path);
                        }
                    }
                }
                
                let config = crate::hvf::cli::Config {
                    fmin: state.fmin,
                    fmax: state.fmax,
                    nf: state.nf,
                    logsam: state.logsam,
                    freq_file: if state.freq_file.is_empty() { None } else { Some(PathBuf::from(&state.freq_file)) },
                    model_file: if state.earth_model_file.is_empty() { None } else { Some(PathBuf::from(&state.earth_model_file)) },
                    model_json: None,
                    method: match state.fwd_engine {
                        ForwardEngine::Herak => "herak".to_string(),
                        ForwardEngine::Dfa => "dfa".to_string(),
                        ForwardEngine::Ellipticity => "ellipticity".to_string(),
                        ForwardEngine::EllipticityLove => "ellipticity-love".to_string(),
                    },
                    use_brocher: state.use_brocher,
                    vp_expr: if state.vp_expr.is_empty() { None } else { Some(state.vp_expr.clone()) },
                    rho_expr: if state.rho_expr.is_empty() { None } else { Some(state.rho_expr.clone()) },
                    love_alpha: state.love_alpha,
                    qp: state.qp,
                    qs: state.qs,
                    kq: state.kq,
                    fref: state.fref,
                    nmr: state.nmr,
                    nml: state.nml,
                    prec: 0.0,
                    nks: state.nks,
                    apsv: state.apsv,
                    ash: state.ash,
                    output_hv: true,
                    output_ph: false,
                    output_gr: false,
                    output_rep: false,
                    output_json: true,
                    output_file: if state.output_file.is_empty() { 
                        None 
                    } else { 
                        let mut pb = PathBuf::from(&state.output_file);
                        if !pb.is_absolute() {
                            if let Some(ref edir) = state.explorer_dir {
                                pb = edir.join(pb);
                            }
                        }
                        Some(pb)
                    },
                    
                    invert: if state.run_mode == RunMode::Inversion {
                        Some(match state.inv_method {
                            InversionMethod::Pso => "pso".to_string(),
                            InversionMethod::LevenbergMarquardt => "lm".to_string(),
                            InversionMethod::Occam => "occam".to_string(),
                            InversionMethod::RjMcmc => "mcmc".to_string(),
                            InversionMethod::SimulatedAnnealing => "sa".to_string(),
                        })
                    } else { None },
                    lm_lambda: state.lm_lambda,
                    occam_alpha: state.occam_alpha,
                    obs: if state.obs_file.is_empty() { None } else { Some(PathBuf::from(&state.obs_file)) },
                    bounds: if state.run_mode == RunMode::Inversion && state.inv_method != InversionMethod::RjMcmc {
                        if state.bounds_source == DataSource::Manual {
                            tmp_bounds
                        } else if state.bounds_file.is_empty() { None } else { Some(PathBuf::from(&state.bounds_file)) }
                    } else { None },
                    initial_model: if state.run_mode == RunMode::Inversion && state.inv_method != InversionMethod::RjMcmc {
                        if state.initial_model_source == OptionalDataSource::Manual {
                            tmp_initial
                        } else if state.initial_model_source == OptionalDataSource::File && !state.initial_model_file.is_empty() { 
                            Some(PathBuf::from(&state.initial_model_file))
                        } else { None }
                    } else { None },
                    mcmc_config: None,
                    pso_pop: state.pso_pop,
                    pso_iter: state.pso_iter,
                    pso_c1: state.pso_c1,
                    pso_c2: state.pso_c2,
                    pso_w: state.pso_w,
                    sa_t_initial: state.sa_t_initial,
                    sa_t_final: state.sa_t_final,
                    sa_cooling_rate: state.sa_cooling_rate,
                    enforce_increasing_vs: state.enforce_increasing_vs,
                    enforce_increasing_h: state.enforce_increasing_h,
                    constraint_mode: state.constraint_mode,
                    plot_dir: None,
                };
                
                let (tx, rx) = channel();
                state.rx = Some(rx);
                state.is_running = true;
                
                thread::spawn(move || {
                    if config.invert.is_some() {
                        let tx_clone = tx.clone();
                        if let Err(e) = crate::hvf::inversion::run_inversion(&config, Some(tx_clone)) {
                            let _ = tx.send(format!("Inversion Error: {}", e));
                        } else {
                            let _ = tx.send("Computation completed successfully.".to_string());
                            let _ = tx.send("DONE".to_string());
                        }
                    } else {
                        // Forward Modeling
                        if config.method == "herak" {
                            let earth_model = if let Some(ref path) = config.model_file {
                                match crate::hvf::model::EarthModel::from_file(path, config.use_brocher, config.vp_expr.as_deref(), config.rho_expr.as_deref()) {
                                    Ok(m) => m,
                                    Err(e) => {
                                        let _ = tx.send(format!("Error reading model file: {}", e));
                                        let _ = tx.send("DONE".to_string());
                                        return;
                                    }
                                }
                            } else {
                                let _ = tx.send("No model file specified for Forward Modeling.".to_string());
                                let _ = tx.send("DONE".to_string());
                                return;
                            };

                            let freqs = match crate::hvf::compute::build_omega_vector(&config) {
                                Ok(w) => w.into_iter().map(|w| w / (2.0 * std::f64::consts::PI)).collect::<Vec<_>>(),
                                Err(e) => {
                                    let _ = tx.send(format!("Error building frequencies: {}", e));
                                    let _ = tx.send("DONE".to_string());
                                    return;
                                }
                            };

                            let _ = tx.send("Computing HVSR (Herak)...".to_string());
                            let hvsr = crate::hvf::herak::compute_hvsr_herak(&earth_model, &freqs, &config);
                            
                            if config.output_json {
                                let out = crate::hvf::output::HvsrOutput {
                                    frequencies: freqs.clone(),
                                    hv_ratio: hvsr.clone(),
                                    rayleigh_phase: None,
                                    love_phase: None,
                                };
                                let json_str = out.to_json();
                                if let Some(ref out_file) = config.output_file {
                                    if let Err(e) = std::fs::write(out_file, json_str) {
                                        let _ = tx.send(format!("Failed to write JSON output: {}", e));
                                    }
                                }
                            }
                            let _ = tx.send("Computation completed successfully.".to_string());
                        } else {
                            if let Err(e) = crate::hvf::compute::run_hvsr(&config) {
                                let _ = tx.send(format!("Error: {}", e));
                            } else {
                                let _ = tx.send("Computation completed successfully.".to_string());
                            }
                        }
                    }
                    let _ = tx.send("DONE".to_string());
                });
            }
        }
    }

    // Save button (shown after inversion is done and result exists)
    if !state.is_running {
        let has_pso_result = state.open_tabs.iter().any(|t| matches!(t, EditorTab::PsoResult(_, _)));
        if has_pso_result {
            ui.add_space(10.0);
            if ui.button("💾 Save Result to File...").clicked() {
                if let Some(path) = FileDialog::new()
                    .add_filter("JSON", &["json"])
                    .set_file_name("inversion_result.json")
                    .save_file() {
                    if let Some(tab) = state.open_tabs.iter().find(|t| matches!(t, EditorTab::PsoResult(_, _))) {
                        if let EditorTab::PsoResult(_, viz) = tab {
                            let json_str = serde_json::to_string_pretty(&viz.result).unwrap_or_default();
                            match std::fs::write(&path, json_str) {
                                Ok(_) => state.log_output = format!("Result saved to {:?}", path),
                                Err(e) => state.log_output = format!("Failed to save: {}", e),
                            }
                        }
                    }
                }
            }
        }
    }
    
    ui.add_space(10.0);
    ui.heading("Log Output");
    egui::ScrollArea::vertical().id_salt("inv_log_scroll").max_height(200.0).show(ui, |ui| {
        ui.add(
            egui::TextEdit::multiline(&mut state.log_output)
                .font(egui::TextStyle::Monospace)
                .desired_width(f32::INFINITY)
                .desired_rows(10)
                .interactive(false)
        );
    });
}

fn show_central_view(ctx: &egui::Context, state: &mut InversionState) {
    egui::CentralPanel::default().show(ctx, |ui| {
        if state.open_tabs.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label(egui::RichText::new("No active tabs.\n\nRight-click files in the Explorer to view curves or select inputs.\nRun inversion to see results here.").italics());
            });
            return;
        }

        // Tab bar
        egui::ScrollArea::horizontal().id_salt("tab_bar_scroll").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.style_mut().spacing.item_spacing = egui::vec2(2.0, 0.0);
                
                let mut close_tab = None;
                for (i, tab) in state.open_tabs.iter().enumerate() {
                    let is_active = i == state.active_tab_index;
                    let (title, is_dirty) = match tab {
                        EditorTab::Visualization(n, _) => (n.clone(), false),
                        EditorTab::DeterministicCurve(n, _) => (n.clone(), false),
                        EditorTab::Ascii(p, _, dirty) => (p.file_name().unwrap_or_default().to_string_lossy().into_owned(), *dirty),
                        EditorTab::ObsCurve(n, _, _) => (n.clone(), false),
                        EditorTab::PsoResult(n, _) => (n.clone(), false),
                    };

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
                            ui.horizontal(|ui| {
                                let response = ui.selectable_label(is_active, title_text);
                                if response.clicked() {
                                    state.active_tab_index = i;
                                }
                                if response.middle_clicked() {
                                    close_tab = Some(i);
                                }
                                response.context_menu(|ui| {
                                    if ui.button("Close Tab").clicked() {
                                        close_tab = Some(i);
                                        ui.close_menu();
                                    }
                                });
                                
                                ui.add_space(4.0);
                                if ui.button(egui::RichText::new("x").size(12.0)).clicked() {
                                    close_tab = Some(i);
                                }
                            });
                        });
                }

                if let Some(idx) = close_tab {
                    state.open_tabs.remove(idx);
                    if state.active_tab_index >= state.open_tabs.len() && !state.open_tabs.is_empty() {
                        state.active_tab_index = state.open_tabs.len() - 1;
                    }
                }
            });
        });

        ui.separator();

        if state.open_tabs.is_empty() {
            return;
        }

        if state.active_tab_index >= state.open_tabs.len() {
            state.active_tab_index = state.open_tabs.len() - 1;
        }

        // Toolbar
        ui.horizontal(|ui| {
            if let EditorTab::Visualization(_, viz) = &state.open_tabs[state.active_tab_index] {
                if ui.button("💾 Save Plot as Image").clicked() {
                    if let Some(path) = FileDialog::new().add_filter("PNG Image", &["png"]).save_file() {
                        match generate_rjmcmc_viz(viz, &path) {
                            Ok(_) => state.log_output = "Image saved successfully.".to_string(),
                            Err(e) => state.log_output = format!("Failed to save image: {}", e),
                        }
                    }
                }
            }
        });
        ui.separator();

        let tab = &mut state.open_tabs[state.active_tab_index];
        match tab {
            EditorTab::Ascii(_, content, _) => {
                egui::ScrollArea::both().id_salt("ascii_scroll").show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(content)
                            .font(egui::TextStyle::Monospace)
                            .desired_width(f32::INFINITY)
                            .desired_rows(40)
                    );
                });
            }
            EditorTab::Visualization(_, viz) => {
                show_visualization_tab(ui, viz);
            }
            EditorTab::DeterministicCurve(_, hvsr_out) => {
                show_deterministic_tab(ui, hvsr_out);
            }
            EditorTab::ObsCurve(_, freqs, hvs) => {
                show_obs_curve_tab(ui, freqs, hvs);
            }
            EditorTab::PsoResult(_, viz) => {
                show_pso_result_tab(ui, viz);
            }
        }
    });
}

fn show_obs_curve_tab(ui: &mut egui::Ui, freqs: &[f64], hvs: &[f64]) {
    ui.label(egui::RichText::new("Observation HVSR Curve").strong().size(18.0));
    ui.add_space(10.0);
    
    let height = (ui.available_height() - 50.0).max(300.0);
    
    Plot::new("obs_curve_plot")
        .height(height)
        .allow_drag(true)
        .allow_zoom(true)
        .allow_scroll(true)
        .x_grid_spacer(egui_plot::log_grid_spacer(10))
        .x_axis_formatter(|mark, _| format!("{:.2} Hz", 10_f64.powf(mark.value)))
        .y_axis_label("H/V Ratio")
        .show(ui, |plot_ui| {
            let pts: PlotPoints = freqs.iter().zip(hvs).map(|(&x, &y)| [x.max(1e-10).log10(), y]).collect();
            plot_ui.line(Line::new(pts).color(egui::Color32::BLUE).width(2.0_f32).name("Observed HVSR"));
        });
}

fn show_pso_result_tab(ui: &mut egui::Ui, viz: &PsoVizData) {
    egui::ScrollArea::both().id_salt("pso_viz_scroll").show(ui, |ui| {
        let w2 = (ui.available_width() / 2.0) - 20.0;
        let h2 = (ui.available_height() / 2.0).clamp(300.0, 500.0);
        
        let result = &viz.result;
        
        // Color mapping based on cost
        let c_min = result.best_cost;
        let c_max = result.history.iter().map(|h| h.cost).fold(f64::NEG_INFINITY, |a, b| a.max(b));
        
        let get_alpha_val = |cost: f64| -> f32 {
            if c_max <= c_min { return 0.5; }
            let normalized = (cost - c_min) / (c_max - c_min);
            let alpha = 0.8 - normalized as f32 * (0.8 - 0.2);
            alpha.max(0.2).min(1.0)
        };
        
        egui::Grid::new("pso_viz_plots").num_columns(2).spacing([10.0, 10.0]).show(ui, |ui| {
            // --- Plot 1: HVSR Fit ---
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("HVSR Fit").strong());
                Plot::new("pso_hvsr_fit")
                    .width(w2.max(300.0)).height(h2)
                    .allow_drag(true).allow_zoom(true).allow_scroll(true)
                    .x_grid_spacer(egui_plot::log_grid_spacer(10))
                    .x_axis_formatter(|mark, _| format!("{:.2} Hz", 10_f64.powf(mark.value)))
                    .y_axis_label("H/V Ratio")
                    .show(ui, |plot_ui| {
                        let synth_freqs = if !result.freqs.is_empty() { &result.freqs } else { &viz.obs_freqs };

                        // Plot history synthetic curves (gray)
                        for (i, entry) in result.history.iter().enumerate() {
                            let alpha = (get_alpha_val(entry.cost) * 255.0) as u8;
                            let pts: PlotPoints = synth_freqs.iter().zip(&entry.hvsr)
                                .map(|(&x, &y)| [x.max(1e-10).log10(), y]).collect();
                            plot_ui.line(Line::new(pts)
                                .color(egui::Color32::from_rgba_unmultiplied(100, 100, 100, alpha))
                                .width(1.5_f32)
                                .name(format!("Iter {}", entry.iter)));
                        }
                        
                        // Plot best model (red dashed)
                        let best_pts: PlotPoints = synth_freqs.iter().zip(&result.estimated_hvsr)
                            .map(|(&x, &y)| [x.max(1e-10).log10(), y]).collect();
                        plot_ui.line(Line::new(best_pts)
                            .color(egui::Color32::RED).width(2.5_f32)
                            .name(format!("Best (Cost: {:.4e})", result.best_cost)));
                        
                        // Plot observed data (blue thick)
                        let obs_pts: PlotPoints = viz.obs_freqs.iter().zip(&viz.obs_hvs)
                            .map(|(&x, &y)| [x.max(1e-10).log10(), y]).collect();
                        plot_ui.line(Line::new(obs_pts)
                            .color(egui::Color32::BLUE).width(3.0_f32)
                            .name("Observed"));
                    });
            });
            
            // --- Plot 2: Vs(z) Profile ---
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("Vs Profile (1D Model)").strong());
                Plot::new("pso_vs_profile")
                    .width(w2.max(300.0)).height(h2)
                    .allow_drag(true).allow_zoom(true).allow_scroll(true)
                    .x_axis_label("Vs (m/s)")
                    .y_axis_label("Depth (m)")
                    .show(ui, |plot_ui| {
                        // History models (gray)
                        for entry in &result.history {
                            let alpha = (get_alpha_val(entry.cost) * 255.0) as u8;
                            let (vs_vals, depths) = get_1d_points(&entry.model.layers);
                            let pts: PlotPoints = vs_vals.iter().zip(&depths)
                                .map(|(&x, &y)| [x, y]).collect();
                            plot_ui.line(Line::new(pts)
                                .color(egui::Color32::from_rgba_unmultiplied(100, 100, 100, alpha))
                                .width(1.5_f32));
                        }
                        
                        // Best model (red)
                        let (vs_vals, depths) = get_1d_points(&result.best_model.layers);
                        let pts: PlotPoints = vs_vals.iter().zip(&depths)
                            .map(|(&x, &y)| [x, y]).collect();
                        plot_ui.line(Line::new(pts)
                            .color(egui::Color32::RED).width(2.5_f32)
                            .name("Best Model"));
                    });
            });
            
            ui.end_row();
            
            // --- Plot 3: Cost Convergence ---
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("Cost Convergence").strong());
                Plot::new("pso_convergence")
                    .width(w2.max(300.0)).height(h2)
                    .allow_drag(true).allow_zoom(true).allow_scroll(true)
                    .x_axis_label("Iteration")
                    .y_axis_label("Cost (L2 Norm)")
                    .show(ui, |plot_ui| {
                        let pts: PlotPoints = result.cost_history.iter().enumerate()
                            .map(|(i, &c)| [(i + 1) as f64, c]).collect();
                        plot_ui.line(Line::new(pts)
                            .color(egui::Color32::RED).width(2.0_f32)
                            .name("Cost History"));
                    });
            });
            
            // --- Plot 4: Layer Table Info ---
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("Best Model Details").strong());
                ui.add_space(5.0);
                ui.label(format!("Best Cost: {:.6e}", result.best_cost));
                ui.label(format!("Number of Layers: {}", result.best_model.layers.len()));
                ui.add_space(5.0);
                
                egui::Grid::new("best_model_table").striped(true).num_columns(5).show(ui, |ui| {
                    ui.label(egui::RichText::new("Layer").strong());
                    ui.label(egui::RichText::new("H (m)").strong());
                    ui.label(egui::RichText::new("Vs (m/s)").strong());
                    ui.label(egui::RichText::new("Vp (m/s)").strong());
                    ui.label(egui::RichText::new("ρ (kg/m³)").strong());
                    ui.end_row();
                    
                    for (i, layer) in result.best_model.layers.iter().enumerate() {
                        ui.label(format!("{}", i + 1));
                        if layer.thickness == 0.0 {
                            ui.label("∞ (halfspace)");
                        } else {
                            ui.label(format!("{:.1}", layer.thickness));
                        }
                        ui.label(format!("{:.1}", layer.vs));
                        ui.label(format!("{:.1}", layer.vp));
                        ui.label(format!("{:.1}", layer.density));
                        ui.end_row();
                    }
                });
                
                ui.add_space(10.0);
                ui.label(egui::RichText::new("Geotechnical Parameters").strong());
                ui.add_space(5.0);
                
                let mut best_vs30: Option<f64> = None;
                let mut best_h800: Option<f64> = None;
                let mut best_z1: Option<f64> = None;
                let mut best_z2_5: Option<f64> = None;
                
                if best_vs30.is_none() || best_h800.is_none() || best_z1.is_none() || best_z2_5.is_none() {
                    let mut z = vec![0.0];
                    let mut vs = Vec::new();
                    let mut sum_h = 0.0;
                    for layer in &result.best_model.layers {
                        if layer.thickness > 0.0 {
                            sum_h += layer.thickness;
                            z.push(sum_h);
                        } else {
                            z.push(sum_h + 10.0);
                        }
                        vs.push(layer.vs);
                    }
                    if let Some(&last) = vs.last() {
                        vs.push(last);
                    }
                    if best_vs30.is_none() { best_vs30 = Some(crate::core::rjmcmc_stats::calc_vs30(&z, &vs)); }
                    if best_h800.is_none() { best_h800 = Some(crate::core::rjmcmc_stats::calc_depth_for_vs(&z, &vs, 800.0)); }
                    if best_z1.is_none() { best_z1 = Some(crate::core::rjmcmc_stats::calc_depth_for_vs(&z, &vs, 1000.0)); }
                    if best_z2_5.is_none() { best_z2_5 = Some(crate::core::rjmcmc_stats::calc_depth_for_vs(&z, &vs, 2500.0)); }
                }
                
                ui.label(format!("Vs30: {:.2} m/s", best_vs30.unwrap_or(0.0)));
                ui.label(format!("H800: {:.2} m", best_h800.unwrap_or(0.0)));
                ui.label(format!("Z1.0: {:.2} m", best_z1.unwrap_or(0.0)));
                ui.label(format!("Z2.5: {:.2} m", best_z2_5.unwrap_or(0.0)));
            });
            
            ui.end_row();
        });
    });
}

fn get_1d_points(layers: &[crate::hvf::model::Layer]) -> (Vec<f64>, Vec<f64>) {
    let mut vs_vals = Vec::new();
    let mut depths = Vec::new();
    let mut cum_depth = 0.0;
    
    for (i, layer) in layers.iter().enumerate() {
        vs_vals.push(layer.vs);
        depths.push(-cum_depth);
        
        if layer.thickness > 0.0 {
            cum_depth += layer.thickness;
            vs_vals.push(layer.vs);
            depths.push(-cum_depth);
            
            if i + 1 < layers.len() {
                vs_vals.push(layers[i + 1].vs);
                depths.push(-cum_depth);
            }
        } else {
            // Halfspace - extend by 20m
            let last_z = cum_depth + 20.0;
            vs_vals.push(layer.vs);
            depths.push(-last_z);
        }
    }
    
    (vs_vals, depths)
}

fn show_visualization_tab(ui: &mut egui::Ui, viz: &VisualizerData) {
    egui::ScrollArea::both().id_salt("viz_scroll").show(ui, |ui| {
        let cmap = |rmse: f64, min_rmse: f64, max_rmse: f64| -> egui::Color32 {
            let norm = if max_rmse > min_rmse { (rmse - min_rmse) / (max_rmse - min_rmse) } else { 0.0 };
            let r = 255;
            let g = (165.0 + norm * (255.0 - 165.0)) as u8;
            let b = (0.0 + norm * 100.0) as u8;
            egui::Color32::from_rgb(r, g, b)
        };
        
        let min_rmse = viz.sorted_rmse.iter().copied().fold(f64::INFINITY, |a, b| a.min(b));
        let max_rmse = viz.sorted_rmse.iter().copied().fold(f64::NEG_INFINITY, |a, b| a.max(b));
        
        let w2 = (ui.available_width() / 2.0) - 20.0;
        let h2 = (ui.available_height() / 2.0).clamp(300.0, 500.0);
        
        egui::Grid::new("viz_plots").num_columns(2).spacing([10.0, 10.0]).show(ui, |ui| {
            
            // --- Plot 1: HVSR Curves ---
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("Posterior HVSR Fits").strong());
                Plot::new("hvsr_plot")
                    .width(w2.max(300.0)).height(h2)
                    .allow_drag(true)
                    .allow_zoom(true)
                    .allow_scroll(true)
                    .x_axis_formatter(|mark, _| format!("{:.2} Hz", 10_f64.powf(mark.value)))
                    .show(ui, |plot_ui| {
                        for m in &viz.samples {
                            let pts: PlotPoints = viz.freq.iter().zip(&m.h_syn).map(|(&x, &y)| [x.max(1e-10).log10(), y]).collect();
                            plot_ui.line(Line::new(pts).color(cmap(m.rmse, min_rmse, max_rmse)).name("Syn"));
                        }
                        
                        let obs_pts: PlotPoints = viz.freq.iter().zip(&viz.h_obs).map(|(&x, &y)| [x.max(1e-10).log10(), y]).collect();
                        plot_ui.points(Points::new(obs_pts).color(egui::Color32::BLACK).radius(2.0_f32).name("Observed"));
                    });
            });
            
            // --- Plot 2: Vs(z) Profile ---
            ui.vertical(|ui| {
                ui.label(egui::RichText::new(format!("Posterior Vs Profiles (Vs30 = {:.0} m/s)", viz.vs30_mean)).strong());
                Plot::new("vsz_plot")
                    .width(w2.max(300.0)).height(h2)
                    .allow_drag(true)
                    .allow_zoom(true)
                    .allow_scroll(true)
                    .show(ui, |plot_ui| {
                        for m in &viz.samples {
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
                            
                            plot_ui.line(Line::new(PlotPoints::new(pts)).color(cmap(m.rmse, min_rmse, max_rmse)));
                        }
                        
                        let mut p05_pts = Vec::new();
                        let mut p95_pts = Vec::new();
                        for (j, &zz) in viz.z_nodes.iter().enumerate() {
                            p05_pts.push([viz.vs_p05[j], -zz]);
                            p95_pts.push([viz.vs_p95[j], -zz]);
                        }
                        plot_ui.line(Line::new(PlotPoints::new(p05_pts)).color(egui::Color32::from_rgb(100, 149, 237)).width(1.0_f32).name("P05"));
                        plot_ui.line(Line::new(PlotPoints::new(p95_pts)).color(egui::Color32::from_rgb(100, 149, 237)).width(1.0_f32).name("P95"));
                        
                        let mut med_pts = Vec::new();
                        for (j, &zz) in viz.z_nodes.iter().enumerate() {
                            med_pts.push([viz.vs_p50[j], -zz]);
                        }
                        plot_ui.line(Line::new(PlotPoints::new(med_pts)).color(egui::Color32::BLUE).width(2.0_f32).name("Median"));
                        
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
                            plot_ui.line(Line::new(PlotPoints::new(pts)).color(egui::Color32::BLACK).width(2.0_f32).name("Best"));
                        }
                        
                        if !viz.vs30_mean.is_nan() {
                            plot_ui.points(Points::new(PlotPoints::new(vec![[viz.vs30_mean, -30.0]])).color(egui::Color32::RED).radius(4.0_f32).name("Vs30"));
                        }
                        if !viz.h800_mean.is_nan() {
                            plot_ui.points(Points::new(PlotPoints::new(vec![[800.0, -viz.h800_mean]])).color(egui::Color32::DARK_GREEN).radius(4.0_f32).name("H800"));
                        }
                        if !viz.z1_mean.is_nan() {
                            plot_ui.points(Points::new(PlotPoints::new(vec![[1000.0, -viz.z1_mean]])).color(egui::Color32::from_rgb(0, 139, 139)).radius(4.0_f32).name("Z1.0"));
                        }
                    });
            });
            
            ui.end_row();
            
            // --- Plot 3: Histogram RMSE ---
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("RMSE Histogram").strong());
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
                    .width(w2.max(300.0)).height(h2)
                    .allow_drag(true)
                    .allow_zoom(true)
                    .allow_scroll(true)
                    .show(ui, |plot_ui| {
                        plot_ui.bar_chart(BarChart::new(bars).color(egui::Color32::ORANGE));
                    });
            });
            
            // --- Plot 4: RMSE vs Index & Layers vs Index ---
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("RMSE vs Index").strong());
                Plot::new("rmse_index")
                    .width(w2.max(300.0)).height(h2 / 2.0 - 15.0)
                    .allow_drag(true)
                    .allow_zoom(true)
                    .allow_scroll(true)
                    .show(ui, |plot_ui| {
                        let mut pts = Vec::new();
                        for (i, &r) in viz.sorted_rmse.iter().enumerate() {
                            pts.push([i as f64, r]);
                        }
                        plot_ui.line(Line::new(PlotPoints::new(pts)).color(egui::Color32::from_rgb(128, 0, 128)).name("RMSE"));
                    });
                    
                ui.add_space(5.0);
                ui.label(egui::RichText::new("Number of Layers vs Index").strong());
                Plot::new("layers_index")
                    .width(w2.max(300.0)).height(h2 / 2.0 - 15.0)
                    .allow_drag(true)
                    .allow_zoom(true)
                    .allow_scroll(true)
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
        
        ui.add_space(10.0);
        ui.group(|ui| {
            ui.heading("Geotechnical Parameters Summary (Posterior)");
            ui.label(format!("Vs30: {:.2} m/s (Range: {:.2} - {:.2})", viz.vs30_mean, viz.vs30_min, viz.vs30_max));
            ui.label(format!("H800: {:.2} m (Range: {:.2} - {:.2})", viz.h800_mean, viz.h800_min, viz.h800_max));
            ui.label(format!("Z1.0: {:.2} m (Range: {:.2} - {:.2})", viz.z1_mean, viz.z1_min, viz.z1_max));
            ui.label(format!("Z2.5: {:.2} m (Range: {:.2} - {:.2})", viz.z2_5_mean, viz.z2_5_min, viz.z2_5_max));
            
            if let Some(best) = &viz.best_sample {
                ui.add_space(8.0);
                ui.heading("Best Model Geotechnical Parameters");
                let mut best_vs30 = best.vs30;
                let mut best_h800 = best.h800;
                let mut best_z1 = best.z1_0;
                let mut best_z2_5 = best.z2_5;
                
                // Fallback: Compute if not present in JSON
                if best_vs30.is_none() || best_h800.is_none() || best_z1.is_none() || best_z2_5.is_none() {
                    let mut z = vec![0.0];
                    let mut vs = Vec::new();
                    let mut sum_h = 0.0;
                    for i in 0..best.h.len() {
                        sum_h += best.h[i];
                        z.push(sum_h);
                        vs.push(best.vs[i]);
                    }
                    if let Some(&last) = best.vs.last() {
                        vs.push(last);
                    }
                    if best_vs30.is_none() { best_vs30 = Some(crate::core::rjmcmc_stats::calc_vs30(&z, &vs)); }
                    if best_h800.is_none() { best_h800 = Some(crate::core::rjmcmc_stats::calc_depth_for_vs(&z, &vs, 800.0)); }
                    if best_z1.is_none() { best_z1 = Some(crate::core::rjmcmc_stats::calc_depth_for_vs(&z, &vs, 1000.0)); }
                    if best_z2_5.is_none() { best_z2_5 = Some(crate::core::rjmcmc_stats::calc_depth_for_vs(&z, &vs, 2500.0)); }
                }
                
                ui.label(format!("Vs30: {:.2} m/s", best_vs30.unwrap_or(0.0)));
                ui.label(format!("H800: {:.2} m", best_h800.unwrap_or(0.0)));
                ui.label(format!("Z1.0: {:.2} m", best_z1.unwrap_or(0.0)));
                ui.label(format!("Z2.5: {:.2} m", best_z2_5.unwrap_or(0.0)));
            }
        });
    });
}

fn show_deterministic_tab(ui: &mut egui::Ui, hvsr_out: &crate::hvf::output::HvsrOutput) {
    egui::ScrollArea::both().id_salt("det_scroll").show(ui, |ui| {
        ui.label(egui::RichText::new("HVSR Theoretical Curve (Deterministic / Forward Modeling)").strong().size(18.0));
        ui.add_space(10.0);
        
        let height = (ui.available_height() - 50.0).max(300.0);
        
        Plot::new("det_plot")
            .height(height)
            .allow_drag(true)
            .allow_zoom(true)
            .allow_scroll(true)
            .x_axis_formatter(|mark, _| format!("{:.2} Hz", 10_f64.powf(mark.value)))
            .show(ui, |plot_ui| {
                let pts: PlotPoints = hvsr_out.frequencies.iter().zip(&hvsr_out.hv_ratio).map(|(&x, &y)| [x.max(1e-10).log10(), y]).collect();
                plot_ui.line(Line::new(pts).color(egui::Color32::BLUE).width(2.0_f32).name("Theoretical HVSR"));
            });
    });
}

fn run_extraction(state: &InversionState, save_path: &Path) -> Result<(), String> {
    use std::fs::File;
    use std::io::Write;
    use crate::hvf::inversion::InversionResult;
    use crate::core::rjmcmc_stats::{calc_vs30, calc_depth_for_vs, load_and_process_data};
    
    let is_csv = state.extractor_format.to_lowercase() == "csv";
    
    let mut file = File::create(save_path).map_err(|e| e.to_string())?;
    
    // Header for CSV
    if is_csv {
        let mut header = vec!["Filename".to_string()];
        if state.extractor_rmse { header.push("RMSE".to_string()); }
        if state.extractor_geotech { 
            header.push("Vs30".to_string());
            header.push("Z800".to_string());
            header.push("Z1.0".to_string());
        }
        if state.extractor_best_model {
            header.push("Num_Layers".to_string());
            header.push("Layer_Data(H;Vs;Vp;Rho|...)".to_string());
        }
        writeln!(file, "{}", header.join(",")).map_err(|e| e.to_string())?;
    }
    
    for input_file in &state.extractor_input_files {
        let name = PathBuf::from(input_file).file_name().unwrap_or_default().to_string_lossy().into_owned();
        let content = std::fs::read_to_string(input_file).unwrap_or_default();
        
        let mut rmse_val = f64::NAN;
        let mut vs30_val = f64::NAN;
        let mut z800_val = f64::NAN;
        let mut z1000_val = f64::NAN;
        let mut num_layers = 0;
        let mut layers_data = String::new();
        
        let is_json = name.ends_with(".json");
        let is_jsonl = name.ends_with(".jsonl");
        
        if is_json {
            if let Ok(result) = serde_json::from_str::<InversionResult>(&content) {
                rmse_val = result.best_cost;
                num_layers = result.best_model.layers.len();
                
                let mut z = vec![0.0];
                let mut vs = Vec::new();
                let mut sum_h = 0.0;
                
                for layer in &result.best_model.layers {
                    vs.push(layer.vs);
                    if layer.thickness > 0.0 {
                        sum_h += layer.thickness;
                        z.push(sum_h);
                    } else {
                        z.push(sum_h + 10.0);
                    }
                }
                if let Some(last) = vs.last().copied() {
                    vs.push(last);
                }
                
                vs30_val = calc_vs30(&z, &vs);
                z800_val = calc_depth_for_vs(&z, &vs, 800.0);
                z1000_val = calc_depth_for_vs(&z, &vs, 1000.0);
                
                if state.extractor_best_model {
                    let parts: Vec<String> = result.best_model.layers.iter().map(|l| {
                        format!("{:.2};{:.2};{:.2};{:.2}", l.thickness, l.vs, l.vp, l.density)
                    }).collect();
                    layers_data = parts.join("|");
                }
            }
        } else if is_jsonl {
            if let Ok(viz) = load_and_process_data(input_file, "") {
                if let Some(best) = viz.best_sample {
                    rmse_val = best.rmse;
                    num_layers = best.n_layers;
                    
                    if state.extractor_best_model {
                        let mut parts = Vec::new();
                        for i in 0..best.h.len() {
                            let vp = best.vs[i] * 1.732; // Assuming Poisson ratio 0.25
                            let rho = if vp > 1000.0 { 310.0 * vp.powf(0.25) } else { 1800.0 }; // Brocher approximation
                            parts.push(format!("{:.2};{:.2};{:.2};{:.2}", best.h[i], best.vs[i], vp, rho));
                        }
                        // halfspace
                        if let Some(&vs_h) = best.vs.last() {
                            let vp_h = vs_h * 1.732;
                            let rho_h = if vp_h > 1000.0 { 310.0 * vp_h.powf(0.25) } else { 1800.0 };
                            parts.push(format!("0.00;{:.2};{:.2};{:.2}", vs_h, vp_h, rho_h));
                        }
                        layers_data = parts.join("|");
                    }
                }
                vs30_val = viz.vs30_mean;
                z800_val = viz.h800_mean;
                z1000_val = viz.z1_mean;
            }
        }
        
        if is_csv {
            let mut row = vec![name];
            if state.extractor_rmse { row.push(format!("{:.5}", rmse_val)); }
            if state.extractor_geotech {
                row.push(format!("{:.2}", vs30_val));
                row.push(format!("{:.2}", z800_val));
                row.push(format!("{:.2}", z1000_val));
            }
            if state.extractor_best_model {
                row.push(format!("{}", num_layers));
                row.push(layers_data);
            }
            writeln!(file, "{}", row.join(",")).map_err(|e| e.to_string())?;
        } else {
            // TXT format
            writeln!(file, "File: {}", name).map_err(|e| e.to_string())?;
            if state.extractor_rmse { writeln!(file, "RMSE: {:.5}", rmse_val).map_err(|e| e.to_string())?; }
            if state.extractor_geotech {
                writeln!(file, "Vs30: {:.2} m/s", vs30_val).map_err(|e| e.to_string())?;
                writeln!(file, "Z800: {:.2} m", z800_val).map_err(|e| e.to_string())?;
                writeln!(file, "Z1.0: {:.2} m", z1000_val).map_err(|e| e.to_string())?;
            }
            if state.extractor_best_model {
                writeln!(file, "Number of Layers: {}", num_layers).map_err(|e| e.to_string())?;
                writeln!(file, "Layers (Thickness, Vs, Vp, Rho):").map_err(|e| e.to_string())?;
                for l in layers_data.split('|') {
                    if !l.is_empty() {
                        let parts: Vec<&str> = l.split(';').collect();
                        if parts.len() == 4 {
                            writeln!(file, "  H: {} m, Vs: {} m/s, Vp: {} m/s, Rho: {} kg/m³", parts[0], parts[1], parts[2], parts[3]).map_err(|e| e.to_string())?;
                        }
                    }
                }
            }
            writeln!(file, "--------------------------------------------------").map_err(|e| e.to_string())?;
        }
    }
    
    Ok(())
}
