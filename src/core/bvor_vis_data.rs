use std::fs::File;
use std::io::Read;
use std::collections::HashMap;
use statrs::distribution::{ContinuousCDF, Normal};

#[derive(Clone)]
pub struct BVorVisData {
    pub b_grid_all: Vec<Vec<f64>>, // [n_reps][grid_size]
    pub b_vor_all: Vec<Vec<f64>>,  // [n_reps][n_nodes.len() * max_j]
    pub mu_vor_all: Vec<Vec<f64>>,
    pub sig_vor_all: Vec<Vec<f64>>,
    pub ln_l_vor_all: Vec<Vec<f64>>,
    pub n_vor_all: Vec<Vec<f64>>,
    pub pnt_vor_all: Vec<Vec<f64>>, // [n_reps][n_nodes.len() * max_j * 2]
    pub bic_all: Vec<Vec<f64>>,     // [n_reps][n_nodes.len()]
    
    pub m_all: Vec<f64>, // raw magnitudes
    pub x_all: Vec<f64>, // raw x coordinates
    pub y_all: Vec<f64>, // raw y coordinates
    
    pub n_nodes: Vec<usize>,
    pub grid_res: usize,
    pub mode: String,
    pub x_min: f64,
    pub x_max: f64,
    pub y_min: f64,
    pub y_max: f64,
}

pub fn load_bvor_npz(path: &str) -> Result<BVorVisData, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    
    // Read metadata
    let mut meta_str = String::new();
    {
        let mut meta_file = archive.by_name("metadata.json")?;
        meta_file.read_to_string(&mut meta_str)?;
    }
    let meta: serde_json::Value = serde_json::from_str(&meta_str)?;
    
    let mode = meta["mode"].as_str().unwrap_or("spatial").to_string();
    let grid_res = meta["grid_res"].as_u64().unwrap_or(200) as usize;
    let x_min = meta["x_min"].as_f64().unwrap_or(0.0);
    let x_max = meta["x_max"].as_f64().unwrap_or(0.0);
    let y_min = meta["y_min"].as_f64().unwrap_or(0.0);
    let y_max = meta["y_max"].as_f64().unwrap_or(0.0);
    
    let n_nodes: Vec<usize> = meta["n_nodes"].as_array()
        .map(|a| a.iter().map(|v| v.as_u64().unwrap() as usize).collect())
        .unwrap_or_else(|| (2..=60).collect());
    
    // The zip contains folders like `sobol/i_000/b_grid.raw`
    // We don't know exactly how many inits, so we group them by a sorted list of their base paths.
    let mut paths = Vec::new();
    for i in 0..archive.len() {
        let name = archive.by_index(i)?.name().to_string();
        if name.ends_with("b_grid.raw") {
            let base_path = name.trim_end_matches("b_grid.raw").to_string();
            paths.push(base_path);
        }
    }
    
    paths.sort();
    
    let m_all = read_raw_f64(&mut archive, "m.raw").unwrap_or_default();
    let x_all = read_raw_f64(&mut archive, "x.raw").unwrap_or_default();
    let y_all = read_raw_f64(&mut archive, "y.raw").unwrap_or_default();
    
    let mut data = BVorVisData {
        b_grid_all: Vec::new(),
        b_vor_all: Vec::new(),
        mu_vor_all: Vec::new(),
        sig_vor_all: Vec::new(),
        ln_l_vor_all: Vec::new(),
        n_vor_all: Vec::new(),
        pnt_vor_all: Vec::new(),
        bic_all: Vec::new(),
        m_all,
        x_all,
        y_all,
        n_nodes,
        grid_res,
        mode,
        x_min,
        x_max,
        y_min,
        y_max,
    };
    
    for base in &paths {
        data.b_grid_all.push(read_raw_f64(&mut archive, &format!("{}b_grid.raw", base))?);
        data.b_vor_all.push(read_raw_f64(&mut archive, &format!("{}b_vor.raw", base))?);
        data.mu_vor_all.push(read_raw_f64(&mut archive, &format!("{}mu_vor.raw", base))?);
        data.sig_vor_all.push(read_raw_f64(&mut archive, &format!("{}sig_vor.raw", base))?);
        data.ln_l_vor_all.push(read_raw_f64(&mut archive, &format!("{}lnL_vor.raw", base))?);
        data.n_vor_all.push(read_raw_f64(&mut archive, &format!("{}N_vor.raw", base))?);
        data.pnt_vor_all.push(read_raw_f64(&mut archive, &format!("{}pnt_vor.raw", base))?);
        data.bic_all.push(read_raw_f64(&mut archive, &format!("{}bic.raw", base))?);
    }
    
    Ok(data)
}

fn read_raw_f64(archive: &mut zip::ZipArchive<File>, name: &str) -> Result<Vec<f64>, Box<dyn std::error::Error>> {
    let mut file = archive.by_name(name)?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)?;
    
    // Convert u8 buffer to f64
    let floats: &[f64] = unsafe {
        std::slice::from_raw_parts(
            buf.as_ptr() as *const f64,
            buf.len() / std::mem::size_of::<f64>()
        )
    };
    
    Ok(floats.to_vec())
}

