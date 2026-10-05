use super::api::{FdsnResult, FdsnSearchParams, FdsnDownloadParams, FdsnStation};
use std::sync::mpsc::Sender;
use std::fs;

pub fn search_stations(params_list: Vec<FdsnSearchParams>, sender: Sender<FdsnResult>) {
    std::thread::spawn(move || {
        use rayon::prelude::*;

        let results: Vec<(Vec<FdsnStation>, Option<String>)> = params_list.into_par_iter().map_with(sender.clone(), |s, params| {
            let client = reqwest::blocking::Client::builder().timeout(std::time::Duration::from_secs(15)).build().unwrap();
            let _ = s.send(FdsnResult::Progress(format!("Fetching stations from {}...", params.name)));
            
            let mut url = format!(
                "{}/fdsnws/station/1/query?starttime={}&endtime={}&channel={}&level=station&format=text",
                params.url,
                params.start_time.format("%Y-%m-%dT%H:%M:%S"),
                params.end_time.format("%Y-%m-%dT%H:%M:%S"),
                params.channel
            );
            
            if let (Some(lat), Some(lon), Some(min_r), Some(max_r)) = (params.lat, params.lon, params.min_radius, params.max_radius) {
                url.push_str(&format!("&latitude={}&longitude={}&minradius={}&maxradius={}", lat, lon, min_r, max_r));
            } else if let (Some(minlat), Some(maxlat), Some(minlon), Some(maxlon)) = (params.min_lat, params.max_lat, params.min_lon, params.max_lon) {
                url.push_str(&format!("&minlatitude={}&maxlatitude={}&minlongitude={}&maxlongitude={}", minlat, maxlat, minlon, maxlon));
            }
            if let Some(net) = &params.network {
                url.push_str(&format!("&network={}", net));
            }
            if let Some(sta) = &params.station {
                url.push_str(&format!("&station={}", sta));
            }

            let mut stations = Vec::new();
            let mut err_msg = None;

            match client.get(&url).send() {
                Ok(resp) => {
                    if resp.status().is_success() {
                        if let Ok(text) = resp.text() {
                            for (i, line) in text.lines().enumerate() {
                                if i == 0 || line.trim().is_empty() { continue; }
                                let parts: Vec<&str> = line.split('|').collect();
                                if parts.len() >= 6 {
                                    let network = parts[0].to_string();
                                    let station = parts[1].to_string();
                                    let lat = parts[2].parse().unwrap_or(0.0);
                                    let lon = parts[3].parse().unwrap_or(0.0);
                                    let elevation = parts[4].parse().unwrap_or(0.0);
                                    let site_name = parts[5].to_string();
                                    stations.push(FdsnStation { 
                                        network, station, lat, lon, elevation, site_name, 
                                        provider_name: params.name.clone(),
                                        provider_url: params.url.clone() 
                                    });
                                }
                            }
                        }
                    } else if resp.status().as_u16() != 204 {
                        err_msg = Some(format!("{} (HTTP {})", params.name, resp.status()));
                    }
                },
                Err(_e) => {
                    err_msg = Some(format!("{} (Timeout / Network Error)", params.name));
                }
            }
            (stations, err_msg)
        }).collect();

        let mut all_stations = Vec::new();
        let mut errors = Vec::new();
        for (st, e) in results {
            all_stations.extend(st);
            if let Some(e_msg) = e {
                errors.push(e_msg);
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
        
        use rayon::prelude::*;
        params_list.into_par_iter().enumerate().for_each_with(sender.clone(), |s, (i, params)| {
            if !params.output_dir.exists() {
                let _ = fs::create_dir_all(&params.output_dir);
            }
            
            let _ = s.send(FdsnResult::Progress(format!("Downloading {}/{} from {} ({} {})...", i+1, total, params.provider_name, params.network, params.station)));
            
            let channels: Vec<&str> = params.channel.split(',').collect();
            for cha in channels {
                let cha = cha.trim();
                if cha.is_empty() { continue; }
                
                let url = format!(
                    "{}/fdsnws/dataselect/1/query?net={}&sta={}&cha={}&loc=*&starttime={}&endtime={}",
                    params.url, params.network, params.station, cha,
                    params.start_time.format("%Y-%m-%dT%H:%M:%S"),
                    params.end_time.format("%Y-%m-%dT%H:%M:%S")
                );
                
                let mut retry_count = 0;
                let max_retries = 3;
                let mut success = false;
                
                while retry_count < max_retries && !success {
                    match client.get(&url).send() {
                        Ok(resp) => {
                            if resp.status().as_u16() == 204 || resp.status().as_u16() == 404 {
                                let _ = s.send(FdsnResult::Progress(format!("Skipped {}.{}.{}: No Data (HTTP {})", params.network, params.station, cha, resp.status())));
                                success = true;
                                continue;
                            }
                            if !resp.status().is_success() {
                                let _ = s.send(FdsnResult::Error(format!("Failed {}.{}.{}: HTTP {} (Retry {}/{})", params.network, params.station, cha, resp.status(), retry_count+1, max_retries)));
                                retry_count += 1;
                                std::thread::sleep(std::time::Duration::from_secs(2));
                                continue;
                            }
                            
                            let bytes = match resp.bytes() {
                                Ok(b) => b,
                                Err(e) => {
                                    let _ = s.send(FdsnResult::Error(format!("Failed to read bytes {}.{}.{}: {} (Retry {}/{})", params.network, params.station, cha, e, retry_count+1, max_retries)));
                                    retry_count += 1;
                                    std::thread::sleep(std::time::Duration::from_secs(2));
                                    continue;
                                }
                            };
                            
                            let safe_channel = cha.replace("*", "ALL").replace("?", "ANY");
                            let filename = format!("{}.{}.{}.mseed", params.network, params.station, safe_channel);
                            let filepath = params.output_dir.join(&filename);
                            
                            if let Err(e) = fs::write(&filepath, bytes) {
                                let _ = s.send(FdsnResult::Error(format!("Failed to write file {}: {}", filename, e)));
                            } else {
                                if params.export_sac {
                                    if let Ok(seis_list) = seisbox_core::core::parser::parse_seismic_file(&filepath) {
                                        for (i, seis) in seis_list.iter().enumerate() {
                                            let sac_filename = format!("{}.{}.{}.sac", params.network, params.station, seis.channel);
                                            let sac_filepath = params.output_dir.join(&sac_filename);
                                            if let Err(e) = seisbox_core::io::sac_export::export_seismogram_to_sac(seis, &sac_filepath) {
                                                let _ = s.send(FdsnResult::Error(format!("Failed to write SAC file {}: {}", sac_filename, e)));
                                            }
                                        }
                                    } else {
                                        let _ = s.send(FdsnResult::Error(format!("Failed to parse MiniSEED for SAC export: {}", filename)));
                                    }
                                }

                                let _ = s.send(FdsnResult::WaveformDownloaded(
                                    params.network.clone(), params.station.clone(), filepath.to_string_lossy().to_string()
                                ));
                            }
                            success = true;
                        },
                        Err(e) => {
                            let _ = s.send(FdsnResult::Error(format!("Request failed for {}.{}.{}: {} (Retry {}/{})", params.network, params.station, cha, e, retry_count+1, max_retries)));
                            retry_count += 1;
                            std::thread::sleep(std::time::Duration::from_secs(2));
                        }
                    }
                }
            }
        });
        let _ = sender.send(FdsnResult::WaveformDownloadsComplete);
    });
}

pub fn download_station_xml(params_list: Vec<FdsnDownloadParams>, sender: Sender<FdsnResult>) {
    std::thread::spawn(move || {
        let total = params_list.len();
        let client = reqwest::blocking::Client::builder().timeout(std::time::Duration::from_secs(60)).build().unwrap();
        
        use rayon::prelude::*;
        params_list.into_par_iter().enumerate().for_each_with(sender.clone(), |s, (i, params)| {
            if !params.output_dir.exists() {
                let _ = fs::create_dir_all(&params.output_dir);
            }
            
            let _ = s.send(FdsnResult::Progress(format!("Downloading StationXML {}/{} from {} ({} {})...", i+1, total, params.provider_name, params.network, params.station)));
            
            let channels: Vec<&str> = params.channel.split(',').collect();
            for cha in channels {
                let cha = cha.trim();
                if cha.is_empty() { continue; }
                
                // Note: FDSN Station service for level=response
                let url = format!(
                    "{}/fdsnws/station/1/query?net={}&sta={}&cha={}&loc=*&starttime={}&endtime={}&level=response&format=xml",
                    params.url, params.network, params.station, cha,
                    params.start_time.format("%Y-%m-%dT%H:%M:%S"),
                    params.end_time.format("%Y-%m-%dT%H:%M:%S")
                );
                
                let mut retry_count = 0;
                let max_retries = 3;
                let mut success = false;
                
                while retry_count < max_retries && !success {
                    match client.get(&url).send() {
                        Ok(resp) => {
                            if resp.status().as_u16() == 204 || resp.status().as_u16() == 404 {
                                let _ = s.send(FdsnResult::Progress(format!("Skipped {}.{}.{}: No Data (HTTP {})", params.network, params.station, cha, resp.status())));
                                success = true;
                                continue;
                            }
                            if !resp.status().is_success() {
                                let _ = s.send(FdsnResult::Error(format!("Failed {}.{}.{}: HTTP {} (Retry {}/{})", params.network, params.station, cha, resp.status(), retry_count+1, max_retries)));
                                retry_count += 1;
                                std::thread::sleep(std::time::Duration::from_secs(2));
                                continue;
                            }
                            
                            let bytes = match resp.bytes() {
                                Ok(b) => b,
                                Err(e) => {
                                    let _ = s.send(FdsnResult::Error(format!("Failed to read XML bytes {}.{}.{}: {} (Retry {}/{})", params.network, params.station, cha, e, retry_count+1, max_retries)));
                                    retry_count += 1;
                                    std::thread::sleep(std::time::Duration::from_secs(2));
                                    continue;
                                }
                            };
                            
                            let safe_channel = cha.replace("*", "ALL").replace("?", "ANY");
                            let filename = format!("{}.{}.{}.xml", params.network, params.station, safe_channel);
                            let filepath = params.output_dir.join(&filename);
                            
                            if let Err(e) = fs::write(&filepath, bytes) {
                                let _ = s.send(FdsnResult::Error(format!("Failed to write StationXML file {}: {}", filename, e)));
                            } else {
                                let _ = s.send(FdsnResult::ResponseDownloaded(
                                    params.network.clone(), params.station.clone(), filepath.to_string_lossy().to_string()
                                ));
                            }
                            success = true;
                        },
                        Err(e) => {
                            let _ = s.send(FdsnResult::Error(format!("Request failed for XML {}.{}.{}: {} (Retry {}/{})", params.network, params.station, cha, e, retry_count+1, max_retries)));
                            retry_count += 1;
                            std::thread::sleep(std::time::Duration::from_secs(2));
                        }
                    }
                }
            }
        });
        let _ = sender.send(FdsnResult::ResponseDownloadsComplete);
    });
}

pub fn download_events(params_list: Vec<FdsnSearchParams>, out_dir: std::path::PathBuf, sender: Sender<FdsnResult>) {
    std::thread::spawn(move || {
        use rayon::prelude::*;
        
        if !out_dir.exists() {
            let _ = fs::create_dir_all(&out_dir);
        }
        
        params_list.into_par_iter().for_each_with(sender.clone(), |s, params| {
            let client = reqwest::blocking::Client::builder().timeout(std::time::Duration::from_secs(30)).build().unwrap();
            let _ = s.send(FdsnResult::Progress(format!("Fetching events from {}...", params.name)));
            
            // Build base URL
            let mut base_url = format!(
                "{}/fdsnws/event/1/query?starttime={}&endtime={}",
                params.url,
                params.start_time.format("%Y-%m-%dT%H:%M:%S"),
                params.end_time.format("%Y-%m-%dT%H:%M:%S")
            );
            
            if let (Some(lat), Some(lon), Some(min_r), Some(max_r)) = (params.lat, params.lon, params.min_radius, params.max_radius) {
                base_url.push_str(&format!("&latitude={}&longitude={}&minradius={}&maxradius={}", lat, lon, min_r, max_r));
            } else if let (Some(minlat), Some(maxlat), Some(minlon), Some(maxlon)) = (params.min_lat, params.max_lat, params.min_lon, params.max_lon) {
                base_url.push_str(&format!("&minlatitude={}&maxlatitude={}&minlongitude={}&maxlongitude={}", minlat, maxlat, minlon, maxlon));
            }
            if let Some(min_mag) = params.min_mag {
                base_url.push_str(&format!("&minmag={}", min_mag));
            }
            if let Some(max_mag) = params.max_mag {
                base_url.push_str(&format!("&maxmag={}", max_mag));
            }
            
            // 1. Download basic event catalog (format=text)
            let event_url = format!("{}&format=text", base_url);
            match client.get(&event_url).send() {
                Ok(resp) => {
                    if resp.status().as_u16() == 204 || resp.status().as_u16() == 404 {
                        let _ = s.send(FdsnResult::Progress(format!("No events found in {} (HTTP {})", params.name, resp.status())));
                    } else if !resp.status().is_success() {
                        let _ = s.send(FdsnResult::Error(format!("Failed to fetch events from {}: HTTP {}", params.name, resp.status())));
                    } else {
                        if let Ok(text) = resp.text() {
                            let filename = format!("{}_events.csv", params.name.to_lowercase().replace(" ", "_"));
                            let filepath = out_dir.join(&filename);
                            if let Err(e) = fs::write(&filepath, text) {
                                let _ = s.send(FdsnResult::Error(format!("Failed to write events file {}: {}", filename, e)));
                            } else {
                                let _ = s.send(FdsnResult::EventsDownloaded(filepath.to_string_lossy().to_string()));
                            }
                        }
                    }
                },
                Err(e) => {
                    let _ = s.send(FdsnResult::Error(format!("Event request failed for {}: {}", params.name, e)));
                }
            }
            
            // 2. Download phase arrivals if requested (format=xml&includearrivals=true)
            if params.include_arrivals {
                let _ = s.send(FdsnResult::Progress(format!("Fetching arrivals from {}...", params.name)));
                let arrival_url = format!("{}&format=xml&includearrivals=true", base_url);
                match client.get(&arrival_url).send() {
                    Ok(resp) => {
                        if resp.status().as_u16() == 204 || resp.status().as_u16() == 404 {
                            let _ = s.send(FdsnResult::Progress(format!("No arrivals found in {} (HTTP {})", params.name, resp.status())));
                        } else if !resp.status().is_success() {
                            let _ = s.send(FdsnResult::Error(format!("Failed to fetch arrivals from {}: HTTP {}", params.name, resp.status())));
                        } else {
                            if let Ok(xml_text) = resp.text() {
                                match super::quakeml::parse_quakeml_to_csv(&xml_text) {
                                    Ok(csv) => {
                                        let filename = format!("{}_arrivals.csv", params.name.to_lowercase().replace(" ", "_"));
                                        let filepath = out_dir.join(&filename);
                                        if let Err(e) = fs::write(&filepath, csv) {
                                            let _ = s.send(FdsnResult::Error(format!("Failed to write arrivals file {}: {}", filename, e)));
                                        } else {
                                            let _ = s.send(FdsnResult::EventsDownloaded(filepath.to_string_lossy().to_string()));
                                        }
                                    },
                                    Err(e) => {
                                        let _ = s.send(FdsnResult::Error(format!("Failed to parse QuakeML from {}: {}", params.name, e)));
                                    }
                                }
                            }
                        }
                    },
                    Err(e) => {
                        let _ = s.send(FdsnResult::Error(format!("Arrival request failed for {}: {}", params.name, e)));
                    }
                }
            }
        });
    });
}
