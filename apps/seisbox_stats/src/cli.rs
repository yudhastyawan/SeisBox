use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    author = "Yudha Styawan, Geophysical Engineering, Institut Teknologi Sumatera, Indonesia",
    version,
    about = "SeisBox Stats — Statistical Seismology CLI",
    allow_negative_numbers = true,
    long_about = "\
SeisBox Stats — Statistical Seismology Command Line Interface

Author: Yudha Styawan, Geophysical Engineering, Institut Teknologi Sumatera, Indonesia

A comprehensive toolkit for earthquake catalogue analysis including:
- Data filtering & declustering
- Magnitude of completeness (Mc) estimation
- Gutenberg-Richter b-value / a-value calculation
- Modified Omori Law fitting
- Spatial gridding & mapping (b-value, Mc, a-value, Z-value, rate)
- Cross-section depth profiling
- B-Value Voronoi tessellation analysis (OK1993)
- FMD, cumulative seismicity, and various statistical plots

INPUT FORMAT:
The input catalogue must be a CSV file with a header row containing at minimum:
  lon, lat, depth, mag, time
Where 'time' is in ISO 8601 format (e.g. 2023-02-06T01:17:35.000Z).
Additional columns are preserved but ignored.
",
    after_help = "\
EXAMPLES:

1. Filter catalogue by magnitude and depth:
   seisbox_stats filter -i catalog.csv --min-mag 3.0 --max-depth 50 -o filtered.csv

2. Decluster using Gardner-Knopoff method:
   seisbox_stats decluster -i catalog.csv --method gk --window gruenthal -o declustered.csv

3. Estimate Magnitude of Completeness:
   seisbox_stats mc -i catalog.csv --method maxc --bin-width 0.1

4. Calculate b-value using MLE:
   seisbox_stats bvalue -i catalog.csv --method mle --mc 3.0

5. Generate FMD plot:
   seisbox_stats plot -i catalog.csv --type fmd --mc 3.0 -o fmd.png

6. Spatial b-value gridding:
   seisbox_stats grid -i catalog.csv --method constant-n --n-events 50 \\
      --param bvalue --mc 3.0 --grid-inc 0.1 -o bvalue_grid.csv --tiff

7. Run Voronoi ensemble:
   seisbox_stats voronoi -i catalog.csv -o bvalue.npz \\
      --n-min 2 --n-max 60 --init sobol:30,uniform:30
"
)]
pub struct CliArgs {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Filter earthquake catalogue (time, magnitude, depth, spatial)
    Filter(FilterArgs),
    
    /// Decluster catalogue (separate mainshocks from aftershocks)
    Decluster(DeclusterArgs),
    
    /// Estimate Magnitude of Completeness (Mc)
    Mc(McArgs),
    
    /// Calculate b-value and a-value (Gutenberg-Richter)
    Bvalue(BvalueArgs),
    
    /// Fit Modified Omori Law to aftershock sequence
    Omori(OmoriArgs),
    
    /// Spatial gridding of seismological parameters
    Grid(GridArgs),
    
    /// Cross-section depth profile analysis
    #[command(name = "cross-sec")]
    CrossSection(CrossSectionArgs),
    
    /// Generate various plots (FMD, cumulative, mag-time, depth-hist, rate, time-of-day)
    Plot(PlotArgs),
    
    /// Plot spatial grid output from a grid CSV
    #[command(name = "plot-grid")]
    PlotGrid(PlotGridArgs),
    
    /// Run B-Value Voronoi tessellation ensemble (OK1993)
    Voronoi(VoronoiArgs),
    
    /// Fractal Dimension / D-value analysis
    Fractal(FractalArgs),
    
    /// B-value time series analysis (sliding window)
    #[command(name = "bvalue-ts")]
    BvalueTimeseries(BvalueTimeseriesArgs),
    
    /// Stress Tensor Inversion from Focal Mechanisms
    Stress(StressArgs),
    
    /// Headless visualization / export of Voronoi results
    #[command(name = "voronoi-vis")]
    VoronoiVis(VoronoiVisArgs),
}

// ============================================================
// Filter Subcommand
// ============================================================
#[derive(Parser, Debug)]
pub struct FilterArgs {
    /// Input catalogue CSV file
    #[arg(short, long)]
    pub input: PathBuf,
    
    /// Output filtered CSV file
    #[arg(short, long)]
    pub output: PathBuf,
    
    /// Minimum magnitude
    #[arg(long)]
    pub min_mag: Option<f64>,
    
    /// Maximum magnitude
    #[arg(long)]
    pub max_mag: Option<f64>,
    
    /// Minimum depth (km)
    #[arg(long)]
    pub min_depth: Option<f64>,
    
    /// Maximum depth (km)
    #[arg(long)]
    pub max_depth: Option<f64>,
    
    /// Start time (ISO 8601: 2023-01-01T00:00:00)
    #[arg(long)]
    pub start_time: Option<String>,
    
    /// End time (ISO 8601: 2023-12-31T23:59:59)
    #[arg(long)]
    pub end_time: Option<String>,
    
    /// Bounding box: min_lon
    #[arg(long)]
    pub min_lon: Option<f64>,
    
    /// Bounding box: max_lon
    #[arg(long)]
    pub max_lon: Option<f64>,
    
    /// Bounding box: min_lat
    #[arg(long)]
    pub min_lat: Option<f64>,
    
    /// Bounding box: max_lat
    #[arg(long)]
    pub max_lat: Option<f64>,
    
    /// Radial filter: center longitude
    #[arg(long)]
    pub center_lon: Option<f64>,
    
    /// Center latitude for radial filtering
    #[arg(long)]
    pub center_lat: Option<f64>,
    
    /// Radius in km for radial filtering
    #[arg(long)]
    pub radius: Option<f64>,
    
    /// Polygon points in format "lon1,lat1;lon2,lat2;..."
    #[arg(long)]
    pub polygon: Option<String>,
    
    /// Slab geometry XYZ file
    #[arg(long)]
    pub slab_file: Option<PathBuf>,
    
    /// Distance buffer for slab geometry filter (km)
    #[arg(long, default_value_t = 20.0)]
    pub slab_buffer: f64,
}

// ============================================================
// Decluster Subcommand
// ============================================================
#[derive(Parser, Debug)]
pub struct DeclusterArgs {
    /// Input catalogue CSV file
    #[arg(short, long)]
    pub input: PathBuf,
    
    /// Output declustered CSV file (mainshocks only)
    #[arg(short, long)]
    pub output: PathBuf,
    
    /// Declustering method: gk (Gardner-Knopoff) or reasenberg
    #[arg(long, default_value = "gk")]
    pub method: String,
    
    /// Window type for GK method: gruenthal, uhrhammer, or gardnerknopoff
    #[arg(long, default_value = "gruenthal")]
    pub window: String,
    
