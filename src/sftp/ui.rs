use super::FileExplorer;
use egui::{Color32, RichText, ScrollArea, TextEdit, Ui, Vec2};
use std::path::PathBuf;

pub fn render_sftp_sidebar(ui: &mut Ui, explorer: &mut FileExplorer) {
    ui.vertical(|ui| {
        // Navigation Bar
        ui.horizontal(|ui| {
            if ui.button("⬆ Up").clicked() {
                explorer.go_up();
            }
            if ui.button("🔄").on_hover_text("Refresh folder").clicked() {
                explorer.refresh();
            }
            let mut path_str = explorer.current_path.to_string_lossy().to_string();
            let response = ui.add(
                TextEdit::singleline(&mut path_str)
                    .desired_width(ui.available_width() - 35.0)
                    .hint_text("Path..."),
            );
            if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                let target = PathBuf::from(path_str);
                explorer.set_path(target);
            }
        });

        ui.add_space(4.0);

        // Action Toolbar
        ui.horizontal_wrapped(|ui| {
            if ui.small_button("📤 Upload").clicked() {
                if let Some(path) = rfd::FileDialog::new().pick_file() {
                    if let Some(file_name) = path.file_name() {
                        let target = explorer.current_path.join(file_name);
                        let _ = std::fs::copy(&path, target);
                        explorer.refresh();
                    }
                }
            }

            if ui.small_button("📥 Download").clicked() {
                if let Some(ref selected) = explorer.selected_file {
                    if let Some(dest_dir) = rfd::FileDialog::new().pick_folder() {
                        if let Some(name) = selected.file_name() {
                            let dest = dest_dir.join(name);
                            let _ = std::fs::copy(selected, dest);
                        }
                    }
                }
            }

            if ui.small_button("📝 Edit").clicked() {
                if let Some(ref selected) = explorer.selected_file.clone() {
                    if selected.is_file() {
                        explorer.open_editor(selected.clone());
                    }
                }
            }

            if ui.small_button("🗑 Delete").clicked() {
                if let Some(ref selected) = explorer.selected_file.clone() {
                    explorer.delete_entry(selected);
                }
            }
        });

        if let Some(ref err) = explorer.error_message {
            ui.colored_label(Color32::from_rgb(255, 100, 100), err);
        }

        ui.separator();

        // File List
        ScrollArea::vertical().show(ui, |ui| {
            let mut clicked_dir: Option<PathBuf> = None;
            let mut double_clicked_file: Option<PathBuf> = None;

            for entry in &explorer.entries {
                let is_selected = explorer.selected_file.as_ref() == Some(&entry.path);
                let icon = if entry.is_dir {
                    "📁 "
                } else if entry.name.ends_with(".rs") || entry.name.ends_with(".py") || entry.name.ends_with(".sh") || entry.name.ends_with(".json") || entry.name.ends_with(".toml") {
                    "📝 "
                } else if entry.name.ends_with(".zip") || entry.name.ends_with(".tar.gz") {
                    "📦 "
                } else {
                    "📄 "
                };

                let size_text = if entry.is_dir {
                    "<DIR>".to_string()
                } else {
                    format_file_size(entry.size)
                };

                ui.horizontal(|ui| {
                    let label_text = format!("{}{}", icon, entry.name);
                    let response = ui.selectable_label(
                        is_selected,
                        RichText::new(label_text).color(if entry.is_dir {
                            Color32::from_rgb(100, 180, 255)
                        } else {
                            Color32::from_rgb(220, 220, 220)
                        }),
                    );

                    if response.clicked() {
                        explorer.selected_file = Some(entry.path.clone());
                    }

                    if response.double_clicked() {
                        if entry.is_dir {
                            clicked_dir = Some(entry.path.clone());
                        } else {
                            double_clicked_file = Some(entry.path.clone());
                        }
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(size_text).weak().size(11.0));
                    });
                });
            }

            if let Some(dir) = clicked_dir {
                explorer.set_path(dir);
            }
            if let Some(file) = double_clicked_file {
                explorer.open_editor(file);
            }
        });
    });

    // Modal File Editor
    if explorer.editor_open {
        let mut open = explorer.editor_open;
        let file_name = explorer
            .editor_file_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Editor".to_string());

        egui::Window::new(format!("📝 Quick Editor - {}", file_name))
            .open(&mut open)
            .default_size(Vec2::new(600.0, 450.0))
            .show(ui.ctx(), |ui| {
                ui.horizontal(|ui| {
                    if ui.button("💾 Save").clicked() {
                        explorer.save_editor();
                    }
                    if let Some(ref st) = explorer.editor_status {
                        ui.label(st);
                    }
                });
                ui.separator();
                ScrollArea::both().show(ui, |ui| {
                    ui.add(
                        TextEdit::multiline(&mut explorer.editor_content)
                            .font(egui::TextStyle::Monospace)
                            .desired_width(f32::INFINITY)
                            .code_editor(),
                    );
                });
            });
        explorer.editor_open = open;
    }
}

fn format_file_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}
