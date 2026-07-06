use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use crate::core::picking::{PhaseType, PickSet};

// ---------------------------------------------------------------------------
// File tree structures for the sidebar
// ---------------------------------------------------------------------------

/// A node in the recursive file tree.
#[derive(Debug, Clone)]
pub struct FileNode {
    /// The full absolute path.
    pub path: PathBuf,
    /// Display name (filename or directory name).
    pub name: String,
    /// Whether this node is a directory.
    pub is_dir: bool,
    /// Children (populated only for directories).
    pub children: Vec<FileNode>,
}

/// Extensions we care about for the sidebar.
const SEISMIC_EXTENSIONS: &[&str] = &["sac", "mseed"];
const PICK_EXTENSIONS: &[&str] = &["picks", "txt"];

impl FileNode {
    /// Recursively scan a directory and build a tree, filtering to only
    /// seismic files (.sac, .mseed) and pick files (.picks, .txt).
    /// Directories that contain no matching files (recursively) are pruned.
    pub fn scan_dir(dir: &Path) -> io::Result<Self> {
        let mut children = Vec::new();

        let mut entries: Vec<_> = fs::read_dir(dir)?
            .filter_map(|e| e.ok())
            .collect();
        entries.sort_by_key(|e| e.file_name());

        for entry in entries {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            // Skip hidden files/directories
            if name.starts_with('.') {
                continue;
            }

            if path.is_dir() {
                // Recurse into subdirectory
                if let Ok(child_node) = Self::scan_dir(&path) {
                    // Only include if it has relevant children
                    if !child_node.children.is_empty() {
                        children.push(child_node);
                    }
                }
            } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                let ext_lower = ext.to_lowercase();
                if SEISMIC_EXTENSIONS.contains(&ext_lower.as_str())
                    || PICK_EXTENSIONS.contains(&ext_lower.as_str())
                {
                    children.push(FileNode {
                        path,
                        name,
                        is_dir: false,
                        children: Vec::new(),
                    });
                }
            }
        }

        Ok(FileNode {
            name: dir
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| dir.to_string_lossy().to_string()),
            path: dir.to_path_buf(),
            is_dir: true,
            children,
        })
    }
}

/// Check if a path has a seismic file extension.
pub fn is_seismic_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| SEISMIC_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// Check if a path has a pick file extension.
pub fn is_pick_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| PICK_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Pick file auto-load / auto-save
// ---------------------------------------------------------------------------

/// Derive the pick file path from a seismic file path.
/// Replaces the extension with `.picks`.
pub fn pick_file_path(seismic_path: &Path) -> PathBuf {
    seismic_path.with_extension("picks")
}

/// Try to find an existing pick file for a seismic file.
/// Checks `.picks` first, then falls back to `.txt`.
pub fn find_pick_file(seismic_path: &Path) -> Option<PathBuf> {
    let picks_path = seismic_path.with_extension("picks");
    if picks_path.exists() {
        return Some(picks_path);
    }
    let txt_path = seismic_path.with_extension("txt");
    if txt_path.exists() {
        return Some(txt_path);
    }
    None
}

/// Load picks from an ASCII pick file.
///
/// Format: one pick per line, space-delimited:
/// ```text
/// P_START 0.075
/// S_START 0.112
/// P_END   0.150
/// S_END   0.200
/// ```
pub fn load_picks(path: &Path) -> io::Result<PickSet> {
    let file = fs::File::open(path)?;
    let reader = io::BufReader::new(file);
    let mut pick_set = PickSet::new();

    for line in reader.lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            if let Some(phase) = PhaseType::from_tag(parts[0]) {
                if let Ok(time) = parts[1].parse::<f64>() {
                    // Start with defaults
                    let mut pick = crate::core::picking::Pick {
                        phase,
                        time,
                        onset: None,
                        polarity: None,
                        uncertainty: None,
                        amplitude: None,
                        amplitude_demeaned: None,
                    };
                    
                    if parts.len() >= 3 && parts[2] != "-" {
                        pick.onset = crate::core::picking::Onset::from_str(parts[2]);
                    }
                    if parts.len() >= 4 && parts[3] != "-" {
                        pick.polarity = crate::core::picking::Polarity::from_str(parts[3]);
                    }
                    if parts.len() >= 5 && parts[4] != "-" {
                        if let Ok(unc) = parts[4].parse::<f64>() {
                            pick.uncertainty = Some(unc);
                        }
                    }
                    if parts.len() >= 6 && parts[5] != "-" {
                        if let Ok(amp) = parts[5].parse::<f64>() {
                            pick.amplitude = Some(amp);
                        }
                    }
                    if parts.len() >= 7 && parts[6] != "-" {
                        if let Ok(amp_dm) = parts[6].parse::<f64>() {
                            pick.amplitude_demeaned = Some(amp_dm);
                        }
                    }

                    // Remove existing phase if any, and add this one
                    pick_set.delete(phase);
                    pick_set.picks.push(pick);
                }
            }
        }
    }

    Ok(pick_set)
}

