use eframe::egui::{self, Color32};
use egui_extras::{TableBuilder, Column};
use egui_plot::{PlotPoints, Points};
use chrono::{NaiveDate, NaiveTime, NaiveDateTime, Duration};
use std::sync::mpsc::Receiver;

use crate::core::fdsn::api::{FdsnStation, FdsnResult, FdsnSearchParams, FdsnDownloadParams};
use crate::ui::spatial_map::MapData;

#[derive(Clone)]
pub struct ProviderSelection {
    pub name: String,
    pub url: String,
    pub desc: String,
    pub selected: bool,
}

pub struct FdsnState {
    pub is_open: bool,
    pub map_state: crate::ui::spatial_map::MapState,
    // Input state
    pub providers: Vec<ProviderSelection>,
    pub ref_lat: String,
    pub ref_lon: String,
    pub ref_date: String,
    pub ref_time: String,
    pub min_radius: String,
    pub max_radius: String,
    pub channel: String,
    pub start_offset_sec: String,
    pub end_offset_sec: String,
    
    // Background task state
    pub receiver: Option<Receiver<FdsnResult>>,
    pub is_searching: bool,
    pub is_downloading: bool,
    pub is_downloading_xml: bool,
    pub status_msg: String,
    pub progress_msg: String,
    
    // Data
    pub stations: Vec<FdsnStation>,
    pub downloaded_count: usize,
}

impl Default for FdsnState {
    fn default() -> Self {
        let providers = vec![
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
        ].into_iter().map(|(name, url, desc)| ProviderSelection {
            name: name.to_string(),
            url: url.to_string(),
            desc: desc.to_string(),
            selected: name == "IRIS",
        }).collect();

        let mut map_state = crate::ui::spatial_map::MapState::default();
        map_state.bbox = crate::core::spatial::BoundingBox {
            bot_lat: std::f64::MAX,
            top_lat: std::f64::MIN,
            left_lon: std::f64::MAX,
            right_lon: std::f64::MIN,
        };

        Self {
            is_open: false,
            map_state,
            providers,
            ref_lat: "-7.0".to_string(),
            ref_lon: "107.0".to_string(),
            ref_date: "2024-01-01".to_string(),
            ref_time: "00:00:00".to_string(),
            min_radius: "0.0".to_string(),
            max_radius: "5.0".to_string(),
            channel: "BHZ,HHZ".to_string(),
            start_offset_sec: "-60".to_string(),
            end_offset_sec: "600".to_string(),
            
            receiver: None,
            is_searching: false,
            is_downloading: false,
            is_downloading_xml: false,
            status_msg: "".to_string(),
            progress_msg: "".to_string(),
            stations: Vec::new(),
            downloaded_count: 0,
        }
    }
}

