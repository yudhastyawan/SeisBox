use crate::core::catalogue::Catalogue;
use crate::core::gutenberg_richter::{fmd, BvalueResult};
use plotters::prelude::*;
use std::path::Path;

pub fn plot_fmd(
    cat: &Catalogue, 
    path: &Path, 
    bin_width: f64, 
    b_res: Option<&BvalueResult>,
    width: u32,
    height: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    
    let root = BitMapBackend::new(path, (width, height)).into_drawing_area();
    root.fill(&WHITE)?;
    
    let mags: Vec<f64> = cat.events.iter().map(|e| e.mag).collect();
    let (centers, incr, cumul) = fmd(&mags, bin_width);
    
    if centers.is_empty() {
        return Err("No data to plot".into());
    }
    
    let min_mag = centers.first().copied().unwrap_or(0.0) - bin_width;
    let max_mag = centers.last().copied().unwrap_or(10.0) + bin_width;
    let max_count = cumul.iter().copied().max().unwrap_or(1) as f64;
    
    let mut chart = ChartBuilder::on(&root)
        .caption("Frequency-Magnitude Distribution", ("sans-serif", 30).into_font())
        .margin(20)
        .x_label_area_size(40)
        .y_label_area_size(50)
        .build_cartesian_2d(min_mag..max_mag, (0.1f64..max_count * 2.0).log_scale())?;
        
    chart.configure_mesh()
        .x_desc("Magnitude")
        .y_desc("Number of Events (Cumulative & Incremental)")
        .draw()?;
        
    // Plot Incremental (Triangles)
    chart.draw_series(
        centers.iter().zip(incr.iter()).filter(|&(_, &c)| c > 0).map(|(&x, &y)| {
            TriangleMarker::new((x, y as f64), 5, &BLUE)
        })
    )?.label("Incremental").legend(|(x, y)| TriangleMarker::new((x, y), 5, &BLUE));
    
    // Plot Cumulative (Squares)
    chart.draw_series(
        centers.iter().zip(cumul.iter()).filter(|&(_, &c)| c > 0).map(|(&x, &y)| {
            Rectangle::new([(x - 0.02, y as f64 * 0.95), (x + 0.02, y as f64 * 1.05)], RED.filled())
        })
    )?.label("Cumulative").legend(|(x, y)| Rectangle::new([(x - 5, y - 5), (x + 5, y + 5)], RED.filled()));
    
    // Plot GR line if available
    if let Some(res) = b_res {
        let x0 = res.mc;
        let y0 = 10f64.powf(res.a - res.b * x0);
        
        let x1 = max_mag;
        let y1 = 10f64.powf(res.a - res.b * x1);
        
        chart.draw_series(LineSeries::new(
            vec![(x0, y0), (x1, y1)],
            &BLACK,
        ))?.label(format!("GR Fit (b={:.2}±{:.2}, Mc={:.1})", res.b, res.b_uncertainty, res.mc))
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 10, y)], &BLACK));
        
        // Draw vertical line for Mc
        chart.draw_series(LineSeries::new(
            vec![(res.mc, 0.1), (res.mc, max_count * 2.0)],
            &BLACK.mix(0.5),
        ))?.label("Mc");
    }
    
    chart.configure_series_labels()
        .background_style(&WHITE.mix(0.8))
        .border_style(&BLACK)
        .draw()?;
        
    root.present()?;
    
    Ok(())
}

pub fn plot_cumulative(
    cat: &Catalogue, 
    path: &Path, 
    width: u32,
    height: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = BitMapBackend::new(path, (width, height)).into_drawing_area();
    root.fill(&WHITE)?;
    
    if cat.events.is_empty() { return Err("No data".into()); }
    
    let mut times: Vec<i64> = cat.events.iter().map(|e| e.time.timestamp()).collect();
    times.sort_unstable();
    
    let min_t = times.first().copied().unwrap_or(0);
    let max_t = times.last().copied().unwrap_or(1);
    
    let mut chart = ChartBuilder::on(&root)
        .caption("Cumulative Seismicity", ("sans-serif", 30).into_font())
        .margin(20)
        .x_label_area_size(40)
        .y_label_area_size(50)
        .build_cartesian_2d(min_t..max_t, 0f64..(times.len() as f64 * 1.05))?;
        
    chart.configure_mesh()
        .x_label_formatter(&|x| chrono::DateTime::from_timestamp(*x, 0).map(|t| t.format("%Y-%m").to_string()).unwrap_or_default())
        .draw()?;
        
    let line_data: Vec<(i64, f64)> = times.iter().enumerate().map(|(i, &t)| (t, (i + 1) as f64)).collect();
    
    chart.draw_series(LineSeries::new(line_data, &BLUE))?;
    
    root.present()?;
    Ok(())
}

