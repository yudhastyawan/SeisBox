use std::path::PathBuf;
use std::sync::mpsc;
use chrono::NaiveDateTime;

use crate::cli::FdsnArgs;
use crate::core::fdsn::api::{FdsnDownloadParams, FdsnResult, FdsnSearchParams};
use crate::core::fdsn::downloader::{search_stations, download_waveforms, download_station_xml, download_events};

const DEFAULT_PROVIDERS: &[(&str, &str, &str)] = &[
    ("AUSPASS", "http://auspass.edu.au", "Australian regional network"),
    ("BGR", "http://eida.bgr.de", "German EIDA Node"),
    ("EIDA", "http://eida-federator.ethz.ch", "European primary federator"),
    ("ETH", "http://eida.ethz.ch", "Swiss / EIDA Node"),
    ("GEONET", "http://service.geonet.org.nz", "New Zealand network"),
    ("GFZ", "http://geofon.gfz-potsdam.de", "GEOFON global network / EIDA Node"),
    ("ICGC", "http://ws.icgc.cat", "Catalonia, Spain"),
    ("IESDMC", "http://batsws.earth.sinica.edu.tw", "BATS network (Taiwan)"),
    ("INGV", "http://webservices.ingv.it", "Italy / EIDA Node"),
    ("IPGP", "http://ws.ipgp.fr", "France (GEOSCOPE)"),
    ("IRIS", "http://service.iris.edu", "Largest global federator (EarthScope)"),
    ("KNMI", "http://rdsa.knmi.nl", "Netherlands / EIDA Node"),
    ("KOERI", "http://eida.koeri.boun.edu.tr", "Turkey / EIDA Node"),
    ("LMU", "https://erde.geophysik.uni-muenchen.de", "Munich University"),
    ("NCEDC", "http://service.ncedc.org", "Northern California"),
    ("NIEP", "http://eida-sc3.infp.ro", "Romania / EIDA Node"),
    ("NOA", "http://eida.gein.noa.gr", "Greece / EIDA Node"),
    ("ORFEUS", "http://www.orfeus-eu.org", "European EIDA coordination center"),
    ("RASPISHAKE", "https://data.raspberryshake.org", "Citizen-science network / IoT geophones"),
    ("RESIF", "http://ws.resif.fr", "France (now Epos-France)"),
    ("RESIFPH5", "http://ph5ws.resif.fr", "Active seismic data from France"),
    ("SCEDC", "http://service.scedc.caltech.edu", "Southern California"),
    ("TEXNET", "http://rtserve.beg.utexas.edu", "Texas Network"),
    ("UIB-NORSAR", "http://eida.geo.uib.no", "Norway / EIDA Node"),
    ("USP", "http://sismo.iag.usp.br", "Brazilian Seismograph Network"),
];

