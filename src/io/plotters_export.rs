use plotters::prelude::*;
use crate::core::isc_client::{EarthquakeEvent, ConversionRule, apply_conversion};
use crate::core::rjmcmc_stats::VisualizerData;
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

pub fn generate_rjmcmc_viz<P: AsRef<Path>>(
    viz: &VisualizerData,
    out_path: P,
) -> Result<(), Box<dyn std::error::Error>> {
    let width = 1200;
    let height = 1400;
    let root = BitMapBackend::new(&out_path, (width, height)).into_drawing_area();
    root.fill(&WHITE)?;

    let title_font = ("sans-serif", 30).into_font();
    let root = root.titled("RJ-MCMC HVSR Inversion Results", title_font)?;

    let (main_area, cb_area) = root.split_vertically(height - 150);
    let grid = main_area.split_evenly((2, 2));
    
    // Some colors
    let cmap_color = |rmse: f64, min_r: f64, max_r: f64| -> RGBColor {
        let norm = if max_r > min_r { (rmse - min_r) / (max_r - min_r) } else { 0.0 };
        let r = 255;
        let g = (165.0 + norm * (255.0 - 165.0)) as u8;
        let b = (0.0 + norm * 100.0) as u8;
        RGBColor(r, g, b)
    };
    
    let min_rmse = viz.sorted_rmse.iter().copied().fold(f64::INFINITY, |a, b| a.min(b));
    let mut max_rmse = viz.sorted_rmse.iter().copied().fold(f64::NEG_INFINITY, |a, b| a.max(b));
    if max_rmse <= min_rmse {
        max_rmse = min_rmse + 0.1;
    }

    // --- 1. Top Left: HVSR Fits ---
    let mut hvsr_area = ChartBuilder::on(&grid[0])
        .caption("Posterior HVSR Fits", ("sans-serif", 25).into_font())
        .margin(30)
        .x_label_area_size(50)
        .y_label_area_size(60)
        .build_cartesian_2d(
            viz.freq.first().copied().unwrap_or(0.1)..viz.freq.last().copied().unwrap_or(20.0),
            0.0..viz.h_obs.iter().copied().fold(0.0, f64::max) * 1.5
        )?;

    hvsr_area.configure_mesh()
        .x_desc("Frequency (Hz)")
        .y_desc("H/V Amplitude")
        .x_label_style(("sans-serif", 18).into_font())
        .y_label_style(("sans-serif", 18).into_font())
        .axis_desc_style(("sans-serif", 22).into_font())
        .draw()?;

    let step = (viz.samples.len() / 500).max(1);
    for m in viz.samples.iter().step_by(step) {
        let pts: Vec<_> = viz.freq.iter().zip(&m.h_syn).map(|(&x, &y)| (x, y)).collect();
        hvsr_area.draw_series(LineSeries::new(pts, cmap_color(m.rmse, min_rmse, max_rmse).mix(0.5)))?;
    }
    
    if let Some(best) = &viz.best_sample {
        let pts: Vec<_> = viz.freq.iter().zip(&best.h_syn).map(|(&x, &y)| (x, y)).collect();
        hvsr_area.draw_series(LineSeries::new(pts, BLACK.stroke_width(2)))?.label("Best Model").legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], BLACK.stroke_width(2)));
    }
    
    let obs_pts: Vec<_> = viz.freq.iter().zip(&viz.h_obs).map(|(&x, &y)| (x, y)).collect();
    // Plot error bars for observed if they exist
    for (i, (&x, &y)) in viz.freq.iter().zip(&viz.h_obs).enumerate() {
        let err = viz.h_err.get(i).copied().unwrap_or(0.0);
        if err > 0.0 {
            hvsr_area.draw_series(std::iter::once(PathElement::new(
                vec![(x, (y - err).max(0.0)), (x, y + err)],
                BLACK,
            )))?;
        }
    }
    hvsr_area.draw_series(PointSeries::of_element(
        obs_pts,
        3,
        &BLACK,
        &|c, s, st| {
            return EmptyElement::at(c)
                + Circle::new((0, 0), s, st.filled());
        },
    ))?.label("Observed").legend(|(x, y)| Circle::new((x + 10, y), 3, BLACK.filled()));
    
    hvsr_area.configure_series_labels()
        .position(SeriesLabelPosition::UpperRight)
        .label_font(("sans-serif", 18).into_font())
        .draw()?;

    // --- 2. Top Right: Vs(z) Profiles ---
    let max_v = viz.vs_p95.iter().copied().fold(0.0, f64::max).max(viz.vs30_max).max(1000.0) * 1.2;
    let mut vsz_area = ChartBuilder::on(&grid[1])
        .caption("Posterior Vs Profiles", ("sans-serif", 25).into_font())
        .margin(30)
        .x_label_area_size(50)
        .y_label_area_size(60)
        .build_cartesian_2d(
            0.0..max_v,
            (viz.max_z.min(3000.0))..0.0, // Inverted Y-axis
        )?;

    vsz_area.configure_mesh()
        .x_desc("Vs (m/s)")
        .y_desc("Depth (m)")
        .x_label_style(("sans-serif", 18).into_font())
        .y_label_style(("sans-serif", 18).into_font())
        .axis_desc_style(("sans-serif", 22).into_font())
        .draw()?;

    let step = (viz.samples.len() / 500).max(1);
    for m in viz.samples.iter().step_by(step) {
        let mut pts = Vec::new();
        let mut z_sum = 0.0;
        pts.push((m.vs[0], 0.0));
        for i in 0..m.h.len() {
            z_sum += m.h[i];
            pts.push((m.vs[i], z_sum));
            pts.push((m.vs[i+1], z_sum));
        }
        let last_z = z_sum + 20.0;
        pts.push((m.vs.last().copied().unwrap_or(0.0), last_z));
        vsz_area.draw_series(LineSeries::new(pts, cmap_color(m.rmse, min_rmse, max_rmse).mix(0.5)))?;
    }

    // Draw polygon for credible interval (filling between P05 and P95)
    let mut poly_pts = Vec::new();
    for (j, &zz) in viz.z_nodes.iter().enumerate() {
        poly_pts.push((viz.vs_p05[j], zz));
    }
    for (j, &zz) in viz.z_nodes.iter().enumerate().rev() {
        poly_pts.push((viz.vs_p95[j], zz));
    }
    vsz_area.draw_series(std::iter::once(Polygon::new(poly_pts, BLUE.mix(0.2))))?
        .label("90% Credible Interval").legend(|(x, y)| Rectangle::new([(x, y - 5), (x + 20, y + 5)], BLUE.mix(0.2).filled()));

    // Draw Median
    let mut med_pts = Vec::new();
    for (j, &zz) in viz.z_nodes.iter().enumerate() {
        med_pts.push((viz.vs_p50[j], zz));
    }
    vsz_area.draw_series(LineSeries::new(med_pts, BLUE.stroke_width(2)))?
        .label("Median").legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], BLUE.stroke_width(2)));

    // Draw Best Model Step
    if let Some(m) = &viz.best_sample {
        let mut best_pts = Vec::new();
        let mut z_sum = 0.0;
        best_pts.push((m.vs[0], 0.0));
        for i in 0..m.h.len() {
            z_sum += m.h[i];
            best_pts.push((m.vs[i], z_sum));
            best_pts.push((m.vs[i+1], z_sum));
        }
        let last_z = z_sum + 20.0;
        best_pts.push((m.vs.last().copied().unwrap_or(0.0), last_z));
        vsz_area.draw_series(LineSeries::new(best_pts, BLACK.stroke_width(2)))?
            .label(format!("Best Model ({:.2})", m.rmse)).legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], BLACK.stroke_width(2)));
    }

    // Add Markers (Vs30, H800, etc) with error bars
    let draw_marker = |area: &mut ChartContext<BitMapBackend, Cartesian2d<plotters::coord::types::RangedCoordf64, plotters::coord::types::RangedCoordf64>>, 
                       x_val: f64, x_min: f64, x_max: f64, 
                       y_val: f64, y_min: f64, y_max: f64, 
                       color: RGBColor, label: &str, is_vertical: bool| {
        if x_val.is_nan() || y_val.is_nan() { return Ok(()); }
        
        if is_vertical {
            let _ = area.draw_series(std::iter::once(PathElement::new(vec![(x_val, y_min), (x_val, y_max)], color.stroke_width(2))));
        } else {
            let _ = area.draw_series(std::iter::once(PathElement::new(vec![(x_min, y_val), (x_max, y_val)], color.stroke_width(2))));
        }
        
        let _ = area.draw_series(PointSeries::of_element(
            vec![(x_val, y_val)],
            4,
            &color,
            &|c, s, st| {
                return EmptyElement::at(c)
                    + Circle::new((0, 0), s, st.filled());
            },
        ))?.label(label).legend(move |(x, y)| Circle::new((x + 10, y), 4, color.filled()));
        Ok::<(), Box<dyn std::error::Error>>(())
    };

    draw_marker(&mut vsz_area, viz.vs30_mean, viz.vs30_min, viz.vs30_max, 30.0, 30.0, 30.0, RED, &format!("Vs30={:.0} ({:.0}-{:.0})", viz.vs30_mean, viz.vs30_min, viz.vs30_max), false)?;
    draw_marker(&mut vsz_area, 800.0, 800.0, 800.0, viz.h800_mean, viz.h800_min, viz.h800_max, GREEN, &format!("H800={:.1} ({:.1}-{:.1})", viz.h800_mean, viz.h800_min, viz.h800_max), true)?;
    draw_marker(&mut vsz_area, 1000.0, 1000.0, 1000.0, viz.z1_mean, viz.z1_min, viz.z1_max, CYAN, &format!("Z1.0={:.1} ({:.1}-{:.1})", viz.z1_mean, viz.z1_min, viz.z1_max), true)?;
    draw_marker(&mut vsz_area, 2500.0, 2500.0, 2500.0, viz.z2_5_mean, viz.z2_5_min, viz.z2_5_max, MAGENTA, &format!("Z2.5={:.1} ({:.1}-{:.1})", viz.z2_5_mean, viz.z2_5_min, viz.z2_5_max), true)?;

    vsz_area.configure_series_labels()
        .position(SeriesLabelPosition::LowerLeft)
        .label_font(("sans-serif", 16).into_font())
        .draw()?;

    // --- 3. Bottom Left: RMSE Histogram ---
    let mut bins = vec![0; 30];
    let bin_width = if max_rmse > min_rmse { (max_rmse - min_rmse) / 30.0 } else { 0.1 };
    for &r in &viz.sorted_rmse {
        let mut idx = if bin_width > 0.0 { ((r - min_rmse) / bin_width) as usize } else { 0 };
        if idx >= 30 { idx = 29; }
        bins[idx] += 1;
    }
    let max_hist = bins.iter().copied().max().unwrap_or(0) as f64;

    let mut hist_area = ChartBuilder::on(&grid[2])
        .caption("RMSE Histogram", ("sans-serif", 25).into_font())
        .margin(30)
        .x_label_area_size(50)
        .y_label_area_size(60)
        .build_cartesian_2d(
            min_rmse..max_rmse,
            0.0..max_hist * 1.1
        )?;

    hist_area.configure_mesh()
        .x_desc("RMSE")
        .y_desc("Count")
        .x_label_style(("sans-serif", 18).into_font())
        .y_label_style(("sans-serif", 18).into_font())
        .axis_desc_style(("sans-serif", 22).into_font())
        .draw()?;

    hist_area.draw_series(
        bins.iter().enumerate().map(|(i, &count)| {
            let x0 = min_rmse + i as f64 * bin_width;
            let x1 = x0 + bin_width;
            Rectangle::new([(x0, 0.0), (x1, count as f64)], RGBColor(255, 140, 0).filled())
        })
    )?;

    // --- 4. Bottom Right: RMSE vs Index (Dual Axis for Layers) ---
    let max_idx = viz.sorted_rmse.len() as f64;
    let max_layers = viz.samples.iter().map(|s| s.n_layers).max().unwrap_or(0) as f64;
    let min_layers = viz.samples.iter().map(|s| s.n_layers).min().unwrap_or(0) as f64;

    let mut rmse_area = ChartBuilder::on(&grid[3])
        .caption("RMSE and Layers vs Valid Index", ("sans-serif", 25).into_font())
        .margin(30)
        .x_label_area_size(50)
        .y_label_area_size(60)
        .right_y_label_area_size(60)
        .build_cartesian_2d(
            0.0..max_idx,
            min_rmse..(max_rmse * 1.1).max(min_rmse + 0.1)
        )?
        .set_secondary_coord(
            0.0..max_idx,
            (min_layers - 1.0).max(0.0)..(max_layers + 1.0)
        );

    rmse_area.configure_mesh()
        .x_desc("Sorted Sample Index")
        .y_desc("RMSE")
        .x_label_style(("sans-serif", 18).into_font())
        .y_label_style(("sans-serif", 18).into_font())
        .axis_desc_style(("sans-serif", 22).into_font())
        .draw()?;
        
    rmse_area.configure_secondary_axes()
        .y_desc("Number of Layers")
        .label_style(("sans-serif", 18).into_font())
        .axis_desc_style(("sans-serif", 22).into_font())
        .draw()?;

    let purple = RGBColor(128, 0, 128);
    let orange = RGBColor(255, 165, 0);

    let rmse_pts: Vec<_> = viz.sorted_rmse.iter().enumerate().map(|(i, &r)| (i as f64, r)).collect();
    rmse_area.draw_series(LineSeries::new(rmse_pts, purple.stroke_width(2)))?
        .label("RMSE").legend(move |(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], purple.stroke_width(2)));

    let layers_pts: Vec<_> = viz.samples.iter().enumerate().map(|(i, s)| (i as f64, s.n_layers as f64)).collect();
    rmse_area.draw_secondary_series(LineSeries::new(layers_pts, orange.stroke_width(1)))?
        .label("Layers").legend(move |(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], orange.stroke_width(1)));

    rmse_area.configure_series_labels()
        .position(SeriesLabelPosition::UpperRight)
        .label_font(("sans-serif", 18).into_font())
        .draw()?;

    let mut cb_chart = ChartBuilder::on(&cb_area)
        .caption("RMSE Colormap", ("sans-serif", 24).into_font())
        .margin_top(15)
        .margin_bottom(15)
        .margin_left(150)
        .margin_right(150)
        .x_label_area_size(40)
        .build_cartesian_2d(min_rmse..max_rmse, 0.0..1.0)?;
        
    cb_chart.configure_mesh()
        .disable_y_mesh()
        .disable_y_axis()
        .x_label_style(("sans-serif", 18).into_font())
        .x_label_formatter(&|v: &f64| format!("{:.4}", v))
        .draw()?;
        
    let steps = 100;
    let step_w = (max_rmse - min_rmse) / steps as f64;
    for i in 0..steps {
        let r0 = min_rmse + i as f64 * step_w;
        let r1 = r0 + step_w;
        cb_chart.draw_series(std::iter::once(
            Rectangle::new([(r0, 0.0), (r1, 1.0)], cmap_color(r0, min_rmse, max_rmse).filled())
        ))?;
    }

    root.present()?;
    Ok(())
}

