use eframe::egui;
use egui_plot::{Line, Plot, PlotPoints, VLine};

use crate::core::picking::PickSet;
use crate::core::seismogram::Seismogram;

/// A single loaded trace with its associated picks.
#[derive(Clone, Debug)]
pub struct TraceState {
    /// Path to the source seismic file.
    pub path: std::path::PathBuf,
    /// Loaded (mock) seismogram data.
    pub seismogram: Seismogram,
    /// Original amplitude for filter revert.
    pub original_amplitude: Option<Vec<f64>>,
    /// Picks on this trace.
    pub pick_set: PickSet,
    /// Whether this trace is rendered in the UI.
    pub is_visible: bool,
    /// Cached decimated points for high-performance rendering.
    pub decimated_points: Option<Vec<[f64; 2]>>,
}

pub fn decimate_for_plot(time: &[f64], amplitude: &[f64]) -> Option<Vec<[f64; 2]>> {
    let n_samples = time.len();
    if n_samples <= 20_000 {
        return None;
    }
    
    let num_buckets = 10_000;
    let chunk_size = n_samples / num_buckets;
    let mut decimated = Vec::with_capacity(num_buckets * 2);
    
    for chunk_idx in 0..num_buckets {
        let start = chunk_idx * chunk_size;
        let end = if chunk_idx == num_buckets - 1 { n_samples } else { start + chunk_size };
        if start >= end { continue; }
        
        let mut min_val = f64::INFINITY;
        let mut max_val = f64::NEG_INFINITY;
        let mut min_t = 0.0;
        let mut max_t = 0.0;
        
        for j in start..end {
            let a = amplitude[j];
            if !a.is_finite() { continue; }
            let t = time[j];
            
            if a < min_val { min_val = a; min_t = t; }
            if a > max_val { max_val = a; max_t = t; }
        }
        
        if min_val.is_finite() && max_val.is_finite() {
            if min_t <= max_t {
                decimated.push([min_t, min_val]);
                decimated.push([max_t, max_val]);
            } else {
                decimated.push([max_t, max_val]);
                decimated.push([min_t, min_val]);
            }
        }
    }
    
    Some(decimated)
}

/// Result of rendering the plot panel.
pub struct PlotResult {
    /// Current X coordinate of the mouse pointer on any sub-plot (if hovering).
    pub hover_x: Option<f64>,
    /// Index of the trace the mouse is currently hovering over.
    pub hover_trace_idx: Option<usize>,
    /// The current visible time window (t_min, t_max) of the plots.
    pub x_bounds: Option<(f64, f64)>,
}

/// Waveform colour palette for multi-component stacking.
const TRACE_COLORS: &[(u8, u8, u8)] = &[
    (50, 220, 120),  // Green (Z / first)
    (80, 160, 255),  // Blue  (N / second)
    (255, 130, 80),  // Orange (E / third)
    (200, 100, 255), // Purple (4th)
    (255, 220, 60),  // Yellow (5th)
    (100, 255, 220), // Teal   (6th)
];