/// Port of calculate_bic_grouped
pub fn calculate_bic_grouped(
    data: &BVorVisData,
    max_nodes: usize,
    n_med: usize,
    eps: f64,
    bic_divisor: f64,
) -> (Vec<f64>, Vec<usize>, Vec<f64>) {
    let rep = data.b_grid_all.len();
    let num_n_nodes = data.n_nodes.len();
    let max_j = *data.n_nodes.iter().max().unwrap_or(&60);
    
    let mut bic = Vec::new();
    let mut sum_ln_l = Vec::new();
    
    for i in 0..rep {
        let ln_l = &data.ln_l_vor_all[i];
        let n_vor = &data.n_vor_all[i];
        
        for (k, &j) in data.n_nodes.iter().enumerate() {
            let offset = k * max_j;
            
            let mut sum_lnlj = 0.0;
            let mut sum_log_nj = 0.0;
            let mut valid_count = 0;
            
            for idx in 0..j {
                let l = ln_l[offset + idx];
                let n = n_vor[offset + idx];
                if !n.is_nan() && n > 0.0 {
                    sum_lnlj += l;
                    sum_log_nj += n.ln();
                    valid_count += 1;
                }
            }
            
            let bic_i = sum_lnlj + 2.5 * (valid_count as f64) * sum_log_nj / bic_divisor;
            bic.push(bic_i);
            sum_ln_l.push(sum_lnlj);
        }
    }
    
    let mut nrep = Vec::new();
    for _ in 0..rep {
        nrep.extend_from_slice(&data.n_nodes);
    }
    
    let mut valid_indices = Vec::new();
    for (i, &n) in nrep.iter().enumerate() {
        if n <= max_nodes && !bic[i].is_nan() {
            valid_indices.push(i);
        }
    }
    
    if valid_indices.is_empty() {
        return (bic, Vec::new(), sum_ln_l);
    }
    
    valid_indices.sort_by(|&a, &b| bic[a].partial_cmp(&bic[b]).unwrap_or(std::cmp::Ordering::Equal));
    
    let mut selected = Vec::new();
    let mut last_bic: Option<f64> = None;
    
    for &idx in &valid_indices {
        let b = bic[idx];
        match last_bic {
            Some(last) if (b - last).abs() > eps => {
                selected.push(idx);
                last_bic = Some(b);
            },
            None => {
                selected.push(idx);
                last_bic = Some(b);
            },
            _ => {}
        }
        
        if selected.len() >= n_med {
            break;
        }
    }
    
    (bic, selected, sum_ln_l)
}

pub fn imfd(m: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let mut x = m.to_vec();
    if x.is_empty() {
        return (Vec::new(), Vec::new());
    }
    
    x.sort_by(|a, b| a.partial_cmp(b).unwrap());
    
    let len_f = x.len() as f64;
    let mut y = vec![1.0 / len_f; x.len()];
    if !y.is_empty() {
        y[0] = 0.0;
    }
    
    let mut u = Vec::new();
    let mut sums = Vec::new();
    
    u.push(x[0]);
    sums.push(y[0]);
    
    for i in 1..x.len() {
        if (x[i] - *u.last().unwrap()).abs() > 1e-6 {
            u.push(x[i]);
            sums.push(y[i]);
        } else {
            let last_idx = sums.len() - 1;
            sums[last_idx] += y[i];
        }
    }
    
    (u, sums)
}

pub fn density(m_vals: &[f64], theta: [f64; 3]) -> Vec<f64> {
    let beta = theta[0];
    let mu = theta[1];
    let sigma = theta[2].max(1e-6); // Prevent zero std dev
    
    let normal = match Normal::new(mu, sigma) {
        Ok(n) => n,
        Err(_) => return vec![0.0; m_vals.len()],
    };
    
    let mut num = Vec::with_capacity(m_vals.len());
    let mut sum = 0.0;
    
    for &m in m_vals {
        let val = (-beta * m).exp() * normal.cdf(m);
        num.push(val);
        sum += val;
    }
    
    if sum > 0.0 {
        for v in &mut num {
            *v /= sum;
        }
    }
    
    num
}

pub fn parse_shapefile(path: &str) -> Result<Vec<Vec<[f64; 2]>>, Box<dyn std::error::Error>> {
    let mut reader = shapefile::Reader::from_path(path)?;
    let mut lines = Vec::new();
    
    for shape_record in reader.iter_shapes_and_records() {
        let (shape, _) = shape_record?;
        match shape {
            shapefile::Shape::Polygon(polygon) => {
                for ring in polygon.rings() {
                    let mut line = Vec::new();
                    for pt in ring.points() {
                        line.push([pt.x, pt.y]);
                    }
                    lines.push(line);
                }
            },
            shapefile::Shape::Polyline(polyline) => {
                for part in polyline.parts() {
                    let mut line = Vec::new();
                    for pt in part {
                        line.push([pt.x, pt.y]);
                    }
                    lines.push(line);
                }
            },
            _ => {} // Ignore points, etc.
        }
    }
    
    Ok(lines)
}

pub fn parse_gmt_file(path: &str) -> Result<Vec<Vec<[f64; 2]>>, Box<dyn std::error::Error>> {
    use std::io::BufRead;
    let file = File::open(path)?;
    let reader = std::io::BufReader::new(file);
    
    let mut lines = Vec::new();
    let mut current_line = Vec::new();
    
    for line in reader.lines() {
        let l = line?;
        let l = l.trim();
        if l.starts_with('>') || l.starts_with('#') {
            if !current_line.is_empty() {
                lines.push(current_line);
                current_line = Vec::new();
            }
        } else if !l.is_empty() {
            let parts: Vec<&str> = l.split_whitespace().collect();
            if parts.len() >= 2 {
                if let (Ok(lon), Ok(lat)) = (parts[0].parse::<f64>(), parts[1].parse::<f64>()) {
                    current_line.push([lon, lat]);
                }
            }
        }
    }
    if !current_line.is_empty() {
        lines.push(current_line);
    }
    
    Ok(lines)
}

