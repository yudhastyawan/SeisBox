use plotters::prelude::*;
use std::collections::{HashSet, BTreeSet};
use std::error::Error;
use std::fs::File;
use std::path::Path;
use contour::ContourBuilder;
use geo::MultiPolygon;

/// Simple CoolWarm colormap implementation.
/// Maps a value between vmin and vmax to an RGB color.
pub fn cool_warm(val: f64, vmin: f64, vmax: f64) -> RGBColor {
    // Normalize to 0.0 .. 1.0
    let mut t = (val - vmin) / (vmax - vmin);
    t = t.clamp(0.0, 1.0);

    // CoolWarm approximation
    // 0.0 -> Blue, 0.5 -> White, 1.0 -> Red
    let (r, g, b) = if t < 0.5 {
        let frac = t * 2.0; // 0.0 to 1.0
        let r = (59.0 + frac * (255.0 - 59.0)) as u8;
        let g = (76.0 + frac * (255.0 - 76.0)) as u8;
        let b = (192.0 + frac * (255.0 - 192.0)) as u8;
        (r, g, b)
    } else {
        let frac = (t - 0.5) * 2.0; // 0.0 to 1.0
        let r = (255.0 - frac * (255.0 - 180.0)) as u8;
        let g = (255.0 - frac * (255.0 - 4.0)) as u8;
        let b = (255.0 - frac * (255.0 - 38.0)) as u8;
        (r, g, b)
    };
    RGBColor(r, g, b)
}

/// Helper to build a 2D matrix (as flat Vec) from unstructured scatter points.
/// Returns (flat_matrix, unique_x, unique_y)
pub fn build_grid_matrix(points: &[CfsPoint]) -> Result<(Vec<f64>, Vec<f64>, Vec<f64>), String> {
    let mut xs: Vec<f64> = points.iter().map(|p| p.x).collect();
    let mut ys: Vec<f64> = points.iter().map(|p| p.y).collect();
    
    // Sort and dedup with epsilon
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
    
    xs.dedup_by(|a, b| (*a - *b).abs() < 1e-4);
    ys.dedup_by(|a, b| (*a - *b).abs() < 1e-4);

    let nx = xs.len();
    let ny = ys.len();
    
    if nx < 2 || ny < 2 {
        return Err("Not enough unique grid points for contouring".into());
    }

    let mut matrix = vec![0.0; nx * ny];
    let mut count = vec![0; nx * ny];

    for p in points {
        // Find closest index
        let xi = xs.binary_search_by(|v| v.partial_cmp(&p.x).unwrap()).unwrap_or_else(|e| e);
        let yi = ys.binary_search_by(|v| v.partial_cmp(&p.y).unwrap()).unwrap_or_else(|e| e);
        
        let xi = xi.min(nx - 1);
        let yi = yi.min(ny - 1);
        
        let idx = yi * nx + xi;
        matrix[idx] += p.coulomb;
        count[idx] += 1;
    }

    for i in 0..matrix.len() {
        if count[i] > 0 {
            matrix[i] /= count[i] as f64;
        }
    }

    Ok((matrix, xs, ys))
}

pub struct CfsPoint {
    pub x: f64,
    pub y: f64,
    pub coulomb: f64,
}

#[derive(Debug, PartialEq)]
pub enum PlotType {
    Grid,
    CrossSection,
    Batch,
}

#[derive(Debug, Clone)]
pub struct PlotConfig {
    pub csv_path: String,
    pub out_path: String,
    pub aspect_equal: bool,
    pub use_contourf: bool,
    pub vmin: Option<f64>,
    pub vmax: Option<f64>,
    pub title: Option<String>,
    pub width: u32,
    pub height: u32,
    pub title_size: u32,
    pub label_size: u32,
    pub tick_size: u32,
    pub x_labels: usize,
    pub y_labels: usize,
    pub cbar_label: Option<String>,
    pub cbar_label_size: Option<u32>,
    pub cbar_tick_size: Option<u32>,
    pub cbar_y_labels: Option<usize>,
    pub cbar_extend: Option<String>,
    pub contour_steps: u32,
    pub upsample_res: u32,
    pub fault_color: Option<String>,
    pub fault_width: Option<u32>,
    pub fault_style: Option<String>,
    pub plot_cs_track: bool,
    pub cs_track_color: Option<String>,
    pub cs_track_width: Option<u32>,
    pub cs_track_style: Option<String>,
    pub plot_inp: Option<std::path::PathBuf>,
    pub cs_start_lon: Option<Vec<f64>>,
    pub cs_start_lat: Option<Vec<f64>>,
    pub cs_finish_lon: Option<Vec<f64>>,
    pub cs_finish_lat: Option<Vec<f64>>,
}

