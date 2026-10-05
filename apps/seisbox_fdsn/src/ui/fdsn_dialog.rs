use eframe::egui::{self, Color32};
use egui_extras::{TableBuilder, Column};
use chrono::{NaiveDateTime, Duration};
use std::sync::mpsc::Receiver;
use std::path::PathBuf;

use crate::core::fdsn::api::{FdsnStation, FdsnResult, FdsnSearchParams, FdsnDownloadParams};
use seisbox_core::ui::spatial_map::MapData;
use seisbox_core::io::file_sync::{FileNode,  is_seismic_file};

#[derive(PartialEq, Clone, Copy)]
pub enum ActiveTab {
    Explorer,
    Search,
    Settings,
}

#[derive(Clone, PartialEq)]
pub enum EditorTab {
    MapView,
    StationList,
    Logs,
}

#[derive(Clone)]
pub struct ProviderSelection {
    pub name: String,
    pub url: String,
    pub desc: String,
    pub selected: bool,
}

pub struct FdsnState {
    pub is_open: bool,
    pub active_tab: ActiveTab,
    pub open_tabs: Vec<EditorTab>,
    pub active_tab_index: usize,

    pub map_state: seisbox_core::ui::spatial_map::MapState,
    
    // Providers
    pub providers: Vec<ProviderSelection>,
    
    // Spatial Mode
    pub use_bounding_box: bool,
    pub ref_lat: String,
    pub ref_lon: String,
    pub min_radius: String,
    pub max_radius: String,
    pub min_lat: String,
    pub max_lat: String,
    pub min_lon: String,
    pub max_lon: String,
    
    // Time Mode
    pub use_absolute_time: bool,
    pub ref_date: String,
    pub ref_time: String,
    pub start_offset_sec: String,
    pub end_offset_sec: String,
    pub start_date: String,
    pub start_time_abs: String,
    pub end_date: String,
    pub end_time_abs: String,
    
    // Filters
    pub channel: String,
    pub network: String,
    pub station: String,
    pub min_mag: String,
    pub max_mag: String,
    
    // Settings & Download
    pub out_dir: String,
    pub download_xml: bool,
    pub download_events: bool,
    pub download_arrivals: bool,
    pub export_sac: bool,
    pub no_mseed: bool,
    
    // Explorer
    pub explorer_dir: Option<PathBuf>,
    pub file_tree: Option<FileNode>,
    
    // Background task state
    pub receiver: Option<Receiver<FdsnResult>>,
    pub is_searching: bool,
    pub is_downloading: bool,
    pub is_downloading_xml: bool,
    pub status_msg: String,
    pub progress_msg: String,
    pub logs: Vec<String>,
    
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

        let mut map_state = seisbox_core::ui::spatial_map::MapState::default();
        map_state.bbox = seisbox_core::core::spatial::BoundingBox {
            bot_lat: std::f64::MAX,
            top_lat: std::f64::MIN,
            left_lon: std::f64::MAX,
            right_lon: std::f64::MIN,
        };

        Self {
            is_open: true,
            active_tab: ActiveTab::Search,
            open_tabs: vec![EditorTab::MapView, EditorTab::StationList, EditorTab::Logs],
            active_tab_index: 0,
            map_state,
            providers,
            use_bounding_box: false,
            ref_lat: "-7.0".to_string(),
            ref_lon: "107.0".to_string(),
            min_radius: "0.0".to_string(),
            max_radius: "5.0".to_string(),
            min_lat: "-10.0".to_string(),
            max_lat: "10.0".to_string(),
            min_lon: "90.0".to_string(),
            max_lon: "120.0".to_string(),
            
            use_absolute_time: false,
            ref_date: "2024-01-01".to_string(),
            ref_time: "00:00:00".to_string(),
            start_offset_sec: "-60".to_string(),
            end_offset_sec: "600".to_string(),
            start_date: "2024-01-01".to_string(),
            start_time_abs: "00:00:00".to_string(),
            end_date: "2024-01-01".to_string(),
            end_time_abs: "00:10:00".to_string(),
            
            channel: "BHZ,HHZ".to_string(),
            network: "".to_string(),
            station: "".to_string(),
            min_mag: "".to_string(),
            max_mag: "".to_string(),
            
            out_dir: "./fdsn_data".to_string(),
            download_xml: false,
            download_events: false,
            download_arrivals: false,
            export_sac: false,
            no_mseed: false,
            
            explorer_dir: Some(PathBuf::from("./fdsn_data")),
            file_tree: None,
            
            receiver: None,
            is_searching: false,
            is_downloading: false,
            is_downloading_xml: false,
            status_msg: "".to_string(),
            progress_msg: "".to_string(),
            logs: Vec::new(),
            stations: Vec::new(),
            downloaded_count: 0,
        }
    }
}

