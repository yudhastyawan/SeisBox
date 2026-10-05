use plotters::prelude::*;
use std::path::Path;
use crate::core::math_hvsr::HvsrStats;

/// Ekspor Kurva H/V ke PNG.
pub fn export_hvsr_curve(
    filepath: &Path,
    freqs: &[f64],
    stats: &HvsrStats,
    all_hvsr: &[Vec<f64>],
    final_indices: &[usize],
    is_dfa: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = BitMapBackend::new(filepath, (1200, 800)).into_drawing_area();
    root.fill(&WHITE)?;

    let min_freq = freqs.first().copied().unwrap_or(0.1).max(0.01);
    let max_freq = freqs.last().copied().unwrap_or(20.0);
    
    let mut min_y = 0.1;
    let mut max_y = 10.0;
    
    for &j in final_indices {
        for &amp in &all_hvsr[j] {
            if amp > max_y { max_y = amp; }
            if amp < min_y && amp > 0.01 { min_y = amp; }
        }
    }
    // Cap max_y if it's crazy high
    max_y = max_y.min(100.0);

    let mut chart = ChartBuilder::on(&root)
        .caption("HVSR Curve", ("sans-serif", 30).into_font())
        .margin(20)
        .x_label_area_size(40)
        .y_label_area_size(40)
        .build_cartesian_2d((min_freq..max_freq).log_scale(), (min_y..max_y).log_scale())?;

    chart
        .configure_mesh()
        .x_desc("Frequency (Hz)")
        .y_desc("H/V Ratio")
        .draw()?;

    // Gambar kurva tiap window (abu-abu tipis)
    for &j in final_indices {
        let win_curve = &all_hvsr[j];
        chart.draw_series(LineSeries::new(
            freqs.iter().zip(win_curve.iter()).map(|(&f, &y)| (f, y)),
            &RGBColor(200, 200, 200).mix(0.5),
        ))?;
    }

    // Gambar area std dev kalau BUKAN DFA
    if !is_dfa {
        let std_area: Vec<_> = freqs.iter().zip(stats.std_plus.iter().zip(stats.std_minus.iter()))
            .map(|(&f, (&sp, &sm))| (f, sm.max(min_y), sp.min(max_y)))
            .collect();

        // Gambar area std dev menggunakan satu Polygon (agar bawahnya transparan)
        let mut poly_points = Vec::new();
        // Garis atas (forward)
        for &(f, _sm, sp) in &std_area {
            poly_points.push((f, sp));
        }
        // Garis bawah (backward)
        for &(f, sm, _sp) in std_area.iter().rev() {
            poly_points.push((f, sm));
        }

        chart.draw_series(std::iter::once(
            Polygon::new(poly_points, BLUE.mix(0.1).filled())
        ))?;
        
        // Garis batas std dev
        chart.draw_series(LineSeries::new(
            freqs.iter().zip(stats.std_plus.iter()).map(|(&f, &y)| (f, y.min(max_y))),
            &BLUE.mix(0.5),
        ))?;
        chart.draw_series(LineSeries::new(
            freqs.iter().zip(stats.std_minus.iter()).map(|(&f, &y)| (f, y.max(min_y))),
            &BLUE.mix(0.5),
        ))?;
    }

    // Gambar kurva rata-rata (biru tebal)
    chart.draw_series(LineSeries::new(
        freqs.iter().zip(stats.mean_hvsr.iter()).map(|(&f, &y)| (f, y)),
        BLUE.stroke_width(3),
    ))?.label(if is_dfa { "DFA Ratio" } else { "Mean H/V" })
     .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], BLUE.stroke_width(3)));

    // Cari titik f0 dan A0 tertinggi dari kurva mean
    let mut mean_max_amp = -1.0;
    let mut mean_peak_f0 = 0.0;
    for (i, &amp) in stats.mean_hvsr.iter().enumerate() {
        if amp > mean_max_amp {
            mean_max_amp = amp;
            mean_peak_f0 = freqs[i];
        }
    }

    // Gambar titik f0 untuk tiap window individual
    let mut individual_f0_points = Vec::new();
    for &j in final_indices {
        let win_curve = &all_hvsr[j];
        let mut max_a = -1.0;
        let mut f0_i = 0.0;
        for (i, &a) in win_curve.iter().enumerate() {
            if a > max_a {
                max_a = a;
                f0_i = freqs[i];
            }
        }
        if max_a > 0.0 {
            individual_f0_points.push(Circle::new((f0_i, max_a.min(max_y)), 2, RED.mix(0.3).filled()));
        }
    }
    chart.draw_series(individual_f0_points)?;

    // Gambar marker f0 rata-rata (f0 dari rata-rata puncak tiap window)
    let f0_avg = stats.f0_mean;
    let f0_std = stats.f0_std;
    
    if f0_avg > 0.0 {
        // Crosshair f0_avg
        chart.draw_series(std::iter::once(
            Cross::new((f0_avg, mean_max_amp), 10, RED.stroke_width(2))
        ))?.label(format!("f0_avg = {:.2} ± {:.2} Hz", f0_avg, f0_std))
         .legend(move |(x, y)| Cross::new((x, y), 5, RED.stroke_width(2)));
         
        // Garis vertikal putus-putus untuk f0_avg
        chart.draw_series(LineSeries::new(
            vec![(f0_avg, min_y), (f0_avg, max_y)],
            RED.mix(0.5).stroke_width(1),
        ))?;
        
        // Garis vertikal untuk batas f0_avg ± f0_std
        if f0_std > 0.0 {
            chart.draw_series(LineSeries::new(
                vec![(f0_avg - f0_std, min_y), (f0_avg - f0_std, max_y)],
                RED.mix(0.3).stroke_width(1),
            ))?;
            chart.draw_series(LineSeries::new(
                vec![(f0_avg + f0_std, min_y), (f0_avg + f0_std, max_y)],
                RED.mix(0.3).stroke_width(1),
            ))?;
        }
    }

    // Info peak dari mean curve (A0 dan f0 dari garis biru tebal)
    chart.draw_series(std::iter::once(
        EmptyElement::at((mean_peak_f0, mean_max_amp))
    ))?.label(format!("Mean Curve Peak: f0 = {:.2} Hz, A0 = {:.2}", mean_peak_f0, mean_max_amp));

    chart.configure_series_labels()
        .position(SeriesLabelPosition::UpperRight)
        .background_style(&WHITE.mix(0.8))
        .border_style(&BLACK)
        .draw()?;

    root.present()?;
    Ok(())
}