/// Render the main waveform plot area with stacked sub-plots.
///
/// Each trace gets its own `egui_plot::Plot` with synchronised X-axis.
/// Pick VLines are shared across all sub-plots so the user can see
/// phase alignment between components.
pub fn show_plot(
    ui: &mut egui::Ui,
    traces: &[TraceState],
    active_trace_idx: Option<usize>,
    zoom_action: &Option<crate::app::ZoomAction>,
    filter_active: bool,
    predictive_filter_on: bool,
    remove_mean: bool,
    spectrogram_target: Option<usize>,
    spectrogram_texture: &Option<egui::TextureHandle>,
    spectrogram_bounds: Option<(f64, f64)>,
    is_screenshot_mode: bool,
) -> PlotResult {
    let mut hover_x: Option<f64> = None;
    let mut hover_trace_idx: Option<usize> = None;
    let mut x_bounds: Option<(f64, f64)> = None;

    // -- Title bar --
    let visible_count = traces.iter().filter(|t| t.is_visible).count();
    ui.horizontal(|ui| {
        if traces.is_empty() {
            ui.heading(
                egui::RichText::new("No trace loaded")
                    .color(ui.visuals().weak_text_color())
                    .italics(),
            );
        } else {
            ui.heading(
                egui::RichText::new(format!("📊 {}/{} traces", visible_count, traces.len()))
                    .color(ui.visuals().strong_text_color()),
            );
            // Show only visible trace names (max 6 to avoid layout overflow)
            let mut shown = 0;
            for (i, ts) in traces.iter().enumerate() {
                if !ts.is_visible { continue; }
                if shown >= 6 { 
                    ui.label(
                        egui::RichText::new("…")
                            .small()
                            .color(ui.visuals().weak_text_color()),
                    );
                    break;
                }
                let color = trace_color(i);
                ui.label(
                    egui::RichText::new(&ts.seismogram.filename)
                        .small()
                        .color(color),
                );
                shown += 1;
            }
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if filter_active {
                ui.label(
                    egui::RichText::new("🔵 FILTERED")
                        .small()
                        .color(ui.visuals().strong_text_color()),
                );
            }
            if predictive_filter_on {
                ui.label(
                    egui::RichText::new("🟢 PREDICT")
                        .small()
                        .color(ui.visuals().strong_text_color()),
                );
            }
        });
    });

    ui.separator();

    if traces.is_empty() {
        // Empty state
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() * 0.3);
            ui.label(
                egui::RichText::new("🌊")
                    .size(48.0)
                    .color(ui.visuals().weak_text_color()),
            );
            ui.add_space(12.0);
            ui.label(
                egui::RichText::new(
                    "Select seismic files in the explorer,\nthen right-click → Open to view waveforms",
                )
                .size(16.0)
                .color(ui.visuals().text_color()),
            );
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("Ctrl/Cmd+Click to multi-select  •  Supported: .sac, .mseed")
                    .small()
                    .color(ui.visuals().weak_text_color()),
            );
        });
    } else {
        let visible_traces: Vec<(usize, &TraceState)> = traces
            .iter()
            .enumerate()
            .filter(|(_, t)| t.is_visible)
            .collect();
        let n = visible_traces.len();

        let status_bar_height = 30.0;
        let available = ui.available_height() - status_bar_height;
        let spacing = if n > 1 { 4.0 } else { 0.0 };
        let mut plot_height = if n == 0 {
            available
        } else {
            let h = (available - spacing * (n as f32 - 1.0)) / n as f32;
            if is_screenshot_mode {
                h.max(20.0) // Allow compressing tightly so it all fits on screen without scroll
            } else {
                h.max(120.0) // Normal scroll behavior if too many
            }
        };

        // Collect the active trace's picks for shared VLine overlay
        let active_picks = active_trace_idx
            .and_then(|idx| traces.get(idx))
            .map(|t| &t.pick_set);

        egui::ScrollArea::vertical().id_salt("plot_scroll").show(ui, |ui| {
            for (render_idx, &(i, trace_state)) in visible_traces.iter().enumerate() {
                let is_active = active_trace_idx == Some(i);
                let seis = &trace_state.seismogram;

                if spectrogram_target == Some(i) {
                    if let Some(tex) = spectrogram_texture {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(format!("  {} (Spectrogram)", seis.filename))
                                    .small()
                                    .color(egui::Color32::from_rgb(255, 150, 0)),
                            );
                        });
                        let spec_plot_id = format!("spectrogram_plot_{}", i);
                        let mut spec_plot = Plot::new(spec_plot_id)
                            .height(plot_height)
                            .allow_drag(true)
                            .allow_zoom(true)
                            .allow_scroll(false)
                            .allow_boxed_zoom(true)
                            .y_axis_min_width(60.0)
                            .y_axis_label(if n > 1 {
                                ""
                            } else {
                                "Freq (Hz)"
                            })
                            .show_axes([true, true]);

                        spec_plot = spec_plot.link_axis("shared_x_axis", egui::Vec2b::new(true, false));
                        spec_plot = spec_plot.link_cursor("shared_cursor", egui::Vec2b::new(true, false));
                        
                        spec_plot.show(ui, |plot_ui| {
                            let (t_min, t_max) = spectrogram_bounds.unwrap_or((
                                seis.time.first().copied().unwrap_or(0.0),
                                seis.time.last().copied().unwrap_or(1.0),
                            ));
                            let f_max = seis.sample_rate / 2.0;
                            
                            let center = egui_plot::PlotPoint::new((t_min + t_max) / 2.0, f_max / 2.0);
                            let size = egui::Vec2::new((t_max - t_min) as f32, f_max as f32);
                            
                            let image = egui_plot::PlotImage::new(tex, center, size);
                            plot_ui.image(image);
                            
                            if let Some(action) = zoom_action {
                                match action {
                                    crate::app::ZoomAction::ZoomX(x_min, x_max) => {
                                        let bounds = plot_ui.plot_bounds();
                                        plot_ui.set_plot_bounds(egui_plot::PlotBounds::from_min_max(
                                            [*x_min, bounds.min()[1]],
                                            [*x_max, bounds.max()[1]],
                                        ));
                                        plot_ui.set_auto_bounds(egui::Vec2b::new(false, false));
                                    }
                                    crate::app::ZoomAction::Reset => {
                                        plot_ui.set_auto_bounds(egui::Vec2b::new(true, true));
                                    }
                                    crate::app::ZoomAction::ResetY => {
                                        // For spectrograms, we might not want to touch Y bounds since it's freq,
                                        // but it's safe to just auto-bound it or leave it alone.
                                        plot_ui.set_auto_bounds(egui::Vec2b::new(false, true));
                                    }
                                }
                            }
                        });
                        ui.add_space(spacing);
                    }
                }

            // Subtle highlight for the active trace
            if is_active && n > 1 {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("▸ {} (active)", seis.filename))
                            .small()
                            .strong()
                            .color(trace_color(i)),
                    );
                });
            } else if n > 1 {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("  {}", seis.filename))
                            .small()
                            .color(egui::Color32::from_rgb(140, 140, 150)),
                    );
                });
            }

            let plot_id = format!("waveform_plot_{}", i);
            let mut plot = Plot::new(plot_id)
                .height(plot_height)
                .allow_drag(true)
                .allow_zoom(true)
                .allow_scroll(false) // Let the parent ScrollArea handle the scroll wheel
                .allow_boxed_zoom(true)
                .y_axis_min_width(60.0)
                .y_axis_formatter(|mark, _range| {
                    if mark.value == 0.0 {
                        "0".to_string()
                    } else if mark.value.abs() >= 1000.0 || mark.value.abs() < 0.01 {
                        format!("{:.1e}", mark.value)
                    } else {
                        format!("{:.1}", mark.value)
                    }
                })
                .y_axis_label(if n > 1 {
                    ""  // save horizontal space in stacked mode
                } else {
                    "Amplitude"
                })
                .show_axes([true, true])
                .label_formatter(move |name, value| {
                    if !name.is_empty() {
                        format!("{}\nt = {:.6} s\nA = {:.4}", name, value.x, value.y)
                    } else {
                        format!("t = {:.6} s\nA = {:.4}", value.x, value.y)
                    }
                });

            // Only show X-axis label on the bottom plot
            if render_idx == n.saturating_sub(1) {
                plot = plot.x_axis_label("Time (s)");
            }

            // Y-bounds: not used in seismology zoom (Y auto-scales)

            // Link X-axes across all subplots for synchronised panning
            if n > 1 {
                plot = plot.link_axis("shared_x_axis", egui::Vec2b::new(true, false));
                plot = plot.link_cursor("shared_cursor", egui::Vec2b::new(true, false));
            }

            let color = trace_color(i);

            let remove_mean_offset = if remove_mean && seis.mean.is_finite() { seis.mean } else { 0.0 };

            let response = plot.show(ui, |plot_ui| {
                if let Some(action) = zoom_action {
                    match action {
                        crate::app::ZoomAction::ZoomX(x_min, x_max) => {
                            if render_idx == 0 {
                                println!("[DEBUG] Applying ZoomX: {:.2} to {:.2}", x_min, x_max);
                            }
                            let bounds = plot_ui.plot_bounds();
                            let min_y = bounds.min()[1];
                            let max_y = bounds.max()[1];
                            
                            plot_ui.set_plot_bounds(egui_plot::PlotBounds::from_min_max(
                                [*x_min, min_y],
                                [*x_max, max_y],
                            ));
                            plot_ui.set_auto_bounds(egui::Vec2b::new(false, false));
                        }
                        crate::app::ZoomAction::Reset => {
                            if render_idx == 0 {
                                println!("[DEBUG] Applying Zoom Reset");
                            }
                            plot_ui.set_auto_bounds(egui::Vec2b::new(true, true));
                        }
                        crate::app::ZoomAction::ResetY => {
                            if render_idx == 0 {
                                println!("[DEBUG] Applying Y-Axis Auto-Scale");
                            }
                            plot_ui.set_auto_bounds(egui::Vec2b::new(false, true));
                        }
                    }
                }

                let points: PlotPoints = if let Some(cached) = &trace_state.decimated_points {
                    PlotPoints::new(
                        cached.iter()
                            .map(|&[t, a]| [t, a - remove_mean_offset])
                            .collect()
                    )
                } else {
                    seis.time
                        .iter()
                        .zip(seis.amplitude.iter())
                        .filter(|(_, &a)| a.is_finite())
                        .map(|(&t, &a)| [t, a - remove_mean_offset])
                        .collect()
                };

                let line = Line::new(points)
                    .name(&seis.filename)
                    .color(color)
                    .width(1.2);
                plot_ui.line(line);

                let mut pick_draws = Vec::new();

                // Draw picks for this trace
                for pick in &trace_state.pick_set.picks {
                    let mut label_str = pick.phase.label().to_string();
                    let mut meta = Vec::new();
                    if let Some(o) = pick.onset { meta.push(o.as_str()); }
                    if let Some(p) = pick.polarity { meta.push(p.as_str()); }
                    if !meta.is_empty() {
                        label_str.push_str(&format!(" ({})", meta.join("")));
                    }
                    if let Some(u) = pick.uncertainty {
                        label_str.push_str(&format!(" ±{:.3}s", u));
                    }

                    // VLine for the main pick (does not affect Y bounds)
                    let vline = VLine::new(pick.time)
                        .name(&label_str)
                        .color(pick.phase.color())
                        .width(2.0);
                    plot_ui.vline(vline);

                    // Compute screen coordinates for manual drawing later
                    let x_center = plot_ui.screen_from_plot(egui_plot::PlotPoint::new(pick.time, 0.0)).x;
                    let (x_min, x_max, has_unc) = if let Some(unc) = pick.uncertainty {
                        let x1 = plot_ui.screen_from_plot(egui_plot::PlotPoint::new(pick.time - unc, 0.0)).x;
                        let x2 = plot_ui.screen_from_plot(egui_plot::PlotPoint::new(pick.time + unc, 0.0)).x;
                        (x1, x2, true)
                    } else {
                        (0.0, 0.0, false)
                    };

                    pick_draws.push((x_center, x_min, x_max, has_unc, pick.phase.color(), label_str));
                }

                // If this is NOT the active trace, also show the active trace's
                // picks as faded lines for cross-component alignment
                if !is_active {
                    if let Some(picks) = active_picks {
                        for pick in &picks.picks {
                            let faded = {
                                let c = pick.phase.color();
                                egui::Color32::from_rgba_premultiplied(
                                    c.r() / 2,
                                    c.g() / 2,
                                    c.b() / 2,
                                    100,
                                )
                            };
                            let vline = VLine::new(pick.time)
                                .color(faded)
                                .width(1.0);
                            plot_ui.vline(vline);
                        }
                    }
                }

                // Crosshair at mouse position (only if actually hovering the plot rect)
                let mut screen_x = None;
                if plot_ui.response().hovered() {
                    if let Some(pos) = plot_ui.pointer_coordinate() {
                        hover_x = Some(pos.x);
                        hover_trace_idx = Some(i);
                        screen_x = Some(plot_ui.screen_from_plot(pos).x);
                    }
                }
                (screen_x, pick_draws, plot_ui.plot_bounds())
            });

            // If we haven't recorded x_bounds yet, take it from this plot (they are all linked)
            if x_bounds.is_none() {
                let b = response.inner.2;
                x_bounds = Some((b.min()[0], b.max()[0]));
            }

            // Draw crosshair manually using raw painter to completely avoid auto-bounds expansion
            let rect = response.response.rect;
            if let Some(screen_x) = response.inner.0 {
                ui.painter().with_clip_rect(rect).vline(
                    screen_x,
                    rect.y_range(),
                    egui::Stroke::new(0.8, egui::Color32::from_rgba_premultiplied(180, 180, 180, 80)),
                );
            }

            // Draw uncertainty bounds and labels manually to avoid infinite bounds expansion
            for (x_center, x_min, x_max, has_unc, color, label_str) in response.inner.1 {
                if has_unc {
                    let fill_color = egui::Color32::from_rgba_premultiplied(
                        color.r() / 4,
                        color.g() / 4,
                        color.b() / 4,
                        40,
                    );
                    ui.painter().with_clip_rect(rect).rect_filled(
                        egui::Rect::from_x_y_ranges(x_min..=x_max, rect.y_range()),
                        0.0,
                        fill_color,
                    );
                }

                // Draw label at the top left of the pick line
                let text_pos = egui::pos2(x_center + 4.0, rect.top() + 4.0);
                ui.painter().with_clip_rect(rect).text(
                    text_pos,
                    egui::Align2::LEFT_TOP,
                    label_str,
                    egui::FontId::proportional(12.0),
                    color,
                );
            }

            // Spacing between subplots
            if render_idx < n - 1 {
                ui.add_space(spacing);
            }
        }
        });
    }

    // -- Status bar --
    ui.separator();
    ui.horizontal(|ui| {
        if let Some(x) = hover_x {
            ui.label(
                    egui::RichText::new(format!("t = {:.6} s", x))
                        .small()
                        .monospace()
                        .color(ui.visuals().text_color()),
            );
        } else {
            ui.label(
                    egui::RichText::new("Hover over plot for cursor position")
                        .small()
                        .color(ui.visuals().weak_text_color()),
            );
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Show active trace's picks
            if let Some(idx) = active_trace_idx {
                if let Some(ts) = traces.get(idx) {
                    let n = ts.pick_set.len();
                    ui.label(
                        egui::RichText::new(format!("{}/4 picks", n))
                            .small()
                            .color(if n > 0 {
                                ui.visuals().strong_text_color()
                            } else {
                                ui.visuals().weak_text_color()
                            }),
                    );

                    for pick in &ts.pick_set.picks {
                        ui.label(
                            egui::RichText::new(format!(
                                "{}={:.4}s",
                                pick.phase.label(),
                                pick.time
                            ))
                            .small()
                            .monospace()
                            .color(pick.phase.color()),
                        );
                    }
                }
            }
        });
    });

    PlotResult {
        hover_x,
        hover_trace_idx,
        x_bounds,
    }
}

/// Get a colour for trace index `i`.
fn trace_color(i: usize) -> egui::Color32 {
    let (r, g, b) = TRACE_COLORS[i % TRACE_COLORS.len()];
    egui::Color32::from_rgb(r, g, b)
}