pub fn show_fdsn_panel(ctx: &egui::Context, state: &mut FdsnState, map_data: &Option<MapData>) {
    let mut is_open = state.is_open;
    if !is_open {
        return;
    }

    // Process background messages
    if let Some(rx) = &state.receiver {
        while let Ok(msg) = rx.try_recv() {
            match msg {
                FdsnResult::Progress(p) => {
                    state.progress_msg = p.clone();
                    state.logs.push(format!("[Progress] {}", p));
                },
                FdsnResult::Error(e) => {
                    state.status_msg = e.clone();
                    state.logs.push(format!("[Error] {}", e));
                    state.is_searching = false;
                    state.is_downloading = false;
                    state.is_downloading_xml = false;
                    state.progress_msg.clear();
                },
                FdsnResult::StationsFound(stations) => {
                    state.stations = stations;
                    state.status_msg = format!("Found {} stations.", state.stations.len());
                    state.logs.push(format!("[Success] Found {} stations.", state.stations.len()));
                    state.is_searching = false;
                    state.progress_msg.clear();
                    
                    // Switch to Station List
                    if !state.open_tabs.contains(&EditorTab::StationList) {
                        state.open_tabs.push(EditorTab::StationList);
                    }
                    if let Some(idx) = state.open_tabs.iter().position(|t| t == &EditorTab::StationList) {
                        state.active_tab_index = idx;
                    }
                },
                FdsnResult::WaveformDownloaded(net, sta, path) => {
                    state.downloaded_count += 1;
                    state.status_msg = format!("Downloaded {}.{}", net, sta);
                    state.logs.push(format!("[Downloaded] Waveform {}.{} -> {:?}", net, sta, path));
                },
                FdsnResult::ResponseDownloaded(net, sta, path) => {
                    state.downloaded_count += 1;
                    state.status_msg = format!("Downloaded XML for {}.{}", net, sta);
                    state.logs.push(format!("[Downloaded] XML {}.{} -> {:?}", net, sta, path));
                },
                FdsnResult::WaveformDownloadsComplete => {
                    state.is_downloading = false;
                    state.status_msg = format!("Downloads complete. Total: {}", state.downloaded_count);
                    state.progress_msg = "Done".to_string();
                    state.logs.push("[Complete] All waveform downloads finished.".to_string());
                },
                FdsnResult::ResponseDownloadsComplete => {
                    state.is_downloading_xml = false;
                    state.status_msg = format!("StationXML Downloads complete. Total: {}", state.downloaded_count);
                    state.progress_msg = "Done".to_string();
                    state.logs.push("[Complete] All StationXML downloads finished.".to_string());
                },
                FdsnResult::EventsDownloaded(count) => {
                    state.status_msg = format!("Downloaded {} events.", count);
                    state.logs.push(format!("[Complete] Event download finished. {} events.", count));
                },
            }
        }
    }

    show_activity_bar(ctx, state);
    show_sidebar(ctx, state);
    show_central_view(ctx, state, map_data);

    state.is_open = is_open;
}

