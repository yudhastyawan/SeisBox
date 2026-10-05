use eframe::egui;

use crate::app::QuakePickApp;
use crate::core::picking::PhaseType;
use crate::io::file_sync;

/// Process all keyboard shortcuts. Must be called from the main update loop.
///
/// Shortcuts only fire when:
/// 1. No text input widget has focus.
/// 2. For pick placement shortcuts: the mouse is hovering over the plot.
pub fn process_shortcuts(app: &mut QuakePickApp, ctx: &egui::Context) {
    // Don't capture keys when a text field is focused
    if ctx.wants_keyboard_input() {
        return;
    }

    ctx.input(|input| {
        // ------------------------------------------------------------------
        // Phase picking (require hover_x and an active trace)
        // ------------------------------------------------------------------
        if let Some(hover_x) = app.hover_x {
            if app.active_trace_idx.is_some() {
                // p — Pick P-wave start
                if input.key_pressed(egui::Key::P) && !input.modifiers.shift {
                    if let Some(ps) = app.active_pick_set_mut() {
                        ps.add_or_update(PhaseType::PStart, hover_x);
                    }
                    auto_save(app);
                    app.status_msg = format!("Picked P-start at {:.6} s", hover_x);
                }
                // s — Pick S-wave start
                if input.key_pressed(egui::Key::S) && !input.modifiers.shift {
                    if let Some(ps) = app.active_pick_set_mut() {
                        ps.add_or_update(PhaseType::SStart, hover_x);
                    }
                    auto_save(app);
                    app.status_msg = format!("Picked S-start at {:.6} s", hover_x);
                }
                // o — Pick P-wave end
                if input.key_pressed(egui::Key::O) && !input.modifiers.shift {
                    if let Some(ps) = app.active_pick_set_mut() {
                        ps.add_or_update(PhaseType::PEnd, hover_x);
                    }
                    auto_save(app);
                    app.status_msg = format!("Picked P-end at {:.6} s", hover_x);
                }
                // a — Pick S-wave end
                if input.key_pressed(egui::Key::A) && !input.modifiers.shift {
                    if let Some(ps) = app.active_pick_set_mut() {
                        ps.add_or_update(PhaseType::SEnd, hover_x);
                    }
                    auto_save(app);
                    app.status_msg = format!("Picked S-end at {:.6} s", hover_x);
                }

                // Metadata shortcuts (applied to nearest pick within 1.0s)
                let nearest_phase = app.active_pick_set()
                    .and_then(|ps| {
                        ps.picks.iter().min_by(|a, b| {
                            (a.time - hover_x).abs().partial_cmp(&(b.time - hover_x).abs()).unwrap_or(std::cmp::Ordering::Equal)
                        })
                        .filter(|p| (p.time - hover_x).abs() < 1.0)
                        .map(|p| p.phase)
                    });

                if let Some(phase) = nearest_phase {
                    let mut modified = false;
                    
                    // i — Impulsive
                    if input.key_pressed(egui::Key::I) && !input.modifiers.shift {
                        if let Some(p) = app.active_pick_set_mut().unwrap().picks.iter_mut().find(|p| p.phase == phase) {
                            p.onset = Some(crate::core::picking::Onset::Impulsive);
                            modified = true;
                            app.status_msg = format!("Set {} to Impulsive", phase.label());
                        }
                    }
                    // e — Emergent
                    if input.key_pressed(egui::Key::E) && !input.modifiers.shift {
                        if let Some(p) = app.active_pick_set_mut().unwrap().picks.iter_mut().find(|p| p.phase == phase) {
                            p.onset = Some(crate::core::picking::Onset::Emergent);
                            modified = true;
                            app.status_msg = format!("Set {} to Emergent", phase.label());
                        }
                    }
                    // u — Up Polarity
                    if input.key_pressed(egui::Key::U) && !input.modifiers.shift {
                        if let Some(p) = app.active_pick_set_mut().unwrap().picks.iter_mut().find(|p| p.phase == phase) {
                            p.polarity = Some(crate::core::picking::Polarity::Up);
                            modified = true;
                            app.status_msg = format!("Set {} to Up", phase.label());
                        }
                    }
                    // d — Down Polarity
                    if input.key_pressed(egui::Key::D) && !input.modifiers.shift {
                        if let Some(p) = app.active_pick_set_mut().unwrap().picks.iter_mut().find(|p| p.phase == phase) {
                            p.polarity = Some(crate::core::picking::Polarity::Down);
                            modified = true;
                            app.status_msg = format!("Set {} to Down", phase.label());
                        }
                    }
                    // [ — Decrease uncertainty
                    if input.key_pressed(egui::Key::OpenBracket) {
                        if let Some(p) = app.active_pick_set_mut().unwrap().picks.iter_mut().find(|p| p.phase == phase) {
                            let mut unc = p.uncertainty.unwrap_or(0.0);
                            unc = (unc - 0.01).max(0.0);
                            if unc <= 0.0 { p.uncertainty = None; } else { p.uncertainty = Some(unc); }
                            modified = true;
                            app.status_msg = format!("{} Uncertainty: ±{:.3}s", phase.label(), unc);
                        }
                    }
                    // ] — Increase uncertainty
                    if input.key_pressed(egui::Key::CloseBracket) {
                        if let Some(p) = app.active_pick_set_mut().unwrap().picks.iter_mut().find(|p| p.phase == phase) {
                            let mut unc = p.uncertainty.unwrap_or(0.0);
                            unc += 0.01;
                            p.uncertainty = Some(unc);
                            modified = true;
                            app.status_msg = format!("{} Uncertainty: ±{:.3}s", phase.label(), unc);
                        }
                    }
                    // = — Capture polarity amplitude at hover_x
                    if input.key_pressed(egui::Key::Equals) {
                        let mut amp_val = None;
                        let mut amp_dm_val = None;
                        if let Some(ts) = app.active_trace_idx.and_then(|idx| app.traces.get(idx)) {
                            let times = &ts.seismogram.time;
                            let amps = &ts.seismogram.amplitude;
                            if !times.is_empty() {
                                // Find index of closest time using binary search
                                let idx = match times.binary_search_by(|t| t.partial_cmp(&hover_x).unwrap_or(std::cmp::Ordering::Equal)) {
                                    Ok(i) => i,
                                    Err(i) => {
                                        if i == 0 { 0 }
                                        else if i >= times.len() { times.len() - 1 }
                                        else if (times[i] - hover_x).abs() < (times[i-1] - hover_x).abs() { i }
                                        else { i - 1 }
                                    }
                                };
                                amp_val = Some(amps[idx]);
                                amp_dm_val = Some(amps[idx] - ts.seismogram.mean);
                            }
                        }
                        
                        if let (Some(amp), Some(amp_dm)) = (amp_val, amp_dm_val) {
                            if let Some(p) = app.active_pick_set_mut().unwrap().picks.iter_mut().find(|p| p.phase == phase) {
                                p.amplitude = Some(amp);
                                p.amplitude_demeaned = Some(amp_dm);
                                modified = true;
                                app.status_msg = format!("{} Amp: {:.4}, Demeaned: {:.4}", phase.label(), amp, amp_dm);
                            }
                        }
                    }

                    if modified {
                        auto_save(app);
                    }
                }
            }
        }

        // ------------------------------------------------------------------
        // Phase deletion (Shift + key, no hover required)
        // ------------------------------------------------------------------
        if app.active_trace_idx.is_some() {
            // P (Shift+p) — Delete P-start
            if input.key_pressed(egui::Key::P) && input.modifiers.shift {
                let deleted = app.active_pick_set_mut().map(|ps| ps.delete(PhaseType::PStart)).unwrap_or(false);
                if deleted {
                    auto_save(app);
                    app.status_msg = "Deleted P-start pick".to_string();
                }
            }
            // S (Shift+s) — Delete S-start
            if input.key_pressed(egui::Key::S) && input.modifiers.shift {
                let deleted = app.active_pick_set_mut().map(|ps| ps.delete(PhaseType::SStart)).unwrap_or(false);
                if deleted {
                    auto_save(app);
                    app.status_msg = "Deleted S-start pick".to_string();
                }
            }
            // O (Shift+o) — Delete P-end
            if input.key_pressed(egui::Key::O) && input.modifiers.shift {
                let deleted = app.active_pick_set_mut().map(|ps| ps.delete(PhaseType::PEnd)).unwrap_or(false);
                if deleted {
                    auto_save(app);
                    app.status_msg = "Deleted P-end pick".to_string();
                }
            }
            // A (Shift+a) — Delete S-end
            if input.key_pressed(egui::Key::A) && input.modifiers.shift {
                let deleted = app.active_pick_set_mut().map(|ps| ps.delete(PhaseType::SEnd)).unwrap_or(false);
                if deleted {
                    auto_save(app);
                    app.status_msg = "Deleted S-end pick".to_string();
                }
            }

            // w — Wipe all picks on active trace
            if input.key_pressed(egui::Key::W) && !input.modifiers.shift {
                let was_nonempty = app.active_pick_set().map(|ps| !ps.is_empty()).unwrap_or(false);
                if was_nonempty {
                    if let Some(ps) = app.active_pick_set_mut() {
                        ps.wipe();
                    }
                    auto_save(app);
                    app.status_msg = "Wiped all picks".to_string();
                }
            }
        }

        // ------------------------------------------------------------------
        // View controls: Cut (X-axis)
        // ------------------------------------------------------------------
        if input.key_pressed(egui::Key::C) && !input.modifiers.shift {
            if let Some(hover_x) = app.hover_x {
                match app.cut_state {
                    CutState::Idle => {
                        app.cut_state = CutState::WaitingForEnd(hover_x);
                        app.status_msg = format!(
                            "Cut start set at {:.6} s — press 'c' again for end",
                            hover_x
                        );
                    }
                    CutState::WaitingForEnd(start) => {
                        let (lo, hi) = if start < hover_x {
                            (start, hover_x)
                        } else {
                            (hover_x, start)
                        };
                        app.zoom_action = Some(crate::app::ZoomAction::ZoomX(lo, hi));
                        app.cut_state = CutState::Idle;
                        app.status_msg = format!("Cut applied: {:.6} – {:.6} s", lo, hi);
                    }
                }
            }
        }
        // C (Shift+c) — Undo cut
        if input.key_pressed(egui::Key::C) && input.modifiers.shift {
            app.zoom_action = Some(crate::app::ZoomAction::Reset);
            app.cut_state = CutState::Idle;
            app.spectrogram_target = None;
            app.status_msg = "Cut reset — full trace view".to_string();
        }

        // ------------------------------------------------------------------
        // View controls: Zoom (Y-axis)
        // ------------------------------------------------------------------
        if input.key_pressed(egui::Key::Z) && !input.modifiers.shift {
            if let Some(hover_x) = app.hover_x {
                match app.zoom_state {
                    ZoomState::Idle => {
                        app.zoom_state = ZoomState::WaitingForEnd(hover_x);
                        app.status_msg = format!(
                            "Zoom start set at {:.6} s — press 'z' again for end",
                            hover_x
                        );
                    }
                    ZoomState::WaitingForEnd(start_x) => {
                        let (x_lo, x_hi) = if start_x < hover_x {
                            (start_x, hover_x)
                        } else {
                            (hover_x, start_x)
                        };
                        app.zoom_action = Some(crate::app::ZoomAction::ZoomX(x_lo, x_hi));
                        app.zoom_state = ZoomState::Idle;
                        app.spectrogram_target = None;
                        app.status_msg = format!(
                            "Zoom applied: {:.6} – {:.6} s",
                            x_lo, x_hi
                        );
                    }
                }
            }
        }
        // Z (Shift+z) — Undo zoom (reset view)
        if input.key_pressed(egui::Key::Z) && input.modifiers.shift {
            app.zoom_action = Some(crate::app::ZoomAction::Reset);
            app.zoom_state = ZoomState::Idle;
            app.spectrogram_target = None;
            app.status_msg = "Zoom reset — full view".to_string();
        }

        // ------------------------------------------------------------------
        // Filter toggles
        // ------------------------------------------------------------------
        if input.key_pressed(egui::Key::F) && !input.modifiers.shift {
            let has_p = app
                .active_pick_set()
                .and_then(|ps| ps.get(PhaseType::PStart))
                .is_some();
            if has_p {
                app.predictive_filter_on = !app.predictive_filter_on;
                if app.predictive_filter_on {
                    apply_predictive_filter(app);
                    app.status_msg = "Predictive filter ON".to_string();
                } else {
                    revert_filter(app);
                    app.status_msg = "Predictive filter OFF".to_string();
                }
            } else {
                app.status_msg = "⚠ Predictive filter requires P-start pick".to_string();
            }
        }

        if input.key_pressed(egui::Key::B) && !input.modifiers.shift {
            app.show_bandpass = !app.show_bandpass;
            app.status_msg = if app.show_bandpass {
                "Bandpass filter dialog opened".to_string()
            } else {
                "Bandpass filter dialog closed".to_string()
            };
        }

        if (input.key_pressed(egui::Key::B) || input.key_pressed(egui::Key::F)) && input.modifiers.shift {
            revert_filter(app);
            app.filter_active = false;
            app.predictive_filter_on = false;
            
            // Clear spectrogram and auto-scale Y so view adjusts to restored amplitudes
            app.spectrogram_target = None;
            app.spectrogram_pending_target = None;
            app.zoom_action = Some(crate::app::ZoomAction::ResetY);
            
            app.status_msg = "All filters removed — original trace restored".to_string();
        }

        // ------------------------------------------------------------------
        // Analysis tools
        // ------------------------------------------------------------------
        if input.key_pressed(egui::Key::H) && !input.modifiers.shift {
            app.show_hodogram = !app.show_hodogram;
        }

        // q — Toggle spectrogram for active trace
        if input.key_pressed(egui::Key::Q) && !input.modifiers.shift {
            if let Some(idx) = app.active_trace_idx {
                if app.spectrogram_target == Some(idx) {
                    app.spectrogram_target = None;
                    app.spectrogram_texture = None;
                    app.status_msg = "Spectrogram hidden".to_string();
                } else {
                    app.spectrogram_target = Some(idx);
                    app.status_msg = format!("Computing spectrogram for trace {}...", idx + 1);
                    
                    if let Some(ts) = app.traces.get(idx) {
                        let times = &ts.seismogram.time;
                        let bounds = app.current_x_bounds.unwrap_or((0.0, f64::MAX));
                        
                        let start_idx = times.binary_search_by(|t| t.partial_cmp(&bounds.0).unwrap()).unwrap_or_else(|x| x);
                        let mut end_idx = times.binary_search_by(|t| t.partial_cmp(&bounds.1).unwrap()).unwrap_or_else(|x| x);
                        if end_idx >= times.len() { end_idx = times.len().saturating_sub(1); }
                        
                        let samples = if start_idx <= end_idx { end_idx - start_idx + 1 } else { 0 };
                        
                        app.spectrogram_pending_target = Some(idx);
                        app.spectrogram_pending_bounds = Some(bounds);
                        app.spectrogram_pending_samples = samples;
                        
                        // Limit for instant computation. >100k samples prompts confirmation.
                        if samples > 100_000 {
                            app.show_spectrogram_confirm = true;
                            app.status_msg = format!("Pending spectrogram computation for {} samples...", samples);
                        } else {
                            app.execute_spectrogram_computation(ctx);
                        }
                    }
                }
            }
        }
        
        // Shift+Q — Hide spectrogram
        if input.key_pressed(egui::Key::Q) && input.modifiers.shift {
            app.spectrogram_target = None;
            app.spectrogram_texture = None;
            app.status_msg = "Spectrogram hidden".to_string();
        }

        if input.key_pressed(egui::Key::M) && !input.modifiers.shift {
            if let Some(picks) = app.active_pick_set() {
                let p_start = picks.get(PhaseType::PStart).unwrap_or(f64::NAN);
                let s_start = picks.get(PhaseType::SStart).unwrap_or(f64::NAN);
                let p_end = picks.get(PhaseType::PEnd).unwrap_or(f64::NAN);
                let s_end = picks.get(PhaseType::SEnd).unwrap_or(f64::NAN);
                println!(
                    "[QuakePick] Saved picks to SAC headers: t0={:.6} (P-start), t1={:.6} (S-start), t2={:.6} (P-end), t3={:.6} (S-end)",
                    p_start, s_start, p_end, s_end
                );
            }
            app.status_msg = "Picks saved to SAC headers (see console)".to_string();
        }
        
        // Shift + < (Comma) — Previous Trace/Station
        if input.key_pressed(egui::Key::Comma) && input.modifiers.shift {
            app.navigate_traces(false);
            app.status_msg = "Navigated to previous trace(s)".to_string();
        }

        // Shift + > (Period) — Next Trace/Station
        if input.key_pressed(egui::Key::Period) && input.modifiers.shift {
            app.navigate_traces(true);
            app.status_msg = "Navigated to next trace(s)".to_string();
        }
    });
}

