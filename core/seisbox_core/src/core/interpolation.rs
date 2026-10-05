use nalgebra::{DMatrix, DVector};
use rayon::prelude::*;
use std::f64::consts::PI;

#[derive(Debug, Clone, Copy)]
pub struct XYZPoint {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VariogramModelType {
    Spherical,
    Exponential,
    Gaussian,
    Linear,
}

#[derive(Debug, Clone, Copy)]
pub struct VariogramModel {
    pub model_type: VariogramModelType,
    pub nugget: f64, // c0
    pub sill: f64,   // c (partial sill is sill - nugget)
    pub range: f64,  // a
}

impl VariogramModel {
    pub fn new(model_type: VariogramModelType, nugget: f64, sill: f64, range: f64) -> Self {
        Self { model_type, nugget, sill, range }
    }

    pub fn compute_semivariance(&self, h: f64) -> f64 {
        if h == 0.0 {
            return 0.0;
        }
        let c = self.sill - self.nugget; // Partial sill
        match self.model_type {
            VariogramModelType::Spherical => {
                if h <= self.range {
                    self.nugget + c * (1.5 * (h / self.range) - 0.5 * (h / self.range).powi(3))
                } else {
                    self.sill
                }
            }
            VariogramModelType::Exponential => {
                self.nugget + c * (1.0 - (-3.0 * h / self.range).exp())
            }
            VariogramModelType::Gaussian => {
                self.nugget + c * (1.0 - (-3.0 * (h / self.range).powi(2)).exp())
            }
            VariogramModelType::Linear => {
                self.nugget + c * (h / self.range)
            }
        }
    }
    
