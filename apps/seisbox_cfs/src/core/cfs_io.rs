use std::path::Path;
use crate::core::cfs_parser::CoulombInput;
use crate::core::cfs_runner::{CoulombResult, DeformationResult, CrossSectionResult};

pub fn write_coulomb_csv(path: &Path, results: &[CoulombResult]) {
    let mut wtr = csv::Writer::from_path(path).unwrap();
    wtr.write_record(&[
        "X_km", "Y_km", "Z_km", "Lon", "Lat",
        "Strike", "Dip", "Rake",
        "ux_m", "uy_m", "uz_m",
        "sxx_bar", "syy_bar", "szz_bar",
        "syz_bar", "sxz_bar", "sxy_bar",
        "Shear_bar", "Normal_bar", "Coulomb_bar"
    ]).unwrap();
    for r in results {
        wtr.write_record(&[
            r.x.to_string(), r.y.to_string(), r.z.to_string(),
            r.lon.to_string(), r.lat.to_string(),
            r.strike.to_string(), r.dip.to_string(), r.rake.to_string(),
            r.ux.to_string(), r.uy.to_string(), r.uz.to_string(),
            r.sxx.to_string(), r.syy.to_string(), r.szz.to_string(),
            r.syz.to_string(), r.sxz.to_string(), r.sxy.to_string(),
            r.shear.to_string(), r.normal.to_string(), r.coulomb.to_string()
        ]).unwrap();
    }
    wtr.flush().unwrap();
}

pub fn write_deformation_csv(path: &Path, results: &[DeformationResult]) {
    let mut wtr = csv::Writer::from_path(path).unwrap();
    wtr.write_record(&["x", "y", "z", "ux", "uy", "uz", "sxx", "syy", "szz", "syz", "sxz", "sxy"]).unwrap();
    for r in results {
        wtr.write_record(&[
            r.x.to_string(), r.y.to_string(), r.z.to_string(),
            r.ux.to_string(), r.uy.to_string(), r.uz.to_string(),
            r.sxx.to_string(), r.syy.to_string(), r.szz.to_string(),
            r.syz.to_string(), r.sxz.to_string(), r.sxy.to_string()
        ]).unwrap();
    }
    wtr.flush().unwrap();
}

pub fn write_cross_section_csv(path: &Path, results: &[CrossSectionResult]) {
    let mut wtr = csv::Writer::from_path(path).unwrap();
    wtr.write_record(&[
        "Distance_km", "Depth_km", "Lon", "Lat",
        "Strike", "Dip", "Rake",
        "ux_m", "uy_m", "uz_m",
        "sxx_bar", "syy_bar", "szz_bar",
        "syz_bar", "sxz_bar", "sxy_bar",
        "Shear_bar", "Normal_bar", "Coulomb_bar"
    ]).unwrap();
    for r in results {
        wtr.write_record(&[
            r.distance.to_string(), (-r.cfs.z).to_string(),
            r.cfs.lon.to_string(), r.cfs.lat.to_string(),
            r.cfs.strike.to_string(), r.cfs.dip.to_string(), r.cfs.rake.to_string(),
            r.cfs.ux.to_string(), r.cfs.uy.to_string(), r.cfs.uz.to_string(),
            r.cfs.sxx.to_string(), r.cfs.syy.to_string(), r.cfs.szz.to_string(),
            r.cfs.syz.to_string(), r.cfs.sxz.to_string(), r.cfs.sxy.to_string(),
            r.cfs.shear.to_string(), r.cfs.normal.to_string(), r.cfs.coulomb.to_string()
        ]).unwrap();
    }
    wtr.flush().unwrap();
}

