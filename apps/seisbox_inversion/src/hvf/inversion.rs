use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use crate::hvf::cli::Config;
use crate::hvf::model::{EarthModel, Layer};
use crate::hvf::pso::{run_pso, PsoConfig};
use crate::hvf::herak::compute_hvsr_herak;
use crate::hvf::compute::compute_hvsr_dfa_internal;

fn compute_hvsr_for_inversion(model: &EarthModel, freqs: &[f64], config: &Config) -> Vec<f64> {
    if config.method == "herak" {
        compute_hvsr_herak(model, freqs, config)
    } else if config.method == "ellipticity" || config.method == "ellipticity-love" {
        let ellip_model: Vec<crate::hvf::ellipticity::EllipLayer> = model.layers.iter().map(|l| crate::hvf::ellipticity::EllipLayer {
            h: l.thickness, vp: l.vp, vs: l.vs, rho: l.density, qp: config.qp, qs: config.qs,
        }).collect();
        let love_alpha = if config.method == "ellipticity-love" { config.love_alpha } else { 0.0 };
        crate::hvf::ellipticity::compute_ellipticity_curve(&ellip_model, freqs, love_alpha).hv
    } else {
        let omega: Vec<f64> = freqs.iter().map(|f| f * std::f64::consts::PI * 2.0).collect();
        compute_hvsr_dfa_internal(model, &omega, config)
    }
}
use crate::hvf::types::TWO_PI;

#[derive(Debug, Deserialize, Serialize)]
pub struct BoundsData {
    pub nlayers: usize,
    /// Minimum and maximum thickness for layers 0 to nlayers-2
    pub h_bounds: Vec<[f64; 2]>,
    /// Minimum and maximum Vs for layers 0 to nlayers-1
    pub vs_bounds: Vec<[f64; 2]>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub iter: usize,
    pub cost: f64,
    pub model: EarthModel,
    pub hvsr: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InversionResult {
    pub best_model: EarthModel,
    pub best_cost: f64,
    pub cost_history: Vec<f64>,
    #[serde(default)]
    pub freqs: Vec<f64>,
    pub estimated_hvsr: Vec<f64>,
    pub history: Vec<HistoryEntry>,
}

pub fn read_obs_data(path: &Path, fmin: f64, fmax: f64) -> Result<(Vec<f64>, Vec<f64>), String> {
    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut freqs = Vec::new();
    let mut hvs = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.to_lowercase().starts_with("freq") {
            continue;
        }
        let parts: Vec<&str> = line.split(|c: char| c == ',' || c.is_whitespace()).filter(|s| !s.is_empty()).collect();
        if parts.len() >= 2 {
            if let (Ok(f), Ok(h)) = (parts[0].parse::<f64>(), parts[1].parse::<f64>()) {
                let f_min_eff = if fmin <= 0.0 { 0.0 } else { fmin };
                let f_max_eff = if fmax <= 0.0 { f64::MAX } else { fmax };
                if f >= f_min_eff && f <= f_max_eff {
                    freqs.push(f);
                    hvs.push(h);
                }
            }
        }
    }
    Ok((freqs, hvs))
}

fn calc_vp(vs_ms: f64) -> f64 {
    let vs_kms = vs_ms / 1000.0;
    (0.9409 + 2.0947 * vs_kms - 0.8206 * vs_kms.powi(2) + 0.2683 * vs_kms.powi(3) - 0.0251 * vs_kms.powi(4)) * 1000.0
}

fn calc_rho(vp_ms: f64) -> f64 {
    let vp_kms = vp_ms / 1000.0;
    (1.6612 * vp_kms - 0.4721 * vp_kms.powi(2) + 0.0671 * vp_kms.powi(3) - 0.0043 * vp_kms.powi(4) + 0.000106 * vp_kms.powi(5)) * 1000.0
}