    // Covariance is Sill - Semivariance
    pub fn compute_covariance(&self, h: f64) -> f64 {
        self.sill - self.compute_semivariance(h)
    }
}

pub fn compute_empirical_variogram(pts: &[XYZPoint], n_lags: usize, max_dist: f64) -> (Vec<f64>, Vec<f64>, Vec<usize>) {
    let lag_dist = max_dist / (n_lags as f64);
    let mut sums = vec![0.0; n_lags];
    let mut counts = vec![0; n_lags];
    
    for i in 0..pts.len() {
        for j in (i + 1)..pts.len() {
            let dx = pts[i].x - pts[j].x;
            let dy = pts[i].y - pts[j].y;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist <= max_dist {
                let bin = (dist / lag_dist).floor() as usize;
                let bin = bin.min(n_lags - 1);
                let diff = pts[i].z - pts[j].z;
                sums[bin] += diff * diff;
                counts[bin] += 1;
            }
        }
    }
    
    let mut lags = Vec::with_capacity(n_lags);
    let mut gamma = Vec::with_capacity(n_lags);
    let mut valid_counts = Vec::with_capacity(n_lags);
    
    for k in 0..n_lags {
        if counts[k] > 0 {
            lags.push((k as f64 + 0.5) * lag_dist);
            gamma.push(sums[k] / (2.0 * counts[k] as f64));
            valid_counts.push(counts[k]);
        }
    }
    
    (lags, gamma, valid_counts)
}

pub fn interpolate_idw(
    pts: &[XYZPoint],
    targets: &[(f64, f64)],
    power: f64,
    search_radius: f64,
) -> Vec<f64> {
    targets.par_iter().map(|&(tx, ty)| {
        let mut sum_w = 0.0;
        let mut sum_wz = 0.0;
        let mut exact = false;
        let mut exact_val = 0.0;
        
        for pt in pts {
            let dx = pt.x - tx;
            let dy = pt.y - ty;
            let dist = (dx * dx + dy * dy).sqrt();
            
            if dist < 1e-10 {
                exact = true;
                exact_val = pt.z;
                break;
            }
            if dist <= search_radius {
                let w = 1.0 / dist.powf(power);
                sum_w += w;
                sum_wz += w * pt.z;
            }
        }
        
        if exact {
            exact_val
        } else if sum_w > 0.0 {
            sum_wz / sum_w
        } else {
            f64::NAN
        }
    }).collect()
}

pub fn interpolate_simple_kriging(
    pts: &[XYZPoint],
    targets: &[(f64, f64)],
    model: &VariogramModel,
    mean_z: f64,
) -> Result<Vec<f64>, String> {
    let n = pts.len();
    if n == 0 {
        return Err("No observation points provided".to_string());
    }

    // Build covariance matrix K for observations
    let mut cov_matrix = DMatrix::<f64>::zeros(n, n);
    for i in 0..n {
        for j in 0..n {
            if i == j {
                cov_matrix[(i, j)] = model.compute_covariance(0.0);
            } else {
                let dx = pts[i].x - pts[j].x;
                let dy = pts[i].y - pts[j].y;
                let h = (dx * dx + dy * dy).sqrt();
                cov_matrix[(i, j)] = model.compute_covariance(h);
            }
        }
    }

    // Compute inverse of K
    let k_inv = cov_matrix.try_inverse().ok_or("Failed to invert covariance matrix. Points might be collinear or identical.")?;

    // Precompute observation vector minus mean
    let z_vec = DVector::from_iterator(n, pts.iter().map(|p| p.z - mean_z));
    
    // Weights K^-1 * z
    let weights = &k_inv * z_vec;

    let results = targets.par_iter().map(|&(tx, ty)| {
        let mut k_target = DVector::<f64>::zeros(n);
        for i in 0..n {
            let dx = pts[i].x - tx;
            let dy = pts[i].y - ty;
            let h = (dx * dx + dy * dy).sqrt();
            k_target[i] = model.compute_covariance(h);
        }
        
        // Z*(x) = m + sum( w_i * (Z_i - m) )
        // sum( w_i * (Z_i - m) ) = k_target^T * K^-1 * z = k_target^T * weights
        let val = k_target.dot(&weights);
        mean_z + val
    }).collect();

    Ok(results)
}

pub fn interpolate_universal_kriging(
    pts: &[XYZPoint],
    targets: &[(f64, f64)],
    model: &VariogramModel,
) -> Result<Vec<f64>, String> {
    let n = pts.len();
    if n == 0 {
        return Err("No observation points provided".to_string());
    }
    
    // Universal Kriging with linear drift: f0(x) = 1, f1(x) = x, f2(x) = y
    // Matrix size: (n + 3) x (n + 3)
    let n_drift = 3; 
    let mut mat = DMatrix::<f64>::zeros(n + n_drift, n + n_drift);
    
    for i in 0..n {
        for j in 0..n {
            let dx = pts[i].x - pts[j].x;
            let dy = pts[i].y - pts[j].y;
            let h = (dx * dx + dy * dy).sqrt();
            mat[(i, j)] = model.compute_semivariance(h);
        }
        // Drift functions
        mat[(i, n)] = 1.0;
        mat[(i, n + 1)] = pts[i].x;
        mat[(i, n + 2)] = pts[i].y;
        
        mat[(n, i)] = 1.0;
        mat[(n + 1, i)] = pts[i].x;
        mat[(n + 2, i)] = pts[i].y;
    }
    // Bottom right n_drift x n_drift stays 0

    let mat_inv = mat.try_inverse().ok_or("Failed to invert UK matrix. Points might be collinear or identical.")?;
    
    let z_vec = DVector::from_iterator(n + n_drift, pts.iter().map(|p| p.z).chain(std::iter::repeat(0.0).take(n_drift)));
    let weights = &mat_inv * z_vec;

    let results = targets.par_iter().map(|&(tx, ty)| {
        let mut d_target = DVector::<f64>::zeros(n + n_drift);
        for i in 0..n {
            let dx = pts[i].x - tx;
            let dy = pts[i].y - ty;
            let h = (dx * dx + dy * dy).sqrt();
            d_target[i] = model.compute_semivariance(h);
        }
        d_target[n] = 1.0;
        d_target[n + 1] = tx;
        d_target[n + 2] = ty;
        
        d_target.dot(&weights)
    }).collect();

    Ok(results)
}
