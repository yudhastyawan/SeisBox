use statrs::distribution::{Normal, ContinuousCDF};
use rand::Rng;
use kiddo::KdTree;

pub struct BVorLikelihood {
    pub m: Vec<f64>,
}

impl BVorLikelihood {
    fn cost(&self, theta: &[f64; 3]) -> f64 {
        let beta = theta[0];
        let mu = theta[1];
        let sigma = theta[2];

        let min_m = self.m.iter().copied().fold(f64::INFINITY, f64::min);
        let max_m = self.m.iter().copied().fold(f64::NEG_INFINITY, f64::max);

        // Soft penalty for bounds
        let mut penalty = 0.0;
        let beta_min = 0.2 * 10f64.ln();
        let beta_max = 1.8 * 10f64.ln();
        if beta < beta_min { penalty += (beta_min - beta) * 1e6; }
        if beta > beta_max { penalty += (beta - beta_max) * 1e6; }
        
        if mu < min_m { penalty += (min_m - mu) * 1e6; }
        if mu > max_m { penalty += (mu - max_m) * 1e6; }

        if sigma < 0.01 { penalty += (0.01 - sigma) * 1e6; }
        if sigma > 0.8 { penalty += (sigma - 0.8) * 1e6; }

        let beta_safe = beta.clamp(beta_min, beta_max);
        let mu_safe = mu.clamp(min_m, max_m);
        let sigma_safe = sigma.clamp(0.01, 0.8);

        let n = self.m.len() as f64;
        let a = beta_safe.ln();
        
        let normal = Normal::new(mu_safe, sigma_safe).unwrap();
        let mut sum_b = 0.0;
        let mut sum_bm = 0.0;
        
        for &m in &self.m {
            let cdf = normal.cdf(m).max(1e-12);
            sum_b += cdf.ln();
            sum_bm += beta_safe * m;
        }
        
        let neg_log_l = -n * a + (sum_bm - sum_b) - n * beta_safe * mu_safe + 0.5 * n * beta_safe * beta_safe * sigma_safe * sigma_safe;
        neg_log_l + penalty
    }
}

pub fn calc_recurrence(magnitudes: &[f64]) -> f64 {
    let min_mag = magnitudes.iter().copied().fold(f64::INFINITY, f64::min);
    let mean_mag = magnitudes.iter().copied().sum::<f64>() / magnitudes.len() as f64;
    std::f64::consts::LOG10_E / (mean_mag - min_mag)
}

fn nelder_mead_3d(cost_fn: &BVorLikelihood, mut simplex: Vec<[f64; 3]>, max_iter: usize) -> ([f64; 3], f64) {
    let mut f_vals: Vec<f64> = simplex.iter().map(|p| cost_fn.cost(p)).collect();
    
    let alpha = 1.0;
    let gamma = 2.0;
    let rho = 0.5;
    let sigma = 0.5;

    for _ in 0..max_iter {
        // Sort simplex based on f_vals
        let mut indices: Vec<usize> = (0..4).collect();
        indices.sort_by(|&a, &b| f_vals[a].partial_cmp(&f_vals[b]).unwrap());
        
        let mut new_simplex = vec![[0.0; 3]; 4];
        let mut new_f = vec![0.0; 4];
        for i in 0..4 {
            new_simplex[i] = simplex[indices[i]];
            new_f[i] = f_vals[indices[i]];
        }
        simplex = new_simplex;
        f_vals = new_f;
        
        let centroid = {
            let mut c = [0.0; 3];
            for i in 0..3 {
                for j in 0..3 { c[j] += simplex[i][j]; }
            }
            for j in 0..3 { c[j] /= 3.0; }
            c
        };
        
        let worst = simplex[3];
        
        let mut xr = [0.0; 3];
        for j in 0..3 { xr[j] = centroid[j] + alpha * (centroid[j] - worst[j]); }
        let fr = cost_fn.cost(&xr);
        
        if f_vals[0] <= fr && fr < f_vals[2] {
            simplex[3] = xr;
            f_vals[3] = fr;
            continue;
        }
        
        if fr < f_vals[0] {
            let mut xe = [0.0; 3];
            for j in 0..3 { xe[j] = centroid[j] + gamma * (xr[j] - centroid[j]); }
            let fe = cost_fn.cost(&xe);
            if fe < fr {
                simplex[3] = xe;
                f_vals[3] = fe;
            } else {
                simplex[3] = xr;
                f_vals[3] = fr;
            }
            continue;
        }
        
        let mut xc = [0.0; 3];
        for j in 0..3 { xc[j] = centroid[j] + rho * (worst[j] - centroid[j]); }
        let fc = cost_fn.cost(&xc);
        
        if fc < f_vals[3] {
            simplex[3] = xc;
            f_vals[3] = fc;
            continue;
        }
        
        for i in 1..4 {
            for j in 0..3 {
                simplex[i][j] = simplex[0][j] + sigma * (simplex[i][j] - simplex[0][j]);
            }
            f_vals[i] = cost_fn.cost(&simplex[i]);
        }
    }
    
    (simplex[0], f_vals[0])
}

