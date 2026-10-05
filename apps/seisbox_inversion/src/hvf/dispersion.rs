//! Dispersion curve computation.
//!
//! Translates Fortran `Dispersion.f90`: the main loop that finds modal slowness
//! values at each frequency for Rayleigh and Love waves.

use crate::hvf::frequency_eq::y_dispatch;
use crate::hvf::matrix::halfspace_rayleigh;
use crate::hvf::model::ModelParams;
use crate::hvf::root_solver::RootSolver;
use crate::hvf::types::*;

/// Result of dispersion curve computation for one wave type.
#[derive(Debug, Clone)]
pub struct DispersionResult {
    /// Slowness values: `slowness[mode * nfreq + ifreq]`.
    pub slowness: Vec<Float>,
    /// Validity flags: whether the root was found.
    pub valid: Vec<bool>,
    /// Cutoff frequency offset per mode (number of freqs below cutoff).
    pub offsets: Vec<i32>,
    /// Actual number of modes found (may be less than requested).
    pub num_modes_found: usize,
}

/// Compute dispersion curves for the given wave type.
///
/// Translates Fortran `FUNCTION DISPERSION(VALUES, VALID)`.
///
/// # Arguments
/// * `params` — Derived model parameters.
/// * `omega` — Vector of circular frequencies (ascending).
/// * `num_modes` — Maximum number of modes to compute.
/// * `is_rayleigh` — `true` for Rayleigh, `false` for Love.
pub fn compute_dispersion(
    params: &ModelParams,
    omega: &[Float],
    num_modes: usize,
    is_rayleigh: bool,
) -> DispersionResult {
    let nfreq = omega.len();
    let mut slowness = vec![0.0_f64; nfreq * num_modes];
    let mut valid = vec![false; nfreq * num_modes];
    let offsets = vec![-1_i32; num_modes];

    let min_slow = params.slow_s_min;
    let initial_sup_bound = if is_rayleigh {
        params.max_rayleigh_slowness
    } else {
        params.slow_s[0] // First layer
    };

    let forbit_velocity_inversion = params.velocity_inversion == -1;

    // Create the frequency equation closure
    let y_fn = |slow: Float, omega_val: Float| -> Float {
        y_dispatch(slow, omega_val, params, is_rayleigh)
    };

    // Initialize solver
    let mut solver = RootSolver::new(1e-6); // Will be overridden per-mode
    solver.dx_is_absolute = false;

    // Initialize polarity at a very low frequency
    let provisional_omega = 0.05;
    solver.polarity = y_fn(initial_sup_bound, provisional_omega);
    solver.x2 = initial_sup_bound;

    let mut last_mode: Vec<Float> = vec![-9999.0; nfreq];
    let mut cur_mode: Vec<Float> = vec![0.0; nfreq];

    let mut num_modes_found = num_modes;

    // =========================================================================
    // Main mode loop
    // =========================================================================
    for imode in 0..num_modes {
        let mut max_slow: Float;
        if imode == 0 {
            if is_rayleigh {
                max_slow = params.max_rayleigh_slowness;
                solver.dx = 0.25 * (1.0 - params.slow_s[0] / params.max_rayleigh_slowness);
            } else {
                max_slow = params.slow_s_max;
                let max_ray = halfspace_rayleigh(
                    params.slow_p[params.slow_s.iter().enumerate()
                        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                        .unwrap().0],
                    params.slow_s_max,
                );
                solver.dx = 0.25 * (1.0 - params.slow_s[0] / max_ray);
            }
        } else {
            // Swap modes
            std::mem::swap(&mut last_mode, &mut cur_mode);
            max_slow = last_mode[nfreq - 1];
            solver.dx = 0.5
                * (params.max_rayleigh_slowness - params.slow_s[0])
                / max_slow
                * (TWO_PI / omega[nfreq - 1]);
            solver.polarity = -solver.polarity;
        }

        let mode_offset = imode * nfreq;
        solver.precision = 1e-6; // Reset

        // =======================================================================
        // Retry loop (for mode jumping detection)
        // =======================================================================
        loop {
            let mut error_detected = false;
            let mut cur_slow = max_slow;
            let mut last_sup_bound = initial_sup_bound;

            // =================================================================
            // Frequency loop (from high to low frequency)
            // =================================================================
            let mut last_i = nfreq; // Track where we stopped
            for i in (0..nfreq).rev() {
                let this_omega = omega[i];
                let y_at_omega = |slow: Float| -> Float { y_fn(slow, this_omega) };

                if imode > 0 {
                    max_slow = last_mode[i]; // Fortran: maxSlow=lastMode(i) — mutates maxSlow!
                    if cur_slow > max_slow {
                        cur_slow = max_slow;
                    }
                }

                if solver.search_down(cur_slow, min_slow, max_slow, &y_at_omega) {
                    solver.neville(&y_at_omega);
                    let new_cur_slow = solver.x1.min(solver.x2);
                    let new_last_sup_bound = solver.x1.max(solver.x2);

                    // Check for retrograde dispersion (velocity inversion in curve)
                    if i < nfreq - 1 && new_cur_slow - cur_slow > solver.precision {
                        if forbit_velocity_inversion {
                            error_detected = true;
                            last_i = i;
                            break;
                        }
                        // Inversion allowed: check previous frequency
                        let prev_omega = omega[i + 1];
                        let y_at_prev = |slow: Float| -> Float { y_fn(slow, prev_omega) };
                        solver.dx *= 0.1;
                        let to = if imode > 0 { last_mode[i + 1] } else { max_slow };
                        if solver.search_up(last_sup_bound, to, &y_at_prev) {
                            error_detected = true;
                            solver.dx *= 10.0;
                            last_i = i;
                            break;
                        }
                        solver.dx *= 10.0;
                    }

                    last_sup_bound = new_last_sup_bound;
                    cur_slow = new_cur_slow;
                    cur_mode[i] = cur_slow;
                    slowness[mode_offset + i] = cur_slow;
                    valid[mode_offset + i] = true;
                } else {
                    if imode == 0 {
                        // Fundamental mode: this should not happen unless mode jumping
                        error_detected = true;
                        last_i = i;
                        break;
                    } else {
                        // Higher mode: end of the curve
                        for j in (0..=i).rev() {
                            valid[mode_offset + j] = false;
                            cur_mode[j] = min_slow;
                        }
                        cur_slow = min_slow;
                        last_i = i;
                        break;
                    }
                }

                last_i = i;
            }

            // Check for mode jumping after frequency loop
            if cur_slow > min_slow {
                cur_slow = solver.x1.max(solver.x2);
            }

            let omega_for_check = omega[last_i.min(nfreq - 1)];
            let y_at_check = |slow: Float| -> Float { y_fn(slow, omega_for_check) };

            if error_detected || solver.search_up(cur_slow, max_slow, &y_at_check) {
                // Need to retry with finer step
                if solver.dx < 1e-4 {
                    // Step is small enough, abort this and higher modes
                    for m in imode..num_modes {
                        for j in 0..nfreq {
                            valid[m * nfreq + j] = false;
                        }
                    }
                    num_modes_found = imode;
                    // Jump out of mode loop
                    return finalize(slowness, valid, offsets, num_modes_found, nfreq, num_modes);
                }
                solver.precision *= 0.1;
                solver.dx *= 0.1;
                // Reset validity for this mode
                for j in 0..nfreq {
                    valid[mode_offset + j] = false;
                    slowness[mode_offset + j] = 0.0;
                }
                // Fortran line 179: IF(iMode > 0) maxSlow=lastMode(G_NX)
                if imode > 0 {
                    max_slow = last_mode[nfreq - 1];
                }
            } else {
                break; // Success, move to next mode
            }
        }
    }

    finalize(slowness, valid, offsets, num_modes_found, nfreq, num_modes)
}

