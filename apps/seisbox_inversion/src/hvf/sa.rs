use rand::Rng;

pub struct SaConfig {
    pub max_iter: usize,
    pub t_initial: f64,
    pub t_final: f64,
    pub cooling_rate: f64,
    pub bounds: Vec<(f64, f64)>, // (min, max) for each variable
}

pub struct SaImprovement {
    pub iter: usize,
    pub cost: f64,
    pub position: Vec<f64>,
}

pub struct SaResult {
    pub best_position: Vec<f64>,
    pub best_cost: f64,
    pub cost_history: Vec<f64>,
    pub improvements: Vec<SaImprovement>,
}

/// `repair` is applied to every candidate before evaluation; the repaired position
/// is what gets accepted/stored. Pass `|_| {}` for no repair.
pub fn run_sa<F, R>(
    config: &SaConfig,
    initial_pos: &[f64],
    objective_func: F,
    repair: R,
    tx: Option<std::sync::mpsc::Sender<String>>,
) -> SaResult
where
    F: Fn(&[f64]) -> f64,
    R: Fn(&mut [f64]),
{
    let num_vars = config.bounds.len();
    let mut rng = rand::thread_rng();

    // Initialize position with initial_pos
    let mut current_pos = initial_pos.to_vec();
    repair(&mut current_pos);
    
    let mut current_cost = objective_func(&current_pos);
    
    let mut best_pos = current_pos.clone();
    let mut best_cost = current_cost;
    
    let mut t = config.t_initial;
    
    let mut cost_history = Vec::with_capacity(config.max_iter);
    let mut improvements = Vec::new();
    
    improvements.push(SaImprovement {
        iter: 0,
        cost: best_cost,
        position: best_pos.clone(),
    });
    
    let mut report_counter = 0;

    for iter in 0..config.max_iter {
        // Generate neighbor
        let mut new_pos = current_pos.clone();
        
        // Pick a random dimension to mutate
        let dim = rng.gen_range(0..num_vars);
        let (min_b, max_b) = config.bounds[dim];
        let range = max_b - min_b;
        
        // Step size depends on temperature, scaled by range
        let step = rng.gen_range(-1.0..1.0) * range * (t / config.t_initial);
        new_pos[dim] += step;
        
        // Clamp to bounds
        if new_pos[dim] < min_b { new_pos[dim] = min_b; }
        if new_pos[dim] > max_b { new_pos[dim] = max_b; }
        repair(&mut new_pos);
        
        let new_cost = objective_func(&new_pos);
        
        // Accept or reject
        if new_cost < current_cost {
            current_pos = new_pos.clone();
            current_cost = new_cost;
            
            if current_cost < best_cost {
                best_cost = current_cost;
                best_pos = current_pos.clone();
                improvements.push(SaImprovement {
                    iter,
                    cost: best_cost,
                    position: best_pos.clone(),
                });
            }
        } else {
            let p_accept = f64::exp((current_cost - new_cost) / t);
            if rng.gen::<f64>() < p_accept {
                current_pos = new_pos.clone();
                current_cost = new_cost;
            }
        }
        
        cost_history.push(current_cost);
        
        // Cooling
        t *= config.cooling_rate;
        if t < config.t_final {
            t = config.t_final;
        }
        
        report_counter += 1;
        if report_counter >= 10 {
            if let Some(ref sender) = tx {
                let _ = sender.send(format!("SA Iteration {}/{}, Best Cost: {:.4e}, T: {:.2e}", iter + 1, config.max_iter, best_cost, t));
            }
            report_counter = 0;
        }
    }
    
    SaResult {
        best_position: best_pos,
        best_cost,
        cost_history,
        improvements,
    }
}
