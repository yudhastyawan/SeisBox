use super::bvor::{b_voronoi, VoronoiResult};
use rayon::prelude::*;
use rayon::iter::IntoParallelRefIterator;
use std::sync::{Arc, Mutex};
use std::fs::File;
use std::io::Write;
use zip::write::SimpleFileOptions;

pub struct BVorConfig {
    pub mode: String, // "spatial" or "temporal"
    pub n_nodes_range: std::ops::RangeInclusive<usize>,
    pub init_methods: Vec<(String, usize)>, // (method, count)
    pub grid_res: usize,
    pub min_obs: usize,
    pub num_threads: usize,
    pub normalize_axes: bool,
}

pub struct BVorProgress {
    pub total: usize,
    pub completed: usize,
    pub current_status: String,
    pub log_messages: Vec<String>,
}

pub fn run_bvor_ensemble(
    config: BVorConfig,
    x: Vec<f64>,
    y: Vec<f64>,
    m: Vec<f64>,
    progress_callback: Arc<Mutex<BVorProgress>>,
    output_path: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut tasks = Vec::new();
    for (init, count) in &config.init_methods {
        for i in 0..*count {
            tasks.push((i, init.clone()));
        }
    }
    
    let total_tasks = tasks.len();
    {
        let mut p = progress_callback.lock().unwrap();
        p.total = total_tasks;
        p.completed = 0;
        let msg = format!("Starting {} simulations across Nnodes {} to {}", total_tasks, config.n_nodes_range.start(), config.n_nodes_range.end());
        p.current_status = msg.clone();
        p.log_messages.push(msg);
    }
    
    let x_min = x.iter().copied().fold(f64::INFINITY, f64::min);
    let x_max = x.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let y_min = y.iter().copied().fold(f64::INFINITY, f64::min);
    let y_max = y.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    
    let extent_x = if config.mode == "spatial" { 0.09 } else { 0.0 };
    let extent_y = if config.mode == "spatial" { 0.09 } else { 0.0 };
    
    // Original un-normalized grid bounds
    let x_min_ext = x_min - extent_x;
    let x_max_ext = x_max + extent_x;
    let y_min_ext = y_min - extent_y;
    let y_max_ext = y_max + extent_y;
    
    let x_grid = linspace(x_min_ext, x_max_ext, config.grid_res);
    let y_grid = linspace(y_min_ext, y_max_ext, config.grid_res);
    
    // Normalization logic
    let mut x_process = x.clone();
    let mut y_process = y.clone();
    
    let mut xy_eval = Vec::with_capacity(config.grid_res * config.grid_res);
    
    if config.normalize_axes {
        let x_range = (x_max - x_min).max(1e-10);
        let y_range = (y_max - y_min).max(1e-10);
        
        for v in &mut x_process { *v = (*v - x_min) / x_range; }
        for v in &mut y_process { *v = (*v - y_min) / y_range; }
        
        for &yg in &y_grid {
            for &xg in &x_grid {
                xy_eval.push([
                    (xg - x_min) / x_range,
                    (yg - y_min) / y_range
                ]);
            }
        }
    } else {
        for &yg in &y_grid {
            for &xg in &x_grid {
                xy_eval.push([xg, yg]);
            }
        }
    }
    

    
    let n_nodes: Vec<usize> = config.n_nodes_range.clone().collect();
    
    // Save results to a ZIP file containing NPZ
    let file = std::fs::File::create(output_path)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let zip_mutex = Arc::new(Mutex::new(zip));

    let pool = rayon::ThreadPoolBuilder::new().num_threads(config.num_threads).build().unwrap();
    pool.install(|| {
        tasks.par_iter().for_each(|(i, init)| {
            {
                let mut p = progress_callback.lock().unwrap();
                p.log_messages.push(format!("Started Init: {} (idx {})", init, i));
                if p.log_messages.len() > 100 {
                    p.log_messages.remove(0);
                }
            }
            let mut b_grid_all = Vec::new();
            let mut bic_all = Vec::new();
            
            let max_j = *n_nodes.iter().max().unwrap_or(&60);
            let mut b_vor_all = Vec::with_capacity(n_nodes.len() * max_j);
            let mut mu_vor_all = Vec::with_capacity(n_nodes.len() * max_j);
            let mut sig_vor_all = Vec::with_capacity(n_nodes.len() * max_j);
            let mut lnL_vor_all = Vec::with_capacity(n_nodes.len() * max_j);
            let mut N_vor_all = Vec::with_capacity(n_nodes.len() * max_j);
            let mut pnt_vor_all = Vec::with_capacity(n_nodes.len() * max_j * 2);
            
            
            for &j in &n_nodes {
                {
                    let mut p = progress_callback.lock().unwrap();
                    p.log_messages.push(format!("Init: {} (idx {}) - Computing Nnodes: {}", init, i, j));
                    if p.log_messages.len() > 100 {
                        p.log_messages.remove(0);
                    }
                }
            if let Some(res) = b_voronoi(&x_process, &y_process, &m, j, config.min_obs, init) {
                for idx in 0..max_j {
                    if idx < j {
                        b_vor_all.push(res.b_all[idx]);
                        mu_vor_all.push(res.mu_all[idx]);
                        sig_vor_all.push(res.sig_all[idx]);
                        lnL_vor_all.push(res.ln_l_all[idx]);
                        N_vor_all.push(res.n_all[idx]);
                        pnt_vor_all.push(res.voronoi_points[idx][0]);
                        pnt_vor_all.push(res.voronoi_points[idx][1]);
                    } else {
                        b_vor_all.push(f64::NAN);
                        mu_vor_all.push(f64::NAN);
                        sig_vor_all.push(f64::NAN);
                        lnL_vor_all.push(f64::NAN);
                        N_vor_all.push(f64::NAN);
                        pnt_vor_all.push(f64::NAN);
                        pnt_vor_all.push(f64::NAN);
                    }
                }
                let mut b_grid = Vec::with_capacity(xy_eval.len());
                for pt in &xy_eval {
                    let nearest = res.kdtree.nearest_one::<kiddo::SquaredEuclidean>(pt);
                    b_grid.push(res.b_all[nearest.item as usize]);
                }
                b_grid_all.extend(b_grid);
                bic_all.push(res.bic);
            } else {
                for _ in 0..max_j {
                    b_vor_all.push(f64::NAN);
                    mu_vor_all.push(f64::NAN);
                    sig_vor_all.push(f64::NAN);
                    lnL_vor_all.push(f64::NAN);
                    N_vor_all.push(f64::NAN);
                    pnt_vor_all.push(f64::NAN);
                    pnt_vor_all.push(f64::NAN);
                }
                
                b_grid_all.extend(vec![f64::NAN; xy_eval.len()]);
                bic_all.push(f64::NAN);
            }
        }
        
        let mut z = zip_mutex.lock().unwrap();
        // Write raw bytes
        let mut save_array = |name: &str, data: &[f64]| {
            let path = format!("{}/i_{:03}/{}", init, i, name);
            let _ = z.start_file(path, options);
            let bytes = unsafe {
                std::slice::from_raw_parts(
                    data.as_ptr() as *const u8,
                    data.len() * std::mem::size_of::<f64>(),
                )
            };
            let _ = z.write_all(bytes);
        };
        
        save_array("b_grid.raw", &b_grid_all);
        save_array("b_vor.raw", &b_vor_all);
        save_array("mu_vor.raw", &mu_vor_all);
        save_array("sig_vor.raw", &sig_vor_all);
        save_array("lnL_vor.raw", &lnL_vor_all);
        save_array("N_vor.raw", &N_vor_all);
        save_array("pnt_vor.raw", &pnt_vor_all);
        save_array("bic.raw", &bic_all);
        
        let mut p = progress_callback.lock().unwrap();
        p.completed += 1;
        let completed = p.completed;
        let total = p.total;
        p.current_status = format!("Completed {} / {} - Init: {}", completed, total, init);
        p.log_messages.push(format!("Finished Init: {} (idx {}) [{}/{}]", init, i, completed, total));
        if p.log_messages.len() > 100 {
            p.log_messages.remove(0);
        }
    });
});

    let mut z = match Arc::try_unwrap(zip_mutex) {
        Ok(mutex) => mutex.into_inner().unwrap(),
        Err(_) => panic!("Arc still has multiple owners"),
    };
    
    // Save metadata
    let metadata = serde_json::json!({
        "x_min": x_min_ext,
        "x_max": x_max_ext,
        "y_min": y_min_ext,
        "y_max": y_max_ext,
        "grid_res": config.grid_res,
        "min_obs": config.min_obs,
        "n_nodes_range": [config.n_nodes_range.start(), config.n_nodes_range.end()],
        "mode": config.mode,
        "normalized": config.normalize_axes,
    });
    
    let _ = z.start_file("metadata.json", options);
    let _ = z.write_all(metadata.to_string().as_bytes());
    
    // Save m.raw
    let _ = z.start_file("m.raw", options);
    let bytes_m = unsafe {
        std::slice::from_raw_parts(
            m.as_ptr() as *const u8,
            m.len() * std::mem::size_of::<f64>(),
        )
    };
    let _ = z.write_all(bytes_m);
    
    // Save x.raw
    let _ = z.start_file("x.raw", options);
    let bytes_x = unsafe {
        std::slice::from_raw_parts(
            x.as_ptr() as *const u8,
            x.len() * std::mem::size_of::<f64>(),
        )
    };
    let _ = z.write_all(bytes_x);
    
    // Save y.raw
    let _ = z.start_file("y.raw", options);
    let bytes_y = unsafe {
        std::slice::from_raw_parts(
            y.as_ptr() as *const u8,
            y.len() * std::mem::size_of::<f64>(),
        )
    };
    let _ = z.write_all(bytes_y);
    
    z.finish()?;
    
    let mut p = progress_callback.lock().unwrap();
    p.current_status = "All complete. File saved as bvalue.npz".to_string();
    
    Ok(())
}

fn linspace(start: f64, end: f64, steps: usize) -> Vec<f64> {
    if steps < 2 {
        return vec![start];
    }
    let step = (end - start) / (steps - 1) as f64;
    (0..steps).map(|i| start + i as f64 * step).collect()
}
