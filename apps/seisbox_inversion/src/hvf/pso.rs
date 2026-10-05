use rand::Rng;
use rayon::prelude::*;

#[derive(Clone, Debug)]
pub struct Particle {
    pub position: Vec<f64>,
    pub velocity: Vec<f64>,
    pub best_position: Vec<f64>,
    pub best_cost: f64,
    pub cost: f64,
}

pub struct PsoConfig {
    pub pop_size: usize,
    pub max_iter: usize,
    pub c1: f64,
    pub c2: f64,
    pub w: f64,
    pub bounds: Vec<(f64, f64)>, // (min, max) for each variable
}

pub struct Improvement {
    pub iter: usize,
    pub cost: f64,
    pub position: Vec<f64>,
}

pub struct PsoResult {
    pub best_position: Vec<f64>,
    pub best_cost: f64,
    pub cost_history: Vec<f64>,
    pub improvements: Vec<Improvement>,
}

/// `repair` is applied to every position before it is evaluated; the repaired
/// position is kept in the particle (Lamarckian repair), so personal/global bests
/// always store feasible (e.g. sorted) models. Pass `|_| {}` for no repair.
pub fn run_pso<F, R>(config: &PsoConfig, initial_pos: &[f64], objective_func: F, repair: R, tx: Option<std::sync::mpsc::Sender<String>>) -> PsoResult
where
    F: Fn(&[f64]) -> f64 + Sync,
    R: Fn(&mut [f64]) + Sync,
{
    let num_vars = config.bounds.len();
    let mut rng = rand::thread_rng();

    // 1. Initialize swarm
    let mut swarm: Vec<Particle> = (0..config.pop_size)
        .map(|i| {
            let mut position = vec![0.0; num_vars];
            let mut velocity = vec![0.0; num_vars];
            if i == 0 {
                position = initial_pos.to_vec();
            } else {
                for j in 0..num_vars {
                    let (min_b, max_b) = config.bounds[j];
                    position[j] = rng.gen_range(min_b..max_b);
                }
            }
            for j in 0..num_vars {
                let (min_b, max_b) = config.bounds[j];
                let v_max = (max_b - min_b) * 0.1;
                velocity[j] = rng.gen_range(-v_max..v_max);
            }
            repair(&mut position);
            Particle {
                position: position.clone(),
                velocity,
                best_position: position,
                best_cost: f64::MAX,
                cost: f64::MAX,
            }
        })
        .collect();

    let mut global_best_position = vec![0.0; num_vars];
    let mut global_best_cost = f64::MAX;
    let mut cost_history = Vec::with_capacity(config.max_iter);

    // Evaluate initial positions
    swarm.par_iter_mut().for_each(|particle| {
        particle.cost = objective_func(&particle.position);
        particle.best_cost = particle.cost;
    });

    let mut improvements = Vec::new();

    for p in &swarm {
        if p.cost < global_best_cost {
            global_best_cost = p.cost;
            global_best_position.clone_from(&p.position);
        }
    }
    improvements.push(Improvement {
        iter: 0,
        cost: global_best_cost,
        position: global_best_position.clone(),
    });

    // 2. Main loop
    for iter in 0..config.max_iter {
        // Update velocity and position
        let w = config.w;
        let c1 = config.c1;
        let c2 = config.c2;
        let global_pos = &global_best_position;
        let bounds = &config.bounds;

        // Use sequential update for random number generation simplicity, 
        // then parallel evaluation for the heavy objective function.
        for particle in swarm.iter_mut() {
            let mut r1: f64;
            let mut r2: f64;
            for j in 0..num_vars {
                r1 = rng.gen::<f64>();
                r2 = rng.gen::<f64>();
                
                // Velocity update
                particle.velocity[j] = w * particle.velocity[j]
                    + c1 * r1 * (particle.best_position[j] - particle.position[j])
                    + c2 * r2 * (global_pos[j] - particle.position[j]);

                // Position update
                particle.position[j] += particle.velocity[j];

                // Clamp to bounds
                let (min_b, max_b) = bounds[j];
                if particle.position[j] < min_b {
                    particle.position[j] = min_b;
                    particle.velocity[j] *= -0.5; // Reflect and damp
                } else if particle.position[j] > max_b {
                    particle.position[j] = max_b;
                    particle.velocity[j] *= -0.5;
                }
            }
            repair(&mut particle.position);
        }

        // Evaluate in parallel
        swarm.par_iter_mut().for_each(|particle| {
            particle.cost = objective_func(&particle.position);
            if particle.cost < particle.best_cost {
                particle.best_cost = particle.cost;
                particle.best_position.clone_from(&particle.position);
            }
        });

        // Update global best
        let mut improved = false;
        for p in &swarm {
            if p.best_cost < global_best_cost {
                global_best_cost = p.best_cost;
                global_best_position.clone_from(&p.best_position);
                improved = true;
            }
        }
        
        if improved {
            improvements.push(Improvement {
                iter: iter + 1,
                cost: global_best_cost,
                position: global_best_position.clone(),
            });
        }

        cost_history.push(global_best_cost);
        let msg = format!("Iteration {:4} | Best Cost: {:.6e}", iter + 1, global_best_cost);
        if let Some(ref tx_chan) = tx {
            let _ = tx_chan.send(msg);
        } else {
            println!("{}", msg);
        }
    }

    PsoResult {
        best_position: global_best_position,
        best_cost: global_best_cost,
        cost_history,
        improvements,
    }
}
