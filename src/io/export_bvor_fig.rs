use std::path::Path;
use plotters::prelude::*;
use crate::ui::bvor_vis_dialog::BVorVisState;

const WHITE: RGBColor = RGBColor(255, 255, 255);
const BLACK: RGBColor = RGBColor(0, 0, 0);
const RED: RGBColor = RGBColor(255, 0, 0);
const BLUE: RGBColor = RGBColor(0, 0, 255);
const LIGHT_GREY: RGBColor = RGBColor(200, 200, 200);

pub fn export_fig1(state: &BVorVisState, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let width = 1200;
    let height = 800;
    
    let root = BitMapBackend::new(path, (width, height)).into_drawing_area();
    root.fill(&WHITE)?;
    
    let (upper, lower) = root.split_vertically(height / 2);
    let (top_left, top_right) = upper.split_horizontally(width / 2);
    
    // --- 1. Top Left: BIC ---
    let mut bic_x_min = f64::MAX;
    let mut bic_x_max = f64::MIN;
    let mut bic_y_min = f64::MAX;
    let mut bic_y_max = f64::MIN;
    for pt in &state.bic_all_pts {
        bic_x_min = bic_x_min.min(pt[0]);
        bic_x_max = bic_x_max.max(pt[0]);
        bic_y_min = bic_y_min.min(pt[1]);
        bic_y_max = bic_y_max.max(pt[1]);
    }
    let bic_padding_x = (bic_x_max - bic_x_min).max(1.0) * 0.1;
    let bic_padding_y = (bic_y_max - bic_y_min).max(1.0) * 0.1;
    
    let mut chart_bic = ChartBuilder::on(&top_left)
        .margin_top(40)
        .margin_bottom(20)
        .margin_left(20)
        .margin_right(20)
        .x_label_area_size(50)
        .y_label_area_size(70)
        .build_cartesian_2d(
            (bic_x_min - bic_padding_x)..(bic_x_max + bic_padding_x),
            (bic_y_min - bic_padding_y)..(bic_y_max + bic_padding_y)
        )?;
        
    chart_bic.configure_mesh()
        .label_style(("sans-serif", 20).into_font())
        .x_desc("Number of Nodes")
        .y_desc("BIC")
        .axis_desc_style(("sans-serif", 25).into_font())
        .y_label_formatter(&|y| format!("{:e}", y))
        .draw()?;
        
    // All models (light grey)
    chart_bic.draw_series(
        state.bic_all_pts.iter().map(|pt| Circle::new((pt[0], pt[1]), 3, LIGHT_GREY.filled()))
    )?;
    
    // Best models (blue)
    chart_bic.draw_series(
        state.bic_best_pts.iter().map(|pt| Circle::new((pt[0], pt[1]), 4, BLUE.filled()))
    )?;
    
    // Mean pts with error bars (black)
    chart_bic.draw_series(
        state.bic_mean_pts.iter().flat_map(|pt| {
            // pt is [nodes, mean, std]
            let x = pt[0];
            let mean = pt[1];
            let std = pt[2];
            vec![
                PathElement::new(vec![(x, mean - std), (x, mean + std)], BLACK.stroke_width(2)),
            ]
        })
    )?;
    chart_bic.draw_series(
        state.bic_mean_pts.iter().map(|pt| Circle::new((pt[0], pt[1]), 5, BLACK.filled()))
    )?;
    
    // Selected model (red)
    if let Some(pt) = state.bic_sel_pt {
        chart_bic.draw_series(std::iter::once(Circle::new((pt[0], pt[1]), 6, RED.filled())))?;
    }
    
    // Manual Horizontal Legend on top_left DrawingArea
    let legend_y = 10;
    top_left.draw(&Circle::new((20, legend_y + 10), 3, LIGHT_GREY.filled()))?;
    top_left.draw(&Text::new("All Models", (30, legend_y), ("sans-serif", 16).into_font().color(&BLACK)))?;
    
    top_left.draw(&Circle::new((140, legend_y + 10), 4, BLUE.filled()))?;
    top_left.draw(&Text::new(format!("{} Best Models", state.n_med), (150, legend_y), ("sans-serif", 16).into_font().color(&BLACK)))?;
    
    top_left.draw(&PathElement::new(vec![(310, legend_y + 5), (310, legend_y + 15)], BLACK.stroke_width(2)))?;
    top_left.draw(&Circle::new((310, legend_y + 10), 5, BLACK.filled()))?;
    top_left.draw(&Text::new("Mean ± Std Dev", (325, legend_y), ("sans-serif", 16).into_font().color(&BLACK)))?;
    
    top_left.draw(&Circle::new((480, legend_y + 10), 6, RED.filled()))?;
    top_left.draw(&Text::new("Selected Model", (490, legend_y), ("sans-serif", 16).into_font().color(&BLACK)))?;
    
    // --- 2. Top Right: FMD ---
    let mut fmd_x_max = 1.0;
    if let Some(&max_u) = state.fmd_u.last() {
        fmd_x_max = max_u;
    }
    
    let mut chart_fmd = ChartBuilder::on(&top_right)
        .margin(20)
        .x_label_area_size(50)
        .y_label_area_size(70)
        .build_cartesian_2d(
            0.0..(fmd_x_max + 1.0),
            -3.0f64..0.1f64
        )?;
        
    chart_fmd.configure_mesh()
        .label_style(("sans-serif", 20).into_font())
        .x_desc("Magnitude")
        .y_desc("Probability (log10)")
        .axis_desc_style(("sans-serif", 25).into_font())
        .draw()?;
        
    // Scatter points (red cross)
    let fmd_scatter: Vec<(f64, f64)> = state.fmd_u.iter().zip(state.fmd_sums.iter())
        .map(|(&x, &y)| (x, y)).collect();
    chart_fmd.draw_series(
        fmd_scatter.iter().map(|pt| Cross::new(*pt, 5, RED.filled()))
    )?;
    
    // Line density (black)
    let fmd_line: Vec<(f64, f64)> = state.fmd_density_x.iter().zip(state.fmd_density_y.iter())
        .map(|(&x, &y)| (x, y)).collect();
    chart_fmd.draw_series(LineSeries::new(fmd_line, BLACK.stroke_width(2)))?;
    
    // Text annotations
    let text = format!("b={:.2}  μ={:.2}  σ={:.2}", state.cell_b, state.cell_mu, state.cell_sig);
    chart_fmd.draw_series(std::iter::once(
        Text::new(text, (0.5, -2.5), ("sans-serif", 20).into_font().color(&BLACK))
    ))?;
    
    // --- 3. Bottom: Voronoi Map ---
    let bnds = state.bounds; // [[xmin, xmax], [ymin, ymax]]
    let mut chart_map = ChartBuilder::on(&lower)
        .margin(20)
        .x_label_area_size(50)
        .y_label_area_size(70)
        .build_cartesian_2d(bnds[0][0]..bnds[0][1], bnds[1][0]..bnds[1][1])?;
        
    chart_map.configure_mesh()
        .label_style(("sans-serif", 20).into_font())
        .x_desc("Longitude")
        .y_desc("Latitude")
        .axis_desc_style(("sans-serif", 25).into_font())
        .draw()?;
        
    if let Some(img_vor) = &state.img_voronoi {
        let (w, h) = (img_vor.width() as u32, img_vor.height() as u32);
        let w_f = w as f64;
        let h_f = h as f64;
        for y in 0..h as usize {
            for x in 0..w as usize {
                let pixel = img_vor.pixels[y * w as usize + x].to_array();
                if pixel[3] > 0 {
                    let lon = bnds[0][0] + (x as f64 / w_f) * (bnds[0][1] - bnds[0][0]);
                    let lat = bnds[1][1] - (y as f64 / h_f) * (bnds[1][1] - bnds[1][0]);
                    let lon_next = bnds[0][0] + ((x + 1) as f64 / w_f) * (bnds[0][1] - bnds[0][0]);
                    let lat_next = bnds[1][1] - ((y + 1) as f64 / h_f) * (bnds[1][1] - bnds[1][0]);
                    let color = RGBColor(pixel[0], pixel[1], pixel[2]);
                    chart_map.draw_series(std::iter::once(Rectangle::new(
                        [(lon, lat), (lon_next, lat_next)],
                        color.filled()
                    )))?;
                }
            }
        }
    }
    
    // Nodes
    chart_map.draw_series(
        state.voronoi_pts.iter().map(|pt| Circle::new((pt[0], pt[1]), 2, BLACK.filled()))
    )?;
    
    // Coastlines
    for line in &state.coastlines {
        let pts: Vec<(f64, f64)> = line.iter().map(|pt| (pt[0], pt[1])).collect();
        chart_map.draw_series(LineSeries::new(pts, BLACK.stroke_width(1)))?;
    }
    
    // Faults
    for line in &state.faults {
        let pts: Vec<(f64, f64)> = line.iter().map(|pt| (pt[0], pt[1])).collect();
        chart_map.draw_series(LineSeries::new(pts, BLACK.stroke_width(2)))?;
    }
    
    root.present()?;
    Ok(())
}

