use std::path::Path;
use image::{RgbaImage, Rgba};

use crate::ui::plot::TraceState;

/// Color constants matching plot.rs TRACE_COLORS
const TRACE_COLORS: &[(u8, u8, u8)] = &[
    (50, 220, 120),  // Green (Z / first)
    (80, 160, 255),  // Blue  (N / second)
    (255, 130, 80),  // Orange (E / third)
    (200, 100, 255), // Purple (4th)
    (255, 220, 60),  // Yellow (5th)
    (100, 255, 220), // Teal   (6th)
];

/// Background color for the plot (white for publication)
const BG: Rgba<u8> = Rgba([255, 255, 255, 255]);
const SUBPLOT_BG: Rgba<u8> = Rgba([255, 255, 255, 255]);
const AXIS_COLOR: Rgba<u8> = Rgba([40, 40, 40, 255]);
const GRID_COLOR: Rgba<u8> = Rgba([200, 200, 200, 255]);
const TEXT_COLOR: Rgba<u8> = Rgba([20, 20, 20, 255]);
const CURVE_COLOR: Rgba<u8> = Rgba([20, 20, 20, 255]);

/// Font scale factor (each glyph pixel becomes SCALE x SCALE pixels)
const FONT_SCALE: u32 = 2;

/// Margins for the publication figure
const MARGIN_LEFT: u32 = 140;
const MARGIN_RIGHT: u32 = 30;
const MARGIN_TOP: u32 = 15;
const MARGIN_BOTTOM: u32 = 70;
const SUBPLOT_GAP: u32 = 10;
const LABEL_HEIGHT: u32 = 22;

