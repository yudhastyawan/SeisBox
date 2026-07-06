use eframe::egui;
use std::collections::HashSet;
use std::path::PathBuf;

use crate::io::file_sync::{is_pick_file, is_seismic_file, FileNode};

/// Render the left sidebar file explorer panel.
///
/// Left-click selects/deselects files (with Ctrl/Cmd and Shift for multi-select).
/// Right-click on selected files opens a context menu with "Open".
pub fn show_sidebar(
    ctx: &egui::Context,
    root_dir: &Option<PathBuf>,
    file_tree: &Option<FileNode>,
    selected_files: &mut HashSet<PathBuf>,
    last_clicked: &mut Option<PathBuf>,
    traces: &mut Vec<crate::ui::plot::TraceState>,
    sidebar_search: &mut String,
) -> SidebarAction {
    let mut action = SidebarAction::None;

    egui::SidePanel::left("file_explorer")
        .default_width(260.0)
        .min_width(180.0)
        .max_width(400.0)
        .show(ctx, |ui| {
            // -- BOTTOM PANEL (Channels) --
            if !traces.is_empty() {
                egui::TopBottomPanel::bottom("sidebar_channels")
                    .resizable(true)
                    .default_height(200.0)
                    .show_inside(ui, |ui| {
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.heading("📈 Channels");
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.button("None").clicked() {
                                    for t in traces.iter_mut() {
                                        t.is_visible = false;
                                    }
                                }
                                if ui.button("All").clicked() {
                                    for t in traces.iter_mut() {
                                        t.is_visible = true;
                                    }
                                }
                            });
                        });
                        ui.separator();
                        egui::ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                let search_lower = sidebar_search.to_lowercase();
                                for (idx, trace) in traces.iter_mut().enumerate() {
                                    if search_lower.is_empty() || trace.seismogram.filename.to_lowercase().contains(&search_lower) {
                                        let resp = ui.checkbox(&mut trace.is_visible, &trace.seismogram.filename);
                                        resp.context_menu(|ui| {
                                            if ui.button("ℹ Show Header Info").clicked() {
                                                action = SidebarAction::ShowHeader(idx);
                                                ui.close_menu();
                                            }
                                            if ui.button("💾 Export ASCII Data").clicked() {
                                                action = SidebarAction::ExportAscii(idx);
                                                ui.close_menu();
                                            }
                                        });
                                    }
                                }
                            });
                    });
            }

            // -- TOP PANEL (File Tree) --
            ui.add_space(4.0);
            
            // Header
            ui.horizontal(|ui| {
                ui.heading("📂 Explorer");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("⟳").on_hover_text("Refresh tree").clicked() {
                        if root_dir.is_some() {
                            action = SidebarAction::Refresh;
                        }
                    }
                });
            });

            // Search Bar
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.label("🔍");
                ui.text_edit_singleline(sidebar_search)
                    .on_hover_text("Search files and channels");
                if ui.button("✖").on_hover_text("Clear search").clicked() {
                    sidebar_search.clear();
                }
            });

            ui.separator();

            // Open folder button
            if ui
                .button("📁  Open Folder…")
                .on_hover_text("Select a directory to scan for seismic files")
                .clicked()
            {
                action = SidebarAction::OpenFolder;
            }

            if let Some(root) = root_dir {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(root.to_string_lossy().to_string())
                        .small()
                        .color(ui.visuals().weak_text_color()),
                );
            }

            // Selection count indicator
            if !selected_files.is_empty() {
                ui.add_space(2.0);
                ui.label(
                    egui::RichText::new(format!("{} file(s) selected", selected_files.len()))
                        .small()
                        .color(ui.visuals().strong_text_color()),
                );
            }

            ui.separator();

            // File tree
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if let Some(tree) = file_tree {
                        // Collect all seismic file paths for Shift-click range selection
                        let all_files = collect_seismic_files(tree);
                        render_tree(ui, tree, selected_files, last_clicked, &all_files, &mut action, sidebar_search);
                    } else {
                        ui.vertical_centered(|ui| {
                            ui.add_space(40.0);
                            ui.label(
                                egui::RichText::new("No folder open")
                                    .color(ui.visuals().weak_text_color())
                                    .italics(),
                            );
                            ui.add_space(8.0);
                            ui.label(
                                egui::RichText::new(
                                    "Click \"Open Folder\" to browse\nfor .sac and .mseed files",
                                )
                                .small()
                                .color(ui.visuals().weak_text_color()),
                            );
                        });
                    }
                });
        });

    action
}

/// Collect all seismic file paths in tree order (for Shift-click range selection).
fn collect_seismic_files(node: &FileNode) -> Vec<PathBuf> {
    let mut result = Vec::new();
    if node.is_dir {
        for child in &node.children {
            result.extend(collect_seismic_files(child));
        }
    } else if is_seismic_file(&node.path) {
        result.push(node.path.clone());
    }
    result
}