fn show_activity_bar(ctx: &egui::Context, state: &mut FdsnState) {
    egui::SidePanel::left("fdsn_activity_bar")
        .resizable(false)
        .exact_width(45.0)
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(10.0);
                
                let tab_size = egui::vec2(35.0, 35.0);
                
                let mut files_btn = egui::Button::new(egui::RichText::new("📁").size(20.0)).min_size(tab_size);
                if state.active_tab == ActiveTab::Explorer { files_btn = files_btn.fill(ui.visuals().selection.bg_fill); }
                if ui.add(files_btn).on_hover_text("Explorer").clicked() {
                    state.active_tab = ActiveTab::Explorer;
                }
                
                ui.add_space(5.0);
                
                let mut search_btn = egui::Button::new(egui::RichText::new("🔍").size(20.0)).min_size(tab_size);
                if state.active_tab == ActiveTab::Search { search_btn = search_btn.fill(ui.visuals().selection.bg_fill); }
                if ui.add(search_btn).on_hover_text("Search & Query").clicked() {
                    state.active_tab = ActiveTab::Search;
                }
                
                ui.add_space(5.0);
                
                let mut settings_btn = egui::Button::new(egui::RichText::new("⚙").size(20.0)).min_size(tab_size);
                if state.active_tab == ActiveTab::Settings { settings_btn = settings_btn.fill(ui.visuals().selection.bg_fill); }
                if ui.add(settings_btn).on_hover_text("Settings & Download").clicked() {
                    state.active_tab = ActiveTab::Settings;
                }
            });
        });
}

fn show_sidebar(ctx: &egui::Context, state: &mut FdsnState) {
    egui::SidePanel::left("fdsn_sidebar")
        .resizable(true)
        .min_width(320.0)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add_space(10.0);
                match state.active_tab {
                    ActiveTab::Explorer => show_explorer_tab(ui, state),
                    ActiveTab::Search => show_search_tab(ui, state),
                    ActiveTab::Settings => show_settings_tab(ui, state),
                }
            });
        });
}

fn show_explorer_tab(ui: &mut egui::Ui, state: &mut FdsnState) {
    ui.heading("📂 Explorer");
    ui.separator();
    
    ui.horizontal(|ui| {
        if ui.button("Refresh").clicked() {
            if let Some(dir) = &state.explorer_dir {
                state.file_tree = Some(FileNode::scan_dir(dir).ok().unwrap_or_else(|| FileNode { path: dir.to_path_buf(), name: dir.file_name().unwrap_or_default().to_string_lossy().to_string(), is_dir: true, children: vec![] }));
            }
        }
        if ui.button("Set Dir").clicked() {
            let path = rfd::FileDialog::new().pick_folder();
            if let Some(p) = path {
                state.explorer_dir = Some(p.clone());
                state.file_tree = Some(FileNode::scan_dir(&p).ok().unwrap_or_else(|| FileNode { path: p.clone(), name: p.file_name().unwrap_or_default().to_string_lossy().to_string(), is_dir: true, children: vec![] }));
            }
        }
    });
    
    if state.file_tree.is_none() {
        if let Some(dir) = &state.explorer_dir {
            state.file_tree = Some(FileNode::scan_dir(dir).ok().unwrap_or_else(|| FileNode { path: dir.to_path_buf(), name: dir.file_name().unwrap_or_default().to_string_lossy().to_string(), is_dir: true, children: vec![] }));
        }
    }
    
    ui.separator();
    if let Some(tree) = &state.file_tree {
        render_file_tree(ui, tree);
    }
}

fn render_file_tree(ui: &mut egui::Ui, node: &FileNode) {
    if node.is_dir {
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), ui.id().with(&node.path), false)
            .show_header(ui, |ui| {
                ui.label(format!("📁 {}", node.name));
            })
            .body(|ui| {
                for child in &node.children {
                    render_file_tree(ui, child);
                }
            });
    } else {
        ui.horizontal(|ui| {
            let icon = if is_seismic_file(&node.path) { "📈" } else { "📄" };
            ui.label(format!("{} {}", icon, node.name));
        });
    }
}

