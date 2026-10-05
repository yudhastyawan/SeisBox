#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
use std::env;
use std::path::PathBuf;
use std::process::Command;

struct AppConfig {
    name: String,
    binary: String,
    description: String,
    icon: String,
    process: Option<std::process::Child>,
}

struct SeisboxLauncher {
    apps: Vec<AppConfig>,
    status_msg: String,
}

impl Default for SeisboxLauncher {
    fn default() -> Self {
        let apps = vec![
            AppConfig {
                name: "Seisbox Picker".to_string(),
                binary: "seisbox_picker".to_string(),
                description: "Main Seismogram Viewer & Phase Picker".to_string(),
                icon: "🔎".to_string(),
                process: None,
            },
            AppConfig {
                name: "Statistical Seismology".to_string(),
                binary: "seisbox_stats".to_string(),
                description: "Statistical Seismology Analysis (B-Value, Mc, Gridding, Voronoi)".to_string(),
                icon: "📊".to_string(),
                process: None,
            },
            AppConfig {
                name: "HVSR Analysis".to_string(),
                binary: "seisbox_hvsr".to_string(),
                description: "HVSR Microtremor Analysis".to_string(),
                icon: "📈".to_string(),
                process: None,
            },
            AppConfig {
                name: "Velocity Inversion".to_string(),
                binary: "seisbox_inversion".to_string(),
                description: "1D Vs Structure Inversion".to_string(),
                icon: "📉".to_string(),
                process: None,
            },
            AppConfig {
                name: "Coulomb Stress".to_string(),
                binary: "seisbox_cfs".to_string(),
                description: "Coulomb Stress Change Analysis".to_string(),
                icon: "⚡".to_string(),
                process: None,
            },
            AppConfig {
                name: "XYZ Interpolator".to_string(),
                binary: "seisbox_interp".to_string(),
                description: "XYZ Grid Interpolator".to_string(),
                icon: "🗺️".to_string(),
                process: None,
            },
            AppConfig {
                name: "FDSN Downloader".to_string(),
                binary: "seisbox_fdsn".to_string(),
                description: "FDSN Event & Waveform Downloader".to_string(),
                icon: "📥".to_string(),
                process: None,
            },
            AppConfig {
                name: "ISC Catalog".to_string(),
                binary: "seisbox_isc".to_string(),
                description: "ISC Catalog Search".to_string(),
                icon: "🌍".to_string(),
                process: None,
            },
        ];

        Self {
            apps,
            status_msg: "Welcome to SeisBox Suite".to_string(),
        }
    }
}

impl eframe::App for SeisboxLauncher {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            // Header
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);
                ui.add(
                    egui::Image::new(egui::include_image!("../../../assets/seisbox_icon.png"))
                        .max_height(80.0)
                );
                ui.add_space(10.0);
                ui.label(
                    egui::RichText::new("SeisBox Desktop Suite")
                        .size(36.0)
                        .strong()
                        .color(ui.visuals().strong_text_color()),
                );
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("Advanced Geophysical Analysis Workspace")
                        .size(16.0)
                        .italics(),
                );
                ui.add_space(40.0);
            });

            // Grid of apps
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    let grid = egui::Grid::new("app_grid").spacing([30.0, 30.0]);
                    grid.show(ui, |ui| {
                        for (i, app) in self.apps.iter_mut().enumerate() {
                            let frame = egui::Frame::new()
                                .fill(ui.visuals().window_fill())
                                .corner_radius(15.0)
                                .shadow(egui::epaint::Shadow {
                                    offset: [0, 4],
                                    blur: 8,
                                    spread: 0,
                                    color: egui::Color32::from_black_alpha(20),
                                })
                                .inner_margin(20.0);
                                
                            frame.show(ui, |ui| {
                                ui.set_width(180.0);
                                ui.set_min_height(180.0);
                                ui.vertical_centered(|ui| {
                                    ui.label(egui::RichText::new(&app.icon).size(48.0));
                                    ui.add_space(15.0);
                                    ui.label(egui::RichText::new(&app.name).size(18.0).strong());
                                    ui.add_space(8.0);
                                    
                                    // Description wrapper
                                    let text = egui::RichText::new(&app.description).size(13.0);
                                    ui.add(egui::Label::new(text).wrap());
                                    
                                    ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                                        // Check if running
                                        let mut is_running = false;
                                        if let Some(mut child) = app.process.take() {
                                            match child.try_wait() {
                                                Ok(Some(_)) => { /* exited */ },
                                                Ok(None) => {
                                                    is_running = true;
                                                    app.process = Some(child);
                                                },
                                                Err(_) => { /* error */ },
                                            }
                                        }
                                        
                                        if is_running {
                                            let btn = egui::Button::new(egui::RichText::new("Running...").color(egui::Color32::WHITE).size(14.0))
                                                .fill(egui::Color32::from_rgb(60, 170, 90))
                                                .corner_radius(8.0)
                                                .min_size(egui::vec2(130.0, 35.0));
                                            if ui.add(btn).clicked() {
                                                self.status_msg = format!("{} is already running!", app.name);
                                            }
                                        } else {
                                            let btn = egui::Button::new(egui::RichText::new("Launch").color(egui::Color32::WHITE).size(14.0).strong())
                                                .fill(ui.visuals().selection.bg_fill)
                                                .corner_radius(8.0)
                                                .min_size(egui::vec2(130.0, 35.0));
                                            if ui.add(btn).clicked() {
                                                match spawn_app(&app.binary) {
                                                    Ok(child) => {
                                                        app.process = Some(child);
                                                        self.status_msg = format!("Launched {}", app.name);
                                                    }
                                                    Err(e) => {
                                                        self.status_msg = format!("Failed to launch {}: {}", app.name, e);
                                                    }
                                                }
                                            }
                                        }
                                    });
                                });
                            });
                            
                            if (i + 1) % 4 == 0 {
                                ui.end_row();
                            }
                        }
                    });
                });
            });
            
            ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                ui.add_space(20.0);
                ui.label(
                    egui::RichText::new(&self.status_msg)
                        .color(ui.visuals().warn_fg_color)
                        .size(14.0)
                );
            });
        });
    }
}

