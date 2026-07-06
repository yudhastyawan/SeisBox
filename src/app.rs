use eframe::egui;
use std::sync::mpsc::Receiver;
use std::collections::HashSet;
use std::path::PathBuf;

use crate::core::picking::PickSet;
use crate::core::seismogram::Seismogram;
use crate::io::file_sync::{self, FileNode};
use crate::ui::{dialogs, plot::{self, TraceState}, spatial_dialog, fdsn_dialog, hvsr_dialog, bvor_dialog};
use crate::ui::spatial_map::{MapData, MapState};
use crate::core::isc_client::{ConversionRule, EarthquakeEvent, IscResult};
use crate::ui::shortcuts::{self, CutState, ZoomState};
use crate::ui::sidebar::{self, SidebarAction};
use crate::ui::bvor_dialog::BVorDialog;
use crate::ui::bvor_vis_dialog::BVorVisState;
use crate::ui::cfs_dialog::CfsDialogState;
/// Main application state.
#[allow(dead_code)]
pub struct QuakePickApp {
    // -- File management --
    pub root_dir: Option<PathBuf>,
    pub file_tree: Option<file_sync::FileNode>,
    /// Set of selected files in the sidebar
    pub selected_files: HashSet<PathBuf>,
    /// Last clicked file for Shift-click ranges
    pub last_clicked: Option<PathBuf>,

    // -- Traces --
    /// Currently loaded traces.
    pub traces: Vec<TraceState>,
    /// Index of the trace that receives keyboard picks (active trace).
    pub active_trace_idx: Option<usize>,
    /// Mouse X coordinate in plot space (for crosshairs).
    pub hover_x: Option<f64>,
    /// The current visible X bounds of the plots.
    pub current_x_bounds: Option<(f64, f64)>,
    /// Index of the trace the cursor is currently hovering over.
    pub hover_trace_idx: Option<usize>,
    
    // -- View state --
    pub zoom_action: Option<ZoomAction>,
    pub cut_state: CutState,
    pub zoom_state: ZoomState,

    // -- Filters --
    pub filter_active: bool,
    pub predictive_filter_on: bool,

    // -- Dialogs --
    pub show_bandpass: bool,
    pub show_hodogram: bool,
    pub show_spectral: bool,
    pub spectrogram_target: Option<usize>,
    pub spectrogram_texture: Option<egui::TextureHandle>,
    pub spectrogram_bounds: Option<(f64, f64)>, // (t_min, t_max) of generated image
    pub spectrogram_raw_data: Option<crate::core::spectrogram::SpectrogramData>,
    pub spectrogram_receiver: Option<Receiver<(usize, (f64, f64), Option<crate::core::spectrogram::SpectrogramData>)>>,
    pub show_spectrogram_confirm: bool,
    pub spectrogram_pending_target: Option<usize>,
    pub spectrogram_pending_samples: usize,
    pub spectrogram_pending_bounds: Option<(f64, f64)>,
    
    pub bandpass_low: f64,
    pub bandpass_high: f64,
    pub remove_mean: bool,

    // UI state
    pub sidebar_search: String,
    pub show_shortcuts_help: bool,

    // -- Status --
    pub status_msg: String,
    
    // -- Export --
    pub pending_screenshot_path: Option<PathBuf>,
    pub last_plot_rect: Option<egui::Rect>,
    pub is_screenshot_mode: bool,
    pub header_popup_idx: Option<usize>,
    pub nav_mode: NavigationMode,

    // -- Spatial Analysis --
    pub show_spatial_window: bool,
    pub map_data: Option<MapData>,
    pub map_state: MapState,
    
    pub show_spatial_viz: bool,
    pub spatial_viz_texture: Option<egui::TextureHandle>,
    pub spatial_viz_texture2: Option<egui::TextureHandle>,
    
    // FDSN
    pub fdsn_state: fdsn_dialog::FdsnState,
    
    // HVSR
    pub hvsr_state: crate::ui::hvsr_dialog::HvsrState,
    
    pub inversion_state: crate::ui::inversion_dialog::InversionState,
    
    pub bvor_dialog_open: bool,
    pub bvor_dialog: BVorDialog,
    pub bvor_vis_dialog: BVorVisState,
    pub cfs_dialog: CfsDialogState,
    pub inp_generator_dialog: crate::ui::inp_generator_dialog::InpGeneratorState,
    
    // Magnitude conversion settings
    pub conversion_rules: Vec<ConversionRule>,
    pub mag_priority: Vec<String>,
    pub show_conversion_settings: bool,
    
    // ISC Search Forms
    pub isc_start_date_str: String,
    pub isc_start_time_str: String,
    pub isc_end_date_str: String,
    pub isc_end_time_str: String,
    pub isc_min_depth: f64,
    pub isc_max_depth: f64,
    pub isc_min_mag: f64,
    pub isc_max_mag: f64,
    
    pub isc_events: Vec<EarthquakeEvent>,
    pub isc_raw_data: String,
    pub isc_receiver: Option<Receiver<IscResult>>,
    pub isc_loading: bool,
}

