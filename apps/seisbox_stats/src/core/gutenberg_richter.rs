/// Gutenberg-Richter analysis: Mc estimation and b-value calculation.

/// Result of b-value calculation
#[derive(Debug, Clone)]
pub struct BvalueResult {
    pub b: f64,
    pub b_uncertainty: f64,
    pub a: f64,
    pub mc: f64,
    pub n_above_mc: usize,
}

// ============================================================
// Frequency-Magnitude Distribution
// ============================================================

/// Compute incremental and cumulative FMD.
/// Returns (bin_centers, incremental_counts, cumulative_counts)
pub fn fmd(mags: &[f64], bin_width: f64) -> (Vec<f64>, Vec<usize>, Vec<usize>) {
    if mags.is_empty() {
        return (Vec::new(), Vec::new(), Vec::new());
    }
    
    let min_m = (mags.iter().copied().fold(f64::INFINITY, f64::min) / bin_width).floor() * bin_width;
    let max_m = (mags.iter().copied().fold(f64::NEG_INFINITY, f64::max) / bin_width).ceil() * bin_width;
    
    let n_bins = ((max_m - min_m) / bin_width).round() as usize + 1;
    let mut bins = vec![0usize; n_bins];
    let mut centers = Vec::with_capacity(n_bins);
    
    for i in 0..n_bins {
        centers.push(min_m + i as f64 * bin_width);
    }
    
    for &m in mags {
        let idx = ((m - min_m) / bin_width).round() as usize;
        if idx < n_bins {
            bins[idx] += 1;
        }
    }
    
    // Cumulative (N >= M)
    let mut cumulative = vec![0usize; n_bins];
    let mut running = 0usize;
    for i in (0..n_bins).rev() {
        running += bins[i];
        cumulative[i] = running;
    }
    
    (centers, bins, cumulative)
}

// ============================================================
// Magnitude of Completeness (Mc) Estimation
// ============================================================

/// MAXC: Maximum Curvature method.
/// Mc = magnitude bin with the highest frequency (+ 0.2 correction)
pub fn mc_maxc(mags: &[f64], bin_width: f64) -> f64 {
    let (centers, incremental, _) = fmd(mags, bin_width);
    if centers.is_empty() {
        return 0.0;
    }
    
    let max_idx = incremental.iter()
        .enumerate()
        .max_by_key(|(_, &c)| c)
        .map(|(i, _)| i)
        .unwrap_or(0);
    
    // MAXC correction: +0.2 (Woessner & Wiemer, 2005)
    centers[max_idx] + 0.2
}

/// GFT: Goodness-of-Fit Test (Wiemer & Wyss, 2000).
/// Tests synthetic GR distribution against observed FMD for each trial Mc.
/// Returns Mc where residual first drops below (1 - confidence).
pub fn mc_gft(mags: &[f64], bin_width: f64, confidence: f64) -> f64 {
    let (centers, _incremental, cumulative) = fmd(mags, bin_width);
    if centers.is_empty() {
        return 0.0;
    }
    
    let threshold = 1.0 - confidence; // e.g. 0.05 for 95%, 0.10 for 90%
    let n_total = mags.len() as f64;
    
    for (trial_idx, &trial_mc) in centers.iter().enumerate() {
        // Filter magnitudes >= trial Mc
        let filtered: Vec<f64> = mags.iter().copied().filter(|&m| m >= trial_mc - bin_width * 0.01).collect();
        let n = filtered.len();
        if n < 10 { continue; }
        
        // Calculate b-value for this trial Mc using MLE
        let mean_m = filtered.iter().sum::<f64>() / n as f64;
        let b = std::f64::consts::LOG10_E / (mean_m - (trial_mc - bin_width / 2.0));
        if b <= 0.0 || b.is_nan() { continue; }
        
        let a = (n as f64).log10() + b * trial_mc;
        
        // Compute residual between observed and synthetic cumulative counts
        let mut residual_sum = 0.0;
        let mut obs_sum = 0.0;
        
        for j in trial_idx..centers.len() {
            let observed = cumulative[j] as f64;
            let synthetic = 10f64.powf(a - b * centers[j]);
            residual_sum += (observed - synthetic).abs();
            obs_sum += observed;
        }
        
        let r = if obs_sum > 0.0 { residual_sum / obs_sum } else { 1.0 };
        
        if r <= threshold {
            return trial_mc;
        }
    }
    
    // Fallback to MAXC
    mc_maxc(mags, bin_width)
}

