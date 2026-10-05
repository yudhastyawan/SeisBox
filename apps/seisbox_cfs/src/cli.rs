use clap::Parser;
use std::path::PathBuf;
use crate::core::cfs_parser::{CoulombInput, BatchInput, open_input_file_cui, open_batch_file};
use crate::core::cfs_runner::{calculate_coulomb_grid, calculate_coulomb_batch, calculate_coulomb_grid_oof, calculate_coulomb_cross_section};
use crate::core::cfs_math::{build_regional_stress_tensor, regional_tensor_to_voigt};
use crate::core::cfs_io::{write_coulomb_csv, write_coulomb_tiff, write_cross_section_csv};

#[derive(Parser, Debug)]
#[command(
    author = "Yudha Styawan, Geophysical Engineering, Institut Teknologi Sumatera, Indonesia", 
    version, 
    about = "SeisBox CFS Command Line Interface", 
    allow_negative_numbers = true,
    long_about = "\
SeisBox CFS Command Line Interface

Author: Yudha Styawan, Geophysical Engineering, Institut Teknologi Sumatera, Indonesia

CREDITS & DISCLAIMER:
This software is a modern CLI replica of the original Coulomb 3.3 software developed by 
Shinji Toda, Ross S. Stein, Volkan Sevilgen, and Jian Lin (USGS). It is developed solely 
for the purpose of facilitating academic research and ease of use. Please cite the original 
Coulomb 3.3 authors when using the underlying mathematical geomechanics models.

BATCH FILE FORMAT:
If using the --batch mode, the file must be a CSV format with a header row.
The header must contain the following columns (case-insensitive):
- Coordinates: 'lon' and 'lat' (degrees) OR 'x' and 'y' (Cartesian km)
- Depth: 'z' (km)
- Mechanism: 'strike', 'dip', 'rake' (degrees)

Example CSV content:
lon,lat,z,strike,dip,rake
127.5,-1.2,10.0,45.0,90.0,0.0
",
    after_help = "\
EXAMPLES:

1. Generate a new INP file from manual coordinates:
   seisbox_cfs --generate-inp file_baru.inp --gen-strike 45.0 --gen-dip 80.0 \\
      --gen-rake 90.0 --gen-lon 128.0 --gen-lat -1.0 --gen-mag 7.2 \\
      --gen-fault-sense ss

2. Validate an existing INP file before running calculations:
   seisbox_cfs -i fault.inp --validate

3. Calculate Standard Grid with specific depths and output to CSV:
   seisbox_cfs -i fault.inp --use-source-mech --depth 10.0 \\
      --depth-finish 15.0 --depth-inc 5.0 -o output.csv

4. Calculate using a Batch file of receiver faults:
   seisbox_cfs -i fault.inp -b receivers.csv -o batch_output.csv

5. Calculate Optimally Oriented Fault (OOF) using regional stress:
   seisbox_cfs -i fault.inp --oof --tiff --depth 10.0 -o oof.csv

6. Calculate Cross-Section profiling:
   seisbox_cfs -i fault.inp --cross-section --use-source-mech \\
      --cs-start-lon 127.0 --cs-finish-lon 130.0 \\
      --cs-start-lat -2.0 --cs-finish-lat 1.0 -o cross_section.csv

7. Generate a custom high-resolution plot from a CSV file:
   seisbox_cfs --plot-csv output.csv --plot-contourf --plot-width 1920 \\
      --plot-height 1080 --plot-title \"My Custom Plot\" \\
      --plot-title-size 60 --plot-cbar-extend both
"
)]
pub struct CliArgs {
    /// Path to the source earthquake input file (.inp)
    #[arg(short, long, help_heading = "Input / Output (Required)")]
    pub input: Option<PathBuf>,

    /// Path to the batch receiver fault file (.csv or .txt)
    #[arg(short, long, help_heading = "Batch Mode")]
    pub batch: Option<PathBuf>,

    /// File path to save the output CSV
    #[arg(short, long, help_heading = "Input / Output (Required)")]
    pub output: Option<PathBuf>,

    #[arg(long, help = "Extract and show the input file information\n(strike, dip, rake, physics, etc) without calculating")]
    #[arg(long, help_heading = "Information")]
    pub show_info: bool,

    /// Validate the input file to ensure it's readable, complete, and ready for calculation
    #[arg(long, help_heading = "Information")]
    pub validate: bool,

    /// Generate a new Coulomb INP file
    #[arg(long, help_heading = "Generator Mode")]
    pub generate_inp: Option<PathBuf>,

    /// Append a new fault to an existing Coulomb INP file
    #[arg(long, help_heading = "Generator Mode")]
    pub append_inp: Option<PathBuf>,

    /// Generator fault strike angle (degrees)
    #[arg(long, default_value_t = 0.0, help_heading = "Generator Mode")]
    pub gen_strike: f64,

    /// Generator fault dip angle (degrees)
    #[arg(long, default_value_t = 90.0, help_heading = "Generator Mode")]
    pub gen_dip: f64,

    /// Generator fault rake angle (degrees)
    #[arg(long, default_value_t = 0.0, help_heading = "Generator Mode")]
    pub gen_rake: f64,