fn show_search_tab(ui: &mut egui::Ui, state: &mut FdsnState) {
    ui.heading("🔍 Search & Query");
    ui.separator();
    
    egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), ui.id().with("providers_group"), true)
        .show_header(ui, |ui| {
            ui.strong("Data Providers");
        })
        .body(|ui| {
            egui::ScrollArea::vertical().max_height(150.0).show(ui, |ui| {
                let mut all_selected = state.providers.iter().all(|p| p.selected);
                if ui.checkbox(&mut all_selected, "Select All").changed() {
                    for p in &mut state.providers {
                        p.selected = all_selected;
                    }
                }
                ui.separator();
                let columns = 2;
                egui::Grid::new("providers_grid").num_columns(columns).show(ui, |ui| {
                    for (i, p) in state.providers.iter_mut().enumerate() {
                        let tooltip = format!("{}
URL: {}", p.desc, p.url);
                        ui.checkbox(&mut p.selected, &p.name).on_hover_text(tooltip);
                        if (i + 1) % columns == 0 {
                            ui.end_row();
                        }
                    }
                });
            });
        });
        
    ui.add_space(5.0);
    
    egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), ui.id().with("spatial_group"), true)
        .show_header(ui, |ui| {
            ui.strong("Spatial Parameters");
        })
        .body(|ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut state.use_bounding_box, false, "Radial Mode");
                ui.selectable_value(&mut state.use_bounding_box, true, "Bounding Box");
            });
            ui.add_space(5.0);
            
            egui::Grid::new("spatial_grid").num_columns(2).show(ui, |ui| {
                if !state.use_bounding_box {
                    ui.label("Latitude"); ui.text_edit_singleline(&mut state.ref_lat); ui.end_row();
                    ui.label("Longitude"); ui.text_edit_singleline(&mut state.ref_lon); ui.end_row();
                    ui.label("Min Radius (deg)"); ui.text_edit_singleline(&mut state.min_radius); ui.end_row();
                    ui.label("Max Radius (deg)"); ui.text_edit_singleline(&mut state.max_radius); ui.end_row();
                } else {
                    ui.label("Min Lat"); ui.text_edit_singleline(&mut state.min_lat); ui.end_row();
                    ui.label("Max Lat"); ui.text_edit_singleline(&mut state.max_lat); ui.end_row();
                    ui.label("Min Lon"); ui.text_edit_singleline(&mut state.min_lon); ui.end_row();
                    ui.label("Max Lon"); ui.text_edit_singleline(&mut state.max_lon); ui.end_row();
                }
            });
        });

    ui.add_space(5.0);
    
    egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), ui.id().with("temporal_group"), true)
        .show_header(ui, |ui| {
            ui.strong("Temporal Parameters");
        })
        .body(|ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut state.use_absolute_time, false, "Ref Time + Offset");
                ui.selectable_value(&mut state.use_absolute_time, true, "Absolute Time");
            });
            ui.add_space(5.0);
            
            egui::Grid::new("temporal_grid").num_columns(2).show(ui, |ui| {
                if !state.use_absolute_time {
                    ui.label("Origin Date (Y-m-d)"); ui.text_edit_singleline(&mut state.ref_date); ui.end_row();
                    ui.label("Origin Time (H:M:S)"); ui.text_edit_singleline(&mut state.ref_time); ui.end_row();
                    ui.label("Start Offset (s)"); ui.text_edit_singleline(&mut state.start_offset_sec); ui.end_row();
                    ui.label("End Offset (s)"); ui.text_edit_singleline(&mut state.end_offset_sec); ui.end_row();
                } else {
                    ui.label("Start Date (Y-m-d)"); ui.text_edit_singleline(&mut state.start_date); ui.end_row();
                    ui.label("Start Time (H:M:S)"); ui.text_edit_singleline(&mut state.start_time_abs); ui.end_row();
                    ui.label("End Date (Y-m-d)"); ui.text_edit_singleline(&mut state.end_date); ui.end_row();
                    ui.label("End Time (H:M:S)"); ui.text_edit_singleline(&mut state.end_time_abs); ui.end_row();
                }
            });
        });

    ui.add_space(5.0);
    
    egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), ui.id().with("filters_group"), false)
        .show_header(ui, |ui| {
            ui.strong("Filters (Network, Channel, Event)");
        })
        .body(|ui| {
            egui::Grid::new("filters_grid").num_columns(2).show(ui, |ui| {
                ui.label("Network"); ui.text_edit_singleline(&mut state.network).on_hover_text("Comma-separated (e.g. GE,IA). Leave blank for all."); ui.end_row();
                ui.label("Station"); ui.text_edit_singleline(&mut state.station).on_hover_text("Comma-separated. Leave blank for all."); ui.end_row();
                ui.label("Channel"); ui.text_edit_singleline(&mut state.channel).on_hover_text("Comma-separated (e.g. BHZ,HHZ)"); ui.end_row();
                ui.label("Min Mag"); ui.text_edit_singleline(&mut state.min_mag); ui.end_row();
                ui.label("Max Mag"); ui.text_edit_singleline(&mut state.max_mag); ui.end_row();
            });
        });

    ui.add_space(15.0);
    
    if ui.add_enabled(!state.is_searching && !state.is_downloading, egui::Button::new("🔍 Search Stations")).clicked() {
        execute_search(state);
    }
    
    if ui.add_enabled(!state.is_searching && !state.is_downloading, egui::Button::new("🌍 Download Events")).clicked() {
        execute_event_search(state);
    }
}