impl Default for QuakePickApp {
    fn default() -> Self {
        Self {
            root_dir: None,
            file_tree: None,
            selected_files: HashSet::new(),
            last_clicked: None,
            traces: Vec::new(),
            active_trace_idx: None,
            hover_x: None,
            current_x_bounds: None,
            hover_trace_idx: None,
            zoom_action: None,
            cut_state: CutState::default(),
            zoom_state: ZoomState::default(),
            filter_active: false,
            predictive_filter_on: false,
            show_bandpass: false,
            show_hodogram: false,
            show_spectral: false,
            spectrogram_target: None,
            spectrogram_texture: None,
            spectrogram_bounds: None,
            spectrogram_raw_data: None,
            spectrogram_receiver: None,
            show_spectrogram_confirm: false,
            spectrogram_pending_target: None,
            spectrogram_pending_samples: 0,
            spectrogram_pending_bounds: None,
            bandpass_low: 1.0,
            bandpass_high: 10.0,
            remove_mean: false,
            sidebar_search: String::new(),
            show_shortcuts_help: false,
            status_msg: "Welcome to QuakePick — open a folder to begin".to_string(),
            pending_screenshot_path: None,
            last_plot_rect: None,
            is_screenshot_mode: false,
            header_popup_idx: None,
            nav_mode: NavigationMode::Single,
            
            show_spatial_window: false,
            map_data: None, // Will be initialized when first opening the window
            map_state: MapState::default(),
            show_spatial_viz: false,
            spatial_viz_texture: None,
            spatial_viz_texture2: None,
            
            fdsn_state: crate::ui::fdsn_dialog::FdsnState::default(),
            
            hvsr_state: crate::ui::hvsr_dialog::HvsrState::default(),
            
            inversion_state: crate::ui::inversion_dialog::InversionState::default(),
            
            bvor_dialog_open: false,
            bvor_dialog: BVorDialog::default(),
            bvor_vis_dialog: BVorVisState::default(),
            cfs_dialog: CfsDialogState::default(),
            inp_generator_dialog: crate::ui::inp_generator_dialog::InpGeneratorState::default(),
            
            conversion_rules: vec![
                ConversionRule { id: 0, source_type: "MB".to_string(), min_mag: -99.0, max_mag: 8.2, multiplier: 1.0107, offset: 0.0801 },
                ConversionRule { id: 1, source_type: "MS".to_string(), min_mag: -99.0, max_mag: 6.1, multiplier: 0.6016, offset: 2.476 },
                ConversionRule { id: 2, source_type: "MS".to_string(), min_mag: 6.2, max_mag: 99.0, multiplier: 0.9239, offset: 0.5671 },
                ConversionRule { id: 3, source_type: "MLV".to_string(), min_mag: -99.0, max_mag: 99.0, multiplier: 1.0, offset: 0.0 },
                ConversionRule { id: 4, source_type: "ML".to_string(), min_mag: -99.0, max_mag: 99.0, multiplier: 1.0, offset: 0.0 },
                ConversionRule { id: 5, source_type: "MW".to_string(), min_mag: -99.0, max_mag: 99.0, multiplier: 1.0, offset: 0.0 },
            ],
            mag_priority: vec!["MW".to_string(), "MS".to_string(), "ML".to_string(), "MB".to_string()],
            show_conversion_settings: false,
            isc_start_date_str: "2010-01-01".to_string(),
            isc_start_time_str: "00:00:00".to_string(),
            isc_end_date_str: "2024-01-01".to_string(),
            isc_end_time_str: "23:59:59".to_string(),
            isc_min_depth: 0.0,
            isc_max_depth: 100.0,
            isc_min_mag: 3.0,
            isc_max_mag: 9.0,
            isc_events: Vec::new(),
            isc_raw_data: String::new(),
            isc_receiver: None,
            isc_loading: false,
        }
    }
}

impl QuakePickApp {
    /// Load one or more seismic files (mock data) and auto-load any associated picks.
    fn load_seismic_files(&mut self, paths: Vec<PathBuf>) {
        // Sort paths for deterministic ordering
        let mut paths = paths;
        paths.sort();

        self.traces.clear();
        self.filter_active = false;
        self.predictive_filter_on = false;
        
        self.hvsr_state.z_comp.clear();
        self.hvsr_state.n_comp.clear();
        self.hvsr_state.e_comp.clear();
        self.hvsr_state.result = None;
        self.hvsr_state.progress_pct = 0.0;
        self.hvsr_state.progress_msg.clear();
        
        let mut all_traces = Vec::new();

        for path in &paths {
            match crate::core::parser::parse_seismic_file(path) {
                Ok(seismograms) => {
                    for seismogram in seismograms {
                        // Create a distinct virtual path for each component so they can have distinct .picks files
                        let trace_path = path.with_file_name(&seismogram.filename);
                        let (pick_set, pick_path) = file_sync::auto_load_picks(&trace_path);

                        if let Some(ref pp) = pick_path {
                            println!(
                                "[QuakePick] Loaded {} with {} picks from {:?}",
                                seismogram.filename,
                                pick_set.len(),
                                pp.file_name().unwrap_or_default()
                            );
                        } else {
                            println!("[QuakePick] Loaded {} (no existing picks)", seismogram.filename);
                        }

                        let is_visible = all_traces.len() < 3; // Default show only first 3 traces

                        let decimated = plot::decimate_for_plot(&seismogram.time, &seismogram.amplitude);

                        all_traces.push(TraceState {
                            path: trace_path,
                            seismogram,
                            original_amplitude: None,
                            pick_set,
                            is_visible,
                            decimated_points: decimated,
                        });
                    }
                }
                Err(e) => {
                    let err_msg = format!("Failed to load {:?}: {}", path, e);
                    eprintln!("[QuakePick] Error: {}", err_msg);
                    self.status_msg = err_msg; // Show in UI status bar
                }
            }
        }

        self.traces = all_traces;

        // Set the first trace as active (if it exists)
        self.active_trace_idx = if self.traces.is_empty() {
            None
        } else {
            Some(0)
        };
        
        // Reset view state
        self.zoom_action = Some(ZoomAction::Reset);
        self.cut_state = CutState::default();
        self.zoom_state = ZoomState::default();
        
        self.status_msg = format!("Loaded {} traces.", self.traces.len());
    }