pub fn show_fdsn_dialog(ctx: &egui::Context, state: &mut FdsnState, map_data: &Option<MapData>) {
    if !state.is_open {
        return;
    }

    // Process background messages
    if let Some(rx) = &state.receiver {
        while let Ok(msg) = rx.try_recv() {
            match msg {
                FdsnResult::Progress(p) => state.progress_msg = p,
                FdsnResult::Error(e) => {
                    state.status_msg = e;
                    state.is_searching = false;
                    state.is_downloading = false;
                    state.is_downloading_xml = false;
                    state.progress_msg.clear();
                },
                FdsnResult::StationsFound(stations) => {
                    state.stations = stations;
                    state.status_msg = format!("Found {} stations.", state.stations.len());
                    state.is_searching = false;
                    state.progress_msg.clear();
                },
                FdsnResult::WaveformDownloaded(net, sta, _path) => {
                    state.downloaded_count += 1;
                    state.status_msg = format!("Downloaded {}.{}", net, sta);
                },
                FdsnResult::ResponseDownloaded(net, sta, _path) => {
                    state.downloaded_count += 1;
                    state.status_msg = format!("Downloaded XML for {}.{}", net, sta);
                },
                FdsnResult::WaveformDownloadsComplete => {
                    state.is_downloading = false;
                    state.status_msg = format!("Downloads complete. Total: {}", state.downloaded_count);
                    state.progress_msg = "Done".to_string();
                },
                FdsnResult::ResponseDownloadsComplete => {
                    state.is_downloading_xml = false;
                    state.status_msg = format!("StationXML Downloads complete. Total: {}", state.downloaded_count);
                    state.progress_msg = "Done".to_string();
                },
            }
        }
    }

    let mut is_open = state.is_open;
    egui::Window::new("FDSN Data Downloader")
        .open(&mut is_open)
        .default_size([900.0, 700.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Left Panel: Inputs
                ui.vertical(|ui| {
                    ui.set_width(300.0);
                    ui.group(|ui| {
                        ui.label(egui::RichText::new("Data Providers").strong());
                        egui::ScrollArea::vertical().max_height(100.0).show(ui, |ui| {
                            let mut all_selected = state.providers.iter().all(|p| p.selected);
                            if ui.checkbox(&mut all_selected, "Select All").changed() {
                                for p in &mut state.providers {
                                    p.selected = all_selected;
                                }
                            }
                            ui.separator();
                            let columns = 3;
                            egui::Grid::new("providers_grid").num_columns(columns).show(ui, |ui| {
                                for (i, p) in state.providers.iter_mut().enumerate() {
                                    let tooltip = format!("{}\nURL: {}", p.desc, p.url);
                                    ui.checkbox(&mut p.selected, &p.name).on_hover_text(tooltip);
                                    if (i + 1) % columns == 0 {
                                        ui.end_row();
                                    }
                                }
                            });
                        });
                    });

                    ui.add_space(10.0);
                    ui.heading("Reference Earthquake");
                    egui::Grid::new("fdsn_eq_grid").num_columns(2).show(ui, |ui| {
                        ui.label("Latitude"); ui.text_edit_singleline(&mut state.ref_lat); ui.end_row();
                        ui.label("Longitude"); ui.text_edit_singleline(&mut state.ref_lon); ui.end_row();
                        ui.label("Origin Date (Y-m-d)"); ui.text_edit_singleline(&mut state.ref_date); ui.end_row();
                        ui.label("Origin Time (H:M:S)"); ui.text_edit_singleline(&mut state.ref_time); ui.end_row();
                    });
                    
                    ui.add_space(10.0);
                    ui.heading("Station Parameters");
                    egui::Grid::new("fdsn_sta_grid").num_columns(2).show(ui, |ui| {
                        ui.label("Min Radius (deg)"); ui.text_edit_singleline(&mut state.min_radius); ui.end_row();
                        ui.label("Max Radius (deg)"); ui.text_edit_singleline(&mut state.max_radius); ui.end_row();
                        ui.label("Channel"); ui.text_edit_singleline(&mut state.channel); ui.end_row();
                    });
                    
                    ui.add_space(10.0);
                    ui.heading("Time Parameters");
                    egui::Grid::new("fdsn_time_grid").num_columns(2).show(ui, |ui| {
                        ui.label("Start Offset (s)"); ui.text_edit_singleline(&mut state.start_offset_sec); ui.end_row();
                        ui.label("End Offset (s)"); ui.text_edit_singleline(&mut state.end_offset_sec); ui.end_row();
                    });
                    
                    ui.add_space(20.0);
                    ui.vertical(|ui| {
                        if ui.add_enabled(!state.is_searching && !state.is_downloading, egui::Button::new("🔍 Search Stations")).clicked() {
                            let lat = state.ref_lat.parse::<f64>().unwrap_or(0.0);
                            let lon = state.ref_lon.parse::<f64>().unwrap_or(0.0);
                            let min_r = state.min_radius.parse::<f64>().unwrap_or(0.0);
                            let max_r = state.max_radius.parse::<f64>().unwrap_or(10.0);
                            
                            let date_str = format!("{}T{}", state.ref_date, state.ref_time);
                            if let Ok(st) = NaiveDateTime::parse_from_str(&date_str, "%Y-%m-%dT%H:%M:%S") {
                                let mut params_list = Vec::new();
                                for p in &state.providers {
                                    if p.selected {
                                        params_list.push(FdsnSearchParams {
                                            name: p.name.clone(),
                                            url: p.url.clone(),
                                            lat,
                                            lon,
                                            min_radius: min_r,
                                            max_radius: max_r,
                                            start_time: st + Duration::seconds(state.start_offset_sec.parse().unwrap_or(-60)),
                                            end_time: st + Duration::seconds(state.end_offset_sec.parse().unwrap_or(600)),
                                            channel: state.channel.clone(),
                                        });
                                    }
                                }
                                
                                if params_list.is_empty() {
                                    state.status_msg = "Error: No data provider selected.".to_string();
                                } else {
                                    state.is_searching = true;
                                    state.stations.clear();
                                    let (tx, rx) = std::sync::mpsc::channel();
                                    state.receiver = Some(rx);
                                    crate::core::fdsn::downloader::search_stations(params_list, tx);
                                }
                            } else {
                                state.status_msg = "Invalid Date/Time format".to_string();
                            }
                        }
                        
                        ui.add_space(5.0);
                        
                        if ui.add_enabled(!state.stations.is_empty() && !state.is_downloading && !state.is_downloading_xml && !state.is_searching, egui::Button::new("⬇️ Download Waveforms")).clicked() {
                            let date_str = format!("{}T{}", state.ref_date, state.ref_time);
                            if let Ok(st) = NaiveDateTime::parse_from_str(&date_str, "%Y-%m-%dT%H:%M:%S") {
                                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                                    let mut params_list = Vec::new();
                                    for s in &state.stations {
                                        params_list.push(FdsnDownloadParams {
                                            provider_name: s.provider_name.clone(),
                                            url: s.provider_url.clone(),
                                            network: s.network.clone(),
                                            station: s.station.clone(),
                                            channel: state.channel.clone(),
                                            start_time: st + Duration::seconds(state.start_offset_sec.parse().unwrap_or(-60)),
                                            end_time: st + Duration::seconds(state.end_offset_sec.parse().unwrap_or(600)),
                                            output_dir: path.to_path_buf(),
                                        });
                                    }
                                    
                                    let (tx, rx) = std::sync::mpsc::channel();
                                    crate::core::fdsn::downloader::download_waveforms(params_list, tx);
                                    state.receiver = Some(rx);
                                    state.is_downloading = true;
                                    state.downloaded_count = 0;
                                    state.status_msg = "Starting downloads...".to_string();
                                }
                            }
                        }
                        
                        ui.add_space(5.0);
                        
                        if ui.add_enabled(!state.stations.is_empty() && !state.is_downloading && !state.is_downloading_xml && !state.is_searching, egui::Button::new("📥 Download StationXML")).clicked() {
                            let date_str = format!("{}T{}", state.ref_date, state.ref_time);
                            if let Ok(st) = NaiveDateTime::parse_from_str(&date_str, "%Y-%m-%dT%H:%M:%S") {
                                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                                    let mut params_list = Vec::new();
                                    for s in &state.stations {
                                        params_list.push(FdsnDownloadParams {
                                            provider_name: s.provider_name.clone(),
                                            url: s.provider_url.clone(),
                                            network: s.network.clone(),
                                            station: s.station.clone(),
                                            channel: state.channel.clone(),
                                            start_time: st + Duration::seconds(state.start_offset_sec.parse().unwrap_or(-60)),
                                            end_time: st + Duration::seconds(state.end_offset_sec.parse().unwrap_or(600)),
                                            output_dir: path.to_path_buf(),
                                        });
                                    }
                                    
                                    // Save CSV of station info
                                    let mut csv_content = String::from("Provider,Network,Station,Latitude,Longitude,Elevation,Site Name\n");
                                    for s in &state.stations {
                                        let clean_site_name = s.site_name.replace(",", " ");
                                        csv_content.push_str(&format!("{},{},{},{},{},{},{}\n", 
                                            s.provider_name, s.network, s.station, s.lat, s.lon, s.elevation, clean_site_name));
                                    }
                                    let csv_path = path.join("stations.csv");
                                    let _ = std::fs::write(&csv_path, csv_content);
                                    
                                    let (tx, rx) = std::sync::mpsc::channel();
                                    crate::core::fdsn::downloader::download_station_xml(params_list, tx);
                                    state.receiver = Some(rx);
                                    state.is_downloading_xml = true;
                                    state.downloaded_count = 0;
                                    state.status_msg = "Starting StationXML downloads...".to_string();
                                }
                            }
                        }
                    });
                });
                
                ui.separator();
                
                // Right Panel: Map
                ui.vertical(|ui| {
                    if let Some(md) = map_data {
                        let draw_extras = |plot_ui: &mut egui_plot::PlotUi| {
                            // Draw EQ Ref (Red Star)
                            if let (Ok(lat), Ok(lon)) = (state.ref_lat.parse::<f64>(), state.ref_lon.parse::<f64>()) {
                                plot_ui.points(
                                    Points::new(vec![[lon, lat]])
                                        .color(Color32::RED)
                                        .radius(8.0)
                                        .shape(egui_plot::MarkerShape::Asterisk) // Close enough to a star
                                );
                                
                                // Draw radius circles
                                let min_r = state.min_radius.parse::<f64>().unwrap_or(0.0);
                                let max_r = state.max_radius.parse::<f64>().unwrap_or(0.0);
                                
                                let mut draw_circle = |r: f64, color: Color32| {
                                    if r > 0.0 {
                                        let num_pts = 64;
                                        let mut circle_pts = Vec::with_capacity(num_pts + 1);
                                        let cos_lat = lat.to_radians().cos().max(0.01);
                                        for i in 0..=num_pts {
                                            let angle = (i as f64) * std::f64::consts::TAU / (num_pts as f64);
                                            let d_lon = r * angle.cos() / cos_lat;
                                            let d_lat = r * angle.sin();
                                            circle_pts.push([lon + d_lon, lat + d_lat]);
                                        }
                                        plot_ui.line(egui_plot::Line::new(PlotPoints::new(circle_pts)).color(color).width(1.5));
                                    }
                                };
                                
                                draw_circle(min_r, Color32::from_rgb(100, 100, 255));
                                draw_circle(max_r, Color32::from_rgb(100, 255, 100));
                            }
                            
                            // Draw stations (Blue Triangle/Diamond)
                            if !state.stations.is_empty() {
                                let pts: Vec<[f64; 2]> = state.stations.iter().map(|s| [s.lon, s.lat]).collect();
                                plot_ui.points(
                                    Points::new(pts)
                                        .color(Color32::LIGHT_BLUE)
                                        .radius(5.0)
                                        .shape(egui_plot::MarkerShape::Diamond)
                                );
                            }
                        };
                        
                        // For FDSN, we just want panning and zooming, no selection box
                        state.map_state.interaction_mode = crate::ui::spatial_map::MapInteractionMode::None;
                        
                        // Empty events for the generic show_map
                        crate::ui::spatial_map::show_map(ui, md, &mut state.map_state, &[], Some(&draw_extras));
                    }
                });
            });
            
            ui.separator();
            
            // Bottom Panel: Progress and Table
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(&state.status_msg).strong());
                if state.is_searching || state.is_downloading || state.is_downloading_xml {
                    ui.spinner();
                }
                ui.label(&state.progress_msg);
                
                if (state.is_downloading || state.is_downloading_xml) && !state.stations.is_empty() {
                    let pct = state.downloaded_count as f32 / state.stations.len() as f32;
                    ui.add(egui::ProgressBar::new(pct).show_percentage());
                }
            });
            
            ui.add_space(5.0);
            
            TableBuilder::new(ui)
                .striped(true)
                .resizable(true)
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                .column(Column::auto()) // Provider
                .column(Column::auto()) // Net
                .column(Column::auto()) // Sta
                .column(Column::auto()) // Lat
                .column(Column::auto()) // Lon
                .column(Column::auto()) // Elev
                .column(Column::remainder()) // SiteName
                .header(20.0, |mut header| {
                    header.col(|ui| { ui.strong("Provider"); });
                    header.col(|ui| { ui.strong("Network"); });
                    header.col(|ui| { ui.strong("Station"); });
                    header.col(|ui| { ui.strong("Latitude"); });
                    header.col(|ui| { ui.strong("Longitude"); });
                    header.col(|ui| { ui.strong("Elevation"); });
                    header.col(|ui| { ui.strong("Site Name"); });
                })
                .body(|mut body| {
                    for sta in &state.stations {
                        body.row(18.0, |mut row| {
                            row.col(|ui| { ui.label(&sta.provider_name); });
                            row.col(|ui| { ui.label(&sta.network); });
                            row.col(|ui| { ui.label(&sta.station); });
                            row.col(|ui| { ui.label(format!("{:.4}", sta.lat)); });
                            row.col(|ui| { ui.label(format!("{:.4}", sta.lon)); });
                            row.col(|ui| { ui.label(format!("{:.1}", sta.elevation)); });
                            row.col(|ui| { ui.label(&sta.site_name); });
                        });
                    }
                });
        });
        
    state.is_open = is_open;
}