pub fn optimize_b_value(magnitudes: &[f64]) -> Option<(f64, f64, f64, f64)> {
    if magnitudes.len() < 5 {
        return None;
    }

    let min_mag = magnitudes.iter().copied().fold(f64::INFINITY, f64::min);
    let max_mag = magnitudes.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let mean_mag = magnitudes.iter().copied().sum::<f64>() / magnitudes.len() as f64;
    
    let mut sum_sq = 0.0;
    for &m in magnitudes {
        sum_sq += (m - mean_mag) * (m - mean_mag);
    }
    let std_mag = (sum_sq / (magnitudes.len() as f64)).sqrt().clamp(0.01, 0.8);

    let b_value_0 = calc_recurrence(magnitudes);
    let beta_0 = 10f64.ln() * b_value_0;
    
    let theta0 = [beta_0, mean_mag, std_mag];

    let mut simplex = vec![theta0];
    for i in 0..3 {
        let mut t = theta0;
        t[i] += if t[i] == 0.0 { 0.05 } else { t[i] * 0.05 };
        simplex.push(t);
    }

    let cost_fn = BVorLikelihood { m: magnitudes.to_vec() };
    
    let (best, best_cost) = nelder_mead_3d(&cost_fn, simplex, 1000);
    
    let b = best[0] / 10f64.ln();
    let mu = best[1];
    let sig = best[2];
    Some((b, mu, sig, best_cost))
}

pub struct VoronoiResult {
    pub b_all: Vec<f64>,
    pub mu_all: Vec<f64>,
    pub sig_all: Vec<f64>,
    pub voronoi_points: Vec<[f64; 2]>,
    pub kdtree: KdTree<f64, 2>,
    pub ln_l_all: Vec<f64>,
    pub n_all: Vec<f64>,
    pub bic: f64,
}

