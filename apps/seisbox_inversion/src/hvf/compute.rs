//! Main computation orchestrator.
//!
//! Translates the `PROGRAM HV` main body from `HV.f90`.

use rayon::prelude::*;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::Path;

use crate::hvf::body_waves::{bw_integrals, BodyWaveResult};
use crate::hvf::cli::Config;
use crate::hvf::dispersion::{compute_dispersion, DispersionResult};
use crate::hvf::ellipticity::{compute_ellipticity_curve, EllipLayer};
use crate::hvf::greens::{compute_gl, compute_gr};
use crate::hvf::model::EarthModel;
use crate::hvf::output::{DispersionOutput, HvsrOutput};
use crate::hvf::types::*;

/// Run the full HVSR forward modeling computation.
pub fn run_hvsr(config: &Config) -> Result<(), String> {
    // =========================================================================
    // 1. Load model
    // =========================================================================
    let earth_model = if let Some(ref path) = config.model_file {
        EarthModel::from_file(path, config.use_brocher, config.vp_expr.as_deref(), config.rho_expr.as_deref()).map_err(|e| format!("Error reading model file: {}", e))?
    } else if let Some(ref path) = config.model_json {
        EarthModel::from_json(path).map_err(|e| format!("Error reading JSON model: {}", e))?
    } else {
        return Err("No model file specified.".into());
    };

    let params = earth_model.derived_params(); 

    // =========================================================================
    // 2. Build omega vector
    // =========================================================================
    let omega = build_omega_vector(config)?;
    let nfreq = omega.len();

    // =========================================================================
    // 2.5 Intercept for Ellipticity Methods
    // =========================================================================
    if config.method == "ellipticity" || config.method == "ellipticity-love" {
        let love_alpha = if config.method == "ellipticity-love" { config.love_alpha } else { 0.0 };
        
        let ellip_model: Vec<EllipLayer> = earth_model.layers.iter().map(|l| EllipLayer {
            h: l.thickness,
            vp: l.vp,
            vs: l.vs,
            rho: l.density,
            qp: config.qp,
            qs: config.qs,
        }).collect();

        let freqs: Vec<f64> = omega.iter().map(|w| w / TWO_PI).collect();
        let result = compute_ellipticity_curve(&ellip_model, &freqs, love_alpha);

        if config.output_hv && !config.output_json {
            if let Some(ref out_file) = config.output_file {
                let mut file = std::fs::File::create(out_file).map_err(|e| format!("Failed to create file: {}", e))?;
                for i in 0..nfreq {
                    writeln!(file, "  {:23.17E}  {:23.17E}", result.freqs[i], result.hv[i])
                        .map_err(|e| format!("Write error: {}", e))?;
                }
            } else {
                let stdout = io::stdout();
                let mut out = stdout.lock();
                for i in 0..nfreq {
                    writeln!(out, "  {:23.17E}  {:23.17E}", result.freqs[i], result.hv[i])
                        .map_err(|e| format!("Write error: {}", e))?;
                }
            }
        }

        if config.output_json {
            let out = HvsrOutput {
                frequencies: result.freqs,
                hv_ratio: result.hv,
                rayleigh_phase: None,
                love_phase: None,
            };
            let json_str = out.to_json();
            if let Some(ref out_file) = config.output_file {
                std::fs::write(out_file, json_str).map_err(|e| format!("Failed to write JSON: {}", e))?;
            } else {
                println!("{}", json_str);
            }
        }

        return Ok(());
    }

    // =========================================================================
    // 3. Compute Rayleigh dispersion curves
    // =========================================================================
    let need_rayleigh = config.output_hv || (config.output_ph && config.nmr > 0) || (config.output_gr && config.nmr > 0);
    let rayleigh_result = if need_rayleigh && config.nmr > 0 {
        Some(compute_dispersion(&params, &omega, config.nmr, true))
    } else {
        None
    };

    // =========================================================================
    // 4. Compute Love dispersion curves
    // =========================================================================
    let need_love = config.output_hv || (config.output_ph && config.nml > 0) || (config.output_gr && config.nml > 0);
    let love_result = if need_love && config.nml > 0 {
        Some(compute_dispersion(&params, &omega, config.nml, false))
    } else {
        None
    };

    // Determine actual number of modes found
    let nm_r = rayleigh_result.as_ref().map_or(0, |r| r.num_modes_found);
    let nm_l = love_result.as_ref().map_or(0, |r| r.num_modes_found);
    let _nm_max = nm_r.max(nm_l);

    // =========================================================================
    // 5. Compute surface-wave Green's functions
    // =========================================================================
    let mut img11_pihalf = vec![0.0_f64; nfreq];
    let mut img33 = vec![0.0_f64; nfreq];

    // Per-mode tracking for G123.dat comparison
    let nm_rl = nm_r.max(nm_l);
    let mut g1_per_mode = vec![vec![0.0_f64; nfreq]; nm_rl];
    let mut img2_per_mode = vec![vec![0.0_f64; nfreq]; nm_rl];
    let mut img3_per_mode = vec![vec![0.0_f64; nfreq]; nm_rl];

    // Make mutable copies of slowness for GR/GL (they overwrite phase→group)
    let mut r_slowness = rayleigh_result.as_ref().map(|r| r.slowness.clone());
    let mut l_slowness = love_result.as_ref().map(|r| r.slowness.clone());

    // Rayleigh modes
    if let Some(ref rr) = rayleigh_result {
        if let Some(ref mut r_slow) = r_slowness {
            for imode in 0..nm_r {
                let offset = rr.offsets[imode];
                if offset < 0 {
                    continue;
                }
                let offset = offset as usize;
                let base = imode * nfreq;
                let mode_slow = &mut r_slow[base..base + nfreq];

                let (g1, ig3) = compute_gr(&params, &omega, mode_slow, offset);

                for i in offset..nfreq {
                    img11_pihalf[i] += 0.5 * g1[i];
                    img33[i] += ig3[i];
                    g1_per_mode[imode][i] = g1[i];
                    img3_per_mode[imode][i] = ig3[i];
                }
            }
        }
    }

    // Love modes
    if let Some(ref lr) = love_result {
        if let Some(ref mut l_slow) = l_slowness {
            for imode in 0..nm_l {
                let offset = lr.offsets[imode];
                if offset < 0 {
                    continue;
                }
                let offset = offset as usize;
                let base = imode * nfreq;
                let mode_slow = &mut l_slow[base..base + nfreq];

                let ig2 = compute_gl(&params, &omega, mode_slow, offset);

                for i in offset..nfreq {
                    img11_pihalf[i] -= 0.5 * ig2[i];
                    img2_per_mode[imode][i] = ig2[i];
                }
            }
        }
    }


    // =========================================================================
    // 6. Body-wave integrals (parallelized)
    // =========================================================================
    let bw_results: Vec<BodyWaveResult> = if config.nks > 0 {
        omega
            .par_iter()
            .map(|&w| bw_integrals(&params, config.nks, w, config.ash, config.apsv))
            .collect()
    } else {
        vec![
            BodyWaveResult {
                sum_v: 0.0,
                sum_psv: 0.0,
                sum_sh: 0.0,
            };
            nfreq
        ]
    };

    // =========================================================================
    // 7. Compute H/V ratio
    // =========================================================================
    let mut hv_ratio = vec![0.0_f64; nfreq];
    let mut frequencies = vec![0.0_f64; nfreq];

    for i in 0..nfreq {
        frequencies[i] = omega[i] / TWO_PI;
        let numerator: f64 = 2.0 * (img11_pihalf[i] + bw_results[i].sum_psv + bw_results[i].sum_sh);
        let denominator: f64 = img33[i] + bw_results[i].sum_v;
        if denominator > 0.0 && numerator > 0.0 {
            hv_ratio[i] = (numerator / denominator).sqrt();
        } else if denominator < 0.0 && numerator < 0.0 {
            hv_ratio[i] = ((-numerator) / (-denominator)).sqrt();
        } else {
            hv_ratio[i] = 0.0;
        }
    }


    // =========================================================================
    // 8. Output
    // =========================================================================

    // H/V output
    if config.output_hv && !config.output_json {
        if let Some(ref out_file) = config.output_file {
            let mut file = std::fs::File::create(out_file).map_err(|e| format!("Failed to create file: {}", e))?;
            for i in 0..nfreq {
                writeln!(file, "  {:23.17E}  {:23.17E}", frequencies[i], hv_ratio[i])
                    .map_err(|e| format!("Write error: {}", e))?;
            }
        } else {
            let stdout = io::stdout();
            let mut out = stdout.lock();
            for i in 0..nfreq {
                writeln!(out, "  {:23.17E}  {:23.17E}", frequencies[i], hv_ratio[i])
                    .map_err(|e| format!("Write error: {}", e))?;
            }
        }
    }
    

    // Phase slowness output
    if config.output_ph {
        if let Some(ref rr) = rayleigh_result {
            write_dispersion_file("Rph.dat", nfreq, nm_r, &rr.slowness, &rr.valid)?;
        }
        if let Some(ref lr) = love_result {
            write_dispersion_file("Lph.dat", nfreq, nm_l, &lr.slowness, &lr.valid)?;
        }
    }

    // Group velocity output
    if config.output_gr {
        if let Some(ref r_slow) = r_slowness {
            if let Some(ref rr) = rayleigh_result {
                write_dispersion_file("Rgr.dat", nfreq, nm_r, r_slow, &rr.valid)?;
            }
        }
        if let Some(ref l_slow) = l_slowness {
            if let Some(ref lr) = love_result {
                write_dispersion_file("Lgr.dat", nfreq, nm_l, l_slow, &lr.valid)?;
            }
        }
    }

    // JSON Output
    if config.output_json {
        let out = HvsrOutput {
            frequencies,
            hv_ratio,
            rayleigh_phase: rayleigh_result.map(|r| to_disp_output(&r, nfreq)),
            love_phase: love_result.map(|r| to_disp_output(&r, nfreq)),
        };
        let json_str = out.to_json();
        if let Some(ref out_file) = config.output_file {
            std::fs::write(out_file, json_str).map_err(|e| format!("Failed to write JSON: {}", e))?;
        } else {
            println!("{}", json_str);
        }
    }

    Ok(())
}