/// Render a publication-quality image of visible seismograms.
///
/// This renders the exact same data visible on screen (including picks, 
/// filter state, spectrogram) to an offscreen image buffer.
pub fn render_figure(
    traces: &[TraceState],
    active_trace_idx: Option<usize>,
    remove_mean: bool,
    spectrogram_target: Option<usize>,
    spectrogram_data: Option<&crate::core::spectrogram::SpectrogramData>,
    spectrogram_bounds: Option<(f64, f64)>,
    x_bounds: Option<(f64, f64)>,
    width: u32,
    height: u32,
) -> RgbaImage {
    let mut img = RgbaImage::from_pixel(width, height, BG);

    let visible: Vec<(usize, &TraceState)> = traces
        .iter()
        .enumerate()
        .filter(|(_, t)| t.is_visible)
        .collect();
    let n = visible.len();
    if n == 0 {
        return img;
    }

    // Count total subplots (spectrogram adds one extra)
    let has_spec = spectrogram_target.is_some() && spectrogram_data.is_some();
    let spec_trace_in_visible = if let Some(st) = spectrogram_target {
        visible.iter().any(|(i, _)| *i == st)
    } else {
        false
    };
    let total_subplots = n + if has_spec && spec_trace_in_visible { 1 } else { 0 };

    let plot_area_w = width.saturating_sub(MARGIN_LEFT + MARGIN_RIGHT);
    let plot_area_h = height.saturating_sub(MARGIN_TOP + MARGIN_BOTTOM);
    let subplot_h = if total_subplots > 0 {
        (plot_area_h.saturating_sub(SUBPLOT_GAP * total_subplots.saturating_sub(1) as u32 + LABEL_HEIGHT * total_subplots as u32)) / total_subplots as u32
    } else {
        plot_area_h
    };

    // Determine global x range
    let (x_min, x_max) = if let Some(bounds) = x_bounds {
        bounds
    } else {
        // Auto from all visible traces
        let mut xmin = f64::INFINITY;
        let mut xmax = f64::NEG_INFINITY;
        for (_, ts) in &visible {
            if let Some(&t) = ts.seismogram.time.first() { if t < xmin { xmin = t; } }
            if let Some(&t) = ts.seismogram.time.last() { if t > xmax { xmax = t; } }
        }
        if !xmin.is_finite() { xmin = 0.0; }
        if !xmax.is_finite() { xmax = 1.0; }
        (xmin, xmax)
    };
    let x_range = if (x_max - x_min).abs() < 1e-12 { 1.0 } else { x_max - x_min };

    let mut current_y = MARGIN_TOP;

    // Mapping function: data x -> pixel x
    let to_px = |x: f64| -> i32 {
        MARGIN_LEFT as i32 + ((x - x_min) / x_range * plot_area_w as f64) as i32
    };

    // Draw subplot index counter
    let mut subplot_idx = 0;

    for &(trace_idx, ts) in &visible {
        let seis = &ts.seismogram;

        // Draw spectrogram BEFORE the waveform if this trace has one
        if spec_trace_in_visible && spectrogram_target == Some(trace_idx) {
            if let Some(spec) = spectrogram_data {
                let label_y = current_y;
                // Draw label
                draw_text_scaled(&mut img, MARGIN_LEFT + 4, label_y, &format!("{} (Spectrogram)", seis.filename), 
                    Rgba([200, 100, 0, 255]), FONT_SCALE);

                let plot_top = label_y + LABEL_HEIGHT;
                let plot_bottom = plot_top + subplot_h;

                // Draw spectrogram image scaled into the subplot rect
                let (st_min, st_max) = spectrogram_bounds.unwrap_or((
                    seis.time.first().copied().unwrap_or(0.0),
                    seis.time.last().copied().unwrap_or(1.0),
                ));

                for py in plot_top..plot_bottom.min(height) {
                    for px in MARGIN_LEFT..(MARGIN_LEFT + plot_area_w).min(width) {
                        // Map pixel to data coordinates
                        let data_x = x_min + (px - MARGIN_LEFT) as f64 / plot_area_w as f64 * x_range;
                        let frac_y = (py - plot_top) as f64 / (plot_bottom - plot_top) as f64;

                        // Map data_x to spectrogram column
                        let spec_frac_x = (data_x - st_min) / (st_max - st_min);
                        if spec_frac_x < 0.0 || spec_frac_x > 1.0 { continue; }

                        let spec_col = (spec_frac_x * spec.width as f64) as usize;
                        let spec_row = (frac_y * spec.height as f64) as usize;

                        if spec_col < spec.width && spec_row < spec.height {
                            let c = spec.pixels[spec_row * spec.width + spec_col];
                            img.put_pixel(px, py, Rgba([c.r(), c.g(), c.b(), 255]));
                        }
                    }
                }

                // Draw border
                draw_rect_outline(&mut img, MARGIN_LEFT, plot_top, MARGIN_LEFT + plot_area_w, plot_bottom, AXIS_COLOR);

                // Y-axis tick labels for spectrogram (frequency)
                let f_max = seis.sample_rate / 2.0;
                let num_freq_ticks = 4u32;
                for fi in 0..=num_freq_ticks {
                    let frac = fi as f64 / num_freq_ticks as f64;
                    // frac=0 → top (f_max), frac=1 → bottom (0 Hz)
                    let freq_val = f_max * (1.0 - frac);
                    let py = plot_top as f64 + frac * (plot_bottom - plot_top) as f64;
                    let py_u = py as u32;
                    // Draw small tick mark
                    if py_u >= plot_top && py_u < plot_bottom.min(height) {
                        draw_hline(&mut img, MARGIN_LEFT.saturating_sub(4), MARGIN_LEFT, py_u, AXIS_COLOR);
                    }
                    // Draw label
                    let label = format!("{:.0}", freq_val);
                    draw_text_scaled(&mut img, 2, py_u.saturating_sub(FONT_SCALE * 7 / 2), &label, AXIS_COLOR, FONT_SCALE);
                }

                current_y = plot_bottom + SUBPLOT_GAP;
                subplot_idx += 1;
            }
        }

        // -- Draw waveform subplot --
        let is_active = active_trace_idx == Some(trace_idx);
        let trace_color = CURVE_COLOR;  // All curves black for publication

        let label_y = current_y;
        // Draw trace label
        let label = if is_active && n > 1 {
            format!("▸ {} (active)", seis.filename)
        } else {
            seis.filename.clone()
        };
        draw_text_scaled(&mut img, MARGIN_LEFT + 4, label_y, &label, TEXT_COLOR, FONT_SCALE);

        let plot_top = label_y + LABEL_HEIGHT;
        let plot_bottom = plot_top + subplot_h;

        // Determine Y range for this subplot
        let remove_mean_offset = if remove_mean && seis.mean.is_finite() { seis.mean } else { 0.0 };
        let mut y_min = f64::INFINITY;
        let mut y_max = f64::NEG_INFINITY;
        for (&t, &a) in seis.time.iter().zip(seis.amplitude.iter()) {
            if t < x_min || t > x_max { continue; }
            if !a.is_finite() { continue; }
            let a = a - remove_mean_offset;
            if a < y_min { y_min = a; }
            if a > y_max { y_max = a; }
        }
        if !y_min.is_finite() { y_min = -1.0; }
        if !y_max.is_finite() { y_max = 1.0; }
        // Add 5% padding
        let y_pad = (y_max - y_min).abs() * 0.05;
        y_min -= y_pad;
        y_max += y_pad;
        let y_range = if (y_max - y_min).abs() < 1e-12 { 1.0 } else { y_max - y_min };

        // Fill subplot background
        for py in plot_top..plot_bottom.min(height) {
            for px in MARGIN_LEFT..(MARGIN_LEFT + plot_area_w).min(width) {
                img.put_pixel(px, py, SUBPLOT_BG);
            }
        }

        // Draw horizontal grid lines (5 lines)
        for gi in 0..=4 {
            let frac = gi as f64 / 4.0;
            let py = plot_top as f64 + (1.0 - frac) * (plot_bottom - plot_top) as f64;
            let py = py as u32;
            if py >= plot_top && py < plot_bottom.min(height) {
                draw_hline(&mut img, MARGIN_LEFT, MARGIN_LEFT + plot_area_w, py, GRID_COLOR);
                // Y-axis tick label
                let val = y_min + frac * y_range;
                let label = if val.abs() >= 1000.0 || (val.abs() < 0.01 && val != 0.0) {
                    format!("{:.1e}", val)
                } else {
                    format!("{:.1}", val)
                };
                draw_text_scaled(&mut img, 2, py.saturating_sub(FONT_SCALE * 7 / 2), &label, AXIS_COLOR, FONT_SCALE);
            }
        }

        // Draw the waveform line
        let mut prev_px: Option<(i32, i32)> = None;
        for (&t, &a) in seis.time.iter().zip(seis.amplitude.iter()) {
            if !a.is_finite() { continue; }
            let a = a - remove_mean_offset;
            let px_x = to_px(t);
            let frac_y = (a - y_min) / y_range;
            let px_y = plot_bottom as i32 - (frac_y * (plot_bottom - plot_top) as f64) as i32;

            if let Some((prev_x, prev_y)) = prev_px {
                draw_line_aa(&mut img, prev_x, prev_y, px_x, px_y, trace_color,
                    MARGIN_LEFT as i32, plot_top as i32, (MARGIN_LEFT + plot_area_w) as i32, plot_bottom as i32);
            }
            prev_px = Some((px_x, px_y));
        }

        // Draw picks for this trace
        for pick in &ts.pick_set.picks {
            let c = pick.phase.color();
            let pick_color = Rgba([c.r(), c.g(), c.b(), 255]);
            let px_x = to_px(pick.time);
            if px_x >= MARGIN_LEFT as i32 && px_x < (MARGIN_LEFT + plot_area_w) as i32 {
                draw_vline_thick(&mut img, px_x as u32, plot_top, plot_bottom, pick_color, 2);
                // Draw label
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
                draw_text_scaled(&mut img, (px_x + 4) as u32, plot_top + 2, &label_str, pick_color, FONT_SCALE);

                // Draw uncertainty band if present
                if let Some(unc) = pick.uncertainty {
                    let x1 = to_px(pick.time - unc).max(MARGIN_LEFT as i32) as u32;
                    let x2 = to_px(pick.time + unc).min((MARGIN_LEFT + plot_area_w) as i32) as u32;
                    for py in plot_top..plot_bottom.min(height) {
                        for px in x1..x2.min(width) {
                            let existing = img.get_pixel(px, py);
                            let blended = blend_alpha(existing, &Rgba([c.r(), c.g(), c.b(), 30]));
                            img.put_pixel(px, py, blended);
                        }
                    }
                }
            }
        }

        // If not the active trace, also draw the active trace's picks as faded lines
        if !is_active {
            if let Some(aidx) = active_trace_idx {
                if let Some(ats) = traces.get(aidx) {
                    for pick in &ats.pick_set.picks {
                        let c = pick.phase.color();
                        let faded = Rgba([c.r() / 2, c.g() / 2, c.b() / 2, 100]);
                        let px_x = to_px(pick.time);
                        if px_x >= MARGIN_LEFT as i32 && px_x < (MARGIN_LEFT + plot_area_w) as i32 {
                            draw_vline_thick(&mut img, px_x as u32, plot_top, plot_bottom, faded, 1);
                        }
                    }
                }
            }
        }

        // Draw subplot border
        draw_rect_outline(&mut img, MARGIN_LEFT, plot_top, MARGIN_LEFT + plot_area_w, plot_bottom, AXIS_COLOR);

        current_y = plot_bottom + SUBPLOT_GAP;
        subplot_idx += 1;
    }

    // -- X-axis labels --
    let x_axis_y = current_y.min(height.saturating_sub(MARGIN_BOTTOM));
    let num_ticks = 8;
    for ti in 0..=num_ticks {
        let frac = ti as f64 / num_ticks as f64;
        let val = x_min + frac * x_range;
        let px_x = MARGIN_LEFT as f64 + frac * plot_area_w as f64;
        let label = format!("{:.2}", val);
        draw_text_scaled(&mut img, (px_x as u32).saturating_sub(20), x_axis_y + 6, &label, AXIS_COLOR, FONT_SCALE);
        // Tick mark
        if px_x >= MARGIN_LEFT as f64 && px_x < (MARGIN_LEFT + plot_area_w) as f64 {
            draw_vline_thick(&mut img, px_x as u32, x_axis_y, x_axis_y + 3, AXIS_COLOR, 1);
        }
    }
    // X-axis label
    let label_x = MARGIN_LEFT + plot_area_w / 2 - 40;
    draw_text_scaled(&mut img, label_x, x_axis_y + 30, "Time (s)", TEXT_COLOR, FONT_SCALE);

    img
}