    /// Output cluster file (optional, saves cluster assignments)
    #[arg(long)]
    pub cluster_output: Option<PathBuf>,
}

// ============================================================
// Mc Subcommand
// ============================================================
#[derive(Parser, Debug)]
pub struct McArgs {
    /// Input catalogue CSV file
    #[arg(short, long)]
    pub input: PathBuf,
    
    /// Mc estimation method: maxc, gft, or emr
    #[arg(long, default_value = "maxc")]
    pub method: String,
    
    /// Magnitude bin width
    #[arg(long, default_value_t = 0.1)]
    pub bin_width: f64,
    
    /// Confidence level for GFT method (0.90 or 0.95)
    #[arg(long, default_value_t = 0.90)]
    pub confidence: f64,
}

// ============================================================
// B-value Subcommand
// ============================================================
#[derive(Parser, Debug)]
pub struct BvalueArgs {
    /// Input catalogue CSV file
    #[arg(short, long)]
    pub input: PathBuf,
    
    /// B-value calculation method: mle or wls
    #[arg(long, default_value = "mle")]
    pub method: String,
    
    /// Magnitude of Completeness (if not provided, auto-estimate using MAXC)
    #[arg(long)]
    pub mc: Option<f64>,
    
    /// Magnitude bin width
    #[arg(long, default_value_t = 0.1)]
    pub bin_width: f64,
}

// ============================================================
// Omori Subcommand
// ============================================================
#[derive(Parser, Debug)]
pub struct OmoriArgs {
    /// Input catalogue CSV file
    #[arg(short, long)]
    pub input: PathBuf,
    
    /// Mainshock time (ISO 8601)
    #[arg(long)]
    pub mainshock_time: String,
    
    /// Minimum magnitude for aftershocks
    #[arg(long)]
    pub min_mag: Option<f64>,
    
    /// Maximum time window in days
    #[arg(long, default_value_t = 365.0)]
    pub max_days: f64,
}

// ============================================================
// Grid Subcommand
// ============================================================
#[derive(Parser, Debug)]
pub struct GridArgs {
    /// Input catalogue CSV file
    #[arg(short, long)]
    pub input: PathBuf,
    
    /// Output grid CSV file
    #[arg(short, long)]
    pub output: PathBuf,
    
    /// Gridding method: constant-n or constant-r
    #[arg(long, default_value = "constant-n")]
    pub method: String,
    
    /// Number of nearest events per grid node (for constant-n)
    #[arg(long, default_value_t = 50)]
    pub n_events: usize,
    
    /// Radius in km (for constant-r)
    #[arg(long, default_value_t = 50.0)]
    pub radius_km: f64,
    
    /// Parameter to map: bvalue, mc, avalue, rate, zvalue
    #[arg(long, default_value = "bvalue")]
    pub param: String,
    
    /// Magnitude of Completeness (if not provided, auto-estimate per node)
    #[arg(long)]
    pub mc: Option<f64>,
    
    /// Mc estimation method: maxc, gft, emr
    #[arg(long, default_value = "maxc")]
    pub mc_method: String,
    
    /// Grid longitude increment (degrees)
    #[arg(long, default_value_t = 0.1)]
    pub grid_inc: f64,
    
    /// Grid latitude increment (degrees, defaults to grid_inc)
    #[arg(long)]
    pub grid_lat_inc: Option<f64>,
    
    /// Minimum longitude for grid
    #[arg(long)]
    pub grid_min_lon: Option<f64>,
    
    /// Maximum longitude for grid
    #[arg(long)]
    pub grid_max_lon: Option<f64>,
    
    /// Minimum latitude for grid
    #[arg(long)]
    pub grid_min_lat: Option<f64>,
    
    /// Maximum latitude for grid
    #[arg(long)]
    pub grid_max_lat: Option<f64>,
    
    /// Minimum events per node to produce a valid result
    #[arg(long, default_value_t = 30)]
    pub min_events: usize,
    
    /// B-value method for gridding: mle or wls
    #[arg(long, default_value = "mle")]
    pub bvalue_method: String,
    
    /// Magnitude bin width
    #[arg(long, default_value_t = 0.1)]
    pub bin_width: f64,
    
    /// For Z-value map: Time window 1 (background) "YYYY-MM-DD/YYYY-MM-DD"
    #[arg(long)]
    pub z_tw1: Option<String>,
    
    /// For Z-value map: Time window 2 (foreground) "YYYY-MM-DD/YYYY-MM-DD"
    #[arg(long)]
    pub z_tw2: Option<String>,
    
    /// Also export as GeoTIFF
    #[arg(long)]
    pub tiff: bool,
    
    /// Slab geometry XYZ file
    #[arg(long)]
    pub slab_file: Option<PathBuf>,
    
    /// Distance buffer for slab geometry filter (km)
    #[arg(long, default_value_t = 20.0)]
    pub slab_buffer: f64,
}

// ============================================================
// Cross-Section Subcommand
// ============================================================
#[derive(Parser, Debug)]
pub struct CrossSectionArgs {
    /// Input catalogue CSV file
    #[arg(short, long)]
    pub input: PathBuf,
    
    /// Output cross-section CSV file
    #[arg(short, long)]
    pub output: PathBuf,
    
    /// Start longitude
    #[arg(long)]
    pub start_lon: f64,
    
    /// Start latitude
    #[arg(long)]
    pub start_lat: f64,
    
    /// End longitude
    #[arg(long)]
    pub end_lon: f64,
    
    /// End latitude
    #[arg(long)]
    pub end_lat: f64,
    
    /// Profile half-width in km
    #[arg(long, default_value_t = 50.0)]
    pub width_km: f64,
    
    /// Parameter: bvalue, mc, or events
    #[arg(long, default_value = "bvalue")]
    pub param: String,
    
    /// Distance increment along profile (km)
    #[arg(long, default_value_t = 5.0)]
    pub dist_inc: f64,
    
    /// Depth increment (km)
    #[arg(long, default_value_t = 5.0)]
    pub depth_inc: f64,
    
    /// Maximum depth (km)
    #[arg(long, default_value_t = 100.0)]
    pub max_depth: f64,
    
    /// Minimum events per cell
    #[arg(long, default_value_t = 30)]
    pub min_events: usize,
    
    /// Mc value
    #[arg(long)]
    pub mc: Option<f64>,
}

// ============================================================
// Plot Subcommand
// ============================================================
#[derive(Parser, Debug)]
pub struct PlotArgs {
    /// Input catalogue CSV file
    #[arg(short, long)]
    pub input: PathBuf,
    
    /// Output image file (PNG)
    #[arg(short, long)]
    pub output: PathBuf,
    
    /// Plot type: fmd, cumulative, mag-time, depth-hist, rate, time-of-day
    #[arg(long, name = "type")]
    pub plot_type: String,
    