/// Save picks to an ASCII pick file (overwrites completely).
pub fn save_picks(path: &Path, pick_set: &PickSet) -> io::Result<()> {
    let mut file = fs::File::create(path)?;

    // Write header comment
    writeln!(file, "# QuakePick phase picks")?;
    writeln!(file, "# Format: PHASE_TAG TIME_SECONDS ONSET POLARITY UNCERTAINTY AMPLITUDE AMPLITUDE_DEMEANED")?;

    // Write picks in a canonical order
    for phase in PhaseType::all() {
        if let Some(pick) = pick_set.picks.iter().find(|p| p.phase == *phase) {
            let onset_str = pick.onset.map(|o| o.as_str()).unwrap_or("-");
            let pol_str = pick.polarity.map(|p| p.as_str()).unwrap_or("-");
            let unc_str = match pick.uncertainty {
                Some(u) => format!("{:.4}", u),
                None => "-".to_string(),
            };
            let amp_str = match pick.amplitude {
                Some(a) => format!("{:.4}", a),
                None => "-".to_string(),
            };
            let amp_dm_str = match pick.amplitude_demeaned {
                Some(a) => format!("{:.4}", a),
                None => "-".to_string(),
            };
            writeln!(file, "{} {:.6} {} {} {} {} {}", phase.tag(), pick.time, onset_str, pol_str, unc_str, amp_str, amp_dm_str)?;
        }
    }

    Ok(())
}

/// Auto-load picks for a seismic file. Returns the pick set and the path
/// of the file that was loaded (if any).
pub fn auto_load_picks(seismic_path: &Path) -> (PickSet, Option<PathBuf>) {
    if let Some(pick_path) = find_pick_file(seismic_path) {
        match load_picks(&pick_path) {
            Ok(pick_set) => (pick_set, Some(pick_path)),
            Err(e) => {
                eprintln!("Warning: failed to load picks from {:?}: {}", pick_path, e);
                (PickSet::new(), None)
            }
        }
    } else {
        (PickSet::new(), None)
    }
}

/// Auto-save picks for a seismic file. Always saves to `.picks` extension.
pub fn auto_save_picks(seismic_path: &Path, pick_set: &PickSet) {
    let path = pick_file_path(seismic_path);
    if let Err(e) = save_picks(&path, pick_set) {
        eprintln!("Error: failed to save picks to {:?}: {}", path, e);
    }
}

// ---------------------------------------------------------------------------
// Exporting
// ---------------------------------------------------------------------------

