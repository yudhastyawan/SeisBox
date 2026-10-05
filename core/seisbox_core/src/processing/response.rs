use std::f64::consts::PI;
use rustfft::{FftPlanner, num_complex::Complex64};

use crate::core::seismogram::Seismogram;

#[derive(Debug, Clone)]
pub struct PolesZeros {
    pub poles: Vec<Complex64>,
    pub zeros: Vec<Complex64>,
    pub normalization_factor: f64,
    pub normalization_freq: f64, // in Hz
    pub sensitivity: f64,
}

impl PolesZeros {
    /// Compute the complex response at a given frequency in Hz
    pub fn evaluate_at(&self, freq_hz: f64) -> Complex64 {
        let s = Complex64::new(0.0, 2.0 * PI * freq_hz);
        
        let mut num = Complex64::new(1.0, 0.0);
        for z in &self.zeros {
            num *= s - *z;
        }
        
        let mut den = Complex64::new(1.0, 0.0);
        for p in &self.poles {
            den *= s - *p;
        }
        
        // Total gain = A0 (normalization_factor) * Sensitivity
        let total_gain = self.normalization_factor * self.sensitivity;
        
        (num / den) * total_gain
    }
}

/// Apply Instrument Response Removal using Poles and Zeros with a water-level stabilization
pub fn remove_response(
    seis: &Seismogram,
    pz: &PolesZeros,
    water_level: f64,
) -> Result<Seismogram, String> {
    let n = seis.amplitude.len();
    if n == 0 {
        return Err("Seismogram is empty".into());
    }

    let dt = if seis.sample_rate > 0.0 { 1.0 / seis.sample_rate } else { 0.0 };
    if dt == 0.0 {
        return Err("Invalid sample rate".into());
    }

    // Next power of two or just the length
    // For simplicity, we use the exact length (rustfft handles non-power-of-two)
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(n);
    let ifft = planner.plan_fft_inverse(n);

    // Convert amplitude to Complex64
    let mut buffer: Vec<Complex64> = seis
        .amplitude
        .iter()
        .map(|&val| Complex64::new(val, 0.0))
        .collect();

    // Forward FFT
    fft.process(&mut buffer);

    // Compute frequencies and instrument response
    let df = 1.0 / (n as f64 * dt);
    
    // Evaluate response at all frequencies to find max amplitude for water level
    let mut responses: Vec<Complex64> = vec![Complex64::new(0.0, 0.0); n];
    let mut max_amp = 0.0f64;
    
    for i in 0..n {
        let freq = if i <= n / 2 {
            i as f64 * df
        } else {
            (i as f64 - n as f64) * df
        };
        
        let resp = pz.evaluate_at(freq);
        responses[i] = resp;
        
        let amp = resp.norm();
        if amp > max_amp {
            max_amp = amp;
        }
    }

    let water_level_amp = max_amp * water_level;

    // Deconvolve in frequency domain
    for i in 0..n {
        let mut resp = responses[i];
        
        if resp.norm() < water_level_amp {
            // Apply water level stabilization: scale magnitude but keep phase
            let phase = resp.arg();
            resp = Complex64::from_polar(water_level_amp, phase);
        }
        
        buffer[i] /= resp;
    }

    // Inverse FFT
    ifft.process(&mut buffer);

    // Convert back to real and normalize by N (since rustfft doesn't normalize on inverse)
    let inv_n = 1.0 / n as f64;
    let mut new_amplitude = Vec::with_capacity(n);
    let mut sum = 0.0;
    
    for c in buffer {
        let val = c.re * inv_n;
        new_amplitude.push(val);
        sum += val;
    }
    
    let mean = sum / n as f64;

    Ok(Seismogram {
        filename: seis.filename.clone(),
        network: seis.network.clone(),
        station: seis.station.clone(),
        location: seis.location.clone(),
        channel: seis.channel.clone(),
        start_time_str: seis.start_time_str.clone(),
        end_time_str: seis.end_time_str.clone(),
        time: seis.time.clone(),
        amplitude: new_amplitude,
        sample_rate: seis.sample_rate,
        mean,
    })
}
