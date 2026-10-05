use super::api::{FdsnResult, FdsnSearchParams, FdsnDownloadParams, FdsnStation};
use std::sync::mpsc::Sender;
use std::fs;

pub fn search_stations(params_list: Vec<FdsnSearchParams>, sender: Sender<FdsnResult>) {
    std::thread::spawn(move || {
        let client = reqwest::blocking::Client::builder().timeout(std::time::Duration::from_secs(15)).build().unwrap();
        let mut all_stations = Vec::new();
        let mut errors = Vec::new();
        
        for params in params_list {
            let _ = sender.send(FdsnResult::Progress(format!("Fetching stations from {}...", params.name)));
            
            let url = format!(
                "{}/fdsnws/station/1/query?latitude={}&longitude={}&minradius={}&maxradius={}&starttime={}&endtime={}&channel={}&level=station&format=text",
                params.url, params.lat, params.lon, params.min_radius, params.max_radius,
                params.start_time.format("%Y-%m-%dT%H:%M:%S"),
                params.end_time.format("%Y-%m-%dT%H:%M:%S"),
                params.channel
            );

            match client.get(&url).send() {
                Ok(resp) => {
                    if resp.status().is_success() {
                        if let Ok(text) = resp.text() {
                            for (i, line) in text.lines().enumerate() {
                                if i == 0 || line.trim().is_empty() { continue; } // skip header and empty lines
                                let parts: Vec<&str> = line.split('|').collect();
                                if parts.len() >= 6 {
                                    let network = parts[0].to_string();
                                    let station = parts[1].to_string();
                                    let lat = parts[2].parse().unwrap_or(0.0);
                                    let lon = parts[3].parse().unwrap_or(0.0);
                                    let elevation = parts[4].parse().unwrap_or(0.0);
                                    let site_name = parts[5].to_string();
                                    all_stations.push(FdsnStation { 
                                        network, station, lat, lon, elevation, site_name, 
                                        provider_name: params.name.clone(),
                                        provider_url: params.url.clone() 
                                    });
                                }
                            }
                        }
                    } else if resp.status().as_u16() != 204 {
                        errors.push(format!("{} (HTTP {})", params.name, resp.status()));
                    }
                },
                Err(_e) => {
                    errors.push(format!("{} (Timeout / Network Error)", params.name));
                }
            }
        }
        
        let _ = sender.send(FdsnResult::StationsFound(all_stations));
        if !errors.is_empty() {
            let _ = sender.send(FdsnResult::Error(format!("Some providers failed: {}", errors.join(", "))));
        }
    });
}

pub fn download_waveforms(params_list: Vec<FdsnDownloadParams>, sender: Sender<FdsnResult>) {
    std::thread::spawn(move || {
        let total = params_list.len();
        let client = reqwest::blocking::Client::builder().timeout(std::time::Duration::from_secs(60)).build().unwrap();
        
        for (i, params) in params_list.into_iter().enumerate() {
            if !params.output_dir.exists() {
                let _ = fs::create_dir_all(&params.output_dir);
            }
            
            let _ = sender.send(FdsnResult::Progress(format!("Downloading {}/{} from {} ({} {})...", i+1, total, params.provider_name, params.network, params.station)));
            
            let url = format!(
                "{}/fdsnws/dataselect/1/query?net={}&sta={}&cha={}&loc=*&starttime={}&endtime={}",
                params.url, params.network, params.station, params.channel,
                params.start_time.format("%Y-%m-%dT%H:%M:%S"),
                params.end_time.format("%Y-%m-%dT%H:%M:%S")
            );
            
            match client.get(&url).send() {
                Ok(resp) => {
                    if resp.status().as_u16() == 204 || resp.status().as_u16() == 404 {
                        // Skip smoothly
                        continue;
                    }
                    if !resp.status().is_success() {
                        let _ = sender.send(FdsnResult::Error(format!("Failed {}.{}: HTTP {}", params.network, params.station, resp.status())));
                        continue;
                    }
                    
                    let bytes = match resp.bytes() {
                        Ok(b) => b,
                        Err(e) => {
                            let _ = sender.send(FdsnResult::Error(format!("Failed to read bytes {}.{}: {}", params.network, params.station, e)));
                            continue;
                        }
                    };
                    
                    let safe_channel = params.channel.replace(",", "_").replace("*", "ALL").replace("?", "ANY");
                    let filename = format!("{}.{}.{}.mseed", params.network, params.station, safe_channel);
                    let filepath = params.output_dir.join(&filename);
                    
                    if let Err(e) = fs::write(&filepath, bytes) {
                        let _ = sender.send(FdsnResult::Error(format!("Failed to write file {}: {}", filename, e)));
                    } else {
                        let _ = sender.send(FdsnResult::WaveformDownloaded(
                            params.network, params.station, filepath.to_string_lossy().to_string()
                        ));
                    }
                },
                Err(e) => {
                    let _ = sender.send(FdsnResult::Error(format!("Request failed for {}.{}: {}", params.network, params.station, e)));
                }
            }
        }
        let _ = sender.send(FdsnResult::WaveformDownloadsComplete);
    });
}

