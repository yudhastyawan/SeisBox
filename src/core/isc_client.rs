use std::sync::mpsc::Sender;
use chrono::{NaiveDate, NaiveTime, NaiveDateTime};

use crate::core::spatial::BoundingBox;

#[derive(Debug, Clone)]
pub struct EarthquakeEvent {
    pub event_id: String,
    pub timestamp: f64, // Unix timestamp in seconds for easy plotting
    pub date_str: String,
    pub time_str: String,
    pub lat: f64,
    pub lon: f64,
    pub depth_km: f64,
    pub mag: f64,
    pub mag_type: String,
    pub author: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConversionRule {
    pub id: usize,
    pub source_type: String,
    pub min_mag: f64,
    pub max_mag: f64,
    pub multiplier: f64,
    pub offset: f64,
}

#[derive(Debug, Clone)]
pub struct IscSearchParams {
    pub bbox: BoundingBox,
    pub start_date: NaiveDate,
    pub start_time: NaiveTime,
    pub end_date: NaiveDate,
    pub end_time: NaiveTime,
    pub min_depth: f64,
    pub max_depth: f64,
    pub min_mag: f64,
    pub max_mag: f64,
    pub mag_priority: Vec<String>,
}

pub enum IscResult {
    Success(Vec<EarthquakeEvent>, String),
    Error(String),
    Progress(String),
}

/// Spawns a background thread to fetch data from ISC and sends the result back via the provided channel.
pub fn fetch_isc_catalog(params: IscSearchParams, sender: Sender<IscResult>) {
    std::thread::spawn(move || {
        use chrono::Datelike;
        let mut all_events = Vec::new();
        let mut all_raw_txt = String::new();
        let mut current_start = params.start_date;
        let mut is_first_chunk = true;
        
        while current_start <= params.end_date {
            let next_year = current_start.year() + 1;
            let mut next_end = chrono::NaiveDate::from_ymd_opt(next_year, current_start.month(), current_start.day())
                .unwrap_or(params.end_date) - chrono::Duration::days(1);
            
            if next_end >= params.end_date {
                next_end = params.end_date;
            }
            
            let mut chunk_params = params.clone();
            chunk_params.start_date = current_start;
            chunk_params.end_date = next_end;
            
            let msg = format!("Downloading: {} to {}", current_start.format("%Y-%m-%d"), next_end.format("%Y-%m-%d"));
            let _ = sender.send(IscResult::Progress(msg));
            
            match do_fetch(&chunk_params) {
                Ok((mut events, raw_txt)) => {
                    all_events.append(&mut events);
                    if is_first_chunk {
                        all_raw_txt.push_str(&raw_txt);
                        is_first_chunk = false;
                    } else {
                        let mut lines = raw_txt.lines();
                        if let Some(_) = lines.next() {
                            for line in lines {
                                all_raw_txt.push_str(line);
                                all_raw_txt.push('\n');
                            }
                        }
                    }
                },
                Err(e) => {
                    let _ = sender.send(IscResult::Error(format!("Failed at {}: {}", current_start.format("%Y-%m-%d"), e)));
                    return;
                }
            }
            
            current_start = next_end + chrono::Duration::days(1);
        }
        
        let _ = sender.send(IscResult::Success(all_events, all_raw_txt));
    });
}

fn do_fetch(params: &IscSearchParams) -> Result<(Vec<EarthquakeEvent>, String), String> {
    let url = "http://www.isc.ac.uk/cgi-bin/web-db-run";
    
    let s_year = params.start_date.format("%Y").to_string();
    let s_month = params.start_date.format("%m").to_string();
    let s_day = params.start_date.format("%d").to_string();
    let s_time = params.start_time.format("%H:%M:%S").to_string();
    
    let e_year = params.end_date.format("%Y").to_string();
    let e_month = params.end_date.format("%m").to_string();
    let e_day = params.end_date.format("%d").to_string();
    let e_time = params.end_time.format("%H:%M:%S").to_string();
    
    let bot_lat = params.bbox.bot_lat.to_string();
    let top_lat = params.bbox.top_lat.to_string();
    let left_lon = params.bbox.left_lon.to_string();
    let right_lon = params.bbox.right_lon.to_string();
    
    let min_dep = params.min_depth.to_string();
    let max_dep = params.max_depth.to_string();
    let min_mag = params.min_mag.to_string();
    let max_mag = params.max_mag.to_string();
    
    let query_params = vec![
        ("out_format", "CATCSV"),
        ("request", "COMPREHENSIVE"),
        ("searchshape", "RECT"),
        ("bot_lat", bot_lat.as_str()), ("top_lat", top_lat.as_str()),
        ("left_lon", left_lon.as_str()), ("right_lon", right_lon.as_str()),
        ("start_year", s_year.as_str()), ("start_month", s_month.as_str()), ("start_day", s_day.as_str()), ("start_time", s_time.as_str()),
        ("end_year", e_year.as_str()), ("end_month", e_month.as_str()), ("end_day", e_day.as_str()), ("end_time", e_time.as_str()),
        ("min_dep", min_dep.as_str()), ("max_dep", max_dep.as_str()),
        ("min_mag", min_mag.as_str()), ("max_mag", max_mag.as_str())
    ];

    let query_string = query_params.iter().map(|(k, v)| format!("{}={}", k, v)).collect::<Vec<_>>().join("&");
    let full_url = format!("{}?{}", url, query_string);

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| format!("Failed to build client: {}", e))?;

