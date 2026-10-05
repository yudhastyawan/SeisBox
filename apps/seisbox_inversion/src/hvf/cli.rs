//! Command-line interface parser.
//!
//! Translates Fortran `read_command_line.f03` using `clap` derive macros.

use clap::Parser;
use std::path::PathBuf;

/// HVSR Forward Modeling (DFA and Herak 2008 methods).
///
/// An implementation of the Sánchez-Sesma et al. (2011) Diffuse Field Approach (DFA)
/// and the Tsai (1970) / Herak (2008) body-wave resonance method.
#[derive(Parser, Debug)]
#[command(
    name = "seisbox_inversion",
    version,
    about,
    after_help = "EXAMPLES AND FILE FORMATS:

1. Model / Initial Model File (--model-file / --initial-model)
   A plain text file containing space-separated values for:
   Thickness(m)  Vp(m/s)  Vs(m/s)  Density(kg/m3)
   0.0 means halfspace (last layer).
   Example (4 columns):
   10.0   500.0   200.0   1800.0
   0.0   1500.0   800.0   2200.0
   
   If --use-brocher, --vp-expr, or --rho-expr are provided, only 2 columns are needed:
   Thickness(m)  Vs(m/s)
   Example (2 columns):
   10.0   200.0
   0.0   800.0

2. Observation File (--obs)
   A text/CSV file for inversion containing Frequency and H/V Amplitude.
   Example:
   0.5   1.2
   1.0   3.4
   1.5   2.1

3. RJ-MCMC Config (--mcmc-config)
   A JSON file containing RJ-MCMC parameters.
   Example:
   {
     \"obs_file\": \"data.txt\",
     \"hvf_path\": \"\",
     \"output_file\": \"output.jsonl\",
     \"n_iter\": 1000,
     \"burnin\": 200,
     \"thin\": 1,
     \"vs_min\": 100.0,
     \"vs_max\": 2000.0,
     \"h_min\": 1.0,
     \"h_max\": 200.0,
     \"min_layers\": 1,
     \"max_layers\": 5,
     \"min_total_depth\": 10.0,
     \"max_total_depth\": 500.0,
     \"prob_asc_vs\": 0.5,
     \"prob_asc_h\": 0.5,
     \"use_avg_vs\": false,
     \"avg_vs_depth\": 30.0,
     \"avg_vs_min\": 100.0,
     \"avg_vs_max\": 1000.0,
     \"n_initial_search\": 100,
     \"f0_min\": 0.5,
     \"f0_max\": 5.0,
     \"f0_weight\": 1.0,
     \"a0_weight\": 1.0
   }

   3. MCMC Inversion:
   seisbox_inversion --invert mcmc --mcmc-config my_mcmc_config.json

   4. Forward Modeling with Ellipticity:
   seisbox_inversion -f model.txt --method ellipticity --fmin 0.5 --fmax 10.0 --nf 50 --hv

   5. Inversion with Ellipticity-Love:
   seisbox_inversion --invert pso -f obs.hv --method ellipticity-love --love-alpha 0.5

   6. Custom Expressions (--vp-expr, --rho-expr)
   Example:
   {
     \"vp_expr\": \"1.16 * vs + 1.36\",
     \"rho_expr\": \"0.31 * vp^0.25\"
   }

   7. Plotting Inversion Results:
   seisbox_inversion --invert pso -f obs.hv --method ellipticity --plot-dir my_plots

   8. Physical Constraints (Vs / thickness non-decreasing with depth):
   seisbox_inversion --invert pso --obs obs.csv --bounds bounds.json --enforce-increasing-vs --constraint-mode repair
   - repair  : PSO/SA sort + clamp each model before evaluation (sorted model is stored);
               LM/Occam use isotonic projection. Default.
   - penalty : legacy, violating models get cost 1e6.
   Tip: keep per-layer bounds monotonic so 'repair' always yields feasible models.


   ---

   CUSTOM EQUATIONS (MEVAL):
   When using `--vp-expr` and `--rho-expr`, you can use standard math operations:
   - `+`, `-`, `*`, `/`, `%` (modulo), `^` (power)
   - Functions: `sin`, `cos`, `tan`, `exp`, `log`, `ln`, `sqrt`, `abs`, etc.
   - Variables: 
      * `vp_expr` MUST use `vs` as the input variable.
      * `rho_expr` MUST use `vp` and/or `vs` as input variables.
   Example: `--vp-expr \"1.16*vs + 1.36\" --rho-expr \"0.31*vp^0.25\"`
"
)]
pub struct Cli {
    // === Frequency Options ===
    /// Minimum frequency (Hz).
    #[arg(help_heading = "Frequency & Sampling", long)]
    pub fmin: Option<f64>,

