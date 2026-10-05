use chrono::NaiveDateTime;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct FdsnStation {
    pub network: String,
    pub station: String,
    pub lat: f64,
    pub lon: f64,
    pub elevation: f64,
    pub site_name: String,
    pub provider_name: String,
    pub provider_url: String,
}

#[derive(Debug, Clone)]
pub struct FdsnSearchParams {
    pub name: String,
    pub url: String,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub min_radius: Option<f64>,
    pub max_radius: Option<f64>,
    pub min_lat: Option<f64>,
    pub max_lat: Option<f64>,
    pub min_lon: Option<f64>,
    pub max_lon: Option<f64>,
    pub min_mag: Option<f64>,
    pub max_mag: Option<f64>,
    pub include_arrivals: bool,
    pub start_time: NaiveDateTime,
    pub end_time: NaiveDateTime,
    pub channel: String,
    pub network: Option<String>,
    pub station: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FdsnDownloadParams {
    pub provider_name: String,
    pub url: String,
    pub network: String,
    pub station: String,
    pub channel: String,
    pub start_time: NaiveDateTime,
    pub end_time: NaiveDateTime,
    pub output_dir: PathBuf,
    pub export_sac: bool,
}

pub enum FdsnResult {
    StationsFound(Vec<FdsnStation>),
    WaveformDownloaded(String, String, String), // Network, Station, filepath
    ResponseDownloaded(String, String, String), // Network, Station, filepath
    EventsDownloaded(String),
    Progress(String),
    Error(String),
    WaveformDownloadsComplete,
    ResponseDownloadsComplete,
}
