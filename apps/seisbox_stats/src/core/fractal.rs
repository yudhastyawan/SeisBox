use crate::core::catalogue::{Catalogue, haversine_km};
use rayon::prelude::*;

#[derive(Debug, Clone)]
pub struct FractalResult {
    pub d_value: f64,
    pub d_unc: f64,
    pub r_min: f64,
    pub r_max: f64,
}

/// Calculate Fractal Correlation Dimension (D-value) using Grassberger-Procaccia algorithm.
pub fn fractal_dimension(
    cat: &Catalogue,
    min_mag: Option<f64>,
    num_r_bins: usize,
) -> (FractalResult, Vec<f64>, Vec<f64>) {
    let events: Vec<&crate::core::catalogue::Earthquake> = cat.events.iter()
        .filter(|e| min_mag.map_or(true, |m| e.mag >= m))
        .collect();
        
    let n = events.len();
    if n < 10 {
        return (FractalResult { d_value: 0.0, d_unc: 0.0, r_min: 0.0, r_max: 0.0 }, vec![], vec![]);
    }
    
    // Calculate all pairwise distances
    // For large N this is O(N^2), so we parallelize
    let pairs = (n * (n - 1)) / 2;
    
    // Instead of collecting all distances (memory heavy), we can bin them directly
    // First, find min/max distance from a sample to set bin edges
    let sample_size = n.min(500);
    let mut sample_dists = Vec::with_capacity((sample_size * (sample_size - 1)) / 2);
    for i in 0..sample_size {
        for j in (i + 1)..sample_size {
            let d = haversine_km(events[i].lat, events[i].lon, events[j].lat, events[j].lon);
            if d > 0.0 {
                sample_dists.push(d);
            }
        }
    }
    
    if sample_dists.is_empty() {
        return (FractalResult { d_value: 0.0, d_unc: 0.0, r_min: 0.0, r_max: 0.0 }, vec![], vec![]);
    }
    
    sample_dists.sort_by(|a, b| a.partial_cmp(b).unwrap());
    
    // Use logarithmic bins for r
    // Skip the absolute zero distances
    let r_min = sample_dists.first().copied().unwrap_or(0.1).max(0.1);
    let r_max = sample_dists.last().copied().unwrap_or(1000.0);
    
    let log_r_min = r_min.log10();
    let log_r_max = r_max.log10();
    
    let log_r_step = (log_r_max - log_r_min) / ((num_r_bins - 1) as f64);
    let mut log_r_bins = Vec::with_capacity(num_r_bins);
    let mut r_bins = Vec::with_capacity(num_r_bins);
    for i in 0..num_r_bins {
        let lr = log_r_min + (i as f64) * log_r_step;
        log_r_bins.push(lr);
        r_bins.push(10f64.powf(lr));
    }
    
    // Accumulate counts for each bin
    let counts: Vec<usize> = (0..n).into_par_iter().map(|i| {
        let mut local_counts = vec![0usize; num_r_bins];
        for j in (i + 1)..n {
            let d = haversine_km(events[i].lat, events[i].lon, events[j].lat, events[j].lon);
            if d > 0.0 {
                // Find bin index
                // d <= r_bins[k]
                for (k, &r) in r_bins.iter().enumerate() {
                    if d <= r {
                        local_counts[k] += 1;
                        // Note: we want cumulative count! So if it's smaller than r_k, it's also smaller than r_{k+1}, etc.
                        // Actually, it's easier to just accumulate histogram and then cumulative sum.
                        break;
                    }
                }
            }
        }
        local_counts
    }).reduce(
        || vec![0usize; num_r_bins],
        |mut a, b| {
            for k in 0..num_r_bins {
                a[k] += b[k];
            }
            a
        }
    );
    
    // Cumulative sum
    let mut c_r = vec![0f64; num_r_bins];
    let mut current_sum = 0usize;
    let norm = 2.0 / (pairs as f64);
    
    for k in 0..num_r_bins {
        current_sum += counts[k];
        c_r[k] = (current_sum as f64) * norm;
    }
    
    // Fit line to linear portion of log(C(r)) vs log(r)
    // Find steepest contiguous region
    let mut best_slope = 0.0;
    let mut log_c_r = vec![0f64; num_r_bins];
    for k in 0..num_r_bins {
        if c_r[k] > 0.0 {
            log_c_r[k] = c_r[k].log10();
        } else {
            log_c_r[k] = f64::NEG_INFINITY;
        }
    }
    
    // Simple fit over the middle 50% of valid bins
    let valid_indices: Vec<usize> = (0..num_r_bins).filter(|&k| log_c_r[k] != f64::NEG_INFINITY).collect();
    let mut x_fit = Vec::new();
    let mut y_fit = Vec::new();
    
    if valid_indices.len() >= 5 {
        let start = valid_indices[valid_indices.len() / 4];
        let end = valid_indices[3 * valid_indices.len() / 4];
        
        for &k in &valid_indices {
            if k >= start && k <= end {
                x_fit.push(log_r_bins[k]);
                y_fit.push(log_c_r[k]);
            }
        }
        
        let n_fit = x_fit.len() as f64;
        let sum_x: f64 = x_fit.iter().sum();
        let sum_y: f64 = y_fit.iter().sum();
        let sum_xx: f64 = x_fit.iter().map(|&x| x * x).sum();
        let sum_xy: f64 = x_fit.iter().zip(y_fit.iter()).map(|(&x, &y)| x * y).sum();
        
        let denom = n_fit * sum_xx - sum_x * sum_x;
        if denom.abs() > 1e-10 {
            best_slope = (n_fit * sum_xy - sum_x * sum_y) / denom;
        }
    }
    
    let res = FractalResult {
        d_value: best_slope,
        d_unc: 0.0, // simplified
        r_min,
        r_max,
    };
    
    (res, r_bins, c_r)
}
