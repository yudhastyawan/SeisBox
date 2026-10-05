use rustfft::{FftPlanner, num_complex::Complex};

#[derive(Clone)]
pub struct HvtfaScatterPoint {
    pub time: f64,
    pub freq: f64,
    pub hv_neg: f64,
    pub hv_pos: f64,
}

#[derive(Clone)]
pub struct HvtfaPickedCurve {
    pub freq: f64,
    pub hv_mode: f64,
    pub hv_std_log: f64,
}

#[derive(Clone)]
pub struct HvtfaResult {
    pub scatter_points: Vec<HvtfaScatterPoint>,
    pub picked_curve: Vec<HvtfaPickedCurve>,
}

fn find_local_maxima(x: &[f64]) -> Vec<usize> {
    let mut maxima = Vec::new();
    for i in 1..(x.len() - 1) {
        if x[i] > x[i - 1] && x[i] > x[i + 1] {
            maxima.push(i);
        }
    }
    maxima
}

pub fn run_hvtfa(
    z: &[f64],
    n: &[f64],
    e: &[f64],
    fs: f64,
    f_min: f64,
    f_max: f64,
    n_freqs: usize,
    m_param: f64,
    tx: Option<std::sync::mpsc::Sender<(f32, String)>>
) -> HvtfaResult {
    let n_samples = z.len().min(n.len()).min(e.len());
    let dt = 1.0 / fs;

    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(n_samples);
    let ifft = planner.plan_fft_inverse(n_samples);

    let mut z_complex: Vec<Complex<f64>> = z[..n_samples].iter().map(|&v| Complex::new(v, 0.0)).collect();
    let mut n_complex: Vec<Complex<f64>> = n[..n_samples].iter().map(|&v| Complex::new(v, 0.0)).collect();
    let mut e_complex: Vec<Complex<f64>> = e[..n_samples].iter().map(|&v| Complex::new(v, 0.0)).collect();

    fft.process(&mut z_complex);
    fft.process(&mut n_complex);
    fft.process(&mut e_complex);

    let freqs: Vec<f64> = (0..n_samples)
        .map(|i| {
            if i <= n_samples / 2 {
                i as f64 * fs / n_samples as f64
            } else {
                (i as f64 - n_samples as f64) * fs / n_samples as f64
            }
        })
        .collect();

    // target freqs (log scale)
    let mut target_freqs = Vec::with_capacity(n_freqs);
    let log_min = f_min.ln();
    let log_max = f_max.ln();
    for i in 0..n_freqs {
        let f = if n_freqs > 1 {
            (log_min + i as f64 * (log_max - log_min) / (n_freqs as f64 - 1.0)).exp()
        } else {
            f_min
        };
        target_freqs.push(f);
    }

    let mut scatter_points = Vec::new();

    for (idx, &fi) in target_freqs.iter().enumerate() {
        if let Some(sender) = &tx {
            let pct = idx as f32 / n_freqs as f32;
            let _ = sender.send((pct, format!("HVTFA freq: {:.2} Hz", fi)));
        }

        let mut filter = vec![Complex::new(0.0, 0.0); n_samples];
        for i in 0..n_samples {
            if freqs[i] > 0.0 {
                let a = 6.0 * (freqs[i] / fi - 1.0);
                let a2 = a * a;
                if a2 < 700.0 {
                    let val = std::f64::consts::PI.powf(-0.25) * (-a2 * m_param).exp();
                    filter[i] = Complex::new(val, 0.0);
                }
            }
        }

        let mut cwt_z = vec![Complex::new(0.0, 0.0); n_samples];
        let mut cwt_n = vec![Complex::new(0.0, 0.0); n_samples];
        let mut cwt_e = vec![Complex::new(0.0, 0.0); n_samples];

        for i in 0..n_samples {
            cwt_z[i] = z_complex[i] * filter[i];
            cwt_n[i] = n_complex[i] * filter[i];
            cwt_e[i] = e_complex[i] * filter[i];
        }

        ifft.process(&mut cwt_z);
        ifft.process(&mut cwt_n);
        ifft.process(&mut cwt_e);

        // Normalize by n_samples
        let norm = 1.0 / n_samples as f64;
        let mut env_z = vec![0.0; n_samples];
        let mut env_h = vec![0.0; n_samples];

        for i in 0..n_samples {
            let ez = (cwt_z[i] * norm).norm() * 2.0;
            let en = (cwt_n[i] * norm).norm() * 2.0;
            let ee = (cwt_e[i] * norm).norm() * 2.0;
            env_z[i] = ez;
            env_h[i] = (en * en + ee * ee).sqrt();
        }

        let maxima_idx = find_local_maxima(&env_z);
        let rayleigh_delay = (0.25 * fs / fi).round() as usize;
        let delta_t_cone = 6.0 / (2.0 * std::f64::consts::PI * fi) * m_param.sqrt();
        let cone_idx = (3.0 * delta_t_cone * fs).round() as usize;
        let margin = cone_idx.max(rayleigh_delay);

        for &idx in &maxima_idx {
            if idx > margin && idx < n_samples - margin - 1 {
                let v_max = env_z[idx];
                if v_max > 0.0 {
                    let h_neg = env_h[idx - rayleigh_delay];
                    let h_pos = env_h[idx + rayleigh_delay];
                    scatter_points.push(HvtfaScatterPoint {
                        time: idx as f64 * dt,
                        freq: fi,
                        hv_neg: h_neg / v_max,
                        hv_pos: h_pos / v_max,
                    });
                }
            }
        }
    }

    // Density Peak Picking
    let mut picked_curve = Vec::new();
    for &fi in &target_freqs {
        let hvs: Vec<f64> = scatter_points
            .iter()
            .filter(|p| (p.freq - fi).abs() < 1e-6)
            .flat_map(|p| vec![p.hv_neg, p.hv_pos])
            .filter(|&v| v > 0.001 && v < 1000.0)
            .collect();

        if hvs.len() < 5 {
            continue;
        }

        let log_hvs: Vec<f64> = hvs.iter().map(|&v| v.log10()).collect();
        let min_val = log_hvs.iter().copied().fold(f64::INFINITY, f64::min);
        let max_val = log_hvs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        
        let bins = 50;
        if max_val - min_val < 1e-6 {
            continue;
        }

        let step = (max_val - min_val) / bins as f64;
        let mut hist = vec![0; bins];
        for &v in &log_hvs {
            let mut idx = ((v - min_val) / step).floor() as usize;
            if idx >= bins {
                idx = bins - 1;
            }
            hist[idx] += 1;
        }

        let mut smooth_hist = vec![0.0; bins];
        for i in 0..bins {
            let left = if i > 0 { hist[i - 1] as f64 } else { 0.0 };
            let right = if i < bins - 1 { hist[i + 1] as f64 } else { 0.0 };
            smooth_hist[i] = (left + hist[i] as f64 * 2.0 + right) / 4.0;
        }

        let mut max_count = -1.0;
        let mut mode_idx = 0;
        for i in 0..bins {
            if smooth_hist[i] > max_count {
                max_count = smooth_hist[i];
                mode_idx = i;
            }
        }

        let mode_log = min_val + (mode_idx as f64 + 0.5) * step;
        let mean_log = log_hvs.iter().sum::<f64>() / log_hvs.len() as f64;
        let mut sum_sq = 0.0;
        for &v in &log_hvs {
            sum_sq += (v - mean_log).powi(2);
        }
        let std_log = (sum_sq / log_hvs.len() as f64).sqrt();

        picked_curve.push(HvtfaPickedCurve {
            freq: fi,
            hv_mode: 10.0_f64.powf(mode_log),
            hv_std_log: std_log,
        });
    }

    HvtfaResult {
        scatter_points,
        picked_curve,
    }
}
