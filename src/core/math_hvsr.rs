use rustfft::{FftPlanner, num_complex::Complex};
use std::f64::consts::PI;
use std::sync::mpsc::Sender;

#[derive(Clone, Debug, Copy)]
pub struct HvsrParams {
    pub window_len_s: f64,
    pub overlap_pct: f64,
    pub sta_len_s: f64,
    pub lta_len_s: f64,
    pub t1: f64,
    pub t2: f64,
    pub b_value: f64,
    pub freq_min: f64,
    pub freq_max: f64,
    pub freq_count: usize,
    pub combine_method: HorizontalCombineMethod,
    pub enable_f0_filter: bool,
    pub f0_filter_n: f64,
}

#[derive(Clone, Debug, Copy, PartialEq)]
pub enum HorizontalCombineMethod {
    Geometric,
    Quadratic,
    Arithmetic,
    Maximum,
}

impl Default for HorizontalCombineMethod {
    fn default() -> Self {
        HorizontalCombineMethod::Geometric
    }
}

impl Default for HvsrParams {
    fn default() -> Self {
        Self {
            window_len_s: 40.0,
            overlap_pct: 50.0, // percent
            sta_len_s: 1.0,
            lta_len_s: 30.0,
            t1: 0.2, // typically lower bound around 0.2
            t2: 2.5, // typically upper bound around 2.5
            b_value: 40.0,
            freq_min: 0.1,
            freq_max: 20.0,
            freq_count: 100,
            combine_method: HorizontalCombineMethod::Geometric,
            enable_f0_filter: false,
            f0_filter_n: 2.0,
        }
    }
}

pub struct HvsrStats {
    pub mean_hvsr: Vec<f64>,
    pub std_plus: Vec<f64>,
    pub std_minus: Vec<f64>,
    pub valid_indices: Vec<usize>,
    pub f0_mean: f64,
    pub f0_std: f64,
}

pub struct HvsrResult {
    pub freq: Vec<f64>,
    pub all_hvsr: Vec<Vec<f64>>,
    pub raw_stats: HvsrStats,
    pub sta_lta_stats: HvsrStats,
    pub f0_stats: Option<HvsrStats>,
    pub sesame_result: Option<crate::core::sesame_evaluator::SesameResult>,
    pub window_starts_s: Vec<f64>,
    pub final_valid_idx: Vec<bool>, // Used for time domain plot coloring
}

#[derive(Clone)]
pub struct HvsrWindows {
    pub window_starts_s: Vec<f64>,
    pub valid_windows_idx: Vec<bool>,
    pub window_len_s: f64,
}

pub enum HvsrProgress {
    Progress(f32, String),
    Complete(HvsrResult),
    Error(String),
}

pub fn detrend_signal(data: &mut [f64]) {
    let n = data.len();
    if n <= 1 { return; }
    
    // Demean
    let mean = data.iter().sum::<f64>() / (n as f64);
    for x in data.iter_mut() {
        *x -= mean;
    }
    
    // Linear Detrend
    let x_mean = (n - 1) as f64 / 2.0;
    
    let mut sum_xy = 0.0;
    let mut sum_xx = 0.0;
    for i in 0..n {
        let x = i as f64 - x_mean;
        sum_xy += x * data[i];
        sum_xx += x * x;
    }
    
    let m = if sum_xx > 0.0 { sum_xy / sum_xx } else { 0.0 };
    
    for i in 0..n {
        let x = i as f64 - x_mean;
        data[i] -= m * x;
    }
}

pub fn apply_taper_detrend(data: &mut [f64]) {
    let n = data.len();
    if n == 0 { return; }
    
    detrend_signal(data);
    
    // Cosine Taper (5% on each end)
    let taper_pct = 0.05;
    let taper_len = (n as f64 * taper_pct) as usize;
    for i in 0..taper_len {
        let w = 0.5 * (1.0 - (PI * (i as f64) / (taper_len as f64)).cos());
        data[i] *= w;
        data[n - 1 - i] *= w;
    }
}

