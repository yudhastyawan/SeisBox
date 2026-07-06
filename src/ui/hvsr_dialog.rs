use eframe::egui;
use egui_plot::{Plot, Line, PlotPoints, Polygon, VLine, Text, PlotPoint, Legend};
use crate::core::math_hvsr::{HvsrParams, HvsrResult, HvsrProgress, process_hvsr_pipeline};
use crate::ui::plot::TraceState;
use std::sync::mpsc::Receiver;
use std::path::PathBuf;

pub struct HvsrState {
    pub params: HvsrParams,
    pub is_open: bool,
    pub is_processing: bool,
    pub progress_msg: String,
    pub progress_pct: f32,
    pub result: Option<HvsrResult>,
    pub windows: Option<crate::core::math_hvsr::HvsrWindows>,
    pub receiver: Option<Receiver<HvsrProgress>>,
    pub z_comp: Vec<f64>,
    pub n_comp: Vec<f64>,
    pub e_comp: Vec<f64>,
    pub z_comp_plot: Option<Vec<[f64; 2]>>,
    pub n_comp_plot: Option<Vec<[f64; 2]>>,
    pub e_comp_plot: Option<Vec<[f64; 2]>>,
    pub dt: f64,
}

impl Default for HvsrState {
    fn default() -> Self {
        Self {
            params: HvsrParams::default(),
            is_open: false,
            is_processing: false,
            progress_msg: "".to_string(),
            progress_pct: 0.0,
            result: None,
            windows: None,
            receiver: None,
            z_comp: Vec::new(),
            n_comp: Vec::new(),
            e_comp: Vec::new(),
            z_comp_plot: None,
            n_comp_plot: None,
            e_comp_plot: None,
            dt: 0.01,
        }
    }
}

