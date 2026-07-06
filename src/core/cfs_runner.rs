use rayon::prelude::*;
use crate::core::cfs_parser::{CoulombInput, BatchInput};
use crate::core::okada_math::okada_dc3d_single;
use crate::core::cfs_math::{coord_conversion_single, tensor_trans_single, calc_coulomb_single};

#[derive(Debug, Clone)]
pub struct DeformationResult {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub ux: f64,
    pub uy: f64,
    pub uz: f64,
    pub sxx: f64,
    pub syy: f64,
    pub szz: f64,
    pub syz: f64,
    pub sxz: f64,
    pub sxy: f64,
}

#[derive(Debug, Clone)]
pub struct CoulombResult {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub lon: f64,
    pub lat: f64,
    pub strike: f64,
    pub dip: f64,
    pub rake: f64,
    pub ux: f64,
    pub uy: f64,
    pub uz: f64,
    pub sxx: f64,
    pub syy: f64,
    pub szz: f64,
    pub syz: f64,
    pub sxz: f64,
    pub sxy: f64,
    pub shear: f64,
    pub normal: f64,
    pub coulomb: f64,
}

pub fn calculate_deformation(input: &CoulombInput, depths: &[f64]) -> Vec<DeformationResult> {
    let alpha = 1.0 / (2.0 * (1.0 - input.pois));
    let sk = input.young / (1.0 + input.pois);
    let gk = input.pois / (1.0 - 2.0 * input.pois);
    
    let mut coords = Vec::new();
    for &x in &input.xvec {
        for &y in &input.yvec {
            for &d in depths {
                coords.push((x, y, -d));
            }
        }
    }
    
    // Process each coordinate in parallel using rayon
    coords.par_iter().map(|&(xg, yg, zg)| {
        let mut uxg_sum = 0.0;
        let mut uyg_sum = 0.0;
        let mut uzg_sum = 0.0;
        let mut sxx_n_sum = 0.0;
        let mut syy_n_sum = 0.0;
        let mut szz_n_sum = 0.0;
        let mut syz_n_sum = 0.0;
        let mut sxz_n_sum = 0.0;
        let mut sxy_n_sum = 0.0;
        
        for (i, el) in input.el.iter().enumerate() {
            let depth = (el[7] + el[8]) / 2.0;
            let (c1, c2, c3, c4) = coord_conversion_single(
                xg, yg, el[0], el[1], el[2], el[3], el[7], el[8], el[6]
            );
            
            let kd = input.kode[i];
            let (ux, uy, uz, uxx, uyx, uzx, uxy, uyy, uzy, uxz, uyz, uzz) = if kd == 100 {
                let (u, _) = okada_dc3d_single(
                    alpha, c1, c2, zg, depth, el[6], c3, c3, c4, c4, -el[4], el[5], 0.0
                );
                (u[0], u[1], u[2], u[3], u[4], u[5], u[6], u[7], u[8], u[9], u[10], u[11])
            } else {
                (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
            };
            
            let sw = ((el[3] - el[1]).powi(2) + (el[2] - el[0]).powi(2)).sqrt();
            let sina = if sw > 0.0 { (el[3] - el[1]) / sw } else { 0.0 };
            let cosa = if sw > 0.0 { (el[2] - el[0]) / sw } else { 1.0 };
            
            let uxg = ux * cosa - uy * sina;
            let uyg = ux * sina + uy * cosa;
            let uzg = uz;
            
            let vol = uxx + uyy + uzz;
            let sxx = sk * (gk * vol + uxx) * 0.001;
            let syy = sk * (gk * vol + uyy) * 0.001;
            let szz = sk * (gk * vol + uzz) * 0.001;
            let sxy = (input.young / (2.0 * (1.0 + input.pois))) * (uxy + uyx) * 0.001;
            let sxz = (input.young / (2.0 * (1.0 + input.pois))) * (uxz + uzx) * 0.001;
            let syz = (input.young / (2.0 * (1.0 + input.pois))) * (uyz + uzy) * 0.001;
            
            let s0 = [sxx, syy, szz, syz, sxz, sxy];
            let s1 = tensor_trans_single(sina, cosa, &s0);
            
            uxg_sum += uxg;
            uyg_sum += uyg;
            uzg_sum += uzg;
            sxx_n_sum += s1[0];
            syy_n_sum += s1[1];
            szz_n_sum += s1[2];
            syz_n_sum += s1[3];
            sxz_n_sum += s1[4];
            sxy_n_sum += s1[5];
        }
        
        DeformationResult {
            x: xg,
            y: yg,
            z: zg,
            ux: uxg_sum,
            uy: uyg_sum,
            uz: uzg_sum,
            sxx: sxx_n_sum,
            syy: syy_n_sum,
            szz: szz_n_sum,
            syz: syz_n_sum,
            sxz: sxz_n_sum,
            sxy: sxy_n_sum,
        }
    }).collect()
}

pub fn calculate_coulomb_grid(input: &CoulombInput, depths: &[f64], receiver_strike: f64, receiver_dip: f64, receiver_rake: f64) -> Vec<CoulombResult> {
    let def_res = calculate_deformation(input, depths);
    
    let zero_lon = input.map_info.zero_lon;
    let zero_lat = input.map_info.zero_lat;
    let earth_r = 6371.0;
    
    let mut all_results: Vec<CoulombResult> = def_res.into_iter().map(|d| {
        let lat = zero_lat + (d.y / earth_r) * (180.0 / std::f64::consts::PI);
        let lon = zero_lon + (d.x / (earth_r * zero_lat.to_radians().cos())) * (180.0 / std::f64::consts::PI);
        
        let ss = [d.sxx, d.syy, d.szz, d.syz, d.sxz, d.sxy];
        let (shear, normal, coulomb) = calc_coulomb_single(receiver_strike, receiver_dip, receiver_rake, input.fric, &ss);
        
        CoulombResult {
            x: d.x,
            y: d.y,
            z: d.z,
            lon,
            lat,
            strike: receiver_strike,
            dip: receiver_dip,
            rake: receiver_rake,
            ux: d.ux,
            uy: d.uy,
            uz: d.uz,
            sxx: d.sxx,
            syy: d.syy,
            szz: d.szz,
            syz: d.syz,
            sxz: d.sxz,
            sxy: d.sxy,
            shear,
            normal,
            coulomb,
        }
    }).collect();

    if depths.len() > 1 {
        let n_depths = depths.len();
        let mut max_results = Vec::with_capacity(all_results.len() / n_depths);
        
        for chunk in all_results.chunks(n_depths) {
            let mut max_res = chunk[0].clone();
            max_res.z = 999.0;
            
            for m in chunk.iter().skip(1) {
                max_res.ux = max_res.ux.max(m.ux);
                max_res.uy = max_res.uy.max(m.uy);
                max_res.uz = max_res.uz.max(m.uz);
                max_res.sxx = max_res.sxx.max(m.sxx);
                max_res.syy = max_res.syy.max(m.syy);
                max_res.szz = max_res.szz.max(m.szz);
                max_res.syz = max_res.syz.max(m.syz);
                max_res.sxz = max_res.sxz.max(m.sxz);
                max_res.sxy = max_res.sxy.max(m.sxy);
                max_res.shear = max_res.shear.max(m.shear);
                max_res.normal = max_res.normal.max(m.normal);
                max_res.coulomb = max_res.coulomb.max(m.coulomb);
            }
            max_results.push(max_res);
        }
        all_results.extend(max_results);
    }
    
    all_results
}

/// Which stress component to maximize during optimization
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OptTarget {
    Shear,
    Normal,
    Coulomb,
}

impl OptTarget {
    pub fn suffix(&self) -> &'static str {
        match self {
            OptTarget::Shear => "_max_shear",
            OptTarget::Normal => "_max_normal",
            OptTarget::Coulomb => "_max_coulomb",
        }
    }
    pub fn extract(&self, r: &CoulombResult) -> f64 {
        match self {
            OptTarget::Shear => r.shear,
            OptTarget::Normal => r.normal,
            OptTarget::Coulomb => r.coulomb,
        }
    }
}