/// Recursively render a file tree node with multi-selection support.
fn render_tree(
    ui: &mut egui::Ui,
    node: &FileNode,
    selected_files: &mut HashSet<PathBuf>,
    last_clicked: &mut Option<PathBuf>,
    all_files: &[PathBuf],
    action: &mut SidebarAction,
    sidebar_search: &String,
) -> bool {
    let search_lower = sidebar_search.to_lowercase();
    let has_search = !search_lower.is_empty();
    
    // If it's a file, check if it matches the search
    if !node.is_dir {
        if has_search && !node.name.to_lowercase().contains(&search_lower) {
            return false;
        }
    }
    
    // If it's a directory, we need to check if any children match
    if node.is_dir {
        // Pre-check if any children will render
        let mut any_child_matches = false;
        if has_search {
            // Very simple recursive check: does this folder or its descendants match?
            // To avoid full recursive pre-pass, we can just rely on the UI rendering it
            // but it's better to hide empty folders during search.
            any_child_matches = node_matches_search(node, &search_lower);
            if !any_child_matches {
                return false;
            }
        }
        
        let is_expanded = if has_search { true } else { false };
        let mut collapser = egui::collapsing_header::CollapsingState::load_with_default_open(
            ui.ctx(),
            ui.make_persistent_id(node.path.clone()),
            true, // default open
        );
        
        if has_search {
            collapser.set_open(true);
        }

        collapser.show_header(ui, |ui| {
            ui.label(format!("📁 {}", node.name));
        })
        .body(|ui| {
            // Sort nodes: directories first, then files
            let mut children = node.children.clone();
            children.sort_by(|a, b| {
                let a_is_dir = a.children.len() > 0;
                let b_is_dir = b.children.len() > 0;
                b_is_dir.cmp(&a_is_dir).then(a.name.cmp(&b.name))
            });
            
            for child in &children {
                render_tree(ui, child, selected_files, last_clicked, all_files, action, sidebar_search);
            }
        });
        
        return true;
    } else {
        let is_seismic = is_seismic_file(&node.path);
        let is_selected = selected_files.contains(&node.path);

        let icon = if is_seismic {
            "📊"
        } else if is_pick_file(&node.path) {
            "📝"
        } else {
            "📄"
        };

        let label_text = format!("{} {}", icon, node.name);
        let text = if is_selected {
            egui::RichText::new(label_text)
                .strong()
                .color(ui.visuals().strong_text_color())
        } else if is_seismic {
            egui::RichText::new(label_text).color(ui.visuals().text_color())
        } else {
            egui::RichText::new(label_text).color(ui.visuals().weak_text_color())
        };

        let mut response = ui.add(egui::SelectableLabel::new(is_selected, text));
        response = response.interact(egui::Sense::click());

        // --- Left-click: Select (with Ctrl/Cmd and Shift modifiers) ---
        if response.clicked() && is_seismic {
            let modifiers = ui.input(|i| i.modifiers);

            if modifiers.shift {
                // Shift+Click: range selection from last_clicked to this file
                if let Some(ref anchor) = last_clicked.clone() {
                    select_range(selected_files, all_files, anchor, &node.path);
                } else {
                    selected_files.insert(node.path.clone());
                    *last_clicked = Some(node.path.clone());
                }
            } else if modifiers.command {
                // Ctrl/Cmd+Click: toggle individual selection
                if is_selected {
                    selected_files.remove(&node.path);
                } else {
                    selected_files.insert(node.path.clone());
                }
                *last_clicked = Some(node.path.clone());
            } else {
                // Plain click: exclusive selection (deselect everything else)
                selected_files.clear();
                selected_files.insert(node.path.clone());
                *last_clicked = Some(node.path.clone());
            }
        }

        // --- Right-click context menu on seismic files ---
        if is_seismic {
            response.context_menu(|ui| {
                // If right-clicking an unselected file, select it first
                if !is_selected {
                    selected_files.clear();
                    selected_files.insert(node.path.clone());
                    *last_clicked = Some(node.path.clone());
                }

                let n = selected_files.len();
                let open_label = if n > 1 {
                    format!("📊 Open {} files", n)
                } else {
                    "📊 Open".to_string()
                };

                if ui.button(open_label).clicked() {
                    // Open all selected seismic files
                    let files_to_open: Vec<PathBuf> =
                        selected_files.iter().cloned().collect();
                    *action = SidebarAction::OpenFiles(files_to_open);
                    ui.close_menu();
                }

                ui.separator();

                if ui.button("✖ Clear selection").clicked() {
                    selected_files.clear();
                    ui.close_menu();
                }
            });
        }
        
        return true;
    }
}

/// Helper function to check if a node or its descendants match the search query
fn node_matches_search(node: &FileNode, search_lower: &str) -> bool {
    if !node.is_dir {
        return node.name.to_lowercase().contains(search_lower);
    }
    for child in &node.children {
        if node_matches_search(child, search_lower) {
            return true;
        }
    }
    false
}

/// Select all files in the range between `anchor` and `target` (inclusive).
fn select_range(
    selected: &mut HashSet<PathBuf>,
    all_files: &[PathBuf],
    anchor: &PathBuf,
    target: &PathBuf,
) {
    let anchor_idx = all_files.iter().position(|p| p == anchor);
    let target_idx = all_files.iter().position(|p| p == target);

    if let (Some(a), Some(t)) = (anchor_idx, target_idx) {
        let (lo, hi) = if a <= t { (a, t) } else { (t, a) };
        for path in &all_files[lo..=hi] {
            selected.insert(path.clone());
        }
    }
}

/// Actions that can result from sidebar interactions.
pub enum SidebarAction {
    None,
    OpenFolder,
    Refresh,
    /// Open one or more seismic files (from context menu).
    OpenFiles(Vec<PathBuf>),
    ShowHeader(usize),
    ExportAscii(usize),
}