fn parse_rgb_color(color: &str) -> RGBColor {
    match color.to_lowercase().as_str() {
        "white" => RGBColor(255, 255, 255),
        "red" => RGBColor(255, 0, 0),
        "green" => RGBColor(0, 255, 0),
        "blue" => RGBColor(0, 0, 255),
        "yellow" => RGBColor(255, 255, 0),
        "magenta" => RGBColor(255, 0, 255),
        "cyan" => RGBColor(0, 255, 255),
        "gray" | "grey" => RGBColor(128, 128, 128),
        "black" | _ => RGBColor(0, 0, 0),
    }
}

fn segments_intersect(p1: (f64, f64), p2: (f64, f64), p3: (f64, f64), p4: (f64, f64)) -> bool {
    let ccw = |a: (f64, f64), b: (f64, f64), c: (f64, f64)| {
        (c.1 - a.1) * (b.0 - a.0) > (b.1 - a.1) * (c.0 - a.0)
    };
    ccw(p1, p3, p4) != ccw(p2, p3, p4) && ccw(p1, p2, p3) != ccw(p1, p2, p4)
}

pub fn plot_cfs_csv(config: &PlotConfig) -> Result<(), Box<dyn Error>> {
    let path = Path::new(&config.csv_path);
    if !path.exists() {
        return Err(format!("File {} not found", config.csv_path).into());
    }

    let mut rdr = csv::Reader::from_reader(File::open(path)?);
    let headers = rdr.headers()?.clone();

    // Find column indices
    let mut lon_idx = None;
    let mut lat_idx = None;
    let mut dist_idx = None;
    let mut depth_idx = None;
    let mut cfs_idx = None;

    for (i, h) in headers.iter().enumerate() {
        match h {
            "Lon" => lon_idx = Some(i),
            "Lat" => lat_idx = Some(i),
            "Distance_km" => dist_idx = Some(i),
            "Depth_km" => depth_idx = Some(i),
            "Coulomb_bar" => cfs_idx = Some(i),
            _ => {}
        }
    }

    let cfs_idx = cfs_idx.ok_or("Coulomb_bar column not found")?;

    let is_cross_section = dist_idx.is_some() && depth_idx.is_some();

    let mut points = Vec::new();
    let mut x_set = HashSet::new();
    let mut y_set = HashSet::new();

    for result in rdr.records() {
        let record = result?;
        let coulomb: f64 = record[cfs_idx].parse()?;

        let (x, y) = if is_cross_section {
            let d: f64 = record[dist_idx.unwrap()].parse()?;
            let z: f64 = record[depth_idx.unwrap()].parse()?;
            (d, z)
        } else {
            let l: f64 = record[lon_idx.unwrap()].parse()?;
            let a: f64 = record[lat_idx.unwrap()].parse()?;
            (l, a)
        };

        points.push(CfsPoint { x, y, coulomb });
        
        // Track unique values to guess if it's a grid (using string formatting to handle float precision)
        x_set.insert(format!("{:.4}", x));
        y_set.insert(format!("{:.4}", y));
    }

    if points.is_empty() {
        return Err("CSV has no data rows".into());
    }

    // Determine Plot Type
    let plot_type = if is_cross_section {
        PlotType::CrossSection
    } else {
        // Simple heuristic: If the number of points is close to x_unique * y_unique, it's a grid
        let expected_grid_size = x_set.len() * y_set.len();
        if expected_grid_size > 0 && (points.len() as f64 / expected_grid_size as f64) > 0.8 {
            PlotType::Grid
        } else {
            PlotType::Batch
        }
    };

    println!("Detected plot type: {:?}", plot_type);

    let (mut min_x, mut max_x) = (f64::MAX, f64::MIN);
    let (mut min_y, mut max_y) = (f64::MAX, f64::MIN);

    for p in &points {
        if p.x < min_x { min_x = p.x; }
        if p.x > max_x { max_x = p.x; }
        if p.y < min_y { min_y = p.y; }
        if p.y > max_y { max_y = p.y; }
    }

    // Determine spatial resolution for rect size
    let dx = if x_set.len() > 1 { (max_x - min_x) / (x_set.len() as f64 - 1.0) } else { 0.1 };
    let dy = if y_set.len() > 1 { (max_y - min_y) / (y_set.len() as f64 - 1.0) } else { 0.1 };

    let vmin = config.vmin.unwrap_or(-0.1);
    let vmax = config.vmax.unwrap_or(0.1);

    // Canvas size
    let mut width = config.width;
    let mut height = config.height;

    if config.aspect_equal {
        let dx_range = max_x - min_x;
        let dy_range = max_y - min_y;
        if dx_range > 0.0 && dy_range > 0.0 {
            let ratio = dy_range / dx_range;
            height = (width as f64 * ratio) as u32;
            
            // Limit height to reasonable bounds
            if height > 2000 { height = 2000; width = (height as f64 / ratio) as u32; }
            if height < 400 { height = 400; width = (height as f64 / ratio) as u32; }
        }
    }

    // Add extra width for colorbar
    let colorbar_width = 200;
    let canvas_width = width + colorbar_width;
    let root = BitMapBackend::new(&config.out_path, (canvas_width, height)).into_drawing_area();
    root.fill(&WHITE)?;

    // Split drawing area: main plot and colorbar
    let (plot_area, colorbar_area) = root.split_horizontally(width);

    let plot_title = if let Some(title) = &config.title {
        title.clone()
    } else {
        match plot_type {
            PlotType::Grid => "SeisBox Coulomb Stress - Grid Mode",
            PlotType::CrossSection => "SeisBox Coulomb Stress - Cross Section",
            PlotType::Batch => "SeisBox Coulomb Stress - Batch Mode",
        }.to_string()
    };

    let mut inp_faults = Vec::new();
    let mut cui_zero_lon = 0.0;
    let mut cui_zero_lat = 0.0;
    
    if let Some(inp_path) = &config.plot_inp {
        if let Ok(cui) = crate::core::cfs_parser::open_input_file_cui(inp_path) {
            inp_faults = cui.el;
            cui_zero_lon = cui.map_info.zero_lon;
            cui_zero_lat = cui.map_info.zero_lat;
            println!("Loaded {} faults from INP for plotting.", inp_faults.len());
        } else {
            println!("Warning: Failed to parse INP file for plotting.");
        }
    }

    let mut chart = ChartBuilder::on(&plot_area)
        .caption(plot_title, ("sans-serif", config.title_size).into_font())
        .margin(30)
        .x_label_area_size(70)
        .y_label_area_size(100)
        .build_cartesian_2d(min_x..max_x, if plot_type == PlotType::CrossSection { max_y..min_y } else { min_y..max_y })?;

    let x_desc = if plot_type == PlotType::CrossSection { "Distance (km)" } else { "Longitude" };
    let y_desc = if plot_type == PlotType::CrossSection { "Depth (km)" } else { "Latitude" };

    chart.configure_mesh()
        .x_desc(x_desc)
        .y_desc(y_desc)
        .label_style(("sans-serif", config.tick_size).into_font())
        .axis_desc_style(("sans-serif", config.label_size).into_font())
        .x_labels(config.x_labels)
        .y_labels(config.y_labels)
        .draw()?;

    match plot_type {
        PlotType::Grid | PlotType::CrossSection => {
            if config.use_contourf {
                println!("Building grid matrix for contourf...");
                match build_grid_matrix(&points) {
                    Ok((matrix, xs, ys)) => {
                        let nx = xs.len();
                        let ny = ys.len();
                        println!("Grid size: {}x{}", nx, ny);
                        
                        let res_x = config.upsample_res;
                        let res_y = config.upsample_res;
                        let dx_res = (max_x - min_x) / res_x as f64;
                        let dy_res = (max_y - min_y) / res_y as f64;
                        
                        let contour_steps = config.contour_steps as f64;
                        let step_size = (vmax - vmin) / contour_steps;
                        
                        let bilinear = |x: f64, y: f64| -> f64 {
                            let x_clamp = x.max(xs[0]).min(xs[nx-1]);
                            let y_clamp = y.max(ys[0]).min(ys[ny-1]);
                            
                            let xi = match xs.binary_search_by(|v| v.partial_cmp(&x_clamp).unwrap()) {
                                Ok(i) => i,
                                Err(i) => if i == 0 { 0 } else { i - 1 }
                            };
                            let yi = match ys.binary_search_by(|v| v.partial_cmp(&y_clamp).unwrap()) {
                                Ok(i) => i,
                                Err(i) => if i == 0 { 0 } else { i - 1 }
                            };
                            
                            let x0 = xi.min(nx.saturating_sub(2));
                            let x1 = (x0 + 1).min(nx - 1);
                            let y0 = yi.min(ny.saturating_sub(2));
                            let y1 = (y0 + 1).min(ny - 1);
                            
                            let q11 = matrix[y0 * nx + x0];
                            let q21 = matrix[y0 * nx + x1];
                            let q12 = matrix[y1 * nx + x0];
                            let q22 = matrix[y1 * nx + x1];
                            
                            let dx = xs[x1] - xs[x0];
                            let dy = ys[y1] - ys[y0];
                            
                            let tx = if dx > 0.0 { (x_clamp - xs[x0]) / dx } else { 0.0 };
                            let ty = if dy > 0.0 { (y_clamp - ys[y0]) / dy } else { 0.0 };
                            
                            let r1 = q11 * (1.0 - tx) + q21 * tx;
                            let r2 = q12 * (1.0 - tx) + q22 * tx;
                            
                            r1 * (1.0 - ty) + r2 * ty
                        };
                        
                        println!("Executing bilinear upsampling ({}x{})...", res_x, res_y);
                        
                        chart.draw_series((0..res_x).flat_map(|i| {
                            (0..res_y).map(move |j| {
                                let x = min_x + i as f64 * dx_res;
                                let y = min_y + j as f64 * dy_res;
                                
                                let mut val = bilinear(x, y);
                                // Quantize for contourf banded effect
                                if val < vmin { val = vmin; }
                                if val > vmax { val = vmax; }
                                let band = ((val - vmin) / step_size).floor();
                                let q_val = vmin + band * step_size;
                                
                                let color = cool_warm(q_val, vmin, vmax);
                                
                                let rect_x0 = x;
                                let rect_x1 = x + dx_res * 1.05; // slight overlap
                                let rect_y0 = y;
                                let rect_y1 = y + dy_res * 1.05;
                                
                                Rectangle::new([(rect_x0, rect_y0), (rect_x1, rect_y1)], color.filled())
                            })
                        }))?;
                    }
                    Err(e) => {
                        eprintln!("Warning: {}. Falling back to pcolormesh.", e);
                        chart.draw_series(points.iter().map(|p| {
                            let color = cool_warm(p.coulomb, vmin, vmax);
                            let x0 = p.x - dx / 2.0;
                            let x1 = p.x + dx / 2.0;
                            let y0 = p.y - dy / 2.0;
                            let y1 = p.y + dy / 2.0;
                            Rectangle::new([(x0, y0), (x1, y1)], color.filled())
                        }))?;
                    }
                }
            } else {
                // Draw as pcolormesh using Rectangles
                chart.draw_series(points.iter().map(|p| {
                    let color = cool_warm(p.coulomb, vmin, vmax);
                    let x0 = p.x - dx / 2.0;
                    let x1 = p.x + dx / 2.0;
                    let y0 = p.y - dy / 2.0;
                    let y1 = p.y + dy / 2.0;
                    Rectangle::new([(x0, y0), (x1, y1)], color.filled())
                }))?;
            }
        }
        PlotType::Batch => {
            if config.use_contourf {
                println!("Warning: --plot-contourf is ignored for Batch mode (unstructured data). Falling back to scatter plot.");
            }
            // Draw as scatter
            chart.draw_series(points.iter().map(|p| {
                let color = cool_warm(p.coulomb, vmin, vmax);
                Circle::new((p.x, p.y), 3, color.filled())
            }))?;
        }
    }

    // --- Overlay INP Faults ---
    if !inp_faults.is_empty() {
        if plot_type == PlotType::Grid || plot_type == PlotType::Batch {
            for f in &inp_faults {
                let xs = f[0];
                let ys = f[1];
                let xf = f[2];
                let yf = f[3];
                let dip = f[6];
                let top = f[7];
                let bottom = f[8];

                let dx = xf - xs;
                let dy = yf - ys;
                let len = (dx*dx + dy*dy).sqrt();
                let mut nx = 0.0;
                let mut ny = 0.0;
                if len > 0.0 {
                    nx = dy / len;
                    ny = -dx / len;
                }

                let h = bottom - top;
                let mut w_h = 0.0;
                if dip != 90.0 && dip != 0.0 {
                    w_h = h / dip.to_radians().tan();
                }

                let bx1 = xs + nx * w_h;
                let by1 = ys + ny * w_h;
                let bx2 = xf + nx * w_h;
                let by2 = yf + ny * w_h;

                let earth_r = 6371.0;
                let rad_conv = 180.0 / std::f64::consts::PI;
                let cos_lat = (cui_zero_lat / rad_conv).cos();

                let to_lon = |x_km: f64| cui_zero_lon + (x_km / (earth_r * cos_lat)) * rad_conv;
                let to_lat = |y_km: f64| cui_zero_lat + (y_km / earth_r) * rad_conv;

                let xs_ll = to_lon(xs);
                let ys_ll = to_lat(ys);
                let xf_ll = to_lon(xf);
                let yf_ll = to_lat(yf);
                let bx1_ll = to_lon(bx1);
                let by1_ll = to_lat(by1);
                let bx2_ll = to_lon(bx2);
                let by2_ll = to_lat(by2);

                let f_color = parse_rgb_color(config.fault_color.as_deref().unwrap_or("black"));
                let f_width = config.fault_width.unwrap_or(4);
                let f_style = config.fault_style.as_deref().unwrap_or("solid").to_lowercase();

                // Draw fault perimeter (excluding top edge which is drawn later)
                let peri_width = (f_width / 2).max(1);
                if let Err(e) = chart.draw_series(std::iter::once(
                    PathElement::new(
                        vec![(xf_ll, yf_ll), (bx2_ll, by2_ll), (bx1_ll, by1_ll), (xs_ll, ys_ll)],
                        ShapeStyle::from(&f_color).stroke_width(peri_width)
                    )
                )) {
                    println!("Warning: Failed to draw fault perimeter: {}", e);
                }

                // Draw top edge
                if f_style == "dashed" || f_style == "dash" {
                    // Manual dashed line implementation
                    let mut segments = Vec::new();
                    let dash_count = 10;
                    let dx = xf_ll - xs_ll;
                    let dy = yf_ll - ys_ll;
                    for i in 0..dash_count {
                        if i % 2 == 0 {
                            let t1 = i as f64 / dash_count as f64;
                            let t2 = (i + 1) as f64 / dash_count as f64;
                            segments.push(PathElement::new(
                                vec![(xs_ll + t1 * dx, ys_ll + t1 * dy), (xs_ll + t2 * dx, ys_ll + t2 * dy)],
                                ShapeStyle::from(&f_color).stroke_width(f_width)
                            ));
                        }
                    }
                    for seg in segments {
                        if let Err(e) = chart.draw_series(std::iter::once(seg)) {
                            println!("Warning: Failed to draw dashed fault edge: {}", e);
                        }
                    }
                } else {
                    if let Err(e) = chart.draw_series(std::iter::once(
                        PathElement::new(
                            vec![(xs_ll, ys_ll), (xf_ll, yf_ll)],
                            ShapeStyle::from(&f_color).stroke_width(f_width)
                        )
                    )) {
                        println!("Warning: Failed to draw fault top edge: {}", e);
                    }
                }
            }
            
            // Draw CS Track if enabled
            if config.plot_cs_track {
                if let (Some(lons1), Some(lats1), Some(lons2), Some(lats2)) = (&config.cs_start_lon, &config.cs_start_lat, &config.cs_finish_lon, &config.cs_finish_lat) {
                    
                    let track_color = parse_rgb_color(config.cs_track_color.as_deref().unwrap_or("black"));
                    let track_width = config.cs_track_width.unwrap_or(5);
                    let track_style = config.cs_track_style.as_deref().unwrap_or("solid").to_lowercase();
                    
                    let min_len = lons1.len().min(lats1.len()).min(lons2.len()).min(lats2.len());
                    
                    for idx in 0..min_len {
                        let x1 = lons1[idx];
                        let y1 = lats1[idx];
                        let x2 = lons2[idx];
                        let y2 = lats2[idx];
                        
                        if track_style == "dashed" || track_style == "dash" {
                            let mut segments = Vec::new();
                            let dash_count = 15;
                            let dx = x2 - x1;
                            let dy = y2 - y1;
                            for i in 0..dash_count {
                                if i % 2 == 0 {
                                    let t1 = i as f64 / dash_count as f64;
                                    let t2 = (i + 1) as f64 / dash_count as f64;
                                    segments.push(PathElement::new(
                                        vec![(x1 + t1 * dx, y1 + t1 * dy), (x1 + t2 * dx, y1 + t2 * dy)],
                                        ShapeStyle::from(&track_color).stroke_width(track_width)
                                    ));
                                }
                            }
                            for seg in segments {
                                if let Err(e) = chart.draw_series(std::iter::once(seg)) {
                                    println!("Warning: Failed to draw dashed CS track: {}", e);
                                }
                            }
                        } else {
                            if let Err(e) = chart.draw_series(std::iter::once(
                                PathElement::new(
                                    vec![(x1, y1), (x2, y2)],
                                    ShapeStyle::from(&track_color).stroke_width(track_width)
                                )
                            )) {
                                println!("Warning: Failed to draw CS track: {}", e);
                            }
                        }
                        
                        // Draw A, B, C... labels at the ends
                        let start_label = (b'A' + (idx as u8 % 26)) as char;
                        let finish_label = format!("{}'", start_label);
                        
                        let label_font = ("sans-serif", config.label_size).into_font().color(&track_color);
                        if let Err(e) = chart.draw_series(std::iter::once(Text::new(start_label.to_string(), (x1, y1), label_font.clone()))) {
                            println!("Warning: Failed to draw CS label {}: {}", start_label, e);
                        }
                        if let Err(e) = chart.draw_series(std::iter::once(Text::new(finish_label.clone(), (x2, y2), label_font))) {
                            println!("Warning: Failed to draw CS label {}: {}", finish_label, e);
                        }
                    }
                }
            }
        } else if plot_type == PlotType::CrossSection {
            if let (Some(lons1), Some(lats1), Some(lons2), Some(lats2)) = (&config.cs_start_lon, &config.cs_start_lat, &config.cs_finish_lon, &config.cs_finish_lat) {
                if lons1.is_empty() || lats1.is_empty() || lons2.is_empty() || lats2.is_empty() {
                    return Ok(());
                }
                let x1 = lons1[0];
                let y1 = lats1[0];
                let x2 = lons2[0];
                let y2 = lats2[0];
                
                let x1_km = x1;
                let y1_km = y1;
                let x2_km = x2;
                let y2_km = y2;

                let cs_dx = x2_km - x1_km;
                let cs_dy = y2_km - y1_km;
                let cs_len = (cs_dx * cs_dx + cs_dy * cs_dy).sqrt();

                if cs_len > 0.0 {
                    for f in &inp_faults {
                        let xs = f[0];
                        let ys = f[1];
                        let xf = f[2];
                        let yf = f[3];
                        let dip = f[6];
                        let top = f[7];
                        let bottom = f[8];

                        let dx = xf - xs;
                        let dy = yf - ys;
                        let len = (dx*dx + dy*dy).sqrt();
                        let mut nx = 0.0;
                        let mut ny = 0.0;
                        if len > 0.0 {
                            nx = dy / len;
                            ny = -dx / len;
                        }

                        let h = bottom - top;
                        let mut w_h = 0.0;
                        if dip != 90.0 && dip != 0.0 {
                            w_h = h / dip.to_radians().tan();
                        }

                        let bx1 = xs + nx * w_h;
                        let by1 = ys + ny * w_h;
                        let bx2 = xf + nx * w_h;
                        let by2 = yf + ny * w_h;

                        let cs_p1 = (x1_km, y1_km);
                        let cs_p2 = (x2_km, y2_km);
                        let e1 = segments_intersect(cs_p1, cs_p2, (xs, ys), (xf, yf));
                        let e2 = segments_intersect(cs_p1, cs_p2, (xf, yf), (bx2, by2));
                        let e3 = segments_intersect(cs_p1, cs_p2, (bx2, by2), (bx1, by1));
                        let e4 = segments_intersect(cs_p1, cs_p2, (bx1, by1), (xs, ys));

                        if !e1 && !e2 && !e3 && !e4 {
                            continue;
                        }

                        let tc_x = (xs + xf) / 2.0;
                        let tc_y = (ys + yf) / 2.0;
                        let bc_x = (bx1 + bx2) / 2.0;
                        let bc_y = (by1 + by2) / 2.0;

                        // Project onto the cross section line
                        let proj_tc = ((tc_x - x1_km) * cs_dx + (tc_y - y1_km) * cs_dy) / cs_len;
                        let proj_bc = ((bc_x - x1_km) * cs_dx + (bc_y - y1_km) * cs_dy) / cs_len;

                        let f_color = parse_rgb_color(config.fault_color.as_deref().unwrap_or("black"));
                        let f_width = config.fault_width.unwrap_or(4);
                        let f_style = config.fault_style.as_deref().unwrap_or("solid").to_lowercase();

                        if f_style == "dashed" || f_style == "dash" {
                            let mut segments = Vec::new();
                            let dash_count = 10;
                            let dx = proj_bc - proj_tc;
                            let dy = bottom - top;
                            for i in 0..dash_count {
                                if i % 2 == 0 {
                                    let t1 = i as f64 / dash_count as f64;
                                    let t2 = (i + 1) as f64 / dash_count as f64;
                                    segments.push(PathElement::new(
                                        vec![(proj_tc + t1 * dx, top + t1 * dy), (proj_tc + t2 * dx, top + t2 * dy)],
                                        ShapeStyle::from(&f_color).stroke_width(f_width)
                                    ));
                                }
                            }
                            for seg in segments {
                                if let Err(e) = chart.draw_series(std::iter::once(seg)) {
                                    println!("Warning: Failed to draw cross section fault: {}", e);
                                }
                            }
                        } else {
                            if let Err(e) = chart.draw_series(std::iter::once(
                                PathElement::new(
                                    vec![(proj_tc, top), (proj_bc, bottom)],
                                    ShapeStyle::from(&f_color).stroke_width(f_width)
                                )
                            )) {
                                println!("Warning: Failed to draw cross section fault: {}", e);
                            }
                        }
                    }
                }
            } else {
                println!("Warning: --plot-inp provided for Cross Section, but cross section coordinates (--cs-start-lon, etc.) are missing. Cannot project faults.");
            }
        }
    }

    // --- Draw Colorbar ---
    let cb_pad = (vmax - vmin) * 0.05; // 5% padding for arrows
    let extend_mode = config.cbar_extend.as_deref().unwrap_or("both");
    let mut cb_min_bound = vmin;
    let mut cb_max_bound = vmax;
    
    if extend_mode == "min" || extend_mode == "both" {
        cb_min_bound -= cb_pad;
    }
    if extend_mode == "max" || extend_mode == "both" {
        cb_max_bound += cb_pad;
    }

    let mut cb_chart = ChartBuilder::on(&colorbar_area)
        .margin(20)
        .set_label_area_size(LabelAreaPosition::Right, 80)
        .build_cartesian_2d(0.0..1.0, cb_min_bound..cb_max_bound)?;

    let mut cbar_mesh = cb_chart.configure_mesh();
    
    let cbar_label = config.cbar_label.as_deref().unwrap_or("Coulomb Stress Change (bar)");
    let cbar_label_size = config.cbar_label_size.unwrap_or(config.label_size);
    let cbar_tick_size = config.cbar_tick_size.unwrap_or(config.tick_size);
    let cbar_y_labels = config.cbar_y_labels.unwrap_or(10);

    cbar_mesh.disable_x_mesh()
        .disable_y_mesh()
        .disable_x_axis()
        .y_labels(cbar_y_labels)
        .label_style(("sans-serif", cbar_tick_size).into_font())
        .y_desc(cbar_label)
        .axis_desc_style(("sans-serif", cbar_label_size).into_font())
        .draw()?;

    let cb_steps = 100;
    let step_size = (vmax - vmin) / cb_steps as f64;
    
    cb_chart.draw_series((0..cb_steps).map(|i| {
        let y0 = vmin + i as f64 * step_size;
        let y1 = y0 + step_size;
        let color = cool_warm(y0, vmin, vmax);
        Rectangle::new([(0.0, y0), (1.0, y1)], color.filled())
    }))?;

    // Draw extend arrows (Top and Bottom Polygons) if requested
    if extend_mode == "max" || extend_mode == "both" {
        cb_chart.draw_series(std::iter::once(
            Polygon::new(vec![(0.0, vmax), (1.0, vmax), (0.5, vmax + cb_pad)], cool_warm(vmax, vmin, vmax).filled())
        ))?;
    }
    if extend_mode == "min" || extend_mode == "both" {
        cb_chart.draw_series(std::iter::once(
            Polygon::new(vec![(0.0, vmin), (1.0, vmin), (0.5, vmin - cb_pad)], cool_warm(vmin, vmin, vmax).filled())
        ))?;
    }

    root.present()?;
    println!("Successfully saved plot to {}", config.out_path);

    Ok(())
}
