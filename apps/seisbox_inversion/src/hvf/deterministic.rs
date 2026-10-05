use nalgebra::{DMatrix, DVector};
use std::time::Instant;

/// Configuration for deterministic inversion
pub struct DeterministicConfig {
    pub max_iter: usize,
    pub lambda_init: f64,
    pub occam_alpha: f64,
    pub mode: DeterministicMode,
    /// Number of thickness parameters at the start of the parameter vector.
    /// Occam smoothing is applied within the thickness block and within the Vs
    /// block, but never across the boundary between them.
    pub n_h: usize,
}

#[derive(PartialEq, Clone, Copy)]
pub enum DeterministicMode {
    LevenbergMarquardt,
    Occam,
}

pub struct DeterministicResult {
    pub best_position: Vec<f64>,
    pub best_cost: f64,
    pub cost_history: Vec<f64>,
}

/// Compute numerical Jacobian (forward difference)
fn compute_jacobian<F>(f: &F, x: &[f64], fx: &[f64]) -> DMatrix<f64>
where
    F: Fn(&[f64]) -> Vec<f64>,
{
    let n_params = x.len();
    let n_data = fx.len();
    let mut j = DMatrix::zeros(n_data, n_params);
    let delta = 1e-4; // small perturbation

    for col in 0..n_params {
        let mut x_pert = x.to_vec();
        let step = if x[col].abs() > 1e-4 { x[col].abs() * delta } else { delta };
        x_pert[col] += step;

        let fx_pert = f(&x_pert);
        for row in 0..n_data {
            j[(row, col)] = (fx_pert[row] - fx[row]) / step;
        }
    }

    j
}

/// `constrain` is applied to the initial model and to every candidate step after
/// clamping (e.g. isotonic projection). It returns `false` if the resulting model is
/// still infeasible, in which case the candidate is rejected.
pub fn run_deterministic<F, C>(
    config: &DeterministicConfig,
    initial_pos: &[f64],
    obs_data: &[f64],
    bounds: &[(f64, f64)],
    f: F,
    constrain: C,
) -> DeterministicResult
where
    F: Fn(&[f64]) -> Vec<f64>,
    C: Fn(&mut [f64]) -> bool,
{
    let n_params = initial_pos.len();
    let _n_data = obs_data.len();
    let mut current_pos = initial_pos.to_vec();
    if !constrain(&mut current_pos) {
        eprintln!("Warning: initial model violates the physical constraints.");
    }

    let mut current_fx = f(&current_pos);
    let mut current_misfit = compute_rmse(&current_fx, obs_data);

    let mut lambda = config.lambda_init;
    let alpha = config.occam_alpha;
    let d_obs = DVector::from_row_slice(obs_data);

    // First-difference roughness operator, skipping the h/vs block boundary.
    let rt_r = {
        let mut r = DMatrix::zeros(n_params, n_params);
        for i in 0..n_params.saturating_sub(1) {
            if config.n_h > 0 && i == config.n_h - 1 {
                continue;
            }
            r[(i, i)] = -1.0;
            r[(i, i + 1)] = 1.0;
        }
        r.transpose() * r
    };

    let mut cost_history = vec![current_misfit];
    let start_time = Instant::now();

    eprintln!("Iter 0: Initial RMSE = {:.6}", current_misfit);

    for iter in 1..=config.max_iter {
        let j = compute_jacobian(&f, &current_pos, &current_fx);
        let j_t = j.transpose();
        let jtj = &j_t * &j;

        let f_cal = DVector::from_row_slice(&current_fx);
        let residual = &d_obs - f_cal; // d - f(m)
        let gradient = &j_t * residual;

        let mut step_accepted = false;

        for _inner_try in 0..10 {
            let lhs = if config.mode == DeterministicMode::LevenbergMarquardt {
                let mut lhs_lm = jtj.clone();
                for i in 0..n_params {
                    lhs_lm[(i, i)] += lambda * jtj[(i, i)].max(1e-6);
                }
                lhs_lm
            } else {
                jtj.clone() + alpha * &rt_r
            };

            let rhs = if config.mode == DeterministicMode::LevenbergMarquardt {
                gradient.clone()
            } else {
                let m_vec = DVector::from_row_slice(&current_pos);
                gradient.clone() - alpha * &rt_r * m_vec
            };

            let delta_m = if let Some(chol) = lhs.clone().cholesky() {
                chol.solve(&rhs)
            } else {
                lhs.lu().solve(&rhs).unwrap_or_else(|| DVector::zeros(n_params))
            };

            let mut candidate_pos = current_pos.clone();
            for i in 0..n_params {
                candidate_pos[i] += delta_m[i];
                candidate_pos[i] = candidate_pos[i].clamp(bounds[i].0, bounds[i].1);
            }
            let feasible = constrain(&mut candidate_pos);

            let (candidate_fx, candidate_misfit) = if feasible {
                let fx = f(&candidate_pos);
                let m = compute_rmse(&fx, obs_data);
                (fx, m)
            } else {
                (Vec::new(), f64::INFINITY)
            };

            if candidate_misfit < current_misfit {
                current_pos = candidate_pos;
                current_fx = candidate_fx;
                current_misfit = candidate_misfit;
                
                if config.mode == DeterministicMode::LevenbergMarquardt {
                    lambda /= 10.0; 
                }
                step_accepted = true;
                break;
            } else {
                if config.mode == DeterministicMode::LevenbergMarquardt {
                    lambda *= 10.0; 
                } else {
                    break;
                }
            }
        }

        cost_history.push(current_misfit);
        
        let elapsed = start_time.elapsed().as_secs_f64();
        eprintln!(
            "Iter {}: RMSE = {:.6} (Time = {:.2}s)",
            iter, current_misfit, elapsed
        );

        if !step_accepted {
            eprintln!("Optimization stalled. Stopping early.");
            break;
        }

        if current_misfit < 1e-6 {
            eprintln!("Converged to target RMSE. Stopping early.");
            break;
        }
    }

    DeterministicResult {
        best_position: current_pos,
        best_cost: current_misfit,
        cost_history,
    }
}

pub fn compute_rmse(cal: &[f64], obs: &[f64]) -> f64 {
    let mut sum_sq = 0.0;
    for i in 0..obs.len() {
        let diff = obs[i] - cal[i];
        sum_sq += diff * diff;
    }
    (sum_sq / obs.len() as f64).sqrt()
}
