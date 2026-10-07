use serde::{Serialize, Deserialize};
use std::fs::File;
use std::io::{Write, Read};
use std::sync::mpsc::Sender;
use rand::Rng;
use rand_distr::{Normal, Uniform};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RjmcmcConfig {
    pub obs_file: String,
    pub hvf_path: String,
    pub output_file: String,
    pub n_iter: usize,
    pub burnin: usize,
    pub thin: usize,
    #[serde(default = "default_fmin")]
    pub fmin: f64,
    #[serde(default = "default_fmax")]
    pub fmax: f64,
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
    #[serde(default = "default_f0_weight")]
    pub f0_weight: f64,
    #[serde(default = "default_a0_weight")]
    pub a0_weight: f64,
    #[serde(default)]
    pub vp_expr: Option<String>,
    #[serde(default)]
    pub rho_expr: Option<String>,
    #[serde(default = "default_method")]
    pub method: String,
    #[serde(default = "default_love_alpha")]
    pub love_alpha: f64,
    #[serde(default)]
    pub plot_dir: Option<String>,
}

fn default_fmin() -> f64 { 0.0 }
fn default_fmax() -> f64 { 100.0 }

fn default_f0_weight() -> f64 { 0.0 }
fn default_a0_weight() -> f64 { 0.0 }
fn default_method() -> String { "dfa".to_string() }
fn default_love_alpha() -> f64 { 0.5 }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RjmcmcSample {
    pub iter: usize,
    pub n_layers: usize,
    pub rmse: f64,
    pub log_likelihood: f64,
    pub vs: Vec<f64>,
    pub h: Vec<f64>,
    pub h_syn: Vec<f64>,
    
    #[serde(default, alias="Vs30", alias="vs30", alias="VS30")]
    pub vs30: Option<f64>,
    #[serde(default, alias="H800", alias="h800", alias="z800", alias="Z800")]
    pub h800: Option<f64>,
    #[serde(default, alias="Z1.0", alias="z1.0", alias="z1_0", alias="Z1000", alias="z1000")]
    pub z1_0: Option<f64>,
    #[serde(default, alias="Z2.5", alias="z2.5", alias="z2_5", alias="Z2500", alias="z2500")]
    pub z2_5: Option<f64>,
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

pub fn forward_hvsr(
    vs: &[f64], h: &[f64], freq: &[f64], cfg: &RjmcmcConfig,
    vp_func: Option<&Box<dyn Fn(f64) -> f64>>,
    rho_func: Option<&Box<dyn Fn(f64, f64) -> f64>>
) -> Option<Vec<f64>> {
    let mut layers = Vec::new();
    for i in 0..vs.len() {
        let current_vs = vs[i];
        let vp = if let Some(ref func) = vp_func {
            func(current_vs)
        } else {
            let v_km = current_vs / 1000.0;
            let v_p_km = 0.9409 + 2.0947 * v_km - 0.8206 * v_km.powi(2) + 0.2683 * v_km.powi(3) - 0.0251 * v_km.powi(4);
            v_p_km * 1000.0
        };
        
        let rho = if let Some(ref func) = rho_func {
            func(vp, current_vs)
        } else {
            let v_p_km = vp / 1000.0;
            let r = 1.6612 * v_p_km - 0.4721 * v_p_km.powi(2) + 0.0671 * v_p_km.powi(3) - 0.0043 * v_p_km.powi(4) + 0.000106 * v_p_km.powi(5);
            r * 1000.0
        };
        
        let thickness = if i < h.len() { h[i] } else { 0.0 }; // Last layer is halfspace

        layers.push(crate::hvf::model::Layer {
            thickness,
            vp,
            vs: vs[i],
            density: rho,
            qp: None,
            qs: None,
        });
    }

    let earth_model = crate::hvf::model::EarthModel { layers };

    let config = crate::hvf::cli::Config {
        fmin: -1.0, fmax: -1.0, nf: 0, logsam: false, freq_file: None,
        model_file: None, model_json: None, method: cfg.method.clone(),
        qp: 100.0, qs: 50.0, kq: 0.0, fref: 0.0,
        nmr: 5, nml: 5, prec: 1e-4, nks: 1000,
        apsv: 0.005, ash: 0.01,
        output_hv: false, output_ph: false, output_gr: false, output_rep: false, output_json: false, output_file: None,
        invert: None, lm_lambda: 1.0, occam_alpha: 0.1, obs: None, bounds: None, initial_model: None, mcmc_config: None, pso_pop: 50, pso_iter: 100, pso_c1: 2.0, pso_c2: 2.0, pso_w: 0.9, sa_t_initial: 10.0, sa_t_final: 1e-4, sa_cooling_rate: 0.95,
        enforce_increasing_vs: false, enforce_increasing_h: false, constraint_mode: crate::hvf::constraints::ConstraintMode::Repair,
        use_brocher: false,
        vp_expr: cfg.vp_expr.clone(),
        rho_expr: cfg.rho_expr.clone(),
        love_alpha: cfg.love_alpha,
        plot_dir: None,
    };
    let omega: Vec<f64> = freq.iter().map(|f| f * 2.0 * std::f64::consts::PI).collect();
    
    let hv_ratio = if config.method == "herak" {
        crate::hvf::herak::compute_hvsr_herak(&earth_model, freq, &config)
    } else if config.method == "ellipticity" || config.method == "ellipticity-love" {
        let ellip_model: Vec<crate::hvf::ellipticity::EllipLayer> = earth_model.layers.iter().map(|l| crate::hvf::ellipticity::EllipLayer {
            h: l.thickness, vp: l.vp, vs: l.vs, rho: l.density, qp: config.qp, qs: config.qs,
        }).collect();
        let love_alpha = if config.method == "ellipticity-love" { config.love_alpha } else { 0.0 };
        crate::hvf::ellipticity::compute_ellipticity_curve(&ellip_model, freq, love_alpha).hv
    } else {
        crate::hvf::compute::compute_hvsr_dfa_internal(&earth_model, &omega, &config)
    };
    
    Some(hv_ratio)
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

pub fn log_likelihood(
    model: &Model, freq: &[f64], h_obs: &[f64], obs_f0: f64, obs_a0: f64, cfg: &RjmcmcConfig,
    vp_func: Option<&Box<dyn Fn(f64) -> f64>>,
    rho_func: Option<&Box<dyn Fn(f64, f64) -> f64>>
) -> (f64, Vec<f64>) {
    let h_syn = forward_hvsr(&model.vs, &model.h, freq, cfg, vp_func, rho_func);
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
    
    let vp_func_opt: Option<Box<dyn Fn(f64) -> f64>> = match &cfg.vp_expr {
        Some(s) if !s.trim().is_empty() => match s.parse::<meval::Expr>() {
            Ok(expr) => match expr.bind("vs") {
                Ok(f) => Some(Box::new(f) as Box<dyn Fn(f64) -> f64>),
                Err(e) => { let _ = tx.send(format!("Error binding vs to vp_expr: {}", e)); None }
            },
            Err(e) => { let _ = tx.send(format!("Error parsing vp_expr: {}", e)); None }
        },
        _ => None,
    };

    let rho_func_opt: Option<Box<dyn Fn(f64, f64) -> f64>> = match &cfg.rho_expr {
        Some(s) if !s.trim().is_empty() => match s.parse::<meval::Expr>() {
            Ok(expr) => match expr.bind2("vp", "vs") {
                Ok(f) => Some(Box::new(f) as Box<dyn Fn(f64, f64) -> f64>),
                Err(e) => { let _ = tx.send(format!("Error binding vp,vs to rho_expr: {}", e)); None }
            },
            Err(e) => { let _ = tx.send(format!("Error parsing rho_expr: {}", e)); None }
        },
        _ => None,
    };
    
    // Read observation data
    let mut freq = Vec::new();
    let mut h_obs = Vec::new();
    let mut obs_f0 = f64::NAN;
    let mut obs_a0 = f64::NAN;
    
    match std::fs::read_to_string(&cfg.obs_file) {
        Ok(content) => {
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') || line.to_lowercase().starts_with("freq") {
                    continue;
                }
                let parts: Vec<&str> = line.split(|c: char| c == ',' || c.is_whitespace()).filter(|s| !s.is_empty()).collect();
                if parts.len() >= 2 {
                    if let (Ok(f), Ok(a)) = (parts[0].parse::<f64>(), parts[1].parse::<f64>()) {
                        let fmin = if cfg.fmin <= 0.0 { 0.0 } else { cfg.fmin };
                        let fmax = if cfg.fmax <= 0.0 { f64::MAX } else { cfg.fmax };
                        if f >= fmin && f <= fmax {
                            freq.push(f);
                            h_obs.push(a);
                        }
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
            let (logL, syn) = log_likelihood(&model, &freq, &h_obs, obs_f0, obs_a0, &cfg, vp_func_opt.as_ref(), rho_func_opt.as_ref());
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

    // To track best model for plotting
    let mut absolute_best_log_l = current_logL;
    let mut absolute_best_model = current_model.clone();
    let mut absolute_best_syn = current_syn.clone();
    let mut cost_history = Vec::new();
    
    let mut accepted = 0;
    
    // Open output file
    let mut out_file = match File::create(&cfg.output_file) {
        Ok(f) => f,
        Err(e) => {
            let _ = tx.send(format!("Error creating output file: {}", e));
            return;
        }
    };
    
    // Write the inversion configuration as the first line in JSONL
    if let Ok(cfg_json) = serde_json::to_string(&cfg) {
        let _ = writeln!(out_file, "{{\"config\": {}}}", cfg_json);
    }
    
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
            let (logL, syn) = log_likelihood(&proposed_model, &freq, &h_obs, obs_f0, obs_a0, &cfg, vp_func_opt.as_ref(), rho_func_opt.as_ref());
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
        
        if current_logL > absolute_best_log_l {
            absolute_best_log_l = current_logL;
            absolute_best_model = current_model.clone();
            absolute_best_syn = current_syn.clone();
        }
        
        let rmse = (h_obs.iter().zip(&current_syn).map(|(a, b)| (a - b).powi(2)).sum::<f64>() / h_obs.len() as f64).sqrt();
        
        if i > cfg.burnin && i % cfg.thin == 0 {
            cost_history.push(rmse);
            let mut z_arr = vec![0.0];
            let mut sum_h = 0.0;
            for &thickness in &current_model.h {
                sum_h += thickness;
                z_arr.push(sum_h);
            }
            z_arr.push(sum_h + 10.0);
            
            let mut vs_arr = current_model.vs.clone();
            if vs_arr.len() < z_arr.len() {
                if let Some(&last) = vs_arr.last() {
                    vs_arr.push(last);
                }
            }
            
            let vs30 = crate::core::rjmcmc_stats::calc_vs30(&z_arr, &vs_arr);
            let h800 = crate::core::rjmcmc_stats::calc_depth_for_vs(&z_arr, &vs_arr, 800.0);
            let z1_0 = crate::core::rjmcmc_stats::calc_depth_for_vs(&z_arr, &vs_arr, 1000.0);
            let z2_5 = crate::core::rjmcmc_stats::calc_depth_for_vs(&z_arr, &vs_arr, 2500.0);

            let sample = RjmcmcSample {
                iter: i,
                n_layers: current_model.vs.len(),
                rmse,
                log_likelihood: current_logL,
                vs: current_model.vs.clone(),
                h: current_model.h.clone(),
                h_syn: current_syn.clone(),
                vs30: Some(vs30),
                h800: Some(h800),
                z1_0: Some(z1_0),
                z2_5: Some(z2_5),
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
    
    // Call plot if enabled
    if let Some(ref dir_str) = cfg.plot_dir {
        let plot_dir = std::path::Path::new(dir_str);
        if !plot_dir.exists() {
            let _ = std::fs::create_dir_all(plot_dir);
        }
        
        let fit_path = plot_dir.join("hv_fit.png");
        let _ = crate::hvf::plot::plot_hv_fit(&freq, &h_obs, &absolute_best_syn, &fit_path);
        
        // Convert internal absolute_best_model to Layer array
        let mut layers = Vec::new();
        for (idx, &v) in absolute_best_model.vs.iter().enumerate() {
            let vp = if let Some(ref func) = vp_func_opt {
                func(v)
            } else {
                let v_km = v / 1000.0;
                let v_p_km = 0.9409 + 2.0947 * v_km - 0.8206 * v_km.powi(2) + 0.2683 * v_km.powi(3) - 0.0251 * v_km.powi(4);
                v_p_km * 1000.0
            };
            let rho = if let Some(ref func) = rho_func_opt {
                func(vp, v)
            } else {
                let v_p_km = vp / 1000.0;
                let r = 1.6612 * v_p_km - 0.4721 * v_p_km.powi(2) + 0.0671 * v_p_km.powi(3) - 0.0043 * v_p_km.powi(4) + 0.000106 * v_p_km.powi(5);
                r * 1000.0
            };
            layers.push(crate::hvf::model::Layer {
                thickness: absolute_best_model.h[idx],
                vs: v,
                vp,
                density: rho,
                qp: None,
                qs: None,
            });
        }
        
        let vs_path = plot_dir.join("vs_profile.png");
        let _ = crate::hvf::plot::plot_vs_profile(&layers, &vs_path);
        
        let conv_path = plot_dir.join("convergence.png");
        let _ = crate::hvf::plot::plot_convergence(&cost_history, &conv_path);
    }
    
    let _ = tx.send("DONE".to_string());
}