pub fn run_inversion(config: &Config, tx: Option<std::sync::mpsc::Sender<String>>) -> Result<(), String> {
    // Read bounds
    let bounds_path = config.bounds.as_ref().unwrap();
    let bounds_str = fs::read_to_string(bounds_path).map_err(|e| format!("Failed to read bounds: {}", e))?;
    let bounds_data: BoundsData = serde_json::from_str(&bounds_str).map_err(|e| format!("Invalid JSON bounds: {}", e))?;

    // Validate bounds length
    if bounds_data.h_bounds.len() != bounds_data.nlayers - 1 {
        return Err(format!("Expected {} h_bounds, got {}", bounds_data.nlayers - 1, bounds_data.h_bounds.len()));
    }
    if bounds_data.vs_bounds.len() != bounds_data.nlayers {
        return Err(format!("Expected {} vs_bounds, got {}", bounds_data.nlayers, bounds_data.vs_bounds.len()));
    }

    let mut pso_bounds = Vec::new();
    for b in &bounds_data.h_bounds {
        pso_bounds.push((b[0], b[1]));
    }
    for b in &bounds_data.vs_bounds {
        pso_bounds.push((b[0], b[1]));
    }

    // Read obs data
    let obs_path = config.obs.as_ref().unwrap();
    let (obs_freqs, obs_hvs) = read_obs_data(obs_path, config.fmin, config.fmax)?;
    if obs_freqs.is_empty() {
        return Err("Observation data is empty.".to_string());
    }

    let nlayers = bounds_data.nlayers;
    let is_herak = config.method == "herak";

    let forward_func = |pos: &[f64]| -> Vec<f64> {
        let mut layers = Vec::with_capacity(nlayers);
        for i in 0..nlayers {
            let h = if i < nlayers - 1 { pos[i] } else { 0.0 };
            let vs = pos[nlayers - 1 + i];
            let vp = calc_vp(vs);
            let rho = calc_rho(vp);
            layers.push(Layer {
                thickness: h,
                vp,
                vs,
                density: rho,
                qp: None,
                qs: None,
            });
        }
        let earth_model = EarthModel { layers };
        compute_hvsr_for_inversion(&earth_model, &obs_freqs, config)
    };

    let constraints = crate::hvf::constraints::Constraints::new(
        nlayers,
        config.enforce_increasing_vs,
        config.enforce_increasing_h,
        config.constraint_mode,
        pso_bounds.clone(),
    );
    if constraints.is_active() {
        let msg = format!(
            "Physical constraints: Vs non-decreasing = {}, Thickness non-decreasing = {}, mode = {}",
            config.enforce_increasing_vs, config.enforce_increasing_h, config.constraint_mode.label()
        );
        if let Some(ref tx_chan) = tx { let _ = tx_chan.send(msg); } else { eprintln!("{}", msg); }
        for w in constraints.bounds_warnings() {
            let msg = format!("Warning: {} (repair cannot guarantee feasibility; graded penalty may apply)", w);
            if let Some(ref tx_chan) = tx { let _ = tx_chan.send(msg); } else { eprintln!("{}", msg); }
        }
    }

    let obj_func = |pos: &[f64]| -> f64 {
        if let Some(p) = constraints.penalty(pos) {
            return p;
        }
        let est_hvsr = forward_func(pos);
        crate::hvf::deterministic::compute_rmse(&est_hvsr, &obs_hvs)
    };
    // Lamarckian repair for stochastic methods (sort + clamp); no-op if inactive / penalty mode.
    let repair_func = |pos: &mut [f64]| constraints.repair(pos);
    // Projection for gradient methods (isotonic + clamp); returns feasibility.
    let constrain_func = |pos: &mut [f64]| -> bool {
        constraints.project(pos);
        constraints.penalty(pos).is_none()
    };

    let initial_pos: Vec<f64> = if let Some(ref path) = config.initial_model {
        let load_msg = format!("Loading initial model from: {:?}", path);
        if let Some(ref tx_chan) = tx { let _ = tx_chan.send(load_msg.clone()); }
        eprintln!("{}", load_msg);
        
        let content = fs::read_to_string(path).expect("Failed to read initial model file");
        let mut model = if let Ok(result) = serde_json::from_str::<InversionResult>(&content) {
            result.best_model
        } else {
            EarthModel::from_file(path, config.use_brocher, config.vp_expr.as_deref(), config.rho_expr.as_deref())
                .expect("Failed to load initial model")
        };
        
        if model.layers.len() != nlayers {
            use rand::Rng;
            let mut rng = rand::thread_rng();
            let orig_len = model.layers.len();
            
            while model.layers.len() > nlayers {
                let n = model.layers.len();
                let idx = rng.gen_range(0..n - 1);
                let l1 = &model.layers[idx];
                let l2 = &model.layers[idx + 1];
                let t1 = l1.thickness;
                let t2 = l2.thickness;
                let total_t = t1 + t2;
                let w1 = if total_t > 0.0 { t1 / total_t } else { 0.5 };
                let w2 = if total_t > 0.0 { t2 / total_t } else { 0.5 };
                
                let new_layer = Layer {
                    thickness: if idx + 1 == n - 1 { 0.0 } else { total_t }, // Keep halfspace thickness 0 if merged into it
                    vp: l1.vp * w1 + l2.vp * w2,
                    vs: l1.vs * w1 + l2.vs * w2,
                    density: l1.density * w1 + l2.density * w2,
                    qp: l1.qp,
                    qs: l1.qs,
                };
                model.layers[idx] = new_layer;
                model.layers.remove(idx + 1);
            }
            
            while model.layers.len() < nlayers {
                let n = model.layers.len();
                let idx = if n > 1 { rng.gen_range(0..n - 1) } else { 0 };
                let mut l1 = model.layers[idx].clone();
                let mut l2 = model.layers[idx].clone();
                if l1.thickness > 0.0 {
                    l1.thickness /= 2.0;
                    l2.thickness /= 2.0;
                } else {
                    l1.thickness = 10.0; // Give some thickness if splitting halfspace
                }
                model.layers[idx] = l1;
                model.layers.insert(idx + 1, l2);
            }
            
            let msg = format!("Warning: Initial model had {} layers but bounds require {}. Randomly added/merged layers to match.", orig_len, nlayers);
            if let Some(ref tx_chan) = tx { let _ = tx_chan.send(msg.clone()); }
            eprintln!("{}", msg);
        }
        
        let mut pos = Vec::with_capacity(nlayers * 2 - 1);
        for i in 0..nlayers - 1 {
            pos.push(model.layers[i].thickness);
        }
        for i in 0..nlayers {
            pos.push(model.layers[i].vs);
        }
        pos
    } else {
        // Calculate initial position as center of bounds
        pso_bounds.iter().map(|b| (b.0 + b.1) / 2.0).collect()
    };

    let is_lm = config.invert.as_deref() == Some("lm");
    let is_occam = config.invert.as_deref() == Some("occam");
    let is_sa = config.invert.as_deref() == Some("sa");

    let result = if is_lm || is_occam {
        let mode = if is_lm {
            crate::hvf::deterministic::DeterministicMode::LevenbergMarquardt
        } else {
            crate::hvf::deterministic::DeterministicMode::Occam
        };
        
        let det_config = crate::hvf::deterministic::DeterministicConfig {
            max_iter: config.pso_iter, // reuse iter arg
            lambda_init: config.lm_lambda,
            occam_alpha: config.occam_alpha,
            mode,
            n_h: nlayers - 1,
        };

        let msg = format!("Starting Deterministic inversion (Max iter: {})...", det_config.max_iter);
        if let Some(ref tx_chan) = tx {
            let _ = tx_chan.send(msg);
        } else {
            eprintln!("{}", msg);
        }
        let det_res = crate::hvf::deterministic::run_deterministic(
            &det_config,
            &initial_pos,
            &obs_hvs,
            &pso_bounds,
            forward_func,
            constrain_func,
        );

        // Map to PSO-like result structure so the rest of the code works
        // We simulate a single improvement history for the deterministic path
        let mut improvements = Vec::new();
        for (i, &cost) in det_res.cost_history.iter().enumerate() {
            // For simplicity, we just put the best position in all history entries to satisfy the struct
            // In a real implementation we'd track the position history in deterministic.rs
            improvements.push(crate::hvf::pso::Improvement {
                iter: i,
                cost,
                position: det_res.best_position.clone(),
            });
        }

        crate::hvf::pso::PsoResult {
            best_position: det_res.best_position,
            best_cost: det_res.best_cost,
            cost_history: det_res.cost_history,
            improvements,
        }
    } else if is_sa {
        let sa_config = crate::hvf::sa::SaConfig {
            max_iter: config.pso_iter,
            t_initial: config.sa_t_initial,
            t_final: config.sa_t_final,
            cooling_rate: config.sa_cooling_rate,
            bounds: pso_bounds.clone(),
        };

        let msg = format!("Starting Simulated Annealing inversion ({} iterations)...", sa_config.max_iter);
        if let Some(ref tx_chan) = tx {
            let _ = tx_chan.send(msg);
        } else {
            eprintln!("{}", msg);
        }
        let sa_res = crate::hvf::sa::run_sa(&sa_config, &initial_pos, obj_func, repair_func, tx.clone());
        
        let mut improvements = Vec::new();
        for imp in sa_res.improvements {
            improvements.push(crate::hvf::pso::Improvement {
                iter: imp.iter,
                cost: imp.cost,
                position: imp.position,
            });
        }
        crate::hvf::pso::PsoResult {
            best_position: sa_res.best_position,
            best_cost: sa_res.best_cost,
            cost_history: sa_res.cost_history,
            improvements,
        }
    } else {
        let pso_config = PsoConfig {
            pop_size: config.pso_pop,
            max_iter: config.pso_iter,
            c1: config.pso_c1,
            c2: config.pso_c2,
            w: config.pso_w,
            bounds: pso_bounds.clone(),
        };

        let msg = format!("Starting PSO inversion ({} particles, {} iterations)...", pso_config.pop_size, pso_config.max_iter);
        if let Some(ref tx_chan) = tx {
            let _ = tx_chan.send(msg);
        } else {
            eprintln!("{}", msg);
        }
        run_pso(&pso_config, &initial_pos, obj_func, repair_func, tx.clone())
    };

    // Build final best model
    let pos = &result.best_position;
    let mut best_layers = Vec::with_capacity(nlayers);
    for i in 0..nlayers {
        let h = if i < nlayers - 1 { pos[i] } else { 0.0 };
        let vs = pos[nlayers - 1 + i];
        let vp = calc_vp(vs);
        let rho = calc_rho(vp);
        best_layers.push(Layer {
            thickness: h,
            vp,
            vs,
            density: rho,
            qp: None,
            qs: None,
        });
    }
    let best_model = EarthModel { layers: best_layers };

    // Compute best HVSR
    let est_hvsr = compute_hvsr_for_inversion(&best_model, &obs_freqs, config);

    let mut history_entries = Vec::new();
    let omega_for_dfa: Vec<f64> = obs_freqs.iter().map(|f| f * TWO_PI).collect();

    for imp in &result.improvements {
        let mut layers = Vec::with_capacity(nlayers);
        for i in 0..nlayers {
            let h = if i < nlayers - 1 { imp.position[i] } else { 0.0 };
            let vs = imp.position[nlayers - 1 + i];
            let vp = calc_vp(vs);
            let rho = calc_rho(vp);
            layers.push(Layer {
                thickness: h,
                vp,
                vs,
                density: rho,
                qp: None,
                qs: None,
            });
        }
        let hist_model = EarthModel { layers };
        let hist_hvsr = compute_hvsr_for_inversion(&hist_model, &obs_freqs, config);
        
        history_entries.push(HistoryEntry {
            iter: imp.iter,
            cost: imp.cost,
            model: hist_model,
            hvsr: hist_hvsr,
        });
    }

    let out = InversionResult {
        best_model,
        best_cost: result.best_cost,
        cost_history: result.cost_history,
        freqs: obs_freqs.clone(),
        estimated_hvsr: est_hvsr,
        history: history_entries,
    };

    if let Some(ref plot_dir) = config.plot_dir {
        if !plot_dir.exists() {
            fs::create_dir_all(plot_dir).expect("Failed to create plot directory");
        }
        
        let fit_path = plot_dir.join("hv_fit.png");
        if let Err(e) = crate::hvf::plot::plot_hv_fit(&obs_freqs, &obs_hvs, &out.estimated_hvsr, &fit_path) {
            eprintln!("Failed to plot H/V fit: {}", e);
        }
        
        let vs_path = plot_dir.join("vs_profile.png");
        if let Err(e) = crate::hvf::plot::plot_vs_profile(&out.best_model.layers, &vs_path) {
            eprintln!("Failed to plot Vs profile: {}", e);
        }
        
        let conv_path = plot_dir.join("convergence.png");
        if let Err(e) = crate::hvf::plot::plot_convergence(&out.cost_history, &conv_path) {
            eprintln!("Failed to plot convergence: {}", e);
        }
        
        eprintln!("Plots saved to {:?}", plot_dir);
    }

    let out_json = serde_json::to_string_pretty(&out).unwrap();
    if let Some(ref out_file) = config.output_file {
        fs::write(out_file, out_json).unwrap_or_else(|e| panic!("Failed to write to {:?}: {}", out_file, e));
        eprintln!("Inversion completed. Results saved to {:?}", out_file);
    } else if config.output_json {
        println!("{}", out_json);
    } else {
        fs::write("inversion_result.json", out_json).expect("Failed to write inversion result");
        eprintln!("Inversion completed. Results saved to inversion_result.json");
    }

    Ok(())
}