pub fn write_coulomb_tiff(path: &Path, results: &[CoulombResult], input: &CoulombInput) -> Result<(), String> {
    let w = input.xvec.len();
    let h = input.yvec.len();
    
    if w == 0 || h == 0 {
        return Err("Grid dimensions are zero".to_string());
    }
    
    if results.len() % (w * h) != 0 {
        return Err(format!("Results length ({}) is not a multiple of grid size ({}x{} = {})", results.len(), w, h, w * h));
    }
    
    // We only take the first w*h results (in case of multiple depths, we just take the first slice, or we could take max)
    // Actually, cfs_runner appends max_results at the end. So if we want the surface or max, we should pick appropriately.
    // The previous GUI just checked if res.len() == w*h. Let's just take the last slice if it's max over depth, or first slice.
    // To be safe, we just use the last w*h items, which corresponds to the max_depth projection if depths > 1, or the only depth.
    let slice_start = results.len() - (w * h);
    let grid_results = &results[slice_start..];
    
    let mut tiff_data = vec![0.0_f32; w * h];
    
    // Since grid_results are sorted by X (outer loop) then Y (inner loop),
    for (i, r) in grid_results.iter().enumerate() {
        let ix = i / h;
        let iy = i % h;
        
        let tiff_y = h - 1 - iy; // flip Y for image coordinates
        let tiff_x = ix;
        tiff_data[tiff_y * w + tiff_x] = r.coulomb as f32;
    }
    
    let mut x_min = *input.xvec.first().unwrap_or(&0.0);
    let mut x_max = *input.xvec.last().unwrap_or(&1.0);
    let mut y_min = *input.yvec.first().unwrap_or(&0.0);
    let mut y_max = *input.yvec.last().unwrap_or(&1.0);
    
    // Gunakan Lon/Lat jika tersedia agar TIFF berada pada koordinat geografis
    if input.map_info.min_lon != 0.0 || input.map_info.max_lon != 0.0 {
        x_min = input.map_info.min_lon;
        x_max = input.map_info.max_lon;
        y_min = input.map_info.min_lat;
        y_max = input.map_info.max_lat;
    }
    
    seisbox_core::io::tiff_export::export_grid_to_tiff(
        path, &tiff_data, 
        w as u32, h as u32, 
        x_min, x_max, y_min, y_max
    ).map_err(|e| e.to_string())
}
pub fn generate_coulomb_inp_content(
    strike: f64, dip: f64, rake: f64,
    length: f64, width: f64, depth: f64, slip: f64,
    min_x: f64, max_x: f64, x_inc: f64,
    min_y: f64, max_y: f64, y_inc: f64,
    min_lon: f64, max_lon: f64, zero_lon: f64,
    min_lat: f64, max_lat: f64, zero_lat: f64,
    fric: f64, pois: f64, young: f64,
) -> String {
    let strike_rad = strike.to_radians();
    let dx = (length / 2.0) * strike_rad.sin();
    let dy = (length / 2.0) * strike_rad.cos();
    
    let mut x_start = -dx;
    let mut y_start = -dy;
    let mut x_fin = dx;
    let mut y_fin = dy;
    
    let dd = (width / 2.0) * dip.to_radians().cos();
    let shift_rad = (strike - 90.0).to_radians();
    let zx = dd * shift_rad.sin();
    let zy = dd * shift_rad.cos();
    
    x_start += zx;
    y_start += zy;
    x_fin += zx;
    y_fin += zy;
    
    let rt_lat = -slip * rake.to_radians().cos();
    let reverse = slip * rake.to_radians().sin();
    
    let top = depth - (width / 2.0) * dip.to_radians().sin();
    let bot = depth + (width / 2.0) * dip.to_radians().sin();
    
    format!(
r#"header line 1 
header line 2 
#reg1=  0  #reg2=  0  #fixed=   1  sym=  1
 PR1=       {pois:.3}     PR2=       {pois:.3}   DEPTH=      {depth:.3}
  E1=      {young:.2e}   E2=      {young:.2e}
XSYM=       .000     YSYM=       .000
FRIC=          {fric:.3}
S1DR=         19.000 S1DP=         -0.010 S1IN=        100.000 S1GD=          0.000
S2DR=         89.990 S2DP=         89.990 S2IN=         30.000 S2GD=          0.000
S3DR=        109.000 S3DP=         -0.010 S3IN=          0.000 S3GD=          0.000

  #   X-start    Y-start     X-fin      Y-fin   Kode  rt.lat    reverse   dip angle     top      bot
xxx xxxxxxxxxx xxxxxxxxxx xxxxxxxxxx xxxxxxxxxx xxx xxxxxxxxxx xxxxxxxxxx xxxxxxxxxx xxxxxxxxxx xxxxxxxxxx
  1 {:10.4} {:10.4} {:10.4} {:10.4} 100 {:10.4} {:10.4} {:10.4} {:10.4} {:10.4}    Fault 1 
  
    Grid Parameters
  1  ----------------------------  Start-x =  {:15.7}
  2  ----------------------------  Start-y =  {:15.7}
  3  --------------------------   Finish-x =  {:15.7}
  4  --------------------------   Finish-y =  {:15.7}
  5  ------------------------  x-increment =  {:15.7}
  6  ------------------------  y-increment =  {:15.7}
     Size Parameters
  1  --------------------------  Plot size =        2.0000000
  2  --------------  Shade/Color increment =        1.0000000
  3  ------  Exaggeration for disp.& dist. =    10000.0000000
  
     Cross section default
  1  ----------------------------  Start-x =  {:15.7}
  2  ----------------------------  Start-y =  {:15.7}
  3  --------------------------   Finish-x =  {:15.7}
  4  --------------------------   Finish-y =  {:15.7}
  5  ------------------  Distant-increment =  {:15.7}
  6  ----------------------------  Z-depth =  {:15.7}
  7  ------------------------  Z-increment =        1.0000000
     Map info
  1  ---------------------------- min. lon =  {:15.7}
  2  ---------------------------- max. lon =  {:15.7}
  3  ---------------------------- zero lon =  {:15.7}
  4  ---------------------------- min. lat =  {:15.7}
  5  ---------------------------- max. lat =  {:15.7}
  6  ---------------------------- zero lat =  {:15.7}
"#,
        x_start, y_start, x_fin, y_fin, rt_lat, reverse, dip, top, bot,
        min_x, min_y, max_x, max_y, x_inc, y_inc,
        min_x, min_y, max_x, max_y, x_inc, depth,
        min_lon, max_lon, zero_lon, min_lat, max_lat, zero_lat
    )
}