    /// Magnitude of completeness (for FMD plot b-value line)
    #[arg(long)]
    pub mc: Option<f64>,
    
    /// B-value method for FMD: mle or wls
    #[arg(long, default_value = "mle")]
    pub bvalue_method: String,
    
    /// Bin width for FMD
    #[arg(long, default_value_t = 0.1)]
    pub bin_width: f64,
    
    /// Bin size in days for rate plot
    #[arg(long, default_value_t = 30.0)]
    pub rate_bin_days: f64,
    
    /// Image width in pixels
    #[arg(long, default_value_t = 1200)]
    pub width: u32,
    
    /// Image height in pixels
    #[arg(long, default_value_t = 800)]
    pub height: u32,
}

#[derive(Parser, Debug)]
pub struct PlotGridArgs {
    /// Input grid CSV file
    #[arg(short, long)]
    pub input: PathBuf,
    
    /// Output image file (PNG)
    #[arg(short, long)]
    pub output: PathBuf,
    
    /// Parameter name for title/colorbar (e.g., b-value)
    #[arg(long, default_value = "Grid Value")]
    pub param: String,
    
    /// Image width in pixels
    #[arg(long, default_value_t = 1200)]
    pub width: u32,
    
    /// Image height in pixels
    #[arg(long, default_value_t = 800)]
    pub height: u32,
    
    /// Radius of circles in plot
    #[arg(long, default_value_t = 5)]
    pub point_radius: u32,
}

// ============================================================
// Voronoi Subcommand
// ============================================================
#[derive(Parser, Debug)]
pub struct VoronoiArgs {
    /// Input catalogue CSV file
    #[arg(short, long)]
    pub input: PathBuf,
    
    /// Output NPZ file
    #[arg(short, long, default_value = "bvalue.npz")]
    pub output: PathBuf,
    
    /// Mode: spatial or temporal
    #[arg(long, default_value = "spatial")]
    pub mode: String,
    
    /// Minimum number of Voronoi nodes
    #[arg(long, default_value_t = 2)]
    pub n_min: usize,
    
    /// Maximum number of Voronoi nodes
    #[arg(long, default_value_t = 60)]
    pub n_max: usize,
    
    /// Init methods as comma-separated list (e.g. "sobol:30,uniform:30,data:30,kde:30,kmeans:1")
    #[arg(long, default_value = "sobol:30,uniform:30,data:30,kde:30,kmeans:1")]
    pub init: String,
    
    /// Grid resolution for interpolation
    #[arg(long, default_value_t = 200)]
    pub grid_res: usize,
    
    /// Minimum observations per Voronoi cell
    #[arg(long, default_value_t = 5)]
    pub min_obs: usize,
    
    /// Number of threads
    #[arg(long)]
    pub threads: Option<usize>,
    
    // Cross-section parameters for temporal mode
    #[arg(long)]
    pub start_lon: Option<f64>,
    #[arg(long)]
    pub start_lat: Option<f64>,
    #[arg(long)]
    pub end_lon: Option<f64>,
    #[arg(long)]
    pub end_lat: Option<f64>,
    #[arg(long)]
    pub width: Option<f64>,
    
    /// Slab geometry XYZ file
    #[arg(long)]
    pub slab_file: Option<PathBuf>,
    
    /// Distance buffer for slab geometry filter (km)
    #[arg(long, default_value_t = 20.0)]
    pub slab_buffer: f64,
}

// ============================================================
// Fractal Subcommand
// ============================================================
#[derive(Parser, Debug)]
pub struct FractalArgs {
    /// Input catalogue CSV file
    #[arg(short, long)]
    pub input: PathBuf,
    
    /// Minimum magnitude
    #[arg(long)]
    pub min_mag: Option<f64>,
    
    /// Output file for correlation integral data
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

// ============================================================
// B-value Timeseries Subcommand
// ============================================================
#[derive(Parser, Debug)]
pub struct BvalueTimeseriesArgs {
    /// Input earthquake catalogue (CSV format)
    #[arg(short, long)]
    pub input: PathBuf,
    
    /// Window size in number of events
    #[arg(short = 'w', long, default_value_t = 100)]
    pub window: usize,
    
    /// Step size in number of events
    #[arg(short = 's', long, default_value_t = 10)]
    pub step: usize,
    
    /// Fixed Magnitude of Completeness (Mc). If not provided, Mc is estimated per window.
    #[arg(short = 'm', long)]
    pub mc: Option<f64>,
    
    /// Method for Mc estimation if fixed Mc is not provided (maxc, gft, emr)
    #[arg(long, default_value = "maxc")]
    pub mc_method: String,
    
    /// Output CSV path
    #[arg(short, long, default_value = "bvalue_ts.csv")]
    pub output: PathBuf,
}

#[derive(Parser, Debug)]
pub struct StressArgs {
    /// Input focal mechanism CSV (must contain strike, dip, rake)
    #[arg(short, long)]
    pub input: PathBuf,
}

#[derive(Parser, Debug)]
pub struct VoronoiVisArgs {
    /// Input bvalue.npz file from voronoi command
    #[arg(short, long)]
    pub input: PathBuf,
    
    /// Method for visualization/export (tiff, plot)
    #[arg(short, long, default_value = "tiff")]
    pub method: String,
    