pub fn show_hvsr_dialog(
    ctx: &egui::Context,
    state: &mut HvsrState,
    traces: &[TraceState]
) {
    if !state.is_open {
        return;
    }
    
    if let Some(rx) = &state.receiver {
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
                    state.result = Some(res);
                    state.progress_msg = "Complete".to_string();
                    state.progress_pct = 1.0;
                    state.is_processing = false;
                }
            }
        }
    }
    
    // Auto-fetch and detrend components if empty and the window is open
    if state.z_comp.is_empty() && state.is_open {
        let z = traces.iter().find(|t| t.seismogram.channel.ends_with('Z') || t.seismogram.filename.contains("BHZ") || t.seismogram.filename.contains("HHZ") || t.seismogram.filename.contains("EHZ") || t.seismogram.filename.ends_with('Z')).map(|t| &t.seismogram);
        let n = traces.iter().find(|t| t.seismogram.channel.ends_with('N') || t.seismogram.channel.ends_with('1') || t.seismogram.filename.contains("BHN") || t.seismogram.filename.contains("HHN") || t.seismogram.filename.contains("EHN") || t.seismogram.filename.ends_with('N') || t.seismogram.filename.ends_with('1')).map(|t| &t.seismogram);
        let e = traces.iter().find(|t| t.seismogram.channel.ends_with('E') || t.seismogram.channel.ends_with('2') || t.seismogram.filename.contains("BHE") || t.seismogram.filename.contains("HHE") || t.seismogram.filename.contains("EHE") || t.seismogram.filename.ends_with('E') || t.seismogram.filename.ends_with('2')).map(|t| &t.seismogram);
        
        if let (Some(zs), Some(ns), Some(es)) = (z, n, e) {
            let mut z_data = zs.amplitude.clone();
            let mut n_data = ns.amplitude.clone();
            let mut e_data = es.amplitude.clone();
            
            crate::core::math_hvsr::detrend_signal(&mut z_data);
            crate::core::math_hvsr::detrend_signal(&mut n_data);
            crate::core::math_hvsr::detrend_signal(&mut e_data);
            
            state.dt = 1.0 / zs.sample_rate;
            
            state.z_comp_plot = crate::ui::plot::decimate_for_plot(&zs.time, &z_data);
            state.n_comp_plot = crate::ui::plot::decimate_for_plot(&ns.time, &n_data);
            state.e_comp_plot = crate::ui::plot::decimate_for_plot(&es.time, &e_data);
            
            state.z_comp = z_data;
            state.n_comp = n_data;
            state.e_comp = e_data;
        }
    }

    let mut is_open = state.is_open;
    egui::Window::new("HVSR Microtremor Analysis (SESAME 2004)")
        .open(&mut is_open)
        .default_size([1100.0, 750.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Left Panel
                ui.vertical(|ui| {
                    ui.set_width(300.0);
                    ui.heading("Parameters");
                    ui.add_space(10.0);
                    
                    egui::CollapsingHeader::new("Windowing & Tapering").default_open(true).show(ui, |ui| {
                        egui::Grid::new("hvsr_win_params").num_columns(2).show(ui, |ui| {
                            ui.label("Window Length (s)").on_hover_text("Length of each time window in seconds.");
                            ui.add(egui::DragValue::new(&mut state.params.window_len_s).speed(1.0).clamp_range(10.0..=120.0));
                            ui.end_row();
                            
                            ui.label("Overlap (%)").on_hover_text("Overlap percentage between consecutive windows.");
                            ui.add(egui::DragValue::new(&mut state.params.overlap_pct).speed(5.0).clamp_range(0.0..=90.0));
                            ui.end_row();
                        });
                    });
                    ui.add_space(5.0);

                    egui::CollapsingHeader::new("Anti-Trigger (STA/LTA)").default_open(true).show(ui, |ui| {
                        egui::Grid::new("hvsr_sta_params").num_columns(2).show(ui, |ui| {
                            ui.label("STA Length (s)").on_hover_text("Short-Time Average window length (e.g. 1.0s).");
                            ui.add(egui::DragValue::new(&mut state.params.sta_len_s).speed(0.1).clamp_range(0.1..=5.0));
                            ui.end_row();
                            
                            ui.label("LTA Length (s)").on_hover_text("Long-Time Average window length (e.g. 30.0s).");
                            ui.add(egui::DragValue::new(&mut state.params.lta_len_s).speed(1.0).clamp_range(10.0..=100.0));
                            ui.end_row();
                            
                            ui.label("Threshold T1 (Lower)").on_hover_text("Lower bound for STA/LTA ratio (rejects windows below this).");
                            ui.add(egui::DragValue::new(&mut state.params.t1).speed(0.05).clamp_range(0.0..=1.0));
                            ui.end_row();
                            
                            ui.label("Threshold T2 (Upper)").on_hover_text("Upper bound for STA/LTA ratio (rejects transients above this).");
                            ui.add(egui::DragValue::new(&mut state.params.t2).speed(0.1).clamp_range(1.5..=10.0));
                            ui.end_row();
                        });
                    });
                    ui.add_space(5.0);

                    egui::CollapsingHeader::new("Smoothing & Frequency").default_open(true).show(ui, |ui| {
                        egui::Grid::new("hvsr_smooth_params").num_columns(2).show(ui, |ui| {
                            ui.label("Konno-Ohmachi (b)").on_hover_text("Smoothing coefficient, typical value is 40.");
                            ui.add(egui::DragValue::new(&mut state.params.b_value).speed(1.0).clamp_range(10.0..=100.0));
                            ui.end_row();
                            
                            ui.label("Min Frequency (Hz)").on_hover_text("Minimum frequency for HVSR curve.");
                            ui.add(egui::DragValue::new(&mut state.params.freq_min).speed(0.1).clamp_range(0.1..=10.0));
                            ui.end_row();
                            
                            ui.label("Max Frequency (Hz)").on_hover_text("Maximum frequency for HVSR curve.");
                            ui.add(egui::DragValue::new(&mut state.params.freq_max).speed(1.0).clamp_range(5.0..=50.0));
                            ui.end_row();
                            
                            ui.label("Sample Count").on_hover_text("Number of log-spaced frequency points.");
                            ui.add(egui::DragValue::new(&mut state.params.freq_count).speed(10.0).clamp_range(50..=1000));
                            ui.end_row();
                            
                            ui.label("Horiz Combine").on_hover_text("Method to combine North and East components");
                            egui::ComboBox::from_id_source("horiz_combine")
                                .selected_text(match state.params.combine_method {
                                    crate::core::math_hvsr::HorizontalCombineMethod::Geometric => "Geometric Mean",
                                    crate::core::math_hvsr::HorizontalCombineMethod::Quadratic => "Quadratic Mean (RMS)",
                                    crate::core::math_hvsr::HorizontalCombineMethod::Arithmetic => "Arithmetic Mean",
                                    crate::core::math_hvsr::HorizontalCombineMethod::Maximum => "Maximum",
                                })
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut state.params.combine_method, crate::core::math_hvsr::HorizontalCombineMethod::Geometric, "Geometric Mean");
                                    ui.selectable_value(&mut state.params.combine_method, crate::core::math_hvsr::HorizontalCombineMethod::Quadratic, "Quadratic Mean (RMS)");
                                    ui.selectable_value(&mut state.params.combine_method, crate::core::math_hvsr::HorizontalCombineMethod::Arithmetic, "Arithmetic Mean");
                                    ui.selectable_value(&mut state.params.combine_method, crate::core::math_hvsr::HorizontalCombineMethod::Maximum, "Maximum");
                                });
                            ui.end_row();
                        });
                    });
                    ui.add_space(5.0);
                    
                    egui::CollapsingHeader::new("f0 Filter (Iterative)").default_open(true).show(ui, |ui| {
                        ui.checkbox(&mut state.params.enable_f0_filter, "Enable Iterative f0 Filter")
                          .on_hover_text("If enabled, filters out windows whose peak frequency (f0) is far from the mean peak frequency.");
                        
                        if state.params.enable_f0_filter {
                            egui::Grid::new("hvsr_f0_params").num_columns(2).show(ui, |ui| {
                                ui.label("Multiplier (n)").on_hover_text("Standard deviation multiplier for bounds (lb, ub).");
                                ui.add(egui::DragValue::new(&mut state.params.f0_filter_n).speed(0.1).clamp_range(0.5..=5.0));
                                ui.end_row();
                            });
                        }
                    });
                    
                    ui.add_space(10.0);
                    
                    if ui.button(egui::RichText::new("1. Preview Windows (STA/LTA)").size(16.0)).clicked() {
                        if state.z_comp.len() > 0 && state.n_comp.len() > 0 && state.e_comp.len() > 0 {
                            match crate::core::math_hvsr::compute_windows(&state.z_comp, &state.n_comp, &state.e_comp, state.dt, &state.params) {
                                Ok(windows) => {
                                    state.windows = Some(windows);
                                    state.progress_msg = "Windows computed".to_string();
                                }
                                Err(e) => {
                                    state.progress_msg = format!("Error: {}", e);
                                }
                            }
                        } else {
                            state.progress_msg = "Error: Missing components".to_string();
                        }
                    }
                    
                    ui.add_space(10.0);
                    
                    if ui.add_enabled(!state.is_processing, egui::Button::new(egui::RichText::new("2. Process HVSR").size(16.0))).clicked() {
                        if state.z_comp.is_empty() || state.n_comp.is_empty() || state.e_comp.is_empty() {
                            state.progress_msg = "Error: Missing Z, N, E components".to_string();
                        } else if state.windows.is_none() {
                            state.progress_msg = "Error: Please preview windows first".to_string();
                        } else {
                            let (tx, rx) = std::sync::mpsc::channel();
                            state.receiver = Some(rx);
                            state.is_processing = true;
                            state.result = None;
                            state.progress_pct = 0.0;
                            state.progress_msg = "Starting...".to_string();
                            
                            let z_c = state.z_comp.clone();
                            let n_c = state.n_comp.clone();
                            let e_c = state.e_comp.clone();
                            let dt_c = state.dt;
                            let p_c = state.params.clone();
                            let w_c = state.windows.as_ref().unwrap().clone();
                            
                            std::thread::spawn(move || {
                                process_hvsr_pipeline(z_c, n_c, e_c, dt_c, p_c, w_c, tx);
                            });
                        }
                    }
                    
                    ui.add_space(10.0);
                    
                    if ui.add_enabled(state.result.is_some() && !state.is_processing, egui::Button::new("💾 Export CSV")).clicked() {
                        if let Some(res) = &state.result {
                            if let Some(path) = rfd::FileDialog::new().add_filter("CSV", &["csv"]).save_file() {
                                let mut csv_content = String::from("Frequency,Mean_HV,Std_Dev_Plus,Std_Dev_Minus\n");
                                let final_stats = if let Some(f0) = &res.f0_stats { f0 } else { &res.sta_lta_stats };
                                for i in 0..res.freq.len() {
                                    csv_content.push_str(&format!("{:.4},{:.4},{:.4},{:.4}\n",
                                        res.freq[i], final_stats.mean_hvsr[i], final_stats.std_plus[i], final_stats.std_minus[i]));
                                }
                                let _ = std::fs::write(path, csv_content);
                                state.progress_msg = "Exported successfully".to_string();
                            }
                        }
                    }
                    
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
                    
                    if let Some(res) = &state.result {
                        ui.add_space(15.0);
                        
                        egui::ScrollArea::vertical().id_source("sesame_scroll").show(ui, |ui| {
                            egui::Frame::group(ui.style()).show(ui, |ui| {
                                ui.set_width(ui.available_width());
                            ui.heading("SESAME Criteria");
                            ui.add_space(5.0);
                            let final_valid_count = if let Some(f0) = &res.f0_stats { f0.valid_indices.len() } else { res.sta_lta_stats.valid_indices.len() };
                            ui.label(format!("Valid Windows (Final): {}/{}", final_valid_count, res.final_valid_idx.len()));
                            
                            ui.add_space(5.0);
                            if let Some(sesame) = &res.sesame_result {
                                let check = |pass: bool| {
                                    if pass { "✅" } else { "❌" }
                                };
                                let color = |pass: bool| {
                                    if pass { egui::Color32::from_rgb(0, 200, 0) } else { egui::Color32::RED }
                                };
                                
                                ui.add_space(5.0);
                                ui.label(egui::RichText::new("Reliability:").strong());
                                ui.label(format!("{} C1: f0 > 10 / lw", check(sesame.reliability_c1)));
                                ui.label(format!("{} C2: nc > 200", check(sesame.reliability_c2)));
                                ui.label(format!("{} C3: sigma_A(f) < limits", check(sesame.reliability_c3)));
                                ui.label(egui::RichText::new(format!("Reliable Curve: {}", if sesame.is_reliable { "YES" } else { "NO" }))
                                    .color(color(sesame.is_reliable)).strong());
                                    
                                ui.add_space(10.0);
                                ui.label(egui::RichText::new("Clear Peak:").strong());
                                ui.label(format!("{} C1: exists f- in [f0/4, f0], A < A0/2", check(sesame.clear_peak_c1)));
                                ui.label(format!("{} C2: exists f+ in [f0, 4*f0], A < A0/2", check(sesame.clear_peak_c2)));
                                ui.label(format!("{} C3: A0 > 2", check(sesame.clear_peak_c3)));
                                ui.label(format!("{} C4: peaks within f0 ± 5%", check(sesame.clear_peak_c4)));
                                ui.label(format!("{} C5: sigma_f < epsilon(f0)", check(sesame.clear_peak_c5)));
                                ui.label(format!("{} C6: sigma_A(f0) < theta(f0)", check(sesame.clear_peak_c6)));
                                
                                let cp_count = [
                                    sesame.clear_peak_c1, sesame.clear_peak_c2, sesame.clear_peak_c3,
                                    sesame.clear_peak_c4, sesame.clear_peak_c5, sesame.clear_peak_c6
                                ].iter().filter(|&&x| x).count();
                                
                                ui.label(egui::RichText::new(format!("Clear Peak: {} ({}/6)", if sesame.is_clear_peak { "YES" } else { "NO" }, cp_count))
                                    .color(color(sesame.is_clear_peak)).strong());
                            }
                        });
                        });
                    }
                });
                
                ui.separator();
                
                // Right Panel (Plots)
                ui.vertical(|ui| {
                    let h1 = ui.available_height() * 0.35;
                    
                    // Time Domain Plot
                    // Time Domain Plot
                    let plot = Plot::new("hvsr_time_plot")
                        .height(h1)
                        .allow_zoom(true)
                        .allow_drag(true)
                        .legend(Legend::default());
                    
                    let plot_response = plot.show(ui, |plot_ui| {
                        // Plot Z, N, E Components
                        if !state.z_comp.is_empty() {
                            let pts_z: PlotPoints = if let Some(cached) = &state.z_comp_plot {
                                PlotPoints::new(cached.clone())
                            } else {
                                state.z_comp.iter().enumerate()
                                    .map(|(i, &v)| [i as f64 * state.dt, v]).collect()
                            };
                            plot_ui.line(Line::new(pts_z).color(egui::Color32::from_rgba_unmultiplied(100, 100, 100, 150)).name("Z"));
                        }
                        if !state.n_comp.is_empty() {
                            let pts_n: PlotPoints = if let Some(cached) = &state.n_comp_plot {
                                PlotPoints::new(cached.clone())
                            } else {
                                state.n_comp.iter().enumerate()
                                    .map(|(i, &v)| [i as f64 * state.dt, v]).collect()
                            };
                            plot_ui.line(Line::new(pts_n).color(egui::Color32::from_rgba_unmultiplied(200, 50, 50, 100)).name("N"));
                        }
                        if !state.e_comp.is_empty() {
                            let pts_e: PlotPoints = if let Some(cached) = &state.e_comp_plot {
                                PlotPoints::new(cached.clone())
                            } else {
                                state.e_comp.iter().enumerate()
                                    .map(|(i, &v)| [i as f64 * state.dt, v]).collect()
                            };
                            plot_ui.line(Line::new(pts_e).color(egui::Color32::from_rgba_unmultiplied(50, 50, 200, 100)).name("E"));
                        }
                        
                        // Find max amplitude for polygon bounds
                        let mut max_amp = 1.0_f64;
                        if let Some(cached) = &state.z_comp_plot {
                            for &[_, a] in cached { if a.abs() > max_amp { max_amp = a.abs(); } }
                        }
                        if let Some(cached) = &state.n_comp_plot {
                            for &[_, a] in cached { if a.abs() > max_amp { max_amp = a.abs(); } }
                        }
                        if let Some(cached) = &state.e_comp_plot {
                            for &[_, a] in cached { if a.abs() > max_amp { max_amp = a.abs(); } }
                        }
                        let y_bound = max_amp * 1.2;
                        
                        // Draw windows if available
                        if let Some(windows) = &state.windows {
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
                                let color = if is_valid {
                                    egui::Color32::from_rgba_unmultiplied(0, 255, 0, 40)
                                } else {
                                    egui::Color32::from_rgba_unmultiplied(255, 0, 0, 20)
                                };
                                
                                plot_ui.polygon(Polygon::new(PlotPoints::new(poly_pts)).fill_color(color));
                            }
                        }
                    });
                    
                    if plot_response.response.clicked() {
                        if let Some(pointer) = plot_response.response.hover_pos() {
                            let plot_pointer = plot_response.transform.value_from_position(pointer);
                            let x = plot_pointer.x;
                            if let Some(windows) = &mut state.windows {
                                let win_len = windows.window_len_s;
                                for i in 0..windows.window_starts_s.len() {
                                    let start = windows.window_starts_s[i];
                                    let end = start + win_len;
                                    if x >= start && x <= end {
                                        windows.valid_windows_idx[i] = !windows.valid_windows_idx[i];
                                    }
                                }
                            }
                        }
                    }
                    
                    ui.add_space(10.0);
                    
                    // Frequency Domain Plot (HVSR)
                    if let Some(res) = &state.result {
                        let mut plot_configs = vec![
                            ("Stage 1: Raw (All Windows)", &res.raw_stats),
                            ("Stage 2: STA/LTA Filtered", &res.sta_lta_stats),
                        ];
                        if let Some(f0) = &res.f0_stats {
                            plot_configs.push(("Stage 3: f0 Filtered", f0));
                        }
                        
                        let plot_count = plot_configs.len() as f32;
                        let h2 = ui.available_height() / plot_count - 10.0;
                        
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            for (i, (title, stats)) in plot_configs.iter().enumerate() {
                                ui.heading(*title);
                                ui.label(format!("Windows used: {}", stats.valid_indices.len()));
                                
                                let window_color = ui.visuals().text_color().linear_multiply(0.35);
                                let rejected_color = egui::Color32::from_rgb(200, 50, 50).linear_multiply(0.15);
                                
                                Plot::new(format!("hvsr_freq_plot_{}", i))
                                    .height(h2.max(250.0))
                                    .x_axis_formatter(|mark, _| format!("{:.2} Hz", 10_f64.powf(mark.value)))
                                    .y_axis_formatter(|mark, _| format!("{:.1}", mark.value))
                                    .allow_scroll(false) // Prevent accidental zoom on wheel scroll
                                    .allow_zoom(false)
                                    .allow_drag(true)
                                    .show(ui, |plot_ui| {
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
                                                    let h = stats.mean_hvsr[j];
                                                    mean_pts.push([x, h]);
                                                    upper_pts.push([x, stats.std_plus[j]]);
                                                    lower_pts.push([x, stats.std_minus[j]]);
                                                    
                                                    if h > max_hvsr {
                                                        max_hvsr = h;
                                                        peak_freq_log = x;
                                                        peak_freq = f;
                                                    }
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
                                                            let h = hvsr_curve[j];
                                                            win_pts.push([x, h]);
                                                        }
                                                    }
                                                    if !win_pts.is_empty() {
                                                        plot_ui.line(Line::new(PlotPoints::new(win_pts))
                                                            .color(rejected_color)
                                                            .width(1.0));
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
                                                        let h = hvsr_curve[j];
                                                        win_pts.push([x, h]);
                                                        
                                                        if h > win_max {
                                                            win_max = h;
                                                            win_max_log = x;
                                                        }
                                                    }
                                                }
                                                
                                                if !win_pts.is_empty() {
                                                    plot_ui.line(Line::new(PlotPoints::new(win_pts.clone()))
                                                        .color(window_color)
                                                        .width(1.0));
                                                        
                                                    // Circle marker for peak of each window
                                                    if win_max > 0.0 {
                                                        plot_ui.points(egui_plot::Points::new(PlotPoints::new(vec![[win_max_log, win_max]]))
                                                            .shape(egui_plot::MarkerShape::Circle)
                                                            .radius(3.0)
                                                            .color(window_color));
                                                    }
                                                }
                                            }
                                            
                                            // Build shaded area for Std Dev
                                            if !mean_pts.is_empty() {
                                                plot_ui.line(Line::new(PlotPoints::new(upper_pts))
                                                    .color(egui::Color32::from_rgb(100, 100, 255))
                                                    .style(egui_plot::LineStyle::Dashed { length: 4.0 })
                                                    .width(1.5));
                                                    
                                                plot_ui.line(Line::new(PlotPoints::new(lower_pts))
                                                    .color(egui::Color32::from_rgb(100, 100, 255))
                                                    .style(egui_plot::LineStyle::Dashed { length: 4.0 })
                                                    .width(1.5));
                                                    
                                                plot_ui.line(Line::new(PlotPoints::new(mean_pts))
                                                    .color(egui::Color32::BLUE)
                                                    .width(2.5));
                                                    
                                                // Diamond marker for peak of mean curve
                                                if max_hvsr > 0.0 {
                                                    plot_ui.points(egui_plot::Points::new(PlotPoints::new(vec![[peak_freq_log, max_hvsr]]))
                                                        .shape(egui_plot::MarkerShape::Diamond)
                                                        .radius(6.0)
                                                        .color(egui::Color32::RED));
                                                        
                                                    plot_ui.vline(VLine::new(peak_freq_log)
                                                        .color(egui::Color32::RED)
                                                        .style(egui_plot::LineStyle::Dashed { length: 5.0 }));
                                                    
                                                    plot_ui.text(Text::new(
                                                        PlotPoint::new(peak_freq_log, max_hvsr + 0.5),
                                                        format!("f0 = {:.2} Hz, A0 = {:.2}", peak_freq, max_hvsr)
                                                    ).color(egui::Color32::RED));
                                                }
                                                
                                                if stats.f0_mean > 0.0 {
                                                    let f0_log = stats.f0_mean.log10();
                                                    let f0_plus = (stats.f0_mean + stats.f0_std).log10();
                                                    let f0_minus = (stats.f0_mean - stats.f0_std).max(0.001).log10();
                                                    
                                                    plot_ui.vline(VLine::new(f0_log)
                                                        .color(egui::Color32::from_rgb(0, 180, 0))
                                                        .width(2.0));
                                                        
                                                    plot_ui.vline(VLine::new(f0_plus)
                                                        .color(egui::Color32::from_rgb(0, 180, 0))
                                                        .style(egui_plot::LineStyle::Dashed { length: 4.0 })
                                                        .width(1.0));
                                                        
                                                    plot_ui.vline(VLine::new(f0_minus)
                                                        .color(egui::Color32::from_rgb(0, 180, 0))
                                                        .style(egui_plot::LineStyle::Dashed { length: 4.0 })
                                                        .width(1.0));
                                                        
                                                    plot_ui.text(Text::new(
                                                        PlotPoint::new(f0_log, max_hvsr + 1.2),
                                                        format!("μ(f0) = {:.2} ± {:.2} Hz", stats.f0_mean, stats.f0_std)
                                                    ).color(egui::Color32::from_rgb(0, 180, 0)));
                                                }
                                            }
                                        }
                                    });
                                ui.add_space(10.0);
                            }
                        });
                    }
                });
            });
        });
        
    state.is_open = is_open;
}