/// Build the circular frequency (omega) vector from config.
pub fn build_omega_vector(config: &Config) -> Result<Vec<Float>, String> {
    if let Some(ref freq_path) = config.freq_file {
        // Read frequencies from file
        let file = fs::File::open(freq_path)
            .map_err(|e| format!("Cannot open frequency file: {}", e))?;
        let reader = io::BufReader::new(file);
        let freqs: Vec<Float> = reader
            .lines()
            .filter_map(|line| {
                line.ok()
                    .and_then(|s| s.trim().parse::<Float>().ok())
            })
            .collect();
        if freqs.is_empty() {
            return Err("Empty frequency file.".into());
        }
        Ok(freqs.iter().map(|f| f * TWO_PI).collect())
    } else {
        let nf = config.nf;
        let fmin = config.fmin;
        let fmax = config.fmax;

        if nf == 1 {
            return Ok(vec![fmin * TWO_PI]);
        }

        let mut omega = vec![0.0; nf];

        if config.logsam {
            let log_fmin = (fmin * TWO_PI).log10();
            let log_fmax = (fmax * TWO_PI).log10();
            for i in 0..nf {
                omega[i] = 10.0_f64.powf(log_fmin + (i as Float) * (log_fmax - log_fmin) / (nf as Float - 1.0));
            }
        } else {
            for i in 0..nf {
                omega[i] = (fmin + (i as Float) * (fmax - fmin) / (nf as Float - 1.0)) * TWO_PI;
            }
        }

        Ok(omega)
    }
}

