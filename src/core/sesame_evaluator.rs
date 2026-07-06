#[derive(Clone, Debug)]
pub struct SesameResult {
    pub reliability_c1: bool,
    pub reliability_c2: bool,
    pub reliability_c3: bool,
    
    pub clear_peak_c1: bool,
    pub clear_peak_c2: bool,
    pub clear_peak_c3: bool,
    pub clear_peak_c4: bool,
    pub clear_peak_c5: bool,
    pub clear_peak_c6: bool,
    
    pub is_reliable: bool,
    pub is_clear_peak: bool,
}

pub fn epsilon(f0: f64) -> f64 {
    if f0 < 0.2 { 0.25 * f0 }
    else if f0 < 0.5 { 0.20 * f0 }
    else if f0 < 1.0 { 0.15 * f0 }
    else if f0 < 2.0 { 0.10 * f0 }
    else { 0.05 * f0 }
}

pub fn theta(f0: f64) -> f64 {
    if f0 < 0.2 { 3.0 }
    else if f0 < 0.5 { 2.5 }
    else if f0 < 1.0 { 2.0 }
    else if f0 < 2.0 { 1.78 }
    else { 1.58 }
}

pub fn evaluate_sesame(
    f0: f64,
    a0: f64,
    lw: f64,
    nw: usize,
    freq: &[f64],
    mean_hvsr: &[f64],
    sigma_a: &[f64],
    sigma_f: f64,
) -> SesameResult {
    // RELIABILITY
    let reliability_c1 = f0 > 10.0 / lw;
    
    let nc = lw * (nw as f64) * f0;
    let reliability_c2 = nc > 200.0;
    
    let mut reliability_c3 = true;
    let limit = if f0 > 0.5 { 2.0 } else { 3.0 };
    for i in 0..freq.len() {
        let f = freq[i];
        if f >= 0.5 * f0 && f <= 2.0 * f0 {
            if sigma_a[i] >= limit {
                reliability_c3 = false;
                break;
            }
        }
    }
    
    let is_reliable = reliability_c1 && reliability_c2 && reliability_c3;
    
    // CLEAR PEAK
    let mut clear_peak_c1 = false;
    for i in 0..freq.len() {
        let f = freq[i];
        if f >= f0 / 4.0 && f <= f0 {
            if mean_hvsr[i] < a0 / 2.0 {
                clear_peak_c1 = true;
                break;
            }
        }
    }
    
    let mut clear_peak_c2 = false;
    for i in 0..freq.len() {
        let f = freq[i];
        if f >= f0 && f <= 4.0 * f0 {
            if mean_hvsr[i] < a0 / 2.0 {
                clear_peak_c2 = true;
                break;
            }
        }
    }
    
    let clear_peak_c3 = a0 > 2.0;
    
    let mut max_f_plus = 0.0;
    let mut max_amp_plus = -1.0;
    let mut max_f_minus = 0.0;
    let mut max_amp_minus = -1.0;
    
    for i in 0..freq.len() {
        let val_plus = mean_hvsr[i] * sigma_a[i];
        let val_minus = mean_hvsr[i] / sigma_a[i];
        
        if val_plus > max_amp_plus {
            max_amp_plus = val_plus;
            max_f_plus = freq[i];
        }
        if val_minus > max_amp_minus {
            max_amp_minus = val_minus;
            max_f_minus = freq[i];
        }
    }
    
    let margin = 0.05 * f0;
    let clear_peak_c4 = (max_f_plus >= f0 - margin && max_f_plus <= f0 + margin) &&
                        (max_f_minus >= f0 - margin && max_f_minus <= f0 + margin);
                        
    let clear_peak_c5 = sigma_f < epsilon(f0);
    
    let mut clear_peak_c6 = false;
    let mut min_diff = std::f64::MAX;
    let mut f0_idx = 0;
    for i in 0..freq.len() {
        let diff = (freq[i] - f0).abs();
        if diff < min_diff {
            min_diff = diff;
            f0_idx = i;
        }
    }
    if f0_idx < freq.len() {
        clear_peak_c6 = sigma_a[f0_idx] < theta(f0);
    }
    
    let cp_count = [
        clear_peak_c1, clear_peak_c2, clear_peak_c3,
        clear_peak_c4, clear_peak_c5, clear_peak_c6
    ].iter().filter(|&&x| x).count();
    
    let is_clear_peak = cp_count >= 5;
    
    SesameResult {
        reliability_c1,
        reliability_c2,
        reliability_c3,
        clear_peak_c1,
        clear_peak_c2,
        clear_peak_c3,
        clear_peak_c4,
        clear_peak_c5,
        clear_peak_c6,
        is_reliable,
        is_clear_peak,
    }
}
