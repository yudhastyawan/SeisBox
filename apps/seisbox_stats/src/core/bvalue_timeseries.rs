use crate::core::catalogue::Catalogue;
use crate::core::gutenberg_richter::{mc_maxc, mc_gft, mc_emr, bvalue_mle, BvalueResult};
use chrono::NaiveDateTime;

#[derive(Debug, Clone)]
pub struct TimeseriesNode {
    pub window_start_time: NaiveDateTime,
    pub window_end_time: NaiveDateTime,
    pub window_center_time: NaiveDateTime,
    pub n_events: usize,
    pub mc: f64,
    pub b_value: f64,
    pub b_unc: f64,
    pub a_value: f64,
}

pub fn run_bvalue_timeseries(
    cat: &Catalogue,
    window_size: usize,
    step_size: usize,
    mc_opt: Option<f64>,
    mc_method_str: &str,
) -> Vec<TimeseriesNode> {
    
    let n = cat.events.len();
    if n < window_size {
        return Vec::new();
    }
    
    let mut results = Vec::new();
    let mut i = 0;
    
    while i + window_size <= n {
        let window_events = &cat.events[i..(i + window_size)];
        let start_time = window_events.first().unwrap().time;
        let end_time = window_events.last().unwrap().time;
        let center_time = start_time + (end_time - start_time) / 2;
        
        let mags: Vec<f64> = window_events.iter().map(|e| e.mag).collect();
        
        let mc = if let Some(m) = mc_opt {
            m
        } else {
            match mc_method_str {
                "maxc" => mc_maxc(&mags, 0.1),
                "gft" => mc_gft(&mags, 0.1, 0.90),
                "emr" => mc_emr(&mags, 0.1),
                _ => mc_maxc(&mags, 0.1),
            }
        };
        
        let b_res = bvalue_mle(&mags, mc, 0.1);
        
        results.push(TimeseriesNode {
            window_start_time: start_time,
            window_end_time: end_time,
            window_center_time: center_time,
            n_events: window_size,
            mc,
            b_value: b_res.b,
            b_unc: b_res.b_uncertainty,
            a_value: b_res.a,
        });
        
        i += step_size;
    }
    
    results
}
