use plotters::prelude::*;
use seisbox_core::core::isc_client::{EarthquakeEvent, ConversionRule, apply_conversion};

use std::path::Path;
use std::collections::HashMap;

pub fn generate_magnitude_catalog_viz<P: AsRef<Path>>(
    events: &[EarthquakeEvent],
    out_path: P,
) -> Result<(), Box<dyn std::error::Error>> {
    if events.is_empty() { return Ok(()); }

    // First convert to Mw so we get `magnitude_real` equivalent
    // Wait, the notebook counts the real magnitudes (before conversion) for top 6!
    // "subset = df[df['magnitude_type'] == mtype]"
    // The notebook does this on the ORIGINAL data, before conversion!
    // But wait, the notebook uses `mag_type_counter` which is based on the original data.
    // So we use ev.mag_type
    
    let mut type_counts = HashMap::new();
    for ev in events {
        *type_counts.entry(ev.mag_type.clone()).or_insert(0) += 1;
    }
    let mut type_counts_vec: Vec<(String, i32)> = type_counts.into_iter().collect();
    type_counts_vec.sort_by(|a, b| b.1.cmp(&a.1));
    let top6: Vec<String> = type_counts_vec.iter().take(6).map(|(k, _)| k.clone()).collect();
    
    let top6_counts: Vec<(String, i32)> = top6.iter().map(|k| {
        let count = type_counts_vec.iter().find(|(t, _)| t == k).map(|(_, c)| *c).unwrap_or(0);
        (k.clone(), count)
    }).collect();

    let width = 1200;
    let height = 2400; // Tall image for 7 rows
    let root = BitMapBackend::new(&out_path, (width, height)).into_drawing_area();
    root.fill(&WHITE)?;

    let root_areas = root.split_vertically(400);
    let top_area = root_areas.0; 
    let bottom_area = root_areas.1; 
    
    // 1. Top Bar Chart
    let max_top_count = top6_counts.iter().map(|(_, c)| *c).max().unwrap_or(0) as f64;
    let mut top_chart = ChartBuilder::on(&top_area)
        .caption("Number of Events per Magnitude Type", ("sans-serif", 30).into_font())
        .margin(20)
        .x_label_area_size(40)
        .y_label_area_size(60)
        .build_cartesian_2d(
            0..top6_counts.len(),
            0.0..max_top_count * 1.1
        )?;

    top_chart.configure_mesh()
        .x_desc("Magnitude Type")
        .y_desc("Number of Events")
        .x_label_formatter(&|v: &usize| {
            if *v < top6_counts.len() {
                return top6_counts[*v].0.clone();
            }
            "".to_string()
        })
        .draw()?;

    top_chart.draw_series(
        top6_counts.iter().enumerate().map(|(i, (_, c))| {
            let x0 = i;
            let x1 = i + 1;
            Rectangle::new(
                [(x0, 0.0), (x1, *c as f64)],
                RGBColor(70, 130, 180).filled(), // steelblue
            )
        })
    )?;

    // Split bottom area into 6 rows
    let grid_areas = bottom_area.split_evenly((6, 2));

    let mut bins = Vec::new();
    let mut b = 0.0;
    while b <= 10.0 {
        bins.push(b);
        b += 0.5;
    }

    // Rows 2-7
    for (i, mtype) in top6.iter().enumerate() {
        let left_area = &grid_areas[i * 2];
        let right_area = &grid_areas[i * 2 + 1];

        // Left: Top 5 authors
        let mut author_counts = HashMap::new();
        for ev in events {
            if &ev.mag_type == mtype {
                let au = if ev.author.is_empty() { "UNKNOWN".to_string() } else { ev.author.clone() };
                *author_counts.entry(au).or_insert(0) += 1;
            }
        }
        let mut au_vec: Vec<(String, i32)> = author_counts.into_iter().collect();
        au_vec.sort_by(|a, b| b.1.cmp(&a.1));
        let top5_au: Vec<(String, i32)> = au_vec.into_iter().take(5).collect();

        let max_au_count = top5_au.iter().map(|(_, c)| *c).max().unwrap_or(0) as f64;
        let mut left_chart = ChartBuilder::on(left_area)
            .caption(format!("{} - Top Authors", mtype), ("sans-serif", 20).into_font())
            .margin(20)
            .x_label_area_size(40)
            .y_label_area_size(60)
            .build_cartesian_2d(
                0..top5_au.len(),
                0.0..max_au_count * 1.1
            )?;

        left_chart.configure_mesh()
            .y_desc("Number of Events")
            .x_label_formatter(&|v: &usize| {
                if *v < top5_au.len() {
                    return top5_au[*v].0.clone();
                }
                "".to_string()
            })
            .draw()?;

        left_chart.draw_series(
            top5_au.iter().enumerate().map(|(idx, (_, c))| {
                let x0 = idx;
                let x1 = idx + 1;
                Rectangle::new(
                    [(x0, 0.0), (x1, *c as f64)],
                    RGBColor(255, 140, 0).filled(), // darkorange
                )
            })
        )?;

        // Right: Magnitude Distribution
        let mut hist_counts = vec![0; bins.len() - 1];
        for ev in events {
            if &ev.mag_type == mtype {
                let m = ev.mag;
                for j in 0..bins.len() - 1 {
                    if m >= bins[j] && m < bins[j+1] {
                        hist_counts[j] += 1;
                        break;
                    }
                }
            }
        }
        
        let max_hist = hist_counts.iter().max().unwrap_or(&0).clone() as f64;
        let mut right_chart = ChartBuilder::on(right_area)
            .caption(format!("{} - Magnitude Distribution", mtype), ("sans-serif", 20).into_font())
            .margin(20)
            .x_label_area_size(40)
            .y_label_area_size(60)
            .build_cartesian_2d(
                0.0f64..10.0f64,
                0.0..max_hist * 1.1
            )?;

        right_chart.configure_mesh()
            .x_desc("Magnitude Range")
            .draw()?;

        right_chart.draw_series(
            hist_counts.iter().enumerate().filter(|(_, &c)| c > 0).map(|(idx, &count)| {
                let x0 = bins[idx];
                let x1 = bins[idx+1];
                Rectangle::new(
                    [(x0, 0.0), (x1, count as f64)],
                    RGBColor(60, 179, 113).filled(), // mediumseagreen
                )
            })
        )?;
    }

    root.present()?;
    Ok(())
}

