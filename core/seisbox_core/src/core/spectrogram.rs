use eframe::egui;
use rustfft::{FftPlanner, num_complex::Complex};

const WINDOW_SIZE: usize = 256;
const HOP_SIZE: usize = 128;

pub struct SpectrogramData {
    pub pixels: Vec<egui::Color32>,
    pub width: usize,
    pub height: usize,
    pub max_freq: f64,
}

pub fn compute_spectrogram(amplitudes: &[f64], sample_rate: f64) -> Option<SpectrogramData> {
    if amplitudes.len() < WINDOW_SIZE {
        return None;
    }

    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(WINDOW_SIZE);

    // Hanning window
    let window: Vec<f64> = (0..WINDOW_SIZE)
        .map(|i| 0.5 * (1.0 - (2.0 * std::f64::consts::PI * i as f64 / (WINDOW_SIZE - 1) as f64).cos()))
        .collect();

    let num_bins = WINDOW_SIZE / 2; // Nyquist
    
    // Calculate required hop size to limit texture width to 2048 pixels
    // (Prevents GPU texture allocation freezes for very long traces)
    let max_frames = 2048;
    let mut hop_size = HOP_SIZE;
    let mut num_frames = (amplitudes.len() - WINDOW_SIZE) / hop_size + 1;
    
    if num_frames > max_frames {
        hop_size = (amplitudes.len() - WINDOW_SIZE) / max_frames;
        if hop_size == 0 { hop_size = 1; }
        num_frames = (amplitudes.len() - WINDOW_SIZE) / hop_size + 1;
    }
    
    let mut power_spectrum = vec![0.0; num_frames * num_bins];
    let mut max_power = f64::MIN;
    let mut min_power = f64::MAX;

    let mut buffer = vec![Complex { re: 0.0, im: 0.0 }; WINDOW_SIZE];

    for frame in 0..num_frames {
        let start = frame * hop_size;
        for i in 0..WINDOW_SIZE {
            buffer[i] = Complex {
                re: amplitudes[start + i] * window[i],
                im: 0.0,
            };
        }
        
        fft.process(&mut buffer);

        for bin in 0..num_bins {
            let magnitude = buffer[bin].norm();
            let power = 20.0 * (magnitude.max(1e-10)).log10(); // dB
            
            if power > max_power { max_power = power; }
            if power < min_power { min_power = power; }
            
            // To render normally, lower frequencies are at the bottom.
            // In egui image, y=0 is top. So we invert the y-axis here.
            let y = num_bins - 1 - bin;
            power_spectrum[y * num_frames + frame] = power;
        }
    }

    // Normalize and map to color
    let range = max_power - min_power;
    let range = if range < 1e-6 { 1.0 } else { range };

    let mut pixels = Vec::with_capacity(num_frames * num_bins);
    for power in power_spectrum {
        let norm = ((power - min_power) / range).clamp(0.0, 1.0);
        pixels.push(heat_map(norm));
    }

    Some(SpectrogramData {
        pixels,
        width: num_frames,
        height: num_bins,
        max_freq: sample_rate / 2.0,
    })
}

fn heat_map(t: f64) -> egui::Color32 {
    let t = t.clamp(0.0, 1.0);
    let (r, g, b) = if t < 0.25 {
        let f = t / 0.25;
        (0.0, 0.0, 0.5 + 0.5 * f) // Dark blue to blue
    } else if t < 0.5 {
        let f = (t - 0.25) / 0.25;
        (0.0, f, 1.0 - f) // Blue to Green
    } else if t < 0.75 {
        let f = (t - 0.5) / 0.25;
        (f, 1.0, 0.0) // Green to Yellow
    } else {
        let f = (t - 0.75) / 0.25;
        (1.0, 1.0 - f, 0.0) // Yellow to Red
    };
    egui::Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}
