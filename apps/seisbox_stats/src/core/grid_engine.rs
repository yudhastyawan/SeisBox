use crate::core::catalogue::{Catalogue, haversine_km};
use crate::core::gutenberg_richter::{mc_maxc, mc_gft, mc_emr, bvalue_mle, bvalue_wls};
use rayon::prelude::*;

#[derive(Debug, Clone)]
pub struct GridNodeResult {
    pub lon: f64,
    pub lat: f64,
    pub n_events: usize,
    pub mc: f64,
    pub b_value: f64,
    pub b_unc: f64,
    pub a_value: f64,
    pub z_value: f64,
    pub radius_km: f64,
}

#[derive(Debug, Clone, Copy)]
pub enum GridMethod {
    ConstantN(usize),
    ConstantR(f64),
}

#[derive(Debug, Clone, Copy)]
pub enum McMethod {
    MaxC,
    Gft(f64), // confidence
    Emr,
    Fixed(f64),
}

#[derive(Debug, Clone, Copy)]
pub enum BvalueMethod {
    Mle,
    Wls,
}

pub struct GridConfig {
    pub method: GridMethod,
    pub min_lon: f64,
    pub max_lon: f64,
    pub min_lat: f64,
    pub max_lat: f64,
    pub dlon: f64,
    pub dlat: f64,
    pub mc_method: McMethod,
    pub bvalue_method: BvalueMethod,
    pub bin_width: f64,
    pub min_events: usize,
    pub param: String,
    pub z_tw1: Option<String>,
    pub z_tw2: Option<String>,
}

pub fn run_grid(cat: &Catalogue, config: &GridConfig) -> Vec<GridNodeResult> {
    let mut nodes = Vec::new();
    let mut lon = config.min_lon;
    
    while lon <= config.max_lon + 1e-5 {
        let mut lat = config.min_lat;
        while lat <= config.max_lat + 1e-5 {
            nodes.push((lon, lat));
            lat += config.dlat;
        }
        lon += config.dlon;
    }
    
    let mut tw1 = None;
    let mut tw2 = None;
    
    if config.param == "zvalue" {
        use chrono::NaiveDateTime;
        let parse_tw = |s: &Option<String>| -> Option<(NaiveDateTime, NaiveDateTime)> {
            if let Some(s) = s {
                let parts: Vec<&str> = s.split('/').collect();
                if parts.len() == 2 {
                    let start = NaiveDateTime::parse_from_str(&format!("{} 00:00:00", parts[0]), "%Y-%m-%d %H:%M:%S").ok();
                    let end = NaiveDateTime::parse_from_str(&format!("{} 23:59:59", parts[1]), "%Y-%m-%d %H:%M:%S").ok();
                    if let (Some(st), Some(en)) = (start, end) {
                        return Some((st, en));
                    }
                }
            }
            None
        };
        tw1 = parse_tw(&config.z_tw1);
        tw2 = parse_tw(&config.z_tw2);
        
        if tw1.is_none() || tw2.is_none() {
            eprintln!("Warning: Invalid or missing z_tw1 / z_tw2 for Z-value map. Output will be 0.");
        }
    }
    
    nodes.par_iter().filter_map(|&(lon, lat)| {
        let mut dists: Vec<(f64, &crate::core::catalogue::Earthquake)> = cat.events.iter()
            .map(|e| (haversine_km(lat, lon, e.lat, e.lon), e))
            .collect();
        
        let (selected_events, radius) = match config.method {
            GridMethod::ConstantN(n) => {
                if dists.len() < n {
                    return None;
                }
                dists.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
                let r = dists[n - 1].0;
                let events: Vec<_> = dists.iter().take(n).map(|x| x.1).collect();
                (events, r)
            },
            GridMethod::ConstantR(r) => {
                let events: Vec<_> = dists.iter()
                    .filter(|x| x.0 <= r)
                    .map(|x| x.1)
                    .collect();
                (events, r)
            }
        };
        
        let selected_mags: Vec<f64> = selected_events.iter().map(|e| e.mag).collect();
        
        if selected_mags.len() < config.min_events {
            return None;
        }
        
        let mut z_value = 0.0;
        let mut final_mc = 0.0;
        let mut final_b = 0.0;
        let mut final_b_unc = 0.0;
        let mut final_a = 0.0;
        
        if config.param == "zvalue" {
            if let (Some((t1_start, t1_end)), Some((t2_start, t2_end))) = (tw1, tw2) {
                let n1 = selected_events.iter().filter(|e| e.time >= t1_start && e.time <= t1_end).count() as f64;
                let n2 = selected_events.iter().filter(|e| e.time >= t2_start && e.time <= t2_end).count() as f64;
                
                let duration1_years = (t1_end.and_utc().timestamp() - t1_start.and_utc().timestamp()) as f64 / (365.25 * 86400.0);
                let duration2_years = (t2_end.and_utc().timestamp() - t2_start.and_utc().timestamp()) as f64 / (365.25 * 86400.0);
                
                if duration1_years > 0.0 && duration2_years > 0.0 {
                    let r1 = n1 / duration1_years; // rate per year
                    let r2 = n2 / duration2_years;
                    
                    // Simple rate Z-test formula based on Poissonian variance: variance ~ rate
                    let denom = ((r1 / duration1_years) + (r2 / duration2_years)).sqrt();
                    if denom > 0.0 {
                        z_value = (r1 - r2) / denom;
                    }
                }
            }
        } else {
            final_mc = match config.mc_method {
                McMethod::Fixed(v) => v,
                McMethod::MaxC => mc_maxc(&selected_mags, config.bin_width),
                McMethod::Gft(conf) => mc_gft(&selected_mags, config.bin_width, conf),
                McMethod::Emr => mc_emr(&selected_mags, config.bin_width),
            };
            
            let b_res = match config.bvalue_method {
                BvalueMethod::Mle => bvalue_mle(&selected_mags, final_mc, config.bin_width),
                BvalueMethod::Wls => bvalue_wls(&selected_mags, final_mc, config.bin_width),
            };
            
            if b_res.n_above_mc < config.min_events {
                return None;
            }
            final_b = b_res.b;
            final_b_unc = b_res.b_uncertainty;
            final_a = b_res.a;
        }
        
        Some(GridNodeResult {
            lon,
            lat,
            n_events: selected_mags.len(),
            mc: final_mc,
            b_value: final_b,
            b_unc: final_b_unc,
            a_value: final_a,
            z_value,
            radius_km: radius,
        })
    }).collect()
}