/// Ekspor Window Selection ke PNG
pub fn export_hvsr_windows(
    filepath: &Path,
    z_data: &[f64],
    n_data: &[f64],
    e_data: &[f64],
    sr: f64,
    window_starts_s: &[f64],
    window_len_s: f64,
    final_valid_idx: &[bool],
) -> Result<(), Box<dyn std::error::Error>> {
    let root = BitMapBackend::new(filepath, (1200, 900)).into_drawing_area();
    root.fill(&WHITE)?;
    
    let total_s = z_data.len() as f64 / sr;
    
    let areas = root.split_evenly((3, 1));
    let titles = ["Z Component", "N Component", "E Component"];
    let datasets = [z_data, n_data, e_data];
    
    for (i, area) in areas.into_iter().enumerate() {
        let data = datasets[i];
        
        let mut min_val = f64::MAX;
        let mut max_val = f64::MIN;
        
        // Sub-sample to find min max to avoid searching 1 million points if not necessary
        for &v in data.iter().step_by(10) {
            if v < min_val { min_val = v; }
            if v > max_val { max_val = v; }
        }
        
        if max_val <= min_val {
            max_val = min_val + 1.0;
        }

        let mut chart = ChartBuilder::on(&area)
            .caption(titles[i], ("sans-serif", 20).into_font())
            .margin(10)
            .x_label_area_size(30)
            .y_label_area_size(40)
            .build_cartesian_2d(0f64..total_s, min_val..max_val)?;

        chart.configure_mesh()
            .x_desc("Time (s)")
            .y_desc("Amplitude")
            .draw()?;

        // Gambar blok windows
        for (w_idx, &start_time) in window_starts_s.iter().enumerate() {
            let is_valid = final_valid_idx[w_idx];
            let end_time = start_time + window_len_s;
            let color = if is_valid {
                &GREEN.mix(0.15)
            } else {
                &RED.mix(0.1)
            };
            
            chart.draw_series(std::iter::once(
                Rectangle::new([(start_time, min_val), (end_time, max_val)], color.filled())
            ))?;
        }

        // Gambar data downsampled (max 5000 points per plot to prevent massive PNGs)
        let n_points = data.len();
        let step = (n_points / 5000).max(1);
        
        let line_data = (0..n_points).step_by(step).map(|idx| {
            let t = idx as f64 / sr;
            (t, data[idx])
        });

        chart.draw_series(LineSeries::new(line_data, &BLACK.mix(0.5)))?;
    }

    root.present()?;
    Ok(())
}

