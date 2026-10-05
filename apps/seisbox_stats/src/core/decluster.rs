use super::catalogue::{Catalogue, haversine_km};

#[derive(Debug, Clone, Copy)]
pub enum WindowType {
    GardnerKnopoff,
    Gruenthal,
    Uhrhammer,
}

/// Time window (days) and distance window (km) for a given magnitude
fn gk_window(mag: f64, window_type: WindowType) -> (f64, f64) {
    match window_type {
        WindowType::GardnerKnopoff => {
            // Gardner & Knopoff (1974)
            let dist = 10f64.powf(0.1238 * mag + 0.983);
            let time = if mag >= 6.5 {
                10f64.powf(0.032 * mag + 2.7389)
            } else {
                10f64.powf(0.5409 * mag - 0.547)
            };
            (time, dist)
        },
        WindowType::Gruenthal => {
            // Gruenthal (1985) / updated
            let dist = (-1.77 + (0.037 + 1.02 * mag).exp()).exp();
            let time = if mag >= 6.5 {
                10f64.powf(2.8 + 0.024 * mag)
            } else {
                (-0.69 + (1.33 + 0.74 * mag).exp()).abs().exp()
            };
            (time, dist)
        },
        WindowType::Uhrhammer => {
            // Uhrhammer (1986)
            let dist = (-1.024 + 0.804 * mag).exp();
            let time = (-2.87 + 1.235 * mag).exp();
            (time, dist)
        },
    }
}

/// Gardner-Knopoff declustering.
/// Returns (mainshock_mask, cluster_ids).
/// cluster_id = 0 means mainshock (no cluster), > 0 means aftershock of that cluster.
pub fn decluster_gardner_knopoff(cat: &Catalogue, window: WindowType) -> (Vec<bool>, Vec<i32>) {
    let n = cat.events.len();
    let mut is_mainshock = vec![true; n];
    let mut cluster_id = vec![0i32; n];
    let mut next_cluster = 1i32;
    
    // Events must be sorted by time (catalogue does this on load)
    for i in 0..n {
        if !is_mainshock[i] {
            continue; // Already classified as aftershock
        }
        
        let eq_i = &cat.events[i];
        let (time_win, dist_win) = gk_window(eq_i.mag, window);
        
        for j in (i + 1)..n {
            let eq_j = &cat.events[j];
            
            // Time difference in days
            let dt = (eq_j.time - eq_i.time).num_seconds() as f64 / 86400.0;
            if dt > time_win {
                break; // Events are sorted by time, no more candidates
            }
            
            // Distance in km
            let dist = haversine_km(eq_i.lat, eq_i.lon, eq_j.lat, eq_j.lon);
            
            if dist <= dist_win {
                if eq_j.mag <= eq_i.mag {
                    // j is aftershock of i
                    is_mainshock[j] = false;
                    if cluster_id[i] == 0 {
                        cluster_id[i] = next_cluster;
                        next_cluster += 1;
                    }
                    cluster_id[j] = cluster_id[i];
                } else {
                    // i is actually foreshock of j
                    is_mainshock[i] = false;
                    if cluster_id[j] == 0 {
                        cluster_id[j] = next_cluster;
                        next_cluster += 1;
                    }
                    cluster_id[i] = cluster_id[j];
                    break;
                }
            }
        }
    }
    
    (is_mainshock, cluster_id)
}

/// Reasenberg (1985) declustering — simplified implementation.
/// Uses a look-ahead approach with interaction zone based on magnitude.
pub fn decluster_reasenberg(cat: &Catalogue) -> (Vec<bool>, Vec<i32>) {
    let n = cat.events.len();
    let mut is_mainshock = vec![true; n];
    let mut cluster_id = vec![0i32; n];
    let mut next_cluster = 1i32;
    
    // Reasenberg interaction zone: radius ~ 10^(0.4*M) km, tau ~ 10^(0.55*M - 0.35) days
    let interaction_radius = |m: f64| -> f64 { 10f64.powf(0.4 * m) };
    let interaction_time = |m: f64| -> f64 { 10f64.powf(0.55 * m - 0.35).min(45.0) };
    
    for i in 0..n {
        if !is_mainshock[i] { continue; }
        
        let eq_i = &cat.events[i];
        let r = interaction_radius(eq_i.mag);
        let tau = interaction_time(eq_i.mag);
        
        for j in (i + 1)..n {
            let eq_j = &cat.events[j];
            let dt = (eq_j.time - eq_i.time).num_seconds() as f64 / 86400.0;
            
            if dt > tau { break; }
            
            let dist = haversine_km(eq_i.lat, eq_i.lon, eq_j.lat, eq_j.lon);
            
            if dist <= r {
                if eq_j.mag <= eq_i.mag {
                    is_mainshock[j] = false;
                    if cluster_id[i] == 0 {
                        cluster_id[i] = next_cluster;
                        next_cluster += 1;
                    }
                    cluster_id[j] = cluster_id[i];
                } else {
                    is_mainshock[i] = false;
                    if cluster_id[j] == 0 {
                        cluster_id[j] = next_cluster;
                        next_cluster += 1;
                    }
                    cluster_id[i] = cluster_id[j];
                    break;
                }
            }
        }
    }
    
    (is_mainshock, cluster_id)
}
