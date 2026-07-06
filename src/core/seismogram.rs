/// Seismogram data container.
///
/// Holds time-series waveform data. In production this would be populated
/// from SAC or miniSEED binary files; here we use mock data generation.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct Seismogram {
    /// Display name (typically the filename stem).
    pub filename: String,
    pub network: String,
    pub station: String,
    pub location: String,
    pub channel: String,
    pub start_time_str: String,
    pub end_time_str: String,
    
    /// Time axis in seconds from trace start.
    pub time: Vec<f64>,
    /// Amplitude values (normalised).
    pub amplitude: Vec<f64>,
    /// Sample rate in Hz.
    pub sample_rate: f64,
    /// Mean of the amplitude array.
    pub mean: f64,
}