    /// Open a folder dialog and scan the selected directory.
    fn open_folder(&mut self) {
        if let Some(dir) = rfd::FileDialog::new().pick_folder() {
            self.scan_directory(dir);
        }
    }

    /// Scan/refresh the file tree for the given directory.
    fn scan_directory(&mut self, dir: PathBuf) {
        match FileNode::scan_dir(&dir) {
            Ok(tree) => {
                self.file_tree = Some(tree);
                self.root_dir = Some(dir);
                self.selected_files.clear();
                self.last_clicked = None;
                self.status_msg = "Folder scanned successfully".to_string();
            }
            Err(e) => {
                self.status_msg = format!("Error scanning directory: {}", e);
                eprintln!("[QuakePick] {}", self.status_msg);
            }
        }
    }

    /// Apply Butterworth bandpass filter to the active trace.
    fn apply_bandpass_filter(&mut self) {
        if let Some(idx) = self.active_trace_idx {
            if let Some(ts) = self.traces.get_mut(idx) {
                if ts.original_amplitude.is_none() {
                    ts.original_amplitude = Some(ts.seismogram.amplitude.clone());
                }
                
                let data_to_filter = ts.original_amplitude.as_ref().unwrap();
                let order = 4;
                
                ts.seismogram.amplitude = crate::core::filter::apply_bandpass(
                    data_to_filter,
                    ts.seismogram.sample_rate,
                    self.bandpass_low,
                    self.bandpass_high,
                    order,
                );
                
                ts.decimated_points = plot::decimate_for_plot(&ts.seismogram.time, &ts.seismogram.amplitude);
                
                self.filter_active = true;
                
                // Clear spectrogram so it regenerates on the new data if re-toggled
                self.spectrogram_target = None;
                self.spectrogram_pending_target = None;
                
                // Auto-scale Y-axis to show the filtered data correctly without losing X-zoom
                self.zoom_action = Some(ZoomAction::ResetY);
                
                self.status_msg = format!(
                    "Bandpass filter applied: {:.1}–{:.1} Hz (Butterworth 4th-order) on {}",
                    self.bandpass_low, self.bandpass_high, ts.seismogram.filename
                );
                println!("[QuakePick] {}", self.status_msg);
            }
        }
    }

    /// Prompts the user for a path and saves all current picks to an ASCII (TSV) file.
    fn save_picks_ascii(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Text File (TSV)", &["txt", "tsv"])
            .set_file_name("quakepick_picks.txt")
            .save_file()
        {
            if let Err(e) = crate::io::file_sync::export_picks_to_ascii(&path, &self.traces) {
                self.status_msg = format!("Failed to save picks: {}", e);
                eprintln!("[QuakePick] {}", self.status_msg);
            } else {
                self.status_msg = format!("Picks saved to {:?}", path);
                println!("[QuakePick] {}", self.status_msg);
            }
        }
    }