    /// Generator fault length (km). Defaults to 50.0 or derived from --gen-mag
    #[arg(long, help_heading = "Generator Mode")]
    pub gen_length: Option<f64>,

    /// Generator fault width (km). Defaults to 20.0 or derived from --gen-mag
    #[arg(long, help_heading = "Generator Mode")]
    pub gen_width: Option<f64>,

    /// Generator fault center depth (km)
    #[arg(long, default_value_t = 10.0, help_heading = "Generator Mode")]
    pub gen_depth: f64,

    /// Generator fault slip amount (m). Defaults to 1.0 or derived from --gen-mag
    #[arg(long, help_heading = "Generator Mode")]
    pub gen_slip: Option<f64>,

    #[arg(long, help = "Generator magnitude. If provided, length, width,\nand slip are calculated automatically using empirical relationships")]
    #[arg(long, help_heading = "Generator Mode")]
    pub gen_mag: Option<f64>,

    #[arg(long, default_value = "all", help = "Fault sense for magnitude calculation:\nall, ss (strike-slip), rev (reverse), norm (normal)")]
    #[arg(long, help_heading = "Generator Mode")]
    pub gen_fault_sense: String,

    /// Generator epicenter longitude (center of the grid)
    #[arg(long, default_value_t = 0.0, help_heading = "Generator Mode")]
    pub gen_lon: f64,

    /// Generator epicenter latitude (center of the grid)
    #[arg(long, default_value_t = 0.0, help_heading = "Generator Mode")]
    pub gen_lat: f64,

    /// Grid size limit (radius in degrees) from epicenter
    #[arg(long, default_value_t = 1.0, help_heading = "Generator Mode")]
    pub gen_grid_size: f64,

    /// Receiver fault strike (if not using batch file)
    #[arg(short, long, default_value_t = 0.0, help_heading = "Standard Grid Mode")]
    pub strike: f64,

    /// Receiver fault dip (if not using batch file)
    #[arg(short, long, default_value_t = 90.0, help_heading = "Standard Grid Mode")]
    pub dip: f64,

    /// Receiver fault rake (if not using batch file)
    #[arg(short, long, default_value_t = 0.0, help_heading = "Standard Grid Mode")]
    pub rake: f64,

    /// Depth for calculation (start depth)
    #[arg(long, default_value_t = 0.0, help_heading = "Standard Grid Mode")]
    pub depth: f64,

    /// Override finish depth (if specified, calculates multiple depths)
    #[arg(long, help_heading = "Standard Grid Mode")]
    pub depth_finish: Option<f64>,

    /// Override depth increment
    #[arg(long, help_heading = "Standard Grid Mode")]
    pub depth_inc: Option<f64>,

    /// Override grid start X
    #[arg(long, help_heading = "Grid Bounds Overrides (Optional)")]
    pub grid_start_x: Option<f64>,

    /// Override grid finish X
    #[arg(long, help_heading = "Grid Bounds Overrides (Optional)")]
    pub grid_finish_x: Option<f64>,

    /// Override grid start Y
    #[arg(long, help_heading = "Grid Bounds Overrides (Optional)")]
    pub grid_start_y: Option<f64>,

    /// Override grid finish Y
    #[arg(long, help_heading = "Grid Bounds Overrides (Optional)")]
    pub grid_finish_y: Option<f64>,

    /// Override grid X increment
    #[arg(long, help_heading = "Grid Bounds Overrides (Optional)")]
    pub grid_x_inc: Option<f64>,

    /// Override grid Y increment
    #[arg(long, help_heading = "Grid Bounds Overrides (Optional)")]
    pub grid_y_inc: Option<f64>,

    /// Override grid start Longitude
    #[arg(long, help_heading = "Grid Bounds Overrides (Optional)")]
    pub grid_start_lon: Option<f64>,

    /// Override grid finish Longitude
    #[arg(long, help_heading = "Grid Bounds Overrides (Optional)")]
    pub grid_finish_lon: Option<f64>,

    /// Override grid start Latitude
    #[arg(long, help_heading = "Grid Bounds Overrides (Optional)")]
    pub grid_start_lat: Option<f64>,

    /// Override grid finish Latitude
    #[arg(long, help_heading = "Grid Bounds Overrides (Optional)")]
    pub grid_finish_lat: Option<f64>,

    /// Override grid Longitude increment (degrees)
    #[arg(long, help_heading = "Grid Bounds Overrides (Optional)")]
    pub grid_lon_inc: Option<f64>,

    /// Override grid Latitude increment (degrees)
    #[arg(long, help_heading = "Grid Bounds Overrides (Optional)")]
    pub grid_lat_inc: Option<f64>,

    /// Override Friction Coefficient (FRIC)
    #[arg(long, help_heading = "Physical Parameter Overrides (Optional)")]
    pub fric: Option<f64>,

    /// Override Poisson's Ratio (PR1/PR2)
    #[arg(long, help_heading = "Physical Parameter Overrides (Optional)")]
    pub poisson: Option<f64>,

    /// Override Young's Modulus (E1/E2)
    #[arg(long, help_heading = "Physical Parameter Overrides (Optional)")]
    pub young: Option<f64>,