fn compute_sta_lta(data: &[f64], sta_len: usize, lta_len: usize) -> Vec<f64> {
    let mut ratio = vec![0.0; data.len()];
    if lta_len == 0 || data.is_empty() { return ratio; }
    
    let mut sta_sum = 0.0;
    let mut lta_sum = 0.0;
    
    for i in 0..data.len() {
        let val = data[i].abs();
        sta_sum += val;
        lta_sum += val;
        
        if i >= sta_len {
            sta_sum -= data[i - sta_len].abs();
        }
        if i >= lta_len {
            lta_sum -= data[i - lta_len].abs();
        }
        
        let mut cur_sta = sta_sum;
        if i < sta_len { cur_sta /= (i + 1) as f64; } else { cur_sta /= sta_len as f64; }
        
        let mut cur_lta = lta_sum;
        if i < lta_len { cur_lta /= (i + 1) as f64; } else { cur_lta /= lta_len as f64; }
        
        if cur_lta > 1e-12 {
            ratio[i] = cur_sta / cur_lta;
        } else {
            ratio[i] = 1.0;
        }
    }
    ratio
}

fn konno_ohmachi_smoothing(amp: &[f64], fft_freq: &[f64], target_freq: &[f64], b_value: f64) -> Vec<f64> {
    let n_in = amp.len();
    let n_out = target_freq.len();
    let mut smoothed = vec![0.0; n_out];
    
    for i in 0..n_out {
        let fc = target_freq[i];
        if fc <= 1e-6 {
            smoothed[i] = amp[0];
            continue;
        }
        
        let mut sum_w = 0.0;
        let mut sum_wa = 0.0;
        
        for j in 0..n_in {
            let f = fft_freq[j];
            if f <= 1e-6 { continue; }
            
            let z = f / fc;
            if z < 0.5 || z > 2.0 {
                continue;
            }
            
            let x = b_value * z.log10();
            
            // Konno Ohmachi formula: W(f, fc) = (sin(x) / x)^4
            let w = if x.abs() < 1e-5 {
                1.0
            } else {
                (x.sin() / x).powi(4)
            };
            
            sum_w += w;
            sum_wa += w * amp[j];
        }
        
        if sum_w > 0.0 {
            smoothed[i] = sum_wa / sum_w;
        }
    }
    smoothed
}

pub fn compute_windows(
    z_data: &[f64],
    n_data: &[f64],
    e_data: &[f64],
    dt: f64,
    params: &HvsrParams
) -> Result<HvsrWindows, String> {
    let total_samples = z_data.len();
    if total_samples == 0 {
        return Err("Empty data array".to_string());
    }
    
    let sr = 1.0 / dt;
    let window_samples = (params.window_len_s * sr) as usize;
    let step_samples = (window_samples as f64 * (1.0 - params.overlap_pct / 100.0)) as usize;
    
    if window_samples > total_samples || step_samples == 0 {
        return Err("Invalid window length or overlap".to_string());
    }
    
    let num_windows = (total_samples - window_samples) / step_samples + 1;
    let mut valid_windows_idx = vec![false; num_windows];
    let mut window_starts_s = vec![0.0; num_windows];
    
    let sta_samples = (params.sta_len_s * sr) as usize;
    let lta_samples = (params.lta_len_s * sr) as usize;
    
    for i in 0..num_windows {
        let start = i * step_samples;
        let end = start + window_samples;
        window_starts_s[i] = start as f64 * dt;
        
        let z_win = &z_data[start..end];
        let n_win = &n_data[start..end];
        let e_win = &e_data[start..end];
        
        let z_sta_lta = compute_sta_lta(z_win, sta_samples, lta_samples);
        let n_sta_lta = compute_sta_lta(n_win, sta_samples, lta_samples);
        let e_sta_lta = compute_sta_lta(e_win, sta_samples, lta_samples);
        
        let mut pass = true;
        for j in 0..z_sta_lta.len() {
            let r_z = z_sta_lta[j];
            let r_n = n_sta_lta[j];
            let r_e = e_sta_lta[j];
            if r_z < params.t1 || r_z > params.t2 || 
               r_n < params.t1 || r_n > params.t2 || 
               r_e < params.t1 || r_e > params.t2 {
                pass = false;
                break;
            }
        }
        
        valid_windows_idx[i] = pass;
    }
    
    Ok(HvsrWindows {
        window_starts_s,
        valid_windows_idx,
        window_len_s: params.window_len_s,
    })
}