    /// Maximum frequency (Hz).
    #[arg(help_heading = "Frequency & Sampling", long)]
    pub fmax: Option<f64>,

    /// Number of frequencies.
    #[arg(help_heading = "Frequency & Sampling", long)]
    pub nf: Option<usize>,

    /// Use logarithmic sampling in frequency.
    #[arg(help_heading = "Frequency & Sampling", long, default_value_t = false)]
    pub logsam: bool,

    /// Frequency list (sorted) from file.
    #[arg(help_heading = "Frequency & Sampling", long)]
    pub ff: Option<PathBuf>,

    // === Model Options ===
    /// Model file in legacy text format (thickness Vp Vs density).
    #[arg(help_heading = "Model Definition", long, short = 'f')]
    pub model_file: Option<PathBuf>,

    /// Model file in JSON format (for GUI integration).
    #[arg(help_heading = "Model Definition", long)]
    pub model_json: Option<PathBuf>,

    // === Method Selection ===
    /// Forward modeling method to use: "dfa" (default), "herak", "ellipticity", or "ellipticity-love".
    #[arg(help_heading = "Forward Modeling Method", long, default_value = "dfa")]
    pub method: String,

    /// Use Brocher (2005) empirical relations to compute Vp and Density from Vs.
    /// If used, the model file only needs to contain 2 columns: thickness and Vs.
    #[arg(help_heading = "Forward Modeling Method", long, default_value_t = false)]
    pub use_brocher: bool,

    /// Custom math expression for Vp as a function of 'vs' (e.g. "1.16 * vs + 1.36").
    /// Only used if use_brocher is false.
    #[arg(help_heading = "Forward Modeling Method", long)]
    pub vp_expr: Option<String>,

    /// Custom math expression for Density as a function of 'vp' and 'vs' (e.g. "0.31 * vp^0.25").
    /// Only used if use_brocher is false.
    #[arg(help_heading = "Forward Modeling Method", long)]
    pub rho_expr: Option<String>,

    /// Love wave energy ratio for ellipticity-love method.
    /// H/V = sqrt(ell_R^2 + alpha / (1 - alpha))
    #[arg(help_heading = "Forward Modeling Method", long, default_value_t = 0.5)]
    pub love_alpha: f64,

    // === Herak (2008) Q Parameters ===
    /// P-wave quality factor (attenuation) for Herak method.
    #[arg(help_heading = "Attenuation (Herak)", long, default_value_t = 100.0)]
    pub qp: f64,

    /// S-wave quality factor (attenuation) for Herak method.
    #[arg(help_heading = "Attenuation (Herak)", long, default_value_t = 50.0)]
    pub qs: f64,

    /// Power for Q frequency dependence for Herak method.
    #[arg(help_heading = "Attenuation (Herak)", long, default_value_t = 0.0)]
    pub kq: f64,

    /// Reference frequency for Q for Herak method.
    #[arg(help_heading = "Attenuation (Herak)", long, default_value_t = 0.0)]
    pub fref: f64,

    // === Dispersion ===
    /// Maximum number of Rayleigh modes to compute.
    #[arg(help_heading = "Dispersion / Root Finding", long, default_value_t = 0)]
    pub nmr: usize,

    /// Maximum number of Love modes to compute.
    #[arg(help_heading = "Dispersion / Root Finding", long, default_value_t = 0)]
    pub nml: usize,

    /// Relative precision in slowness (default: 1e-4 per cent).
    #[arg(help_heading = "Dispersion / Root Finding", long, default_value_t = 1e-4)]
    pub prec: f64,

    // === Body Waves ===
    /// Number of k values for numerical integration (0 to skip BW calculation).
    #[arg(help_heading = "Integration / Body Waves", long, default_value_t = 0)]
    pub nks: usize,

    /// Attenuation to stabilize PSV: ω → ω − i·apsv·ω.
    #[arg(help_heading = "Integration / Body Waves", long, default_value_t = 1e-5)]
    pub apsv: f64,

    /// Attenuation to stabilize SH: ω → ω − i·ash·ω.
    #[arg(help_heading = "Integration / Body Waves", long, default_value_t = 1e-5)]
    pub ash: f64,

    // === Output Selection ===
    /// Output (freq, H/V) pairs.
    #[arg(help_heading = "Output & Logging", long, default_value_t = false)]
    pub hv: bool,

    /// Output phase slowness (Rph.dat, Lph.dat).
    #[arg(help_heading = "Output & Logging", long, default_value_t = false)]
    pub ph: bool,

    /// Output group velocities (Rgr.dat, Lgr.dat).
    #[arg(help_heading = "Output & Logging", long, default_value_t = false)]
    pub gr: bool,

    /// Write a report if computation fails.
    #[arg(help_heading = "Output & Logging", long, default_value_t = false)]
    pub rep: bool,