    /// Which init method to visualize (e.g. sobol:0, mean, best). Currently supports init folder name like "sobol/i_000"
    #[arg(long, default_value = "sobol/i_000")]
    pub init: String,
}

// ============================================================
// CLI Runner
// ============================================================
pub fn run_cli(args: CliArgs) {
    match args.command {
        Commands::Filter(a) => cmd_filter(a),
        Commands::Decluster(a) => cmd_decluster(a),
        Commands::Mc(a) => cmd_mc(a),
        Commands::Bvalue(a) => cmd_bvalue(a),
        Commands::Omori(a) => cmd_omori(a),
        Commands::Grid(a) => cmd_grid(a),
        Commands::CrossSection(a) => cmd_cross_section(a),
        Commands::Plot(args) => cmd_plot(args),
        Commands::PlotGrid(args) => cmd_plot_grid(&args),
        Commands::Voronoi(args) => cmd_voronoi(args),
        Commands::Fractal(args) => cmd_fractal(args),
        Commands::BvalueTimeseries(args) => cmd_bvalue_timeseries(args),
        Commands::Stress(args) => cmd_stress(args),
        Commands::VoronoiVis(args) => cmd_voronoi_vis(args),
    }
}

fn cmd_filter(args: FilterArgs) {
    use crate::core::catalogue::Catalogue;
    
    println!("SeisBox Stats — Filter");
    println!("======================");
    
    let mut cat = match Catalogue::load_csv(&args.input) {
        Ok(c) => c,
        Err(e) => { eprintln!("Error loading catalogue: {}", e); return; }
    };
    println!("Loaded {} events from {:?}", cat.events.len(), args.input);
    
    if let (Some(start), _) = (args.start_time.as_ref(), ()) {
        if let Ok(t) = chrono::NaiveDateTime::parse_from_str(start, "%Y-%m-%dT%H:%M:%S") {
            cat.filter_time_start(t);
        } else if let Ok(t) = chrono::NaiveDateTime::parse_from_str(start, "%Y-%m-%d %H:%M:%S") {
            cat.filter_time_start(t);
        }
    }
    if let Some(end) = args.end_time.as_ref() {
        if let Ok(t) = chrono::NaiveDateTime::parse_from_str(end, "%Y-%m-%dT%H:%M:%S") {
            cat.filter_time_end(t);
        } else if let Ok(t) = chrono::NaiveDateTime::parse_from_str(end, "%Y-%m-%d %H:%M:%S") {
            cat.filter_time_end(t);
        }
    }
    
    if let Some(v) = args.min_mag { cat.filter_min_mag(v); }
    if let Some(v) = args.max_mag { cat.filter_max_mag(v); }
    if let Some(v) = args.min_depth { cat.filter_min_depth(v); }
    if let Some(v) = args.max_depth { cat.filter_max_depth(v); }
    
    if args.min_lon.is_some() || args.max_lon.is_some() || args.min_lat.is_some() || args.max_lat.is_some() {
        cat.filter_bbox(
            args.min_lon.unwrap_or(-180.0),
            args.max_lon.unwrap_or(180.0),
            args.min_lat.unwrap_or(-90.0),
            args.max_lat.unwrap_or(90.0),
        );
    }
    
    if let (Some(lon), Some(lat), Some(r)) = (args.center_lon, args.center_lat, args.radius) {
        cat.filter_radial(lon, lat, r);
        println!("Filtered radial distance (center: {},{}, radius: {}km)", lon, lat, r);
    }
    
    if let Some(poly_str) = args.polygon {
        let mut polygon = Vec::new();
        for point_str in poly_str.split(';') {
            let parts: Vec<&str> = point_str.split(',').collect();
            if parts.len() == 2 {
                if let (Ok(lon), Ok(lat)) = (parts[0].parse::<f64>(), parts[1].parse::<f64>()) {
                    polygon.push((lon, lat));
                }
            }
        }
        if polygon.len() >= 3 {
            cat.filter_polygon(&polygon);
            println!("Filtered using custom polygon with {} vertices.", polygon.len());
        } else {
            eprintln!("Warning: Polygon must have at least 3 valid 'lon,lat' pairs separated by ';'. Ignoring polygon filter.");
        }
    }
    
    if let Some(slab_file) = &args.slab_file {
        if let Err(e) = cat.filter_slab(slab_file, args.slab_buffer) {
            eprintln!("Error applying slab filter: {}", e);
        } else {
            println!("Applied slab filter (buffer: {} km).", args.slab_buffer);
        }
    }
    
    println!("Events after filtering: {}", cat.events.len());
    
    if let Err(e) = cat.save_csv(&args.output) {
        eprintln!("Error saving: {}", e);
    } else {
        println!("Saved to {:?}", args.output);
    }
}

fn cmd_decluster(args: DeclusterArgs) {
    use crate::core::catalogue::Catalogue;
    use crate::core::decluster;
    
    println!("SeisBox Stats — Decluster");
    println!("=========================");
    
    let cat = match Catalogue::load_csv(&args.input) {
        Ok(c) => c,
        Err(e) => { eprintln!("Error loading catalogue: {}", e); return; }
    };
    println!("Loaded {} events", cat.events.len());
    
    let (mainshock_mask, cluster_ids) = match args.method.to_lowercase().as_str() {
        "gk" | "gardnerknopoff" | "gardner-knopoff" => {
            let window = match args.window.to_lowercase().as_str() {
                "gruenthal" => decluster::WindowType::Gruenthal,
                "uhrhammer" => decluster::WindowType::Uhrhammer,
                _ => decluster::WindowType::GardnerKnopoff,
            };
            println!("Method: Gardner-Knopoff (window: {:?})", window);
            decluster::decluster_gardner_knopoff(&cat, window)
        },
        "reasenberg" => {
            println!("Method: Reasenberg");
            decluster::decluster_reasenberg(&cat)
        },
        _ => {
            eprintln!("Unknown method: {}. Use 'gk' or 'reasenberg'.", args.method);
            return;
        }
    };
    
    let mainshock_count = mainshock_mask.iter().filter(|&&b| b).count();
    println!("Result: {} mainshocks, {} aftershocks", mainshock_count, cat.events.len() - mainshock_count);
    
    let declustered = cat.subset(&mainshock_mask);
    if let Err(e) = declustered.save_csv(&args.output) {
        eprintln!("Error saving: {}", e);
    } else {
        println!("Saved declustered catalogue to {:?}", args.output);
    }
    
    if let Some(cluster_path) = args.cluster_output {
        let mut wtr = csv::Writer::from_path(&cluster_path).unwrap();
        wtr.write_record(&["time", "lon", "lat", "depth", "mag", "is_mainshock", "cluster_id"]).unwrap();
        for (i, eq) in cat.events.iter().enumerate() {
            wtr.write_record(&[
                eq.time.format("%Y-%m-%dT%H:%M:%S").to_string(),
                eq.lon.to_string(), eq.lat.to_string(),
                eq.depth.to_string(), eq.mag.to_string(),
                if mainshock_mask[i] { "1" } else { "0" }.to_string(),
                cluster_ids[i].to_string(),
            ]).unwrap();
        }
        wtr.flush().unwrap();
        println!("Saved cluster assignments to {:?}", cluster_path);
    }
}

fn cmd_mc(args: McArgs) {
    use crate::core::catalogue::Catalogue;
    use crate::core::gutenberg_richter as gr;
    
    println!("SeisBox Stats — Magnitude of Completeness");
    println!("==========================================");
    
    let cat = match Catalogue::load_csv(&args.input) {
        Ok(c) => c,
        Err(e) => { eprintln!("Error: {}", e); return; }
    };
    
    let mags: Vec<f64> = cat.events.iter().map(|e| e.mag).collect();
    println!("Loaded {} events", mags.len());
    
    let mc = match args.method.to_lowercase().as_str() {
        "maxc" => {
            let mc = gr::mc_maxc(&mags, args.bin_width);
            println!("Method: Maximum Curvature (MAXC)");
            mc
        },
        "gft" => {
            let mc = gr::mc_gft(&mags, args.bin_width, args.confidence);
            println!("Method: Goodness-of-Fit Test (GFT, confidence={:.0}%)", args.confidence * 100.0);
            mc
        },
        "emr" => {
            let mc = gr::mc_emr(&mags, args.bin_width);
            println!("Method: Entire Magnitude Range (EMR)");
            mc
        },
        _ => {
            eprintln!("Unknown method: {}. Use 'maxc', 'gft', or 'emr'.", args.method);
            return;
        }
    };
    
    println!("Estimated Mc = {:.2}", mc);
}

fn cmd_bvalue(args: BvalueArgs) {
    use crate::core::catalogue::Catalogue;
    use crate::core::gutenberg_richter as gr;
    
    println!("SeisBox Stats — B-value Calculation");
    println!("====================================");
    
    let cat = match Catalogue::load_csv(&args.input) {
        Ok(c) => c,
        Err(e) => { eprintln!("Error: {}", e); return; }
    };
    
    let mags: Vec<f64> = cat.events.iter().map(|e| e.mag).collect();
    println!("Loaded {} events", mags.len());
    
    let mc = args.mc.unwrap_or_else(|| {
        let mc = gr::mc_maxc(&mags, args.bin_width);
        println!("Auto-estimated Mc = {:.2} (MAXC)", mc);
        mc
    });
    
    match args.method.to_lowercase().as_str() {
        "mle" => {
            let result = gr::bvalue_mle(&mags, mc, args.bin_width);
            println!("Method: Maximum Likelihood Estimation (Aki-Utsu)");
            println!("Mc      = {:.2}", mc);
            println!("b-value = {:.4} ± {:.4}", result.b, result.b_uncertainty);
            println!("a-value = {:.4}", result.a);
            println!("N(>=Mc) = {}", result.n_above_mc);
        },
        "wls" => {
            let result = gr::bvalue_wls(&mags, mc, args.bin_width);
            println!("Method: Weighted Least Squares");
            println!("Mc      = {:.2}", mc);
            println!("b-value = {:.4} ± {:.4}", result.b, result.b_uncertainty);
            println!("a-value = {:.4}", result.a);
            println!("N(>=Mc) = {}", result.n_above_mc);
        },
        _ => {
            eprintln!("Unknown method: {}. Use 'mle' or 'wls'.", args.method);
        }
    }
}

fn cmd_omori(args: OmoriArgs) {
    use crate::core::catalogue::Catalogue;
    use crate::core::omori;
    use chrono::NaiveDateTime;
    
    println!("SeisBox Stats — Omori Law Fitting");
    println!("==================================");
    
    let cat = match Catalogue::load_csv(&args.input) {
        Ok(c) => c,
        Err(e) => { eprintln!("Error: {}", e); return; }
    };
    
    let mainshock_time = match NaiveDateTime::parse_from_str(&args.mainshock_time, "%Y-%m-%dT%H:%M:%S") {
        Ok(t) => t,
        Err(_) => {
            match NaiveDateTime::parse_from_str(&args.mainshock_time, "%Y-%m-%d %H:%M:%S") {
                Ok(t) => t,
                Err(_) => {
                    eprintln!("Error: Mainshock time must be ISO 8601 format (e.g., 2023-01-01T12:00:00)");
                    return;
                }
            }
        }
    };
    
    let result = omori::fit_omori(&cat, mainshock_time, args.max_days, args.min_mag);
    println!("Omori Law Parameters (MLE):");
    println!("k = {:.4}", result.k);
    println!("c = {:.4} days", result.c);
    println!("p = {:.4}", result.p);
    println!("Log-Likelihood = {:.4}", result.log_likelihood);
}

fn cmd_grid(args: GridArgs) {
    use crate::core::catalogue::Catalogue;
    use crate::core::grid_engine::{GridConfig, GridMethod, McMethod, BvalueMethod, run_grid};
    
    println!("SeisBox Stats — Spatial Gridding");
    println!("=================================");
    
    let mut cat = match Catalogue::load_csv(&args.input) {
        Ok(c) => c,
        Err(e) => { eprintln!("Error loading catalogue: {}", e); return; }
    };
    
    if let Some(slab_file) = &args.slab_file {
        if let Err(e) = cat.filter_slab(slab_file, args.slab_buffer) {
            eprintln!("Error applying slab filter: {}", e);
            return;
        }
        println!("Applied slab filter (buffer: {} km). Remaining events: {}", args.slab_buffer, cat.events.len());
    }
    
    let method = match args.method.to_lowercase().as_str() {
        "constant-n" => GridMethod::ConstantN(args.n_events),
        "constant-r" => GridMethod::ConstantR(args.radius_km),
        _ => { eprintln!("Error: Method must be 'constant-n' or 'constant-r'"); return; }
    };
    
    let mc_method = if let Some(fixed_mc) = args.mc {
        McMethod::Fixed(fixed_mc)
    } else {
        match args.mc_method.to_lowercase().as_str() {
            "maxc" => McMethod::MaxC,
            "gft" => McMethod::Gft(0.90),
            "emr" => McMethod::Emr,
            _ => McMethod::MaxC,
        }
    };
    
    let bvalue_method = match args.bvalue_method.to_lowercase().as_str() {
        "mle" => BvalueMethod::Mle,
        "wls" => BvalueMethod::Wls,
        _ => BvalueMethod::Mle,
    };
    
    // Auto-detect bounds if not provided
    let min_lon = args.grid_min_lon.unwrap_or_else(|| cat.events.iter().map(|e| e.lon).fold(f64::INFINITY, f64::min).floor());
    let max_lon = args.grid_max_lon.unwrap_or_else(|| cat.events.iter().map(|e| e.lon).fold(f64::NEG_INFINITY, f64::max).ceil());
    let min_lat = args.grid_min_lat.unwrap_or_else(|| cat.events.iter().map(|e| e.lat).fold(f64::INFINITY, f64::min).floor());
    let max_lat = args.grid_max_lat.unwrap_or_else(|| cat.events.iter().map(|e| e.lat).fold(f64::NEG_INFINITY, f64::max).ceil());
    let dlat = args.grid_lat_inc.unwrap_or(args.grid_inc);
    
    let config = GridConfig {
        method,
        min_lon, max_lon, min_lat, max_lat,
        dlon: args.grid_inc,
        dlat,
        mc_method,
        bvalue_method,
        bin_width: args.bin_width,
        min_events: args.min_events,
        param: args.param.clone(),
        z_tw1: args.z_tw1.clone(),
        z_tw2: args.z_tw2.clone(),
    };
    
    println!("Grid bounds: Lon [{:.2}, {:.2}], Lat [{:.2}, {:.2}]", min_lon, max_lon, min_lat, max_lat);
    println!("Calculating grid points (this may take a while)...");
    
    let results = run_grid(&cat, &config);
    println!("Completed {} grid nodes successfully.", results.len());
    
    // Save to CSV
    let mut wtr = csv::Writer::from_path(&args.output).unwrap();
    wtr.write_record(&["lon", "lat", "val", "n_events", "radius_km"]).unwrap();
    for res in &results {
        let val = match args.param.to_lowercase().as_str() {
            "mc" => res.mc,
            "avalue" => res.a_value,
            "rate" => res.n_events as f64,
            "zvalue" => res.z_value,
            _ => res.b_value,
        };
        wtr.write_record(&[
            res.lon.to_string(), res.lat.to_string(), val.to_string(),
            res.n_events.to_string(), res.radius_km.to_string()
        ]).unwrap();
    }
    wtr.flush().unwrap();
    println!("Saved to {:?}", args.output);
    
    if args.tiff {
        use seisbox_core::io::tiff_export::export_grid_to_tiff;
        println!("Exporting GeoTIFF...");
        
        let width = ((config.max_lon - config.min_lon) / config.dlon).round() as u32 + 1;
        let height = ((config.max_lat - config.min_lat) / config.dlat).round() as u32 + 1;
        
        let mut grid = vec![f32::NAN; (width * height) as usize];
        let param = args.param.to_lowercase();
        
        for res in &results {
            let x_idx = ((res.lon - config.min_lon) / config.dlon).round() as u32;
            let y_idx = ((res.lat - config.min_lat) / config.dlat).round() as u32;
            
            if x_idx < width && y_idx < height {
                let tiff_y = height - 1 - y_idx;
                let idx = (tiff_y * width + x_idx) as usize;
                
                let val = match param.as_str() {
                    "mc" => res.mc,
                    "avalue" => res.a_value,
                    "rate" => res.n_events as f64,
                    "zvalue" => res.z_value,
                    _ => res.b_value,
                };
                grid[idx] = val as f32;
            }
        }
        
        let out_str = args.output.to_string_lossy().to_string();
        let base_name = if out_str.ends_with(".csv") {
            &out_str[..out_str.len() - 4]
        } else {
            &out_str
        };
        
        let tiff_path = std::path::PathBuf::from(format!("{}_{}.tif", base_name, param));
        
        if let Err(e) = export_grid_to_tiff(
            &tiff_path, &grid, width, height, 
            config.min_lon, config.max_lon, config.min_lat, config.max_lat
        ) {
            eprintln!("Error exporting GeoTIFF: {}", e);
        } else {
            println!("Saved {} GeoTIFF to {:?}", param, tiff_path);
        }
    }
}

fn cmd_cross_section(args: CrossSectionArgs) {
    use crate::core::catalogue::Catalogue;
    use crate::core::grid_engine::{CrossSectionConfig, McMethod, BvalueMethod, run_cross_section};
    
    println!("SeisBox Stats — Cross Section");
    println!("==============================");
    
    let cat = match Catalogue::load_csv(&args.input) {
        Ok(c) => c,
        Err(e) => { eprintln!("Error: {}", e); return; }
    };
    
    let mc_method = if let Some(fixed_mc) = args.mc {
        McMethod::Fixed(fixed_mc)
    } else {
        McMethod::MaxC // default for now, could be added to args
    };
    
    let config = CrossSectionConfig {
        start_lon: args.start_lon,
        start_lat: args.start_lat,
        end_lon: args.end_lon,
        end_lat: args.end_lat,
        width_km: args.width_km,
        dist_inc: args.dist_inc,
        depth_inc: args.depth_inc,
        max_depth: args.max_depth,
        mc_method,
        bvalue_method: BvalueMethod::Mle,
        bin_width: 0.1, // could be arg
        min_events: args.min_events,
    };
    
    println!("Profile: ({:.3}, {:.3}) to ({:.3}, {:.3})", config.start_lon, config.start_lat, config.end_lon, config.end_lat);
    println!("Calculating cross-section grid...");
    
    let results = run_cross_section(&cat, &config);
    println!("Completed {} grid nodes successfully.", results.len());
    
    let mut wtr = csv::Writer::from_path(&args.output).unwrap();
    wtr.write_record(&["dist_km", "depth_km", "n_events", "mc", "b_value", "b_unc", "a_value"]).unwrap();
    for res in &results {
        wtr.write_record(&[
            res.dist_km.to_string(), res.depth_km.to_string(), res.n_events.to_string(),
            res.mc.to_string(), res.b_value.to_string(), res.b_unc.to_string(), res.a_value.to_string()
        ]).unwrap();
    }
    wtr.flush().unwrap();
    println!("Saved to {:?}", args.output);
}

fn cmd_plot(args: PlotArgs) {
    use crate::core::catalogue::Catalogue;
    use crate::io::plot_stats;
    use crate::core::gutenberg_richter::{mc_maxc, bvalue_mle, bvalue_wls};
    
    println!("SeisBox Stats — Plot Generation");
    println!("================================");
    
    let cat = match Catalogue::load_csv(&args.input) {
        Ok(c) => c,
        Err(e) => { eprintln!("Error: {}", e); return; }
    };
    
    match args.plot_type.to_lowercase().as_str() {
        "fmd" => {
            let mags: Vec<f64> = cat.events.iter().map(|e| e.mag).collect();
            let mc = args.mc.unwrap_or_else(|| mc_maxc(&mags, args.bin_width));
            
            let b_res = match args.bvalue_method.to_lowercase().as_str() {
                "wls" => bvalue_wls(&mags, mc, args.bin_width),
                _ => bvalue_mle(&mags, mc, args.bin_width),
            };
            
            if let Err(e) = plot_stats::plot_fmd(&cat, &args.output, args.bin_width, Some(&b_res), args.width, args.height) {
                eprintln!("Error generating plot: {}", e);
            } else {
                println!("FMD plot saved to {:?}", args.output);
            }
        },
        "cumulative" => {
            if let Err(e) = plot_stats::plot_cumulative(&cat, &args.output, args.width, args.height) {
                eprintln!("Error generating plot: {}", e);
            } else {
                println!("Cumulative seismicity plot saved to {:?}", args.output);
            }
        },
        "mag-time" => {
            if let Err(e) = plot_stats::plot_mag_vs_time(&cat, &args.output, args.width, args.height) {
                eprintln!("Error generating plot: {}", e);
            } else {
                println!("Magnitude vs Time plot saved to {:?}", args.output);
            }
        },
        "depth-hist" => {
            if let Err(e) = plot_stats::plot_depth_histogram(&cat, &args.output, args.width, args.height) {
                eprintln!("Error generating plot: {}", e);
            } else {
                println!("Depth histogram saved to {:?}", args.output);
            }
        },
        "rate" => {
            if let Err(e) = plot_stats::plot_seismicity_rate(&cat, &args.output, 30, args.width, args.height) { // default 30 days
                eprintln!("Error generating plot: {}", e);
            } else {
                println!("Seismicity rate plot saved to {:?}", args.output);
            }
        },
        "time-of-day" => {
            if let Err(e) = plot_stats::plot_time_of_day(&cat, &args.output, args.width, args.height) {
                eprintln!("Error generating plot: {}", e);
            } else {
                println!("Time-of-day histogram saved to {:?}", args.output);
            }
        },
        _ => {
            eprintln!("Plot type '{}' is not recognized. Try 'fmd', 'cumulative', 'mag-time', 'depth-hist', 'rate', or 'time-of-day'.", args.plot_type);
        }
    }
}

fn cmd_voronoi(args: VoronoiArgs) {
    use crate::core::catalogue::Catalogue;
    use crate::core::bvor_runner::{run_bvor_ensemble, BVorConfig, BVorProgress};
    use std::sync::{Arc, Mutex};
    
    println!("SeisBox Stats — Voronoi Ensemble");
    println!("=================================");
    
    let mut cat = match Catalogue::load_csv(&args.input) {
        Ok(c) => c,
        Err(e) => { eprintln!("Error: {}", e); return; }
    };
    
    if let Some(slab_file) = &args.slab_file {
        if let Err(e) = cat.filter_slab(slab_file, args.slab_buffer) {
            eprintln!("Error applying slab filter: {}", e);
            return;
        }
        println!("Applied slab filter (buffer: {} km). Remaining events: {}", args.slab_buffer, cat.events.len());
    }
    
    let mut x: Vec<f64> = Vec::new();
    let mut y: Vec<f64> = Vec::new();
    let mut m: Vec<f64> = Vec::new();
    
    if args.mode.to_lowercase() == "temporal" {
        use crate::core::catalogue::haversine_km;
        if let (Some(slon), Some(slat), Some(elon), Some(elat), Some(width)) = (args.start_lon, args.start_lat, args.end_lon, args.end_lat, args.width) {
            let start_lat_rad = slat.to_radians();
            let start_lon_rad = slon.to_radians();
            let end_lat_rad = elat.to_radians();
            let end_lon_rad = elon.to_radians();
            
            let dlon = end_lon_rad - start_lon_rad;
            let profile_length = haversine_km(slat, slon, elat, elon);
            
            // Initial filter: bounding box slightly larger than profile
            let mut min_t = f64::INFINITY;
            let mut extracted = Vec::new();
            
            for eq in &cat.events {
                let eq_lat_rad = eq.lat.to_radians();
                let eq_lon_rad = eq.lon.to_radians();
                let dlon_eq = eq_lon_rad - start_lon_rad;
                
                let y_eq = dlon_eq.sin() * eq_lat_rad.cos();
                let x_eq = start_lat_rad.cos() * eq_lat_rad.sin() - start_lat_rad.sin() * eq_lat_rad.cos() * dlon_eq.cos();
                let bearing_eq = y_eq.atan2(x_eq);
                
                let y_prof = dlon.sin() * end_lat_rad.cos();
                let x_prof = start_lat_rad.cos() * end_lat_rad.sin() - start_lat_rad.sin() * end_lat_rad.cos() * dlon.cos();
                let bearing_prof = y_prof.atan2(x_prof);
                
                let dist_from_start = haversine_km(slat, slon, eq.lat, eq.lon);
                let angle_diff = bearing_eq - bearing_prof;
                
                let dist_along = dist_from_start * angle_diff.cos();
                let dist_across = (dist_from_start * angle_diff.sin()).abs();
                
                if dist_along >= 0.0 && dist_along <= profile_length && dist_across <= width {
                    extracted.push((dist_along, eq.time.and_utc().timestamp() as f64, eq.mag));
                    if (eq.time.and_utc().timestamp() as f64) < min_t {
                        min_t = eq.time.and_utc().timestamp() as f64;
                    }
                }
            }
            
            for (dist_along, time_sec, mag) in extracted {
                x.push(dist_along);
                y.push((time_sec - min_t) / 86400.0); // Convert to days since first event
                m.push(mag);
            }
            println!("Temporal Voronoi: Projected {} events into cross-section.", x.len());
        } else {
            eprintln!("Error: Temporal mode requires --start-lon, --start-lat, --end-lon, --end-lat, and --width");
            return;
        }
    } else {
        x = cat.events.iter().map(|e| e.lon).collect();
        y = cat.events.iter().map(|e| e.lat).collect();
        m = cat.events.iter().map(|e| e.mag).collect();
    }
    
    // Parse init methods
    let mut init_methods = Vec::new();
    for part in args.init.split(',') {
        let parts: Vec<&str> = part.split(':').collect();
        if parts.len() == 2 {
            if let Ok(count) = parts[1].parse::<usize>() {
                init_methods.push((parts[0].to_string(), count));
            }
        } else {
            init_methods.push((part.to_string(), 1));
        }
    }
    
    let config = BVorConfig {
        mode: args.mode.clone(),
        n_nodes_range: args.n_min..=args.n_max,
        init_methods,
        grid_res: args.grid_res,
        min_obs: args.min_obs,
        num_threads: args.threads.unwrap_or_else(num_cpus::get),
        normalize_axes: args.mode.to_lowercase() == "temporal",
    };
    
    let progress = Arc::new(Mutex::new(BVorProgress {
        total: 0,
        completed: 0,
        current_status: String::new(),
        log_messages: Vec::new(),
    }));
    
    let progress_clone = progress.clone();
    
    // Spawn a thread to monitor progress and print to stdout
    let is_done = Arc::new(Mutex::new(false));
    let is_done_clone = is_done.clone();
    
    std::thread::spawn(move || {
        while !*is_done_clone.lock().unwrap() {
            let p = progress_clone.lock().unwrap();
            if p.total > 0 {
                print!("\rProgress: {} / {} ({:.1}%)", 
                       p.completed, p.total, 
                       (p.completed as f64 / p.total as f64) * 100.0);
                use std::io::Write;
                std::io::stdout().flush().unwrap();
            }
            drop(p);
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    });
    
    match run_bvor_ensemble(config, x, y, m, progress, args.output.to_str().unwrap_or("bvalue.npz")) {
        Ok(_) => {
            *is_done.lock().unwrap() = true;
            println!("\nVoronoi ensemble completed successfully.");
            println!("Results saved to {:?}", args.output);
        },
        Err(e) => {
            *is_done.lock().unwrap() = true;
            eprintln!("\nError running Voronoi ensemble: {}", e);
        }
    }
}

fn cmd_fractal(args: FractalArgs) {
    use crate::core::catalogue::Catalogue;
    use crate::core::fractal::fractal_dimension;
    
    println!("SeisBox Stats — Fractal Dimension");
    println!("==================================");
    
    let cat = match Catalogue::load_csv(&args.input) {
        Ok(c) => c,
        Err(e) => { eprintln!("Error: {}", e); return; }
    };
    
    let num_bins = 50;
    println!("Calculating Fractal Correlation Dimension (D-value)...");
    let (res, r_bins, c_r) = fractal_dimension(&cat, args.min_mag, num_bins);
    
    println!("D-value (Fractal Dimension) = {:.4}", res.d_value);
    
    if let Some(out_path) = args.output {
        let mut wtr = csv::Writer::from_path(&out_path).unwrap();
        wtr.write_record(&["r", "C_r"]).unwrap();
        for i in 0..num_bins {
            if c_r[i] > 0.0 {
                wtr.write_record(&[r_bins[i].to_string(), c_r[i].to_string()]).unwrap();
            }
        }
        wtr.flush().unwrap();
        println!("Saved correlation integral data to {:?}", out_path);
    }
}

fn cmd_bvalue_timeseries(args: BvalueTimeseriesArgs) {
    use crate::core::catalogue::Catalogue;
    use crate::core::bvalue_timeseries::run_bvalue_timeseries;
    
    println!("SeisBox Stats — B-value Timeseries");
    println!("====================================");
    
    let cat = match Catalogue::load_csv(&args.input) {
        Ok(c) => c,
        Err(e) => { eprintln!("Error: {}", e); return; }
    };
    
    println!("Calculating B-value Timeseries...");
    println!("Window: {} events, Step: {} events", args.window, args.step);
    
    let results = run_bvalue_timeseries(&cat, args.window, args.step, args.mc, &args.mc_method);
    println!("Completed {} timeseries windows.", results.len());
    
    let mut wtr = csv::Writer::from_path(&args.output).unwrap();
    wtr.write_record(&[
        "window_start", "window_end", "window_center", "n_events", 
        "mc", "b_value", "b_unc", "a_value"
    ]).unwrap();
    
    for res in &results {
        wtr.write_record(&[
            res.window_start_time.format("%Y-%m-%dT%H:%M:%S").to_string(),
            res.window_end_time.format("%Y-%m-%dT%H:%M:%S").to_string(),
            res.window_center_time.format("%Y-%m-%dT%H:%M:%S").to_string(),
            res.n_events.to_string(),
            res.mc.to_string(),
            res.b_value.to_string(),
            res.b_unc.to_string(),
            res.a_value.to_string(),
        ]).unwrap();
    }
    wtr.flush().unwrap();
    println!("Saved to {:?}", args.output);
}

fn cmd_stress(args: StressArgs) {
    use crate::core::stress_inversion::{load_focal_mechanisms, invert_stress_tensor};
    
    println!("SeisBox Stats — Stress Tensor Inversion");
    println!("========================================");
    
    let fms = match load_focal_mechanisms(&args.input) {
        Ok(data) => data,
        Err(e) => { eprintln!("Error loading focal mechanisms: {}", e); return; }
    };
    
    println!("Loaded {} focal mechanisms.", fms.len());
    
    match invert_stress_tensor(&fms) {
        Ok(res) => {
            println!("Principal Stress Axes (Azimuth, Plunge):");
            println!("Sigma 1 (Maximum): Azimuth = {:.1}°, Plunge = {:.1}°", res.sigma1.0, res.sigma1.1);
            println!("Sigma 2 (Intermediate): Azimuth = {:.1}°, Plunge = {:.1}°", res.sigma2.0, res.sigma2.1);
            println!("Sigma 3 (Minimum): Azimuth = {:.1}°, Plunge = {:.1}°", res.sigma3.0, res.sigma3.1);
        },
        Err(e) => {
            eprintln!("Error computing stress tensor: {}", e);
        }
    }
}

fn cmd_voronoi_vis(args: VoronoiVisArgs) {
    use std::fs::File;
    use std::io::Read;
    use zip::ZipArchive;
    use serde_json::Value;
    
    println!("SeisBox Stats — Voronoi Visualization");
    println!("========================================");
    
    let file = match File::open(&args.input) {
        Ok(f) => f,
        Err(e) => { eprintln!("Error opening file: {}", e); return; }
    };
    
    let mut archive = match ZipArchive::new(file) {
        Ok(a) => a,
        Err(e) => { eprintln!("Error reading ZIP archive: {}", e); return; }
    };
    
    let mut meta_str = String::new();
    if let Ok(mut meta_file) = archive.by_name("metadata.json") {
        meta_file.read_to_string(&mut meta_str).unwrap();
    } else {
        eprintln!("metadata.json not found in archive.");
        return;
    }
    
    let meta: Value = serde_json::from_str(&meta_str).unwrap();
    let grid_res = meta["grid_res"].as_u64().unwrap() as u32;
    let x_min = meta["x_min"].as_f64().unwrap();
    let x_max = meta["x_max"].as_f64().unwrap();
    let y_min = meta["y_min"].as_f64().unwrap();
    let y_max = meta["y_max"].as_f64().unwrap();
    
    let grid_path = format!("{}/b_grid.raw", args.init);
    
    let mut b_grid_raw = Vec::new();
    if let Ok(mut grid_file) = archive.by_name(&grid_path) {
        grid_file.read_to_end(&mut b_grid_raw).unwrap();
    } else {
        eprintln!("{} not found in archive.", grid_path);
        return;
    }
    
    let n_elements = b_grid_raw.len() / 8;
    if n_elements != (grid_res * grid_res) as usize {
        eprintln!("Grid size mismatch. Expected {}, got {}", grid_res * grid_res, n_elements);
        return;
    }
    
    // The raw data is f64, we need f32 for TIFF
    let mut b_grid_f32 = vec![0.0f32; n_elements];
    let bytes = b_grid_raw.as_slice();
    for i in 0..n_elements {
        let mut b = [0u8; 8];
        b.copy_from_slice(&bytes[i*8..(i+1)*8]);
        let val = f64::from_ne_bytes(b);
        
        // Voronoi grid iterates y outer loop, x inner loop (y from y_min to y_max)
        // TIFF needs y inverted (y_max to y_min)
        let y_idx = (i as u32) / grid_res;
        let x_idx = (i as u32) % grid_res;
        let tiff_y = grid_res - 1 - y_idx;
        let tiff_idx = (tiff_y * grid_res + x_idx) as usize;
        
        b_grid_f32[tiff_idx] = val as f32;
    }
    
    let out_path = std::path::PathBuf::from(format!("voronoi_{}.tif", args.init.replace("/", "_")));
    use seisbox_core::io::tiff_export::export_grid_to_tiff;
    
    println!("Exporting to GeoTIFF: {:?}", out_path);
    if let Err(e) = export_grid_to_tiff(&out_path, &b_grid_f32, grid_res, grid_res, x_min, x_max, y_min, y_max) {
        eprintln!("Error exporting TIFF: {}", e);
    } else {
        println!("Successfully exported GeoTIFF.");
    }
}

fn cmd_plot_grid(args: &PlotGridArgs) {
    println!("SeisBox Stats — Plot Grid");
    println!("==========================");
    
    if let Err(e) = crate::io::plot_stats::plot_spatial_grid(
        &args.input,
        &args.output,
        &args.param,
        args.width,
        args.height,
        args.point_radius,
    ) {
        eprintln!("Error plotting grid: {}", e);
    } else {
        println!("Successfully saved grid plot to {:?}", args.output);
    }
}