    #[arg(long, help = "Use the source fault mechanism (average strike, dip, rake\nfrom INP) for the receiver fault")]
    #[arg(long, help_heading = "Standard Grid Mode")]
    pub use_source_mech: bool,

    /// Enable Optimally Oriented Fault (OOF) mode.
    /// Receiver fault geometry is determined from the total stress field
    /// (regional tectonic stress + coseismic perturbation) using Mohr-Coulomb
    /// failure theory. Requires regional stress parameters (from INP file or
    /// --regional-* overrides).
    #[arg(long, help_heading = "Optimally Oriented Fault (OOF)")]
    pub oof: bool,

    /// Override regional stress magnitude σ₁−σ₃ (bar)
    #[arg(long, help_heading = "Optimally Oriented Fault (OOF)")]
    pub regional_mag: Option<f64>,

    /// Override regional stress σ₁ azimuth (degrees CW from North)
    #[arg(long, help_heading = "Optimally Oriented Fault (OOF)")]
    pub regional_azimuth: Option<f64>,

    /// Override regional stress σ₁ plunge (degrees downward from horizontal)
    #[arg(long, help_heading = "Optimally Oriented Fault (OOF)")]
    pub regional_plunge: Option<f64>,

    /// Output a TIFF raster image along with the CSV (Grid mode only)
    #[arg(long, help_heading = "Output Options")]
    pub tiff: bool,

    /// Save the maximum Coulomb stress across all depth layers (Grid mode only)
    #[arg(long, help_heading = "Output Options")]
    pub max_depth: bool,

    /// Enable cross-section calculation mode
    #[arg(long, help_heading = "Cross Section Mode")]
    pub cross_section: bool,

    /// Cross-section start longitude (degrees)
    #[arg(long, num_args = 1.., help_heading = "Cross Section Mode")]
    pub cs_start_lon: Option<Vec<f64>>,

    /// Cross-section finish longitude (degrees)
    #[arg(long, num_args = 1.., help_heading = "Cross Section Mode")]
    pub cs_finish_lon: Option<Vec<f64>>,

    /// Cross-section start latitude (degrees)
    #[arg(long, num_args = 1.., help_heading = "Cross Section Mode")]
    pub cs_start_lat: Option<Vec<f64>>,

    /// Cross-section finish latitude (degrees)
    #[arg(long, num_args = 1.., help_heading = "Cross Section Mode")]
    pub cs_finish_lat: Option<Vec<f64>>,

    /// Distance increment for cross-section (km)
    #[arg(long, default_value_t = 2.0, help_heading = "Cross Section Mode")]
    pub cs_dist_inc: f64,

    /// Read an existing CSV output file and plot it directly to PNG
    #[arg(long, help_heading = "Plotting Engine")]
    pub plot_csv: Option<PathBuf>,

    /// Specific output PNG file path for the plot
    #[arg(long, help_heading = "Plotting Engine")]
    pub plot_out: Option<PathBuf>,

    /// Force aspect ratio to 1:1 for the plot
    #[arg(long, help_heading = "Plotting Engine")]
    pub plot_aspect_equal: bool,

    /// Enable smooth contour plotting (contourf) instead of blocky pcolormesh
    #[arg(long, help_heading = "Plotting Engine")]
    pub plot_contourf: bool,

    /// Overlay faults from an INP file onto the plot
    #[arg(long, help_heading = "Plotting Engine Options")]
    pub plot_inp: Option<PathBuf>,

    /// Override minimum value for colormap (vmin) (default: auto calculated from data)
    #[arg(long, help_heading = "Plotting Engine Options")]
    pub plot_vmin: Option<f64>,

    /// Override maximum value for colormap (vmax) (default: auto calculated from data)
    #[arg(long, help_heading = "Plotting Engine Options")]
    pub plot_vmax: Option<f64>,

    /// Set a custom title for the plot (default: dynamic based on mode)
    #[arg(long, help_heading = "Plotting Engine Options")]
    pub plot_title: Option<String>,

    /// Set custom output image width (default: 1200)
    #[arg(long, help_heading = "Plotting Engine Options")]
    pub plot_width: Option<u32>,

    /// Set custom output image height (default: 800)
    #[arg(long, help_heading = "Plotting Engine Options")]
    pub plot_height: Option<u32>,

    /// Set the number of color contour steps (default: 50)
    #[arg(long, help_heading = "Plotting Engine Options")]
    pub plot_contour_steps: Option<u32>,

    /// Set the upsampling resolution for contourf (default: 300)
    #[arg(long, help_heading = "Plotting Engine Options")]
    pub plot_upsample_res: Option<u32>,

    /// Set font size for the plot title (default: 45)
    #[arg(long, help_heading = "Plotting Engine Options")]
    pub plot_title_size: Option<u32>,

    /// Set font size for the axis labels (default: 32)
    #[arg(long, help_heading = "Plotting Engine Options")]
    pub plot_label_size: Option<u32>,

    /// Set font size for the axis ticks (default: 24)
    #[arg(long, help_heading = "Plotting Engine Options")]
    pub plot_tick_size: Option<u32>,

