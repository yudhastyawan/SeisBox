use eframe::egui::{Color32, ColorImage, TextureHandle, Context};
use eframe::egui;
use egui_plot::{Plot, PlotImage, PlotPoint};
use crate::core::bvor_vis_data::{load_bvor_npz, calculate_bic_grouped, BVorVisData};
use std::path::PathBuf;

pub struct BVorVisState {
    pub is_open: bool,
    pub npz_path: String,
    pub data: Option<BVorVisData>,
    pub error_msg: Option<String>,
    
    // Processed textures & raw images for export
    pub tex_median_b: Option<TextureHandle>,
    pub img_median_b: Option<egui::ColorImage>,
    pub tex_mad_b: Option<TextureHandle>,
    pub img_mad_b: Option<egui::ColorImage>,
    pub tex_n_b: Option<TextureHandle>,
    pub img_n_b: Option<egui::ColorImage>,
    
    // Bounds
    pub bounds: [[f64; 2]; 2], // [[xmin, xmax], [ymin, ymax]]
    
    // Map overlay vectors
    pub coastlines: Vec<Vec<[f64; 2]>>,
    pub faults: Vec<Vec<[f64; 2]>>,
    
    // Fig1 data
    pub fmd_u: Vec<f64>,
    pub fmd_sums: Vec<f64>,
    pub fmd_density_x: Vec<f64>,
    pub fmd_density_y: Vec<f64>,
    
    // BIC data
    pub bic_all_pts: Vec<[f64; 2]>,
    pub bic_best_pts: Vec<[f64; 2]>,
    pub bic_mean_pts: Vec<[f64; 3]>, // [x, mean, std]
    pub bic_sel_pt: Option<[f64; 2]>,
    
    // Voronoi map data
    pub tex_voronoi: Option<TextureHandle>,
    pub img_voronoi: Option<egui::ColorImage>,
    pub voronoi_pts: Vec<[f64; 2]>,
    
    // Interactive State
    pub seld: Vec<usize>,
    pub bic: Vec<f64>,
    pub selected_model_rank: usize,
    pub selected_cell_idx: Option<usize>,
    pub cell_b: f64,
    pub cell_mu: f64,
    pub cell_sig: f64,
    pub n_med: usize,
    pub bic_divisor: f64,
}

impl Default for BVorVisState {
    fn default() -> Self {
        Self {
            is_open: false,
            npz_path: "bvalue.npz".to_string(),
            data: None,
            error_msg: None,
            tex_median_b: None,
            img_median_b: None,
            tex_mad_b: None,
            img_mad_b: None,
            tex_n_b: None,
            img_n_b: None,
            bounds: [[0.0, 1.0], [0.0, 1.0]],
            coastlines: Vec::new(),
            faults: Vec::new(),
            
            fmd_u: Vec::new(),
            fmd_sums: Vec::new(),
            fmd_density_x: Vec::new(),
            fmd_density_y: Vec::new(),
            
            bic_all_pts: Vec::new(),
            bic_best_pts: Vec::new(),
            bic_mean_pts: Vec::new(),
            bic_sel_pt: None,
            
            tex_voronoi: None,
            img_voronoi: None,
            voronoi_pts: Vec::new(),
            
            seld: Vec::new(),
            bic: Vec::new(),
            selected_model_rank: 0,
            selected_cell_idx: None,
            cell_b: 0.0,
            cell_mu: 0.0,
            cell_sig: 0.0,
            n_med: 200,
            bic_divisor: 100.0,
        }
    }
}