    /// Output all results in JSON format (for GUI integration).
    #[arg(help_heading = "Output & Logging", long, default_value_t = false)]
    pub json_output: bool,

    /// Optional file path to write results (instead of stdout).
    #[arg(help_heading = "Output & Logging", long, short = 'o')]
    pub output: Option<PathBuf>,

    // === Inversion Options ===
    /// Run in inversion mode with specified method ("pso", "lm", "occam", or "mcmc").
    #[arg(help_heading = "Inversion Settings", long)]
    pub invert: Option<String>,

    /// Observation data file (CSV format: freq, hv_amp) for inversion.
    #[arg(help_heading = "Inversion Settings", long)]
    pub obs: Option<PathBuf>,

    /// Bounds file for PSO/Deterministic inversion (JSON format).
    #[arg(help_heading = "Inversion Settings", long)]
    pub bounds: Option<PathBuf>,

    /// Initial model for deterministic inversion (text format). Defaults to center of bounds if not provided.
    #[arg(help_heading = "Inversion Settings", long)]
    pub initial_model: Option<PathBuf>,

    /// Initial damping factor for Levenberg-Marquardt inversion.
    #[arg(help_heading = "Inversion Settings", long, default_value_t = 1.0)]
    pub lm_lambda: f64,

    /// Regularization weight alpha for Occam's inversion.
    #[arg(help_heading = "Inversion Settings", long, default_value_t = 0.1)]
    pub occam_alpha: f64,

    /// Config file for RJ-MCMC (JSON format).
    #[arg(help_heading = "Inversion Settings", long)]
    pub mcmc_config: Option<PathBuf>,

    /// PSO population size.
    #[arg(help_heading = "Inversion Settings", long, default_value_t = 50)]
    pub pso_pop: usize,

    /// PSO maximum iterations.
    #[arg(help_heading = "Inversion Settings", long, default_value_t = 100)]
    pub pso_iter: usize,

    /// PSO cognitive coefficient (c1).
    #[arg(help_heading = "Inversion Settings", long, default_value_t = 2.0)]
    pub pso_c1: f64,

    /// PSO social coefficient (c2).
    #[arg(help_heading = "Inversion Settings", long, default_value_t = 2.0)]
    pub pso_c2: f64,

    /// PSO inertia weight (w).
    #[arg(help_heading = "Inversion Settings", long, default_value_t = 0.9)]
    pub pso_w: f64,

    /// SA initial temperature.
    #[arg(help_heading = "Inversion Settings", long, default_value_t = 10.0)]
    pub sa_t_initial: f64,

    /// SA final temperature.
    #[arg(help_heading = "Inversion Settings", long, default_value_t = 1e-4)]
    pub sa_t_final: f64,

    /// SA cooling rate.
    #[arg(help_heading = "Inversion Settings", long, default_value_t = 0.95)]
    pub sa_cooling_rate: f64,

    /// Enforce non-decreasing Vs with depth.
    #[arg(help_heading = "Inversion Settings", long, default_value_t = false)]
    pub enforce_increasing_vs: bool,

    /// Enforce non-decreasing thickness with depth.
    #[arg(help_heading = "Inversion Settings", long, default_value_t = false)]
    pub enforce_increasing_h: bool,

    /// How constraints are handled: "repair" (sort/project before evaluation) or "penalty" (reject, legacy).
    #[arg(help_heading = "Inversion Settings", long, default_value = "repair")]
    pub constraint_mode: String,

    /// Directory to save output plots (if specified, enables plotting).
    #[arg(help_heading = "Inversion Settings", long)]
    pub plot_dir: Option<PathBuf>,
}

/// Parsed and validated configuration for the computation.
#[derive(Debug)]
pub struct Config {
    pub fmin: f64,
    pub fmax: f64,
    pub nf: usize,
    pub logsam: bool,
    pub freq_file: Option<PathBuf>,
    pub model_file: Option<PathBuf>,
    pub model_json: Option<PathBuf>,
    pub method: String,
    pub use_brocher: bool,
    pub vp_expr: Option<String>,
    pub rho_expr: Option<String>,
    pub love_alpha: f64,
    pub qp: f64,
    pub qs: f64,
    pub kq: f64,
    pub fref: f64,
    pub nmr: usize,
    pub nml: usize,
    pub prec: f64,
    pub nks: usize,
    pub apsv: f64,
    pub ash: f64,
    pub output_hv: bool,
    pub output_ph: bool,
    pub output_gr: bool,
    pub output_rep: bool,
    pub output_json: bool,
    pub output_file: Option<PathBuf>,