/// Calculate coulomb grid with optimization: searches over selected receiver parameters
/// to find the combination that maximizes the target stress component at each grid point.
pub fn calculate_coulomb_grid_optimized(
    input: &CoulombInput,
    depths: &[f64],
    base_strike: f64,
    base_dip: f64,
    base_rake: f64,
    opt_strike: bool,
    opt_dip: bool,
    opt_rake: bool,
    opt_strike_inc: f64,
    opt_dip_inc: f64,
    opt_rake_inc: f64,
    target: OptTarget,
) -> Vec<CoulombResult> {
    // Pre-compute deformation (shared across all receiver orientations)
    let def_res = calculate_deformation(input, depths);
    
    let zero_lon = input.map_info.zero_lon;
    let zero_lat = input.map_info.zero_lat;
    let earth_r = 6371.0;
    let fric = input.fric;
    
    // Build search ranges
    let strikes: Vec<f64> = if opt_strike {
        let inc = opt_strike_inc.max(1.0);
        let mut vals = Vec::new();
        let mut v = 0.0;
        while v < 360.0 { vals.push(v); v += inc; }
        vals
    } else {
        vec![base_strike]
    };
    let dips: Vec<f64> = if opt_dip {
        let inc = opt_dip_inc.max(1.0);
        let mut vals = Vec::new();
        let mut v = 0.0;
        while v <= 90.0 { vals.push(v); v += inc; }
        vals
    } else {
        vec![base_dip]
    };
    let rakes: Vec<f64> = if opt_rake {
        let inc = opt_rake_inc.max(1.0);
        let mut vals = Vec::new();
        let mut v = -180.0;
        while v <= 180.0 { vals.push(v); v += inc; }
        vals
    } else {
        vec![base_rake]
    };
    
    // For each deformation point, search for optimal receiver parameters
    let mut all_results: Vec<CoulombResult> = def_res.par_iter().map(|d| {
        let lat = zero_lat + (d.y / earth_r) * (180.0 / std::f64::consts::PI);
        let lon = zero_lon + (d.x / (earth_r * zero_lat.to_radians().cos())) * (180.0 / std::f64::consts::PI);
        let ss = [d.sxx, d.syy, d.szz, d.syz, d.sxz, d.sxy];
        
        let mut best_val = f64::NEG_INFINITY;
        let mut best_s = base_strike;
        let mut best_d = base_dip;
        let mut best_r = base_rake;
        let mut best_shear = 0.0;
        let mut best_normal = 0.0;
        let mut best_coulomb = 0.0;
        
        for &s in &strikes {
            for &dp in &dips {
                for &rk in &rakes {
                    let (shear, normal, coulomb) = calc_coulomb_single(s, dp, rk, fric, &ss);
                    let val = match target {
                        OptTarget::Shear => shear,
                        OptTarget::Normal => normal,
                        OptTarget::Coulomb => coulomb,
                    };
                    if val > best_val {
                        best_val = val;
                        best_s = s;
                        best_d = dp;
                        best_r = rk;
                        best_shear = shear;
                        best_normal = normal;
                        best_coulomb = coulomb;
                    }
                }
            }
        }
        
        CoulombResult {
            x: d.x,
            y: d.y,
            z: d.z,
            lon,
            lat,
            strike: best_s,
            dip: best_d,
            rake: best_r,
            ux: d.ux,
            uy: d.uy,
            uz: d.uz,
            sxx: d.sxx,
            syy: d.syy,
            szz: d.szz,
            syz: d.syz,
            sxz: d.sxz,
            sxy: d.sxy,
            shear: best_shear,
            normal: best_normal,
            coulomb: best_coulomb,
        }
    }).collect();
    
    // Append max-over-depth rows if multiple depths
    if depths.len() > 1 {
        let n_depths = depths.len();
        let mut max_results = Vec::with_capacity(all_results.len() / n_depths);
        
        for chunk in all_results.chunks(n_depths) {
            // Find the entry with the best target value
            let best_idx = chunk.iter().enumerate()
                .max_by(|(_, a), (_, b)| {
                    let va = match target {
                        OptTarget::Shear => a.shear,
                        OptTarget::Normal => a.normal,
                        OptTarget::Coulomb => a.coulomb,
                    };
                    let vb = match target {
                        OptTarget::Shear => b.shear,
                        OptTarget::Normal => b.normal,
                        OptTarget::Coulomb => b.coulomb,
                    };
                    va.partial_cmp(&vb).unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(i, _)| i)
                .unwrap_or(0);
            
            let mut max_res = chunk[best_idx].clone();
            max_res.z = 999.0;
            max_results.push(max_res);
        }
        all_results.extend(max_results);
    }
    
    all_results
}

pub fn calculate_coulomb_batch(input: &CoulombInput, batch: &BatchInput) -> Vec<CoulombResult> {
    let alpha = 1.0 / (2.0 * (1.0 - input.pois));
    let sk = input.young / (1.0 + input.pois);
    let gk = input.pois / (1.0 - 2.0 * input.pois);
    
    batch.pos.par_iter().enumerate().map(|(idx, pos)| {
        let (xg, yg, zg_pos) = (pos[0], pos[1], pos[2]);
        let zg = zg_pos;
        
        let mut sxx_n_sum = 0.0;
        let mut syy_n_sum = 0.0;
        let mut szz_n_sum = 0.0;
        let mut syz_n_sum = 0.0;
        let mut sxz_n_sum = 0.0;
        let mut sxy_n_sum = 0.0;
        
        let mut uxg_sum = 0.0;
        let mut uyg_sum = 0.0;
        let mut uzg_sum = 0.0;
        
        for (i, el) in input.el.iter().enumerate() {
            let depth = (el[7] + el[8]) / 2.0;
            let (c1, c2, c3, c4) = coord_conversion_single(
                xg, yg, el[0], el[1], el[2], el[3], el[7], el[8], el[6]
            );
            
            let kd = input.kode[i];
            let (ux, uy, uz, uxx, uyx, uzx, uxy, uyy, uzy, uxz, uyz, uzz) = if kd == 100 {
                let (u, _) = okada_dc3d_single(
                    alpha, c1, c2, zg, depth, el[6], c3, c3, c4, c4, -el[4], el[5], 0.0
                );
                (u[0], u[1], u[2], u[3], u[4], u[5], u[6], u[7], u[8], u[9], u[10], u[11])
            } else {
                (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
            };
            
            let sw = ((el[3] - el[1]).powi(2) + (el[2] - el[0]).powi(2)).sqrt();
            let sina = if sw > 0.0 { (el[3] - el[1]) / sw } else { 0.0 };
            let cosa = if sw > 0.0 { (el[2] - el[0]) / sw } else { 1.0 };
            
            let vol = uxx + uyy + uzz;
            let sxx = sk * (gk * vol + uxx) * 0.001;
            let syy = sk * (gk * vol + uyy) * 0.001;
            let szz = sk * (gk * vol + uzz) * 0.001;
            let sxy = (input.young / (2.0 * (1.0 + input.pois))) * (uxy + uyx) * 0.001;
            let sxz = (input.young / (2.0 * (1.0 + input.pois))) * (uxz + uzx) * 0.001;
            let syz = (input.young / (2.0 * (1.0 + input.pois))) * (uyz + uzy) * 0.001;
            
            let s0 = [sxx, syy, szz, syz, sxz, sxy];
            let s1 = tensor_trans_single(sina, cosa, &s0);
            
            sxx_n_sum += s1[0];
            syy_n_sum += s1[1];
            szz_n_sum += s1[2];
            syz_n_sum += s1[3];
            sxz_n_sum += s1[4];
            sxy_n_sum += s1[5];
            
            // Transform displacement back to global coordinate system
            // In python: 
            // uxg = ux * sina - uy * cosa
            // uyg = ux * cosa + uy * sina
            // uzg = uz
            let uxg = ux * sina - uy * cosa;
            let uyg = ux * cosa + uy * sina;
            let uzg = uz;
            
            uxg_sum += uxg;
            uyg_sum += uyg;
            uzg_sum += uzg;
        }
        
        let ss = [sxx_n_sum, syy_n_sum, szz_n_sum, syz_n_sum, sxz_n_sum, sxy_n_sum];
        let (shear, normal, coulomb) = calc_coulomb_single(batch.strike[idx], batch.dip[idx], batch.rake[idx], input.fric, &ss);
        
        let zero_lon = input.map_info.zero_lon;
        let zero_lat = input.map_info.zero_lat;
        let earth_r = 6371.0;
        let lat = zero_lat + (yg / earth_r) * (180.0 / std::f64::consts::PI);
        let lon = zero_lon + (xg / (earth_r * zero_lat.to_radians().cos())) * (180.0 / std::f64::consts::PI);
        
        CoulombResult {
            x: xg,
            y: yg,
            z: zg_pos,
            lon,
            lat,
            strike: batch.strike[idx],
            dip: batch.dip[idx],
            rake: batch.rake[idx],
            ux: uxg_sum,
            uy: uyg_sum,
            uz: uzg_sum,
            sxx: sxx_n_sum,
            syy: syy_n_sum,
            szz: szz_n_sum,
            syz: syz_n_sum,
            sxz: sxz_n_sum,
            sxy: sxy_n_sum,
            shear,
            normal,
            coulomb,
        }
    }).collect()
}