pub fn download_station_xml(params_list: Vec<FdsnDownloadParams>, sender: Sender<FdsnResult>) {
    std::thread::spawn(move || {
        let total = params_list.len();
        let client = reqwest::blocking::Client::builder().timeout(std::time::Duration::from_secs(60)).build().unwrap();
        
        for (i, params) in params_list.into_iter().enumerate() {
            if !params.output_dir.exists() {
                let _ = fs::create_dir_all(&params.output_dir);
            }
            
            let _ = sender.send(FdsnResult::Progress(format!("Downloading StationXML {}/{} from {} ({} {})...", i+1, total, params.provider_name, params.network, params.station)));
            
            // Note: FDSN Station service for level=response
            let url = format!(
                "{}/fdsnws/station/1/query?net={}&sta={}&cha={}&loc=*&starttime={}&endtime={}&level=response&format=xml",
                params.url, params.network, params.station, params.channel,
                params.start_time.format("%Y-%m-%dT%H:%M:%S"),
                params.end_time.format("%Y-%m-%dT%H:%M:%S")
            );
            
            match client.get(&url).send() {
                Ok(resp) => {
                    if resp.status().as_u16() == 204 || resp.status().as_u16() == 404 {
                        continue;
                    }
                    if !resp.status().is_success() {
                        let _ = sender.send(FdsnResult::Error(format!("Failed {}.{}: HTTP {}", params.network, params.station, resp.status())));
                        continue;
                    }
                    
                    let bytes = match resp.bytes() {
                        Ok(b) => b,
                        Err(e) => {
                            let _ = sender.send(FdsnResult::Error(format!("Failed to read XML bytes {}.{}: {}", params.network, params.station, e)));
                            continue;
                        }
                    };
                    
                    let safe_channel = params.channel.replace(",", "_").replace("*", "ALL").replace("?", "ANY");
                    let filename = format!("{}.{}.{}.xml", params.network, params.station, safe_channel);
                    let filepath = params.output_dir.join(&filename);
                    
                    if let Err(e) = fs::write(&filepath, bytes) {
                        let _ = sender.send(FdsnResult::Error(format!("Failed to write StationXML file {}: {}", filename, e)));
                    } else {
                        let _ = sender.send(FdsnResult::ResponseDownloaded(
                            params.network, params.station, filepath.to_string_lossy().to_string()
                        ));
                    }
                },
                Err(e) => {
                    let _ = sender.send(FdsnResult::Error(format!("StationXML Request failed for {}.{}: {}", params.network, params.station, e)));
                }
            }
        }
        let _ = sender.send(FdsnResult::ResponseDownloadsComplete);
    });
}