pub fn generate_magnitude_piechart_viz<P: AsRef<Path>>(
    events: &[EarthquakeEvent],
    rules: &[ConversionRule],
    out_path: P,
) -> Result<(), Box<dyn std::error::Error>> {
    if events.is_empty() { return Ok(()); }

    let mut conv_counts = HashMap::new();
    let mut total_converted = 0;
    for ev in events {
        let (_, _, converted_from) = apply_conversion(rules, ev.mag, &ev.mag_type);
        if !converted_from.is_empty() {
            *conv_counts.entry(converted_from).or_insert(0) += 1;
            total_converted += 1;
        }
    }

    let mut conv_vec: Vec<(String, i32)> = conv_counts.into_iter().collect();
    conv_vec.sort_by(|a, b| b.1.cmp(&a.1));

    let width = 800;
    let height = 800;
    let root = BitMapBackend::new(&out_path, (width, height)).into_drawing_area();
    root.fill(&WHITE)?;

    let mut empty_chart = ChartBuilder::on(&root)
        .caption("Distribusi Tipe Magnitudo Asal yang Dikonversi (Mw)", ("sans-serif", 25).into_font())
        .build_cartesian_2d(0..1, 0..1)?;
    empty_chart.configure_mesh().disable_mesh().disable_axes().draw()?;

    let sizes: Vec<f64> = conv_vec.iter().map(|(_, c)| *c as f64).collect();
    let labels: Vec<String> = conv_vec.iter().map(|(t, c)| {
        let pct = (*c as f64 / total_converted as f64) * 100.0;
        format!("{} ({:.1}%)", t, pct)
    }).collect();

    let palette = vec![
        RGBColor(166, 206, 227),
        RGBColor(31, 120, 180),
        RGBColor(178, 223, 138),
        RGBColor(51, 160, 44),
        RGBColor(251, 154, 153),
        RGBColor(227, 26, 28),
        RGBColor(253, 191, 111),
        RGBColor(255, 127, 0),
        RGBColor(202, 178, 214),
        RGBColor(106, 61, 154),
    ];
    let mut colors = Vec::new();
    for i in 0..sizes.len() {
        colors.push(palette[i % palette.len()]);
    }
    let pie = Pie::new(&(400, 400), &250.0, &sizes, &colors, &labels);
    root.draw(&pie)?;

    root.present()?;
    Ok(())
}

