use crate::hvf::model::Layer;
use plotters::prelude::*;
use std::path::Path;

pub fn plot_hv_fit(
    obs_freqs: &[f64],
    obs_hvs: &[f64],
    est_hvs: &[f64],
    out_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = BitMapBackend::new(out_path, (800, 600)).into_drawing_area();
    root.fill(&WHITE)?;

    let min_f = obs_freqs.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_f = obs_freqs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    
    let min_h_obs = obs_hvs.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_h_obs = obs_hvs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let min_h_est = est_hvs.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_h_est = est_hvs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    
    let min_h = min_h_obs.min(min_h_est) * 0.9;
    let max_h = max_h_obs.max(max_h_est) * 1.1;

    let mut chart = ChartBuilder::on(&root)
        .caption("H/V Curve Fitting", ("sans-serif", 30).into_font())
        .margin(10)
        .x_label_area_size(40)
        .y_label_area_size(50)
        .build_cartesian_2d((min_f..max_f).log_scale(), min_h..max_h)?;

    chart
        .configure_mesh()
        .x_desc("Frequency (Hz)")
        .y_desc("H/V Amplitude")
        .draw()?;

    chart
        .draw_series(
            obs_freqs
                .iter()
                .zip(obs_hvs.iter())
                .map(|(x, y)| Circle::new((*x, *y), 3, BLACK.filled())),
        )?
        .label("Observation")
        .legend(|(x, y)| Circle::new((x + 10, y), 3, BLACK.filled()));

    chart
        .draw_series(LineSeries::new(
            obs_freqs.iter().zip(est_hvs.iter()).map(|(x, y)| (*x, *y)),
            &RED,
        ))?
        .label("Best Model")
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], &RED));

    chart
        .configure_series_labels()
        .background_style(&WHITE.mix(0.8))
        .border_style(&BLACK)
        .draw()?;

    root.present()?;
    Ok(())
}

pub fn plot_vs_profile(
    layers: &[Layer],
    out_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = BitMapBackend::new(out_path, (600, 800)).into_drawing_area();
    root.fill(&WHITE)?;

    let mut depth = 0.0;
    let mut vs_points = Vec::new();
    
    let max_vs = layers.iter().map(|l| l.vs).fold(0.0_f64, f64::max) * 1.2;
    
    // Default halfspace thickness for plotting if it's 0
    let mut total_depth = 0.0;
    for l in layers.iter().take(layers.len().saturating_sub(1)) {
        total_depth += l.thickness;
    }
    let halfspace_depth = if total_depth == 0.0 { 100.0 } else { total_depth * 0.2 };

    for l in layers {
        vs_points.push((l.vs, depth));
        let h = if l.thickness > 0.0 { l.thickness } else { halfspace_depth };
        depth += h;
        vs_points.push((l.vs, depth));
    }
    
    let max_depth = depth;

    let mut chart = ChartBuilder::on(&root)
        .caption("1D Vs Profile", ("sans-serif", 30).into_font())
        .margin(10)
        .x_label_area_size(40)
        .y_label_area_size(50)
        // Y-axis is inverted (depth goes down)
        .build_cartesian_2d(0.0..max_vs, max_depth..0.0)?;

    chart
        .configure_mesh()
        .x_desc("Vs (m/s)")
        .y_desc("Depth (m)")
        .draw()?;

    chart.draw_series(LineSeries::new(vs_points, BLUE.stroke_width(3)))?;

    root.present()?;
    Ok(())
}

pub fn plot_convergence(
    history: &[f64],
    out_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    if history.is_empty() {
        return Ok(());
    }
    
    let root = BitMapBackend::new(out_path, (800, 400)).into_drawing_area();
    root.fill(&WHITE)?;

    let max_iter = history.len() as f64;
    let min_cost = history.iter().cloned().fold(f64::INFINITY, f64::min) * 0.9;
    let max_cost = history.iter().cloned().fold(f64::NEG_INFINITY, f64::max) * 1.1;

    let mut chart = ChartBuilder::on(&root)
        .caption("Inversion Convergence", ("sans-serif", 30).into_font())
        .margin(10)
        .x_label_area_size(40)
        .y_label_area_size(50)
        .build_cartesian_2d(0.0..max_iter, min_cost..max_cost)?;

    chart
        .configure_mesh()
        .x_desc("Iteration")
        .y_desc("Cost (Misfit)")
        .draw()?;

    chart.draw_series(LineSeries::new(
        history.iter().enumerate().map(|(i, &c)| (i as f64, c)),
        &GREEN,
    ))?;

    root.present()?;
    Ok(())
}
