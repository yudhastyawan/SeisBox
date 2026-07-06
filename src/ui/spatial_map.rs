use std::io::Cursor;
use eframe::egui;
use eframe::egui::Color32;
use egui_plot::{Plot, Polygon, Line, PlotPoints, Points, Text, PlotPoint, PlotBounds};
use shapefile::{ShapeReader, Shape};

use crate::core::spatial::{BoundingBox, GeoPoint, CrossSectionLine};
use crate::core::isc_client::EarthquakeEvent;

pub struct MapData {
    pub coastlines: Vec<Vec<[f64; 2]>>,
    pub plates: Vec<Vec<[f64; 2]>>,
}

impl MapData {
    pub fn new() -> Self {
        let coast_bytes = include_bytes!("../../src/lib/coastlines/ne_110m_land.shp");
        let plates_bytes = include_bytes!("../../src/lib/plates/PB2002_boundaries.shp");
        
        let coastlines = parse_shapefile(coast_bytes);
        let plates = parse_shapefile(plates_bytes);
        
        Self { coastlines, plates }
    }
}

fn parse_shapefile(bytes: &[u8]) -> Vec<Vec<[f64; 2]>> {
    let mut lines = Vec::new();
    let cursor = Cursor::new(bytes);
    if let Ok(reader) = ShapeReader::new(cursor) {
        if let Ok(shapes) = reader.read() {
            for shape in shapes {
                match shape {
                    Shape::Polygon(poly) => {
                        for ring in poly.rings() {
                            let mut pts = Vec::new();
                            for point in ring.points() {
                                pts.push([point.x, point.y]);
                            }
                            lines.push(pts);
                        }
                    },
                    Shape::Polyline(pline) => {
                        for part in pline.parts() {
                            let mut pts = Vec::new();
                            for point in part {
                                pts.push([point.x, point.y]);
                            }
                            lines.push(pts);
                        }
                    },
                    _ => {}
                }
            }
        }
    }
    lines
}

#[derive(PartialEq)]
pub enum MapInteractionMode {
    None,
    DrawBBox,
    DrawCrossSection,
}

pub struct MapState {
    pub interaction_mode: MapInteractionMode,
    pub bbox: BoundingBox,
    pub cross_section: Option<CrossSectionLine>,
    pub cross_section_buffer_km: f64,
    
    pub temp_drag_start: Option<GeoPoint>,
    pub temp_drag_end: Option<GeoPoint>,
    
    pub zoom_shortcut_pt: Option<GeoPoint>,
    pub bbox_shortcut_pt: Option<GeoPoint>,
    pub zoom_bounds: Option<BoundingBox>,
    pub apply_zoom_next_frame: bool,
    pub reset_zoom_next_frame: bool,
    
    pub cs_shortcut_pt: Option<GeoPoint>,
}

impl Default for MapState {
    fn default() -> Self {
        Self {
            interaction_mode: MapInteractionMode::None,
            bbox: BoundingBox {
                bot_lat: -15.0,
                top_lat: 10.0,
                left_lon: 95.0,
                right_lon: 145.0, // Default to Indonesia region roughly
            },
            cross_section: None,
            cross_section_buffer_km: 50.0,
            temp_drag_start: None,
            temp_drag_end: None,
            zoom_shortcut_pt: None,
            bbox_shortcut_pt: None,
            zoom_bounds: None,
            apply_zoom_next_frame: false,
            reset_zoom_next_frame: false,
            cs_shortcut_pt: None,
        }
    }
}