fn get_subapp_path(app_name: &str) -> PathBuf {
    let exe_name = format!("{}{}", app_name, env::consts::EXE_SUFFIX);
    
    if let Ok(current_exe) = env::current_exe() {
        if let Some(dir) = current_exe.parent() {
            // 1. Check same directory (Windows / standard cargo run)
            let mut path = dir.to_path_buf();
            path.push(&exe_name);
            if path.exists() {
                return path;
            }
            
            // 2. Check macOS sibling .app bundles
            if let Some(contents) = dir.parent() {
                if let Some(app_bundle) = contents.parent() {
                    if let Some(dist_dir) = app_bundle.parent() {
                        let display_name = match app_name {
                            "seisbox_picker" => "SeisBox Picker",
                            "seisbox_stats" => "SeisBox Stats",
                            "seisbox_hvsr" => "SeisBox HVSR",
                            "seisbox_inversion" => "SeisBox Inversion",
                            "seisbox_cfs" => "SeisBox CFS",
                            "seisbox_interp" => "SeisBox Interpolator",
                            "seisbox_fdsn" => "SeisBox FDSN",
                            "seisbox_isc" => "SeisBox ISC",
                            _ => app_name,
                        };
                        
                        let candidate = dist_dir.join(format!("{}.app", display_name))
                                                .join("Contents")
                                                .join("MacOS")
                                                .join(display_name);
                        if candidate.exists() {
                            return candidate;
                        }
                    }
                }
            }
        }
    }
    PathBuf::from(exe_name)
}

fn spawn_app(app_name: &str) -> Result<std::process::Child, std::io::Error> {
    let path = get_subapp_path(app_name);
    
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        Command::new(path)
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
    }

    #[cfg(not(target_os = "windows"))]
    {
        Command::new(path).spawn()
    }
}

fn load_icon() -> Option<egui::IconData> {
    let icon_data = include_bytes!("../../../assets/seisbox_icon.png");
    if let Ok(image) = image::load_from_memory(icon_data) {
        let image_buffer = image.into_rgba8();
        let (width, height) = image_buffer.dimensions();
        let rgba = image_buffer.into_raw();
        Some(egui::IconData { rgba, width, height })
    } else {
        None
    }
}

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt::init();
    
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1050.0, 750.0])
            .with_min_inner_size([800.0, 650.0])
            .with_icon(load_icon().unwrap_or_else(|| egui::IconData::default())),
        ..Default::default()
    };
    
    eframe::run_native(
        "SeisBox Launcher",
        options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            cc.egui_ctx.set_visuals(egui::Visuals::light());
            Ok(Box::new(SeisboxLauncher::default()))
        }),
    )
}