/// EMR: Entire Magnitude Range method (Woessner & Wiemer, 2005).
/// Fits GR distribution to complete part and normal CDF to incomplete part.
pub fn mc_emr(mags: &[f64], bin_width: f64) -> f64 {
    let (centers, incremental, _) = fmd(mags, bin_width);
    if centers.is_empty() {
        return 0.0;
    }
    
    let mut best_mc = centers[0];
    let mut best_residual = f64::INFINITY;
    
    for (trial_idx, &trial_mc) in centers.iter().enumerate() {
        let filtered: Vec<f64> = mags.iter().copied().filter(|&m| m >= trial_mc - bin_width * 0.01).collect();
        let n = filtered.len();
        if n < 10 { continue; }
        
        let mean_m = filtered.iter().sum::<f64>() / n as f64;
        let b = std::f64::consts::LOG10_E / (mean_m - (trial_mc - bin_width / 2.0));
        if b <= 0.0 || b.is_nan() { continue; }
        
        let a = (n as f64).log10() + b * trial_mc;
        
        // Residual for the complete part (>= Mc)
        let mut res_complete = 0.0;
        for j in trial_idx..centers.len() {
            let obs = incremental[j] as f64;
            let syn = 10f64.powf(a - b * centers[j]) - 10f64.powf(a - b * (centers[j] + bin_width));
            res_complete += (obs - syn.max(0.0)).powi(2);
        }
        
        // Residual for incomplete part (< Mc): model as linear decrease
        let mut res_incomplete = 0.0;
        if trial_idx > 0 {
            let max_count = incremental[trial_idx] as f64;
            for j in 0..trial_idx {
                let frac = (j as f64) / (trial_idx as f64);
                let expected = max_count * frac;
                res_incomplete += (incremental[j] as f64 - expected).powi(2);
            }
        }
        
        let total_res = res_complete + res_incomplete;
        if total_res < best_residual {
            best_residual = total_res;
            best_mc = trial_mc;
        }
    }
    
    best_mc
}

// ============================================================
// B-value Calculation
// ============================================================

/// MLE: Maximum Likelihood Estimation (Aki, 1965; Utsu, 1965).
/// b = log10(e) / (M_mean - (Mc - ΔM/2))
/// Uncertainty: Shi & Bolt (1982) formula.
pub fn bvalue_mle(mags: &[f64], mc: f64, bin_width: f64) -> BvalueResult {
    let filtered: Vec<f64> = mags.iter().copied().filter(|&m| m >= mc - bin_width * 0.01).collect();
    let n = filtered.len();
    
    if n < 2 {
        return BvalueResult { b: 0.0, b_uncertainty: 0.0, a: 0.0, mc, n_above_mc: n };
    }
    
    let mean_m = filtered.iter().sum::<f64>() / n as f64;
    let b = std::f64::consts::LOG10_E / (mean_m - (mc - bin_width / 2.0));
    
    // Shi & Bolt (1982) uncertainty
    let var = filtered.iter().map(|&m| (m - mean_m).powi(2)).sum::<f64>() / ((n - 1) as f64);
    let b_uncertainty = 2.30 * b * b * var.sqrt() / (n as f64).sqrt();
    
    // a-value: log10(N) = a - b * Mc
    let a = (n as f64).log10() + b * mc;
    
    BvalueResult { b, b_uncertainty, a, mc, n_above_mc: n }
}

/// WLS: Weighted Least Squares on log10(N) vs M.
pub fn bvalue_wls(mags: &[f64], mc: f64, bin_width: f64) -> BvalueResult {
    let filtered: Vec<f64> = mags.iter().copied().filter(|&m| m >= mc - bin_width * 0.01).collect();
    let n_above = filtered.len();
    
    if n_above < 5 {
        return BvalueResult { b: 0.0, b_uncertainty: 0.0, a: 0.0, mc, n_above_mc: n_above };
    }
    
    let (centers, _incr, cumul) = fmd(&filtered, bin_width);
    
    // Only use bins with non-zero cumulative counts
    let mut x_vals = Vec::new();
    let mut y_vals = Vec::new();
    let mut weights = Vec::new();
    
    for (i, &c) in cumul.iter().enumerate() {
        if c > 0 {
            x_vals.push(centers[i]);
            y_vals.push((c as f64).log10());
            weights.push(c as f64); // Weight by count
        }
    }
    
    if x_vals.len() < 2 {
        return BvalueResult { b: 0.0, b_uncertainty: 0.0, a: 0.0, mc, n_above_mc: n_above };
    }
    
    // Weighted least squares: y = a - b*x
    let w_sum: f64 = weights.iter().sum();
    let wx_sum: f64 = weights.iter().zip(x_vals.iter()).map(|(&w, &x)| w * x).sum();
    let wy_sum: f64 = weights.iter().zip(y_vals.iter()).map(|(&w, &y)| w * y).sum();
    let wxx_sum: f64 = weights.iter().zip(x_vals.iter()).map(|(&w, &x)| w * x * x).sum();
    let wxy_sum: f64 = weights.iter().zip(x_vals.iter()).zip(y_vals.iter())
        .map(|((&w, &x), &y)| w * x * y).sum();
    
    let denom = w_sum * wxx_sum - wx_sum * wx_sum;
    if denom.abs() < 1e-15 {
        return BvalueResult { b: 0.0, b_uncertainty: 0.0, a: 0.0, mc, n_above_mc: n_above };
    }
    
    let a = (wxx_sum * wy_sum - wx_sum * wxy_sum) / denom;
    let b_neg = (w_sum * wxy_sum - wx_sum * wy_sum) / denom; // This is -b
    let b = -b_neg;
    
    // Uncertainty from residuals
    let n_pts = x_vals.len() as f64;
    let residual_var: f64 = x_vals.iter().zip(y_vals.iter()).zip(weights.iter())
        .map(|((&x, &y), &w)| w * (y - a + b * x).powi(2))
        .sum::<f64>() / (w_sum * (n_pts - 2.0) / n_pts);
    let b_uncertainty = (residual_var * w_sum / denom).sqrt();
    
    BvalueResult { b, b_uncertainty, a, mc, n_above_mc: n_above }
}