pub fn generate_spatial_map_viz<P: AsRef<Path>>(
    events: &[EarthquakeEvent],
    cs_track: Option<((f64, f64), (f64, f64))>,
    out_path: P,
) -> Result<(), Box<dyn std::error::Error>> {
    if events.is_empty() { return Ok(()); }

    let mut min_lon = f64::MAX;
    let mut max_lon = f64::MIN;
    let mut min_lat = f64::MAX;
    let mut max_lat = f64::MIN;

    for ev in events {
        if ev.lon < min_lon { min_lon = ev.lon; }
        if ev.lon > max_lon { max_lon = ev.lon; }
        if ev.lat < min_lat { min_lat = ev.lat; }
        if ev.lat > max_lat { max_lat = ev.lat; }
    }

    let lon_margin = (max_lon - min_lon) * 0.1;
    let lat_margin = (max_lat - min_lat) * 0.1;
    let lon_margin = if lon_margin == 0.0 { 1.0 } else { lon_margin };
    let lat_margin = if lat_margin == 0.0 { 1.0 } else { lat_margin };

    min_lon -= lon_margin; max_lon += lon_margin;
    min_lat -= lat_margin; max_lat += lat_margin;

    let width = 1000;
    let height = 800;
    let root = BitMapBackend::new(&out_path, (width, height)).into_drawing_area();
    root.fill(&WHITE)?;

    let mut chart = ChartBuilder::on(&root)
        .caption(format!("Spatial Distribution ({} Events)", events.len()), ("sans-serif", 30).into_font())
        .margin(30)
        .x_label_area_size(50)
        .y_label_area_size(50)
        .build_cartesian_2d(min_lon..max_lon, min_lat..max_lat)?;

    chart.configure_mesh()
        .x_desc("Longitude")
        .y_desc("Latitude")
        .axis_desc_style(("sans-serif", 20).into_font())
        .draw()?;

    chart.draw_series(std::iter::once(EmptyElement::at((0.0, 0.0))))?
        .label("Depth <= 70 km")
        .legend(|(x, y)| Circle::new((x, y), 5, RGBColor(220, 20, 60).filled()));
    
    chart.draw_series(std::iter::once(EmptyElement::at((0.0, 0.0))))?
        .label("Depth 70 - 300 km")
        .legend(|(x, y)| Circle::new((x, y), 5, RGBColor(34, 139, 34).filled()));

    chart.draw_series(std::iter::once(EmptyElement::at((0.0, 0.0))))?
        .label("Depth > 300 km")
        .legend(|(x, y)| Circle::new((x, y), 5, RGBColor(0, 0, 205).filled()));

    chart.draw_series(
        events.iter().map(|ev| {
            // Depth colors: 0-70=Red, 70-300=Green, >300=Blue
            let color = if ev.depth_km <= 70.0 {
                RGBColor(220, 20, 60) // Crimson
            } else if ev.depth_km <= 300.0 {
                RGBColor(34, 139, 34) // ForestGreen
            } else {
                RGBColor(0, 0, 205)   // MediumBlue
            };
            
            // Size by magnitude
            let size = if ev.mag > 0.0 { (ev.mag * 1.5) as u32 } else { 2 };
            Circle::new((ev.lon, ev.lat), size, color.filled())
        })
    )?;

    chart.configure_series_labels()
        .background_style(WHITE.mix(0.8))
        .border_style(BLACK)
        .position(SeriesLabelPosition::UpperRight)
        .label_font(("sans-serif", 20))
        .draw()?;

    if let Some(((lon1, lat1), (lon2, lat2))) = cs_track {
        chart.draw_series(LineSeries::new(
            vec![(lon1, lat1), (lon2, lat2)],
            RGBColor(0, 0, 0).stroke_width(3),
        ))?;
    }

    root.present()?;
    Ok(())
}

