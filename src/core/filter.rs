use sci_rs::signal::filter::design::{
    butter_dyn, FilterBandType, FilterOutputType, DigitalFilter, Sos
};
use sci_rs::signal::filter::sosfiltfilt_dyn;

/// Applies a zero-phase Butterworth bandpass filter using `sci-rs`.
pub fn apply_bandpass(
    data: &[f64],
    sample_rate: f64,
    low_cut: f64,
    high_cut: f64,
    order: usize,
) -> Vec<f64> {
    let nyquist = sample_rate / 2.0;
    
    // Validate frequency bounds
    let mut lc = low_cut;
    let mut hc = high_cut;
    if lc <= 0.0 { lc = 0.001; }
    if hc >= nyquist { hc = nyquist - 0.001; }
    
    // Generate Second Order Sections (SOS) for Butterworth filter
    let filter = butter_dyn::<f64>(
        order,
        vec![lc, hc],
        Some(FilterBandType::Bandpass),
        Some(false), // not analog
        Some(FilterOutputType::Sos),
        Some(sample_rate),
    );
    
    // filter returns Sos, Ba, or Zpk. We requested Sos.
    let sos: Vec<Sos<f64>> = match filter {
        DigitalFilter::Sos(s) => s.sos,
        _ => panic!("Expected SOS output"),
    };
    
    // Apply zero-phase forward-backward filter
    sosfiltfilt_dyn(data.iter().copied(), &sos)
}