pub fn plot_mag_vs_time(
    cat: &Catalogue, 
    path: &Path, 
    width: u32,
    height: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = BitMapBackend::new(path, (width, height)).into_drawing_area();
    root.fill(&WHITE)?;
    
    if cat.events.is_empty() { return Err("No data".into()); }
    
    let min_t = cat.events.iter().map(|e| e.time.timestamp()).min().unwrap_or(0);
    let max_t = cat.events.iter().map(|e| e.time.timestamp()).max().unwrap_or(1);
    let min_m = cat.events.iter().map(|e| e.mag).fold(f64::INFINITY, f64::min);
    let max_m = cat.events.iter().map(|e| e.mag).fold(f64::NEG_INFINITY, f64::max);
    
    let mut chart = ChartBuilder::on(&root)
        .caption("Magnitude vs Time", ("sans-serif", 30).into_font())
        .margin(20)
        .x_label_area_size(40)
        .y_label_area_size(50)
        .build_cartesian_2d(min_t..max_t, (min_m - 0.5)..(max_m + 0.5))?;
        
    chart.configure_mesh()
        .x_label_formatter(&|x| chrono::DateTime::from_timestamp(*x, 0).map(|t| t.format("%Y-%m").to_string()).unwrap_or_default())
        .draw()?;
        
    chart.draw_series(
        cat.events.iter().map(|e| Circle::new((e.time.timestamp(), e.mag), 2, BLUE.filled()))
    )?;
    
    root.present()?;
    Ok(())
}

pub fn plot_depth_histogram(
    cat: &Catalogue, 
    path: &Path, 
    width: u32,
    height: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = BitMapBackend::new(path, (width, height)).into_drawing_area();
    root.fill(&WHITE)?;
    
    if cat.events.is_empty() { return Err("No data".into()); }
    
    let max_depth = cat.events.iter().map(|e| e.depth).fold(0f64, f64::max).ceil() as u32;
    let bin_size = 5;
    let n_bins = (max_depth / bin_size) as usize + 1;
    let mut bins = vec![0u32; n_bins];
    
    for e in &cat.events {
        let b = (e.depth as u32) / bin_size;
        if (b as usize) < n_bins { bins[b as usize] += 1; }
    }
    
    let max_count = *bins.iter().max().unwrap_or(&1) as u32;
    
    let mut chart = ChartBuilder::on(&root)
        .caption("Depth Distribution", ("sans-serif", 30).into_font())
        .margin(20)
        .x_label_area_size(40)
        .y_label_area_size(50)
        .build_cartesian_2d(0..max_depth, 0..(max_count as f32 * 1.1) as u32)?;
        
    chart.configure_mesh().x_desc("Depth (km)").y_desc("Count").draw()?;
    
    chart.draw_series(
        bins.into_iter().enumerate().map(|(i, c)| {
            let x0 = (i as u32) * bin_size;
            let x1 = x0 + bin_size;
            Rectangle::new([(x0, 0), (x1, c)], BLUE.filled())
        })
    )?;
    
    root.present()?;
    Ok(())
}

pub fn plot_seismicity_rate(
    cat: &Catalogue, 
    path: &Path, 
    bin_days: i64,
    width: u32,
    height: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = BitMapBackend::new(path, (width, height)).into_drawing_area();
    root.fill(&WHITE)?;
    
    if cat.events.is_empty() { return Err("No data".into()); }
    
    let min_t = cat.events.iter().map(|e| e.time.timestamp()).min().unwrap_or(0);
    let max_t = cat.events.iter().map(|e| e.time.timestamp()).max().unwrap_or(1);
    
    let bin_secs = bin_days * 86400;
    if bin_secs <= 0 { return Err("bin_days must be > 0".into()); }
    
    let n_bins = ((max_t - min_t) / bin_secs) as usize + 1;
    let mut bins = vec![0u32; n_bins];
    
    for e in &cat.events {
        let b = (e.time.timestamp() - min_t) / bin_secs;
        if (b as usize) < n_bins { bins[b as usize] += 1; }
    }
    
    let max_count = *bins.iter().max().unwrap_or(&1) as u32;
    
    let mut chart = ChartBuilder::on(&root)
        .caption(format!("Seismicity Rate ({} days/bin)", bin_days), ("sans-serif", 30).into_font())
        .margin(20)
        .x_label_area_size(40)
        .y_label_area_size(50)
        .build_cartesian_2d(min_t..max_t, 0..(max_count as f32 * 1.1) as u32)?;
        
    chart.configure_mesh()
        .x_label_formatter(&|x| chrono::DateTime::from_timestamp(*x, 0).map(|t| t.format("%Y-%m").to_string()).unwrap_or_default())
        .y_desc("Number of events")
        .draw()?;
        
    chart.draw_series(
        bins.into_iter().enumerate().map(|(i, c)| {
            let x0 = min_t + (i as i64) * bin_secs;
            let x1 = x0 + bin_secs;
            Rectangle::new([(x0, 0), (x1, c)], BLUE.filled())
        })
    )?;
    
    root.present()?;
    Ok(())
}

