use crate::core::rjmcmc::RjmcmcSample;
use std::fs::File;
use std::io::{BufRead, BufReader};

#[derive(Clone, Debug)]
pub struct VisualizerData {
    pub freq: Vec<f64>,
    pub h_obs: Vec<f64>,
    pub h_err: Vec<f64>,
    
    pub samples: Vec<RjmcmcSample>,
    pub sorted_rmse: Vec<f64>,
    pub best_sample: Option<RjmcmcSample>,
    
    pub vs30_mean: f64,
    pub vs30_min: f64,
    pub vs30_max: f64,
    
    pub h800_mean: f64,
    pub h800_min: f64,
    pub h800_max: f64,
    
    pub z1_mean: f64,
    pub z1_min: f64,
    pub z1_max: f64,
    
    pub z2_5_mean: f64,
    pub z2_5_min: f64,
    pub z2_5_max: f64,
    
    pub z_nodes: Vec<f64>,
    pub vs_p05: Vec<f64>,
    pub vs_p50: Vec<f64>,
    pub vs_p95: Vec<f64>,
    
    pub max_z: f64,
}

impl Default for VisualizerData {
    fn default() -> Self {
        Self {
            freq: Vec::new(), h_obs: Vec::new(), h_err: Vec::new(),
            samples: Vec::new(), sorted_rmse: Vec::new(), best_sample: None,
            vs30_mean: f64::NAN, vs30_min: f64::NAN, vs30_max: f64::NAN,
            h800_mean: f64::NAN, h800_min: f64::NAN, h800_max: f64::NAN,
            z1_mean: f64::NAN, z1_min: f64::NAN, z1_max: f64::NAN,
            z2_5_mean: f64::NAN, z2_5_min: f64::NAN, z2_5_max: f64::NAN,
            z_nodes: Vec::new(), vs_p05: Vec::new(), vs_p50: Vec::new(), vs_p95: Vec::new(),
            max_z: 0.0,
        }
    }
}

pub fn calc_vs30(z: &[f64], vs: &[f64]) -> f64 {
    let mut z_sub = Vec::new();
    let mut vs_sub = Vec::new();
    
    for i in 0..z.len() {
        if z[i] <= 30.0 {
            z_sub.push(z[i]);
            vs_sub.push(vs[i]);
        }
    }
    
    if z_sub.is_empty() || *z_sub.last().unwrap() < 30.0 {
        let vs_at_30 = if z.iter().any(|&d| d >= 30.0) {
            let mut val = vs.last().copied().unwrap_or(f64::NAN);
            for i in 1..z.len() {
                if z[i-1] < 30.0 && z[i] >= 30.0 {
                    val = vs[i-1];
                    break;
                }
            }
            val
        } else {
            vs.last().copied().unwrap_or(f64::NAN)
        };
        z_sub.push(30.0);
        vs_sub.push(vs_at_30);
    }
    
    let mut time_sum = 0.0;
    for i in 1..z_sub.len() {
        let dz = z_sub[i] - z_sub[i-1];
        time_sum += dz / vs_sub[i-1];
    }
    
    if time_sum > 0.0 { 30.0 / time_sum } else { f64::NAN }
}

pub fn calc_depth_for_vs(z: &[f64], vs: &[f64], target_vs: f64) -> f64 {
    let max_v = vs.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
    if max_v < target_vs {
        return f64::NAN;
    }
    
    for i in 0..vs.len() {
        if vs[i] >= target_vs {
            return z[i]; // Return depth where Vs is first >= target
        }
    }
    f64::NAN
}

fn stats(arr: &[f64]) -> (f64, f64, f64) {
    let valid: Vec<f64> = arr.iter().copied().filter(|&x| !x.is_nan()).collect();
    if valid.is_empty() {
        (f64::NAN, f64::NAN, f64::NAN)
    } else {
        let mean = valid.iter().sum::<f64>() / valid.len() as f64;
        let min = valid.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max = valid.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        (mean, min, max)
    }
}

