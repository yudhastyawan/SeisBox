use eframe::egui;
use eframe::egui::{Color32, Ui, Window};
use egui_plot::{Plot, Points};
use chrono::{NaiveDate, NaiveTime};

use crate::app::QuakePickApp;
use crate::core::isc_client::{IscSearchParams, fetch_isc_catalog, IscResult};
use crate::core::spatial::{cross_section_projection, GeoPoint};
use crate::ui::spatial_map::{show_map, MapInteractionMode};

pub fn show_spatial_window(ctx: &egui::Context, app: &mut QuakePickApp) {
    let mut is_open = app.show_spatial_window;
    
    Window::new("🌍 ISC Catalog Search")
        .open(&mut is_open)
        .default_size([900.0, 700.0])
        .resizable(true)
        .show(ctx, |ui| {
            // Check for ISC results
            if let Some(rx) = &app.isc_receiver {
                if let Ok(result) = rx.try_recv() {
                    match result {
                        IscResult::Success(events, raw_txt) => {
                            app.isc_loading = false;
                            app.isc_receiver = None;
                            app.isc_events = events;
                            app.isc_raw_data = raw_txt;
                            app.status_msg = format!("Loaded {} events from ISC", app.isc_events.len());
                        },
                        IscResult::Error(err) => {
                            app.isc_loading = false;
                            app.isc_receiver = None;
                            app.status_msg = format!("ISC API Error: {}", err);
                        },
                        IscResult::Progress(msg) => {
                            app.status_msg = msg;
                        }
                    }
                }
            }

            ui.horizontal(|ui| {
                // Left Panel: Controls
                ui.vertical(|ui| {
                    ui.set_width(280.0);
                    
                    ui.heading("ISC Catalog Search");
                    ui.separator();
                    
                    egui::Grid::new("isc_search_grid").num_columns(2).show(ui, |ui| {
                        ui.label("Start Date (YYYY-MM-DD):"); ui.text_edit_singleline(&mut app.isc_start_date_str); ui.end_row();
                        ui.label("Start Time (HH:MM:SS):"); ui.text_edit_singleline(&mut app.isc_start_time_str); ui.end_row();
                        ui.label("End Date (YYYY-MM-DD):"); ui.text_edit_singleline(&mut app.isc_end_date_str); ui.end_row();
                        ui.label("End Time (HH:MM:SS):"); ui.text_edit_singleline(&mut app.isc_end_time_str); ui.end_row();
                        
                        ui.label("Min Depth (km):"); ui.add(egui::DragValue::new(&mut app.isc_min_depth)); ui.end_row();
                        ui.label("Max Depth (km):"); ui.add(egui::DragValue::new(&mut app.isc_max_depth)); ui.end_row();
                        ui.label("Min Mag:"); ui.add(egui::DragValue::new(&mut app.isc_min_mag)); ui.end_row();
                        ui.label("Max Mag:"); ui.add(egui::DragValue::new(&mut app.isc_max_mag)); ui.end_row();
                    });
                    
                    ui.add_space(10.0);
                    ui.label("Bounding Box:");
                    ui.horizontal(|ui| {
                        if ui.button("Draw BBox").clicked() {
                            app.map_state.interaction_mode = MapInteractionMode::DrawBBox;
                        }
                        ui.label(format!("{:.1},{:.1} to {:.1},{:.1}", 
                            app.map_state.bbox.bot_lat, app.map_state.bbox.left_lon,
                            app.map_state.bbox.top_lat, app.map_state.bbox.right_lon
                        ));
                    });

                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.button(if app.isc_loading { "Searching..." } else { "Search ISC" }).clicked() {
                            if !app.isc_loading {
                                let sd = NaiveDate::parse_from_str(&app.isc_start_date_str, "%Y-%m-%d");
                                let st = NaiveTime::parse_from_str(&app.isc_start_time_str, "%H:%M:%S");
                                let ed = NaiveDate::parse_from_str(&app.isc_end_date_str, "%Y-%m-%d");
                                let et = NaiveTime::parse_from_str(&app.isc_end_time_str, "%H:%M:%S");

                                if let (Ok(sd_ok), Ok(st_ok), Ok(ed_ok), Ok(et_ok)) = (sd, st, ed, et) {
                                    let params = IscSearchParams {
                                        bbox: app.map_state.bbox,
                                        start_date: sd_ok,
                                        start_time: st_ok,
                                        end_date: ed_ok,
                                        end_time: et_ok,
                                        min_depth: app.isc_min_depth,
                                        max_depth: app.isc_max_depth,
                                        min_mag: app.isc_min_mag,
                                        max_mag: app.isc_max_mag,
                                        mag_priority: app.mag_priority.clone(),
                                    };
                                    let (tx, rx) = std::sync::mpsc::channel();
                                    crate::core::isc_client::fetch_isc_catalog(params, tx);
                                    app.isc_receiver = Some(rx);
                                    app.isc_loading = true;
                                    app.status_msg = "Fetching ISC catalog...".to_string();
                                } else {
                                    app.status_msg = "Invalid date/time format".to_string();
                                }
                            }
                        }
                        
                        ui.add_space(15.0);
                        
                        egui::CollapsingHeader::new("⚙️ Magnitude Conversion & Priority Settings")
                            .default_open(true)
                            .show(ui, |ui| {
                                ui.heading("Magnitude Priority");
                                ui.label("Format: comma-separated list of prefixes (e.g. MW, MS, ML, MB)");
                                let mut prio_str = app.mag_priority.join(", ");
                                if ui.text_edit_singleline(&mut prio_str).changed() {
                                    app.mag_priority = prio_str.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
                                }
                                
                                ui.add_space(15.0);
                                ui.heading("Conversion Rules to Mw");
                                ui.label("Format: Mw = Multiplier * Mag + Offset");
                                
                                let mut to_remove = None;
                                
                                egui::ScrollArea::horizontal().id_source("conv_rules_scroll").show(ui, |ui| {
                                    egui::Grid::new("rules_grid").striped(true).show(ui, |ui| {
                                        ui.label("Source Prefix");
                                        ui.label("Min Mag");
                                        ui.label("Max Mag");
                                        ui.label("Multiplier (a)");
                                        ui.label("Offset (b)");
                                        ui.label("Action");
                                        ui.end_row();
                                        
                                        for (i, rule) in app.conversion_rules.iter_mut().enumerate() {
                                            ui.text_edit_singleline(&mut rule.source_type);
                                            ui.add(egui::DragValue::new(&mut rule.min_mag).speed(0.1));
                                            ui.add(egui::DragValue::new(&mut rule.max_mag).speed(0.1));
                                            ui.add(egui::DragValue::new(&mut rule.multiplier).speed(0.01));
                                            ui.add(egui::DragValue::new(&mut rule.offset).speed(0.01));
                                            if ui.button("❌").clicked() {
                                                to_remove = Some(i);
                                            }
                                            ui.end_row();
                                        }
                                    });
                                });
                                
                                if let Some(idx) = to_remove {
                                    app.conversion_rules.remove(idx);
                                }
                                
                                ui.add_space(5.0);
                                if ui.button("➕ Add Rule").clicked() {
                                    let new_id = app.conversion_rules.iter().map(|r| r.id).max().unwrap_or(0) + 1;
                                    app.conversion_rules.push(crate::core::isc_client::ConversionRule {
                                        id: new_id,
                                        source_type: "XX".to_string(),
                                        min_mag: -99.0,
                                        max_mag: 99.0,
                                        multiplier: 1.0,
                                        offset: 0.0,
                                    });
                                }
                                ui.add_space(10.0);
                            });
                    });
                    
                    if !app.isc_events.is_empty() {
                        ui.add_space(10.0);
                        ui.label("Export Data:");
                        ui.horizontal(|ui| {
                            if ui.button("Raw TXT").clicked() {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("Text", &["txt"])
                                    .set_file_name("isc_raw_data.txt")
                                    .save_file() 
                                {
                                    if let Err(e) = std::fs::write(&path, &app.isc_raw_data) {
                                        app.status_msg = format!("Error saving TXT: {}", e);
                                    } else {
                                        app.status_msg = format!("Saved {}", path.display());
                                    }
                                }
                            }
                            if ui.button("Processed CSV").clicked() {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("CSV", &["csv"])
                                    .set_file_name("isc_processed.csv")
                                    .save_file() 
                                {
                                    if let Ok(mut wtr) = csv::Writer::from_path(&path) {
                                        let _ = wtr.write_record(&["event_id", "time", "latitude", "longitude", "depth", "magnitude", "magnitude_type", "author", "converted_from", "magnitude_real"]);
                                        for ev in &app.isc_events {
                                            let dt = chrono::DateTime::from_timestamp_millis((ev.timestamp * 1000.0) as i64).unwrap();
                                            let (mw_mag, mw_type, converted_from) = crate::core::isc_client::apply_conversion(&app.conversion_rules, ev.mag, &ev.mag_type);
                                            // Only include if it has a valid converted_from (this mimics the filter in python)
                                            if !converted_from.is_empty() {
                                                let _ = wtr.write_record(&[
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
                                                ]);
                                            }
                                        }
                                        let _ = wtr.flush();
                                        app.status_msg = format!("Saved {}", path.display());
                                    } else {
                                        app.status_msg = "Error creating CSV file".to_string();
                                    }
                                }
                            }
                        });
                        ui.add_space(5.0);
                        if ui.button("📊 Show Statistical Visualization").clicked() {
                            let temp_path1 = std::env::temp_dir().join("magnitude_catalog_visualization.png");
                            let temp_path2 = std::env::temp_dir().join("distribusi_tipe_mag_piechart.png");

                            if let Ok(_) = crate::io::plotters_export::generate_magnitude_catalog_viz(&app.isc_events, &temp_path1) {
                                if let Ok(img) = image::open(&temp_path1) {
                                    let size = [img.width() as _, img.height() as _];
                                    let img_buf = img.to_rgba8();
                                    let pixels = img_buf.as_flat_samples();
                                    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
                                    app.spatial_viz_texture = Some(ctx.load_texture("catalog_viz", color_image, egui::TextureOptions::LINEAR));
                                }
                            }
                            if let Ok(_) = crate::io::plotters_export::generate_magnitude_piechart_viz(&app.isc_events, &app.conversion_rules, &temp_path2) {
                                if let Ok(img) = image::open(&temp_path2) {
                                    let size = [img.width() as _, img.height() as _];
                                    let img_buf = img.to_rgba8();
                                    let pixels = img_buf.as_flat_samples();
                                    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
                                    app.spatial_viz_texture2 = Some(ctx.load_texture("pie_viz", color_image, egui::TextureOptions::LINEAR));
                                }
                            }
                            app.show_spatial_viz = true;
                        }
                    }

                    ui.add_space(20.0);
                    ui.heading("Cross Section");
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Draw Line A-B").clicked() {
                            app.map_state.interaction_mode = MapInteractionMode::DrawCrossSection;
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Buffer (km):");
                        ui.add(egui::DragValue::new(&mut app.map_state.cross_section_buffer_km));
                    });
                    if let Some(cs) = &app.map_state.cross_section {
                        ui.label(format!("A: {:.2},{:.2}\nB: {:.2},{:.2}", cs.point_a.lat, cs.point_a.lon, cs.point_b.lat, cs.point_b.lon));
                    }
                });
                
                ui.separator();
                
                // Right Panel: Map
                ui.vertical(|ui| {
                    let map_data = app.map_data.as_ref().unwrap();
                    crate::ui::spatial_map::show_map(ui, map_data, &mut app.map_state, &app.isc_events, None);
                });
            });
            
            ui.separator();
            
            // Bottom Panels: Mag vs Time & Cross Section Plot
            ui.columns(2, |cols| {
                cols[0].vertical(|ui| {
                    ui.heading("Magnitude vs Time");
                    
                    let mut plot = Plot::new("mag_time_plot")
                        .x_axis_formatter(|mark, _range| {
                            let x = mark.value;
                            if let Some(dt) = chrono::DateTime::from_timestamp_millis((x * 1000.0) as i64) {
                                dt.format("%Y-%m-%d").to_string()
                            } else {
                                String::new()
                            }
                        })
                        .show_axes([true, true])
                        .show_grid(true);
                        
                    plot.show(ui, |plot_ui| {
                        let pts: Vec<[f64; 2]> = app.isc_events.iter().map(|e| [e.timestamp, e.mag]).collect();
                        plot_ui.points(Points::new(pts).color(Color32::ORANGE).radius(3.0));
                    });
                });
                
                cols[1].vertical(|ui| {
                    ui.heading("Cross Section (Distance vs Depth)");
                    
                    let mut plot = Plot::new("cross_section_plot")
                        // Invert Y axis conceptually by plotting negative depth, then labeling it positive
                        .y_axis_formatter(|mark, _range| format!("{:.1}", mark.value.abs()))
                        .show_axes([true, true])
                        .show_grid(true);
                        
                    plot.show(ui, |plot_ui| {
                        if let Some(cs) = &app.map_state.cross_section {
                            let mut filtered_pts = Vec::new();
                            for e in &app.isc_events {
                                let p = GeoPoint::new(e.lat, e.lon);
                                let (along, cross) = cross_section_projection(&cs.point_a, &cs.point_b, &p);
                                if cross <= cs.buffer_km {
                                    // Plot depth as negative so 0 is at top
                                    filtered_pts.push([along, -e.depth_km]);
                                }
                            }
                            plot_ui.points(Points::new(filtered_pts).color(Color32::RED).radius(3.0));
                        }
                    });
                });
            });
        });

    app.show_spatial_window = is_open;

    if app.show_spatial_viz {
        let mut viz_open = true;
        egui::Window::new("📊 ISC Statistical Visualization")
            .open(&mut viz_open)
            .default_size([900.0, 600.0])
            .resizable(true)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if app.spatial_viz_texture.is_some() && ui.button("💾 Save Catalog Viz").clicked() {
                        if let Some(path) = rfd::FileDialog::new().add_filter("PNG Image", &["png"]).set_file_name("magnitude_catalog_visualization.png").save_file() {
                            let temp_path = std::env::temp_dir().join("magnitude_catalog_visualization.png");
                            let _ = std::fs::copy(&temp_path, &path);
                        }
                    }
                    if app.spatial_viz_texture2.is_some() && ui.button("💾 Save Pie Chart").clicked() {
                        if let Some(path) = rfd::FileDialog::new().add_filter("PNG Image", &["png"]).set_file_name("distribusi_tipe_mag_piechart.png").save_file() {
                            let temp_path = std::env::temp_dir().join("distribusi_tipe_mag_piechart.png");
                            let _ = std::fs::copy(&temp_path, &path);
                        }
                    }
                });
                ui.add_space(10.0);
                
                egui::ScrollArea::both().show(ui, |ui| {
                    if let Some(tex) = &app.spatial_viz_texture {
                        ui.image(tex);
                        ui.add_space(20.0);
                    }
                    if let Some(tex2) = &app.spatial_viz_texture2 {
                        ui.image(tex2);
                    }
                });
            });
        
        if !viz_open {
            app.show_spatial_viz = false;
        }
    }
    
}