pub fn b_voronoi(x: &[f64], y: &[f64], m: &[f64], n_voronoi: usize, min_obs: usize, init: &str) -> Option<VoronoiResult> {
    if x.len() != y.len() || x.len() != m.len() || x.is_empty() {
        return None;
    }
    
    let mut rng = rand::thread_rng();
    let mut voronoi_points = Vec::with_capacity(n_voronoi);
    
    let x_min = x.iter().copied().fold(f64::INFINITY, f64::min);
    let x_max = x.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let y_min = y.iter().copied().fold(f64::INFINITY, f64::min);
    let y_max = y.iter().copied().fold(f64::NEG_INFINITY, f64::max);

    if init == "uniform" {
        for _ in 0..n_voronoi {
            voronoi_points.push([
                rng.gen_range(x_min..=x_max),
                rng.gen_range(y_min..=y_max)
            ]);
        }
    } else if init == "sobol" {
        // Halton sequence as a low-discrepancy substitute for Sobol
        let halton = |index: usize, base: usize| -> f64 {
            let mut f = 1.0;
            let mut r = 0.0;
            let mut i = index;
            while i > 0 {
                f /= base as f64;
                r += f * (i % base) as f64;
                i /= base;
            }
            r
        };
        let n_sobol = 1 << ((n_voronoi as f64).log2().floor() as usize + 1);
        let mut pts = Vec::with_capacity(n_sobol);
        for i in 1..=n_sobol {
            pts.push([
                x_min + halton(i, 2) * (x_max - x_min),
                y_min + halton(i, 3) * (y_max - y_min)
            ]);
        }
        use rand::seq::SliceRandom;
        pts.shuffle(&mut rng);
        voronoi_points.extend_from_slice(&pts[..n_voronoi]);
    } else if init == "data" {
        use rand::seq::SliceRandom;
        let mut indices: Vec<usize> = (0..x.len()).collect();
        indices.shuffle(&mut rng);
        for i in 0..n_voronoi.min(x.len()) {
            let idx = indices[i];
            voronoi_points.push([x[idx], y[idx]]);
        }
    } else if init == "kmeans" {
        // K-Means++ initialization
        let mut centers = Vec::with_capacity(n_voronoi);
        centers.push([x[rng.gen_range(0..x.len())], y[rng.gen_range(0..x.len())]]);
        for _ in 1..n_voronoi {
            let mut dists = Vec::with_capacity(x.len());
            for i in 0..x.len() {
                let mut min_d = f64::INFINITY;
                for c in &centers {
                    let d = (x[i] - c[0]).powi(2) + (y[i] - c[1]).powi(2);
                    if d < min_d { min_d = d; }
                }
                dists.push(min_d);
            }
            let dist_sum: f64 = dists.iter().sum();
            let mut r = rng.gen_range(0.0..dist_sum);
            let mut next_idx = x.len() - 1;
            for (i, &d) in dists.iter().enumerate() {
                r -= d;
                if r <= 0.0 {
                    next_idx = i;
                    break;
                }
            }
            centers.push([x[next_idx], y[next_idx]]);
        }
        // Lloyd's algorithm 10 iterations
        for _ in 0..10 {
            let mut new_centers = vec![[0.0, 0.0]; n_voronoi];
            let mut counts = vec![0; n_voronoi];
            for i in 0..x.len() {
                let mut min_d = f64::INFINITY;
                let mut best_c = 0;
                for (j, c) in centers.iter().enumerate() {
                    let d = (x[i] - c[0]).powi(2) + (y[i] - c[1]).powi(2);
                    if d < min_d {
                        min_d = d;
                        best_c = j;
                    }
                }
                new_centers[best_c][0] += x[i];
                new_centers[best_c][1] += y[i];
                counts[best_c] += 1;
            }
            for j in 0..n_voronoi {
                if counts[j] > 0 {
                    centers[j][0] = new_centers[j][0] / counts[j] as f64;
                    centers[j][1] = new_centers[j][1] / counts[j] as f64;
                }
            }
        }
        voronoi_points = centers;
    } else if init == "kde" {
        let n_f = x.len() as f64;
        let mut x_mean = 0.0;
        let mut y_mean = 0.0;
        for i in 0..x.len() { x_mean += x[i]; y_mean += y[i]; }
        x_mean /= n_f; y_mean /= n_f;
        
        let mut x_var = 0.0;
        let mut y_var = 0.0;
        for i in 0..x.len() {
            x_var += (x[i] - x_mean).powi(2);
            y_var += (y[i] - y_mean).powi(2);
        }
        x_var /= n_f; y_var /= n_f;
        
        let h_x = x_var.sqrt() * n_f.powf(-1.0/6.0);
        let h_y = y_var.sqrt() * n_f.powf(-1.0/6.0);
        
        let evaluate_kde = |px: f64, py: f64| -> f64 {
            let mut sum = 0.0;
            for i in 0..x.len() {
                let dx = (px - x[i]) / h_x;
                let dy = (py - y[i]) / h_y;
                sum += (-0.5 * (dx*dx + dy*dy)).exp();
            }
            sum / (n_f * h_x * h_y * 2.0 * std::f64::consts::PI)
        };
        
        let mut samples = Vec::with_capacity(n_voronoi);
        let mut max_p = 0.0;
        for _ in 0..100 {
            let px = rng.gen_range(x_min..=x_max);
            let py = rng.gen_range(y_min..=y_max);
            let p = evaluate_kde(px, py);
            if p > max_p { max_p = p; }
        }
        
        while samples.len() < n_voronoi {
            let px = rng.gen_range(x_min..=x_max);
            let py = rng.gen_range(y_min..=y_max);
            let p = evaluate_kde(px, py);
            if max_p == 0.0 || rng.gen::<f64>() < (p / max_p) {
                samples.push([px, py]);
            }
        }
        voronoi_points = samples;
    } else {
        return None;
    }
    
    let mut kdtree: KdTree<f64, 2> = KdTree::new();
    for (i, &pt) in voronoi_points.iter().enumerate() {
        kdtree.add(&pt, i as u64);
    }
    
    let mut regions_data: Vec<Vec<f64>> = vec![Vec::new(); n_voronoi];
    
    for i in 0..x.len() {
        let pt = [x[i], y[i]];
        let nearest = kdtree.nearest_one::<kiddo::SquaredEuclidean>(&pt);
        regions_data[nearest.item as usize].push(m[i]);
    }
    
    let mut b_all = Vec::with_capacity(n_voronoi);
    let mut mu_all = Vec::with_capacity(n_voronoi);
    let mut sig_all = Vec::with_capacity(n_voronoi);
    let mut ln_l_all = Vec::with_capacity(n_voronoi);
    let mut n_all = Vec::with_capacity(n_voronoi);
    let mut bic = 0.0;
    
    for region_m in regions_data {
        if region_m.len() < min_obs {
            b_all.push(f64::NAN);
            mu_all.push(f64::NAN);
            sig_all.push(f64::NAN);
            ln_l_all.push(f64::NAN);
            n_all.push(f64::NAN);
            continue;
        }
        
        if let Some((b, mu, sig, cost)) = optimize_b_value(&region_m) {
            b_all.push(b);
            mu_all.push(mu);
            sig_all.push(sig);
            ln_l_all.push(-cost);
            n_all.push(region_m.len() as f64);
            bic += 2.0 * (-cost) + 3.0 * (region_m.len() as f64).ln();
        } else {
            b_all.push(f64::NAN);
            mu_all.push(f64::NAN);
            sig_all.push(f64::NAN);
            ln_l_all.push(f64::NAN);
            n_all.push(f64::NAN);
        }
    }
    
    Some(VoronoiResult {
        b_all,
        mu_all,
        sig_all,
        voronoi_points,
        kdtree,
        ln_l_all,
        n_all,
        bic
    })
}