pub fn process_hvsr_pipeline(
    z_data: Vec<f64>, n_data: Vec<f64>, e_data: Vec<f64>, 
    dt: f64, 
    params: HvsrParams, 
    mut windows: HvsrWindows,
    sender: Sender<HvsrProgress>
) {
    if z_data.len() != n_data.len() || z_data.len() != e_data.len() {
        let _ = sender.send(HvsrProgress::Error("Components must have the same length".to_string()));
        return;
    }
    
    let total_samples = z_data.len();
    if total_samples == 0 {
        let _ = sender.send(HvsrProgress::Error("Empty data array".to_string()));
        return;
    }
    
    let sr = 1.0 / dt;
    let window_samples = (params.window_len_s * sr) as usize;
    let step_samples = (window_samples as f64 * (1.0 - params.overlap_pct / 100.0)) as usize;
    
    let num_windows = windows.window_starts_s.len();
    let mut valid_count = 0;
    for &v in &windows.valid_windows_idx {
        if v { valid_count += 1; }
    }
    
    let mut all_hvsr = Vec::new();
    
    let mut planner = FftPlanner::<f64>::new();
    let fft = planner.plan_fft_forward(window_samples);
    let num_freqs = window_samples / 2 + 1;
    let fft_freqs: Vec<f64> = (0..num_freqs).map(|i| (i as f64 * sr) / (window_samples as f64)).collect();
    
    // Generate logarithmically spaced target frequencies
    let mut target_freqs = Vec::with_capacity(params.freq_count);
    let log_min = params.freq_min.max(0.001).ln();
    let log_max = params.freq_max.max(params.freq_min + 0.1).ln();
    for i in 0..params.freq_count {
        let frac = if params.freq_count > 1 { i as f64 / (params.freq_count - 1) as f64 } else { 0.0 };
        let f = (log_min + frac * (log_max - log_min)).exp();
        target_freqs.push(f);
    }
    let num_target_freqs = target_freqs.len();
    
    let mut sta_lta_indices = Vec::new();
    let mut raw_indices = Vec::with_capacity(num_windows);
    
    for i in 0..num_windows {
        let prog = (i as f32) / (num_windows as f32);
        let _ = sender.send(HvsrProgress::Progress(prog, format!("Processing window {}/{}...", i + 1, num_windows)));
        
        let pass = windows.valid_windows_idx[i];
        
        let start = i * step_samples;
        let end = start + window_samples;
        
        let z_win = &z_data[start..end];
        let n_win = &n_data[start..end];
        let e_win = &e_data[start..end];
        
        let process_comp = |comp: &[f64]| -> Vec<f64> {
            let mut data = comp.to_vec();
            apply_taper_detrend(&mut data);
            
            let mut complex_data: Vec<Complex<f64>> = data.iter().map(|&x| Complex { re: x, im: 0.0 }).collect();
            fft.process(&mut complex_data);
            
            let amp: Vec<f64> = complex_data[0..num_freqs].iter().map(|c| c.norm()).collect();
            konno_ohmachi_smoothing(&amp, &fft_freqs, &target_freqs, params.b_value)
        };
        
        let prog = (i as f32 + 0.5) / (num_windows as f32);
        let _ = sender.send(HvsrProgress::Progress(prog, format!("Smoothing window {}/{}...", i + 1, num_windows)));
        let z_smoothed = process_comp(z_win);
        let n_smoothed = process_comp(n_win);
        let e_smoothed = process_comp(e_win);
        
        let mut hvsr = vec![0.0; num_target_freqs];
        for j in 0..num_target_freqs {
            let n = n_smoothed[j];
            let e = e_smoothed[j];
            let h = match params.combine_method {
                HorizontalCombineMethod::Geometric => (n * e).sqrt(),
                HorizontalCombineMethod::Quadratic => ((n.powi(2) + e.powi(2)) / 2.0).sqrt(),
                HorizontalCombineMethod::Arithmetic => (n + e) / 2.0,
                HorizontalCombineMethod::Maximum => n.max(e),
            };
            hvsr[j] = h / z_smoothed[j].max(1e-12);
        }
        all_hvsr.push(hvsr);
        raw_indices.push(i);
        if pass {
            sta_lta_indices.push(i);
        }
    }
    
    let raw_stats = compute_stats(&raw_indices, &all_hvsr, num_target_freqs, &target_freqs);
    let sta_lta_stats = compute_stats(&sta_lta_indices, &all_hvsr, num_target_freqs, &target_freqs);
    
    let mut f0_stats = None;
    let final_indices;
    
    if params.enable_f0_filter && !sta_lta_indices.is_empty() {
        let _ = sender.send(HvsrProgress::Progress(0.95, "Running iterative f0 filter...".to_string()));
        let mut max_amps_idx = Vec::with_capacity(sta_lta_indices.len());
        for &i in &sta_lta_indices {
            let mut max_amp = -1.0;
            let mut max_idx = 0;
            for (j, &amp) in all_hvsr[i].iter().enumerate() {
                if amp > max_amp {
                    max_amp = amp;
                    max_idx = j;
                }
            }
            max_amps_idx.push(max_idx);
        }
        
        let filtered_local = filter_f0_iterative(&target_freqs, &max_amps_idx, sta_lta_indices.len(), params.f0_filter_n);
        let mut filtered_global = Vec::with_capacity(filtered_local.len());
        for local_idx in filtered_local {
            filtered_global.push(sta_lta_indices[local_idx]);
        }
        
        let stats = compute_stats(&filtered_global, &all_hvsr, num_target_freqs, &target_freqs);
        f0_stats = Some(stats);
        final_indices = filtered_global;
    } else {
        final_indices = sta_lta_indices.clone();
    }
    
    let mut final_valid_idx = vec![false; num_windows];
    for &i in &final_indices {
        final_valid_idx[i] = true;
    }
    
    let final_count = final_indices.len();
    
    let mut sesame_result = None;
    if final_count > 0 {
        let final_stats = if let Some(f0_s) = &f0_stats { f0_s } else { &sta_lta_stats };
        
        let mut max_mean = -1.0;
        let mut f0 = 0.0;
        for i in 0..num_target_freqs {
            if final_stats.mean_hvsr[i] > max_mean {
                max_mean = final_stats.mean_hvsr[i];
                f0 = target_freqs[i];
            }
        }
        
        // Calculate sigma_f from final valid windows
        let mut f0s = Vec::with_capacity(final_count);
        for &i in &final_indices {
            let mut w_max = -1.0;
            let mut w_f0 = 0.0;
            for j in 0..num_target_freqs {
                if all_hvsr[i][j] > w_max {
                    w_max = all_hvsr[i][j];
                    w_f0 = target_freqs[j];
                }
            }
            f0s.push(w_f0);
        }
        
        let mu_f0 = f0s.iter().sum::<f64>() / final_count as f64;
        let mut sum_sq = 0.0;
        for &w_f0 in &f0s {
            sum_sq += (w_f0 - mu_f0).powi(2);
        }
        let sigma_f = (sum_sq / final_count as f64).sqrt();
        
        let mut sigma_a = vec![0.0; num_target_freqs];
        for i in 0..num_target_freqs {
            sigma_a[i] = final_stats.std_plus[i] / final_stats.mean_hvsr[i];
        }
        
        sesame_result = Some(crate::core::sesame_evaluator::evaluate_sesame(
            f0, max_mean, params.window_len_s, final_count, &target_freqs, &final_stats.mean_hvsr, &sigma_a, sigma_f
        ));
    }
    
    let _ = sender.send(HvsrProgress::Complete(HvsrResult {
        freq: target_freqs,
        all_hvsr,
        raw_stats,
        sta_lta_stats,
        f0_stats,
        sesame_result,
        window_starts_s: windows.window_starts_s,
        final_valid_idx,
    }));
}