/// Save the rendered figure to a PNG file.
pub fn save_figure(
    traces: &[TraceState],
    active_trace_idx: Option<usize>,
    remove_mean: bool,
    spectrogram_target: Option<usize>,
    spectrogram_data: Option<&crate::core::spectrogram::SpectrogramData>,
    spectrogram_bounds: Option<(f64, f64)>,
    x_bounds: Option<(f64, f64)>,
    path: &Path,
    width: u32,
    height: u32,
) -> Result<(), String> {
    let img = render_figure(
        traces, active_trace_idx, remove_mean,
        spectrogram_target, spectrogram_data, spectrogram_bounds,
        x_bounds, width, height,
    );
    img.save(path).map_err(|e| format!("Failed to save figure: {}", e))
}

// ---------------------------------------------------------------------------
// Drawing primitives
// ---------------------------------------------------------------------------

fn blend_alpha(base: &Rgba<u8>, overlay: &Rgba<u8>) -> Rgba<u8> {
    let a = overlay[3] as f32 / 255.0;
    let inv_a = 1.0 - a;
    Rgba([
        (overlay[0] as f32 * a + base[0] as f32 * inv_a) as u8,
        (overlay[1] as f32 * a + base[1] as f32 * inv_a) as u8,
        (overlay[2] as f32 * a + base[2] as f32 * inv_a) as u8,
        255,
    ])
}

