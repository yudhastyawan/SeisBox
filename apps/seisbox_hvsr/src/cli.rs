use clap::{Parser, ValueEnum};
use std::path::PathBuf;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
pub enum CombineMethod {
    Geometric,
    Quadratic,
    Arithmetic,
    Maximum,
    Dfa,
}

#[derive(Parser, Debug)]
#[command(name = "seisbox_hvsr")]
#[command(about = "SeisBox HVSR Analysis Tool", long_about = None)]
#[command(
    after_help = "EXAMPLES:
   1. Launch GUI:
   seisbox_hvsr

   2. Run headless HVSR processing (STA/LTA windowing):
   seisbox_hvsr -f my_data_Z.sac my_data_N.sac my_data_E.sac -o result.csv

   3. Run headless HVTFA processing (Continuous Wavelet Transform):
   seisbox_hvsr -f my_data_Z.sac my_data_N.sac my_data_E.sac -o result.max --hvtfa --plot-curve

   4. Run headless HVSR with specific combination method and smoothing:
   seisbox_hvsr -f Z.sac N.sac E.sac -o result.csv --combine dfa --b-value 20.0
"
)]
pub struct Cli {
    /// Files to load (Z, N, E). Can be SAC or miniseed.
    #[arg(short, long, num_args = 1..)]
    pub files: Option<Vec<PathBuf>>,

    /// Output CSV file for the headless run
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Window length in seconds
    #[arg(long, default_value_t = 40.0)]
    pub window_len: f64,

    /// Overlap percentage (0 to 90)
    #[arg(long, default_value_t = 50.0)]
    pub overlap: f64,

    /// STA length in seconds
    #[arg(long, default_value_t = 1.0)]
    pub sta_len: f64,

    /// LTA length in seconds
    #[arg(long, default_value_t = 30.0)]
    pub lta_len: f64,

    /// Lower threshold for STA/LTA (T1)
    #[arg(long, default_value_t = 0.2)]
    pub t1: f64,

    /// Upper threshold for STA/LTA (T2)
    #[arg(long, default_value_t = 2.5)]
    pub t2: f64,

    /// Konno-Ohmachi smoothing coefficient (b-value)
    #[arg(long, default_value_t = 40.0)]
    pub b_value: f64,

    /// Minimum frequency (Hz)
    #[arg(long, default_value_t = 0.1)]
    pub freq_min: f64,

    /// Maximum frequency (Hz)
    #[arg(long, default_value_t = 20.0)]
    pub freq_max: f64,

    /// Number of logarithmically spaced frequency points
    #[arg(long, default_value_t = 100)]
    pub freq_count: usize,

    /// Horizontal combination method
    #[arg(long, value_enum, default_value_t = CombineMethod::Geometric)]
    pub combine: CombineMethod,

    /// Enable HVTFA (H/V Time-Frequency Analysis) using Continuous Wavelet Transform
    #[arg(long, default_value_t = false)]
    pub hvtfa: bool,

    /// Parameter m (Wavelet width) for Morlet Wavelet in HVTFA
    #[arg(long, default_value_t = 1.0)]
    pub hvtfa_m: f64,

    /// Enable iterative f0 filter
    #[arg(long)]
    pub f0_filter: bool,

    /// Multiplier for f0 filter (standard deviations)
    #[arg(long, default_value_t = 2.0)]
    pub f0_filter_n: f64,

    /// Max iterations for f0 filter
    #[arg(long, default_value_t = 50)]
    pub f0_filter_max_iter: usize,

    /// Generate PNG plot for the HVSR curve
    #[arg(long, default_value_t = false)]
    pub plot_curve: bool,

    /// Generate PNG plot for the raw time series and window selection
    #[arg(long, default_value_t = false)]
    pub plot_windows: bool,
}