fn show_settings_tab(ui: &mut egui::Ui, state: &mut FdsnState) {
    ui.heading("⚙ Settings & Download");
    ui.separator();
    
    ui.group(|ui| {
        ui.strong("Output Directory");
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut state.out_dir);
            if ui.button("Browse").clicked() {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    state.out_dir = path.to_string_lossy().to_string();
                }
            }
        });
    });
    
    ui.add_space(10.0);
    ui.group(|ui| {
        ui.strong("Download Options");
        ui.checkbox(&mut state.download_xml, "Download StationXML");
        ui.checkbox(&mut state.download_events, "Download Event Catalog (CSV)");
        ui.checkbox(&mut state.download_arrivals, "Download Phase Arrivals");
        ui.checkbox(&mut state.export_sac, "Export to SAC format");
        ui.checkbox(&mut state.no_mseed, "Skip MiniSEED (no_mseed)");
    });
    
    ui.add_space(20.0);
    if ui.add_enabled(!state.is_searching && !state.is_downloading && (!state.stations.is_empty() || state.download_events), 
        egui::Button::new("⬇ Start Download")).clicked() {
        execute_download(state);
    }
    
    if state.stations.is_empty() && (!state.download_events && !state.download_xml && !state.no_mseed) {
        ui.label(egui::RichText::new("⚠️ You must search for stations first to download waveforms.").color(Color32::from_rgb(200, 100, 0)));
    }
}

fn execute_search(state: &mut FdsnState) {
    state.is_searching = true;
    state.status_msg = "Searching...".to_string();
    state.logs.push("[Action] Starting Station Search...".to_string());
    
    let mut params_list = Vec::new();
    
    let (st, et) = if !state.use_absolute_time {
        let date_str = format!("{}T{}", state.ref_date, state.ref_time);
        let base_t = NaiveDateTime::parse_from_str(&date_str, "%Y-%m-%dT%H:%M:%S").unwrap_or_default();
        let s_t = base_t + Duration::seconds(state.start_offset_sec.parse().unwrap_or(-60));
        let e_t = base_t + Duration::seconds(state.end_offset_sec.parse().unwrap_or(600));
        (s_t, e_t)
    } else {
        let s_date_str = format!("{}T{}", state.start_date, state.start_time_abs);
        let e_date_str = format!("{}T{}", state.end_date, state.end_time_abs);
        let s_t = NaiveDateTime::parse_from_str(&s_date_str, "%Y-%m-%dT%H:%M:%S").unwrap_or_default();
        let e_t = NaiveDateTime::parse_from_str(&e_date_str, "%Y-%m-%dT%H:%M:%S").unwrap_or_default();
        (s_t, e_t)
    };
    
    for p in &state.providers {
        if p.selected {
            let mut params = FdsnSearchParams {
                name: p.name.clone(),
                url: p.url.clone(),
                lat: None,
                lon: None,
                min_radius: None,
                max_radius: None,
                min_lat: None,
                max_lat: None,
                min_lon: None,
                max_lon: None,
                min_mag: None,
                max_mag: None,
                include_arrivals: false,
                start_time: st,
                end_time: et,
                channel: state.channel.clone(),
                network: if state.network.is_empty() { None } else { Some(state.network.clone()) },
                station: if state.station.is_empty() { None } else { Some(state.station.clone()) },
            };
            
            if !state.use_bounding_box {
                params.lat = Some(state.ref_lat.parse().unwrap_or(0.0));
                params.lon = Some(state.ref_lon.parse().unwrap_or(0.0));
                params.min_radius = Some(state.min_radius.parse().unwrap_or(0.0));
                params.max_radius = Some(state.max_radius.parse().unwrap_or(5.0));
            } else {
                params.min_lat = Some(state.min_lat.parse().unwrap_or(-90.0));
                params.max_lat = Some(state.max_lat.parse().unwrap_or(90.0));
                params.min_lon = Some(state.min_lon.parse().unwrap_or(-180.0));
                params.max_lon = Some(state.max_lon.parse().unwrap_or(180.0));
            }
            
            params_list.push(params);
        }
    }
    
    let (tx, rx) = std::sync::mpsc::channel();
    state.receiver = Some(rx);
    
    std::thread::spawn(move || {
                crate::core::fdsn::downloader::search_stations(params_list, tx.clone());
    });
}