/// Ekspor grafik HVTFA (Scatter + Kurva mode).
pub fn export_hvtfa_plot(
    filepath: &Path,
    f_min: f64,
    f_max: f64,
    scatter_points: &[crate::core::math_hvtfa::HvtfaScatterPoint],
    picked_curve: &[crate::core::math_hvtfa::HvtfaPickedCurve],
) -> Result<(), Box<dyn std::error::Error>> {
    let root = BitMapBackend::new(filepath, (1200, 800)).into_drawing_area();
    root.fill(&WHITE)?;

    let min_y = 0.1;
    let max_y = 50.0;

    let mut chart = ChartBuilder::on(&root)
        .caption("HVTFA Scatter Plot & Picked Curve", ("sans-serif", 30).into_font())
        .margin(20)
        .x_label_area_size(50)
        .y_label_area_size(50)
        .build_cartesian_2d((f_min..f_max).log_scale(), (min_y..max_y).log_scale())?;

    chart.configure_mesh()
        .x_desc("Frequency (Hz)")
        .y_desc("H/V Ratio (Rayleigh Ellipticity)")
        .draw()?;

    let points = scatter_points.iter()
        .filter(|p| p.hv_neg > min_y && p.hv_neg < max_y && p.hv_pos > min_y && p.hv_pos < max_y)
        .flat_map(|p| vec![
            Circle::new((p.freq, p.hv_neg), 1, BLUE.mix(0.02).filled()),
            Circle::new((p.freq, p.hv_pos), 1, BLUE.mix(0.02).filled())
        ]);
        
    chart.draw_series(points)?;

    if !picked_curve.is_empty() {
        chart.draw_series(LineSeries::new(
            picked_curve.iter().map(|p| (p.freq, p.hv_mode)),
            RED.stroke_width(3),
        ))?.label("Picked Curve (Mode)")
         .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], RED.stroke_width(3)));

        let mut std_up = Vec::new();
        let mut std_dn = Vec::new();
        for p in picked_curve {
            let mode_log = p.hv_mode.log10();
            let up = 10.0_f64.powf(mode_log + p.hv_std_log);
            let dn = 10.0_f64.powf(mode_log - p.hv_std_log);
            std_up.push((p.freq, up.min(max_y)));
            std_dn.push((p.freq, dn.max(min_y)));
        }

        chart.draw_series(LineSeries::new(
            std_up.iter().copied(),
            RED.mix(0.5).stroke_width(1),
        ))?.label("±1 StdDev")
         .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], RED.mix(0.5).stroke_width(1)));

        chart.draw_series(LineSeries::new(
            std_dn.iter().copied(),
            RED.mix(0.5).stroke_width(1),
        ))?;
    }

    chart.configure_series_labels()
        .position(SeriesLabelPosition::UpperRight)
        .background_style(&WHITE.mix(0.8))
        .border_style(&BLACK)
        .draw()?;

    root.present()?;
    Ok(())
}