fn get_median_b_color(norm: f64) -> RGBColor {
    let r = if norm < 0.5 { (norm * 2.0 * 255.0) as u8 } else { 255 };
    let g = if norm < 0.45 { (norm * 2.0 * 255.0) as u8 } else if norm > 0.55 { ((1.0 - norm) * 2.0 * 255.0) as u8 } else { 255 };
    let b_col = if norm > 0.5 { ((1.0 - norm) * 2.0 * 255.0) as u8 } else { 255 };
    let (r, g, b_col) = if (norm - 0.5).abs() < 0.025 { (255, 255, 255) } else { (r, g, b_col) };
    RGBColor(r, g, b_col)
}

fn get_mad_b_color(norm: f64) -> RGBColor {
    let r = (norm * 3.0 * 255.0).clamp(0.0, 255.0) as u8;
    let g = ((norm - 0.33) * 3.0 * 255.0).clamp(0.0, 255.0) as u8;
    let b_col = ((norm - 0.66) * 3.0 * 255.0).clamp(0.0, 255.0) as u8;
    RGBColor(r, g, b_col)
}

fn get_n_b_color(norm: f64) -> RGBColor {
    let norm = 1.0 - norm;
    let r = (norm * 3.0 * 255.0).clamp(0.0, 255.0) as u8;
    let g = ((norm - 0.33) * 3.0 * 255.0).clamp(0.0, 255.0) as u8;
    let b_col = ((norm - 0.66) * 3.0 * 255.0).clamp(0.0, 255.0) as u8;
    RGBColor(r, g, b_col)
}