fn filter_f0_iterative(
    target_freqs: &[f64],
    max_amps_idx: &[usize],
    valid_count: usize,
    n_param: f64
) -> Vec<usize> {
    let mut current_indices: Vec<usize> = (0..valid_count).collect();
    let max_iter = 50;
    
    let mut last_d = f64::MAX;
    let mut last_sigma = f64::MAX;
    
    for _iter in 0..max_iter {
        if current_indices.is_empty() {
            break;
        }
        
        let mut sum_ln = 0.0;
        let mut ln_f0s = Vec::with_capacity(current_indices.len());
        let mut curr_f0s = Vec::with_capacity(current_indices.len());
        
        for &idx in &current_indices {
            let f0 = target_freqs[max_amps_idx[idx]];
            curr_f0s.push(f0);
            let ln_f0 = f0.ln();
            ln_f0s.push(ln_f0);
            sum_ln += ln_f0;
        }
        
        let mu_lnf0_b = sum_ln / current_indices.len() as f64;
        let mut sum_sq_diff = 0.0;
        for &ln_f0 in &ln_f0s {
            sum_sq_diff += (ln_f0 - mu_lnf0_b).powi(2);
        }
        let sigma_lnf0_b = (sum_sq_diff / current_indices.len() as f64).sqrt();
        let lm_f0_b = mu_lnf0_b.exp();
        
        curr_f0s.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mid = curr_f0s.len() / 2;
        let f0_mc_b = if curr_f0s.len() % 2 == 0 {
            (curr_f0s[mid - 1] + curr_f0s[mid]) / 2.0
        } else {
            curr_f0s[mid]
        };
        let d_b = (lm_f0_b - f0_mc_b).abs();
        
        let lb = (mu_lnf0_b - n_param * sigma_lnf0_b).exp();
        let ub = (mu_lnf0_b + n_param * sigma_lnf0_b).exp();
        
        let mut next_indices = Vec::new();
        for &idx in &current_indices {
            let f0 = target_freqs[max_amps_idx[idx]];
            if f0 >= lb && f0 <= ub {
                next_indices.push(idx);
            }
        }
        
        if next_indices.is_empty() || next_indices.len() == current_indices.len() {
            break;
        }
        
        let mut sum_ln_e = 0.0;
        let mut ln_f0s_e = Vec::with_capacity(next_indices.len());
        let mut next_f0s = Vec::with_capacity(next_indices.len());
        
        for &idx in &next_indices {
            let f0 = target_freqs[max_amps_idx[idx]];
            next_f0s.push(f0);
            let ln_f0 = f0.ln();
            ln_f0s_e.push(ln_f0);
            sum_ln_e += ln_f0;
        }
        
        let mu_lnf0_e = sum_ln_e / next_indices.len() as f64;
        let mut sum_sq_diff_e = 0.0;
        for &ln_f0 in &ln_f0s_e {
            sum_sq_diff_e += (ln_f0 - mu_lnf0_e).powi(2);
        }
        let sigma_lnf0_e = (sum_sq_diff_e / next_indices.len() as f64).sqrt();
        let lm_f0_e = mu_lnf0_e.exp();
        
        next_f0s.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mid_e = next_f0s.len() / 2;
        let f0_mc_e = if next_f0s.len() % 2 == 0 {
            (next_f0s[mid_e - 1] + next_f0s[mid_e]) / 2.0
        } else {
            next_f0s[mid_e]
        };
        let d_e = (lm_f0_e - f0_mc_e).abs();
        
        let delta_d = if d_b > 1e-12 { (d_e - d_b).abs() / d_b } else { 0.0 };
        let delta_sigma = (sigma_lnf0_e - sigma_lnf0_b).abs();
        
        current_indices = next_indices;
        
        if delta_d < 0.01 && delta_sigma < 0.01 {
            break;
        }
        
        last_d = d_e;
        last_sigma = sigma_lnf0_e;
    }
    
    current_indices
}