fn draw_hline(img: &mut RgbaImage, x1: u32, x2: u32, y: u32, color: Rgba<u8>) {
    let (w, h) = img.dimensions();
    if y >= h { return; }
    for x in x1..x2.min(w) {
        img.put_pixel(x, y, color);
    }
}

fn draw_vline_thick(img: &mut RgbaImage, x: u32, y1: u32, y2: u32, color: Rgba<u8>, thickness: u32) {
    let (w, h) = img.dimensions();
    let half = thickness / 2;
    for dx in 0..thickness {
        let px = x + dx - half;
        if px >= w { continue; }
        for y in y1..y2.min(h) {
            if color[3] < 255 {
                let existing = img.get_pixel(px, y);
                img.put_pixel(px, y, blend_alpha(existing, &color));
            } else {
                img.put_pixel(px, y, color);
            }
        }
    }
}

fn draw_rect_outline(img: &mut RgbaImage, x1: u32, y1: u32, x2: u32, y2: u32, color: Rgba<u8>) {
    draw_hline(img, x1, x2, y1, color);
    draw_hline(img, x1, x2, y2.saturating_sub(1), color);
    draw_vline_thick(img, x1, y1, y2, color, 1);
    draw_vline_thick(img, x2.saturating_sub(1), y1, y2, color, 1);
}

