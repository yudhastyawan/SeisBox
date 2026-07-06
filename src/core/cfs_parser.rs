use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::f64::consts::PI;

#[derive(Debug, Clone)]
pub struct MapInfo {
    pub min_lon: f64,
    pub max_lon: f64,
    pub zero_lon: f64,
    pub min_lat: f64,
    pub max_lat: f64,
    pub zero_lat: f64,
}

impl Default for MapInfo {
    fn default() -> Self {
        Self {
            min_lon: 0.0,
            max_lon: 0.0,
            zero_lon: 0.0,
            min_lat: 0.0,
            max_lat: 0.0,
            zero_lat: 0.0,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CrossSection {
    pub start_x: f64,
    pub start_y: f64,
    pub finish_x: f64,
    pub finish_y: f64,
}

#[derive(Debug, Clone)]
pub struct CoulombInput {
    pub xvec: Vec<f64>,
    pub yvec: Vec<f64>,
    pub z: f64,
    pub el: Vec<[f64; 9]>, // [xs, ys, xf, yf, latslip, dipslip, dip, top, bottom]
    pub kode: Vec<i32>,
    pub pois: f64,
    pub young: f64,
    pub cdepth: f64,
    pub fric: f64,
    pub rstress: [f64; 3],
    pub map_info: MapInfo,
    pub cross_section: CrossSection,
    pub av_strike: f64,
    pub av_dip: f64,
    pub av_rake: f64,
}

pub fn open_input_file_cui<P: AsRef<Path>>(filename: P) -> Result<CoulombInput, String> {
    let file = File::open(filename).map_err(|e| e.to_string())?;
    let reader = BufReader::new(file);

    let mut pois = 0.25;
    let mut cdepth = 7.5;
    let mut young = 8e5;
    let mut fric = 0.4;
    let mut rstress = [0.0, 0.0, 0.0];

    // grid: [xstart, ystart, xend, yend, xinc, yinc]
    let mut grid = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
    let mut map_info = MapInfo::default();
    let mut cross_section = CrossSection::default();

    let mut in_fault_elements = false;
    let mut el = Vec::new();
    let mut kode = Vec::new();
    
    let lines: Vec<String> = reader.lines().filter_map(|r| r.ok()).collect();
    let all_text = lines.join(" ").to_uppercase();

    #[derive(PartialEq)]
    enum Section { Header, Faults, Grid, SizeParams, CrossSection, MapInfo }
    let mut section = Section::Header;

    for line in &lines {
        let up_line = line.to_uppercase();
        let trimmed = up_line.trim();

        // --- Detect section transitions ---
        if trimmed.contains("GRID PARAMETERS") {
            section = Section::Grid;
            continue;
        }
        if trimmed.contains("SIZE PARAMETERS") {
            section = Section::SizeParams;
            continue;
        }
        if trimmed.contains("CROSS SECTION") {
            section = Section::CrossSection;
            continue;
        }
        if trimmed.contains("MAP INFO") {
            section = Section::MapInfo;
            continue;
        }

        // --- Header section: PR1, E1, FRIC, depth ---
        if up_line.contains("PR1=") {
            // Parse PR1= value
            if let Some(pr1_pos) = up_line.find("PR1=") {
                let after = &up_line[pr1_pos + 4..];
                if let Some(v) = after.split_whitespace().next().and_then(|s| s.parse::<f64>().ok()) {
                    pois = v;
                }
            }
            // Parse DEPTH= value
            if let Some(depth_pos) = up_line.find("DEPTH=") {
                let after = &up_line[depth_pos + 6..];
                if let Some(v) = after.split_whitespace().next().and_then(|s| s.parse::<f64>().ok()) {
                    cdepth = v;
                }
            }
        }

        if up_line.contains("E1=") && up_line.contains("E2=") {
            if let Some(e1_pos) = up_line.find("E1=") {
                let after = &up_line[e1_pos + 3..];
                if let Some(v) = after.split_whitespace().next().and_then(|s| s.parse::<f64>().ok()) {
                    young = v;
                }
            }
        }

        if up_line.contains("FRIC=") {
            if let Some(fric_pos) = up_line.find("FRIC=") {
                let after = &up_line[fric_pos + 5..];
                if let Some(v) = after.split_whitespace().next().and_then(|s| s.parse::<f64>().ok()) {
                    fric = v;
                }
            }
        }

        if up_line.contains("SIGMA1-SIGMA3") {
            let floats: Vec<f64> = up_line.split_whitespace().filter_map(|s| s.parse().ok()).collect();
            if floats.len() >= 3 {
                rstress[0] = floats[0];
                rstress[1] = floats[1];
                rstress[2] = floats[2];
            }
        }

        // --- Fault element toggle ---
        if up_line.contains("XXX") {
            in_fault_elements = !in_fault_elements;
            continue;
        }

        if in_fault_elements {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 11 {
                if let (Ok(xs), Ok(ys), Ok(xf), Ok(yf), Ok(k)) = (
                    parts[1].parse::<f64>(), parts[2].parse::<f64>(), 
                    parts[3].parse::<f64>(), parts[4].parse::<f64>(), 
                    parts[5].parse::<i32>()
                ) {
                    if all_text.contains("RAKE") {
                        if let (Ok(rake), Ok(netslip), Ok(dip), Ok(top), Ok(bottom)) = (
                            parts[6].parse::<f64>(), parts[7].parse::<f64>(), parts[8].parse::<f64>(), parts[9].parse::<f64>(), parts[10].parse::<f64>()
                        ) {
                            let rake_rad = rake.to_radians();
                            let latslip = rake_rad.cos() * netslip * -1.0;
                            let dipslip = rake_rad.sin() * netslip;
                            el.push([xs, ys, xf, yf, latslip, dipslip, dip, top, bottom]);
                            kode.push(k);
                        }
                    } else {
                        if let (Ok(latslip), Ok(dipslip), Ok(dip), Ok(top), Ok(bottom)) = (
                            parts[6].parse::<f64>(), parts[7].parse::<f64>(), parts[8].parse::<f64>(), parts[9].parse::<f64>(), parts[10].parse::<f64>()
                        ) {
                            el.push([xs, ys, xf, yf, latslip, dipslip, dip, top, bottom]);
                            kode.push(k);
                        }
                    }
                }
            }
        }

        // --- Parse key=value lines in specific sections ---
        if up_line.contains('=') {
            let parts: Vec<&str> = up_line.split('=').collect();
            if parts.len() == 2 {
                let val_str = parts[1].split_whitespace().next().unwrap_or("");
                if let Ok(val) = val_str.parse::<f64>() {
                    match section {
                        Section::Grid => {
                            if up_line.contains("START-X") { grid[0] = val; }
                            else if up_line.contains("START-Y") { grid[1] = val; }
                            else if up_line.contains("FINISH-X") { grid[2] = val; }
                            else if up_line.contains("FINISH-Y") { grid[3] = val; }
                            else if up_line.contains("X-INC") { grid[4] = val; }
                            else if up_line.contains("Y-INC") { grid[5] = val; }
                        }
                        Section::CrossSection => {
                            if up_line.contains("START-X") { cross_section.start_x = val; }
                            else if up_line.contains("START-Y") { cross_section.start_y = val; }
                            else if up_line.contains("FINISH-X") { cross_section.finish_x = val; }
                            else if up_line.contains("FINISH-Y") { cross_section.finish_y = val; }
                        }
                        Section::MapInfo => {
                            if up_line.contains("MIN. LON") { map_info.min_lon = val; }
                            else if up_line.contains("MAX. LON") { map_info.max_lon = val; }
                            else if up_line.contains("ZERO LON") { map_info.zero_lon = val; }
                            else if up_line.contains("MIN. LAT") { map_info.min_lat = val; }
                            else if up_line.contains("MAX. LAT") { map_info.max_lat = val; }
                            else if up_line.contains("ZERO LAT") { map_info.zero_lat = val; }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    if el.is_empty() {
        return Err("No valid faults found in the input file.".into());
    }

    let xin = grid[4].max(0.001);
    let yin = grid[5].max(0.001);

    let mut xvec = Vec::new();
    let mut x = grid[0];
    while x <= grid[2] + xin * 0.5 {
        xvec.push(x);
        x += xin;
    }

    let mut yvec = Vec::new();
    let mut y = grid[1];
    while y <= grid[3] + yin * 0.5 {
        yvec.push(y);
        y += yin;
    }

    let mut sum_strike = 0.0;
    let mut sum_dip = 0.0;
    let mut sum_latslip = 0.0;
    let mut sum_dipslip = 0.0;

    for element in &el {
        let xs = element[0];
        let ys = element[1];
        let xf = element[2];
        let yf = element[3];
        
        let mut a = ((yf - ys) / (xf - xs)).atan().to_degrees();
        if a.is_nan() { a = 0.0; } // Handle xs == xf gracefully though not in original
        
        let strike = if xs > xf {
            270.0 - a
        } else {
            90.0 - a
        };
        
        sum_strike += strike;
        sum_dip += element[6];
        sum_latslip += element[4];
        sum_dipslip += element[5];
    }
    
    let num_f64 = el.len() as f64;
    let av_strike = sum_strike / num_f64;
    let av_dip = sum_dip / num_f64;
    
    let mut av_lat_slip = sum_latslip / num_f64;
    let av_dip_slip = sum_dipslip / num_f64;
    
    if av_lat_slip == 0.0 {
        av_lat_slip = 0.000001;
    }
    
    let b = (av_dip_slip / av_lat_slip).atan().to_degrees();
    let av_rake = if av_lat_slip >= 0.0 {
        if av_dip_slip >= 0.0 {
            180.0 - b
        } else {
            -180.0 - b
        }
    } else {
        if av_dip_slip >= 0.0 {
            -b
        } else {
            -b
        }
    };

    Ok(CoulombInput {
        xvec,
        yvec,
        z: cdepth,
        el,
        kode,
        pois,
        young,
        cdepth,
        fric,
        rstress,
        map_info,
        cross_section,
        av_strike,
        av_dip,
        av_rake,
    })
}

#[derive(Debug, Clone)]
pub struct BatchInput {
    pub pos: Vec<[f64; 3]>,
    pub strike: Vec<f64>,
    pub dip: Vec<f64>,
    pub rake: Vec<f64>,
}

pub fn open_batch_file<P: AsRef<Path>>(filename: P, map_info: &MapInfo) -> Result<BatchInput, String> {
    let path = filename.as_ref();
    let is_csv = path.extension().and_then(|e| e.to_str()) == Some("csv");

    let mut pos = Vec::new();
    let mut strike = Vec::new();
    let mut dip = Vec::new();
    let mut rake = Vec::new();

    let earth_r = 6371.0;

    if is_csv {
        let mut rdr = csv::ReaderBuilder::new().has_headers(true).from_path(path).map_err(|e| e.to_string())?;
        let headers = rdr.headers().map_err(|e| e.to_string())?.clone();
        
        let mut idx_lon = None;
        let mut idx_lat = None;
        let mut idx_x = None;
        let mut idx_y = None;
        let mut idx_z = None;
        let mut idx_strike = None;
        let mut idx_dip = None;
        let mut idx_rake = None;
        
        for (i, h) in headers.iter().enumerate() {
            let hl = h.to_lowercase();
            if hl.contains("lon") { idx_lon = Some(i); }
            else if hl.contains("lat") { idx_lat = Some(i); }
            else if hl == "x" || hl == "x_km" || hl.starts_with("x") { idx_x = Some(i); }
            else if hl == "y" || hl == "y_km" || hl.starts_with("y") { idx_y = Some(i); }
            else if hl == "z" || hl == "z_km" || hl.starts_with("z") { idx_z = Some(i); }
            else if hl.contains("strike") { idx_strike = Some(i); }
            else if hl.contains("dip") { idx_dip = Some(i); }
            else if hl.contains("rake") { idx_rake = Some(i); }
        }
        
        for result in rdr.records() {
            let record = result.map_err(|e| e.to_string())?;
            
            let mut x_val = 0.0;
            let mut y_val = 0.0;
            
            if let (Some(ilon), Some(ilat)) = (idx_lon, idx_lat) {
                if let (Ok(lon), Ok(lat)) = (record.get(ilon).unwrap_or("").parse::<f64>(), record.get(ilat).unwrap_or("").parse::<f64>()) {
                    y_val = (lat - map_info.zero_lat) * earth_r / (180.0 / PI);
                    x_val = (lon - map_info.zero_lon) * (earth_r * map_info.zero_lat.to_radians().cos()) / (180.0 / PI);
                }
            } else if let (Some(ix), Some(iy)) = (idx_x, idx_y) {
                if let (Ok(x), Ok(y)) = (record.get(ix).unwrap_or("").parse::<f64>(), record.get(iy).unwrap_or("").parse::<f64>()) {
                    x_val = x;
                    y_val = y;
                }
            }
            
            let z_val = if let Some(iz) = idx_z { record.get(iz).unwrap_or("").parse::<f64>().unwrap_or(0.0) } else { 0.0 };
            let s_val = if let Some(is) = idx_strike { record.get(is).unwrap_or("").parse::<f64>().unwrap_or(0.0) } else { 0.0 };
            let d_val = if let Some(id) = idx_dip { record.get(id).unwrap_or("").parse::<f64>().unwrap_or(0.0) } else { 0.0 };
            let r_val = if let Some(ir) = idx_rake { record.get(ir).unwrap_or("").parse::<f64>().unwrap_or(0.0) } else { 0.0 };
            
            pos.push([x_val, y_val, z_val]);
            strike.push(s_val);
            dip.push(d_val);
            rake.push(r_val);
        }
    } else {
        let file = File::open(path).map_err(|e| e.to_string())?;
        let reader = BufReader::new(file);

        for (i, line) in reader.lines().enumerate() {
            if i < 2 { continue; } // skip 2 lines for .dat / .txt
            if let Ok(l) = line {
                let parts: Vec<&str> = l.split_whitespace().collect();
                if parts.len() >= 6 {
                    if let (Ok(x), Ok(y), Ok(z), Ok(s), Ok(d), Ok(r)) = (
                        parts[0].parse::<f64>(), parts[1].parse::<f64>(), parts[2].parse::<f64>(),
                        parts[3].parse::<f64>(), parts[4].parse::<f64>(), parts[5].parse::<f64>()
                    ) {
                        pos.push([x, y, z]);
                        strike.push(s);
                        dip.push(d);
                        rake.push(r);
                    }
                }
            }
        }
    }

    Ok(BatchInput { pos, strike, dip, rake })
}
