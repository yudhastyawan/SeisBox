use std::fs;
use std::sync::mpsc;
use chrono::{NaiveDate, NaiveTime};

use seisbox_core::core::isc_client::{fetch_isc_catalog, IscResult, IscSearchParams, apply_conversion, ConversionRule, EarthquakeEvent};
use seisbox_core::core::spatial::BoundingBox;

use crate::cli::Cli;

pub fn run_cli(cli: Cli) -> Result<(), String> {
    let mut final_events = Vec::new();
    let mut final_raw_txt = String::new();

    let mag_priority: Vec<String> = cli.mag_priority.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if let Some(in_csv) = cli.input_csv {
        println!("Reading events from {} ...", in_csv.display());
        let mut rdr = csv::ReaderBuilder::new().has_headers(true).from_path(&in_csv)
            .map_err(|e| format!("Failed to read input CSV: {}", e))?;
        
        for (i, result) in rdr.records().enumerate() {
            let record = result.map_err(|e| format!("CSV error at line {}: {}", i + 2, e))?;
            if record.len() < 8 { continue; }
            let time_str = &record[1];
            let timestamp = chrono::NaiveDateTime::parse_from_str(time_str, "%Y-%m-%dT%H:%M:%SZ")
                .map(|dt| dt.and_utc().timestamp() as f64)
                .unwrap_or(0.0);
            
            let date_str_val = time_str.split('T').next().unwrap_or("").to_string();
            let time_str_val = time_str.split('T').nth(1).unwrap_or("").replace("Z", "");

            final_events.push(EarthquakeEvent {
                event_id: record[0].to_string(),
                timestamp,
                date_str: date_str_val,
                time_str: time_str_val,
                lat: record[2].parse().unwrap_or(0.0),
                lon: record[3].parse().unwrap_or(0.0),
                depth_km: record[4].parse().unwrap_or(0.0),
                mag: record[5].parse().unwrap_or(0.0),
                mag_type: record[6].to_string(),
                author: record[7].to_string(),
            });
        }
        println!("Loaded {} events from CSV.", final_events.len());
    } else if let Some(in_raw) = cli.input_raw {
        println!("Reading raw ISC text from {} ...", in_raw.display());
        let body = std::fs::read_to_string(&in_raw)
            .map_err(|e| format!("Failed to read input raw text file: {}", e))?;
        
        match seisbox_core::core::isc_client::parse_isc_html_body(&body, &mag_priority) {
            Ok((events, raw_txt)) => {
                final_events = events;
                final_raw_txt = raw_txt;
                println!("Loaded {} events from raw ISC text.", final_events.len());
            },
            Err(e) => {
                return Err(format!("Failed to parse raw text: {}", e));
            }
        }
    } else {
        // Validate fetch parameters
        let min_lon = cli.min_lon.ok_or("Missing --min-lon")?;
        let max_lon = cli.max_lon.ok_or("Missing --max-lon")?;
        let min_lat = cli.min_lat.ok_or("Missing --min-lat")?;
        let max_lat = cli.max_lat.ok_or("Missing --max-lat")?;

        let start_date_str = cli.start_date.ok_or("Missing --start-date")?;
        let end_date_str = cli.end_date.ok_or("Missing --end-date")?;

        let sd = NaiveDate::parse_from_str(&start_date_str, "%Y-%m-%d")
            .map_err(|_| "Invalid --start-date format. Expected YYYY-MM-DD")?;
        let st = NaiveTime::parse_from_str(&cli.start_time, "%H:%M:%S")
            .map_err(|_| "Invalid --start-time format. Expected HH:MM:SS")?;
        let ed = NaiveDate::parse_from_str(&end_date_str, "%Y-%m-%d")
            .map_err(|_| "Invalid --end-date format. Expected YYYY-MM-DD")?;
        let et = NaiveTime::parse_from_str(&cli.end_time, "%H:%M:%S")
            .map_err(|_| "Invalid --end-time format. Expected HH:MM:SS")?;



        let bbox = BoundingBox {
            left_lon: min_lon,
            right_lon: max_lon,
            bot_lat: min_lat,
            top_lat: max_lat,
        };

        let params = IscSearchParams {
            bbox,
            start_date: sd,
            start_time: st,
            end_date: ed,
            end_time: et,
            min_depth: cli.min_depth,
            max_depth: cli.max_depth,
            min_mag: cli.min_mag,
            max_mag: cli.max_mag,
            mag_priority,
            chunk_days: cli.chunk_days,
        };

        println!("Starting ISC Catalog download (Headless Mode)...");
        println!("Area: Lon [{:.2}, {:.2}], Lat [{:.2}, {:.2}]", min_lon, max_lon, min_lat, max_lat);
        println!("Time: {} {} to {} {}", sd, st, ed, et);
        
        let (tx, rx) = mpsc::channel();
        fetch_isc_catalog(params, tx, false);

        let mut is_done = false;
        while !is_done {
            if let Ok(result) = rx.recv() {
                match result {
                    IscResult::Success(events, raw_txt, _) => {
                        final_events = events;
                        final_raw_txt = raw_txt;
                        println!("Successfully downloaded {} events.", final_events.len());
                        is_done = true;
                    },
                    IscResult::Error(err) => {
                        return Err(format!("Download Error: {}", err));
                    },
                    IscResult::PartialError(err, events, raw_txt, _, _) => {
                        println!("Warning: Partial Error occurred: {}", err);
                        final_events = events;
                        final_raw_txt = raw_txt;
                        println!("Recovered {} events before failure.", final_events.len());
                        is_done = true;
                    },
                    IscResult::Progress(msg) => {
                        println!("[Progress] {}", msg);
                    }
                }
            }
        }
    }

    // Save Raw TXT if requested
    if let Some(raw_path) = cli.raw_output {
        if cli.append {
            println!("Appending Raw TXT to {} ...", raw_path.display());
            let mut file = std::fs::OpenOptions::new().create(true).append(true).open(&raw_path)
                .map_err(|e| format!("Failed to open raw output for appending: {}", e))?;
            use std::io::Write;
            file.write_all(final_raw_txt.as_bytes()).map_err(|e| format!("Failed to append raw output: {}", e))?;
        } else {
            println!("Saving Raw TXT to {} ...", raw_path.display());
            fs::write(&raw_path, &final_raw_txt).map_err(|e| format!("Failed to save raw output: {}", e))?;
        }
    }

    // Default Conversion rules
    let mut conversion_rules = vec![
        ConversionRule { id: 0, source_type: "MB".to_string(), min_mag: -99.0, max_mag: 8.2, multiplier: 1.0107, offset: 0.0801 },
        ConversionRule { id: 1, source_type: "MS".to_string(), min_mag: -99.0, max_mag: 6.1, multiplier: 0.6016, offset: 2.476 },
        ConversionRule { id: 2, source_type: "MS".to_string(), min_mag: 6.2, max_mag: 99.0, multiplier: 0.9239, offset: 0.5671 },
        ConversionRule { id: 3, source_type: "MLV".to_string(), min_mag: -99.0, max_mag: 99.0, multiplier: 1.0, offset: 0.0 },
        ConversionRule { id: 4, source_type: "ML".to_string(), min_mag: -99.0, max_mag: 99.0, multiplier: 1.0, offset: 0.0 },
        ConversionRule { id: 5, source_type: "MW".to_string(), min_mag: -99.0, max_mag: 99.0, multiplier: 1.0, offset: 0.0 },
    ];

    if let Some(conv_path) = cli.conversion_file {
        println!("Loading custom conversion rules from {} ...", conv_path.display());
        let mut rdr = csv::ReaderBuilder::new().has_headers(false).from_path(&conv_path)
            .map_err(|e| format!("Failed to open conversion file: {}", e))?;
        
        conversion_rules.clear();
        let mut id_counter = 0;
        for (line_idx, result) in rdr.records().enumerate() {
            let record = result.map_err(|e| format!("CSV error at line {}: {}", line_idx + 1, e))?;
            if record.len() < 5 {
                if record[0].to_lowercase() == "source_type" { continue; }
                return Err(format!("Conversion file must have at least 5 columns at line {}", line_idx + 1));
            }
            if line_idx == 0 && record[1].parse::<f64>().is_err() {
                continue;
            }
            let source_type = record[0].to_string().to_uppercase();
            let min_mag: f64 = record[1].parse().map_err(|_| format!("Invalid min_mag at line {}", line_idx + 1))?;
            let max_mag: f64 = record[2].parse().map_err(|_| format!("Invalid max_mag at line {}", line_idx + 1))?;
            let multiplier: f64 = record[3].parse().map_err(|_| format!("Invalid multiplier at line {}", line_idx + 1))?;
            let offset: f64 = record[4].parse().map_err(|_| format!("Invalid offset at line {}", line_idx + 1))?;

            conversion_rules.push(ConversionRule {
                id: id_counter,
                source_type,
                min_mag,
                max_mag,
                multiplier,
                offset,
            });
            id_counter += 1;
        }
        println!("Loaded {} custom conversion rules.", conversion_rules.len());
    }

    // Save Processed CSV if requested
    if let Some(csv_path) = cli.output {
        let file_exists = csv_path.exists();
        
        let file = if cli.append {
            println!("Appending Processed CSV to {} ...", csv_path.display());
            std::fs::OpenOptions::new().create(true).append(true).open(&csv_path)
                .map_err(|e| format!("Failed to open CSV for appending: {}", e))?
        } else {
            println!("Saving Processed CSV to {} ...", csv_path.display());
            std::fs::File::create(&csv_path)
                .map_err(|e| format!("Failed to create CSV: {}", e))?
        };

        let mut wtr = csv::WriterBuilder::new().has_headers(false).from_writer(file);
        
        if !cli.append || !file_exists {
            wtr.write_record(&["event_id", "time", "latitude", "longitude", "depth", "magnitude", "magnitude_type", "author", "converted_from", "magnitude_real"]).map_err(|e| e.to_string())?;
        }

        let mut row_count = 0;
        for ev in &final_events {
            if let Some(dt) = chrono::DateTime::from_timestamp_millis((ev.timestamp * 1000.0) as i64) {
                let (mw_mag, mw_type, converted_from) = apply_conversion(&conversion_rules, ev.mag, &ev.mag_type);
                if !converted_from.is_empty() {
                    wtr.write_record(&[
                        &ev.event_id,
                        &dt.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
                        &ev.lat.to_string(),
                        &ev.lon.to_string(),
                        &ev.depth_km.to_string(),
                        &mw_mag.to_string(),
                        &mw_type,
                        &ev.author,
                        &converted_from,
                        &ev.mag_type,
                    ]).map_err(|e| e.to_string())?;
                    row_count += 1;
                }
            }
        }
        wtr.flush().map_err(|e| e.to_string())?;
        println!("Saved {} valid processed rows to CSV.", row_count);
    }

    if let Some(out_dir) = cli.plot_stats {
        println!("Generating Statistical Visualization Plots in {} ...", out_dir.display());
        std::fs::create_dir_all(&out_dir).map_err(|e| format!("Failed to create plot output dir: {}", e))?;
        
        let path1 = out_dir.join("magnitude_catalog_visualization.png");
        let path2 = out_dir.join("distribusi_tipe_mag_piechart.png");

        crate::io::plotters_export::generate_magnitude_catalog_viz(&final_events, &path1)
            .map_err(|e| format!("Failed to generate catalog viz: {}", e))?;
        println!("  -> Saved: {}", path1.display());

        crate::io::plotters_export::generate_magnitude_piechart_viz(&final_events, &conversion_rules, &path2)
            .map_err(|e| format!("Failed to generate pie chart: {}", e))?;
        println!("  -> Saved: {}", path2.display());
    }

    if let Some(map_out) = cli.plot_map {
        println!("Generating Spatial Map in {} ...", map_out.display());
        
        let mut cs_track = None;
        if cli.plot_cs_track {
            if let (Some(lon1), Some(lat1), Some(lon2), Some(lat2)) = (cli.cs_start_lon, cli.cs_start_lat, cli.cs_end_lon, cli.cs_end_lat) {
                cs_track = Some(((lon1, lat1), (lon2, lat2)));
            } else {
                println!("Warning: --plot-cs-track requires --cs-start-lon/lat and --cs-end-lon/lat to be provided.");
            }
        }

        crate::io::plotters_export::generate_spatial_map_viz(&final_events, cs_track, &map_out)
            .map_err(|e| format!("Failed to generate spatial map: {}", e))?;
        println!("  -> Saved: {}", map_out.display());
    }

    // Cross-Section Processing
    if let (Some(lon1), Some(lat1), Some(lon2), Some(lat2)) = (cli.cs_start_lon, cli.cs_start_lat, cli.cs_end_lon, cli.cs_end_lat) {
        use seisbox_core::core::spatial::{GeoPoint, cross_section_projection};
        let p_start = GeoPoint::new(lat1, lon1);
        let p_end = GeoPoint::new(lat2, lon2);
        
        let mut cs_data = Vec::new();
        for ev in &final_events {
            let p_ev = GeoPoint::new(ev.lat, ev.lon);
            let (along, cross) = cross_section_projection(&p_start, &p_end, &p_ev);
            if cross.abs() <= cli.cs_buffer_km {
                cs_data.push((ev.clone(), along, cross));
            }
        }
        
        println!("Filtered {} events for Cross-Section (buffer: {} km).", cs_data.len(), cli.cs_buffer_km);

        if let Some(cs_csv) = cli.cs_out_csv {
            println!("Saving Cross-Section CSV to {} ...", cs_csv.display());
            let file = std::fs::File::create(&cs_csv).map_err(|e| format!("Failed to create CS CSV: {}", e))?;
            let mut wtr = csv::WriterBuilder::new().has_headers(true).from_writer(file);
            wtr.write_record(&["event_id", "date", "time", "latitude", "longitude", "depth", "magnitude", "magnitude_type", "along_track_km", "cross_track_km"]).map_err(|e| e.to_string())?;
            for (ev, along, cross) in &cs_data {
                wtr.write_record(&[
                    &ev.event_id,
                    &ev.date_str,
                    &ev.time_str,
                    &ev.lat.to_string(),
                    &ev.lon.to_string(),
                    &ev.depth_km.to_string(),
                    &ev.mag.to_string(),
                    &ev.mag_type,
                    &along.to_string(),
                    &cross.to_string(),
                ]).map_err(|e| e.to_string())?;
            }
            wtr.flush().map_err(|e| e.to_string())?;
            println!("  -> Saved: {}", cs_csv.display());
        }

        if let Some(cs_plot) = cli.plot_cross_section {
            println!("Generating Cross-Section Plot in {} ...", cs_plot.display());
            crate::io::plotters_export::generate_cross_section_viz(&cs_data, &cs_plot)
                .map_err(|e| format!("Failed to generate cross section plot: {}", e))?;
            println!("  -> Saved: {}", cs_plot.display());
        }
    }

    println!("All tasks completed successfully.");
    Ok(())
}
