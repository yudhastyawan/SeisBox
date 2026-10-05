use eframe::egui;
use std::fs::File;
use std::io::Write;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FaultSense {
    All,
    StrikeSlip,
    Reverse,
    Normal,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InpGenMode {
    CreateNew,
    AppendToExisting,
}

#[derive(Debug, Clone)]
pub struct InpGeneratorState {
    pub is_open: bool,
    pub fault_sense: FaultSense,
    pub lat: f64,
    pub lon: f64,
    pub depth: f64,
    pub mag: f64,
    pub strike: f64,
    pub dip: f64,
    pub rake: f64,
    pub length: f64,
    pub width: f64,
    pub slip: f64,
    pub min_x: f64,
    pub max_x: f64,
    pub min_y: f64,
    pub max_y: f64,
    pub x_inc: f64,
    pub y_inc: f64,
    pub min_lon: f64,
    pub max_lon: f64,
    pub min_lat: f64,
    pub max_lat: f64,
    pub lon_inc: f64,
    pub lat_inc: f64,
    pub generated_path: Option<String>,
    pub gen_mode: InpGenMode,
    pub append_path: Option<String>,
}

impl Default for InpGeneratorState {
    fn default() -> Self {
        let mut state = Self {
            is_open: false,
            fault_sense: FaultSense::StrikeSlip,
            lat: -0.586,
            lon: 128.034,
            depth: 19.0,
            mag: 7.2,
            strike: 303.0,
            dip: 80.0,
            rake: -11.0,
            length: 0.0,
            width: 0.0,
            slip: 0.0,
            min_x: -100.0,
            max_x: 100.0,
            min_y: -100.0,
            max_y: 100.0,
            x_inc: 5.0,
            y_inc: 5.0,
            min_lon: 0.0,
            max_lon: 0.0,
            min_lat: 0.0,
            max_lat: 0.0,
            lon_inc: 0.0,
            lat_inc: 0.0,
            generated_path: None,
            gen_mode: InpGenMode::CreateNew,
            append_path: None,
        };
        state.recalc_from_mag();
        state.sync_xy_to_lonlat();
        state
    }
}

impl InpGeneratorState {
    pub fn recalc_from_mag(&mut self) {
        // Match Matlab Coulomb 3.4 logic (from unused/sources/utm/wells_coppersmith_window.m)
        // Uses the M = a + b * Log10(L) relations inverted to L = 10^((M - a)/b)
        let (al, bl, aw, bw) = match self.fault_sense {
            FaultSense::All => (4.38, 1.49, 4.06, 2.25),
            FaultSense::StrikeSlip => (4.33, 1.49, 3.80, 2.59),
            FaultSense::Reverse => (4.49, 1.49, 4.37, 1.95),
            FaultSense::Normal => (4.34, 1.54, 4.04, 2.11),
        };
        self.length = 10f64.powf((self.mag - al) / bl);
        self.width = 10f64.powf((self.mag - aw) / bw);
        
        // Coulomb 3.4 derives slip from Moment Magnitude.
        // Mw = (2/3) * Log10(Mo) - 6.07 => Log10(Mo) = 1.5 * Mw + 9.1 (Hanks & Kanamori, Mo in N-m)
        let mo = 10f64.powf(1.5 * self.mag + 9.1);
        let mu = 3.4e10; // Coulomb 3.4 uses shr = 3.4e11 dyne/cm^2 = 3.4e10 N/m^2
        let area = self.length * 1000.0 * self.width * 1000.0;
        self.slip = mo / (mu * area);
    }

    pub fn sync_xy_to_lonlat(&mut self) {
        let earth_r = 6371.0;
        let km_per_deg_lat = std::f64::consts::PI / 180.0 * earth_r;
        let km_per_deg_lon = km_per_deg_lat * self.lat.to_radians().cos();
        self.min_lon = self.lon + (self.min_x / km_per_deg_lon);
        self.max_lon = self.lon + (self.max_x / km_per_deg_lon);
        self.min_lat = self.lat + (self.min_y / km_per_deg_lat);
        self.max_lat = self.lat + (self.max_y / km_per_deg_lat);
        self.lon_inc = self.x_inc / km_per_deg_lon;
        self.lat_inc = self.y_inc / km_per_deg_lat;
    }

    pub fn sync_lonlat_to_xy(&mut self) {
        let earth_r = 6371.0;
        let km_per_deg_lat = std::f64::consts::PI / 180.0 * earth_r;
        let km_per_deg_lon = km_per_deg_lat * self.lat.to_radians().cos();
        self.min_x = (self.min_lon - self.lon) * km_per_deg_lon;
        self.max_x = (self.max_lon - self.lon) * km_per_deg_lon;
        self.min_y = (self.min_lat - self.lat) * km_per_deg_lat;
        self.max_y = (self.max_lat - self.lat) * km_per_deg_lat;
        self.x_inc = self.lon_inc * km_per_deg_lon;
        self.y_inc = self.lat_inc * km_per_deg_lat;
    }
}

pub fn show_inp_generator_panel(ui: &mut egui::Ui, state: &mut InpGeneratorState) {
    egui::ScrollArea::both().show(ui, |ui| {
            egui::Grid::new("inp_generator_grid").num_columns(2).spacing([10.0, 10.0]).show(ui, |ui| {
                ui.label("Fault Sense:");
                let mut sense = state.fault_sense;
                egui::ComboBox::from_id_salt("fault_sense")
                    .selected_text(format!("{:?}", sense))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut sense, FaultSense::All, "All");
                        ui.selectable_value(&mut sense, FaultSense::StrikeSlip, "StrikeSlip");
                        ui.selectable_value(&mut sense, FaultSense::Reverse, "Reverse");
                        ui.selectable_value(&mut sense, FaultSense::Normal, "Normal");
                    });
                if sense != state.fault_sense {
                    state.fault_sense = sense;
                    state.recalc_from_mag();
                }
                ui.end_row();

                ui.label("Magnitude (M):");
                let mut m = state.mag;
                if ui.add(egui::DragValue::new(&mut m).speed(0.1)).changed() {
                    state.mag = m;
                    state.recalc_from_mag();
                }
                ui.end_row();

                ui.label("Latitude (deg):");
                ui.add(egui::DragValue::new(&mut state.lat).speed(0.01));
                ui.end_row();

                ui.label("Longitude (deg):");
                ui.add(egui::DragValue::new(&mut state.lon).speed(0.01));
                ui.end_row();

                ui.label("Depth (km):");
                ui.add(egui::DragValue::new(&mut state.depth).speed(0.1));
                ui.end_row();

                ui.label("Strike (deg):");
                ui.add(egui::DragValue::new(&mut state.strike).speed(1.0));
                ui.end_row();

                ui.label("Dip (deg):");
                ui.add(egui::DragValue::new(&mut state.dip).speed(1.0));
                ui.end_row();

                ui.label("Rake (deg):");
                ui.add(egui::DragValue::new(&mut state.rake).speed(1.0));
                ui.end_row();

                ui.label("Length (km):");
                ui.add(egui::DragValue::new(&mut state.length).speed(0.1));
                ui.end_row();

                ui.label("Width (km):");
                ui.add(egui::DragValue::new(&mut state.width).speed(0.1));
                ui.end_row();

                ui.label("Slip (m):");
                ui.add(egui::DragValue::new(&mut state.slip).speed(0.01));
                ui.end_row();
            });

            ui.add_space(10.0);
            
            let mut changed_xy = false;
            let mut changed_lonlat = false;
            let mut changed_center = false;

            ui.group(|ui| {
                ui.label("Grid Parameters (X / Y in km)");
                egui::Grid::new("inp_grid_xy").num_columns(2).spacing([10.0, 10.0]).show(ui, |ui| {
                    if ui.add(egui::DragValue::new(&mut state.min_x).speed(0.1).min_decimals(4).max_decimals(7).prefix("Min X: ")).changed() { changed_xy = true; }
                    if ui.add(egui::DragValue::new(&mut state.max_x).speed(0.1).min_decimals(4).max_decimals(7).prefix("Max X: ")).changed() { changed_xy = true; }
                    ui.end_row();
                    if ui.add(egui::DragValue::new(&mut state.x_inc).speed(0.1).min_decimals(4).max_decimals(7).prefix("X-Inc: ")).changed() { changed_xy = true; }
                    ui.end_row();
                    
                    if ui.add(egui::DragValue::new(&mut state.min_y).speed(0.1).min_decimals(4).max_decimals(7).prefix("Min Y: ")).changed() { changed_xy = true; }
                    if ui.add(egui::DragValue::new(&mut state.max_y).speed(0.1).min_decimals(4).max_decimals(7).prefix("Max Y: ")).changed() { changed_xy = true; }
                    ui.end_row();
                    if ui.add(egui::DragValue::new(&mut state.y_inc).speed(0.1).min_decimals(4).max_decimals(7).prefix("Y-Inc: ")).changed() { changed_xy = true; }
                });
            });

            ui.group(|ui| {
                ui.label("Map Info (Longitude / Latitude in degrees)");
                egui::Grid::new("inp_grid_lonlat").num_columns(2).spacing([10.0, 10.0]).show(ui, |ui| {
                    if ui.add(egui::DragValue::new(&mut state.lon).speed(0.001).min_decimals(4).max_decimals(7).prefix("Zero Lon: ")).changed() { changed_center = true; }
                    ui.end_row();
                    if ui.add(egui::DragValue::new(&mut state.min_lon).speed(0.001).min_decimals(4).max_decimals(7).prefix("Min Lon: ")).changed() { changed_lonlat = true; }
                    if ui.add(egui::DragValue::new(&mut state.max_lon).speed(0.001).min_decimals(4).max_decimals(7).prefix("Max Lon: ")).changed() { changed_lonlat = true; }
                    ui.end_row();
                    if ui.add(egui::DragValue::new(&mut state.lon_inc).speed(0.001).min_decimals(4).max_decimals(7).prefix("Lon-Inc: ")).changed() { changed_lonlat = true; }
                    ui.end_row();
                    
                    if ui.add(egui::DragValue::new(&mut state.lat).speed(0.001).min_decimals(4).max_decimals(7).prefix("Zero Lat: ")).changed() { changed_center = true; }
                    ui.end_row();
                    if ui.add(egui::DragValue::new(&mut state.min_lat).speed(0.001).min_decimals(4).max_decimals(7).prefix("Min Lat: ")).changed() { changed_lonlat = true; }
                    if ui.add(egui::DragValue::new(&mut state.max_lat).speed(0.001).min_decimals(4).max_decimals(7).prefix("Max Lat: ")).changed() { changed_lonlat = true; }
                    ui.end_row();
                    if ui.add(egui::DragValue::new(&mut state.lat_inc).speed(0.001).min_decimals(4).max_decimals(7).prefix("Lat-Inc: ")).changed() { changed_lonlat = true; }
                });
            });

            if changed_center || changed_xy {
                state.sync_xy_to_lonlat();
            } else if changed_lonlat {
                state.sync_lonlat_to_xy();
            }

            ui.add_space(10.0);
            
            ui.horizontal(|ui| {
                ui.radio_value(&mut state.gen_mode, InpGenMode::CreateNew, "Create New INP");
                ui.radio_value(&mut state.gen_mode, InpGenMode::AppendToExisting, "Append to Existing INP");
            });
            
            if state.gen_mode == InpGenMode::AppendToExisting {
                ui.horizontal(|ui| {
                    if let Some(path) = &state.append_path {
                        ui.label(format!("Target INP (Selected in Explorer): {}", path));
                    } else {
                        ui.label(egui::RichText::new("⚠️ Please select an INP file in the Explorer tab first!").color(egui::Color32::RED).strong());
                    }
                });
            }
            
            ui.add_space(10.0);
            
            ui.add_enabled_ui(state.gen_mode == InpGenMode::CreateNew || state.append_path.is_some(), |ui| {
                let btn_text = if state.gen_mode == InpGenMode::CreateNew { "Generate & Save" } else { "Append to File" };
                if ui.button(btn_text).clicked() {
                if state.gen_mode == InpGenMode::CreateNew {
                    if let Some(path) = rfd::FileDialog::new().add_filter("Input File", &["inp"]).set_file_name("fault.inp").save_file() {
                        let inp_content = crate::core::cfs_io::generate_coulomb_inp_content(
                            state.strike, state.dip, state.rake,
                            state.length, state.width, state.depth, state.slip,
                            state.min_x, state.max_x, state.x_inc,
                            state.min_y, state.max_y, state.y_inc,
                            state.min_lon, state.max_lon, state.lon,
                            state.min_lat, state.max_lat, state.lat,
                            0.400, 0.250, 800000.0 // Default phys params matching previous string format
                        );
                        
                        if let Ok(mut f) = File::create(&path) {
                            let _ = f.write_all(inp_content.as_bytes());
                            state.generated_path = Some(path.display().to_string());
                            state.is_open = false;
                        }
                    }
                } else if state.gen_mode == InpGenMode::AppendToExisting {
                    if let Some(path) = &state.append_path {
                        if let Ok(input) = crate::core::cfs_parser::open_input_file_cui(path) {
                            let new_id = input.el.len() + 1;
                            
                            // Convert lon/lat center to x/y center in the grid of the target file
                            let earth_r = 6371.0;
                            let km_per_deg_lat = std::f64::consts::PI / 180.0 * earth_r;
                            let km_per_deg_lon = km_per_deg_lat * input.map_info.zero_lat.to_radians().cos();
                            
                            let center_x = (state.lon - input.map_info.zero_lon) * km_per_deg_lon;
                            let center_y = (state.lat - input.map_info.zero_lat) * km_per_deg_lat;
                            
                            let new_fault_line = crate::core::cfs_io::generate_fault_line_string(
                                new_id, state.strike, state.dip, state.rake,
                                state.length, state.width, state.depth, state.slip,
                                center_x, center_y
                            );
                            
                            if let Ok(raw_content) = std::fs::read_to_string(path) {
                                let mut lines: Vec<&str> = raw_content.lines().collect();
                                
                                // Find where to insert (before Grid Parameters)
                                let mut insert_idx = lines.len();
                                for (i, line) in lines.iter().enumerate() {
                                    if line.trim().starts_with("Grid Parameters") {
                                        insert_idx = i;
                                        break;
                                    }
                                }
                                
                                // Step back to skip empty lines before Grid Parameters
                                while insert_idx > 0 && lines[insert_idx - 1].trim().is_empty() {
                                    insert_idx -= 1;
                                }
                                
                                let mut new_content = String::new();
                                for i in 0..insert_idx {
                                    new_content.push_str(lines[i]);
                                    new_content.push('\n');
                                }
                                new_content.push_str(&new_fault_line);
                                new_content.push('\n');
                                for i in insert_idx..lines.len() {
                                    new_content.push_str(lines[i]);
                                    new_content.push('\n');
                                }
                                
                                if let Ok(mut f) = File::create(path) {
                                    let _ = f.write_all(new_content.as_bytes());
                                    state.generated_path = Some(path.clone());
                                    state.is_open = false;
                                }
                            }
                        }
                    }
                }
                }
            });
        });
}