fn execute_download(state: &mut FdsnState) {
    if state.stations.is_empty() { return; }
    
    state.is_downloading = true;
    state.downloaded_count = 0;
    state.logs.push("[Action] Starting Downloads...".to_string());
    
    let (tx, rx) = std::sync::mpsc::channel();
    state.receiver = Some(rx);
    
    let (st, et) = if !state.use_absolute_time {
        let date_str = format!("{}T{}", state.ref_date, state.ref_time);
        let base_t = NaiveDateTime::parse_from_str(&date_str, "%Y-%m-%dT%H:%M:%S").unwrap_or_default();
        let s_t = base_t + Duration::seconds(state.start_offset_sec.parse().unwrap_or(-60));
        let e_t = base_t + Duration::seconds(state.end_offset_sec.parse().unwrap_or(600));
        (s_t, e_t)
    } else {
        let s_date_str = format!("{}T{}", state.start_date, state.start_time_abs);
        let e_date_str = format!("{}T{}", state.end_date, state.end_time_abs);
        let s_t = NaiveDateTime::parse_from_str(&s_date_str, "%Y-%m-%dT%H:%M:%S").unwrap_or_default();
        let e_t = NaiveDateTime::parse_from_str(&e_date_str, "%Y-%m-%dT%H:%M:%S").unwrap_or_default();
        (s_t, e_t)
    };
    
    let mut params_list = Vec::new();
    for s in &state.stations {
        params_list.push(FdsnDownloadParams {
            provider_name: s.provider_name.clone(),
            url: s.provider_url.clone(),
            network: s.network.clone(),
            station: s.station.clone(),
            channel: state.channel.clone(),
            start_time: st,
            end_time: et,
            output_dir: PathBuf::from(&state.out_dir),
            export_sac: state.export_sac,
        });
    }
    
    let is_download_xml = state.download_xml;
    std::thread::spawn(move || {
        crate::core::fdsn::downloader::download_waveforms(params_list.clone(), tx.clone());
        if is_download_xml {
            crate::core::fdsn::downloader::download_station_xml(params_list, tx.clone());
        }
        let _ = tx.send(FdsnResult::WaveformDownloadsComplete);
    });
}