    /// Prompts the user for a path and saves the figure.
    fn save_figure_to_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("PNG Image", &["png"])
            .set_file_name("quakepick_figure.png")
            .save_file()
        {
            // Calculate image size: base 1920x1080, scale height by visible trace count
            let visible_count = self.traces.iter().filter(|t| t.is_visible).count();
            let has_spec = self.spectrogram_target.is_some() && self.spectrogram_raw_data.is_some();
            let total = visible_count + if has_spec { 1 } else { 0 };
            let img_w = 1920u32;
            let img_h = (200 * total as u32 + 80).max(400).min(4000);

            match crate::io::export_image::save_figure(
                &self.traces,
                self.active_trace_idx,
                self.remove_mean,
                self.spectrogram_target,
                self.spectrogram_raw_data.as_ref(),
                self.spectrogram_bounds,
                self.current_x_bounds,
                &path,
                img_w,
                img_h,
            ) {
                Ok(()) => self.status_msg = format!("Figure saved to {:?}", path),
                Err(e) => self.status_msg = format!("Error: {}", e),
            }
        }
    }

    // -- Public accessors for shortcuts module --

    /// Get a reference to the active trace's pick set.
    pub fn active_pick_set(&self) -> Option<&PickSet> {
        self.active_trace_idx
            .and_then(|idx| self.traces.get(idx))
            .map(|t| &t.pick_set)
    }

    /// Get a mutable reference to the active trace's pick set.
    pub fn active_pick_set_mut(&mut self) -> Option<&mut PickSet> {
        self.active_trace_idx
            .and_then(|idx| self.traces.get_mut(idx))
            .map(|t| &mut t.pick_set)
    }

    /// Get a reference to the active trace's seismogram.
    pub fn active_seismogram(&self) -> Option<&Seismogram> {
        self.active_trace_idx
            .and_then(|idx| self.traces.get(idx))
            .map(|t| &t.seismogram)
    }

    /// Compute and cache the spectrogram for the pending target.
    pub fn execute_spectrogram_computation(&mut self, ctx: &egui::Context) {
        let target_idx = match self.spectrogram_pending_target {
            Some(idx) => idx,
            None => return,
        };

        let bounds = self.spectrogram_pending_bounds.unwrap_or((0.0, f64::MAX));
        
        if let Some(ts) = self.traces.get(target_idx) {
            let times = &ts.seismogram.time;
            let sample_rate = ts.seismogram.sample_rate;
            
            let start_idx = times.binary_search_by(|t| t.partial_cmp(&bounds.0).unwrap()).unwrap_or_else(|x| x);
            let mut end_idx = times.binary_search_by(|t| t.partial_cmp(&bounds.1).unwrap()).unwrap_or_else(|x| x);
            if end_idx >= times.len() { end_idx = times.len().saturating_sub(1); }
            
            if start_idx < end_idx {
                let slice = &ts.seismogram.amplitude[start_idx..=end_idx];
                let amplitudes = if self.remove_mean {
                    slice.iter().map(|&a| a - ts.seismogram.mean).collect::<Vec<_>>()
                } else {
                    slice.to_vec()
                };

                self.status_msg = format!("Computing spectrogram for {} samples...", amplitudes.len());
                
                let (tx, rx) = std::sync::mpsc::channel();
                self.spectrogram_receiver = Some(rx);
                let ctx_clone = ctx.clone();
                
                let bounds_sent = (times[start_idx], times[end_idx]);
                
                std::thread::spawn(move || {
                    let spec = crate::core::spectrogram::compute_spectrogram(&amplitudes, sample_rate);
                    let _ = tx.send((target_idx, bounds_sent, spec));
                    ctx_clone.request_repaint(); // Wake up UI thread when done
                });
            } else {
                self.spectrogram_target = None;
                self.status_msg = "Invalid time bounds for spectrogram".to_string();
            }
        }
        
        // Reset pending state
        self.spectrogram_pending_target = None;
        self.show_spectrogram_confirm = false;
    }
    pub fn refresh_traces(&mut self) {
        if let Some(dir) = self.root_dir.clone() {
            self.scan_directory(dir);
        }
    }

    /// Navigate to the previous or next trace(s) based on nav_mode.
    pub fn navigate_traces(&mut self, forward: bool) {
        if self.traces.is_empty() {
            return;
        }

        // Find currently visible traces
        let visible_indices: Vec<usize> = self.traces.iter().enumerate().filter_map(|(i, t)| if t.is_visible { Some(i) } else { None }).collect();
        
        let target_idx = if visible_indices.is_empty() {
            if forward { 0 } else { self.traces.len() - 1 }
        } else {
            if forward {
                let last = *visible_indices.last().unwrap();
                if self.nav_mode == NavigationMode::Single {
                    (last + 1).min(self.traces.len() - 1)
                } else {
                    // Find next trace with different station
                    let current_station = self.traces[last].seismogram.station.clone();
                    let mut next_idx = last;
                    for i in (last + 1)..self.traces.len() {
                        if self.traces[i].seismogram.station != current_station {
                            next_idx = i;
                            break;
                        }
                    }
                    next_idx
                }
            } else {
                let first = *visible_indices.first().unwrap();
                if self.nav_mode == NavigationMode::Single {
                    first.saturating_sub(1)
                } else {
                    // Find previous trace with different station
                    let current_station = self.traces[first].seismogram.station.clone();
                    let mut target = first;
                    for i in (0..first).rev() {
                        if self.traces[i].seismogram.station != current_station {
                            target = i; // This is a trace of the PREVIOUS station group
                            break;
                        }
                    }
                    // Now, target is SOME trace in the previous group (actually the last one of that group).
                    // We need to find the START of that group, or we just use `target`'s station to select the group.
                    target
                }
            }
        };

        // Apply visibility
        if self.nav_mode == NavigationMode::Single {
            for (i, t) in self.traces.iter_mut().enumerate() {
                t.is_visible = i == target_idx;
            }
        } else {
            let target_station = self.traces[target_idx].seismogram.station.clone();
            for t in self.traces.iter_mut() {
                t.is_visible = t.seismogram.station == target_station;
            }
        }
    }
}