pub fn plot_time_of_day(
    cat: &Catalogue, 
    path: &Path, 
    width: u32,
    height: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    use chrono::Timelike;
    let root = BitMapBackend::new(path, (width, height)).into_drawing_area();
    root.fill(&WHITE)?;
    
    if cat.events.is_empty() { return Err("No data".into()); }
    
    let mut bins = vec![0u32; 24]; // 24 hours
    for e in &cat.events {
        let hour = e.time.hour() as usize;
        if hour < 24 { bins[hour] += 1; }
    }
    
    let max_count = *bins.iter().max().unwrap_or(&1) as u32;
    
    let mut chart = ChartBuilder::on(&root)
        .caption("Time-of-day Histogram (UTC)", ("sans-serif", 30).into_font())
        .margin(20)
        .x_label_area_size(40)
        .y_label_area_size(50)
        .build_cartesian_2d(0..24u32, 0..(max_count as f32 * 1.1) as u32)?;
        
    chart.configure_mesh().x_desc("Hour of day").y_desc("Count").draw()?;
    
    chart.draw_series(
        bins.into_iter().enumerate().map(|(i, c)| {
            Rectangle::new([(i as u32, 0), ((i + 1) as u32, c)], BLUE.filled())
        })
    )?;
    
    root.present()?;
    Ok(())
}

use serde::Deserialize;
#[derive(Deserialize, Debug)]
struct GridRow {
    lon: f64,
    lat: f64,
    val: f64,
    n_events: usize,
    radius_km: f64,
}

pub fn plot_spatial_grid(
    csv_path: &Path,
    out_path: &Path,
    param_name: &str,
    width: u32,
    height: u32,
    point_radius: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut rdr = csv::Reader::from_path(csv_path)?;
    let mut rows: Vec<GridRow> = Vec::new();
    
    for result in rdr.deserialize() {
        let record: GridRow = result?;
        // Ignore NaN or Infinity values
        if record.val.is_finite() {
            rows.push(record);
        }
    }
    
    if rows.is_empty() {
        return Err("No valid data found in grid CSV".into());
    }
    
    // Find bounds
    let min_lon = rows.iter().map(|r| r.lon).fold(f64::INFINITY, f64::min);
    let max_lon = rows.iter().map(|r| r.lon).fold(f64::NEG_INFINITY, f64::max);
    let min_lat = rows.iter().map(|r| r.lat).fold(f64::INFINITY, f64::min);
    let max_lat = rows.iter().map(|r| r.lat).fold(f64::NEG_INFINITY, f64::max);
    let min_val = rows.iter().map(|r| r.val).fold(f64::INFINITY, f64::min);
    let max_val = rows.iter().map(|r| r.val).fold(f64::NEG_INFINITY, f64::max);
    
    // Pad bounds
    let lon_pad = (max_lon - min_lon) * 0.05;
    let lat_pad = (max_lat - min_lat) * 0.05;
    
    let root = BitMapBackend::new(out_path, (width, height)).into_drawing_area();
    root.fill(&WHITE)?;
    
    let mut chart = ChartBuilder::on(&root)
        .caption(format!("Spatial Grid Map ({})", param_name), ("sans-serif", 30).into_font())
        .margin(20)
        .x_label_area_size(40)
        .y_label_area_size(50)
        .build_cartesian_2d((min_lon - lon_pad)..(max_lon + lon_pad), (min_lat - lat_pad)..(max_lat + lat_pad))?;
        
    chart.configure_mesh().x_desc("Longitude").y_desc("Latitude").draw()?;
    
    let val_range = if (max_val - min_val).abs() < 1e-6 { 1.0 } else { max_val - min_val };
    
    chart.draw_series(
        rows.into_iter().map(|row| {
            // Simple Blue-to-Red interpolator
            let norm = ((row.val - min_val) / val_range).clamp(0.0, 1.0);
            let r = (255.0 * norm) as u8;
            let b = (255.0 * (1.0 - norm)) as u8;
            let color = RGBColor(r, 50, b);
            Circle::new((row.lon, row.lat), point_radius, color.filled())
        })
    )?;
    
    root.present()?;
    Ok(())
}