/// Export picks for all given traces to a single TSV (Tab-Separated Values) file.
pub fn export_picks_to_ascii(path: &Path, traces: &[crate::ui::plot::TraceState]) -> io::Result<()> {
    let mut file = fs::File::create(path)?;

    // Write header
    writeln!(
        file,
        "Station\tPhase\tTime(s)\tOnset\tPolarity\tUncertainty\tAmplitude\tAmpDemeaned"
    )?;

    for trace in traces {
        let station = &trace.seismogram.filename;
        for pick in &trace.pick_set.picks {
            let onset_str = pick.onset.map(|o| o.as_str()).unwrap_or("-");
            let pol_str = pick.polarity.map(|p| p.as_str()).unwrap_or("-");
            let unc_str = pick.uncertainty.map(|u| format!("{:.4}", u)).unwrap_or_else(|| "-".to_string());
            let amp_str = pick.amplitude.map(|a| format!("{:.4}", a)).unwrap_or_else(|| "-".to_string());
            let amp_dm_str = pick.amplitude_demeaned.map(|a| format!("{:.4}", a)).unwrap_or_else(|| "-".to_string());

            writeln!(
                file,
                "{}\t{}\t{:.6}\t{}\t{}\t{}\t{}\t{}",
                station,
                pick.phase.tag(),
                pick.time,
                onset_str,
                pol_str,
                unc_str,
                amp_str,
                amp_dm_str
            )?;
        }
    }

    Ok(())
}

/// Save an egui ColorImage to disk as a PNG file, optionally cropping it.
pub fn save_image_to_disk(
    image: std::sync::Arc<eframe::egui::ColorImage>,
    path: PathBuf,
    crop_rect: Option<eframe::egui::Rect>,
    pixels_per_point: f32,
) {
    if image.size[0] == 0 || image.size[1] == 0 {
        return;
    }
    
    let pixels: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
    
    if let Some(mut img_buffer) = image::RgbaImage::from_raw(image.size[0] as u32, image.size[1] as u32, pixels) {
        if let Some(rect) = crop_rect {
            let min_x = (rect.min.x * pixels_per_point).max(0.0) as u32;
            let min_y = (rect.min.y * pixels_per_point).max(0.0) as u32;
            let width = (rect.width() * pixels_per_point).max(0.0) as u32;
            let height = (rect.height() * pixels_per_point).max(0.0) as u32;
            
            // Ensure we don't crop outside the image bounds
            let width = width.min(img_buffer.width().saturating_sub(min_x));
            let height = height.min(img_buffer.height().saturating_sub(min_y));
            
            if width > 0 && height > 0 {
                let cropped = image::imageops::crop(&mut img_buffer, min_x, min_y, width, height).to_image();
                if let Err(e) = cropped.save(&path) {
                    eprintln!("Failed to save screenshot to {:?}: {}", path, e);
                    return;
                }
                println!("Screenshot successfully saved to {:?}", path);
                return;
            }
        }
        
        // Fallback to full image if no crop or invalid crop bounds
        if let Err(e) = img_buffer.save(&path) {
            eprintln!("Failed to save screenshot to {:?}: {}", path, e);
        } else {
            println!("Screenshot successfully saved to {:?}", path);
        }
    } else {
        eprintln!("Failed to create image buffer for screenshot.");
    }
}

/// Export a single trace to an ASCII file with a header.
pub fn export_trace_ascii(seis: &crate::core::seismogram::Seismogram, path: &Path) -> io::Result<()> {
    let mut file = fs::File::create(path)?;
    
    let npts = seis.time.len();
    let delta = if seis.sample_rate > 0.0 { 1.0 / seis.sample_rate } else { 0.0 };
    let duration = seis.time.last().copied().unwrap_or(0.0);
    
    // Write header
    writeln!(file, "# Network: {}", seis.network)?;
    writeln!(file, "# Station: {}", seis.station)?;
    writeln!(file, "# Location: {}", seis.location)?;
    writeln!(file, "# Channel: {}", seis.channel)?;
    writeln!(file, "# Start Time (UTC): {}", seis.start_time_str)?;
    writeln!(file, "# End Time (UTC): {}", seis.end_time_str)?;
    writeln!(file, "# Sampling Rate: {:.2} Hz", seis.sample_rate)?;
    writeln!(file, "# Delta: {:.6} s", delta)?;
    writeln!(file, "# Npts: {}", npts)?;
    writeln!(file, "# Mean: {:.6}", seis.mean)?;
    writeln!(file, "# Time(s) Amplitude")?;
    
    // Write data
    for (t, a) in seis.time.iter().zip(seis.amplitude.iter()) {
        writeln!(file, "{:.6} {:.6}", t, a)?;
    }
    
    Ok(())
}