/// Draw a line with basic anti-aliasing using Bresenham, clipped to bounds.
fn draw_line_aa(
    img: &mut RgbaImage,
    x0: i32, y0: i32, x1: i32, y1: i32,
    color: Rgba<u8>,
    clip_x0: i32, clip_y0: i32, clip_x1: i32, clip_y1: i32,
) {
    // Bresenham's line algorithm
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut cx = x0;
    let mut cy = y0;

    loop {
        if cx >= clip_x0 && cx < clip_x1 && cy >= clip_y0 && cy < clip_y1 {
            img.put_pixel(cx as u32, cy as u32, color);
        }
        if cx == x1 && cy == y1 { break; }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            cx += sx;
        }
        if e2 <= dx {
            err += dx;
            cy += sy;
        }
    }
}

/// Scaled bitmap text renderer. Each glyph pixel is rendered as a scale×scale block.
fn draw_text_scaled(img: &mut RgbaImage, x: u32, y: u32, text: &str, color: Rgba<u8>, scale: u32) {
    let (w, h) = img.dimensions();
    let mut cursor_x = x;
    for ch in text.chars() {
        let glyph = get_glyph(ch);
        for (row, &bits) in glyph.iter().enumerate() {
            for col in 0..5u32 {
                if bits & (1 << (4 - col)) != 0 {
                    // Draw a scale×scale block
                    for sy in 0..scale {
                        for sx in 0..scale {
                            let px = cursor_x + col * scale + sx;
                            let py = y + row as u32 * scale + sy;
                            if px < w && py < h {
                                img.put_pixel(px, py, color);
                            }
                        }
                    }
                }
            }
        }
        cursor_x += 6 * scale; // (5px char + 1px gap) * scale
    }
}

/// 5x7 bitmap font for basic ASCII characters.
fn get_glyph(ch: char) -> [u8; 7] {
    match ch {
        '0' => [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
        '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        '2' => [0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111],
        '3' => [0b01110, 0b10001, 0b00001, 0b00110, 0b00001, 0b10001, 0b01110],
        '4' => [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
        '5' => [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
        '6' => [0b01110, 0b10000, 0b11110, 0b10001, 0b10001, 0b10001, 0b01110],
        '7' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
        '8' => [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
        '9' => [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110],
        'A' | 'a' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'B' | 'b' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110],
        'C' | 'c' => [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110],
        'D' | 'd' => [0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110],
        'E' | 'e' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
        'F' | 'f' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000],
        'G' | 'g' => [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110],
        'H' | 'h' => [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'I' | 'i' => [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        'J' | 'j' => [0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100],
        'K' | 'k' => [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001],
        'L' | 'l' => [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
        'M' | 'm' => [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001],
        'N' | 'n' => [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001],
        'O' | 'o' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'P' | 'p' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
        'Q' | 'q' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101],
        'R' | 'r' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
        'S' | 's' => [0b01110, 0b10001, 0b10000, 0b01110, 0b00001, 0b10001, 0b01110],
        'T' | 't' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
        'U' | 'u' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'V' | 'v' => [0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b01010, 0b00100],
        'W' | 'w' => [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001],
        'X' | 'x' => [0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001],
        'Y' | 'y' => [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100],
        'Z' | 'z' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111],
        ' ' => [0; 7],
        '.' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b01100],
        ',' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00110, 0b00100, 0b01000],
        ':' => [0b00000, 0b01100, 0b01100, 0b00000, 0b01100, 0b01100, 0b00000],
        '-' => [0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000],
        '+' => [0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000],
        '(' => [0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010],
        ')' => [0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000],
        '/' => [0b00001, 0b00010, 0b00010, 0b00100, 0b01000, 0b01000, 0b10000],
        '_' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b11111],
        '=' => [0b00000, 0b00000, 0b11111, 0b00000, 0b11111, 0b00000, 0b00000],
        '±' => [0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000, 0b11111],
        '▸' => [0b01000, 0b01100, 0b01110, 0b01111, 0b01110, 0b01100, 0b01000],
        _ => [0b11111, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11111], // unknown = box
    }
}