/// Finalize the result: compute offsets (cutoff frequencies) for each mode.
fn finalize(
    slowness: Vec<Float>,
    valid: Vec<bool>,
    mut offsets: Vec<i32>,
    mut num_modes_found: usize,
    nfreq: usize,
    num_modes: usize,
) -> DispersionResult {
    // Compute offsets
    for imode in 0..num_modes {
        let base = imode * nfreq;
        let mut found = false;
        for i in 0..nfreq {
            if valid[base + i] {
                offsets[imode] = i as i32;
                found = true;
                break;
            }
        }
        if !found {
            offsets[imode] = -1;
            if imode < num_modes_found {
                num_modes_found = imode;
            }
        }
    }

    DispersionResult {
        slowness,
        valid,
        offsets,
        num_modes_found,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nan_1hz() {
        use crate::hvf::cli::Config;
        use crate::hvf::model::EarthModel;
        use crate::hvf::frequency_eq;
        let config = Config {
            input_file: "../model.txt".to_string(),
            fmin: 1.0, fmax: 1.0, nf: 1, nmr: 1, mode: 0, prec: 1e-12,
            phase_velocity: true, group_velocity: false, output_rep: false, disable_disp: false,
        };
        let model = EarthModel::from_file(std::path::Path::new("../model.txt"), false, None, None).unwrap();
        let disp = DispersionCurve::new(&model, &config);
        
        let slow = 0.0007692307692307692;
        let w = 2.0 * std::f64::consts::PI * 1.0;
        let y = frequency_eq::y_rayleigh(slow, w, &disp.derived_params);
        eprintln!("test_nan_1hz y = {}", y);
    }
}
