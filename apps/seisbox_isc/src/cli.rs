use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Minimum longitude (e.g. 120.0)
    #[arg(long, allow_hyphen_values = true)]
    pub min_lon: Option<f64>,

    /// Maximum longitude (e.g. 125.0)
    #[arg(long, allow_hyphen_values = true)]
    pub max_lon: Option<f64>,

    /// Minimum latitude (e.g. -10.0)
    #[arg(long, allow_hyphen_values = true)]
    pub min_lat: Option<f64>,

    /// Maximum latitude (e.g. 0.0)
    #[arg(long, allow_hyphen_values = true)]
    pub max_lat: Option<f64>,

    /// Start date (YYYY-MM-DD)
    #[arg(long)]
    pub start_date: Option<String>,

    /// Start time (HH:MM:SS), defaults to 00:00:00
    #[arg(long, default_value = "00:00:00")]
    pub start_time: String,

    /// End date (YYYY-MM-DD)
    #[arg(long)]
    pub end_date: Option<String>,

    /// End time (HH:MM:SS), defaults to 23:59:59
    #[arg(long, default_value = "23:59:59")]
    pub end_time: String,

    /// Minimum depth in km
    #[arg(long, default_value_t = 0.0, allow_hyphen_values = true)]
    pub min_depth: f64,

    /// Maximum depth in km
    #[arg(long, default_value_t = 1000.0)]
    pub max_depth: f64,

    /// Minimum magnitude
    #[arg(long, default_value_t = 0.0, allow_hyphen_values = true)]
    pub min_mag: f64,

    /// Maximum magnitude
    #[arg(long, default_value_t = 10.0)]
    pub max_mag: f64,

    /// Magnitude type priority for the final selection (e.g. "MW,MS,ML,MB")
    #[arg(long, default_value = "MW,MS,ML,MB")]
    pub mag_priority: String,

    /// Data chunk fetching interval in days (to avoid ISC limits)
    #[arg(long, default_value_t = 30)]
    pub chunk_days: i32,

    /// Output CSV file path
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Output Raw TXT file path
    #[arg(long)]
    pub raw_output: Option<PathBuf>,

    /// Optional CSV file containing custom conversion rules to Mw (Format: source_type,min_mag,max_mag,multiplier,offset)
    #[arg(long)]
    pub conversion_file: Option<PathBuf>,

    /// Append results to the existing output files instead of overwriting them
    #[arg(long, default_value_t = false)]
    pub append: bool,

    /// Output directory to save the generated statistical plots (Magnitude Catalog & Pie Chart)
    #[arg(long)]
    pub plot_stats: Option<PathBuf>,

    /// Output PNG file path to save the spatial map plot of the events
    #[arg(long)]
    pub plot_map: Option<PathBuf>,

    /// Read an existing ISC CSV file instead of downloading (bypasses fetch parameters)
    #[arg(long)]
    pub input_csv: Option<PathBuf>,

    /// Read a raw ISC text file (output from --raw-output) instead of downloading
    #[arg(long)]
    pub input_raw: Option<PathBuf>,

    // === Cross-Section Features ===
    
    /// Cross-section start longitude
    #[arg(long)]
    pub cs_start_lon: Option<f64>,
    
    /// Cross-section start latitude
    #[arg(long)]
    pub cs_start_lat: Option<f64>,
    
    /// Cross-section end longitude
    #[arg(long)]
    pub cs_end_lon: Option<f64>,
    
    /// Cross-section end latitude
    #[arg(long)]
    pub cs_end_lat: Option<f64>,
    
    /// Buffer width (in km) from the cross-section line for filtering events
    #[arg(long, default_value_t = 50.0)]
    pub cs_buffer_km: f64,
    
    /// Output PNG path for cross-section plot (distance vs depth)
    #[arg(long)]
    pub plot_cross_section: Option<PathBuf>,

    /// Output CSV path for cross-section relative data (along and cross-track distance)
    #[arg(long)]
    pub cs_out_csv: Option<PathBuf>,

    /// Draw the cross-section track line on the spatial map plot (requires --plot-map, --cs-start-lon/lat, and --cs-end-lon/lat)
    #[arg(long, default_value_t = false)]
    pub plot_cs_track: bool,
}