    let response = client.get(&full_url)
        .send()
        .map_err(|e| format!("Request failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("ISC API returned status: {}", response.status()));
    }

    let body = response.text().map_err(|e| format!("Failed to read text: {}", e))?;

    // The ISC API often returns an HTML wrapper even when CSV is requested.
    // We need to extract the actual CSV payload between the header and "STOP".
    let mut csv_data = String::new();
    let mut inside_data = false;
    
    for line in body.lines() {
        let t_line = line.trim();
        if t_line.starts_with("EVENTID,") {
            inside_data = true;
            csv_data.push_str(t_line);
            csv_data.push('\n');
            continue;
        }
        if inside_data {
            if t_line == "STOP" || t_line.starts_with("<") {
                inside_data = false;
                break;
            }
            if !t_line.is_empty() {
                csv_data.push_str(t_line);
                csv_data.push('\n');
            }
        }
    }

    if csv_data.is_empty() {
        if body.contains("No events were found") {
            return Ok((vec![], String::new()));
        }
        if body.contains("Sorry, but your request cannot be processed at the present time") {
            return Err("ISC API Server is busy or rate-limited (Try again in a few minutes).".to_string());
        }
        if body.contains("The search could not be run due to problems") {
            return Err("ISC API Error: The search could not be run due to problems with the search criteria.".to_string());
        }
        
        let snippet: String = body.chars().take(200).collect();
        return Err(format!("ISC API returned HTML instead of CSV data. Snippet: {}", snippet.replace('\n', " ")));
    }

    // Parse the data line by line to handle variable number of columns
    let mut events = Vec::new();
    let priority = &params.mag_priority;

    for line in csv_data.lines() {
        let t_line = line.trim();
        if t_line.starts_with("EVENTID") || t_line.is_empty() {
            continue;
        }

        // Split while preserving empty fields (unlike the python code which dropped them)
        let fields: Vec<&str> = t_line.split(',').map(|s| s.trim()).collect();
        
        // Minimum length 12: EVENTID to first MAG
        if fields.len() < 12 || !fields[0].chars().all(char::is_numeric) {
            continue;
        }

        let event_id = fields[0].to_string();
        let date_str = fields[3].to_string();
        let time_str = fields[4].to_string();
        
        let lat: f64 = match fields[5].parse() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let lon: f64 = match fields[6].parse() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let depth_km: f64 = fields[7].parse().unwrap_or(0.0);

        // Extract magnitude triplets starting from index 9
        // 9: AUTHOR, 10: TYPE, 11: MAG
        let mut mag_data = Vec::new();
        let mut i = 9;
        while i + 2 < fields.len() {
            let author = fields[i];
            let mag_type = fields[i + 1];
            if let Ok(mag_value) = fields[i + 2].parse::<f64>() {
                mag_data.push((mag_type.to_string(), mag_value, author.to_string()));
            }
            i += 3;
        }

        // Sort by magnitude value descending
        mag_data.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let mut selected_mag = None;
        for p in priority {
            for (mt, mv, au) in &mag_data {
                if mt.to_uppercase().starts_with(&p.to_uppercase()) {
                    selected_mag = Some((mt.clone(), *mv, au.clone()));
                    break;
                }
            }
            if selected_mag.is_some() { break; }
        }

        if selected_mag.is_none() && !mag_data.is_empty() {
            selected_mag = Some(mag_data[0].clone());
        }

        let (mag_type, mag, author) = if let Some((mt, mv, au)) = selected_mag {
            (mt.to_uppercase(), mv, au)
        } else {
            ("".to_string(), 0.0, "".to_string())
        };

        let dt_str = format!("{} {}", date_str, time_str);
        let timestamp = if let Ok(dt) = NaiveDateTime::parse_from_str(&dt_str, "%Y-%m-%d %H:%M:%S%.f") {
            dt.and_utc().timestamp_millis() as f64 / 1000.0
        } else {
            if let Ok(dt) = NaiveDateTime::parse_from_str(&dt_str, "%Y-%m-%d %H:%M:%S") {
                dt.and_utc().timestamp_millis() as f64 / 1000.0
            } else {
                0.0
            }
        };

        events.push(EarthquakeEvent {
            event_id,
            timestamp,
            date_str,
            time_str,
            lat,
            lon,
            depth_km,
            mag,
            mag_type,
            author,
        });
    }

    Ok((events, csv_data))
}

pub fn apply_conversion(rules: &[ConversionRule], mag: f64, mag_type: &str) -> (f64, String, String) {
    for rule in rules {
        if mag_type.starts_with(&rule.source_type) {
            if mag >= rule.min_mag && mag <= rule.max_mag {
                let mut converted_mag = rule.multiplier * mag + rule.offset;
                converted_mag = (converted_mag * 1000.0).round() / 1000.0;
                return (converted_mag, "MW".to_string(), rule.source_type.clone());
            }
        }
    }
    (mag, mag_type.to_string(), "".to_string())
}