pub fn run_cli(args: FdsnArgs) -> Result<(), String> {
    println!("SeisBox FDSN Downloader (CLI Mode)");
    println!("====================================");

    // 1. Parse dates
    let (start_time, end_time) = if let Some(ref_time_str) = &args.ref_time {
        let r_time = NaiveDateTime::parse_from_str(ref_time_str, "%Y-%m-%d %H:%M:%S")
            .map_err(|e| format!("Invalid ref_time format (expected YYYY-MM-DD HH:MM:SS): {}", e))?;
        
        let s_off = args.start_offset.unwrap_or(-60);
        let e_off = args.end_offset.unwrap_or(600);
        
        let st = r_time + chrono::Duration::seconds(s_off);
        let et = r_time + chrono::Duration::seconds(e_off);
        (st, et)
    } else {
        if args.start_time.is_none() || args.end_time.is_none() {
            return Err("You must provide either --ref-time (with offsets) OR both --start-time and --end-time.".to_string());
        }
        let st = NaiveDateTime::parse_from_str(args.start_time.as_ref().unwrap(), "%Y-%m-%d %H:%M:%S")
            .map_err(|e| format!("Invalid start_time format (expected YYYY-MM-DD HH:MM:SS): {}", e))?;
        let et = NaiveDateTime::parse_from_str(args.end_time.as_ref().unwrap(), "%Y-%m-%d %H:%M:%S")
            .map_err(|e| format!("Invalid end_time format (expected YYYY-MM-DD HH:MM:SS): {}", e))?;
        (st, et)
    };

    // 2. Resolve providers
    let mut selected_providers = Vec::new();
    if let Some(prov_str) = &args.providers {
        if prov_str.trim().eq_ignore_ascii_case("ALL") {
            selected_providers = DEFAULT_PROVIDERS.iter().map(|(n, u, _)| (n.to_string(), u.to_string())).collect();
        } else {
            let names: Vec<&str> = prov_str.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
            for name in names {
                if let Some((n, u, _)) = DEFAULT_PROVIDERS.iter().find(|(n, _, _)| n.eq_ignore_ascii_case(name)) {
                    selected_providers.push((n.to_string(), u.to_string()));
                } else if name.contains("http://") || name.contains("https://") {
                    // Try to treat as custom URL (e.g. "USGS:https://earthquake.usgs.gov" or just "https://...")
                    if let Some(idx) = name.find("http") {
                        if idx > 1 && &name[idx-1..idx] == ":" {
                            let custom_name = &name[0..idx-1];
                            let custom_url = &name[idx..];
                            selected_providers.push((custom_name.to_string(), custom_url.to_string()));
                        } else {
                            selected_providers.push(("CUSTOM".to_string(), name.to_string()));
                        }
                    } else {
                        selected_providers.push(("CUSTOM".to_string(), name.to_string()));
                    }
                } else {
                    println!("Warning: Unknown provider '{}'. Skipping.", name);
                }
            }
        }
    } else {
        // Default to IRIS
        selected_providers.push(("IRIS".to_string(), "http://service.iris.edu".to_string()));
    }

    if selected_providers.is_empty() {
        return Err("No valid providers selected.".to_string());
    }
    
    // Validate spatial arguments
    if args.lat.is_none() && args.lon.is_none() && args.min_lat.is_none() && args.min_lon.is_none() {
        return Err("You must provide either --lat/--lon (for radial search) OR --min-lat/--max-lat/--min-lon/--max-lon (for bounding box search).".to_string());
    }

    println!("Selected Providers:");
    for (n, u) in &selected_providers {
        println!("  - {}: {}", n, u);
    }
    
    if let (Some(lat), Some(lon)) = (args.lat, args.lon) {
        println!("Search area (Radial): Lat {}, Lon {}, Radius {} - {} deg", lat, lon, args.min_radius, args.max_radius);
    } else if let (Some(minlat), Some(maxlat), Some(minlon), Some(maxlon)) = (args.min_lat, args.max_lat, args.min_lon, args.max_lon) {
        println!("Search area (Box): Lat {} to {}, Lon {} to {}", minlat, maxlat, minlon, maxlon);
    }
    println!("Time range: {} to {}", start_time, end_time);
    println!("Channel: {}", args.channel);
    println!("Output dir: {}", args.out_dir);

    // 3. Search Stations
    let (tx, rx) = mpsc::channel();
    
    let mut search_params = Vec::new();
    for (name, url) in selected_providers {
        search_params.push(FdsnSearchParams {
            name,
            url,
            lat: args.lat,
            lon: args.lon,
            min_radius: Some(args.min_radius),
            max_radius: Some(args.max_radius),
            min_lat: args.min_lat,
            max_lat: args.max_lat,
            min_lon: args.min_lon,
            max_lon: args.max_lon,
            min_mag: args.min_mag,
            max_mag: args.max_mag,
            include_arrivals: args.download_arrivals,
            start_time,
            end_time,
            channel: args.channel.clone(),
            network: args.network.clone(),
            station: args.station.clone(),
        });
    }
    
    if args.download_events {
        println!("\n[Events] Downloading event catalog...");
        let (ev_tx, ev_rx) = mpsc::channel();
        download_events(search_params.clone(), PathBuf::from(&args.out_dir), ev_tx);
        
        loop {
            match ev_rx.recv() {
                Ok(FdsnResult::Progress(msg)) => {
                    println!("[Event] {}", msg);
                }
                Ok(FdsnResult::Error(err)) => {
                    println!("[Event Error] {}", err);
                }
                Ok(FdsnResult::EventsDownloaded(path)) => {
                    println!("[Event OK] Saved catalog to {}", path);
                }
                Err(_) => break,
                _ => {}
            }
        }
        
        if args.no_mseed && !args.download_xml && args.plot_map.is_none() {
            println!("Event download complete. Skipping waveforms and XML.");
            return Ok(());
        }
    }

    println!("\n1. Searching for stations...");
    search_stations(search_params, tx.clone());

    let mut stations = Vec::new();
    loop {
        match rx.recv() {
            Ok(FdsnResult::Progress(msg)) => {
                println!("[Search] {}", msg);
            }
            Ok(FdsnResult::Error(err)) => {
                println!("[Search Error] {}", err);
            }
            Ok(FdsnResult::StationsFound(found)) => {
                stations = found;
                break;
            }
            _ => {}
        }
    }

    println!("Found {} matching stations.", stations.len());
    if stations.is_empty() {
        println!("No stations found. Exiting.");
        return Ok(());
    }

    // 3.5 Plot map if requested
    if let Some(map_out) = &args.plot_map {
        println!("Generating Spatial Map in {} ...", map_out);
        let center_lat = args.lat.unwrap_or_else(|| (args.min_lat.unwrap_or(0.0) + args.max_lat.unwrap_or(0.0)) / 2.0);
        let center_lon = args.lon.unwrap_or_else(|| (args.min_lon.unwrap_or(0.0) + args.max_lon.unwrap_or(0.0)) / 2.0);
        crate::core::fdsn::plot::generate_spatial_map_viz(&stations, center_lat, center_lon, map_out)
            .map_err(|e| format!("Failed to generate map: {}", e))?;
        println!("  -> Saved: {}", map_out);
    }

    // 4. Download StationXML
    let out_dir = PathBuf::from(&args.out_dir);
    let mut download_params = Vec::new();
    for sta in &stations {
        download_params.push(FdsnDownloadParams {
            provider_name: sta.provider_name.clone(),
            url: sta.provider_url.clone(),
            network: sta.network.clone(),
            station: sta.station.clone(),
            channel: args.channel.clone(),
            start_time,
            end_time,
            output_dir: out_dir.clone(),
            export_sac: args.export_sac,
        });
    }

    if args.download_xml {
        println!("\n2. Downloading StationXML...");
        download_station_xml(download_params.clone(), tx.clone());
        loop {
            match rx.recv() {
                Ok(FdsnResult::Progress(msg)) => {
                    println!("[XML] {}", msg);
                }
                Ok(FdsnResult::Error(err)) => {
                    println!("[XML Error] {}", err);
                }
                Ok(FdsnResult::ResponseDownloaded(net, sta, file)) => {
                    println!("[XML OK] {}.{} -> {}", net, sta, file);
                }
                Ok(FdsnResult::ResponseDownloadsComplete) => {
                    break;
                }
                _ => {}
            }
        }
    }

    // 5. Download Waveforms
    if !args.no_mseed {
        println!("\n3. Downloading Waveforms (MiniSEED)...");
        download_waveforms(download_params, tx.clone());
        loop {
            match rx.recv() {
                Ok(FdsnResult::Progress(msg)) => {
                    println!("[mseed] {}", msg);
                }
                Ok(FdsnResult::Error(err)) => {
                    println!("[mseed Error] {}", err);
                }
                Ok(FdsnResult::WaveformDownloaded(net, sta, file)) => {
                    println!("[mseed OK] {}.{} -> {}", net, sta, file);
                }
                Ok(FdsnResult::WaveformDownloadsComplete) => {
                    break;
                }
                _ => {}
            }
        }
    }

    println!("\nAll tasks completed successfully.");
    Ok(())
}