pub fn show_bvor_vis_dialog(ctx: &Context, state: &mut crate::app::QuakePickApp) {
    if !state.bvor_vis_dialog.is_open {
        return;
    }

    let mut is_open = state.bvor_vis_dialog.is_open;

    egui::Window::new("B-Value Voronoi Visualization")
        .open(&mut is_open)
        .default_size(egui::vec2(1000.0, 800.0))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Data Source:");
                ui.text_edit_singleline(&mut state.bvor_vis_dialog.npz_path);
                if ui.button("Browse").clicked() {
                    if let Some(path) = rfd::FileDialog::new().add_filter("NPZ", &["npz"]).pick_file() {
                        state.bvor_vis_dialog.npz_path = path.display().to_string();
                    }
                }
                if ui.button("Load & Process").clicked() {
                    match load_bvor_npz(&state.bvor_vis_dialog.npz_path) {
                        Ok(data) => {
                            state.bvor_vis_dialog.bounds = [
                                [data.x_min, data.x_max],
                                [data.y_min, data.y_max],
                            ];
                            state.bvor_vis_dialog.error_msg = None;
                            process_data(ctx, &mut state.bvor_vis_dialog, data);
                        },
                        Err(e) => {
                            state.bvor_vis_dialog.error_msg = Some(e.to_string());
                        }
                    }
                }
            });
            
            if let Some(err) = &state.bvor_vis_dialog.error_msg {
                ui.colored_label(Color32::RED, format!("Error: {}", err));
            }
            
            ui.horizontal(|ui| {
                if ui.button("Load Basemap (.shp)").clicked() {
                    if let Some(path) = rfd::FileDialog::new().add_filter("SHP", &["shp"]).pick_file() {
                        if let Ok(lines) = crate::core::bvor_vis_data::parse_shapefile(&path.display().to_string()) {
                            state.bvor_vis_dialog.coastlines = lines;
                        }
                    }
                }
                if ui.button("Load Faults (.gmt)").clicked() {
                    if let Some(path) = rfd::FileDialog::new().add_filter("GMT", &["gmt"]).pick_file() {
                        if let Ok(mut lines) = crate::core::bvor_vis_data::parse_gmt_file(&path.display().to_string()) {
                            state.bvor_vis_dialog.faults.append(&mut lines);
                        }
                    }
                }
                if ui.button("Clear Overlays").clicked() {
                    state.bvor_vis_dialog.coastlines.clear();
                    state.bvor_vis_dialog.faults.clear();
                }
            });
            
            ui.separator();
            
            if state.bvor_vis_dialog.data.is_some() {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        // Helper to plot overlays
                        let plot_overlays = |plot_ui: &mut egui_plot::PlotUi, coastlines: &[Vec<[f64; 2]>], faults: &[Vec<[f64; 2]>], invert: bool, bounds: [[f64; 2]; 2]| {
                            let coast_color = if invert { Color32::WHITE } else { Color32::BLACK };
                            let fault_color = if invert { Color32::WHITE } else { Color32::BLACK };
                            
                            for line in coastlines {
                                let pts: Vec<[f64; 2]> = line.clone();
                                plot_ui.line(egui_plot::Line::new(pts).color(coast_color).width(1.0));
                            }
                            for line in faults {
                                let pts: Vec<[f64; 2]> = line.clone();
                                plot_ui.line(egui_plot::Line::new(pts).color(fault_color).width(1.5));
                            }
                            
                            plot_ui.set_plot_bounds(egui_plot::PlotBounds::from_min_max(
                                [bounds[0][0], bounds[1][0]],
                                [bounds[0][1], bounds[1][1]]
                            ));
                        };
                        
                        // Plot Median
                        if let Some(tex) = &state.bvor_vis_dialog.tex_median_b {
                            ui.vertical(|ui| {
                                ui.label("Median(b)");
                                let plot = Plot::new("plot_median_b").view_aspect(1.0).width(300.0).height(300.0);
                                plot.show(ui, |plot_ui| {
                                    let bounds = state.bvor_vis_dialog.bounds;
                                    let pos = PlotPoint::new((bounds[0][0] + bounds[0][1])/2.0, (bounds[1][0] + bounds[1][1])/2.0);
                                    let size = egui::vec2((bounds[0][1] - bounds[0][0]) as f32, (bounds[1][1] - bounds[1][0]) as f32);
                                    plot_ui.image(PlotImage::new(tex, pos, size));
                                    
                                    plot_overlays(plot_ui, &state.bvor_vis_dialog.coastlines, &state.bvor_vis_dialog.faults, false, bounds);
                                });
                            });
                        }
                        
                        // Plot MAD
                        if let Some(tex) = state.bvor_vis_dialog.tex_mad_b.clone() {
                            ui.vertical(|ui| {
                                ui.label("MAD(b)");
                                let plot = Plot::new("plot_mad_b").view_aspect(1.0).width(300.0).height(300.0);
                                plot.show(ui, |plot_ui| {
                                    let bounds = state.bvor_vis_dialog.bounds;
                                    let pos = PlotPoint::new((bounds[0][0] + bounds[0][1])/2.0, (bounds[1][0] + bounds[1][1])/2.0);
                                    let size = egui::vec2((bounds[0][1] - bounds[0][0]) as f32, (bounds[1][1] - bounds[1][0]) as f32);
                                    plot_ui.image(PlotImage::new(&tex, pos, size));
                                    
                                    plot_overlays(plot_ui, &state.bvor_vis_dialog.coastlines, &state.bvor_vis_dialog.faults, true, bounds);
                                });
                            });
                        }
                        // Plot N
                        if let Some(tex) = state.bvor_vis_dialog.tex_n_b.clone() {
                            ui.vertical(|ui| {
                                ui.label("N(b)");
                                let plot = Plot::new("plot_n_b").view_aspect(1.0).width(300.0).height(300.0);
                                plot.show(ui, |plot_ui| {
                                    let bounds = state.bvor_vis_dialog.bounds;
                                    let pos = PlotPoint::new((bounds[0][0] + bounds[0][1])/2.0, (bounds[1][0] + bounds[1][1])/2.0);
                                    let size = egui::vec2((bounds[0][1] - bounds[0][0]) as f32, (bounds[1][1] - bounds[1][0]) as f32);
                                    plot_ui.image(PlotImage::new(&tex, pos, size));
                                    
                                    plot_overlays(plot_ui, &state.bvor_vis_dialog.coastlines, &state.bvor_vis_dialog.faults, true, bounds);
                                });
                            });
                        }
                        
                        // Plot Voronoi (Fig1 - axm)
                        if let Some(tex) = state.bvor_vis_dialog.tex_voronoi.clone() {
                            ui.vertical(|ui| {
                                ui.label("Voronoi Cells (Selected Model)");
                                let plot = Plot::new("plot_voronoi").view_aspect(1.0).width(300.0).height(300.0);
                                let mut clicked_pos = None;
                                let _plot_response = plot.show(ui, |plot_ui| {
                                    let bounds = state.bvor_vis_dialog.bounds;
                                    let pos = PlotPoint::new((bounds[0][0] + bounds[0][1])/2.0, (bounds[1][0] + bounds[1][1])/2.0);
                                    let size = egui::vec2((bounds[0][1] - bounds[0][0]) as f32, (bounds[1][1] - bounds[1][0]) as f32);
                                    plot_ui.image(PlotImage::new(&tex, pos, size));
                                    
                                    // Plot nodes
                                    plot_ui.points(egui_plot::Points::new(state.bvor_vis_dialog.voronoi_pts.clone())
                                        .color(Color32::BLACK).radius(2.0));
                                        
                                    plot_overlays(plot_ui, &state.bvor_vis_dialog.coastlines, &state.bvor_vis_dialog.faults, true, bounds);
                                    
                                    if plot_ui.response().clicked() {
                                        if let Some(pos) = plot_ui.pointer_coordinate() {
                                            clicked_pos = Some([pos.x, pos.y]);
                                        }
                                    }
                                });
                                
                                if let Some(cpos) = clicked_pos {
                                    // Find nearest cell
                                    let mut min_dist = f64::MAX;
                                    let mut nearest = None;
                                    for (i, pt) in state.bvor_vis_dialog.voronoi_pts.iter().enumerate() {
                                        let dx = pt[0] - cpos[0];
                                        let dy = pt[1] - cpos[1];
                                        let d2 = dx*dx + dy*dy;
                                        if d2 < min_dist {
                                            min_dist = d2;
                                            nearest = Some(i);
                                        }
                                    }
                                    if nearest != state.bvor_vis_dialog.selected_cell_idx {
                                        state.bvor_vis_dialog.selected_cell_idx = nearest;
                                        update_fig1_data(ctx, &mut state.bvor_vis_dialog);
                                    }
                                }
                            });
                        }
                    });
                    
                    ui.separator();
                    ui.horizontal(|ui| {
                        let total_models = state.bvor_vis_dialog.data.as_ref().map_or(1000, |d| d.b_grid_all.len() * d.n_nodes.len());
                        let mut nmed = state.bvor_vis_dialog.n_med;
                        if ui.add(egui::Slider::new(&mut nmed, 1..=total_models).text("N Best Models (Nmed)")).changed() {
                            state.bvor_vis_dialog.n_med = nmed;
                            let cloned_data = state.bvor_vis_dialog.data.as_ref().unwrap().clone();
                            process_data(ctx, &mut state.bvor_vis_dialog, cloned_data);
                            state.bvor_vis_dialog.selected_model_rank = 0;
                            state.bvor_vis_dialog.selected_cell_idx = Some(0);
                            update_fig1_data(ctx, &mut state.bvor_vis_dialog);
                        }
                        
                        let mut bic_div = state.bvor_vis_dialog.bic_divisor;
                        ui.label("BIC Divisor:");
                        if ui.add(egui::DragValue::new(&mut bic_div).speed(1.0)).changed() {
                            state.bvor_vis_dialog.bic_divisor = bic_div.max(1e-6); // Prevent zero division
                            let cloned_data = state.bvor_vis_dialog.data.as_ref().unwrap().clone();
                            process_data(ctx, &mut state.bvor_vis_dialog, cloned_data);
                            state.bvor_vis_dialog.selected_model_rank = 0;
                            state.bvor_vis_dialog.selected_cell_idx = Some(0);
                            update_fig1_data(ctx, &mut state.bvor_vis_dialog);
                        }
                        
                        ui.label(format!("Select Model Rank (1-{}):", state.bvor_vis_dialog.seld.len()));
                        let mut rank = state.bvor_vis_dialog.selected_model_rank;
                        if ui.add(egui::Slider::new(&mut rank, 0..=state.bvor_vis_dialog.seld.len().saturating_sub(1)).text("Rank")).changed() {
                            state.bvor_vis_dialog.selected_model_rank = rank;
                            state.bvor_vis_dialog.selected_cell_idx = Some(0);
                            update_fig1_data(ctx, &mut state.bvor_vis_dialog);
                        }
                        
                        let num_cells = if let Some(data) = &state.bvor_vis_dialog.data {
                            if !state.bvor_vis_dialog.seld.is_empty() {
                                let sel_idx = state.bvor_vis_dialog.seld[state.bvor_vis_dialog.selected_model_rank];
                                let node_idx = sel_idx % data.n_nodes.len();
                                data.n_nodes[node_idx]
                            } else { 0 }
                        } else { 0 };
                        
                        if num_cells > 0 {
                            let mut cell_idx = state.bvor_vis_dialog.selected_cell_idx.unwrap_or(0);
                            if ui.add(egui::Slider::new(&mut cell_idx, 0..=num_cells.saturating_sub(1)).text("Cell Index")).changed() {
                                state.bvor_vis_dialog.selected_cell_idx = Some(cell_idx);
                                update_fig1_data(ctx, &mut state.bvor_vis_dialog);
                            }
                        }
                        
                        if ui.button("Reset Cell Selection").clicked() {
                            state.bvor_vis_dialog.selected_cell_idx = None;
                            update_fig1_data(ctx, &mut state.bvor_vis_dialog);
                        }
                    });
                    
                    ui.add_space(5.0);
                    
                    ui.horizontal(|ui| {
                        if ui.button("Save Map & FMD (Fig1)").clicked() {
                            if let Some(path) = rfd::FileDialog::new().set_file_name("the_best_selection.png").save_file() {
                                if let Err(e) = crate::io::export_bvor_fig::export_fig1(&state.bvor_vis_dialog, &path) {
                                    state.bvor_vis_dialog.error_msg = Some(format!("Fig1 Export Failed: {}", e));
                                } else {
                                    state.bvor_vis_dialog.error_msg = Some("Fig1 Exported Successfully!".to_string());
                                }
                            }
                        }
                        
                        if ui.button("Save Heatmaps (Fig2)").clicked() {
                            if let Some(path) = rfd::FileDialog::new().set_file_name("b_map.png").save_file() {
                                if let Err(e) = crate::io::export_bvor_fig::export_fig2(&state.bvor_vis_dialog, &path) {
                                    state.bvor_vis_dialog.error_msg = Some(format!("Fig2 Export Failed: {}", e));
                                } else {
                                    state.bvor_vis_dialog.error_msg = Some("Fig2 Exported Successfully!".to_string());
                                }
                            }
                        }
                    });
                    ui.separator();
                    
                    ui.horizontal(|ui| {
                        // Plot FMD (Fig1)
                        ui.vertical(|ui| {
                            ui.vertical(|ui| {
                                ui.label("Magnitude Histogram (FMD)");
                                let plot = Plot::new("plot_fmd")
                                    .view_aspect(1.5).width(450.0).height(300.0)
                                    .include_y(0.0)
                                    .include_y(-3.0)
                                    .y_axis_formatter(|mark, _max_chars| {
                                        let val = mark.value;
                                        format!("10^{:.1}", val)
                                    });
                                let is_dark_mode = ui.visuals().dark_mode;
                                plot.show(ui, |plot_ui| {
                                    let mut pts = Vec::new();
                                    for (i, &x) in state.bvor_vis_dialog.fmd_u.iter().enumerate() {
                                        pts.push([x, state.bvor_vis_dialog.fmd_sums[i]]);
                                    }
                                    plot_ui.points(egui_plot::Points::new(pts).color(Color32::RED).radius(3.0).shape(egui_plot::MarkerShape::Cross));
                                    
                                    let mut line_pts = Vec::new();
                                    for (i, &x) in state.bvor_vis_dialog.fmd_density_x.iter().enumerate() {
                                        line_pts.push([x, state.bvor_vis_dialog.fmd_density_y[i]]);
                                    }
                                    let line_color = if is_dark_mode { Color32::WHITE } else { Color32::BLACK };
                                    plot_ui.line(egui_plot::Line::new(line_pts).color(line_color).width(1.5));
                                    
                                    // Overlay text for b, mu, sigma
                                    let text = format!(
                                        "b={:.2}\nμ={:.2}\nσ={:.2}",
                                        state.bvor_vis_dialog.cell_b,
                                        state.bvor_vis_dialog.cell_mu,
                                        state.bvor_vis_dialog.cell_sig
                                    );
                                    let text_pos = PlotPoint::new(
                                        state.bvor_vis_dialog.fmd_density_x.first().copied().unwrap_or(0.0) + 0.5,
                                        state.bvor_vis_dialog.fmd_sums.first().copied().unwrap_or(0.0) - 0.5
                                    );
                                    plot_ui.text(egui_plot::Text::new(text_pos, text)
                                        .color(line_color)
                                        .anchor(egui::Align2::LEFT_TOP));
                                    
                                });
                            });  });
                        
                        // Plot BIC (Fig1)
                        ui.vertical(|ui| {
                            ui.label("BIC vs Nodes");
                            let plot = Plot::new("plot_bic").view_aspect(1.5).width(450.0).height(300.0);
                            plot.show(ui, |plot_ui| {
                                plot_ui.points(egui_plot::Points::new(state.bvor_vis_dialog.bic_all_pts.clone())
                                    .color(Color32::LIGHT_GRAY).radius(2.0).name("All Models"));
                                plot_ui.points(egui_plot::Points::new(state.bvor_vis_dialog.bic_best_pts.clone())
                                    .color(Color32::BLUE).radius(3.0).name("Best Models"));
                                
                                let color = if ctx.style().visuals.dark_mode { Color32::WHITE } else { Color32::BLACK };
                                
                                for pt in &state.bvor_vis_dialog.bic_mean_pts {
                                    plot_ui.line(egui_plot::Line::new(vec![[pt[0], pt[1] - pt[2]], [pt[0], pt[1] + pt[2]]]).color(color).width(1.5));
                                }
                                let mean_pts: Vec<[f64; 2]> = state.bvor_vis_dialog.bic_mean_pts.iter().map(|pt| [pt[0], pt[1]]).collect();
                                plot_ui.points(egui_plot::Points::new(mean_pts)
                                    .color(color).radius(4.0).name("Mean ± Std Dev"));
                                
                                if let Some(sel) = state.bvor_vis_dialog.bic_sel_pt {
                                    plot_ui.points(egui_plot::Points::new(vec![sel])
                                        .color(Color32::RED).radius(5.0).name("Selected Model"));
                                }
                            });
                        });
                    });
                });
            } else {
                ui.label("No data loaded. Click 'Load & Process' to visualize.");
            }
        });