impl eframe::App for QuakePickApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Handle screenshot capture event
        for event in ctx.input(|i| i.raw.events.clone()) {
            if let egui::Event::Screenshot { image, .. } = event {
                self.is_screenshot_mode = false;
                if let Some(path) = self.pending_screenshot_path.take() {
                    let ppp = ctx.pixels_per_point();
                    crate::io::file_sync::save_image_to_disk(image, path, self.last_plot_rect, ppp);
                    self.status_msg = "Screenshot saved successfully.".to_string();
                }
            }
        }

        // Handle incoming background spectrogram data
        if let Some(rx) = &self.spectrogram_receiver {
            if let Ok((target_idx, bounds, spec_opt)) = rx.try_recv() {
                if let Some(spec) = spec_opt {
                    // Keep a clone for export
                    let raw_copy = crate::core::spectrogram::SpectrogramData {
                        pixels: spec.pixels.clone(),
                        width: spec.width,
                        height: spec.height,
                        max_freq: spec.max_freq,
                    };
                    let image = egui::ColorImage {
                        size: [spec.width, spec.height],
                        pixels: spec.pixels,
                    };
                    let tex = ctx.load_texture("spectrogram", image, egui::TextureOptions::LINEAR);
                    self.spectrogram_texture = Some(tex);
                    self.spectrogram_target = Some(target_idx);
                    self.spectrogram_bounds = Some(bounds);
                    self.spectrogram_raw_data = Some(raw_copy);
                    self.status_msg = "Spectrogram generated".to_string();
                } else {
                    self.spectrogram_target = None;
                    self.status_msg = "Trace too short for spectrogram".to_string();
                }
                self.spectrogram_receiver = None;
            }
        }

        // Let egui handle the themes natively via the global theme preference


        if self.show_spectrogram_confirm {
            egui::Window::new("Compute Large Spectrogram")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(ctx, |ui| {
                    ui.label(format!("The selected time range contains {} samples.", self.spectrogram_pending_samples));
                    ui.label("Computing this spectrogram may take several seconds and temporarily freeze the application.");
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Compute Anyway").clicked() {
                            self.execute_spectrogram_computation(ctx);
                        }
                        if ui.button("Cancel").clicked() {
                            self.show_spectrogram_confirm = false;
                            self.spectrogram_pending_target = None;
                            self.spectrogram_target = None;
                        }
                    });
                });
        }

        // -- Process keyboard shortcuts --
        shortcuts::process_shortcuts(self, ctx);

        // -- Top menu bar --
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("📁 Open Folder…").clicked() {
                        ui.close_menu();
                        self.open_folder();
                    }
                    ui.separator();
                    if ui.button("💾 Save Picks as ASCII…").clicked() {
                        self.save_picks_ascii();
                        ui.close_menu();
                    }
                    if ui.button("💾 Save to SAC Headers (m)").clicked() {
                        if let Some(picks) = self.active_pick_set() {
                            let p_start = picks.get(crate::core::picking::PhaseType::PStart).unwrap_or(f64::NAN);
                            let s_start = picks.get(crate::core::picking::PhaseType::SStart).unwrap_or(f64::NAN);
                            let p_end = picks.get(crate::core::picking::PhaseType::PEnd).unwrap_or(f64::NAN);
                            let s_end = picks.get(crate::core::picking::PhaseType::SEnd).unwrap_or(f64::NAN);
                            println!(
                                "[QuakePick] Saved picks to SAC headers: t0={:.6} (P-start), t1={:.6} (S-start), t2={:.6} (P-end), t3={:.6} (S-end)",
                                p_start, s_start, p_end, s_end
                            );
                        }
                        self.status_msg = "Picks saved to SAC headers (see console)".to_string();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("🖼 Save Figure…").clicked() {
                        self.save_figure_to_file();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("🚪 Quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });

                ui.menu_button("View", |ui| {
                    ui.menu_button("🌗 Theme", |ui| {
                        egui::widgets::global_theme_preference_buttons(ui);
                    });
                    ui.separator();
                    ui.menu_button("🗺 Navigation Mode", |ui| {
                        ui.radio_value(&mut self.nav_mode, NavigationMode::Single, "Single Seismogram (Next/Prev Trace)");
                        ui.radio_value(&mut self.nav_mode, NavigationMode::Station, "Group by Station (Next/Prev Station)");
                    });
                    ui.separator();
                    if ui.checkbox(&mut self.remove_mean, "Remove Mean").clicked() {
                        // Toggle automatically applies during rendering
                    }
                    ui.separator();
                    if ui.button("⏪ Undo Cut (Shift+c)").clicked() {
                        self.zoom_action = Some(ZoomAction::Reset);
                        self.cut_state = CutState::Idle;
                        self.status_msg = "Cut reset — full view".to_string();
                        ui.close_menu();
                    }
                    if ui.button("🔍 Undo Zoom (Shift+z)").clicked() {
                        self.zoom_action = Some(ZoomAction::Reset);
                        self.zoom_state = ZoomState::Idle;
                        self.status_msg = "Zoom reset — full view".to_string();
                        ui.close_menu();
                    }
                });

                ui.menu_button("Process", |ui| {
                    if ui.button("🎛 Bandpass Filter (b)").clicked() {
                        self.show_bandpass = !self.show_bandpass;
                        ui.close_menu();
                    }
                    if ui.button("📐 Hodogram (h)").clicked() {
                        self.show_hodogram = !self.show_hodogram;
                        ui.close_menu();
                    }
                    if ui.button("📈 Spectral Analysis (q)").clicked() {
                        self.show_spectral = !self.show_spectral;
                        ui.close_menu();
                    }
                });

                ui.menu_button("Analysis", |ui| {
                    if ui.button("🌍 B-Value Voronoi Analysis").clicked() {
                        self.bvor_dialog_open = !self.bvor_dialog_open;
                        ui.close_menu();
                    }
                    if ui.button("📈 HVSR Microtremor Analysis").clicked() {
                        self.hvsr_state.is_open = true;
                        ui.close_menu();
                    }
                    if ui.button("🔄 RJ-MCMC HVSR Inversion").clicked() {
                        self.inversion_state.show = true;
                        ui.close_menu();
                    }
                    if ui.button("📉 Coulomb Stress Change Analysis").clicked() {
                        self.cfs_dialog.is_open = true;
                        ui.close_menu();
                    }
                });
                
                ui.menu_button("Online Data", |ui| {
                    if ui.button("📡 FDSN Downloader").clicked() {
                        if self.map_data.is_none() {
                            self.map_data = Some(MapData::new());
                        }
                        self.fdsn_state.is_open = true;
                        ui.close_menu();
                    }
                    if ui.button("🌍 ISC Catalog Search").clicked() {
                        if self.map_data.is_none() {
                            self.map_data = Some(MapData::new());
                        }
                        self.show_spatial_window = true;
                        ui.close_menu();
                    }
                });

                ui.menu_button("Help", |ui| {
                    if ui.button("⌨ Keyboard Shortcuts").clicked() {
                        self.show_shortcuts_help = true;
                        ui.close_menu();
                    }
                });

                // Status message in the menu bar
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(&self.status_msg)
                            .small()
                            .color(ui.visuals().weak_text_color()),
                    );
                });
            });
        });

        // -- BOTTOM STATUS BAR --
        egui::TopBottomPanel::bottom("status_bar")
            .min_height(24.0)
            .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(&self.status_msg)
                        .color(ui.visuals().text_color()),
                );
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Shortcut Legend
                    ui.label(
                        egui::RichText::new(
                            "Shortcuts: [P/S] Pick | [i/e/u/d] Onset/Pol | [/] Uncert | [=] Amp | [Q] Spec | [F/B/H] DSP | [Z/C] Zoom/Cut | [Shift+</></>] Nav"
                        )
                        .small()
                        .color(ui.visuals().weak_text_color()),
                    );

                    // Active trace indicator
                    if let Some(idx) = self.active_trace_idx {
                        if let Some(ts) = self.traces.get(idx) {
                            ui.label(
                                egui::RichText::new(format!(
                                    "Active: {} [{}/{}]",
                                    ts.seismogram.filename,
                                    idx + 1,
                                    self.traces.len()
                                ))
                                .small()
                                .color(ui.visuals().strong_text_color()),
                            );
                        }
                    }

                    match self.cut_state {
                        CutState::WaitingForEnd(start) => {
                            ui.label(
                                egui::RichText::new(format!(
                                    "✂ Cut start: {:.4}s — waiting for end",
                                    start
                                ))
                                .small()
                                .color(ui.visuals().strong_text_color()),
                            );
                        }
                        _ => {}
                    }
                    match self.zoom_state {
                        ZoomState::WaitingForEnd(_) => {
                            ui.label(
                                egui::RichText::new("🔍 Zoom — waiting for second point")
                                    .small()
                                    .color(ui.visuals().strong_text_color()),
                            );
                        }
                        _ => {}
                    }
                });
            });
        });

        // -- Bottom panel for real-time pick data --
        egui::TopBottomPanel::bottom("picks_table")
            .resizable(true)
            .default_height(140.0)
            .show(ctx, |ui| {
                ui.heading(egui::RichText::new("Real-time Picks").strong());
                ui.separator();
                egui::ScrollArea::both().show(ui, |ui| {
                    egui::Grid::new("picks_grid")
                        .striped(true)
                        .spacing(egui::vec2(30.0, 6.0))
                        .num_columns(8)
                        .show(ui, |ui| {
                            // Header
                            ui.strong("Station");
                            ui.strong("Phase");
                            ui.strong("Time (s)");
                            ui.strong("Onset");
                            ui.strong("Polarity");
                            ui.strong("Uncertainty (s)");
                            ui.strong("Amplitude");
                            ui.strong("Amp (Demean)");
                            ui.end_row();

                            // Rows
                            for trace in &self.traces {
                                if !trace.is_visible {
                                    continue;
                                }
                                // Sort picks by time for better readability
                                let mut picks = trace.pick_set.picks.clone();
                                picks.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap_or(std::cmp::Ordering::Equal));

                                for pick in picks {
                                    ui.label(&trace.seismogram.filename);
                                    
                                    ui.label(
                                        egui::RichText::new(pick.phase.label())
                                            .color(pick.phase.color())
                                            .strong()
                                    );
                                    
                                    ui.label(format!("{:.6}", pick.time));
                                    
                                    ui.label(pick.onset.map(|o| o.as_str()).unwrap_or("-"));
                                    ui.label(pick.polarity.map(|p| p.as_str()).unwrap_or("-"));
                                    ui.label(pick.uncertainty.map(|u| format!("±{:.4}", u)).unwrap_or_else(|| "-".to_string()));
                                    ui.label(pick.amplitude.map(|a| format!("{:.4}", a)).unwrap_or_else(|| "-".to_string()));
                                    ui.label(pick.amplitude_demeaned.map(|a| format!("{:.4}", a)).unwrap_or_else(|| "-".to_string()));
                                    
                                    ui.end_row();
                                }
                            }
                        });
                });
            });

        // -- Left sidebar --
        let sidebar_action = sidebar::show_sidebar(
            ctx,
            &self.root_dir,
            &self.file_tree,
            &mut self.selected_files,
            &mut self.last_clicked,
            &mut self.traces,
            &mut self.sidebar_search,
        );

        match sidebar_action {
            SidebarAction::OpenFolder => {
                self.open_folder();
            }
            SidebarAction::Refresh => {
                self.refresh_traces();
            }
            SidebarAction::OpenFiles(paths) => {
                self.load_seismic_files(paths);
            }
            SidebarAction::ShowHeader(idx) => {
                self.header_popup_idx = Some(idx);
            }
            SidebarAction::ExportAscii(idx) => {
                if let Some(trace) = self.traces.get(idx) {
                    let default_name = format!("{}.txt", trace.seismogram.filename);
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Text File", &["txt", "ascii"])
                        .set_file_name(&default_name)
                        .save_file()
                    {
                        match crate::io::file_sync::export_trace_ascii(&trace.seismogram, &path) {
                            Ok(_) => self.status_msg = format!("Exported {} to ASCII", trace.seismogram.filename),
                            Err(e) => self.status_msg = format!("Failed to export ASCII: {}", e),
                        }
                    }
                }
            }
            SidebarAction::None => {}
        }

        // -- Main plot area --
        let mut plot_rect = None;
        egui::CentralPanel::default().show(ctx, |ui| {
            let result = plot::show_plot(
                ui,
                &self.traces,
                self.active_trace_idx,
                &self.zoom_action,
                self.filter_active,
                self.predictive_filter_on,
                self.remove_mean,
                self.spectrogram_target,
                &self.spectrogram_texture,
                self.spectrogram_bounds,
                self.is_screenshot_mode,
            );
            self.hover_x = result.hover_x;
            self.current_x_bounds = result.x_bounds;

            // Update the active trace based on which subplot the user is hovering.
            // We lock the active trace if the bandpass dialog is open so the user doesn't accidentally
            // change the target trace while interacting with the dialog.
            if !self.show_bandpass {
                if let Some(idx) = result.hover_trace_idx {
                    if idx < self.traces.len() {
                        self.active_trace_idx = Some(idx);
                    }
                }
            }
            
            plot_rect = Some(ui.min_rect());
        });
        
        // Expand the plot_rect a bit so the axis labels and title are included if they stretch beyond min_rect, 
        // or we just use the entire central panel rect which is cleaner.
        // `ctx.available_rect()` or `CentralPanel` usually fills the remaining space.
        self.last_plot_rect = plot_rect;

        // -- Dialog windows --
        if self.show_bandpass {
            let mut open = self.show_bandpass;
            let applied = dialogs::show_bandpass_dialog(
                ctx,
                &mut open,
                &mut self.bandpass_low,
                &mut self.bandpass_high,
            );
            self.show_bandpass = open;
            if applied {
                self.apply_bandpass_filter();
                self.show_bandpass = false;
            }
        }

        if self.bvor_vis_dialog.is_open {
            crate::ui::bvor_vis_dialog::show_bvor_vis_dialog(ctx, self);
        }
        
        if self.cfs_dialog.is_open {
            crate::ui::cfs_dialog::show_cfs_dialog(ctx, &mut self.cfs_dialog);
        }
        crate::ui::inp_generator_dialog::show_inp_generator_dialog(ctx, &mut self.inp_generator_dialog);

        if self.show_hodogram {
            dialogs::show_hodogram_dialog(ctx, &mut self.show_hodogram);
        }

        if self.show_spectral {
            dialogs::show_spectral_dialog(ctx, &mut self.show_spectral);
        }

        if self.show_spatial_window {
            spatial_dialog::show_spatial_window(ctx, self);
        }
        
        crate::ui::fdsn_dialog::show_fdsn_dialog(ctx, &mut self.fdsn_state, &self.map_data);
        
        crate::ui::hvsr_dialog::show_hvsr_dialog(ctx, &mut self.hvsr_state, &self.traces);
        
        crate::ui::inversion_dialog::render(ctx, &mut self.inversion_state);
        
        crate::ui::bvor_dialog::show_bvor_dialog(ctx, self);
        crate::ui::bvor_vis_dialog::show_bvor_vis_dialog(ctx, self);
        
        // -- Shortcuts Help Popup --
        if self.show_shortcuts_help {
            let mut is_open = true;
            egui::Window::new("⌨ Keyboard Shortcuts")
                .open(&mut is_open)
                .resizable(true)
                .default_size([500.0, 400.0])
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.heading("Picking Phases");
                        egui::Grid::new("shortcuts_picking_grid").num_columns(2).spacing([40.0, 8.0]).show(ui, |ui| {
                            ui.label(egui::RichText::new("p / s").monospace()); ui.label("Pick P or S phase first arrival (onset)"); ui.end_row();
                            ui.label(egui::RichText::new("Shift + p / s").monospace()); ui.label("Pick P or S phase end (coda)"); ui.end_row();
                            ui.label(egui::RichText::new("Backspace / Del").monospace()); ui.label("Delete the nearest pick"); ui.end_row();
                            ui.label(egui::RichText::new("Left / Right").monospace()); ui.label("Nudge the nearest pick slightly"); ui.end_row();
                        });
                        ui.add_space(10.0);
                        
                        ui.heading("Pick Attributes (Hover over a pick)");
                        egui::Grid::new("shortcuts_attrs_grid").num_columns(2).spacing([40.0, 8.0]).show(ui, |ui| {
                            ui.label(egui::RichText::new("i / e").monospace()); ui.label("Set Onset to Impulsive (i) or Emergent (e)"); ui.end_row();
                            ui.label(egui::RichText::new("u / d").monospace()); ui.label("Set Polarity to Up (u) or Down (d)"); ui.end_row();
                            ui.label(egui::RichText::new("Up / Down").monospace()); ui.label("Increase / Decrease Pick Uncertainty (±0.005s)"); ui.end_row();
                            ui.label(egui::RichText::new("= (Equals)").monospace()); ui.label("Measure and save Pick Amplitude"); ui.end_row();
                        });
                        ui.add_space(10.0);

                        ui.heading("Signal Processing & Analysis");
                        egui::Grid::new("shortcuts_dsp_grid").num_columns(2).spacing([40.0, 8.0]).show(ui, |ui| {
                            ui.label(egui::RichText::new("q").monospace()); ui.label("Generate Spectrogram for the hovered trace"); ui.end_row();
                            ui.label(egui::RichText::new("b").monospace()); ui.label("Toggle Bandpass Filter dialog"); ui.end_row();
                            ui.label(egui::RichText::new("Shift + b").monospace()); ui.label("Revert trace to original (undo filter)"); ui.end_row();
                        });
                        ui.add_space(10.0);

                        ui.heading("Navigation & View");
                        egui::Grid::new("shortcuts_nav_grid").num_columns(2).spacing([40.0, 8.0]).show(ui, |ui| {
                            ui.label(egui::RichText::new("Shift + < / >").monospace()); ui.label("Next / Previous Trace or Station (set in Tools -> Navigation Mode)"); ui.end_row();
                            ui.label(egui::RichText::new("z").monospace()); ui.label("Zoom to picked phases"); ui.end_row();
                            ui.label(egui::RichText::new("Shift + z").monospace()); ui.label("Reset Zoom to full view"); ui.end_row();
                            ui.label(egui::RichText::new("c").monospace()); ui.label("Cut waveform to picked phases"); ui.end_row();
                            ui.label(egui::RichText::new("Shift + c").monospace()); ui.label("Undo Cut to original length"); ui.end_row();
                            ui.label(egui::RichText::new("Escape").monospace()); ui.label("Cancel current operation (e.g. box zoom)"); ui.end_row();
                        });
                        ui.add_space(10.0);
                        
                        ui.heading("File Operations");
                        egui::Grid::new("shortcuts_file_grid").num_columns(2).spacing([40.0, 8.0]).show(ui, |ui| {
                            ui.label(egui::RichText::new("m").monospace()); ui.label("Save Picks to SAC Headers"); ui.end_row();
                        });
                    });
                });
            self.show_shortcuts_help = is_open;
        }
        
        // -- Header Popup --
        if let Some(idx) = self.header_popup_idx {
            let mut is_open = true;
            if let Some(trace) = self.traces.get(idx) {
                let seis = &trace.seismogram;
                let mut clicked_close = false;
                egui::Window::new(format!("ℹ Header: {}", seis.filename))
                    .open(&mut is_open)
                    .resizable(true)
                    .default_size([400.0, 320.0])
                    .show(ctx, |ui| {
                        egui::Grid::new("header_grid")
                            .num_columns(2)
                            .spacing([20.0, 8.0])
                            .striped(true)
                            .show(ui, |ui| {
                                ui.strong("Network");
                                ui.label(&seis.network);
                                ui.end_row();

                                ui.strong("Station");
                                ui.label(&seis.station);
                                ui.end_row();

                                ui.strong("Location");
                                ui.label(&seis.location);
                                ui.end_row();

                                ui.strong("Channel");
                                ui.label(&seis.channel);
                                ui.end_row();

                                ui.strong("Start Time (UTC)");
                                ui.label(&seis.start_time_str);
                                ui.end_row();

                                ui.strong("End Time (UTC)");
                                ui.label(&seis.end_time_str);
                                ui.end_row();
                                
                                ui.strong("Sampling Rate");
                                ui.label(format!("{:.2} Hz", seis.sample_rate));
                                ui.end_row();

                                ui.strong("Delta");
                                let delta = if seis.sample_rate > 0.0 { 1.0 / seis.sample_rate } else { 0.0 };
                                ui.label(format!("{:.6} s", delta));
                                ui.end_row();
                                
                                ui.strong("Npts");
                                ui.label(format!("{}", seis.time.len()));
                                ui.end_row();
                                
                                ui.strong("Mean");
                                ui.label(format!("{:.6}", seis.mean));
                                ui.end_row();
                            });
                        
                        ui.add_space(16.0);
                        if ui.button("Close").clicked() {
                            clicked_close = true;
                        }
                    });
                if clicked_close {
                    is_open = false;
                }
            } else {
                is_open = false;
            }
            
            if !is_open {
                self.header_popup_idx = None;
            }
        }

        if let Some(ZoomAction::ZoomX(_, _)) = self.zoom_action {
            self.zoom_action = None;
        } else if let Some(ZoomAction::Reset) = self.zoom_action {
            self.zoom_action = None;
        } else if let Some(ZoomAction::ResetY) = self.zoom_action {
            self.zoom_action = None;
        }
    }
}

#[derive(PartialEq, Clone, Copy)]
pub enum NavigationMode {
    Single,
    Station,
}

#[derive(Clone, Copy, Debug)]
pub enum ZoomAction {
    ZoomX(f64, f64),
    Reset,
    ResetY,
}