fn compute_stats(indices: &[usize], all_hvsr: &[Vec<f64>], num_target_freqs: usize, target_freqs: &[f64]) -> HvsrStats {
    let mut mean_hvsr = vec![0.0; num_target_freqs];
    let mut std_plus = vec![0.0; num_target_freqs];
    let mut std_minus = vec![0.0; num_target_freqs];
    
    let count = indices.len();
    let mut f0_mean = 0.0;
    let mut f0_std = 0.0;
    
    if count > 0 {
        for j in 0..num_target_freqs {
            let mut sum_log = 0.0;
            for &i in indices {
                sum_log += all_hvsr[i][j].ln();
            }
            let mean_log = sum_log / (count as f64);
            mean_hvsr[j] = mean_log.exp();
            
            let mut sum_sq_diff = 0.0;
            for &i in indices {
                sum_sq_diff += (all_hvsr[i][j].ln() - mean_log).powi(2);
            }
            let std_log = (sum_sq_diff / (count as f64)).sqrt();
            std_plus[j] = (mean_log + std_log).exp();
            std_minus[j] = (mean_log - std_log).exp();
        }
        
        let mut f0s = Vec::with_capacity(count);
        for &i in indices {
            let mut max_amp = -1.0;
            let mut f0 = 0.0;
            for j in 0..num_target_freqs {
                if all_hvsr[i][j] > max_amp {
                    max_amp = all_hvsr[i][j];
                    f0 = target_freqs[j];
                }
            }
            f0s.push(f0);
        }
        
        f0_mean = f0s.iter().sum::<f64>() / (count as f64);
        let mut sum_sq = 0.0;
        for &f0 in &f0s {
            sum_sq += (f0 - f0_mean).powi(2);
        }
        f0_std = (sum_sq / (count as f64)).sqrt();
    }
    
    HvsrStats {
        mean_hvsr,
        std_plus,
        std_minus,
        valid_indices: indices.to_vec(),
        f0_mean,
        f0_std,
    }
}