    // Inversion configurations
    pub invert: Option<String>,
    pub lm_lambda: f64,
    pub occam_alpha: f64,
    pub obs: Option<PathBuf>,
    pub bounds: Option<PathBuf>,
    pub initial_model: Option<PathBuf>,
    pub mcmc_config: Option<PathBuf>,
    pub pso_pop: usize,
    pub pso_iter: usize,
    pub pso_c1: f64,
    pub pso_c2: f64,
    pub pso_w: f64,
    pub sa_t_initial: f64,
    pub sa_t_final: f64,
    pub sa_cooling_rate: f64,
    pub enforce_increasing_vs: bool,
    pub enforce_increasing_h: bool,
    pub constraint_mode: crate::hvf::constraints::ConstraintMode,
    pub plot_dir: Option<PathBuf>,
}

impl Cli {
    /// Validate and convert CLI arguments into a Config.
    pub fn into_config(self) -> Result<Config, String> {
        let is_inverting = self.invert.is_some();

        // Validate that at least one output is requested (only if not inverting)
        if !is_inverting && !self.hv && !self.ph && !self.gr && !self.rep && !self.json_output {
            return Err("Use --hv, --ph, --gr, --json-output, or --invert to select an output.".into());
        }

        // Validate that a model/bounds is provided
        if !is_inverting && self.model_file.is_none() && self.model_json.is_none() {
            return Err("A model file must be provided via --model-file or --model-json for forward modeling.".into());
        }

        let is_mcmc = self.invert.as_deref().map(|s| s.to_lowercase()) == Some("mcmc".to_string());

        if is_inverting && !is_mcmc {
            if self.obs.is_none() {
                return Err("Observation data --obs is required for inversion.".into());
            }
            if self.bounds.is_none() {
                return Err("Bounds file --bounds is required for inversion.".into());
            }
        }

        if is_mcmc && self.mcmc_config.is_none() {
            return Err("MCMC config --mcmc-config is required for MCMC inversion.".into());
        }

        // Validate frequency parameters (only if NOT inverting, since inversion uses obs freqs)
        let freq_file = self.ff.clone();
        let fmin = self.fmin.unwrap_or(-1.0);
        let fmax = self.fmax.unwrap_or(-1.0);
        let nf = self.nf.unwrap_or(0);

        if !is_inverting {
            if freq_file.is_none() {
                if fmin < 0.0 || fmax < 0.0 || fmax < fmin || nf == 0 {
                    return Err("Incorrect or insufficient frequency values for forward modeling. Use --fmin, --fmax, --nf, or --ff.".into());
                }
            }
        }

        // Validate that at least some modes or body waves are requested (for DFA)
        if !is_inverting && self.method == "dfa" && self.nmr == 0 && self.nml == 0 && self.nks == 0 {
            return Err("Nothing to compute. Check --nmr, --nml, --nks.".into());
        }

        let method = self.method.to_lowercase();
        if method != "dfa" && method != "herak" && method != "ellipticity" && method != "ellipticity-love" {
            return Err("Invalid method. Must be 'dfa', 'herak', 'ellipticity', or 'ellipticity-love'.".into());
        }

        let constraint_mode = crate::hvf::constraints::ConstraintMode::parse(&self.constraint_mode)?;

        Ok(Config {
            fmin,
            fmax,
            nf,
            logsam: self.logsam,
            freq_file,
            model_file: self.model_file,
            model_json: self.model_json,
            method,
            use_brocher: self.use_brocher,
            vp_expr: self.vp_expr,
            rho_expr: self.rho_expr,
            love_alpha: self.love_alpha,
            qp: self.qp,
            qs: self.qs,
            kq: self.kq,
            fref: self.fref,
            nmr: self.nmr,
            nml: self.nml,
            prec: self.prec,
            nks: self.nks,
            apsv: self.apsv,
            ash: self.ash,
            output_hv: self.hv,
            output_ph: self.ph,
            output_gr: self.gr,
            output_rep: self.rep,
            output_json: self.json_output,
            output_file: self.output,
            invert: self.invert.map(|s| s.to_lowercase()),
            lm_lambda: self.lm_lambda,
            occam_alpha: self.occam_alpha,
            obs: self.obs,
            bounds: self.bounds,
            initial_model: self.initial_model,
            mcmc_config: self.mcmc_config,
            pso_pop: self.pso_pop,
            pso_iter: self.pso_iter,
            pso_c1: self.pso_c1,
            pso_c2: self.pso_c2,
            pso_w: self.pso_w,
            sa_t_initial: self.sa_t_initial,
            sa_t_final: self.sa_t_final,
            sa_cooling_rate: self.sa_cooling_rate,
            enforce_increasing_vs: self.enforce_increasing_vs,
            enforce_increasing_h: self.enforce_increasing_h,
            constraint_mode,
            plot_dir: self.plot_dir,
        })
    }
}