pub fn load_and_process_data(jsonl_file: &str, obs_file: &str) -> Result<VisualizerData, String> {
    // 1. Read Observation Data
    let mut freq = Vec::new();
    let mut h_obs = Vec::new();
    let mut h_err = Vec::new();
    
    if let Ok(mut rdr) = csv::Reader::from_path(obs_file) {
        for result in rdr.records() {
            if let Ok(record) = result {
                if let (Ok(f), Ok(a)) = (record[0].parse::<f64>(), record[1].parse::<f64>()) {
                    freq.push(f);
                    h_obs.push(a);
                    
                    let err = if record.len() > 2 {
                        record[2].parse::<f64>().unwrap_or(0.0)
                    } else {
                        0.0
                    };
                    h_err.push(err);
                }
            }
        }
    } else {
        return Err("Failed to read observation CSV file.".into());
    }
    
    if freq.is_empty() {
        return Err("Observation file is empty.".into());
    }
    
    // 2. Read JSONL samples
    let file = File::open(jsonl_file).map_err(|e| format!("Error opening jsonl: {}", e))?;
    let reader = BufReader::new(file);
    
    let mut all_samples: Vec<RjmcmcSample> = Vec::new();
    for line_result in reader.lines() {
        if let Ok(line) = line_result {
            if let Ok(s) = serde_json::from_str::<RjmcmcSample>(&line) {
                all_samples.push(s);
            }
        }
    }
    
    if all_samples.is_empty() {
        return Err("No valid samples found in JSONL file.".into());
    }
    
    // 3. Filter Best Samples (rmse <= min + 0.2, max 2000)
    let min_rmse = all_samples.iter().fold(f64::INFINITY, |a, s| a.min(s.rmse));
    let mut filtered_samples: Vec<_> = all_samples.into_iter().filter(|s| s.rmse <= min_rmse + 0.2).collect();
    
    if filtered_samples.is_empty() {
        return Err("No samples passed RMSE filter.".into());
    }
    
    filtered_samples.sort_by(|a, b| a.rmse.partial_cmp(&b.rmse).unwrap());
    
    if filtered_samples.len() > 2000 {
        filtered_samples.truncate(2000); // keep best 2000
    }
    
    // Reverse to match python code if needed, but sorted ascending is fine.
    // Python sorts descending then takes [-1] for best. We sort ascending, so index 0 is best.
    let best_sample = filtered_samples[0].clone();
    let sorted_rmse: Vec<f64> = filtered_samples.iter().map(|s| s.rmse).collect();
    
    // 4. Calculate Geotechnical parameters (Vs30, H800, Z1.0, Z2.5)
    let mut vs30_list = Vec::new();
    let mut h800_list = Vec::new();
    let mut z1_list = Vec::new();
    let mut z2_5_list = Vec::new();
    
    let mut z_all_nodes = Vec::new();
    
    for m in &filtered_samples {
        let mut z = vec![0.0];
        let mut sum_h = 0.0;
        for h in &m.h {
            sum_h += h;
            z.push(sum_h);
        }
        z.push(sum_h + 10.0); // extend for halfspace
        
        let mut vs = m.vs.clone();
        if let Some(last_vs) = vs.last().copied() {
            vs.push(last_vs); // copy for halfspace extended node
        } else {
            vs.push(1000.0);
        }
        
        vs30_list.push(calc_vs30(&z, &vs));
        h800_list.push(calc_depth_for_vs(&z, &vs, 800.0));
        z1_list.push(calc_depth_for_vs(&z, &vs, 1000.0));
        z2_5_list.push(calc_depth_for_vs(&z, &vs, 2500.0));
        
        for &zz in &z {
            z_all_nodes.push(zz);
        }
    }
    
    let (vs30_mean, vs30_min, vs30_max) = stats(&vs30_list);
    let (h800_mean, h800_min, h800_max) = stats(&h800_list);
    let (z1_mean, z1_min, z1_max) = stats(&z1_list);
    let (z2_5_mean, z2_5_min, z2_5_max) = stats(&z2_5_list);
    
    // 5. Percentile calculation
    z_all_nodes.sort_by(|a, b| a.partial_cmp(b).unwrap());
    z_all_nodes.dedup();
    
    let max_z = z_all_nodes.last().copied().unwrap_or(100.0);
    
    // We need vs_matrix: [len(samples), len(z_nodes)]
    let n_nodes = z_all_nodes.len();
    let mut vs_matrix = vec![vec![0.0; n_nodes]; filtered_samples.len()];
    
    for (i, m) in filtered_samples.iter().enumerate() {
        let mut z = vec![0.0];
        let mut sum_h = 0.0;
        for h in &m.h {
            sum_h += h;
            z.push(sum_h);
        }
        z.push(sum_h + 10.0);
        
        let mut vs = m.vs.clone();
        if let Some(last_vs) = vs.last().copied() {
            vs.push(last_vs);
        } else {
            vs.push(1000.0);
        }
        
        for (j, &zz) in z_all_nodes.iter().enumerate() {
            // find right-side insertion index using binary search
            let idx = match z.binary_search_by(|v| v.partial_cmp(&zz).unwrap()) {
                Ok(index) => index,
                Err(index) => index.saturating_sub(1),
            };
            let idx = idx.min(vs.len().saturating_sub(1));
            vs_matrix[i][j] = vs[idx];
        }
    }
    
    let mut vs_p05 = Vec::with_capacity(n_nodes);
    let mut vs_p50 = Vec::with_capacity(n_nodes);
    let mut vs_p95 = Vec::with_capacity(n_nodes);
    
    for j in 0..n_nodes {
        let mut col: Vec<f64> = vs_matrix.iter().map(|row| row[j]).collect();
        col.sort_by(|a, b| a.partial_cmp(b).unwrap());
        
        let i05 = ((col.len() as f64) * 0.05) as usize;
        let i50 = ((col.len() as f64) * 0.50) as usize;
        let i95 = ((col.len() as f64) * 0.95) as usize;
        
        let i05 = i05.clamp(0, col.len() - 1);
        let i50 = i50.clamp(0, col.len() - 1);
        let i95 = i95.clamp(0, col.len() - 1);
        
        vs_p05.push(col[i05]);
        vs_p50.push(col[i50]);
        vs_p95.push(col[i95]);
    }
    
    Ok(VisualizerData {
        freq, h_obs, h_err,
        samples: filtered_samples,
        sorted_rmse,
        best_sample: Some(best_sample),
        vs30_mean, vs30_min, vs30_max,
        h800_mean, h800_min, h800_max,
        z1_mean, z1_min, z1_max,
        z2_5_mean, z2_5_min, z2_5_max,
        z_nodes: z_all_nodes,
        vs_p05, vs_p50, vs_p95,
        max_z,
    })
}