// ============================================================
// Cross Section
// ============================================================

#[derive(Debug, Clone)]
pub struct CrossSectionNodeResult {
    pub dist_km: f64,
    pub depth_km: f64,
    pub n_events: usize,
    pub mc: f64,
    pub b_value: f64,
    pub b_unc: f64,
    pub a_value: f64,
}

pub struct CrossSectionConfig {
    pub start_lon: f64,
    pub start_lat: f64,
    pub end_lon: f64,
    pub end_lat: f64,
    pub width_km: f64,
    pub dist_inc: f64,
    pub depth_inc: f64,
    pub max_depth: f64,
    pub mc_method: McMethod,
    pub bvalue_method: BvalueMethod,
    pub bin_width: f64,
    pub min_events: usize,
}

pub fn run_cross_section(cat: &Catalogue, config: &CrossSectionConfig) -> Vec<CrossSectionNodeResult> {
    let profile_length = haversine_km(config.start_lat, config.start_lon, config.end_lat, config.end_lon);
    
    // Convert geographic coordinates to local Cartesian (km) relative to start point
    // Approximation for projection onto the profile line
    
    // 1. Calculate bearing
    let start_lat_rad = config.start_lat.to_radians();
    let start_lon_rad = config.start_lon.to_radians();
    let end_lat_rad = config.end_lat.to_radians();
    let end_lon_rad = config.end_lon.to_radians();
    
    let dlon = end_lon_rad - start_lon_rad;
    let y = dlon.sin() * end_lat_rad.cos();
    let x = start_lat_rad.cos() * end_lat_rad.sin() - start_lat_rad.sin() * end_lat_rad.cos() * dlon.cos();
    let bearing = y.atan2(x);
    
    // 2. Project events onto profile
    let mut projected_events: Vec<(f64, f64, f64)> = Vec::new(); // (dist_along, dist_across, mag)
    for eq in &cat.events {
        let dist = haversine_km(config.start_lat, config.start_lon, eq.lat, eq.lon);
        
        let eq_lat_rad = eq.lat.to_radians();
        let eq_lon_rad = eq.lon.to_radians();
        let dlon_eq = eq_lon_rad - start_lon_rad;
        let y_eq = dlon_eq.sin() * eq_lat_rad.cos();
        let x_eq = start_lat_rad.cos() * eq_lat_rad.sin() - start_lat_rad.sin() * eq_lat_rad.cos() * dlon_eq.cos();
        let bearing_eq = y_eq.atan2(x_eq);
        
        let angle_diff = bearing_eq - bearing;
        
        let dist_along = dist * angle_diff.cos();
        let dist_across = dist * angle_diff.sin();
        
        if dist_along >= 0.0 && dist_along <= profile_length && dist_across.abs() <= config.width_km {
            projected_events.push((dist_along, eq.depth, eq.mag));
        }
    }
    
    // 3. Grid along profile and depth
    let mut nodes = Vec::new();
    let mut d = 0.0;
    while d <= profile_length + 1e-5 {
        let mut z = 0.0;
        while z <= config.max_depth + 1e-5 {
            nodes.push((d, z));
            z += config.depth_inc;
        }
        d += config.dist_inc;
    }
    
    nodes.par_iter().filter_map(|&(dist, depth)| {
        // Collect events in this node's cell: 
        // dist ± dist_inc/2, depth ± depth_inc/2
        let cell_mags: Vec<f64> = projected_events.iter()
            .filter(|e| (e.0 - dist).abs() <= config.dist_inc / 2.0 && (e.1 - depth).abs() <= config.depth_inc / 2.0)
            .map(|e| e.2)
            .collect();
            
        if cell_mags.len() < config.min_events {
            return None;
        }
        
        let mc = match config.mc_method {
            McMethod::Fixed(v) => v,
            McMethod::MaxC => mc_maxc(&cell_mags, config.bin_width),
            McMethod::Gft(conf) => mc_gft(&cell_mags, config.bin_width, conf),
            McMethod::Emr => mc_emr(&cell_mags, config.bin_width),
        };
        
        let b_res = match config.bvalue_method {
            BvalueMethod::Mle => bvalue_mle(&cell_mags, mc, config.bin_width),
            BvalueMethod::Wls => bvalue_wls(&cell_mags, mc, config.bin_width),
        };
        
        if b_res.n_above_mc < config.min_events {
            return None;
        }
        
        Some(CrossSectionNodeResult {
            dist_km: dist,
            depth_km: depth,
            n_events: cell_mags.len(),
            mc,
            b_value: b_res.b,
            b_unc: b_res.b_uncertainty,
            a_value: b_res.a,
        })
    }).collect()
}