pub fn export_fig2(state: &BVorVisState, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let width = 800;
    let height = 1200;
    
    let root = BitMapBackend::new(path, (width, height)).into_drawing_area();
    root.fill(&WHITE)?;
    
    let areas = root.split_evenly((3, 1));
    
    let bnds = state.bounds;
    
    let images = [
        (&state.img_median_b, "Median(b)", 2.0),
        (&state.img_mad_b, "MAD(b)", 1.0),
        (&state.img_n_b, "N(b)", state.n_med as f64)
    ];
    
    for (i, (img_opt, title, max_val)) in images.iter().enumerate() {
        let area = &areas[i];
        let (chart_area, cb_area) = area.split_horizontally(width - 120);
        let (_, cb_inner) = cb_area.split_horizontally(20); // 20px margin
        
        let mut chart = ChartBuilder::on(&chart_area)
            .margin(20)
            .caption(*title, ("sans-serif", 25).into_font())
            .x_label_area_size(50)
            .y_label_area_size(70)
            .build_cartesian_2d(bnds[0][0]..bnds[0][1], bnds[1][0]..bnds[1][1])?;
            
        chart.configure_mesh()
            .label_style(("sans-serif", 18).into_font())
            .x_desc("Longitude")
            .y_desc("Latitude")
            .axis_desc_style(("sans-serif", 22).into_font())
            .draw()?;
            
        if let Some(img) = img_opt {
            let (w, h) = (img.width() as u32, img.height() as u32);
            let w_f = w as f64;
            let h_f = h as f64;
            for y in 0..h as usize {
                for x in 0..w as usize {
                    let pixel = img.pixels[y * w as usize + x].to_array();
                    if pixel[3] > 0 {
                        let lon = bnds[0][0] + (x as f64 / w_f) * (bnds[0][1] - bnds[0][0]);
                        let lat = bnds[1][1] - (y as f64 / h_f) * (bnds[1][1] - bnds[1][0]);
                        let lon_next = bnds[0][0] + ((x + 1) as f64 / w_f) * (bnds[0][1] - bnds[0][0]);
                        let lat_next = bnds[1][1] - ((y + 1) as f64 / h_f) * (bnds[1][1] - bnds[1][0]);
                        let color = RGBColor(pixel[0], pixel[1], pixel[2]);
                        chart.draw_series(std::iter::once(Rectangle::new(
                            [(lon, lat), (lon_next, lat_next)],
                            color.filled()
                        )))?;
                    }
                }
            }
        }
        
        // Coastlines & Faults
        for line in &state.coastlines {
            let pts: Vec<(f64, f64)> = line.iter().map(|pt| (pt[0], pt[1])).collect();
            chart.draw_series(LineSeries::new(pts, BLACK.stroke_width(1)))?;
        }
        
        for line in &state.faults {
            let pts: Vec<(f64, f64)> = line.iter().map(|pt| (pt[0], pt[1])).collect();
            chart.draw_series(LineSeries::new(pts, BLACK.stroke_width(2)))?;
        }
        
        // --- Colorbar ---
        let mut cb_chart = ChartBuilder::on(&cb_inner)
            .margin_top(40)
            .margin_bottom(70)
            .y_label_area_size(50)
            .build_cartesian_2d(0.0..1.0, 0.0..*max_val)?;
            
        cb_chart.configure_mesh()
            .label_style(("sans-serif", 18).into_font())
            .disable_x_mesh()
            .disable_x_axis()
            .draw()?;
        
        for y in 0..100 {
            let norm = y as f64 / 100.0;
            let val = norm * max_val;
            let val_next = (y + 1) as f64 / 100.0 * max_val;
            
            let color = if *title == "Median(b)" {
                get_median_b_color(norm)
            } else if *title == "MAD(b)" {
                get_mad_b_color(norm)
            } else {
                get_n_b_color(norm)
            };
            
            cb_chart.draw_series(std::iter::once(Rectangle::new(
                [(0.0, val), (1.0, val_next)],
                color.filled()
            )))?;
        }
    }
    
    root.present()?;
    Ok(())
}