    /// Set the maximum number of x-axis tick labels (default: 10)
    #[arg(long, help_heading = "Plotting Engine Options")]
    pub plot_x_labels: Option<usize>,

    /// Set the maximum number of y-axis tick labels (default: 10)
    #[arg(long, help_heading = "Plotting Engine Options")]
    pub plot_y_labels: Option<usize>,

    /// Set custom title for the colorbar (default: Coulomb Stress Change (bar))
    #[arg(long, help_heading = "Plotting Engine Options")]
    pub plot_cbar_label: Option<String>,

    /// Set custom font size for the colorbar title (default: follows --plot-label-size)
    #[arg(long, help_heading = "Plotting Engine Options")]
    pub plot_cbar_label_size: Option<u32>,

    /// Font size for colorbar ticks
    #[arg(long, help = "Font size for colorbar ticks")]
    pub plot_cbar_tick_size: Option<u32>,

    /// Fault top-edge line color (e.g. black, red, blue, magenta, white)
    #[arg(long, help = "Color of the fault's top edge line (default: black)")]
    pub plot_fault_color: Option<String>,
    
    /// Fault top-edge line width
    #[arg(long, help = "Thickness of the fault's top edge line (default: 4)")]
    pub plot_fault_width: Option<u32>,
    
    /// Fault top-edge line style (solid or dashed)
    #[arg(long, help = "Style of the fault's top edge line: 'solid' or 'dashed' (default: solid)")]
    pub plot_fault_style: Option<String>,

    /// Draw cross-section track on map
    #[arg(long, help = "Draw the cross-section track line on the map\n(requires --cs-start-lon, --cs-finish-lon, etc.)")]
    pub plot_cs_track: bool,

    /// CS track line color
    #[arg(long, help = "Color of the cross-section track line (default: black)")]
    pub plot_cs_track_color: Option<String>,

    /// CS track line width
    #[arg(long, help = "Thickness of the cross-section track line (default: 5)")]
    pub plot_cs_track_width: Option<u32>,

    /// CS track line style (solid or dashed)
    #[arg(long, help = "Style of the cross-section track line: 'solid' or 'dashed' (default: solid)")]
    pub plot_cs_track_style: Option<String>,

    /// Y-axis labels count for colorbar tick labels (default: 10)
    #[arg(long, help_heading = "Plotting Engine Options")]
    pub plot_cbar_y_labels: Option<usize>,

    /// Set colorbar extend arrows (none, min, max, both) (default: both)
    #[arg(long, help_heading = "Plotting Engine Options")]
    pub plot_cbar_extend: Option<String>,
}