state.bvor_vis_dialog.is_open = is_open;
}

fn process_data(ctx: &Context, state: &mut BVorVisState, data: BVorVisData) {
    // 1. Calculate BIC Probabilities and find Best Models
    let n_med = state.n_med;
    let bic_div = state.bic_divisor;
    let (_bic, seld, _lnLs) = crate::core::bvor_vis_data::calculate_bic_grouped(&data, 60, n_med, 1e-3, bic_div);
    state.bic = _bic.clone();
    state.seld = seld.clone();
    
    // If no models selected, just abort for now
    if seld.is_empty() {
        state.error_msg = Some("No models matched the criteria".to_string());
        return;
    }
    
    // 2. Compute median and MAD of b_grid across selected models
    let num_n_nodes = data.n_nodes.len();
    let rep = data.b_grid_all.len();
    let res = data.grid_res; // res x res image
    let grid_size = res * res;
    
    let mut median_b = vec![f64::NAN; grid_size];
    let mut mad_b = vec![f64::NAN; grid_size];
    let mut n_b = vec![0; grid_size];
    
    for i in 0..grid_size {
        let mut vals = Vec::with_capacity(seld.len());
        for &idx in &seld {
            let rep_idx = idx / num_n_nodes;
            let node_idx = idx % num_n_nodes;
            
            if rep_idx < rep {
                let offset = node_idx * grid_size;
                if offset + i < data.b_grid_all[rep_idx].len() {
                    let v = data.b_grid_all[rep_idx][offset + i];
                    if !v.is_nan() {
                        vals.push(v);
                    }
                }
            }
        }
        
        if vals.is_empty() {
            continue;
        }
        
        n_b[i] = vals.len();
        vals.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mid = vals.len() / 2;
        let med = if vals.len() % 2 == 0 {
            (vals[mid - 1] + vals[mid]) / 2.0
        } else {
            vals[mid]
        };
        
        median_b[i] = med;
        
        // MAD (Mean Absolute Deviation or Std, python code used sqrt of mean squared diff)
        let mut sum_sq = 0.0;
        for &v in &vals {
            sum_sq += (v - med).powi(2);
        }
        mad_b[i] = (sum_sq / vals.len() as f64).sqrt();
    }
    
    // 3. Generate ColorImages
    // Res is width and height
    let w = res;
    let h = res;
    
    let mut img_median = ColorImage::new([w, h], Color32::TRANSPARENT);
    let mut img_mad = ColorImage::new([w, h], Color32::TRANSPARENT);
    let mut img_n = ColorImage::new([w, h], Color32::TRANSPARENT);
    
    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            if idx >= grid_size { break; }
            
            // Map b_median to Seismic with White center
            // range: 0.0 to 2.0. Center = 1.0 (White)
            let b = median_b[idx];
            if !b.is_nan() {
                let norm = ((b - 0.0) / 2.0).clamp(0.0, 1.0);
                // Simple seismic-like
                let r = if norm < 0.5 { (norm * 2.0 * 255.0) as u8 } else { 255 };
                let g = if norm < 0.45 { (norm * 2.0 * 255.0) as u8 } else if norm > 0.55 { ((1.0 - norm) * 2.0 * 255.0) as u8 } else { 255 };
                let b_col = if norm > 0.5 { ((1.0 - norm) * 2.0 * 255.0) as u8 } else { 255 };
                
                // Python's seismic_with_white used 0.95 to 1.05
                // let's just make close to 1.0 white
                let (r, g, b_col) = if (b - 1.0).abs() < 0.05 {
                    (255, 255, 255)
                } else {
                    (r, g, b_col)
                };
                
                // Note: Image needs to be flipped vertically because egui PlotImage has origin at bottom-left? 
                // Actually PlotImage maps [0..w] to [xmin..xmax] and [0..h] to [ymax..ymin] if we don't flip.
                // We'll flip Y for egui image so it renders bottom-up
                let flip_y = h - 1 - y;
                img_median.pixels[flip_y * w + x] = Color32::from_rgb(r, g, b_col);
            }
            
            // Map MAD to Hot
            let m = mad_b[idx];
            if !m.is_nan() {
                let norm = ((m - 0.0) / 1.0).clamp(0.0, 1.0);
                // simple hot
                let r = (norm * 3.0 * 255.0).clamp(0.0, 255.0) as u8;
                let g = ((norm - 0.33) * 3.0 * 255.0).clamp(0.0, 255.0) as u8;
                let b_col = ((norm - 0.66) * 3.0 * 255.0).clamp(0.0, 255.0) as u8;
                let flip_y = h - 1 - y;
                img_mad.pixels[flip_y * w + x] = Color32::from_rgb(r, g, b_col);
            }
            
            // Map N(b) to Hot_r
            let n = n_b[idx] as f64;
            if n > 0.0 {
                let norm = 1.0 - ((n - 0.0) / state.n_med as f64).clamp(0.0, 1.0); // reversed hot
                let r = (norm * 3.0 * 255.0).clamp(0.0, 255.0) as u8;
                let g = ((norm - 0.33) * 3.0 * 255.0).clamp(0.0, 255.0) as u8;
                let b_col = ((norm - 0.66) * 3.0 * 255.0).clamp(0.0, 255.0) as u8;
                let flip_y = h - 1 - y;
                img_n.pixels[flip_y * w + x] = Color32::from_rgb(r, g, b_col);
            }
        }
    }
    
    state.img_median_b = Some(img_median.clone());
    state.tex_median_b = Some(ctx.load_texture("tex_median_b", img_median, Default::default()));
    state.img_mad_b = Some(img_mad.clone());
    state.tex_mad_b = Some(ctx.load_texture("tex_mad_b", img_mad, Default::default()));
    state.img_n_b = Some(img_n.clone());
    state.tex_n_b = Some(ctx.load_texture("tex_n_b", img_n, Default::default()));
    
    state.data = Some(data);
    
    update_fig1_data(ctx, state);
}