fn show_central_view(ctx: &egui::Context, state: &mut FdsnState, map_data: &Option<MapData>) {
    egui::CentralPanel::default().show(ctx, |ui| {
        // Tab Bar
        ui.horizontal(|ui| {
            for (i, tab) in state.open_tabs.iter().enumerate() {
                let name = match tab {
                    EditorTab::MapView => "🌍 Map View",
                    EditorTab::StationList => "📋 Station List",
                    EditorTab::Logs => "📜 Logs",
                };
                
                let is_selected = state.active_tab_index == i;
                if ui.selectable_label(is_selected, name).clicked() {
                    state.active_tab_index = i;
                }
            }
        });
        ui.separator();
        
        // Tab Content
        if let Some(active_tab) = state.open_tabs.get(state.active_tab_index) {
            match active_tab {
                EditorTab::MapView => {
                    if let Some(md) = map_data {
                        let draw_extras = |plot_ui: &mut egui_plot::PlotUi| {
                            // Draw EQ Ref (Red Star)
                            if !state.use_bounding_box {
                                if let (Ok(lat), Ok(lon)) = (state.ref_lat.parse::<f64>(), state.ref_lon.parse::<f64>()) {
                                    plot_ui.points(
                                        egui_plot::Points::new(vec![[lon, lat]])
                                            .color(Color32::RED)
                                            .radius(8.0_f32)
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
                                            plot_ui.line(egui_plot::Line::new(egui_plot::PlotPoints::new(circle_pts)).color(color).width(1.5_f32));
                                        }
                                    };
                                    
                                    draw_circle(min_r, Color32::from_rgb(100, 100, 255));
                                    draw_circle(max_r, Color32::from_rgb(100, 255, 100));
                                }
                            } else {
                                // Draw Bounding Box
                                if let (Ok(min_lat), Ok(max_lat), Ok(min_lon), Ok(max_lon)) = (
                                    state.min_lat.parse::<f64>(), state.max_lat.parse::<f64>(),
                                    state.min_lon.parse::<f64>(), state.max_lon.parse::<f64>()
                                ) {
                                    let pts = vec![
                                        [min_lon, min_lat],
                                        [max_lon, min_lat],
                                        [max_lon, max_lat],
                                        [min_lon, max_lat],
                                        [min_lon, min_lat], // Close the loop
                                    ];
                                    plot_ui.line(egui_plot::Line::new(egui_plot::PlotPoints::new(pts)).color(Color32::from_rgb(100, 255, 100)).width(1.5_f32));
                                }
                            }
                            
                            // Draw stations (Blue Triangle/Diamond)
                            if (!state.stations.is_empty() || state.download_events) {
                                let pts: Vec<[f64; 2]> = state.stations.iter().map(|s| [s.lon, s.lat]).collect();
                                plot_ui.points(
                                    egui_plot::Points::new(pts)
                                        .color(Color32::LIGHT_BLUE)
                                        .radius(5.0_f32)
                                        .shape(egui_plot::MarkerShape::Diamond)
                                );
                            }
                        };
                        
                        // For FDSN, we just want panning and zooming, no selection box
                        state.map_state.interaction_mode = seisbox_core::ui::spatial_map::MapInteractionMode::None;
                        
                        // Empty events for the generic show_map
                        seisbox_core::ui::spatial_map::show_map(ui, md, &mut state.map_state, &[], Some(&draw_extras));
                    }
                },
                EditorTab::StationList => {
                    show_station_list_tab(ui, state);
                },
                EditorTab::Logs => {
                    show_logs_tab(ui, state);
                }
            }
        }
    });
}

fn show_station_list_tab(ui: &mut egui::Ui, state: &mut FdsnState) {
    if state.stations.is_empty() {
        ui.label(egui::RichText::new("No stations found. Please perform a search first.").italics());
        return;
    }
    
    ui.heading(format!("Found {} Stations", state.stations.len()));
    ui.add_space(10.0);
    
    TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::auto()) // Provider
        .column(Column::auto()) // Network
        .column(Column::auto()) // Station
        .column(Column::auto()) // Lat
        .column(Column::auto()) // Lon
        .column(Column::auto()) // Elevation
        .header(20.0, |mut header| {
            header.col(|ui| { ui.strong("Provider"); });
            header.col(|ui| { ui.strong("Net"); });
            header.col(|ui| { ui.strong("Sta"); });
            header.col(|ui| { ui.strong("Lat"); });
            header.col(|ui| { ui.strong("Lon"); });
            header.col(|ui| { ui.strong("Elev"); });
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
                });
            }
        });
}

fn show_logs_tab(ui: &mut egui::Ui, state: &mut FdsnState) {
    ui.heading("Console Logs");
    ui.separator();
    
    egui::ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
        for log in &state.logs {
            if log.starts_with("[Error]") {
                ui.label(egui::RichText::new(log).color(Color32::RED));
            } else if log.starts_with("[Success]") || log.starts_with("[Complete]") {
                ui.label(egui::RichText::new(log).color(Color32::GREEN));
            } else if log.starts_with("[Action]") {
                ui.label(egui::RichText::new(log).color(Color32::YELLOW));
            } else {
                ui.label(log);
            }
        }
    });
}

fn execute_event_search(state: &mut FdsnState) {
    state.download_events = true; // Automatically check download events
    execute_download(state);
}
