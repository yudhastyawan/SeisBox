use crate::core::catalogue::Catalogue;
use chrono::NaiveDateTime;

#[derive(Debug, Clone)]
pub struct OmoriResult {
    pub k: f64,
    pub c: f64,
    pub p: f64,
    pub log_likelihood: f64,
}

/// Fit the Modified Omori Law (MOL) to an aftershock sequence using Maximum Likelihood Estimation.
/// Rate: lambda(t) = k / (t + c)^p
pub fn fit_omori(
    cat: &Catalogue,
    mainshock_time: NaiveDateTime,
    max_days: f64,
    min_mag: Option<f64>,
) -> OmoriResult {
    // 1. Filter events after mainshock up to max_days
    let mut aftershocks: Vec<f64> = cat.events.iter()
        .filter(|e| e.time > mainshock_time)
        .filter(|e| min_mag.map_or(true, |m| e.mag >= m))
        .map(|e| (e.time - mainshock_time).num_seconds() as f64 / 86400.0)
        .filter(|&t| t <= max_days)
        .collect();
    
    aftershocks.sort_by(|a, b| a.partial_cmp(b).unwrap());
    
    let n = aftershocks.len();
    if n < 10 {
        return OmoriResult { k: 0.0, c: 0.0, p: 0.0, log_likelihood: f64::NEG_INFINITY };
    }
    
    let t_start = aftershocks.first().copied().unwrap_or(0.0).max(1e-5);
    let t_end = aftershocks.last().copied().unwrap_or(max_days);
    
    // Grid search for Maximum Likelihood Estimation
    // Log-likelihood function (Ogata 1983):
    // ln L = N * ln(K) - p * Sum(ln(t_i + c)) - Sum(ln(t_i + c)) - K * Integral_start^end (t+c)^-p dt
    // Wait, K is derived analytically from p and c:
    // K = N / Integral_start^end (t+c)^-p dt
    
    let mut best_res = OmoriResult { k: 0.0, c: 0.0, p: 0.0, log_likelihood: f64::NEG_INFINITY };
    
    // Grid bounds
    let p_vals = linspace(0.5, 2.5, 100);
    let c_vals = linspace(0.001, 2.0, 100);
    
    for &p in &p_vals {
        for &c in &c_vals {
            let integral = if (p - 1.0).abs() < 1e-4 {
                (t_end + c).ln() - (t_start + c).ln()
            } else {
                ((t_end + c).powf(1.0 - p) - (t_start + c).powf(1.0 - p)) / (1.0 - p)
            };
            
            if integral <= 0.0 { continue; }
            
            let k = (n as f64) / integral;
            
            let sum_ln_t_c: f64 = aftershocks.iter().map(|&t| (t + c).ln()).sum();
            
            let log_likelihood = (n as f64) * k.ln() - p * sum_ln_t_c - k * integral;
            
            if log_likelihood > best_res.log_likelihood {
                best_res = OmoriResult { k, c, p, log_likelihood };
            }
        }
    }
    
    // Optional: fine grid search around the best result
    let p_fine = linspace((best_res.p - 0.1).max(0.01), best_res.p + 0.1, 50);
    let c_fine = linspace((best_res.c - 0.05).max(0.0001), best_res.c + 0.05, 50);
    
    for &p in &p_fine {
        for &c in &c_fine {
            let integral = if (p - 1.0).abs() < 1e-4 {
                (t_end + c).ln() - (t_start + c).ln()
            } else {
                ((t_end + c).powf(1.0 - p) - (t_start + c).powf(1.0 - p)) / (1.0 - p)
            };
            if integral <= 0.0 { continue; }
            let k = (n as f64) / integral;
            let sum_ln_t_c: f64 = aftershocks.iter().map(|&t| (t + c).ln()).sum();
            let log_likelihood = (n as f64) * k.ln() - p * sum_ln_t_c - k * integral;
            
            if log_likelihood > best_res.log_likelihood {
                best_res = OmoriResult { k, c, p, log_likelihood };
            }
        }
    }
    
    best_res
}

fn linspace(start: f64, end: f64, num: usize) -> Vec<f64> {
    if num == 0 { return Vec::new(); }
    if num == 1 { return vec![start]; }
    let step = (end - start) / ((num - 1) as f64);
    (0..num).map(|i| start + (i as f64) * step).collect()
}