fn update_fig1_data(ctx: &egui::Context, state: &mut BVorVisState) {
    let data = match &state.data {
        Some(d) => d,
        None => return,
    };
    
    let num_n_nodes = data.n_nodes.len();
    let rep = data.b_grid_all.len();
    let res = data.grid_res;
    
    // Safety check for selected rank
    if state.selected_model_rank >= state.seld.len() {
        state.selected_model_rank = 0;
    }
    
    if state.seld.is_empty() {
        return;
    }
    
    let sel_idx = state.seld[state.selected_model_rank];
    let rep_idx = sel_idx / num_n_nodes;
    let node_idx = sel_idx % num_n_nodes;
    let j = data.n_nodes[node_idx];
    
    let offset = node_idx * data.n_nodes[data.n_nodes.len() - 1] * 2;
    let val_offset = node_idx * data.n_nodes[data.n_nodes.len() - 1];
    
    let mut kdtree: kiddo::KdTree<f64, 2> = kiddo::KdTree::new();
    let mut pts = Vec::new();
    
    if rep_idx < rep && offset + j * 2 <= data.pnt_vor_all[rep_idx].len() {
        for v_idx in 0..j {
            let px = data.pnt_vor_all[rep_idx][offset + v_idx * 2];
            let py = data.pnt_vor_all[rep_idx][offset + v_idx * 2 + 1];
            if !px.is_nan() && !py.is_nan() {
                pts.push([px, py]);
                kdtree.add(&[px, py], v_idx as u64);
            }
        }
    }
    
    if !pts.is_empty() {
        state.voronoi_pts = pts;
        
        let mut img_vor = ColorImage::new([res, res], Color32::TRANSPARENT);
        let dx = (data.x_max - data.x_min) / (res as f64 - 1.0);
        let dy = (data.y_max - data.y_min) / (res as f64 - 1.0);
        
        let mut colors = Vec::with_capacity(j);
        for i in 0..j {
            let v = data.b_vor_all[rep_idx][val_offset + i];
            let norm = (v / 2.0).clamp(0.0, 1.0);
            let r = if norm < 0.5 { (norm * 2.0 * 255.0) as u8 } else { 255 };
            let g = if norm < 0.45 { (norm * 2.0 * 255.0) as u8 } else if norm > 0.55 { ((1.0 - norm) * 2.0 * 255.0) as u8 } else { 255 };
            let b_col = if norm > 0.5 { ((1.0 - norm) * 2.0 * 255.0) as u8 } else { 255 };
            let (r, g, b_col) = if (norm - 0.5).abs() < 0.025 { (255, 255, 255) } else { (r, g, b_col) };
            
            // Highlight selected cell by making unselected ones slightly transparent
            let mut alpha = 180;
            if let Some(selected) = state.selected_cell_idx {
                if i == selected { alpha = 255; }
            }
            colors.push(Color32::from_rgba_premultiplied(r, g, b_col, alpha));
        }
        
        let mut cell_indices = vec![0; res * res];
        for y in 0..res {
            for x in 0..res {
                let px = data.x_min + (x as f64) * dx;
                let py = data.y_min + (y as f64) * dy;
                let nearest = kdtree.nearest_one::<kiddo::SquaredEuclidean>(&[px, py]);
                cell_indices[y * res + x] = nearest.item as usize;
            }
        }
        
        for y in 0..res {
            for x in 0..res {
                let cell_idx = cell_indices[y * res + x];
                let mut is_edge = false;
                let mut is_selected_edge = false;
                
                let mut check_neighbor = |nx: usize, ny: usize| {
                    let neighbor_cell = cell_indices[ny * res + nx];
                    if neighbor_cell != cell_idx {
                        is_edge = true;
                        if let Some(sel) = state.selected_cell_idx {
                            if cell_idx == sel || neighbor_cell == sel {
                                is_selected_edge = true;
                            }
                        }
                    }
                };
                
                if x > 0 { check_neighbor(x - 1, y); }
                if x < res - 1 { check_neighbor(x + 1, y); }
                if y > 0 { check_neighbor(x, y - 1); }
                if y < res - 1 { check_neighbor(x, y + 1); }
                
                if is_selected_edge {
                    img_vor.pixels[(res - 1 - y) * res + x] = Color32::from_rgb(0, 255, 255); // Cyan
                } else if is_edge {
                    img_vor.pixels[(res - 1 - y) * res + x] = Color32::BLACK;
                } else {
                    img_vor.pixels[(res - 1 - y) * res + x] = colors[cell_idx % colors.len()];
                }
            }
        }
        
        state.img_voronoi = Some(img_vor.clone());
        state.tex_voronoi = Some(ctx.load_texture("tex_voronoi", img_vor, Default::default()));
    }
    
    // --- Compute FMD for Selected Cell or Average ---
    let mut selected_magnitudes = Vec::new();
    if let Some(selected) = state.selected_cell_idx {
        if data.x_all.len() == data.m_all.len() && data.y_all.len() == data.m_all.len() {
            for i in 0..data.m_all.len() {
                let px = data.x_all[i];
                let py = data.y_all[i];
                if !px.is_nan() && !py.is_nan() {
                    let nearest = kdtree.nearest_one::<kiddo::SquaredEuclidean>(&[px, py]);
                    if nearest.item as usize == selected {
                        selected_magnitudes.push(data.m_all[i]);
                    }
                }
            }
        } else {
            // Legacy file, just use all magnitudes
            selected_magnitudes = data.m_all.clone();
        }
    } else {
        selected_magnitudes = data.m_all.clone();
    }
    
    let (u, mut sums) = crate::core::bvor_vis_data::imfd(&selected_magnitudes);
    for v in sums.iter_mut() {
        if *v <= 1e-10 {
            *v = -10.0;
        } else {
            *v = v.log10();
        }
    }
    state.fmd_u = u.clone();
    state.fmd_sums = sums.clone();
    
    let max_u = u.last().copied().unwrap_or(10.0);
    let mut x2 = Vec::new();
    let mut curr = 0.0;
    while curr <= max_u {
        x2.push(curr);
        curr += 0.1;
    }
    state.fmd_density_x = x2.clone();
    
    if let Some(selected) = state.selected_cell_idx {
        if rep_idx < rep && val_offset + selected < data.b_vor_all[rep_idx].len() {
            state.cell_b = data.b_vor_all[rep_idx][val_offset + selected];
            state.cell_mu = data.mu_vor_all[rep_idx][val_offset + selected];
            state.cell_sig = data.sig_vor_all[rep_idx][val_offset + selected];
        }
    } else {
        // Fallback to average over model
        let mut avg_b = 0.0;
        let mut avg_mu = 0.0;
        let mut avg_sig = 0.0;
        let mut valid_models = 0;
        for v_idx in 0..j {
            if val_offset + v_idx < data.b_vor_all[rep_idx].len() {
                let b = data.b_vor_all[rep_idx][val_offset + v_idx];
                let mu = data.mu_vor_all[rep_idx][val_offset + v_idx];
                let sig = data.sig_vor_all[rep_idx][val_offset + v_idx];
                if !b.is_nan() && !mu.is_nan() && !sig.is_nan() {
                    avg_b += b;
                    avg_mu += mu;
                    avg_sig += sig;
                    valid_models += 1;
                }
            }
        }
        if valid_models > 0 {
            state.cell_b = avg_b / valid_models as f64;
            state.cell_mu = avg_mu / valid_models as f64;
            state.cell_sig = avg_sig / valid_models as f64;
        }
    }
    
    let beta = state.cell_b * 10f64.ln();
    let mut density_y = crate::core::bvor_vis_data::density(&x2, [beta, state.cell_mu, state.cell_sig]);
    for v in density_y.iter_mut() {
        if *v <= 1e-10 {
            *v = -10.0;
        } else {
            *v = v.log10();
        }
    }
    state.fmd_density_y = density_y;
    
    // Calculate BIC scatter
    state.bic_all_pts.clear();
    state.bic_best_pts.clear();
    state.bic_mean_pts.clear();
    
    let mut bic_vals = vec![Vec::new(); num_n_nodes];
    
    for i in 0..rep {
        for (k, &nodes) in data.n_nodes.iter().enumerate() {
            let idx = i * num_n_nodes + k;
            let b = state.bic[idx];
            if !b.is_nan() {
                state.bic_all_pts.push([nodes as f64, b]);
                if state.seld.contains(&idx) {
                    state.bic_best_pts.push([nodes as f64, b]);
                }
                
                bic_vals[k].push(b);
            }
        }
    }
    
    for (k, &nodes) in data.n_nodes.iter().enumerate() {
        if !bic_vals[k].is_empty() {
            let n = bic_vals[k].len() as f64;
            let mean = bic_vals[k].iter().sum::<f64>() / n;
            let var = bic_vals[k].iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / n;
            state.bic_mean_pts.push([nodes as f64, mean, var.sqrt()]);
        }
    }
    
    state.bic_sel_pt = Some([j as f64, state.bic[sel_idx]]);
}