pub fn generate_cross_section_viz<P: AsRef<Path>>(
    events: &[(EarthquakeEvent, f64, f64)],
    out_path: P,
) -> Result<(), Box<dyn std::error::Error>> {
    if events.is_empty() { return Ok(()); }

    let mut min_dist = f64::MAX;
    let mut max_dist = f64::MIN;
    let mut min_depth = f64::MAX;
    let mut max_depth = f64::MIN;

    for (_, along_track, _) in events {
        if *along_track < min_dist { min_dist = *along_track; }
        if *along_track > max_dist { max_dist = *along_track; }
    }
    
    for (ev, _, _) in events {
        if ev.depth_km < min_depth { min_depth = ev.depth_km; }
        if ev.depth_km > max_depth { max_depth = ev.depth_km; }
    }
    
    let dist_margin = (max_dist - min_dist) * 0.1;
    let depth_margin = (max_depth - min_depth) * 0.1;
    let dist_margin = if dist_margin == 0.0 { 1.0 } else { dist_margin };
    let depth_margin = if depth_margin == 0.0 { 1.0 } else { depth_margin };

    min_dist -= dist_margin; max_dist += dist_margin;
    
    // Extend depth bounds slightly (limit top depth to 0 if we exceed it upwards)
    min_depth = (min_depth - depth_margin).max(0.0);
    max_depth += depth_margin;
    
    let width = 1000;
    let height = 600;
    let root = BitMapBackend::new(&out_path, (width, height)).into_drawing_area();
    root.fill(&WHITE)?;

    let mut chart = ChartBuilder::on(&root)
        .caption(format!("Cross-Section ({} Events)", events.len()), ("sans-serif", 30).into_font())
        .margin(30)
        .x_label_area_size(50)
        .y_label_area_size(50)
        .build_cartesian_2d(min_dist..max_dist, max_depth..min_depth)?; // Invert Y axis for depth

    chart.configure_mesh()
        .x_desc("Distance along profile (km)")
        .y_desc("Depth (km)")
        .axis_desc_style(("sans-serif", 20).into_font())
        .draw()?;

    // Add legend markers
    chart.draw_series(std::iter::once(EmptyElement::at((0.0, 0.0))))?
        .label("Depth <= 70 km")
        .legend(|(x, y)| Circle::new((x, y), 5, RGBColor(220, 20, 60).filled()));
    
    chart.draw_series(std::iter::once(EmptyElement::at((0.0, 0.0))))?
        .label("Depth 70 - 300 km")
        .legend(|(x, y)| Circle::new((x, y), 5, RGBColor(34, 139, 34).filled()));

    chart.draw_series(std::iter::once(EmptyElement::at((0.0, 0.0))))?
        .label("Depth > 300 km")
        .legend(|(x, y)| Circle::new((x, y), 5, RGBColor(0, 0, 205).filled()));

    chart.draw_series(
        events.iter().map(|(ev, along, _)| {
            let color = if ev.depth_km <= 70.0 {
                RGBColor(220, 20, 60) // Crimson
            } else if ev.depth_km <= 300.0 {
                RGBColor(34, 139, 34) // ForestGreen
            } else {
                RGBColor(0, 0, 205)   // MediumBlue
            };
            
            let size = if ev.mag > 0.0 { (ev.mag * 1.5) as u32 } else { 2 };
            Circle::new((*along, ev.depth_km), size, color.filled())
        })
    )?;

    chart.configure_series_labels()
        .background_style(WHITE.mix(0.8))
        .border_style(BLACK)
        .position(SeriesLabelPosition::UpperRight)
        .label_font(("sans-serif", 20))
        .draw()?;

    root.present()?;
    Ok(())
}