pub fn parse_and_run(args: CliArgs) {
    // Plotting mode check
    if let Some(csv_path) = &args.plot_csv {
        let out_path = args.plot_out.clone().unwrap_or_else(|| {
            let mut p = csv_path.clone();
            p.set_extension("png");
            p
        });
        
        println!("Running SeisBox Plot Engine...");
        
        let config = crate::plot::cfs_plotter::PlotConfig {
            csv_path: csv_path.to_string_lossy().to_string(),
            out_path: out_path.to_string_lossy().to_string(),
            aspect_equal: args.plot_aspect_equal,
            use_contourf: args.plot_contourf,
            vmin: args.plot_vmin,
            vmax: args.plot_vmax,
            title: args.plot_title,
            width: args.plot_width.unwrap_or(1200),
            height: args.plot_height.unwrap_or(800),
            title_size: args.plot_title_size.unwrap_or(45),
            label_size: args.plot_label_size.unwrap_or(32),
            tick_size: args.plot_tick_size.unwrap_or(24),
            x_labels: args.plot_x_labels.unwrap_or(10),
            y_labels: args.plot_y_labels.unwrap_or(10),
            cbar_label: args.plot_cbar_label,
            cbar_label_size: args.plot_cbar_label_size,
            cbar_tick_size: args.plot_cbar_tick_size,
            cbar_y_labels: args.plot_cbar_y_labels,
            cbar_extend: args.plot_cbar_extend,
            contour_steps: args.plot_contour_steps.unwrap_or(50),
            upsample_res: args.plot_upsample_res.unwrap_or(300),
            plot_inp: args.plot_inp,
            fault_color: args.plot_fault_color,
            fault_width: args.plot_fault_width,
            fault_style: args.plot_fault_style,
            plot_cs_track: args.plot_cs_track,
            cs_track_color: args.plot_cs_track_color,
            cs_track_width: args.plot_cs_track_width,
            cs_track_style: args.plot_cs_track_style,
            cs_start_lon: args.cs_start_lon.clone(),
            cs_start_lat: args.cs_start_lat.clone(),
            cs_finish_lon: args.cs_finish_lon.clone(),
            cs_finish_lat: args.cs_finish_lat.clone(),
        };
        
        if let Err(e) = crate::plot::cfs_plotter::plot_cfs_csv(&config) {
            eprintln!("Failed to generate plot: {}", e);
        }
        return;
    }

    // Generator mode check
    if let Some(out_path) = &args.generate_inp {
        println!("Generating new INP file at: {:?}", out_path);
        
        let min_lon = args.gen_lon - args.gen_grid_size;
        let max_lon = args.gen_lon + args.gen_grid_size;
        let min_lat = args.gen_lat - args.gen_grid_size;
        let max_lat = args.gen_lat + args.gen_grid_size;
        
        // Approximate to km
        let earth_r = 6371.0;
        let rad_conv = 180.0 / std::f64::consts::PI;
        let cos_lat = args.gen_lat.to_radians().cos();
        
        let min_x = (min_lon - args.gen_lon) * (earth_r * cos_lat) / rad_conv;
        let max_x = (max_lon - args.gen_lon) * (earth_r * cos_lat) / rad_conv;
        let min_y = (min_lat - args.gen_lat) * earth_r / rad_conv;
        let max_y = (max_lat - args.gen_lat) * earth_r / rad_conv;
        
        let x_inc = (max_x - min_x) / 50.0;
        let y_inc = (max_y - min_y) / 50.0;
        
        // Empirical calculations if magnitude is provided
        let mut calc_length = 50.0;
        let mut calc_width = 20.0;
        let mut calc_slip = 1.0;

        if let Some(mag) = args.gen_mag {
            let (al, bl, aw, bw) = match args.gen_fault_sense.to_lowercase().as_str() {
                "ss" | "strikeslip" => (4.33, 1.49, 3.80, 2.59),
                "rev" | "reverse" => (4.49, 1.49, 4.37, 1.95),
                "norm" | "normal" => (4.34, 1.54, 4.04, 2.11),
                _ => (4.38, 1.49, 4.06, 2.25), // "all"
            };
            calc_length = 10f64.powf((mag - al) / bl);
            calc_width = 10f64.powf((mag - aw) / bw);
            
            let mo = 10f64.powf(1.5 * mag + 9.1);
            let mu = 3.4e10; // Coulomb 3.4 uses 3.4e11 dyne/cm^2 = 3.4e10 N/m^2
            let area = calc_length * 1000.0 * calc_width * 1000.0;
            calc_slip = mo / (mu * area);
            println!("Magnitude {} ({}) -> Length: {:.2} km, Width: {:.2} km, Slip: {:.3} m", 
                mag, args.gen_fault_sense, calc_length, calc_width, calc_slip);
        }

        let final_length = args.gen_length.unwrap_or(calc_length);
        let final_width = args.gen_width.unwrap_or(calc_width);
        let final_slip = args.gen_slip.unwrap_or(calc_slip);

        let content = crate::core::cfs_io::generate_coulomb_inp_content(
            args.gen_strike, args.gen_dip, args.gen_rake,
            final_length, final_width, args.gen_depth, final_slip,
            min_x, max_x, x_inc,
            min_y, max_y, y_inc,
            min_lon, max_lon, args.gen_lon,
            min_lat, max_lat, args.gen_lat,
            args.fric.unwrap_or(0.400), args.poisson.unwrap_or(0.250), args.young.unwrap_or(800000.0)
        );
        
        if let Err(e) = std::fs::write(out_path, content) {
            eprintln!("Error writing INP file: {}", e);
            std::process::exit(1);
        }
        println!("Generation complete. File ready for use.");
        std::process::exit(0);
    }

    // Append mode check
    if let Some(target_inp) = &args.append_inp {
        println!("Appending to existing INP file at: {:?}", target_inp);
        
        let mut coulomb_input = match open_input_file_cui(target_inp) {
            Ok(inp) => inp,
            Err(e) => {
                eprintln!("Error reading target INP file: {}", e);
                std::process::exit(1);
            }
        };
        
        let new_fault_id = coulomb_input.el.len() + 1;
        
        // Calculate offset from map zero
        let zero_lon = coulomb_input.map_info.zero_lon;
        let zero_lat = coulomb_input.map_info.zero_lat;
        
        let earth_r = 6371.0;
        let rad_conv = 180.0 / std::f64::consts::PI;
        let cos_lat = zero_lat.to_radians().cos();
        
        let fault_center_x = (args.gen_lon - zero_lon) * (earth_r * cos_lat) / rad_conv;
        let fault_center_y = (args.gen_lat - zero_lat) * earth_r / rad_conv;
        
        let mut calc_length = 50.0;
        let mut calc_width = 20.0;
        let mut calc_slip = 1.0;
        if let Some(mag) = args.gen_mag {
            let (al, bl, aw, bw) = match args.gen_fault_sense.to_lowercase().as_str() {
                "ss" | "strikeslip" => (4.33, 1.49, 3.80, 2.59),
                "rev" | "reverse" => (4.49, 1.49, 4.37, 1.95),
                "norm" | "normal" => (4.34, 1.54, 4.04, 2.11),
                _ => (4.38, 1.49, 4.06, 2.25), // "all"
            };
            calc_length = 10f64.powf((mag - al) / bl);
            calc_width = 10f64.powf((mag - aw) / bw);
            let mo = 10f64.powf(1.5 * mag + 9.1);
            calc_slip = mo / (3.4e10 * calc_length * 1000.0 * calc_width * 1000.0);
            println!("Magnitude {} ({}) -> Length: {:.2} km, Width: {:.2} km, Slip: {:.3} m", 
                mag, args.gen_fault_sense, calc_length, calc_width, calc_slip);
        }
        
        let final_length = args.gen_length.unwrap_or(calc_length);
        let final_width = args.gen_width.unwrap_or(calc_width);
        let final_slip = args.gen_slip.unwrap_or(calc_slip);
        
        let fault_str = crate::core::cfs_io::generate_fault_line_string(
            new_fault_id, args.gen_strike, args.gen_dip, args.gen_rake,
            final_length, final_width, args.gen_depth, final_slip,
            fault_center_x, fault_center_y
        );
        
        // Read raw string to inject
        let raw_contents = match std::fs::read_to_string(target_inp) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Error reading raw INP file: {}", e);
                std::process::exit(1);
            }
        };
        
        let mut lines: Vec<&str> = raw_contents.lines().collect();
        let mut insert_idx = lines.len();
        
        for (i, line) in lines.iter().enumerate() {
            if line.contains("Grid Parameters") {
                insert_idx = i;
                break;
            }
        }
        
        // Backtrack to skip empty lines right before 'Grid Parameters'
        while insert_idx > 0 && lines[insert_idx - 1].trim().is_empty() {
            insert_idx -= 1;
        }
        
        let mut new_contents = String::new();
        for (i, line) in lines.iter().enumerate() {
            if i == insert_idx {
                new_contents.push_str(&fault_str);
                new_contents.push('\n');
            }
            new_contents.push_str(line);
            new_contents.push('\n');
        }
        
        let out_path = args.output.as_ref().unwrap_or(target_inp);
        if let Err(e) = std::fs::write(out_path, new_contents) {
            eprintln!("Error writing appended INP file: {}", e);
            std::process::exit(1);
        }
        println!("Successfully appended Fault {} to INP file: {:?}", new_fault_id, out_path);
        std::process::exit(0);
    }

    let input_path = match &args.input {
        Some(p) => p,
        None => {
            eprintln!("Error: --input file is required when running in CLI mode. Use --help for more info.");
            std::process::exit(1);
        }
    };

    println!("Parsing input file: {:?}", input_path);
    let mut coulomb_input = match open_input_file_cui(input_path) {
        Ok(inp) => inp,
        Err(e) => {
            eprintln!("Error parsing input file: {}", e);
            std::process::exit(1);
        }
    };

    if args.show_info {
        println!("==================================================");
        println!("           SEISBOX COULOMB INPUT INFO             ");
        println!("==================================================");
        println!("File Path  : {:?}", input_path);
        println!("Avg Strike : {:.2}°", coulomb_input.av_strike);
        println!("Avg Dip    : {:.2}°", coulomb_input.av_dip);
        println!("Avg Rake   : {:.2}°", coulomb_input.av_rake);
        println!("Friction   : {:.2}", coulomb_input.fric);
        println!("Poisson's  : {:.3}", coulomb_input.pois);
        println!("Young's Mod: {:.2} bar", coulomb_input.young);
        println!("Fault count: {}", coulomb_input.el.len());
        println!("Map Origin : Lon {:.4}°, Lat {:.4}°", coulomb_input.map_info.zero_lon, coulomb_input.map_info.zero_lat);
        println!("==================================================");
        std::process::exit(0);
    }

    if args.validate {
        println!("==================================================");
        println!("               SEISBOX VALIDATION                 ");
        println!("==================================================");
        let mut is_valid = true;
        
        if coulomb_input.el.is_empty() {
            println!("❌ ERROR: No faults found in the input file.");
            is_valid = false;
        } else {
            println!("✅ Faults: {} elements loaded successfully.", coulomb_input.el.len());
        }

        if coulomb_input.xvec.is_empty() || coulomb_input.yvec.is_empty() {
            println!("❌ ERROR: Grid coordinates (xvec/yvec) are missing or invalid.");
            is_valid = false;
        } else {
            println!("✅ Grid  : X-range [{:.2}, {:.2}], Y-range [{:.2}, {:.2}]", 
                coulomb_input.xvec.first().unwrap_or(&0.0), coulomb_input.xvec.last().unwrap_or(&0.0),
                coulomb_input.yvec.first().unwrap_or(&0.0), coulomb_input.yvec.last().unwrap_or(&0.0));
        }

        if coulomb_input.young <= 0.0 || coulomb_input.pois <= 0.0 {
            println!("❌ ERROR: Invalid physics parameters (Young's modulus or Poisson's ratio).");
            is_valid = false;
        } else {
            println!("✅ Physics: PR1={:.3}, E1={:.2}, FRIC={:.2}", coulomb_input.pois, coulomb_input.young, coulomb_input.fric);
        }

        println!("==================================================");
        if is_valid {
            println!("STATUS: OK! The input file is ready for execution.");
            std::process::exit(0);
        } else {
            println!("STATUS: FAILED! Please fix the errors above.");
            std::process::exit(1);
        }
    }

    // Physics overrides
    if let Some(val) = args.fric { coulomb_input.fric = val; }
    if let Some(val) = args.poisson { coulomb_input.pois = val; }
    if let Some(val) = args.young { coulomb_input.young = val; }

    let mut override_grid = false;
    let mut x_start = coulomb_input.xvec.first().copied().unwrap_or(0.0);
    let mut x_finish = coulomb_input.xvec.last().copied().unwrap_or(0.0);
    let mut y_start = coulomb_input.yvec.first().copied().unwrap_or(0.0);
    let mut y_finish = coulomb_input.yvec.last().copied().unwrap_or(0.0);
    let mut x_inc = if coulomb_input.xvec.len() > 1 { coulomb_input.xvec[1] - coulomb_input.xvec[0] } else { 1.0 };
    let mut y_inc = if coulomb_input.yvec.len() > 1 { coulomb_input.yvec[1] - coulomb_input.yvec[0] } else { 1.0 };

    let earth_r = 6371.0;
    let rad_conv = 180.0 / std::f64::consts::PI;
    let zero_lon = coulomb_input.map_info.zero_lon;
    let zero_lat = coulomb_input.map_info.zero_lat;
    let cos_lat = zero_lat.to_radians().cos();

    if let Some(v) = args.grid_start_x { x_start = v; override_grid = true; }
    if let Some(v) = args.grid_finish_x { x_finish = v; override_grid = true; }
    if let Some(v) = args.grid_start_y { y_start = v; override_grid = true; }
    if let Some(v) = args.grid_finish_y { y_finish = v; override_grid = true; }
    if let Some(v) = args.grid_x_inc { x_inc = v.max(0.001); override_grid = true; }
    if let Some(v) = args.grid_y_inc { y_inc = v.max(0.001); override_grid = true; }

    // Lat/Lon overrides (converted to km)
    if let Some(lon) = args.grid_start_lon { x_start = (lon - zero_lon) * (earth_r * cos_lat) / rad_conv; override_grid = true; }
    if let Some(lon) = args.grid_finish_lon { x_finish = (lon - zero_lon) * (earth_r * cos_lat) / rad_conv; override_grid = true; }
    if let Some(lat) = args.grid_start_lat { y_start = (lat - zero_lat) * earth_r / rad_conv; override_grid = true; }
    if let Some(lat) = args.grid_finish_lat { y_finish = (lat - zero_lat) * earth_r / rad_conv; override_grid = true; }
    if let Some(inc) = args.grid_lon_inc { x_inc = (inc * (earth_r * cos_lat) / rad_conv).max(0.001); override_grid = true; }
    if let Some(inc) = args.grid_lat_inc { y_inc = (inc * earth_r / rad_conv).max(0.001); override_grid = true; }

    if override_grid {
        println!("Overriding grid boundaries from CLI arguments...");
        let mut new_xvec = Vec::new();
        let mut x = x_start;
        while x <= x_finish + x_inc * 0.5 {
            new_xvec.push(x);
            x += x_inc;
        }
        
        let mut new_yvec = Vec::new();
        let mut y = y_start;
        while y <= y_finish + y_inc * 0.5 {
            new_yvec.push(y);
            y += y_inc;
        }
        
        
        coulomb_input.xvec = new_xvec;
        coulomb_input.yvec = new_yvec;
        
        coulomb_input.map_info.min_lon = zero_lon + (x_start / (earth_r * cos_lat) * rad_conv);
        coulomb_input.map_info.max_lon = zero_lon + (x_finish / (earth_r * cos_lat) * rad_conv);
        coulomb_input.map_info.min_lat = zero_lat + (y_start / earth_r * rad_conv);
        coulomb_input.map_info.max_lat = zero_lat + (y_finish / earth_r * rad_conv);
    }

    let results = if let Some(ref batch_path) = args.batch {
        println!("Running batch calculation using: {:?}", batch_path);
        let batch_input = match open_batch_file(batch_path, &coulomb_input.map_info) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Error parsing batch file: {}", e);
                std::process::exit(1);
            }
        };
        calculate_coulomb_batch(&coulomb_input, &batch_input)
    } else {
        let calc_strike = if args.use_source_mech { coulomb_input.av_strike } else { args.strike };
        let calc_dip = if args.use_source_mech { coulomb_input.av_dip } else { args.dip };
        let calc_rake = if args.use_source_mech { coulomb_input.av_rake } else { args.rake };
        
        
        let mut depths = Vec::new();
        if let Some(finish) = args.depth_finish {
            let inc = args.depth_inc.unwrap_or(5.0).max(0.1);
            let mut d = args.depth;
            while d <= finish + inc * 0.5 {
                depths.push(d);
                d += inc;
            }
        } else {
            depths.push(args.depth);
        }
        
        let calc_strike = if args.use_source_mech { coulomb_input.av_strike } else { args.strike };
        let calc_dip = if args.use_source_mech { coulomb_input.av_dip } else { args.dip };
        let calc_rake = if args.use_source_mech { coulomb_input.av_rake } else { args.rake };

        let default_out = PathBuf::from("coulomb_out.csv");

        // Determine regional stress tensor if OOF mode is active
        let regional_voigt = if args.oof {
            let reg_mag = args.regional_mag.unwrap_or(coulomb_input.rstress[0]);
            let reg_az = args.regional_azimuth.unwrap_or(coulomb_input.rstress[1]);
            let reg_pl = args.regional_plunge.unwrap_or(coulomb_input.rstress[2]);
            
            if reg_mag.abs() < 1e-10 {
                eprintln!("WARNING: Regional stress magnitude is zero or not set.");
                eprintln!("OOF mode requires non-zero regional stress (σ₁−σ₃).");
                eprintln!("Set it in the INP file or use --regional-mag, --regional-azimuth, --regional-plunge.");
                std::process::exit(1);
            }
            
            println!("Regional stress: magnitude={:.2} bar, azimuth={:.1}°, plunge={:.1}°", reg_mag, reg_az, reg_pl);
            let tensor = build_regional_stress_tensor(reg_mag, reg_az, reg_pl);
            Some(regional_tensor_to_voigt(&tensor))
        } else {
            None
        };

        if args.cross_section {
            println!("Running CROSS SECTION calculation...");
            
            let start_lon = args.cs_start_lon.clone().and_then(|v| v.into_iter().next()).unwrap_or(coulomb_input.cross_section.start_x);
            let finish_lon = args.cs_finish_lon.clone().and_then(|v| v.into_iter().next()).unwrap_or(coulomb_input.cross_section.finish_x);
            let start_lat = args.cs_start_lat.clone().and_then(|v| v.into_iter().next()).unwrap_or(coulomb_input.cross_section.start_y);
            let finish_lat = args.cs_finish_lat.clone().and_then(|v| v.into_iter().next()).unwrap_or(coulomb_input.cross_section.finish_y);

            let cs_start_x_km = (start_lon - zero_lon) * (earth_r * cos_lat) / rad_conv;
            let cs_start_y_km = (start_lat - zero_lat) * earth_r / rad_conv;
            let cs_finish_x_km = (finish_lon - zero_lon) * (earth_r * cos_lat) / rad_conv;
            let cs_finish_y_km = (finish_lat - zero_lat) * earth_r / rad_conv;

            let cs_results = calculate_coulomb_cross_section(
                &coulomb_input,
                cs_start_x_km, cs_start_y_km,
                cs_finish_x_km, cs_finish_y_km,
                args.cs_dist_inc,
                &depths,
                calc_strike, calc_dip, calc_rake,
                regional_voigt.as_ref(),
            );

            let out_path = args.output.as_ref().unwrap_or(&default_out);
            println!("Saving cross section results to: {:?}", out_path);
            write_cross_section_csv(out_path, &cs_results);
            println!("Done. Calculated {} points.", cs_results.len());
            return;
        }

        if args.oof {
            let reg_v = regional_voigt.as_ref().unwrap();
            println!("Running Optimally Oriented Fault (OOF) calculation...");
            calculate_coulomb_grid_oof(&coulomb_input, &depths, reg_v)
        } else {
            println!("Running standard grid calculation (Strike: {}, Dip: {}, Rake: {}, Depths: {} layers)", calc_strike, calc_dip, calc_rake, depths.len());
            calculate_coulomb_grid(&coulomb_input, &depths, calc_strike, calc_dip, calc_rake)
        }
    };

    let default_out = PathBuf::from("coulomb_out.csv");
    let out_path = args.output.as_ref().unwrap_or(&default_out);
    let out_str = out_path.to_string_lossy().to_string();
    let base_out = if out_str.ends_with(".csv") {
        out_str.trim_end_matches(".csv").to_string()
    } else {
        out_str
    };

    if args.batch.is_some() {
        println!("Saving batch results to: {:?}", out_path);
        write_coulomb_csv(out_path, &results);
    } else {
        // Collect depths actually calculated
        let mut target_depths = Vec::new();
        if let Some(finish) = args.depth_finish {
            let inc = args.depth_inc.unwrap_or(5.0).max(0.1);
            let mut d = args.depth;
            while d <= finish + inc * 0.5 {
                target_depths.push(d);
                d += inc;
            }
        } else {
            target_depths.push(args.depth);
        }

        if args.max_depth && target_depths.len() > 1 {
            target_depths.push(999.0); // max flag
        }

        for d in target_depths {
            let suffix = if d == 999.0 { "_max" } else { &format!("_{}", d) };
            let current_csv = std::path::PathBuf::from(format!("{}{}.csv", base_out, suffix));
            
            // Filter results for this depth (z is negative depth, except 999.0 for max)
            let search_z = if d == 999.0 { 999.0 } else { -d };
            let depth_results: Vec<_> = results.iter().filter(|r| (r.z - search_z).abs() < 1e-4).cloned().collect();
            
            if !depth_results.is_empty() {
                println!("Saving depth{} results to: {:?}", suffix, current_csv);
                write_coulomb_csv(&current_csv, &depth_results);

                if args.tiff {
                    let current_tif = std::path::PathBuf::from(format!("{}{}.tif", base_out, suffix));
                    println!("Writing TIFF depth{} results to: {:?}", suffix, current_tif);
                    if let Err(e) = write_coulomb_tiff(&current_tif, &depth_results, &coulomb_input) {
                        println!("Warning: Failed to write TIFF file: {}", e);
                    }
                }
            }
        }
    }

    println!("Done. Calculated {} points.", results.len());
}
