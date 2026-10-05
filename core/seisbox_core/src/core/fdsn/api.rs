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
    pub lat: f64,
    pub lon: f64,
    pub min_radius: f64,
    pub max_radius: f64,
    pub start_time: NaiveDateTime,
    pub end_time: NaiveDateTime,
    pub channel: String,
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
}

pub enum FdsnResult {
    StationsFound(Vec<FdsnStation>),
    WaveformDownloaded(String, String, String), // Network, Station, filepath
    ResponseDownloaded(String, String, String), // Network, Station, filepath
    Progress(String),
    Error(String),
    WaveformDownloadsComplete,
    ResponseDownloadsComplete,
}
