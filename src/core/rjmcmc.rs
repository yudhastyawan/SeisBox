use serde::{Serialize, Deserialize};
use std::process::Command;
use std::fs::File;
use std::io::{Write, Read};
use std::sync::mpsc::Sender;
use rand::Rng;
use rand_distr::{Normal, Uniform, Distribution};
use tempfile::tempdir;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RjmcmcConfig {
    pub obs_file: String,
    pub hvf_path: String,
    pub output_file: String,
    pub n_iter: usize,
    pub burnin: usize,
    pub thin: usize,
    pub vs_min: f64,
    pub vs_max: f64,
    pub h_min: f64,
    pub h_max: f64,
    pub min_layers: usize,
    pub max_layers: usize,
    pub min_total_depth: f64,
    pub max_total_depth: f64,
    pub prob_asc_vs: f64,
    pub prob_asc_h: f64,
    pub use_avg_vs: bool,
    pub avg_vs_depth: f64,
    pub avg_vs_min: f64,
    pub avg_vs_max: f64,
    pub n_initial_search: usize,
    pub f0_min: f64,
    pub f0_max: f64,
    pub f0_weight: f64,
    pub a0_weight: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RjmcmcSample {
    pub iter: usize,
    pub n_layers: usize,
    pub rmse: f64,
    pub log_likelihood: f64,
    pub vs: Vec<f64>,
    pub h: Vec<f64>,
    pub h_syn: Vec<f64>,
}

pub struct Model {
    pub vs: Vec<f64>,
    pub h: Vec<f64>,
}

impl Model {
    pub fn clone(&self) -> Self {
        Self {
            vs: self.vs.clone(),
            h: self.h.clone(),
        }
    }
}

pub fn calculate_average_vs(vs: &[f64], h: &[f64], target_depth: f64) -> f64 {
    let mut time_sum = 0.0;
    let mut current_depth = 0.0;
    
    for i in 0..h.len() {
        let h_layer = h[i];
        let vs_layer = vs[i];
        
        if current_depth + h_layer >= target_depth {
            let h_effective = target_depth - current_depth;
            time_sum += h_effective / vs_layer;
            current_depth = target_depth;
            break;
        } else {
            time_sum += h_layer / vs_layer;
            current_depth += h_layer;
        }
    }
    
    if current_depth < target_depth {
        let h_remaining = target_depth - current_depth;
        let vs_halfspace = vs.last().copied().unwrap_or(1000.0);
        time_sum += h_remaining / vs_halfspace;
    }
    
    if time_sum > 0.0 { target_depth / time_sum } else { 0.0 }
}

pub fn get_peak_data(freq: &[f64], amp: &[f64], f_min: f64, f_max: f64) -> (f64, f64) {
    let mut max_amp = -1.0;
    let mut max_f = f64::NAN;
    
    for i in 0..freq.len() {
        if freq[i] >= f_min && freq[i] <= f_max {
            if amp[i] > max_amp {
                max_amp = amp[i];
                max_f = freq[i];
            }
        }
    }
    (max_f, max_amp)
}

pub fn forward_hvsr(vs: &[f64], h: &[f64], freq: &[f64], cfg: &RjmcmcConfig) -> Option<Vec<f64>> {
    let n_layers = vs.len();
    
    let mut vp = Vec::with_capacity(n_layers);
    let mut rho = Vec::with_capacity(n_layers);
    
    for &v in vs {
        let v_km = v / 1000.0;
        let v_p_km = 0.9409 + 2.0947 * v_km - 0.8206 * v_km.powi(2) + 0.2683 * v_km.powi(3) - 0.0251 * v_km.powi(4);
        vp.push(v_p_km * 1000.0);
        
        let r = 1.6612 * v_p_km - 0.4721 * v_p_km.powi(2) + 0.0671 * v_p_km.powi(3) - 0.0043 * v_p_km.powi(4) + 0.000106 * v_p_km.powi(5);
        rho.push(r * 1000.0);
    }
    
    let mut h_all = h.to_vec();
    h_all.push(0.0); // half-space
    
    let dir = match tempdir() {
        Ok(d) => d,
        Err(_) => return None,
    };
    
    let ffrec_path = dir.path().join("ffrec");
    let modl_path = dir.path().join("modl");
    
    if let Ok(mut f) = File::create(&ffrec_path) {
        for &fq in freq {
            let _ = writeln!(f, "{}", fq);
        }
    } else {
        return None;
    }
    
    if let Ok(mut f) = File::create(&modl_path) {
        let _ = writeln!(f, "{}", n_layers);
        for i in 0..n_layers {
            let _ = writeln!(f, "{:.6}\t{:.6}\t{:.6}\t{:.2}", h_all[i], vp[i], vs[i], rho[i]);
        }
    } else {
        return None;
    }
    
    let out = Command::new(&cfg.hvf_path)
        .args(&[
            "-nmr", "5", "-nml", "5", "-nks", "1000",
            "-apsv", "0.005", "-ash", "0.01", "-hv",
            "-ff", ffrec_path.to_str().unwrap(),
            "-f", modl_path.to_str().unwrap()
        ])
        .output();
        
    match out {
        Ok(output) => {
            let s = String::from_utf8_lossy(&output.stdout);
            let vals: Vec<f64> = s.split_whitespace().filter_map(|x| x.parse().ok()).collect();
            if vals.is_empty() {
                return None;
            }
            // Return every 2nd value (amp)
            let mut amp = Vec::with_capacity(vals.len() / 2);
            for i in (1..vals.len()).step_by(2) {
                amp.push(vals[i]);
            }
            Some(amp)
        }
        Err(_) => None,
    }
}

pub fn log_prior(model: &Model, cfg: &RjmcmcConfig) -> f64 {
    let n_vs = model.vs.len();
    
    if n_vs < cfg.min_layers || n_vs > cfg.max_layers {
        return f64::NEG_INFINITY;
    }
    
    let total_depth: f64 = model.h.iter().sum();
    if total_depth < cfg.min_total_depth || total_depth > cfg.max_total_depth {
        return f64::NEG_INFINITY;
    }
    
    if cfg.use_avg_vs {
        let avg_val = calculate_average_vs(&model.vs, &model.h, cfg.avg_vs_depth);
        if avg_val < cfg.avg_vs_min || avg_val > cfg.avg_vs_max {
            return f64::NEG_INFINITY;
        }
    }
    
    for i in 0..n_vs {
        if model.vs[i] < cfg.vs_min || model.vs[i] > cfg.vs_max {
            return f64::NEG_INFINITY;
        }
        if i < n_vs - 1 {
            if model.h[i] < cfg.h_min || model.h[i] > cfg.h_max {
                return f64::NEG_INFINITY;
            }
        }
    }
    
    0.0
}

pub fn log_likelihood(model: &Model, freq: &[f64], h_obs: &[f64], obs_f0: f64, obs_a0: f64, cfg: &RjmcmcConfig) -> (f64, Vec<f64>) {
    let h_syn = forward_hvsr(&model.vs, &model.h, freq, cfg);
    match h_syn {
        Some(syn) => {
            if syn.len() != h_obs.len() || syn.iter().any(|x| x.is_nan()) {
                return (f64::NEG_INFINITY, syn);
            }
            
            let obs_mean = h_obs.iter().sum::<f64>() / h_obs.len() as f64;
            let mut misfit_amp = 0.0;
            for i in 0..h_obs.len() {
                let weight = h_obs[i] / obs_mean;
                misfit_amp += weight * (h_obs[i] - syn[i]).powi(2);
            }
            
            let mut misfit_f0 = 0.0;
            let mut misfit_a0 = 0.0;
            
            if cfg.f0_weight > 0.0 || cfg.a0_weight > 0.0 {
                let (syn_f0, syn_a0) = get_peak_data(freq, &syn, cfg.f0_min, cfg.f0_max);
                if !syn_f0.is_nan() && !obs_f0.is_nan() {
                    misfit_f0 = (obs_f0 - syn_f0).powi(2);
                    if cfg.a0_weight > 0.0 {
                        misfit_a0 = (obs_a0 - syn_a0).powi(2);
                    }
                }
            }
            
            let total_misfit = misfit_amp + (cfg.f0_weight * misfit_f0) + (cfg.a0_weight * misfit_a0);
            (-0.5 * total_misfit, syn)
        }
        None => (f64::NEG_INFINITY, Vec::new()),
    }
}

pub fn sample_prior_model(cfg: &RjmcmcConfig, rng: &mut impl Rng) -> Model {
    let n_layers = rng.gen_range(cfg.min_layers..=cfg.max_layers);
    let mut vs = Vec::with_capacity(n_layers);
    let mut h = Vec::with_capacity(n_layers - 1);
    
    for _ in 0..n_layers {
        vs.push(rng.gen_range(cfg.vs_min..=cfg.vs_max));
    }
    
    // Sort vs according to probability
    if rng.gen_bool(cfg.prob_asc_vs) {
        vs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    }
    
    for _ in 0..(n_layers - 1) {
        h.push(rng.gen_range(cfg.h_min..=cfg.h_max));
    }
    
    if rng.gen_bool(cfg.prob_asc_h) {
        h.sort_by(|a, b| a.partial_cmp(b).unwrap());
    }
    
    Model { vs, h }
}

pub fn propose_update(current: &Model, cfg: &RjmcmcConfig, rng: &mut impl Rng) -> Model {
    let mut proposed = current.clone();
    let n_vs = proposed.vs.len();
    
    let move_type = rng.gen_range(0..3);
    
    if move_type == 0 || move_type == 2 {
        // Update Vs
        let layer_idx = rng.gen_range(0..n_vs);
        let perturb = rng.sample(Normal::new(0.0, (cfg.vs_max - cfg.vs_min) * 0.1).unwrap());
        proposed.vs[layer_idx] += perturb;
    }
    
    if move_type == 1 || move_type == 2 {
        // Update h
        if n_vs > 1 {
            let layer_idx = rng.gen_range(0..(n_vs - 1));
            let perturb = rng.sample(Normal::new(0.0, (cfg.h_max - cfg.h_min) * 0.1).unwrap());
            proposed.h[layer_idx] += perturb;
        }
    }
    
    proposed
}

pub fn propose_birth(current: &Model, cfg: &RjmcmcConfig, rng: &mut impl Rng) -> Model {
    let mut proposed = current.clone();
    let n_vs = proposed.vs.len();
    
    if n_vs >= cfg.max_layers {
        return proposed;
    }
    
    let insert_idx = rng.gen_range(0..n_vs);
    let new_vs = rng.gen_range(cfg.vs_min..=cfg.vs_max);
    proposed.vs.insert(insert_idx, new_vs);
    
    let new_h = rng.gen_range(cfg.h_min..=cfg.h_max);
    if insert_idx < proposed.h.len() {
        proposed.h.insert(insert_idx, new_h);
    } else {
        proposed.h.push(new_h);
    }
    
    proposed
}

pub fn propose_death(current: &Model, cfg: &RjmcmcConfig, rng: &mut impl Rng) -> Model {
    let mut proposed = current.clone();
    let n_vs = proposed.vs.len();
    
    if n_vs <= cfg.min_layers {
        return proposed;
    }
    
    let del_idx = rng.gen_range(0..n_vs);
    proposed.vs.remove(del_idx);
    
    if del_idx < proposed.h.len() {
        proposed.h.remove(del_idx);
    } else if !proposed.h.is_empty() {
        proposed.h.pop();
    }
    
    proposed
}

pub fn run_inversion(cfg: RjmcmcConfig, tx: Sender<String>) {
    let mut rng = rand::thread_rng();
    
    let _ = tx.send(format!("Starting RJ-MCMC Inversion..."));
    
    // Read observation data
    let mut freq = Vec::new();
    let mut h_obs = Vec::new();
    let mut obs_f0 = f64::NAN;
    let mut obs_a0 = f64::NAN;
    
    match csv::Reader::from_path(&cfg.obs_file) {
        Ok(mut rdr) => {
            for result in rdr.records() {
                if let Ok(record) = result {
                    if let (Ok(f), Ok(a)) = (record[0].parse::<f64>(), record[1].parse::<f64>()) {
                        freq.push(f);
                        h_obs.push(a);
                    }
                }
            }
        }
        Err(e) => {
            let _ = tx.send(format!("Error reading {}: {}", cfg.obs_file, e));
            return;
        }
    }
    
    if freq.is_empty() {
        let _ = tx.send("Error: No data found in observation file.".to_string());
        return;
    }
    
    if cfg.f0_weight > 0.0 || cfg.a0_weight > 0.0 {
        let (f0, a0) = get_peak_data(&freq, &h_obs, cfg.f0_min, cfg.f0_max);
        obs_f0 = f0;
        obs_a0 = a0;
        let _ = tx.send(format!("Target f0: {:.3} Hz, A0: {:.3}", obs_f0, obs_a0));
    }
    
    // Initial Search
    let _ = tx.send(format!("Searching for initial model ({} attempts)...", cfg.n_initial_search));
    let mut best_model = sample_prior_model(&cfg, &mut rng);
    let mut best_logL = f64::NEG_INFINITY;
    let mut best_syn = Vec::new();
    
    for i in 1..=cfg.n_initial_search {
        let model = sample_prior_model(&cfg, &mut rng);
        if log_prior(&model, &cfg) > f64::NEG_INFINITY {
            let (logL, syn) = log_likelihood(&model, &freq, &h_obs, obs_f0, obs_a0, &cfg);
            if logL > best_logL {
                best_logL = logL;
                best_model = model;
                best_syn = syn;
            }
        }
    }
    
    if best_logL == f64::NEG_INFINITY {
        let _ = tx.send("Error: Failed to find valid initial model.".to_string());
        return;
    }
    
    let mut current_model = best_model;
    let mut current_logL = best_logL;
    let mut current_syn = best_syn;
    
    let mut accepted = 0;
    
    // Open output file
    let mut out_file = match File::create(&cfg.output_file) {
        Ok(f) => f,
        Err(e) => {
            let _ = tx.send(format!("Error creating output file: {}", e));
            return;
        }
    };
    
    let _ = tx.send("Starting MCMC iterations...".to_string());
    
    for i in 1..=cfg.n_iter {
        // Propose new model
        let move_type = rng.gen_range(0..100);
        let proposed_model = if move_type < 70 {
            propose_update(&current_model, &cfg, &mut rng)
        } else if move_type < 85 {
            propose_birth(&current_model, &cfg, &mut rng)
        } else {
            propose_death(&current_model, &cfg, &mut rng)
        };
        
        let prior = log_prior(&proposed_model, &cfg);
        if prior > f64::NEG_INFINITY {
            let (logL, syn) = log_likelihood(&proposed_model, &freq, &h_obs, obs_f0, obs_a0, &cfg);
            if logL > f64::NEG_INFINITY {
                // Hastings Ratio
                let log_alpha = logL - current_logL;
                let log_u = (rng.gen::<f64>()).ln();
                
                if log_u < log_alpha {
                    current_model = proposed_model;
                    current_logL = logL;
                    current_syn = syn;
                    if i > cfg.burnin { accepted += 1; }
                }
            }
        }
        
        if i > cfg.burnin && i % cfg.thin == 0 {
            let rmse = (h_obs.iter().zip(&current_syn).map(|(a, b)| (a - b).powi(2)).sum::<f64>() / h_obs.len() as f64).sqrt();
            let sample = RjmcmcSample {
                iter: i,
                n_layers: current_model.vs.len(),
                rmse,
                log_likelihood: current_logL,
                vs: current_model.vs.clone(),
                h: current_model.h.clone(),
                h_syn: current_syn.clone(),
            };
            let json = serde_json::to_string(&sample).unwrap();
            let _ = writeln!(out_file, "{}", json);
        }
        
        if i % 100 == 0 {
            let acc_rate = if i > cfg.burnin { (accepted as f64 / (i - cfg.burnin) as f64) * 100.0 } else { 0.0 };
            let rmse = (h_obs.iter().zip(&current_syn).map(|(a, b)| (a - b).powi(2)).sum::<f64>() / h_obs.len() as f64).sqrt();
            let move_str = if move_type < 70 { "Update" } else if move_type < 85 { "Birth " } else { "Death " };
            let _ = tx.send(format!("Iter: {:6} | Lapis: {:2} | RMSE: {:.4} | LogL: {:.2} | Acc: {:.1}% | Move type: {}", 
                    i, current_model.vs.len(), rmse, current_logL, acc_rate, move_str));
        }
    }
    
    let _ = tx.send("DONE".to_string());
}