/// Write a dispersion result to a file.
fn write_dispersion_file(
    filename: &str,
    nfreq: usize,
    nmodes: usize,
    slowness: &[Float],
    valid: &[bool],
) -> Result<(), String> {
    let path = Path::new(filename);
    let mut file = fs::File::create(path)
        .map_err(|e| format!("Cannot create {}: {}", filename, e))?;

    writeln!(file, " {} {}", nfreq, nmodes)
        .map_err(|e| format!("Write error: {}", e))?;

    // Write slowness values
    let vals: Vec<String> = slowness[..nfreq * nmodes]
        .iter()
        .map(|v| format!("{:23.17E}", v))
        .collect();
    writeln!(file, " {}", vals.join(" "))
        .map_err(|e| format!("Write error: {}", e))?;

    // Write validity flags
    let flags: Vec<String> = valid[..nfreq * nmodes]
        .iter()
        .map(|v| if *v { "T".to_string() } else { "F".to_string() })
        .collect();
    writeln!(file, " {}", flags.join(" "))
        .map_err(|e| format!("Write error: {}", e))?;

    Ok(())
}

/// Computes the DFA HVSR array directly from an EarthModel and omega vector.
/// Used primarily by the inversion module to avoid file I/O and heavy formatting overhead.
pub fn compute_hvsr_dfa_internal(
    earth_model: &EarthModel,
    omega: &[Float],
    config: &Config,
) -> Vec<Float> {
    let params = earth_model.derived_params(); 
    let nfreq = omega.len(); 

    let rayleigh_result = if config.nmr > 0 {
        Some(compute_dispersion(&params, omega, config.nmr, true))
    } else {
        None
    };

    let love_result = if config.nml > 0 {
        Some(compute_dispersion(&params, omega, config.nml, false))
    } else {
        None
    };

    let nm_r = rayleigh_result.as_ref().map_or(0, |r| r.num_modes_found);
    let nm_l = love_result.as_ref().map_or(0, |r| r.num_modes_found);

    let mut img11_pihalf = vec![0.0_f64; nfreq];
    let mut img33 = vec![0.0_f64; nfreq];

    // Rayleigh modes
    if let Some(ref rr) = rayleigh_result {
        let mut r_slow = rr.slowness.clone();
        for imode in 0..nm_r {
            let offset = rr.offsets[imode];
            if offset < 0 {
                continue;
            }
            let offset = offset as usize;
            let base = imode * nfreq;
            let mode_slow = &mut r_slow[base..base + nfreq];

            let (g1, ig3) = compute_gr(&params, omega, mode_slow, offset);

            for i in offset..nfreq {
                img11_pihalf[i] += 0.5 * g1[i];
                img33[i] += ig3[i];
            }
        }
    }

    // Love modes
    if let Some(ref lr) = love_result {
        let mut l_slow = lr.slowness.clone();
        for imode in 0..nm_l {
            let offset = lr.offsets[imode];
            if offset < 0 {
                continue;
            }
            let offset = offset as usize;
            let base = imode * nfreq;
            let mode_slow = &mut l_slow[base..base + nfreq];

            let ig2 = compute_gl(&params, omega, mode_slow, offset);

            for i in offset..nfreq {
                img11_pihalf[i] -= 0.5 * ig2[i];
            }
        }
    }

    // Body-wave integrals (parallelized)
    let bw_results: Vec<BodyWaveResult> = if config.nks > 0 {
        omega
            .par_iter()
            .map(|&w| bw_integrals(&params, config.nks, w, config.ash, config.apsv))
            .collect()
    } else {
        vec![
            BodyWaveResult {
                sum_v: 0.0,
                sum_psv: 0.0,
                sum_sh: 0.0,
            };
            nfreq
        ]
    };

    let mut hv_ratio = vec![0.0_f64; nfreq];
    for i in 0..nfreq {
        let numerator: f64 = 2.0 * (img11_pihalf[i] + bw_results[i].sum_psv + bw_results[i].sum_sh);
        let denominator: f64 = img33[i] + bw_results[i].sum_v;
        if denominator > 0.0 && numerator > 0.0 {
            hv_ratio[i] = (numerator / denominator).sqrt();
        } else if denominator < 0.0 && numerator < 0.0 {
            hv_ratio[i] = ((-numerator) / (-denominator)).sqrt();
        } else {
            hv_ratio[i] = 0.0;
        }
    }

    hv_ratio
}

/// Convert a DispersionResult to DispersionOutput for JSON.
fn to_disp_output(result: &DispersionResult, nfreq: usize) -> DispersionOutput {
    let nmodes = result.num_modes_found;
    let mut slowness = Vec::with_capacity(nmodes);
    let mut valid = Vec::with_capacity(nmodes);

    for imode in 0..nmodes {
        let base = imode * nfreq;
        slowness.push(result.slowness[base..base + nfreq].to_vec());
        valid.push(result.valid[base..base + nfreq].to_vec());
    }

    DispersionOutput {
        num_frequencies: nfreq,
        num_modes: nmodes,
        slowness,
        valid,
    }
}
