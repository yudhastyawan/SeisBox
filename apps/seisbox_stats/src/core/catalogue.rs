use std::path::Path;
use std::error::Error;
use std::fs::File;
use std::io::{BufRead, BufReader};
use chrono::NaiveDateTime;
use kiddo::KdTree;

#[derive(Debug, Clone)]
pub struct Earthquake {
    pub time: NaiveDateTime,
    pub lon: f64,
    pub lat: f64,
    pub depth: f64,
    pub mag: f64,
}

#[derive(Debug, Clone)]
pub struct Catalogue {
    pub events: Vec<Earthquake>,
}

impl Catalogue {
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }
    
    /// Load earthquake catalogue from CSV file.
    /// Auto-detects columns: time/datetime, lon/longitude, lat/latitude, depth, mag/magnitude
    pub fn load_csv(path: &Path) -> Result<Self, String> {
        let mut rdr = csv::ReaderBuilder::new()
            .flexible(true)
            .from_path(path)
            .map_err(|e| format!("Cannot open CSV: {}", e))?;
        
        let headers: Vec<String> = rdr.headers()
            .map_err(|e| format!("Cannot read headers: {}", e))?
            .iter()
            .map(|h| h.trim().to_lowercase())
            .collect();
        
        // Find column indices
        let col_time = headers.iter().position(|h| h == "time" || h == "datetime" || h == "origin_time");
        let col_lon = headers.iter().position(|h| h == "lon" || h == "longitude" || h == "long");
        let col_lat = headers.iter().position(|h| h == "lat" || h == "latitude");
        let col_depth = headers.iter().position(|h| h == "depth" || h == "dep");
        let col_mag = headers.iter().position(|h| h == "mag" || h == "magnitude" || h == "ml" || h == "mw" || h == "mb");
        
        let col_time = col_time.ok_or("Missing 'time' column")?;
        let col_lon = col_lon.ok_or("Missing 'lon' column")?;
        let col_lat = col_lat.ok_or("Missing 'lat' column")?;
        let col_depth = col_depth.ok_or("Missing 'depth' column")?;
        let col_mag = col_mag.ok_or("Missing 'mag' column")?;
        
        let mut events = Vec::new();
        
        for result in rdr.records() {
            let record = result.map_err(|e| format!("CSV parse error: {}", e))?;
            
            let time_str = record.get(col_time).unwrap_or("").trim();
            let time = parse_datetime(time_str);
            
            let lon: f64 = record.get(col_lon).unwrap_or("0").trim().parse().unwrap_or(0.0);
            let lat: f64 = record.get(col_lat).unwrap_or("0").trim().parse().unwrap_or(0.0);
            let depth: f64 = record.get(col_depth).unwrap_or("0").trim().parse().unwrap_or(0.0);
            let mag: f64 = record.get(col_mag).unwrap_or("0").trim().parse().unwrap_or(0.0);
            
            if let Some(t) = time {
                events.push(Earthquake { time: t, lon, lat, depth, mag });
            }
        }
        
        // Sort by time
        events.sort_by(|a, b| a.time.cmp(&b.time));
        
        Ok(Catalogue { events })
    }
    
    /// Save catalogue to CSV
    pub fn save_csv(&self, path: &Path) -> Result<(), String> {
        let mut wtr = csv::Writer::from_path(path).map_err(|e| e.to_string())?;
        wtr.write_record(&["time", "lon", "lat", "depth", "mag"]).map_err(|e| e.to_string())?;
        for eq in &self.events {
            wtr.write_record(&[
                eq.time.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
                format!("{:.6}", eq.lon),
                format!("{:.6}", eq.lat),
                format!("{:.2}", eq.depth),
                format!("{:.2}", eq.mag),
            ]).map_err(|e| e.to_string())?;
        }
        wtr.flush().map_err(|e| e.to_string())?;
        Ok(())
    }
    
    /// Create a subset from a boolean mask
    pub fn subset(&self, mask: &[bool]) -> Catalogue {
        let events: Vec<Earthquake> = self.events.iter()
            .zip(mask.iter())
            .filter(|(_, &m)| m)
            .map(|(e, _)| e.clone())
            .collect();
        Catalogue { events }
    }
    
    // ======= Filters (in-place) =======
    
    pub fn filter_time_start(&mut self, start: NaiveDateTime) {
        self.events.retain(|e| e.time >= start);
    }
    
    pub fn filter_time_end(&mut self, end: NaiveDateTime) {
        self.events.retain(|e| e.time <= end);
    }
    
    pub fn filter_min_mag(&mut self, min: f64) {
        self.events.retain(|e| e.mag >= min);
    }
    
    pub fn filter_max_mag(&mut self, max: f64) {
        self.events.retain(|e| e.mag <= max);
    }
    
    pub fn filter_min_depth(&mut self, min: f64) {
        self.events.retain(|e| e.depth >= min);
    }
    
    pub fn filter_max_depth(&mut self, max: f64) {
        self.events.retain(|e| e.depth <= max);
    }
    
    pub fn filter_bbox(&mut self, lon_min: f64, lon_max: f64, lat_min: f64, lat_max: f64) {
        self.events.retain(|e| {
            e.lon >= lon_min && e.lon <= lon_max && e.lat >= lat_min && e.lat <= lat_max
        });
    }
    
    pub fn filter_radial(&mut self, center_lon: f64, center_lat: f64, radius_km: f64) {
        self.events.retain(|e| {
            haversine_km(center_lat, center_lon, e.lat, e.lon) <= radius_km
        });
    }
    
    pub fn filter_polygon(&mut self, polygon: &[(f64, f64)]) {
        if polygon.len() < 3 { return; }
        self.events.retain(|e| {
            let mut inside = false;
            let mut j = polygon.len() - 1;
            for i in 0..polygon.len() {
                let (xi, yi) = polygon[i];
                let (xj, yj) = polygon[j];
                
                let intersect = ((yi > e.lat) != (yj > e.lat))
                    && (e.lon < (xj - xi) * (e.lat - yi) / (yj - yi) + xi);
                if intersect {
                    inside = !inside;
                }
                j = i;
            }
            inside
        });
    }

    pub fn filter_slab(&mut self, slab_path: &std::path::Path, buffer_km: f64) -> Result<(), Box<dyn Error>> {
        println!("Loading slab geometry from {:?}...", slab_path);
        
        let file = File::open(slab_path)?;
        let reader = BufReader::new(file);
        
        let mut kdtree: KdTree<f64, 2> = KdTree::new();
        let mut depths: Vec<f64> = Vec::new();
        
        let mut seen = std::collections::HashSet::new();
        
        for line_res in reader.lines() {
            let line = line_res?;
            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() >= 3 {
                if let (Ok(lon), Ok(lat), Ok(depth)) = (parts[0].trim().parse::<f64>(), parts[1].trim().parse::<f64>(), parts[2].trim().parse::<f64>()) {
                    if !depth.is_nan() {
                        let key = ((lon * 10000.0).round() as i64, (lat * 10000.0).round() as i64);
                        if seen.insert(key) {
                            // Add a random jitter (~1m) to prevent Kiddo from panicking on exact grid alignments
                            let j_lon = lon + (rand::random::<f64>() - 0.5) * 1e-4;
                            let j_lat = lat + (rand::random::<f64>() - 0.5) * 1e-4;
                            
                            kdtree.add(&[j_lon, j_lat], depths.len() as u64);
                            depths.push(depth);
                        }
                    }
                }
            }
        }
        
        if depths.is_empty() {
            return Err("No valid slab nodes found in the provided XYZ file.".into());
        }
        
        println!("Loaded {} valid slab nodes. Filtering events...", depths.len());
        
        let max_lon_lat_dist_sq = 0.5 * 0.5; // ~50 km max horizontal tolerance
        
        self.events.retain(|e| {
            let nearest = kdtree.nearest_one::<kiddo::SquaredEuclidean>(&[e.lon, e.lat]);
            if nearest.distance > max_lon_lat_dist_sq {
                return false; // Too far horizontally from any slab definition
            }
            let slab_depth = depths[nearest.item as usize];
            let diff = (e.depth.abs() - slab_depth.abs()).abs();
            diff <= buffer_km
        });
        
        Ok(())
    }
    
    /// Get all magnitudes
    pub fn magnitudes(&self) -> Vec<f64> {
        self.events.iter().map(|e| e.mag).collect()
    }
}

/// Haversine distance in km
pub fn haversine_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let r = 6371.0;
    let dlat = (lat2 - lat1).to_radians();
    let dlon = (lon2 - lon1).to_radians();
    let a = (dlat / 2.0).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos() * (dlon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().asin();
    r * c
}

/// Parse datetime from various formats
fn parse_datetime(s: &str) -> Option<NaiveDateTime> {
    // Try ISO 8601 with fractional seconds and Z
    if let Ok(t) = NaiveDateTime::parse_from_str(s.trim_end_matches('Z'), "%Y-%m-%dT%H:%M:%S%.f") {
        return Some(t);
    }
    // ISO 8601 without fractional seconds
    if let Ok(t) = NaiveDateTime::parse_from_str(s.trim_end_matches('Z'), "%Y-%m-%dT%H:%M:%S") {
        return Some(t);
    }
    // Space-separated
    if let Ok(t) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S%.f") {
        return Some(t);
    }
    if let Ok(t) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        return Some(t);
    }
    // Date only
    if let Ok(d) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Some(d.and_hms_opt(0, 0, 0).unwrap());
    }
    None
}
