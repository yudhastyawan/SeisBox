use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "seisbox_fdsn")]
#[command(about = "SeisBox FDSN Downloader CLI", long_about = None)]
pub struct FdsnArgs {
    /// Comma-separated list of FDSN providers (e.g., "IRIS,GEONET", or "ALL").
    /// If empty, a default set of primary providers will be used.
    #[arg(long)]
    pub providers: Option<String>,

    /// Reference Latitude for search center (degrees).
    #[arg(long, allow_hyphen_values = true)]
    pub lat: Option<f64>,

    /// Reference Longitude for search center (degrees).
    #[arg(long, allow_hyphen_values = true)]
    pub lon: Option<f64>,

    /// Minimum radius (degrees).
    #[arg(long, default_value_t = 0.0)]
    pub min_radius: f64,

    /// Maximum radius (degrees).
    #[arg(long, default_value_t = 5.0)]
    pub max_radius: f64,

    /// Bounding Box: Minimum Latitude (degrees).
    #[arg(long, allow_hyphen_values = true)]
    pub min_lat: Option<f64>,

    /// Bounding Box: Maximum Latitude (degrees).
    #[arg(long, allow_hyphen_values = true)]
    pub max_lat: Option<f64>,

    /// Bounding Box: Minimum Longitude (degrees).
    #[arg(long, allow_hyphen_values = true)]
    pub min_lon: Option<f64>,

    /// Bounding Box: Maximum Longitude (degrees).
    #[arg(long, allow_hyphen_values = true)]
    pub max_lon: Option<f64>,

    /// Start time in format YYYY-MM-DD HH:MM:SS (e.g., "2024-01-01 00:00:00")
    #[arg(long)]
    pub start_time: Option<String>,

    /// End time in format YYYY-MM-DD HH:MM:SS (e.g., "2024-01-01 23:59:59")
    #[arg(long)]
    pub end_time: Option<String>,

    /// Reference time for earthquake origin (YYYY-MM-DD HH:MM:SS). If provided, overrides start_time and end_time.
    #[arg(long)]
    pub ref_time: Option<String>,

    /// Start offset in seconds relative to ref_time (e.g., -60)
    #[arg(long, allow_hyphen_values = true)]
    pub start_offset: Option<i64>,

    /// End offset in seconds relative to ref_time (e.g., 600)
    #[arg(long, allow_hyphen_values = true)]
    pub end_offset: Option<i64>,

    /// Channel filter (e.g., "BH?,HH?")
    #[arg(long, default_value = "BHZ,HHZ")]
    pub channel: String,

    /// Network filter (e.g., "GE,IA")
    #[arg(long)]
    pub network: Option<String>,

    /// Station filter (e.g., "CISI,JAGI")
    #[arg(long)]
    pub station: Option<String>,

    /// Output directory for downloaded files.
    #[arg(long, default_value = "./fdsn_data")]
    pub out_dir: String,

    /// Download StationXML files
    #[arg(long, default_value_t = false)]
    pub download_xml: bool,

    /// Download event catalog (earthquakes) as a CSV file to the output directory.
    #[arg(long, default_value_t = false)]
    pub download_events: bool,

    /// Download phase arrivals for events (forces format=xml)
    #[arg(long, default_value_t = false)]
    pub download_arrivals: bool,

    /// Automatically export downloaded waveforms to SAC format
    #[arg(long, default_value_t = false)]
    pub export_sac: bool,

    /// Do NOT download MiniSEED waveform files (useful if only XML is needed).
    #[arg(long, default_value_t = false)]
    pub no_mseed: bool,

    /// Output a map showing the reference point and found stations.
    #[arg(long)]
    pub plot_map: Option<String>,



    /// Minimum magnitude for event download.
    #[arg(long)]
    pub min_mag: Option<f64>,

    /// Maximum magnitude for event download.
    #[arg(long)]
    pub max_mag: Option<f64>,
}
