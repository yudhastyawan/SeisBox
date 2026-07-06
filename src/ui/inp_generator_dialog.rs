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

pub fn show_inp_generator_dialog(ctx: &egui::Context, state: &mut InpGeneratorState) {
    if !state.is_open {
        return;
    }

    let mut is_open = state.is_open;
    egui::Window::new("Generate .inp File")
        .open(&mut is_open)
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            egui::Grid::new("inp_generator_grid").num_columns(2).spacing([10.0, 10.0]).show(ui, |ui| {
                ui.label("Fault Sense:");
                let mut sense = state.fault_sense;
                egui::ComboBox::from_id_source("fault_sense")
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
                ui.horizontal(|ui| {
                    if ui.add(egui::DragValue::new(&mut state.min_x).speed(0.1).min_decimals(4).max_decimals(7).prefix("Min X: ")).changed() { changed_xy = true; }
                    if ui.add(egui::DragValue::new(&mut state.max_x).speed(0.1).min_decimals(4).max_decimals(7).prefix("Max X: ")).changed() { changed_xy = true; }
                    if ui.add(egui::DragValue::new(&mut state.x_inc).speed(0.1).min_decimals(4).max_decimals(7).prefix("X-Inc: ")).changed() { changed_xy = true; }
                });
                ui.horizontal(|ui| {
                    if ui.add(egui::DragValue::new(&mut state.min_y).speed(0.1).min_decimals(4).max_decimals(7).prefix("Min Y: ")).changed() { changed_xy = true; }
                    if ui.add(egui::DragValue::new(&mut state.max_y).speed(0.1).min_decimals(4).max_decimals(7).prefix("Max Y: ")).changed() { changed_xy = true; }
                    if ui.add(egui::DragValue::new(&mut state.y_inc).speed(0.1).min_decimals(4).max_decimals(7).prefix("Y-Inc: ")).changed() { changed_xy = true; }
                });
            });

            ui.group(|ui| {
                ui.label("Map Info (Longitude / Latitude in degrees)");
                ui.horizontal(|ui| {
                    if ui.add(egui::DragValue::new(&mut state.lon).speed(0.001).min_decimals(4).max_decimals(7).prefix("Zero Lon: ")).changed() { changed_center = true; }
                    if ui.add(egui::DragValue::new(&mut state.min_lon).speed(0.001).min_decimals(4).max_decimals(7).prefix("Min Lon: ")).changed() { changed_lonlat = true; }
                    if ui.add(egui::DragValue::new(&mut state.max_lon).speed(0.001).min_decimals(4).max_decimals(7).prefix("Max Lon: ")).changed() { changed_lonlat = true; }
                    if ui.add(egui::DragValue::new(&mut state.lon_inc).speed(0.001).min_decimals(4).max_decimals(7).prefix("Lon-Inc: ")).changed() { changed_lonlat = true; }
                });
                ui.horizontal(|ui| {
                    if ui.add(egui::DragValue::new(&mut state.lat).speed(0.001).min_decimals(4).max_decimals(7).prefix("Zero Lat: ")).changed() { changed_center = true; }
                    if ui.add(egui::DragValue::new(&mut state.min_lat).speed(0.001).min_decimals(4).max_decimals(7).prefix("Min Lat: ")).changed() { changed_lonlat = true; }
                    if ui.add(egui::DragValue::new(&mut state.max_lat).speed(0.001).min_decimals(4).max_decimals(7).prefix("Max Lat: ")).changed() { changed_lonlat = true; }
                    if ui.add(egui::DragValue::new(&mut state.lat_inc).speed(0.001).min_decimals(4).max_decimals(7).prefix("Lat-Inc: ")).changed() { changed_lonlat = true; }
                });
            });

            if changed_center || changed_xy {
                state.sync_xy_to_lonlat();
            } else if changed_lonlat {
                state.sync_lonlat_to_xy();
            }

            ui.add_space(10.0);
            if ui.button("Generate & Save").clicked() {
                if let Some(path) = rfd::FileDialog::new().add_filter("Input File", &["inp"]).set_file_name("fault.inp").save_file() {
                    // Calculate fault corners
                    let strike_rad = state.strike.to_radians();
                    let dx = (state.length / 2.0) * strike_rad.sin();
                    let dy = (state.length / 2.0) * strike_rad.cos();
                    
                    let mut x_start = -dx;
                    let mut y_start = -dy;
                    let mut x_fin = dx;
                    let mut y_fin = dy;
                    
                    // Match Matlab's up-dip shift of the fault trace (zx and zy)
                    // Matlab uses convoluted atan/abs logic, which mathematically simplifies to 
                    // shifting perpendicular to the left of the strike (strike - 90 degrees)
                    // by the horizontal projection of the fault half-width.
                    let dd = (state.width / 2.0) * state.dip.to_radians().cos();
                    let shift_rad = (state.strike - 90.0).to_radians();
                    let zx = dd * shift_rad.sin();
                    let zy = dd * shift_rad.cos();
                    
                    x_start += zx;
                    y_start += zy;
                    x_fin += zx;
                    y_fin += zy;
                    
                    let rt_lat = -state.slip * state.rake.to_radians().cos();
                    let reverse = state.slip * state.rake.to_radians().sin();
                    
                    let top = state.depth - (state.width / 2.0) * state.dip.to_radians().sin();
                    let bot = state.depth + (state.width / 2.0) * state.dip.to_radians().sin();
                    
                    // Create inp content
                    let inp_content = format!(
r#"header line 1 
header line 2 
#reg1=  0  #reg2=  0  #fixed=   1  sym=  1
 PR1=       0.250     PR2=       0.250   DEPTH=      {depth:.3}
  E1=      8.000e+05   E2=      8.000e+05
XSYM=       .000     YSYM=       .000
FRIC=          0.400
S1DR=         19.000 S1DP=         -0.010 S1IN=        100.000 S1GD=          0.000
S2DR=         89.990 S2DP=         89.990 S2IN=         30.000 S2GD=          0.000
S3DR=        109.000 S3DP=         -0.010 S3IN=          0.000 S3GD=          0.000

  #   X-start    Y-start     X-fin      Y-fin   Kode  rt.lat    reverse   dip angle     top      bot
xxx xxxxxxxxxx xxxxxxxxxx xxxxxxxxxx xxxxxxxxxx xxx xxxxxxxxxx xxxxxxxxxx xxxxxxxxxx xxxxxxxxxx xxxxxxxxxx
  1 {:10.4} {:10.4} {:10.4} {:10.4} 100 {:10.4} {:10.4} {:10.4} {:10.4} {:10.4}    Fault 1 
  
    Grid Parameters
  1  ----------------------------  Start-x =  {:15.7}
  2  ----------------------------  Start-y =  {:15.7}
  3  --------------------------   Finish-x =  {:15.7}
  4  --------------------------   Finish-y =  {:15.7}
  5  ------------------------  x-increment =  {:15.7}
  6  ------------------------  y-increment =  {:15.7}
     Size Parameters
  1  --------------------------  Plot size =        2.0000000
  2  --------------  Shade/Color increment =        1.0000000
  3  ------  Exaggeration for disp.& dist. =    10000.0000000
  
     Cross section default
  1  ----------------------------  Start-x =      -16.0000000
  2  ----------------------------  Start-y =      -16.0000000
  3  --------------------------   Finish-x =       18.0000000
  4  --------------------------   Finish-y =       26.0000000
  5  ------------------  Distant-increment =        1.0000000
  6  ----------------------------  Z-depth =       30.0000000
  7  ------------------------  Z-increment =        1.0000000
     Map info
  1  ---------------------------- min. lon =  {:15.7}
  2  ---------------------------- max. lon =  {:15.7}
  3  ---------------------------- zero lon =  {:15.7}
  4  ---------------------------- min. lat =  {:15.7}
  5  ---------------------------- max. lat =  {:15.7}
  6  ---------------------------- zero lat =  {:15.7}
"#,
                        x_start, y_start, x_fin, y_fin, rt_lat, reverse, state.dip, top, bot,
                        state.min_x, state.min_y, state.max_x, state.max_y, state.x_inc, state.y_inc,
                        state.min_lon, state.max_lon, state.lon, state.min_lat, state.max_lat, state.lat,
                        depth=state.depth
                    );
                    
                    if let Ok(mut f) = File::create(&path) {
                        let _ = f.write_all(inp_content.as_bytes());
                        state.generated_path = Some(path.display().to_string());
                        state.is_open = false;
                    }
                }
            }
        });

    state.is_open = is_open;
}