pub fn generate_fault_line_string(
    id: usize,
    strike: f64, dip: f64, rake: f64,
    length: f64, width: f64, depth: f64, slip: f64,
    fault_center_x: f64, fault_center_y: f64
) -> String {
    let strike_rad = strike.to_radians();
    let dx = (length / 2.0) * strike_rad.sin();
    let dy = (length / 2.0) * strike_rad.cos();
    
    let mut x_start = -dx;
    let mut y_start = -dy;
    let mut x_fin = dx;
    let mut y_fin = dy;
    
    let dd = (width / 2.0) * dip.to_radians().cos();
    let shift_rad = (strike - 90.0).to_radians();
    let zx = dd * shift_rad.sin();
    let zy = dd * shift_rad.cos();
    
    x_start += zx;
    y_start += zy;
    x_fin += zx;
    y_fin += zy;
    
    // Shift by fault_center
    x_start += fault_center_x;
    x_fin += fault_center_x;
    y_start += fault_center_y;
    y_fin += fault_center_y;
    
    let rt_lat = -slip * rake.to_radians().cos();
    let reverse = slip * rake.to_radians().sin();
    
    let top = depth - (width / 2.0) * dip.to_radians().sin();
    let bot = depth + (width / 2.0) * dip.to_radians().sin();
    
    format!(
        "{:3} {:10.4} {:10.4} {:10.4} {:10.4} 100 {:10.4} {:10.4} {:10.4} {:10.4} {:10.4}    Fault {} ",
        id, x_start, y_start, x_fin, y_fin, rt_lat, reverse, dip, top, bot, id
    )
}

pub fn read_coulomb_csv(path: &Path) -> Result<Vec<CoulombResult>, Box<dyn std::error::Error>> {
    let mut rdr = csv::Reader::from_path(path)?;
    let mut results = Vec::new();
    for result in rdr.records() {
        let record = result?;
        if record.len() < 20 { continue; }
        results.push(CoulombResult {
            x: record[0].parse()?,
            y: record[1].parse()?,
            z: record[2].parse()?,
            lon: record[3].parse()?,
            lat: record[4].parse()?,
            strike: record[5].parse()?,
            dip: record[6].parse()?,
            rake: record[7].parse()?,
            ux: record[8].parse()?,
            uy: record[9].parse()?,
            uz: record[10].parse()?,
            sxx: record[11].parse()?,
            syy: record[12].parse()?,
            szz: record[13].parse()?,
            syz: record[14].parse()?,
            sxz: record[15].parse()?,
            sxy: record[16].parse()?,
            shear: record[17].parse()?,
            normal: record[18].parse()?,
            coulomb: record[19].parse()?,
        });
    }
    Ok(results)
}

pub fn read_cross_section_csv(path: &Path) -> Result<Vec<CrossSectionResult>, Box<dyn std::error::Error>> {
    let mut rdr = csv::Reader::from_path(path)?;
    let mut results = Vec::new();
    for result in rdr.records() {
        let record = result?;
        if record.len() < 19 { continue; }
        results.push(CrossSectionResult {
            distance: record[0].parse()?,
            cfs: CoulombResult {
                x: 0.0, y: 0.0,
                z: -record[1].parse::<f64>()?,
                lon: record[2].parse()?,
                lat: record[3].parse()?,
                strike: record[4].parse()?,
                dip: record[5].parse()?,
                rake: record[6].parse()?,
                ux: record[7].parse()?,
                uy: record[8].parse()?,
                uz: record[9].parse()?,
                sxx: record[10].parse()?,
                syy: record[11].parse()?,
                szz: record[12].parse()?,
                syz: record[13].parse()?,
                sxz: record[14].parse()?,
                sxy: record[15].parse()?,
                shear: record[16].parse()?,
                normal: record[17].parse()?,
                coulomb: record[18].parse()?,
            }
        });
    }
    Ok(results)
}

