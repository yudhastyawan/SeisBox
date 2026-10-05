use nalgebra::{Matrix3, Vector3, SymmetricEigen};
use std::path::Path;
use std::error::Error;

#[derive(Debug, Clone)]
pub struct FocalMechanism {
    pub strike: f64,
    pub dip: f64,
    pub rake: f64,
}

#[derive(Debug, Clone)]
pub struct StressTensorResult {
    pub sigma1: (f64, f64), // (azimuth, plunge)
    pub sigma2: (f64, f64),
    pub sigma3: (f64, f64),
}

pub fn load_focal_mechanisms(path: &Path) -> Result<Vec<FocalMechanism>, Box<dyn Error>> {
    let mut rdr = csv::Reader::from_path(path)?;
    let mut data = Vec::new();
    
    // Auto detect headers for strike, dip, rake
    let headers = rdr.headers()?.clone();
    let mut strike_idx = 0;
    let mut dip_idx = 0;
    let mut rake_idx = 0;
    
    for (i, h) in headers.iter().enumerate() {
        let h_low = h.to_lowercase();
        if h_low.contains("strike") { strike_idx = i; }
        else if h_low.contains("dip") { dip_idx = i; }
        else if h_low.contains("rake") { rake_idx = i; }
    }
    
    for result in rdr.records() {
        let record = result?;
        let strike: f64 = record.get(strike_idx).unwrap_or("0").parse()?;
        let dip: f64 = record.get(dip_idx).unwrap_or("0").parse()?;
        let rake: f64 = record.get(rake_idx).unwrap_or("0").parse()?;
        data.push(FocalMechanism { strike, dip, rake });
    }
    
    Ok(data)
}

/// Computes the P, T, and B axes for a given focal mechanism.
/// Returns (P-axis, T-axis, B-axis) as unit vectors.
fn focal_mech_to_ptb(fm: &FocalMechanism) -> (Vector3<f64>, Vector3<f64>, Vector3<f64>) {
    let s = fm.strike.to_radians();
    let d = fm.dip.to_radians();
    let r = fm.rake.to_radians();

    // Fault normal vector (n)
    let n = Vector3::new(
        -d.sin() * s.sin(),
        d.sin() * s.cos(),
        -d.cos(),
    );

    // Slip vector (s)
    let slip = Vector3::new(
        r.cos() * s.cos() + r.sin() * d.cos() * s.sin(),
        r.cos() * s.sin() - r.sin() * d.cos() * s.cos(),
        -r.sin() * d.sin(),
    );

    let p = (n - slip).normalize();
    let t = (n + slip).normalize();
    let b = n.cross(&slip).normalize();

    (p, t, b)
}

/// Simplified stress tensor inversion based on summing P, T, and B axes matrices.
/// This is a basic kinematic approach.
pub fn invert_stress_tensor(fms: &[FocalMechanism]) -> Result<StressTensorResult, Box<dyn Error>> {
    if fms.is_empty() {
        return Err("No focal mechanism data provided.".into());
    }

    let mut sum_matrix = Matrix3::zeros();

    for fm in fms {
        let (p, t, _b) = focal_mech_to_ptb(fm);
        // We sum the outer products. P is compressional, T is extensional.
        let m = t * t.transpose() - p * p.transpose();
        sum_matrix += m;
    }

    sum_matrix /= fms.len() as f64;

    // Calculate eigenvalues and eigenvectors
    let eigen = SymmetricEigen::new(sum_matrix);
    
    // Sort eigenvalues (sigma1 > sigma2 > sigma3)
    let mut evals: Vec<(usize, f64)> = eigen.eigenvalues.iter().enumerate().map(|(i, &v)| (i, v)).collect();
    evals.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap()); // Descending

    let vec_to_az_pl = |v: Vector3<f64>| -> (f64, f64) {
        let v = if v[2] > 0.0 { -v } else { v }; // point downwards
        let mut az = v[0].atan2(v[1]).to_degrees(); // North is Y, East is X
        if az < 0.0 { az += 360.0; }
        let pl = v[2].asin().to_degrees().abs();
        (az, pl)
    };

    let sigma1_vec = eigen.eigenvectors.column(evals[0].0).into_owned();
    let sigma2_vec = eigen.eigenvectors.column(evals[1].0).into_owned();
    let sigma3_vec = eigen.eigenvectors.column(evals[2].0).into_owned();

    Ok(StressTensorResult {
        sigma1: vec_to_az_pl(sigma1_vec),
        sigma2: vec_to_az_pl(sigma2_vec),
        sigma3: vec_to_az_pl(sigma3_vec),
    })
}
