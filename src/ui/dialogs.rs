use eframe::egui;

/// Render the bandpass filter configuration dialog.
/// Returns `true` if the user clicked "Apply".
pub fn show_bandpass_dialog(
    ctx: &egui::Context,
    open: &mut bool,
    low_freq: &mut f64,
    high_freq: &mut f64,
) -> bool {
    let mut applied = false;
    let mut should_close = false;

    let mut is_open = true;
    egui::Window::new("🎛 Bandpass Filter")
        .open(&mut is_open)
        .resizable(false)
        .collapsible(false)
        .default_width(280.0)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.label("Low corner freq (Hz):");
                ui.add(
                    egui::DragValue::new(low_freq)
                        .speed(1.0)
                        .range(0.1..=10000.0)
                        .suffix(" Hz"),
                );
            });

            ui.add_space(4.0);

            ui.horizontal(|ui| {
                ui.label("High corner freq (Hz):");
                ui.add(
                    egui::DragValue::new(high_freq)
                        .speed(1.0)
                        .range(0.1..=10000.0)
                        .suffix(" Hz"),
                );
            });

            ui.add_space(8.0);

            // Validation
            if *low_freq >= *high_freq {
                ui.label(
                    egui::RichText::new("⚠ Low freq must be less than high freq")
                        .small()
                        .color(ui.visuals().error_fg_color),
                );
                ui.add_space(4.0);
            }

            ui.separator();
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                if ui.button("Apply").clicked() && *low_freq < *high_freq {
                    applied = true;
                    should_close = true;
                }
                if ui.button("Cancel").clicked() {
                    should_close = true;
                }
            });

            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("Note: This is a mock filter for demonstration.")
                    .small()
                    .color(ui.visuals().weak_text_color())
                    .italics(),
            );
        });

    // Handle closing: either from the X button or from Cancel/Apply
    if !is_open || should_close {
        *open = false;
    }

    applied
}

/// Render the hodogram analysis placeholder dialog.
pub fn show_hodogram_dialog(ctx: &egui::Context, open: &mut bool) {
    let mut is_open = true;
    let mut should_close = false;

    egui::Window::new("📐 Hodogram Analysis")
        .open(&mut is_open)
        .resizable(true)
        .default_size([400.0, 300.0])
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.add_space(20.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("📐")
                        .size(48.0)
                        .color(egui::Color32::from_rgb(160, 120, 255)),
                );
                ui.add_space(12.0);
                ui.heading("Hodogram Analysis");
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("Particle motion analysis — Coming Soon")
                        .color(ui.visuals().text_color()),
                );
                ui.add_space(16.0);
                ui.label(
                    egui::RichText::new(
                        "This module will display 2D/3D particle motion plots\n\
                         using multi-component seismogram data.",
                    )
                    .small()
                    .color(egui::Color32::from_rgb(130, 130, 130)),
                );
                ui.add_space(20.0);
                if ui.button("Close").clicked() {
                    should_close = true;
                }
            });
        });

    if !is_open || should_close {
        *open = false;
    }
}

/// Render the spectral analysis placeholder dialog.
pub fn show_spectral_dialog(ctx: &egui::Context, open: &mut bool) {
    let mut is_open = true;
    let mut should_close = false;

    egui::Window::new("📈 Spectral Analysis")
        .open(&mut is_open)
        .resizable(true)
        .default_size([400.0, 300.0])
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.add_space(20.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("📈")
                        .size(48.0)
                        .color(egui::Color32::from_rgb(255, 200, 80)),
                );
                ui.add_space(12.0);
                ui.heading("Spectral Analysis");
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("Frequency-domain analysis — Coming Soon")
                        .color(ui.visuals().text_color()),
                );
                ui.add_space(16.0);
                ui.label(
                    egui::RichText::new(
                        "This module will display FFT power spectra,\n\
                         spectrograms, and dominant frequency estimation.",
                    )
                    .small()
                    .color(egui::Color32::from_rgb(130, 130, 130)),
                );
                ui.add_space(20.0);
                if ui.button("Close").clicked() {
                    should_close = true;
                }
            });
        });

    if !is_open || should_close {
        *open = false;
    }
}
