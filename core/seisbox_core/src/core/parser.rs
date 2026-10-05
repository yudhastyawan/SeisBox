use std::path::Path;
use std::fs;
use chrono::{NaiveDateTime, Duration};

use crate::core::seismogram::Seismogram;

/// Parse a seismic file (.sac or .mseed) and return one or more Seismograms.
pub fn parse_seismic_file(path: &Path) -> Result<Vec<Seismogram>, String> {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    match extension.as_str() {
        "sac" => parse_sac(path).map(|s| vec![s]),
        "mseed" | "m" | "miniseed" => parse_miniseed(path),
        _ => Err(format!("Unsupported file extension: .{}", extension)),
    }
}

/// Parse a SAC file using the `sacio` crate.
fn parse_sac(path: &Path) -> Result<Seismogram, String> {
    let filename = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    // The `sacio` crate uses `Sac::from_file` which returns a Result.
    let sac = sacio::Sac::from_file(path).map_err(|e| format!("Failed to read SAC file: {:?}", e))?;

    let network = String::new();
    let station = String::new();
    let location = String::new();
    let channel = String::new();
    
    // Construct ISO8601-like UTC string for SAC
    let start_time_str = String::new();
    let end_time_str = String::new();

    let start_time = sac.b() as f64;
    let delta = sac.delta() as f64;
    let sample_rate = if delta > 0.0 { 1.0 / delta } else { 0.0 };

    let y = sac.y;
    if y.is_empty() {
        return Err("SAC file contains no data (empty y array)".to_string());
    }

    let mut time = Vec::with_capacity(y.len());
    let mut amplitude = Vec::with_capacity(y.len());

    let mut sum_amp = 0.0;
    let mut valid_count = 0;
    for (i, &amp) in y.iter().enumerate() {
        let a = amp as f64;
        time.push(start_time + (i as f64) * delta);
        amplitude.push(a);
        if a.is_finite() {
            sum_amp += a;
            valid_count += 1;
        }
    }
    
    let mean = if valid_count == 0 { 0.0 } else { sum_amp / valid_count as f64 };

    Ok(Seismogram {
        filename,
        network: network.trim().to_string(),
        station: station.trim().to_string(),
        location: location.trim().to_string(),
        channel: channel.trim().to_string(),
        start_time_str,
        end_time_str,
        time,
        amplitude,
        sample_rate,
        mean,
    })
}

/// Parse a miniSEED file using the `miniseed-rs` crate.
fn parse_miniseed(path: &Path) -> Result<Vec<Seismogram>, String> {
    let filename = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let bytes = fs::read(path).map_err(|e| format!("Failed to read file bytes: {}", e))?;

    use miniseed_rs::{MseedReader, Samples};
    use std::collections::BTreeMap;

    // MseedReader iterates through all records in the file
    let records = MseedReader::new(&bytes)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Failed to parse miniSEED records: {:?}", e))?;

    if records.is_empty() {
        return Err("miniSEED file contains no valid records".to_string());
    }

    // Group records by NSLC (Network, Station, Location, Channel)
    let mut groups: BTreeMap<String, Vec<miniseed_rs::MseedRecord>> = BTreeMap::new();
    for rec in records {
        // Build an identifier for this channel (trimming to handle fixed-width padding)
        let key = format!(
            "{}.{}.{}.{}",
            rec.network.trim(),
            rec.station.trim(),
            rec.location.trim(),
            rec.channel.trim()
        );
        groups.entry(key).or_default().push(rec);
    }

    let mut seismograms = Vec::new();

    let num_groups = groups.len();

    for (key, recs) in groups {
        if recs.is_empty() {
            continue;
        }

        let network = recs[0].network.trim().to_string();
        let station = recs[0].station.trim().to_string();
        let location = recs[0].location.trim().to_string();
        let channel = recs[0].channel.trim().to_string();
        
        let raw_start = recs[0].start_time.to_string();
        let (start_time_str, start_dt) = if let Ok(dt) = NaiveDateTime::parse_from_str(&raw_start, "%Y-%j %H:%M:%S%.f") {
            (dt.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(), Some(dt))
        } else {
            (raw_start, None)
        };

        let sample_rate = recs[0].sample_rate;
        if sample_rate <= 0.0 {
            // Skip invalid sample rates
            continue;
        }
        let delta = 1.0 / (sample_rate as f64);

        // For simplicity, we assume the first record's start time is t=0 for all channels
        let mut current_time = 0.0;

        let mut time = Vec::new();
        let mut amplitude = Vec::new();

        for rec in recs {
            let n_samples = rec.samples.len();
            time.reserve(n_samples);
            amplitude.reserve(n_samples);

            match rec.samples {
                Samples::Int(vals) => {
                    for v in vals {
                        time.push(current_time);
                        amplitude.push(v as f64);
                        current_time += delta;
                    }
                }
                Samples::Float(vals) => {
                    for v in vals {
                        time.push(current_time);
                        amplitude.push(v as f64);
                        current_time += delta;
                    }
                }
                Samples::Double(vals) => {
                    for v in vals {
                        time.push(current_time);
                        amplitude.push(v);
                        current_time += delta;
                    }
                }
            }
        }

        let mut valid_count = 0;
        let mut sum = 0.0;
        for &a in &amplitude {
            if a.is_finite() {
                sum += a;
                valid_count += 1;
            }
        }
        let mean = if valid_count == 0 { 0.0 } else { sum / valid_count as f64 };

        // Give each trace a unique filename to distinguish them in the UI
        let trace_filename = if num_groups > 1 {
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or(&filename);
            let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
            if ext.is_empty() {
                format!("{}_{}", stem, key)
            } else {
                format!("{}_{}.{}", stem, key, ext)
            }
        } else {
            filename.clone()
        };

        let end_time_str = if let Some(dt) = start_dt {
            let duration_ms = (time.last().copied().unwrap_or(0.0) * 1000.0) as i64;
            let end_dt = dt + Duration::try_milliseconds(duration_ms).unwrap_or_default();
            end_dt.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
        } else {
            String::new()
        };

        seismograms.push(Seismogram {
            filename: trace_filename,
            network,
            station,
            location,
            channel,
            start_time_str,
            end_time_str,
            time,
            amplitude,
            sample_rate,
            mean,
        });
    }

    if seismograms.is_empty() {
        Err("Could not extract any valid traces from miniSEED file".to_string())
    } else {
        Ok(seismograms)
    }
}