pub fn show_map(
    ui: &mut egui::Ui,
    map_data: &MapData,
    state: &mut MapState,
    events: &[EarthquakeEvent],
    additional_draw: Option<&dyn Fn(&mut egui_plot::PlotUi)>,
) {
    let plot = Plot::new("spatial_map")
        .data_aspect(1.0)
        .show_axes([true, true])
        .show_grid(true);

    let is_dark_mode = ui.visuals().dark_mode;
    let theme_color = if is_dark_mode { Color32::WHITE } else { Color32::BLACK };

    let plot_response = plot.show(ui, |plot_ui| {
        if state.reset_zoom_next_frame {
            plot_ui.set_plot_bounds(PlotBounds::from_min_max([-180.0, -90.0], [180.0, 90.0]));
            state.reset_zoom_next_frame = false;
        } else if let Some(bounds) = &state.zoom_bounds {
            if state.apply_zoom_next_frame {
                plot_ui.set_plot_bounds(PlotBounds::from_min_max(
                    [bounds.left_lon, bounds.bot_lat],
                    [bounds.right_lon, bounds.top_lat],
                ));
                state.apply_zoom_next_frame = false;
            }
        }

        // Draw coastlines
        for coast in &map_data.coastlines {
            plot_ui.polygon(
                Polygon::new(PlotPoints::new(coast.clone()))
                    .fill_color(Color32::TRANSPARENT)
                    .stroke(egui::Stroke::new(1.0, Color32::from_rgb(100, 150, 100)))
            );
        }
        
        // Draw plates
        for plate in &map_data.plates {
            plot_ui.line(
                Line::new(PlotPoints::new(plate.clone()))
                    .stroke(egui::Stroke::new(1.5, Color32::from_rgb(200, 80, 80)))
            );
        }

        // Draw Earthquakes
        if !events.is_empty() {
            let pts: Vec<[f64; 2]> = events.iter().map(|e| [e.lon, e.lat]).collect();
            plot_ui.points(
                Points::new(pts)
                    .color(Color32::YELLOW)
                    .radius(3.0)
            );
        }

        // Draw Bounding Box
        if state.bbox.is_valid() {
            let pts = vec![
                [state.bbox.left_lon, state.bbox.bot_lat],
                [state.bbox.right_lon, state.bbox.bot_lat],
                [state.bbox.right_lon, state.bbox.top_lat],
                [state.bbox.left_lon, state.bbox.top_lat],
            ];
            plot_ui.polygon(
                Polygon::new(PlotPoints::new(pts))
                    .fill_color(Color32::TRANSPARENT)
                    .stroke(egui::Stroke::new(2.0, Color32::CYAN))
            );
        }

        // Draw Cross Section Line
        if let Some(cs) = &state.cross_section {
            plot_ui.line(
                Line::new(PlotPoints::new(vec![
                    [cs.point_a.lon, cs.point_a.lat],
                    [cs.point_b.lon, cs.point_b.lat]
                ])).stroke(egui::Stroke::new(3.0, theme_color))
            );
            plot_ui.text(Text::new(PlotPoint::new(cs.point_a.lon, cs.point_a.lat), "A").color(theme_color));
            plot_ui.text(Text::new(PlotPoint::new(cs.point_b.lon, cs.point_b.lat), "B").color(theme_color));
        }

        if let Some(pt) = state.zoom_shortcut_pt {
            plot_ui.points(Points::new(vec![[pt.lon, pt.lat]]).color(theme_color).radius(5.0));
            plot_ui.text(Text::new(PlotPoint::new(pt.lon, pt.lat), " Zoom Pt 1").color(theme_color));
        }
        if let Some(pt) = state.bbox_shortcut_pt {
            plot_ui.points(Points::new(vec![[pt.lon, pt.lat]]).color(Color32::CYAN).radius(5.0));
            plot_ui.text(Text::new(PlotPoint::new(pt.lon, pt.lat), " BBox Pt 1").color(Color32::CYAN));
        }
        if let Some(pt) = state.cs_shortcut_pt {
            plot_ui.points(Points::new(vec![[pt.lon, pt.lat]]).color(theme_color).radius(5.0));
            plot_ui.text(Text::new(PlotPoint::new(pt.lon, pt.lat), " CS Pt 1").color(theme_color));
        }

        // Draw interactive temp line/box
        if let (Some(start), Some(end)) = (state.temp_drag_start, state.temp_drag_end) {
            match state.interaction_mode {
                MapInteractionMode::DrawBBox => {
                    let pts = vec![
                        [start.lon, start.lat],
                        [end.lon, start.lat],
                        [end.lon, end.lat],
                        [start.lon, end.lat],
                    ];
                    plot_ui.polygon(
                        Polygon::new(PlotPoints::new(pts))
                            .fill_color(Color32::from_rgba_unmultiplied(0, 255, 255, 30))
                            .stroke(egui::Stroke::new(1.0, Color32::CYAN))
                    );
                },
                MapInteractionMode::DrawCrossSection => {
                    let pts = vec![
                        [start.lon, start.lat],
                        [end.lon, end.lat]
                    ];
                    plot_ui.line(Line::new(PlotPoints::new(pts)).stroke(egui::Stroke::new(2.0, theme_color)));
                },
                _ => {}
            }
        }
        
        if let Some(f) = additional_draw {
            f(plot_ui);
        }
    });

    // Handle interactions
    if plot_response.response.drag_started() && state.interaction_mode != MapInteractionMode::None {
        if let Some(pos) = plot_response.response.interact_pointer_pos() {
            let plot_pos = plot_response.transform.value_from_position(pos);
            state.temp_drag_start = Some(GeoPoint::new(plot_pos.y, plot_pos.x));
        }
    }
    
    if plot_response.response.dragged() && state.interaction_mode != MapInteractionMode::None {
        if let Some(pos) = plot_response.response.interact_pointer_pos() {
            let plot_pos = plot_response.transform.value_from_position(pos);
            state.temp_drag_end = Some(GeoPoint::new(plot_pos.y, plot_pos.x));
        }
    }
    
    if plot_response.response.drag_stopped() && state.interaction_mode != MapInteractionMode::None {
        if let (Some(start), Some(end)) = (state.temp_drag_start, state.temp_drag_end) {
            match state.interaction_mode {
                MapInteractionMode::DrawBBox => {
                    state.bbox = BoundingBox {
                        bot_lat: start.lat.min(end.lat),
                        top_lat: start.lat.max(end.lat),
                        left_lon: start.lon.min(end.lon),
                        right_lon: start.lon.max(end.lon),
                    };
                    state.interaction_mode = MapInteractionMode::None;
                },
                MapInteractionMode::DrawCrossSection => {
                    state.cross_section = Some(CrossSectionLine {
                        point_a: start,
                        point_b: end,
                        buffer_km: state.cross_section_buffer_km,
                    });
                    state.interaction_mode = MapInteractionMode::None;
                },
                _ => {}
            }
        }
        state.temp_drag_start = None;
        state.temp_drag_end = None;
    }

    if plot_response.response.hovered() {
        if let Some(pos) = plot_response.response.hover_pos() {
            let plot_pos = plot_response.transform.value_from_position(pos);
            let geo_pt = GeoPoint::new(plot_pos.y, plot_pos.x);

            // Shift+Z for undo zoom
            if ui.input(|i| i.key_pressed(eframe::egui::Key::Z) && i.modifiers.shift) {
                state.zoom_bounds = None;
                state.zoom_shortcut_pt = None;
                state.reset_zoom_next_frame = true;
            } else if ui.input(|i| i.key_pressed(eframe::egui::Key::Z)) {
                if let Some(pt1) = state.zoom_shortcut_pt {
                    state.zoom_bounds = Some(BoundingBox {
                        bot_lat: pt1.lat.min(geo_pt.lat),
                        top_lat: pt1.lat.max(geo_pt.lat),
                        left_lon: pt1.lon.min(geo_pt.lon),
                        right_lon: pt1.lon.max(geo_pt.lon),
                    });
                    state.apply_zoom_next_frame = true;
                    state.zoom_shortcut_pt = None;
                } else {
                    state.zoom_shortcut_pt = Some(geo_pt);
                }
            }

            if ui.input(|i| i.key_pressed(eframe::egui::Key::X)) {
                if let Some(pt1) = state.bbox_shortcut_pt {
                    state.bbox = BoundingBox {
                        bot_lat: pt1.lat.min(geo_pt.lat),
                        top_lat: pt1.lat.max(geo_pt.lat),
                        left_lon: pt1.lon.min(geo_pt.lon),
                        right_lon: pt1.lon.max(geo_pt.lon),
                    };
                    state.bbox_shortcut_pt = None;
                } else {
                    state.bbox_shortcut_pt = Some(geo_pt);
                }
            }

            if ui.input(|i| i.key_pressed(eframe::egui::Key::C) && i.modifiers.shift) {
                state.cs_shortcut_pt = None;
                state.cross_section = None;
            } else if ui.input(|i| i.key_pressed(eframe::egui::Key::C)) {
                if let Some(pt1) = state.cs_shortcut_pt {
                    state.cross_section = Some(CrossSectionLine {
                        point_a: pt1,
                        point_b: geo_pt,
                        buffer_km: state.cross_section_buffer_km,
                    });
                    state.cs_shortcut_pt = None;
                } else {
                    state.cs_shortcut_pt = Some(geo_pt);
                }
            }
        }
    }
}