/// Auto-save picks for the active trace.
fn auto_save(app: &QuakePickApp) {
    if let Some(idx) = app.active_trace_idx {
        if let Some(ts) = app.traces.get(idx) {
            file_sync::auto_save_picks(&ts.path, &ts.pick_set);
        }
    }
}

/// Apply a mock predictive filter (multiply amplitudes by 0.8) on the active trace.
fn apply_predictive_filter(app: &mut QuakePickApp) {
    if let Some(idx) = app.active_trace_idx {
        if let Some(ts) = app.traces.get_mut(idx) {
            if ts.original_amplitude.is_none() {
                ts.original_amplitude = Some(ts.seismogram.amplitude.clone());
            }
            for amp in ts.seismogram.amplitude.iter_mut() {
                *amp *= 0.8;
            }
            ts.decimated_points = crate::ui::plot::decimate_for_plot(&ts.seismogram.time, &ts.seismogram.amplitude);
            app.filter_active = true;
        }
    }
}

/// Revert to original amplitude data on the active trace.
fn revert_filter(app: &mut QuakePickApp) {
    if let Some(idx) = app.active_trace_idx {
        if let Some(ts) = app.traces.get_mut(idx) {
            if let Some(original) = ts.original_amplitude.take() {
                ts.seismogram.amplitude = original;
                ts.decimated_points = crate::ui::plot::decimate_for_plot(&ts.seismogram.time, &ts.seismogram.amplitude);
            }
        }
    }
}

/// Find the index of the time sample nearest to the given value.
fn find_nearest_index(time: &[f64], target: f64) -> usize {
    time.iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            ((**a) - target)
                .abs()
                .partial_cmp(&((**b) - target).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(i, _)| i)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Cut / Zoom state machines
// ---------------------------------------------------------------------------

/// Two-step state for X-axis cutting.
#[derive(Debug, Clone, Copy)]
pub enum CutState {
    Idle,
    WaitingForEnd(f64),
}

impl Default for CutState {
    fn default() -> Self {
        CutState::Idle
    }
}

/// Two-step state for Y-axis zooming.
#[derive(Debug, Clone, Copy)]
pub enum ZoomState {
    Idle,
    WaitingForEnd(f64),
}

impl Default for ZoomState {
    fn default() -> Self {
        ZoomState::Idle
    }
}
