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
            if ui.small_button("📤 Up").clicked() {
                if let Some(path) = rfd::FileDialog::new().pick_file() {
                    if let Some(file_name) = path.file_name() {
                        let target = explorer.current_path.join(file_name);
                        let _ = std::fs::copy(&path, target);
                        explorer.refresh();
                    }
                }
            }

            if ui.small_button("📥 Down").clicked() {
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

            if ui.small_button("🔒 Chmod").clicked() {
                if let Some(ref selected) = explorer.selected_file.clone() {
                    explorer.open_chmod(selected.clone());
                }
            }

            if ui.small_button("➕ Folder").clicked() {
                explorer.show_new_folder = true;
            }

            if ui.small_button("📄 File").clicked() {
                explorer.show_new_file = true;
            }

            if ui.small_button("🗑 Del").clicked() {
                if let Some(ref selected) = explorer.selected_file.clone() {
                    explorer.delete_entry(selected);
                }
            }
        });

        // Inline input for New Folder
        if explorer.show_new_folder {
            ui.horizontal(|ui| {
                ui.label("Folder:");
                ui.add(TextEdit::singleline(&mut explorer.new_folder_name).desired_width(120.0));
                if ui.button("Create").clicked() {
                    let name = explorer.new_folder_name.clone();
                    explorer.create_folder(&name);
                    explorer.show_new_folder = false;
                }
                if ui.button("✖").clicked() {
                    explorer.show_new_folder = false;
                }
            });
        }

        // Inline input for New File
        if explorer.show_new_file {
            ui.horizontal(|ui| {
                ui.label("File:");
                ui.add(TextEdit::singleline(&mut explorer.new_file_name).desired_width(120.0));
                if ui.button("Create").clicked() {
                    let name = explorer.new_file_name.clone();
                    explorer.create_file(&name);
                    explorer.show_new_file = false;
                }
                if ui.button("✖").clicked() {
                    explorer.show_new_file = false;
                }
            });
        }

        if let Some(ref err) = explorer.error_message {
            ui.colored_label(Color32::from_rgb(255, 100, 100), err);
        }

        ui.separator();

        // File List
        ScrollArea::vertical().show(ui, |ui| {
            let mut clicked_dir: Option<PathBuf> = None;
            let mut double_clicked_file: Option<PathBuf> = None;
            let mut context_chmod: Option<PathBuf> = None;
            let mut context_del: Option<PathBuf> = None;

            for entry in &explorer.entries {
                let is_selected = explorer.selected_file.as_ref() == Some(&entry.path);
                let icon = if entry.is_dir {
                    "📁 "
                } else if entry.name.ends_with(".rs")
                    || entry.name.ends_with(".py")
                    || entry.name.ends_with(".sh")
                    || entry.name.ends_with(".json")
                    || entry.name.ends_with(".toml")
                {
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

                    let path_clone = entry.path.clone();
                    let is_file_entry = !entry.is_dir;
                    response.context_menu(|ui| {
                        if is_file_entry && ui.button("📝 Edit in Quick Editor").clicked() {
                            double_clicked_file = Some(path_clone.clone());
                            ui.close_menu();
                        }
                        if ui.button("🔒 Permissions (chmod)").clicked() {
                            context_chmod = Some(path_clone.clone());
                            ui.close_menu();
                        }
                        if ui.button("🗑 Delete").clicked() {
                            context_del = Some(path_clone.clone());
                            ui.close_menu();
                        }
                    });

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
            if let Some(target) = context_chmod {
                explorer.open_chmod(target);
            }
            if let Some(target) = context_del {
                explorer.delete_entry(&target);
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

    // Modal Permissions (chmod) - MobaXterm TFORMCHMOD
    if explorer.chmod_open {
        let mut open = explorer.chmod_open;
        let target_name = explorer
            .chmod_target
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Selected item".to_string());

        egui::Window::new(format!("🔒 Change Permissions: {}", target_name))
            .open(&mut open)
            .default_size(Vec2::new(360.0, 220.0))
            .resizable(false)
            .show(ui.ctx(), |ui| {
                ui.label("Set Unix file permissions (chmod):");
                ui.separator();

                ui.columns(3, |cols| {
                    cols[0].vertical(|ui| {
                        ui.label(RichText::new("Owner").strong());
                        ui.checkbox(&mut explorer.chmod_u_r, "Read (r)");
                        ui.checkbox(&mut explorer.chmod_u_w, "Write (w)");
                        ui.checkbox(&mut explorer.chmod_u_x, "Execute (x)");
                    });

                    cols[1].vertical(|ui| {
                        ui.label(RichText::new("Group").strong());
                        ui.checkbox(&mut explorer.chmod_g_r, "Read (r)");
                        ui.checkbox(&mut explorer.chmod_g_w, "Write (w)");
                        ui.checkbox(&mut explorer.chmod_g_x, "Execute (x)");
                    });

                    cols[2].vertical(|ui| {
                        ui.label(RichText::new("Other").strong());
                        ui.checkbox(&mut explorer.chmod_o_r, "Read (r)");
                        ui.checkbox(&mut explorer.chmod_o_w, "Write (w)");
                        ui.checkbox(&mut explorer.chmod_o_x, "Execute (x)");
                    });
                });

                ui.separator();
                // Compute octal
                let u = (if explorer.chmod_u_r { 4 } else { 0 }) + (if explorer.chmod_u_w { 2 } else { 0 }) + (if explorer.chmod_u_x { 1 } else { 0 });
                let g = (if explorer.chmod_g_r { 4 } else { 0 }) + (if explorer.chmod_g_w { 2 } else { 0 }) + (if explorer.chmod_g_x { 1 } else { 0 });
                let o = (if explorer.chmod_o_r { 4 } else { 0 }) + (if explorer.chmod_o_w { 2 } else { 0 }) + (if explorer.chmod_o_x { 1 } else { 0 });
                ui.label(RichText::new(format!("Octal mode: 0{:01}{:01}{:01}", u, g, o)).monospace().strong());

                ui.horizontal(|ui| {
                    if ui.button("✔ Apply").clicked() {
                        explorer.apply_chmod();
                    }
                    if ui.button("Cancel").clicked() {
                        explorer.chmod_open = false;
                    }
                });
            });
        explorer.chmod_open = open;
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
