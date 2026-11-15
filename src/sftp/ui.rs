use crate::terminal::widget::copy_text_to_clipboard;
use super::{
    format_modified_time, format_size_kb, ExternalOpenMode, FileEntry, FileExplorer,
    SftpSortColumn,
};
use egui::{
    pos2, vec2, Align2, Color32, FontId, Margin, Rect, RichText, Rounding,
    ScrollArea, Sense, Stroke, TextEdit, Ui, Vec2,
};
use std::path::PathBuf;

pub fn render_sftp_sidebar(ui: &mut Ui, explorer: &mut FileExplorer) {
    explorer.poll();

    let sidebar_resp = ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 5.0;

        // Header indicator: Remote SFTP vs Local
        ui.horizontal(|ui| {
            if explorer.is_remote {
                let label = explorer.remote_label.as_deref().unwrap_or("VPS");
                let dot_color = if explorer.is_loading {
                    Color32::from_rgb(234, 179, 8)
                } else if explorer.is_confirmed_disconnected() {
                    Color32::from_rgb(248, 81, 73)
                } else {
                    Color32::from_rgb(63, 185, 80)
                };
                ui.label(RichText::new("●").color(dot_color).size(12.0));
                ui.label(RichText::new(format!("SFTP: {}", label)).strong().size(11.5).color(Color32::from_rgb(88, 166, 255)));
                if explorer.is_loading {
                    ui.spinner();
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("💻 Local").on_hover_text("Switch to browsing local files").clicked() {
                        explorer.disconnect_remote();
                    }

                    // Sudo SFTP Toggle Button
                    let is_sudo = explorer.is_active_sudo_mode();
                    let sudo_btn = if is_sudo {
                        egui::Button::new(RichText::new("🛡️ Sudo Active").strong().size(11.0).color(Color32::WHITE))
                            .fill(Color32::from_rgb(234, 88, 12)) // Bright orange/red
                    } else {
                        egui::Button::new(RichText::new("🛡️ Sudo SFTP").size(11.0).color(Color32::from_rgb(209, 217, 224)))
                            .fill(Color32::from_rgb(33, 38, 45))
                    };
                    if ui.add(sudo_btn).on_hover_text("Bật / Tắt chế độ Sudo SFTP (Root Mode để đọc, ghi, sửa file hệ thống)").clicked() {
                        explorer.toggle_sudo_mode();
                    }
                });
            } else {
                ui.label(RichText::new("💻 Local Files").strong().size(11.5).color(Color32::from_rgb(139, 148, 158)));
            }
        });

        // 1. RUSTYBOX 10-ICON VECTOR ACTION TOOLBAR
        let avail_w = ui.available_width();
        let num_btns = 10usize;
        let gap = 2.0f32;
        let btn_w = ((avail_w - gap * (num_btns as f32 - 1.0)) / num_btns as f32).clamp(20.0, 26.0);
        let btn_h = 24.0f32;

        let (strip_rect, _) = ui.allocate_exact_size(vec2(avail_w, btn_h), Sense::hover());

        let tooltips = [
            "Parent directory (..)",
            "Upload file(s) [Right-click: Upload folder]",
            "Download selected file",
            "Refresh folder",
            "Create new folder",
            "Create new file",
            "Delete selected item",
            "Open in Quick Editor",
            "Show / Hide hidden dotfiles (.*)",
            "Permissions (chmod)",
        ];

        let mut clicked_tool: Option<usize> = None;

        for i in 0..num_btns {
            let x0 = strip_rect.left() + i as f32 * (btn_w + gap);
            let r = Rect::from_min_size(pos2(x0, strip_rect.top()), vec2(btn_w, btn_h));
            let resp = ui.interact(r, ui.id().with(("sftp_tool_btn", i)), Sense::click());
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if i == 1 && resp.secondary_clicked()
                && let Some(folder) = rfd::FileDialog::new().set_title("Select Folder to Upload").pick_folder() {
                    explorer.upload_folder(folder);
                }

            let is_active_toggle = i == 8 && explorer.show_hidden;
            let bg = if is_active_toggle {
                Color32::from_rgba_unmultiplied(31, 111, 235, 65)
            } else if resp.hovered() {
                Color32::from_rgb(48, 54, 61)
            } else {
                Color32::from_rgb(33, 38, 45)
            };
            let border = if is_active_toggle {
                Stroke::new(1.2_f32, Color32::from_rgb(56, 189, 248))
            } else if resp.hovered() {
                Stroke::new(1.0_f32, Color32::from_rgb(88, 166, 255))
            } else {
                Stroke::new(1.0_f32, Color32::from_rgb(48, 54, 61))
            };

            let p = ui.painter();
            p.rect(r, Rounding::same(4.0), bg, border);
            let c = r.center();

            match i {
                0 => {
                    // Up folder (Golden folder + green up arrow)
                    draw_mini_folder(p, c, Color32::from_rgb(245, 158, 11));
                    let s = Stroke::new(1.6_f32, Color32::from_rgb(52, 211, 153));
                    p.line_segment([pos2(c.x, c.y + 3.0), pos2(c.x, c.y - 2.5)], s);
                    p.line_segment([pos2(c.x - 2.5, c.y), pos2(c.x, c.y - 2.5)], s);
                    p.line_segment([pos2(c.x + 2.5, c.y), pos2(c.x, c.y - 2.5)], s);
                }
                1 => {
                    // Upload (Green upward arrow)
                    let s = Stroke::new(1.8_f32, Color32::from_rgb(63, 185, 80));
                    p.line_segment([pos2(c.x, c.y + 3.5), pos2(c.x, c.y - 4.0)], s);
                    p.line_segment([pos2(c.x - 3.5, c.y - 0.5), pos2(c.x, c.y - 4.0)], s);
                    p.line_segment([pos2(c.x + 3.5, c.y - 0.5), pos2(c.x, c.y - 4.0)], s);
                    p.line_segment(
                        [pos2(c.x - 4.5, c.y + 5.0), pos2(c.x + 4.5, c.y + 5.0)],
                        Stroke::new(1.4_f32, Color32::from_rgb(139, 148, 158)),
                    );
                }
                2 => {
                    // Download (Blue downward arrow)
                    let s = Stroke::new(1.8_f32, Color32::from_rgb(56, 189, 248));
                    p.line_segment([pos2(c.x, c.y - 4.0), pos2(c.x, c.y + 3.5)], s);
                    p.line_segment([pos2(c.x - 3.5, c.y), pos2(c.x, c.y + 3.5)], s);
                    p.line_segment([pos2(c.x + 3.5, c.y), pos2(c.x, c.y + 3.5)], s);
                    p.line_segment(
                        [pos2(c.x - 4.5, c.y + 5.0), pos2(c.x + 4.5, c.y + 5.0)],
                        Stroke::new(1.4_f32, Color32::from_rgb(139, 148, 158)),
                    );
                }
                3 => {
                    // Refresh (Cyan circular arc + arrowheads)
                    let col = Color32::from_rgb(88, 166, 255);
                    p.circle_stroke(c, 4.5, Stroke::new(1.5_f32, col));
                    p.line_segment(
                        [pos2(c.x + 2.5, c.y - 4.5), pos2(c.x + 5.2, c.y - 4.5)],
                        Stroke::new(1.5_f32, col),
                    );
                    p.line_segment(
                        [pos2(c.x + 4.5, c.y - 4.5), pos2(c.x + 4.5, c.y - 1.8)],
                        Stroke::new(1.5_f32, col),
                    );
                }
                4 => {
                    // New Folder (Folder + green plus)
                    draw_mini_folder(p, pos2(c.x - 1.0, c.y), Color32::from_rgb(251, 191, 36));
                    let ps = Stroke::new(1.6_f32, Color32::from_rgb(63, 185, 80));
                    p.line_segment([pos2(c.x + 2.0, c.y + 1.5), pos2(c.x + 6.0, c.y + 1.5)], ps);
                    p.line_segment([pos2(c.x + 4.0, c.y - 0.5), pos2(c.x + 4.0, c.y + 3.5)], ps);
                }
                5 => {
                    // New File (Document page + plus)
                    let doc = Rect::from_center_size(pos2(c.x - 0.5, c.y), vec2(8.5, 11.0));
                    p.rect(
                        doc,
                        Rounding::same(1.2),
                        Color32::from_rgb(209, 217, 224),
                        Stroke::NONE,
                    );
                    let ps = Stroke::new(1.6_f32, Color32::from_rgb(31, 111, 235));
                    p.line_segment([pos2(c.x - 2.8, c.y), pos2(c.x + 1.8, c.y)], ps);
                    p.line_segment([pos2(c.x - 0.5, c.y - 2.3), pos2(c.x - 0.5, c.y + 2.3)], ps);
                }
                6 => {
                    // Delete (Red X)
                    let s = Stroke::new(2.0_f32, Color32::from_rgb(248, 81, 73));
                    p.line_segment([pos2(c.x - 3.8, c.y - 3.8), pos2(c.x + 3.8, c.y + 3.8)], s);
                    p.line_segment([pos2(c.x + 3.8, c.y - 3.8), pos2(c.x - 3.8, c.y + 3.8)], s);
                }
                7 => {
                    // Edit (Page with text lines / 'A')
                    let doc = Rect::from_center_size(c, vec2(9.0, 11.5));
                    p.rect(
                        doc,
                        Rounding::same(1.2),
                        Color32::from_rgb(33, 38, 45),
                        Stroke::new(1.2_f32, Color32::from_rgb(165, 180, 252)),
                    );
                    let ls = Stroke::new(1.2_f32, Color32::from_rgb(165, 180, 252));
                    p.line_segment([pos2(c.x - 2.5, c.y - 2.5), pos2(c.x + 2.5, c.y - 2.5)], ls);
                    p.line_segment([pos2(c.x - 2.5, c.y), pos2(c.x + 2.5, c.y)], ls);
                    p.line_segment([pos2(c.x - 2.5, c.y + 2.5), pos2(c.x + 1.0, c.y + 2.5)], ls);
                }
                8 => {
                    // Show/Hide Hidden Dotfiles (Folder + cyan dot)
                    draw_mini_folder(
                        p,
                        c,
                        if explorer.show_hidden {
                            Color32::from_rgb(251, 191, 36)
                        } else {
                            Color32::from_rgb(110, 118, 129)
                        },
                    );
                    p.circle_filled(
                        pos2(c.x + 3.5, c.y + 2.5),
                        2.5,
                        if explorer.show_hidden {
                            Color32::from_rgb(56, 189, 248)
                        } else {
                            Color32::from_rgb(139, 148, 158)
                        },
                    );
                }
                _ => {
                    // Permissions / Chmod (Lock icon)
                    let body = Rect::from_center_size(pos2(c.x, c.y + 2.0), vec2(8.5, 6.5));
                    p.rect_filled(body, Rounding::same(1.5), Color32::from_rgb(251, 191, 36));
                    p.circle_stroke(
                        pos2(c.x, c.y - 2.0),
                        2.6,
                        Stroke::new(1.4_f32, Color32::from_rgb(251, 191, 36)),
                    );
                }
            }

            if resp.on_hover_text(tooltips[i]).clicked() {
                clicked_tool = Some(i);
            }
        }

        if let Some(tool) = clicked_tool {
            match tool {
                0 => explorer.go_up(),
                1 => {
                    if let Some(files) = rfd::FileDialog::new().set_title("Select File(s) to Upload").pick_files() {
                        for path in files {
                            explorer.upload_file(path);
                        }
                    }
                }
                2 => {
                    let selected_paths = explorer.get_selected_paths();
                    if !selected_paths.is_empty()
                        && let Some(dest_dir) = rfd::FileDialog::new().set_title("Select Destination Folder").pick_folder() {
                            for path in selected_paths {
                                let file_name = path
                                    .file_name()
                                    .map(|n| n.to_string_lossy().to_string())
                                    .unwrap_or_else(|| "downloaded_file".to_string());
                                explorer.download_file(&path, dest_dir.join(file_name));
                            }
                        }
                }
                3 => explorer.refresh(),
                4 => {
                    explorer.show_new_folder = true;
                    explorer.new_folder_name = "new_folder".to_string();
                }
                5 => {
                    explorer.show_new_file = true;
                    explorer.new_file_name = "new_file.txt".to_string();
                }
                6 => {
                    let selected_paths = explorer.get_selected_paths();
                    if selected_paths.len() > 1 {
                        explorer.pending_delete = None;
                        explorer.pending_delete_many = selected_paths;
                    } else if let Some(first) = selected_paths.into_iter().next() {
                        explorer.pending_delete_many.clear();
                        explorer.pending_delete = Some(first);
                    }
                }
                7 => {
                    if explorer.can_edit_selected()
                        && let Some(selected) = explorer.selected_file.clone() {
                            explorer.open_editor(selected);
                        }
                }
                8 => {
                    explorer.toggle_show_hidden();
                }
                9 => {
                    if let Some(ref selected) = explorer.selected_file.clone() {
                        explorer.open_chmod(selected.clone());
                    }
                }
                _ => {}
            }
        }

        // 2. PATH BAR WITH BOOKMARK & DROPDOWN (Pixel-aligned row: `[★] /home/ruby/ [▾]`)
        let path_h = 25.0;
        let (path_row, _) = ui.allocate_exact_size(vec2(ui.available_width(), path_h), Sense::hover());
        let star_w = 22.0;
        let drop_w = 24.0;
        let star_rect = Rect::from_min_max(
            path_row.min,
            pos2(path_row.min.x + star_w, path_row.bottom()),
        );
        let drop_rect = Rect::from_min_max(
            pos2(path_row.right() - drop_w, path_row.top()),
            path_row.max,
        );
        let input_rect = Rect::from_min_max(
            pos2(star_rect.right() + 3.0, path_row.top()),
            pos2(drop_rect.left() - 4.0, path_row.bottom()),
        );

        let current_path_str = explorer.current_path.to_string_lossy().to_string();
        let is_current_bookmarked = explorer.is_bookmarked(&current_path_str);
        let star_resp = ui.interact(star_rect, ui.id().with("sftp_star_btn"), Sense::click());
        if star_resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        {
            let p = ui.painter();
            let bg = if star_resp.hovered() {
                Color32::from_rgb(48, 54, 61)
            } else {
                Color32::from_rgb(33, 38, 45)
            };
            p.rect(star_rect, Rounding::same(4.0), bg, Stroke::new(1.0_f32, Color32::from_rgb(48, 54, 61)));
            let star_color = if is_current_bookmarked {
                Color32::from_rgb(250, 204, 21) // Gold
            } else {
                Color32::from_rgb(107, 114, 128) // Dim gray
            };
            p.text(
                star_rect.center(),
                egui::Align2::CENTER_CENTER,
                "★",
                FontId::proportional(13.0),
                star_color,
            );
        }
        if star_resp.on_hover_text(if is_current_bookmarked { "Remove from SFTP Bookmarks" } else { "Bookmark this directory (⭐)" }).clicked() {
            explorer.toggle_bookmark(&current_path_str);
        }

        let path_resp = ui.put(
            input_rect,
            TextEdit::singleline(&mut explorer.path_input)
                .font(FontId::monospace(11.5))
                .margin(Margin::symmetric(7.0, 4.0))
                .desired_width((input_rect.width() - 16.0).max(60.0))
                .hint_text("/home/..."),
        );
        let mut path_copy_requested = false;
        path_resp.context_menu(|ui| {
            if ui.button("📋 Copy Path").clicked() {
                path_copy_requested = true;
                ui.close_menu();
            }
        });
        if path_copy_requested {
            copy_text_to_clipboard(ui, &explorer.path_input);
        }
        if path_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            let trimmed = explorer.path_input.trim().to_string();
            if !trimmed.is_empty() {
                explorer.set_path(PathBuf::from(trimmed));
            }
        }

        let drop_id = ui.id().with("sftp_path_history_popup");
        let drop_resp = ui.interact(drop_rect, ui.id().with("sftp_path_drop_btn"), Sense::click());
        if drop_resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        {
            let p = ui.painter();
            p.rect(
                drop_rect,
                Rounding::same(4.0),
                if drop_resp.hovered() {
                    Color32::from_rgb(48, 54, 61)
                } else {
                    Color32::from_rgb(33, 38, 45)
                },
                Stroke::new(1.0_f32, Color32::from_rgb(48, 54, 61)),
            );
            let dc = drop_rect.center();
            let cs = Stroke::new(1.6_f32, Color32::from_rgb(201, 209, 217));
            p.line_segment([pos2(dc.x - 3.5, dc.y - 1.5), pos2(dc.x, dc.y + 2.0)], cs);
            p.line_segment([pos2(dc.x, dc.y + 2.0), pos2(dc.x + 3.5, dc.y - 1.5)], cs);
        }
        let drop_resp = drop_resp.on_hover_text("Recent & Bookmarked Paths");
        if drop_resp.clicked() {
            ui.memory_mut(|m| m.toggle_popup(drop_id));
        }

        let mut jump_to: Option<PathBuf> = None;
        let mut remove_bookmark: Option<String> = None;
        egui::popup::popup_below_widget(
            ui,
            drop_id,
            &drop_resp,
            egui::PopupCloseBehavior::CloseOnClickOutside,
            |ui| {
                ui.set_min_width(230.0);
                if !explorer.bookmarks.is_empty() {
                    ui.label(RichText::new("⭐ Bookmarked Directories").size(11.0).color(Color32::from_rgb(250, 204, 21)).strong());
                    for bm in explorer.bookmarks.clone() {
                        ui.horizontal(|ui| {
                            if ui.button(RichText::new(&bm).monospace().size(11.0)).clicked() {
                                jump_to = Some(PathBuf::from(&bm));
                            }
                            if ui.small_button("🗑").on_hover_text("Remove bookmark").clicked() {
                                remove_bookmark = Some(bm);
                            }
                        });
                    }
                    ui.separator();
                }

                ui.label(RichText::new("Quick Jump").size(11.0).weak());
                if explorer.is_remote {
                    let remote_home = explorer.remote_home_dir.clone().unwrap_or_else(|| {
                        if explorer.remote_os == crate::sftp::remote::RemoteOsKind::Windows {
                            PathBuf::from("C:\\")
                        } else if let Some(ref lbl) = explorer.remote_label {
                            if lbl.starts_with("root@") {
                                PathBuf::from("/root")
                            } else if let Some((u, _)) = lbl.split_once('@') {
                                PathBuf::from(format!("/home/{}", u))
                            } else {
                                PathBuf::from("/root")
                            }
                        } else {
                            PathBuf::from("/root")
                        }
                    });
                    let home_disp = remote_home.to_string_lossy();
                    if ui.button(format!("Home ({})", home_disp)).clicked() {
                        jump_to = Some(remote_home);
                    }
                    if explorer.remote_os == crate::sftp::remote::RemoteOsKind::Posix {
                        if ui.button("/ (Root)").clicked() {
                            jump_to = Some(PathBuf::from("/"));
                        }
                        if ui.button("/tmp").clicked() {
                            jump_to = Some(PathBuf::from("/tmp"));
                        }
                        if ui.button("/var/log").clicked() {
                            jump_to = Some(PathBuf::from("/var/log"));
                        }
                        if ui.button("/etc").clicked() {
                            jump_to = Some(PathBuf::from("/etc"));
                        }
                    } else {
                        if ui.button("C:\\").clicked() {
                            jump_to = Some(PathBuf::from("C:\\"));
                        }
                        if ui.button("C:\\Users").clicked() {
                            jump_to = Some(PathBuf::from("C:\\Users"));
                        }
                        if ui.button("C:\\ProgramData").clicked() {
                            jump_to = Some(PathBuf::from("C:\\ProgramData"));
                        }
                    }
                } else {
                    if let Some(home) = dirs::home_dir()
                        && ui.button(format!("Home ({})", home.display())).clicked() {
                            jump_to = Some(home);
                        }
                    #[cfg(windows)]
                    {
                        if ui.button("C:\\").clicked() {
                            jump_to = Some(PathBuf::from("C:\\"));
                        }
                    }
                    #[cfg(not(windows))]
                    {
                        if ui.button("/ (Root)").clicked() {
                            jump_to = Some(PathBuf::from("/"));
                        }
                        if ui.button("/tmp").clicked() {
                            jump_to = Some(PathBuf::from("/tmp"));
                        }
                        if ui.button("/var/log").clicked() {
                            jump_to = Some(PathBuf::from("/var/log"));
                        }
                        if ui.button("/etc").clicked() {
                            jump_to = Some(PathBuf::from("/etc"));
                        }
                    }
                }
                if !explorer.path_history.is_empty() {
                    ui.separator();
                    ui.label(RichText::new("Recent Directories").size(11.0).weak());
                    for hist in explorer.path_history.clone() {
                        if ui.button(hist.to_string_lossy()).clicked() {
                            jump_to = Some(hist);
                        }
                    }
                }
            },
        );
        if let Some(bm_to_del) = remove_bookmark {
            explorer.toggle_bookmark(&bm_to_del);
        }
        if let Some(target) = jump_to {
            explorer.set_path(target);
        }

        // 2b. MOBAXTERM SFTP CONTROLS (Remote monitoring & Follow terminal folder)
        ui.add_space(3.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            // Mini monitor icon with ECG pulse wave
            let (mon_icon_rect, _) = ui.allocate_exact_size(vec2(15.0, 14.0), Sense::hover());
            let p = ui.painter();
            let scr = Rect::from_min_size(pos2(mon_icon_rect.left(), mon_icon_rect.top() + 1.0), vec2(15.0, 10.0));
            p.rect(
                scr,
                Rounding::same(2.0),
                Color32::from_rgb(18, 22, 28),
                Stroke::new(1.0_f32, Color32::from_rgb(75, 85, 99)),
            );
            // Monitor stand
            p.line_segment(
                [pos2(scr.center().x, scr.bottom()), pos2(scr.center().x, scr.bottom() + 2.0)],
                Stroke::new(1.2_f32, Color32::from_rgb(107, 114, 128)),
            );
            p.line_segment(
                [pos2(scr.center().x - 3.5, scr.bottom() + 2.0), pos2(scr.center().x + 3.5, scr.bottom() + 2.0)],
                Stroke::new(1.2_f32, Color32::from_rgb(107, 114, 128)),
            );
            // Green pulse line
            let cy = scr.center().y;
            let lx = scr.left() + 2.0;
            let rx = scr.right() - 2.0;
            let mx = scr.center().x;
            let s_pulse = Stroke::new(1.1_f32, Color32::from_rgb(52, 211, 153));
            p.line_segment([pos2(lx, cy), pos2(mx - 3.0, cy)], s_pulse);
            p.line_segment([pos2(mx - 3.0, cy), pos2(mx - 1.5, cy - 2.5)], s_pulse);
            p.line_segment([pos2(mx - 1.5, cy - 2.5), pos2(mx + 0.5, cy + 2.5)], s_pulse);
            p.line_segment([pos2(mx + 0.5, cy + 2.5), pos2(mx + 2.0, cy)], s_pulse);
            p.line_segment([pos2(mx + 2.0, cy), pos2(rx, cy)], s_pulse);

            let mon_text = if explorer.remote_monitoring_enabled {
                RichText::new("Remote monitoring").size(11.5).color(Color32::from_rgb(52, 211, 153))
            } else {
                RichText::new("Remote monitoring").size(11.5).color(Color32::from_rgb(201, 209, 217))
            };
            if ui.add(egui::Label::new(mon_text).sense(Sense::click()))
                .on_hover_text("Toggle remote host hardware monitoring (CPU, RAM, Network, Disk)")
                .clicked()
            {
                explorer.remote_monitoring_enabled = !explorer.remote_monitoring_enabled;
            }
        });

        ui.horizontal(|ui| {
            ui.checkbox(
                &mut explorer.follow_terminal_folder,
                RichText::new("Follow terminal folder")
                    .size(11.5)
                    .color(Color32::from_rgb(201, 209, 217)),
            )
            .on_hover_text("Automatically sync SFTP folder when changing directory (cd) in terminal");
        });
        ui.add_space(2.0);

        // Disk Usage Bar right below the Path Bar (RustyBox style)
        let (used_b, total_b, used_pct) = explorer.disk_usage_stats();
        let (disk_r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 14.0), Sense::hover());
        {
            let p = ui.painter();
            p.rect(
                disk_r,
                Rounding::same(3.0),
                Color32::from_rgb(13, 17, 23),
                Stroke::new(1.0_f32, Color32::from_rgb(40, 46, 56)),
            );
            let fill_w = ((used_pct / 100.0).clamp(0.0, 1.0) * (disk_r.width() - 2.0)).max(2.0);
            p.rect_filled(
                Rect::from_min_size(
                    pos2(disk_r.left() + 1.0, disk_r.top() + 1.0),
                    vec2(fill_w, disk_r.height() - 2.0),
                ),
                Rounding::same(2.0),
                Color32::from_rgb(31, 111, 235),
            );
            let used_gb = used_b as f64 / 1_073_741_824.0;
            let total_gb = total_b as f64 / 1_073_741_824.0;
            p.text(
                disk_r.center(),
                Align2::CENTER_CENTER,
                format!("Disk: {:.0}% ({:.1} / {:.1} GB)", used_pct, used_gb, total_gb),
                FontId::monospace(9.5),
                Color32::from_rgb(230, 237, 243),
            );
        }

        if let Some(ref prog) = explorer.transfer_progress.clone() {
            egui::Frame::none()
                .fill(Color32::from_rgb(22, 27, 34))
                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(56, 189, 248)))
                .rounding(Rounding::same(5.0))
                .inner_margin(Margin::symmetric(6.0, 5.0))
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            let icon = if prog.is_upload { "⬆ Uploading" } else { "⬇ Downloading" };
                            ui.label(RichText::new(format!("{}: {}", icon, prog.file_name)).strong().size(11.0).color(Color32::from_rgb(56, 189, 248)));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.small_button("✕ Cancel").on_hover_text("Abort file transfer and remove incomplete file").clicked() {
                                    explorer.cancel_current_transfer();
                                }
                            });
                        });

                        // Progress bar widget
                        let bar_h = 10.0;
                        let (bar_r, _) = ui.allocate_exact_size(vec2(ui.available_width(), bar_h), Sense::hover());
                        let p = ui.painter();
                        p.rect_filled(bar_r, Rounding::same(2.0), Color32::from_rgb(13, 17, 23));
                        let fill_w = ((prog.percent / 100.0).clamp(0.0, 1.0) * bar_r.width()).max(1.0);
                        p.rect_filled(
                            Rect::from_min_size(bar_r.min, vec2(fill_w, bar_r.height())),
                            Rounding::same(2.0),
                            Color32::from_rgb(34, 197, 94),
                        );

                        let trans_mb = prog.transferred_bytes as f64 / (1024.0 * 1024.0);
                        let total_mb = prog.total_bytes as f64 / (1024.0 * 1024.0);
                        let speed_kb = prog.bytes_per_sec / 1024.0;
                        let speed_str = if speed_kb > 1024.0 {
                            format!("{:.1} MB/s", speed_kb / 1024.0)
                        } else {
                            format!("{:.0} KB/s", speed_kb)
                        };

                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("{:.1}% ({:.1}/{:.1} MB)", prog.percent, trans_mb, total_mb)).size(10.0).weak());
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(RichText::new(speed_str).size(10.0).color(Color32::from_rgb(139, 148, 158)));
                            });
                        });
                    });
                });
        }

        let mut error_reconnect = false;
        if !explorer.is_loading
            && let Some(ref err) = explorer.error_message {
                let is_disconnect = explorer.is_confirmed_disconnected();
                // Only show top error line if entries is not empty or error is not a disconnect
                if !explorer.entries.is_empty() || !is_disconnect {
                    ui.horizontal(|ui| {
                        let display_err = if is_disconnect { "⚠️ SFTP disconnected" } else { err.as_str() };
                        ui.colored_label(Color32::from_rgb(248, 81, 73), RichText::new(display_err).size(11.0))
                            .on_hover_text(err);
                        if is_disconnect
                            && ui.add_sized([80.0, 19.0], egui::Button::new(RichText::new("🔄 Reconnect").size(10.5).color(Color32::from_rgb(88, 166, 255)))).clicked() {
                                error_reconnect = true;
                            }
                    });
                }
            }
        if error_reconnect {
            explorer.refresh();
        }
        if let Some(ref msg) = explorer.status_message {
            ui.colored_label(Color32::from_rgb(63, 185, 80), RichText::new(msg).size(11.0));
        }

        // 3. RECESSED SFTP FILE LIST TABLE (With `Name ▼` Header + `..` Row + Vector Icons)
        let table_h = (ui.available_height() - 6.0).max(120.0);

        let table_frame_resp = egui::Frame::none()
            .fill(Color32::from_rgb(13, 17, 23))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(40, 46, 56)))
            .rounding(Rounding::same(6.0))
            .inner_margin(Margin::same(1.0))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());

                let col_size_w = 68.0f32;
                let col_date_w = 115.0f32;
                let col_owner_w = 60.0f32;
                let col_group_w = 60.0f32;
                let col_access_w = 85.0f32;
                let fixed_cols_total = col_size_w + col_date_w + col_owner_w + col_group_w + col_access_w; // 388.0

                let avail_w = ui.available_width();
                let base_name_w = (avail_w - fixed_cols_total).max(165.0);
                let col_name_w = explorer.col_name_width_override.unwrap_or(base_name_w).max(120.0);
                let table_w = col_name_w + fixed_cols_total;

                let mut clicked_dir: Option<PathBuf> = None;
                let mut double_clicked_file: Option<PathBuf> = None;
                let mut context_chmod: Option<PathBuf> = None;
                let mut context_del: Option<PathBuf> = None;
                let mut context_del_many: Vec<PathBuf> = Vec::new();
                let mut context_rename: Option<PathBuf> = None;
                let mut context_properties: Option<FileEntry> = None;
                let mut context_download: Option<PathBuf> = None;
                let mut context_download_many: Vec<PathBuf> = Vec::new();
                let mut context_new_folder = false;
                let mut context_new_file = false;
                let mut context_refresh = false;
                let mut context_upload = false;
                let mut context_upload_folder = false;
                let mut context_copy_text: Option<String> = None;
                let mut context_toggle_hidden = false;
                let mut context_terminal_here: Option<String> = None;
                let mut context_terminal_paste_path: Option<String> = None;
                let mut context_open_external: Option<(PathBuf, ExternalOpenMode)> = None;
                let mut context_compare: Option<PathBuf> = None;
                let mut go_parent = false;

                let (ctrl_held, shift_held, primary_down, primary_released, pointer_pos) = ui.ctx().input(|i| {
                    (
                        i.modifiers.ctrl || i.modifiers.command,
                        i.modifiers.shift,
                        i.pointer.primary_down(),
                        i.pointer.primary_released(),
                        i.pointer.interact_pos().or_else(|| i.pointer.hover_pos()),
                    )
                });
                let mut row_bounds: Vec<(Rect, PathBuf)> = Vec::with_capacity(explorer.entries.len());
                let mut click_single: Option<PathBuf> = None;
                let mut click_toggle: Option<PathBuf> = None;
                let mut click_range: Option<PathBuf> = None;
                let mut clear_sel = false;

                ScrollArea::horizontal()
                    .id_salt("sftp_horiz_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.set_min_width(table_w);

                        // 1. STICKY HEADER ROW (Matching MobaXterm 6 Columns)
                        let (hdr_rect, _) = ui.allocate_exact_size(vec2(table_w, 24.0), Sense::hover());
                        let p = ui.painter();
                        p.rect_filled(
                            hdr_rect,
                            Rounding {
                                nw: 5.0,
                                ne: 5.0,
                                sw: 0.0,
                                se: 0.0,
                            },
                            Color32::from_rgb(28, 33, 40),
                        );
                        p.line_segment(
                            [pos2(hdr_rect.left(), hdr_rect.bottom()), pos2(hdr_rect.right(), hdr_rect.bottom())],
                            Stroke::new(1.0_f32, Color32::from_rgb(48, 54, 61)),
                        );

                        // Header Column Rectangles
                        let r_name = Rect::from_min_max(hdr_rect.min, pos2(hdr_rect.min.x + col_name_w, hdr_rect.max.y));
                        let r_size = Rect::from_min_max(pos2(r_name.max.x, hdr_rect.min.y), pos2(r_name.max.x + col_size_w, hdr_rect.max.y));
                        let r_date = Rect::from_min_max(pos2(r_size.max.x, hdr_rect.min.y), pos2(r_size.max.x + col_date_w, hdr_rect.max.y));
                        let r_owner = Rect::from_min_max(pos2(r_date.max.x, hdr_rect.min.y), pos2(r_date.max.x + col_owner_w, hdr_rect.max.y));
                        let r_group = Rect::from_min_max(pos2(r_owner.max.x, hdr_rect.min.y), pos2(r_owner.max.x + col_group_w, hdr_rect.max.y));
                        let r_access = Rect::from_min_max(pos2(r_group.max.x, hdr_rect.min.y), pos2(hdr_rect.max.x, hdr_rect.max.y));

                        // Vertical Column Dividers & Resizer for Name Column
                        let resizer_rect = Rect::from_center_size(pos2(r_name.max.x, hdr_rect.center().y), vec2(8.0, hdr_rect.height()));
                        let resizer_id = ui.id().with("sftp_col_name_resizer");
                        let resizer_resp = ui.interact(resizer_rect, resizer_id, Sense::drag());
                        if resizer_resp.hovered() || resizer_resp.dragged() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeColumn);
                        }
                        if resizer_resp.dragged() {
                            let delta = resizer_resp.drag_delta().x;
                            let new_w = (col_name_w + delta).clamp(100.0, 1600.0);
                            explorer.col_name_width_override = Some(new_w);
                        }
                        if resizer_resp.double_clicked() {
                            // Reset to auto/default width
                            explorer.col_name_width_override = None;
                        }

                        for &div_x in &[r_name.max.x, r_size.max.x, r_date.max.x, r_owner.max.x, r_group.max.x] {
                            let stroke_col = if div_x == r_name.max.x && (resizer_resp.hovered() || resizer_resp.dragged()) {
                                Color32::from_rgb(88, 166, 255)
                            } else {
                                Color32::from_rgb(48, 54, 61)
                            };
                            p.line_segment(
                                [pos2(div_x, hdr_rect.top() + 3.0), pos2(div_x, hdr_rect.bottom() - 3.0)],
                                Stroke::new(1.0_f32, stroke_col),
                            );
                        }

                        // Column Header Interactions
                        let resp_name = ui.interact(r_name, ui.id().with("hdr_name"), Sense::click()).on_hover_text("Sort by Name");
                        let resp_size = ui.interact(r_size, ui.id().with("hdr_size"), Sense::click()).on_hover_text("Sort by Size (KB)");
                        let resp_date = ui.interact(r_date, ui.id().with("hdr_date"), Sense::click()).on_hover_text("Sort by Last Modified");
                        let resp_owner = ui.interact(r_owner, ui.id().with("hdr_owner"), Sense::click()).on_hover_text("Sort by Owner");
                        let resp_group = ui.interact(r_group, ui.id().with("hdr_group"), Sense::click()).on_hover_text("Sort by Group");
                        let resp_access = ui.interact(r_access, ui.id().with("hdr_access"), Sense::click()).on_hover_text("Sort by Access Permissions");

                        if resp_name.hovered() || resp_size.hovered() || resp_date.hovered() || resp_owner.hovered() || resp_group.hovered() || resp_access.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }

                        if resp_name.clicked() { explorer.set_sort_column(SftpSortColumn::Name); }
                        if resp_size.clicked() { explorer.set_sort_column(SftpSortColumn::Size); }
                        if resp_date.clicked() { explorer.set_sort_column(SftpSortColumn::Modified); }
                        if resp_owner.clicked() { explorer.set_sort_column(SftpSortColumn::Owner); }
                        if resp_group.clicked() { explorer.set_sort_column(SftpSortColumn::Group); }
                        if resp_access.clicked() { explorer.set_sort_column(SftpSortColumn::Access); }

                        // Draw Header Labels
                        p.text(
                            pos2(r_name.left() + 8.0, r_name.center().y),
                            Align2::LEFT_CENTER,
                            "Name",
                            FontId::proportional(11.5),
                            Color32::from_rgb(201, 209, 217),
                        );
                        if explorer.sort_column == SftpSortColumn::Name {
                            draw_sort_arrow(p, r_name.left() + 48.0, r_name.center().y, explorer.sort_ascending);
                        }

                        p.text(
                            pos2(r_size.right() - 8.0, r_size.center().y),
                            Align2::RIGHT_CENTER,
                            "Size (KB)",
                            FontId::proportional(11.0),
                            Color32::from_rgb(139, 148, 158),
                        );
                        if explorer.sort_column == SftpSortColumn::Size {
                            draw_sort_arrow(p, r_size.left() + 10.0, r_size.center().y, explorer.sort_ascending);
                        }

                        p.text(
                            pos2(r_date.left() + 8.0, r_date.center().y),
                            Align2::LEFT_CENTER,
                            "Last modified",
                            FontId::proportional(11.0),
                            Color32::from_rgb(139, 148, 158),
                        );
                        if explorer.sort_column == SftpSortColumn::Modified {
                            draw_sort_arrow(p, r_date.left() + 86.0, r_date.center().y, explorer.sort_ascending);
                        }

                        p.text(
                            pos2(r_owner.left() + 8.0, r_owner.center().y),
                            Align2::LEFT_CENTER,
                            "Owner",
                            FontId::proportional(11.0),
                            Color32::from_rgb(139, 148, 158),
                        );
                        if explorer.sort_column == SftpSortColumn::Owner {
                            draw_sort_arrow(p, r_owner.left() + 48.0, r_owner.center().y, explorer.sort_ascending);
                        }

                        p.text(
                            pos2(r_group.left() + 8.0, r_group.center().y),
                            Align2::LEFT_CENTER,
                            "Group",
                            FontId::proportional(11.0),
                            Color32::from_rgb(139, 148, 158),
                        );
                        if explorer.sort_column == SftpSortColumn::Group {
                            draw_sort_arrow(p, r_group.left() + 46.0, r_group.center().y, explorer.sort_ascending);
                        }

                        p.text(
                            pos2(r_access.left() + 8.0, r_access.center().y),
                            Align2::LEFT_CENTER,
                            "Access",
                            FontId::proportional(11.0),
                            Color32::from_rgb(139, 148, 158),
                        );
                        if explorer.sort_column == SftpSortColumn::Access {
                            draw_sort_arrow(p, r_access.left() + 50.0, r_access.center().y, explorer.sort_ascending);
                        }

                        let list_h = (table_h - 26.0).max(90.0);

                        ScrollArea::vertical()
                            .id_salt("sftp_entries_scroll")
                            .max_height(list_h)
                            .min_scrolled_height(list_h)
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                ui.set_min_width(table_w);
                                ui.spacing_mut().item_spacing.y = 1.0;

                                // Parent directory `..` row (always at top just like MobaXterm)
                                let (up_rect, up_resp) =
                                    ui.allocate_exact_size(vec2(table_w, 22.0), Sense::click());
                                if up_resp.hovered() {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    ui.painter().rect_filled(
                                        up_rect,
                                        Rounding::same(3.0),
                                        Color32::from_rgb(28, 33, 40),
                                    );
                                }
                                draw_mini_folder(
                                    ui.painter(),
                                    pos2(up_rect.left() + 14.0, up_rect.center().y),
                                    Color32::from_rgb(245, 158, 11),
                                );
                                ui.painter().text(
                                    pos2(up_rect.left() + 26.0, up_rect.center().y),
                                    Align2::LEFT_CENTER,
                                    "..",
                                    FontId::monospace(12.0),
                                    Color32::from_rgb(230, 237, 243),
                                );
                                let up_resp = up_resp.on_hover_text("Double-click to go to parent directory");
                                if up_resp.double_clicked() {
                                    go_parent = true;
                                }
                                up_resp.context_menu(|ui| {
                                    if ui.button("⬆  Parent directory").clicked() {
                                        go_parent = true;
                                        ui.close_menu();
                                    }
                                    ui.separator();
                                    if ui.button("🔄  Refresh current folder").clicked() {
                                        context_refresh = true;
                                        ui.close_menu();
                                    }
                                });

                                if explorer.is_confirmed_disconnected() && explorer.entries.is_empty() {
                                    ui.add_space(20.0);
                                    let badge_w = 200.0f32.min(avail_w - 24.0);
                                    let badge_h = 26.0f32;
                                    let offset_x = ((avail_w - badge_w) / 2.0).max(6.0);

                                    let (row_rect, _) = ui.allocate_exact_size(vec2(table_w, badge_h + 8.0), Sense::hover());
                                    let badge_rect = Rect::from_min_size(
                                        pos2(row_rect.left() + offset_x, row_rect.top() + 4.0),
                                        vec2(badge_w, badge_h),
                                    );

                                    let p = ui.painter();
                                    p.rect_filled(badge_rect, Rounding::same(13.0), Color32::from_rgb(22, 27, 34));
                                    p.rect_stroke(badge_rect, Rounding::same(13.0), Stroke::new(1.0_f32, Color32::from_rgb(56, 68, 77)));

                                    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(badge_rect), |ui| {
                                        ui.horizontal_centered(|ui| {
                                            ui.add_space(10.0);
                                            ui.label(RichText::new("⚠️ Disconnected").size(11.0).color(Color32::from_rgb(248, 81, 73)));
                                            ui.label(RichText::new("·").size(10.0).color(Color32::from_rgb(100, 116, 139)));
                                            if ui.add(egui::Button::new(RichText::new("🔄 Reconnect").size(10.5).color(Color32::from_rgb(88, 166, 255))).frame(false)).on_hover_text("Tái kết nối SFTP với máy chủ").clicked() {
                                                context_refresh = true;
                                            }
                                        });
                                    });
                                }

                                for entry in &explorer.entries {
                                    let is_selected = explorer.is_entry_selected(&entry.path);
                                    let (row_rect, resp) =
                                        ui.allocate_exact_size(vec2(table_w, 22.0), Sense::click_and_drag());
                                    row_bounds.push((row_rect, entry.path.clone()));
                                    if resp.hovered() {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }

                                    let p = ui.painter();
                                    if is_selected {
                                        p.rect(
                                            row_rect,
                                            Rounding::same(3.0),
                                            Color32::from_rgba_unmultiplied(31, 111, 235, 55),
                                            Stroke::new(1.0_f32, Color32::from_rgb(56, 189, 248)),
                                        );
                                    } else if resp.hovered() {
                                        p.rect_filled(
                                            row_rect,
                                            Rounding::same(3.0),
                                            Color32::from_rgb(26, 31, 38),
                                        );
                                    }

                                    // Col 1: Icon + Name
                                    let icon_c = pos2(row_rect.left() + 14.0, row_rect.center().y);
                                    if entry.is_dir {
                                        let folder_col = if entry.is_hidden {
                                            Color32::from_rgb(217, 159, 56)
                                        } else {
                                            Color32::from_rgb(251, 191, 36)
                                        };
                                        draw_mini_folder(p, icon_c, folder_col);
                                    } else {
                                        let doc_col = if entry.is_hidden {
                                            Color32::from_rgb(139, 148, 158)
                                        } else if entry.name.ends_with(".sh")
                                            || entry.name.ends_with(".py")
                                            || entry.name.ends_with(".rs")
                                            || entry.name.ends_with(".rb")
                                        {
                                            Color32::from_rgb(88, 166, 255)
                                        } else {
                                            Color32::from_rgb(201, 209, 217)
                                        };
                                        draw_mini_file(p, icon_c, doc_col);
                                    }

                                    let name_clip = Rect::from_min_max(
                                        pos2(row_rect.left() + 26.0, row_rect.top()),
                                        pos2(row_rect.left() + col_name_w - 6.0, row_rect.bottom()),
                                    );
                                    let name_color = if entry.is_dir {
                                        if entry.is_hidden {
                                            Color32::from_rgb(210, 218, 226)
                                        } else {
                                            Color32::from_rgb(240, 246, 252)
                                        }
                                    } else if entry.is_hidden {
                                        Color32::from_rgb(165, 175, 186)
                                    } else {
                                        Color32::from_rgb(215, 222, 230)
                                    };

                                    p.with_clip_rect(name_clip).text(
                                        pos2(row_rect.left() + 26.0, row_rect.center().y),
                                        Align2::LEFT_CENTER,
                                        &entry.name,
                                        FontId::proportional(12.0),
                                        name_color,
                                    );

                                    // Col 2: Size (KB)
                                    let size_right_x = row_rect.left() + col_name_w + col_size_w - 8.0;
                                    if !entry.is_dir {
                                        let size_str = format_size_kb(entry.size);
                                        p.text(
                                            pos2(size_right_x, row_rect.center().y),
                                            Align2::RIGHT_CENTER,
                                            &size_str,
                                            FontId::monospace(10.5),
                                            Color32::from_rgb(139, 148, 158),
                                        );
                                    }

                                    // Col 3: Last modified
                                    let date_left_x = row_rect.left() + col_name_w + col_size_w + 8.0;
                                    let date_str = format_modified_time(entry.modified);
                                    if !date_str.is_empty() {
                                        p.text(
                                            pos2(date_left_x, row_rect.center().y),
                                            Align2::LEFT_CENTER,
                                            &date_str,
                                            FontId::monospace(10.5),
                                            Color32::from_rgb(139, 148, 158),
                                        );
                                    }

                                    // Col 4: Owner
                                    let owner_left_x = date_left_x + col_date_w;
                                    p.text(
                                        pos2(owner_left_x, row_rect.center().y),
                                        Align2::LEFT_CENTER,
                                        &entry.owner,
                                        FontId::proportional(11.0),
                                        Color32::from_rgb(201, 209, 217),
                                    );

                                    // Col 5: Group
                                    let group_left_x = owner_left_x + col_owner_w;
                                    p.text(
                                        pos2(group_left_x, row_rect.center().y),
                                        Align2::LEFT_CENTER,
                                        &entry.group,
                                        FontId::proportional(11.0),
                                        Color32::from_rgb(139, 148, 158),
                                    );

                                    // Col 6: Access
                                    let access_left_x = group_left_x + col_group_w;
                                    p.text(
                                        pos2(access_left_x, row_rect.center().y),
                                        Align2::LEFT_CENTER,
                                        &entry.permissions,
                                        FontId::monospace(10.5),
                                        Color32::from_rgb(139, 148, 158),
                                    );

                                    if resp.clicked() {
                                        if ctrl_held {
                                            click_toggle = Some(entry.path.clone());
                                        } else if shift_held {
                                            click_range = Some(entry.path.clone());
                                        } else {
                                            click_single = Some(entry.path.clone());
                                        }
                                    }
                                    if resp.secondary_clicked() && !is_selected {
                                        click_single = Some(entry.path.clone());
                                    }
                                    if resp.drag_started_by(egui::PointerButton::Primary) {
                                        explorer.drag_select_start = pointer_pos.or(Some(row_rect.center()));
                                    }
                                    if resp.middle_clicked() {
                                        context_terminal_paste_path = Some(entry.path.to_string_lossy().to_string());
                                    }
                                    if resp.double_clicked() {
                                        if entry.is_dir {
                                            clicked_dir = Some(entry.path.clone());
                                        } else {
                                            double_clicked_file = Some(entry.path.clone());
                                        }
                                    }

                                    let date_display = if date_str.is_empty() { "-".to_string() } else { date_str };
                                    let tip = format!(
                                        "{}\nSize: {}\nLast modified: {}\nOwner / Group: {} / {}\nAccess: {}",
                                        entry.path.display(),
                                        if entry.is_dir { "Directory".to_string() } else { format!("{} bytes ({} KB)", entry.size, format_size_kb(entry.size)) },
                                        date_display,
                                        entry.owner,
                                        entry.group,
                                        entry.permissions,
                                    );
                                    let path_clone = entry.path.clone();
                                    let is_dir_entry = entry.is_dir;
                                    let entry_clone = entry.clone();
                                    resp.on_hover_text(tip).context_menu(|ui| {
                                        let sel_paths = explorer.get_selected_paths();
                                        let multi_count = if sel_paths.contains(&path_clone) {
                                            sel_paths.len()
                                        } else {
                                            1
                                        };

                                        if multi_count > 1 {
                                            ui.label(
                                                RichText::new(format!("📑 {} items selected", multi_count))
                                                    .strong()
                                                    .color(Color32::from_rgb(56, 189, 248)),
                                            );
                                            ui.separator();
                                            if ui.button(format!("📥  Download {} selected items...", multi_count)).clicked() {
                                                context_download_many = sel_paths.clone();
                                                ui.close_menu();
                                            }
                                            if ui.button(format!("🗑  Delete {} selected items...", multi_count)).clicked() {
                                                context_del_many = sel_paths.clone();
                                                ui.close_menu();
                                            }
                                            ui.separator();
                                            if ui.button(format!("📋  Copy {} file paths", multi_count)).clicked() {
                                                let joined = sel_paths
                                                    .iter()
                                                    .map(|p| p.to_string_lossy().to_string())
                                                    .collect::<Vec<_>>()
                                                    .join("\n");
                                                context_copy_text = Some(joined);
                                                ui.close_menu();
                                            }
                                            if ui.button("🖥  Paste selected paths to terminal").clicked() {
                                                let joined = sel_paths
                                                    .iter()
                                                    .map(|p| format!("\"{}\"", p.to_string_lossy()))
                                                    .collect::<Vec<_>>()
                                                    .join(" ");
                                                context_terminal_paste_path = Some(joined);
                                                ui.close_menu();
                                            }
                                            return;
                                        }

                                if is_dir_entry {
                                    if ui.button("📂  Open").clicked() {
                                        clicked_dir = Some(path_clone.clone());
                                        ui.close_menu();
                                    }
                                    if ui.button("📥  Download folder").clicked() {
                                        context_download = Some(path_clone.clone());
                                        ui.close_menu();
                                    }
                                    ui.separator();
                                    if ui.button("📤  Upload file(s) here...").clicked() {
                                        context_upload = true;
                                        ui.close_menu();
                                    }
                                    if ui.button("📁  Upload folder here...").clicked() {
                                        context_upload_folder = true;
                                        ui.close_menu();
                                    }
                                    ui.separator();
                                    if ui.button("🗑  Delete").clicked() {
                                        context_del = Some(path_clone.clone());
                                        ui.close_menu();
                                    }
                                    if ui.button("✏  Rename").clicked() {
                                        context_rename = Some(path_clone.clone());
                                        ui.close_menu();
                                    }
                                    ui.separator();
                                    if ui.button("📋  Copy file path").clicked() {
                                        context_copy_text = Some(path_clone.to_string_lossy().to_string());
                                        ui.close_menu();
                                    }
                                    if ui.button("🖥  Copy file path to terminal (Middle mouse click)").clicked() {
                                        context_terminal_paste_path = Some(path_clone.to_string_lossy().to_string());
                                        ui.close_menu();
                                    }
                                    if ui.button("ℹ️  Properties").clicked() {
                                        context_properties = Some(entry_clone);
                                        ui.close_menu();
                                    }
                                    if ui.button("🔒  Permissions").clicked() {
                                        context_chmod = Some(path_clone.clone());
                                        ui.close_menu();
                                    }
                                } else {
                                    if ui.button("📂  Open").clicked() {
                                        double_clicked_file = Some(path_clone.clone());
                                        ui.close_menu();
                                    }
                                    if ui.button("📄  Open with default text editor").clicked() {
                                        context_open_external = Some((path_clone.clone(), ExternalOpenMode::TextEditor));
                                        ui.close_menu();
                                    }
                                    if ui.button("⚙️  Open with...").clicked() {
                                        context_open_external = Some((path_clone.clone(), ExternalOpenMode::OpenWithDialog));
                                        ui.close_menu();
                                    }
                                    if ui.button("📎  Open with default program...").clicked() {
                                        context_open_external = Some((path_clone.clone(), ExternalOpenMode::DefaultProgram));
                                        ui.close_menu();
                                    }
                                    if ui.button("🔍  Compare file with...").clicked() {
                                        context_compare = Some(path_clone.clone());
                                        ui.close_menu();
                                    }
                                    if ui.button("📥  Download").clicked() {
                                        context_download = Some(path_clone.clone());
                                        ui.close_menu();
                                    }
                                    ui.separator();
                                    if ui.button("📤  Upload file(s)...").clicked() {
                                        context_upload = true;
                                        ui.close_menu();
                                    }
                                    if ui.button("📁  Upload folder...").clicked() {
                                        context_upload_folder = true;
                                        ui.close_menu();
                                    }
                                    ui.separator();
                                    if ui.button("🗑  Delete").clicked() {
                                        context_del = Some(path_clone.clone());
                                        ui.close_menu();
                                    }
                                    if ui.button("✏  Rename").clicked() {
                                        context_rename = Some(path_clone.clone());
                                        ui.close_menu();
                                    }
                                    ui.separator();
                                    if ui.button("📋  Copy file path").clicked() {
                                        context_copy_text = Some(path_clone.to_string_lossy().to_string());
                                        ui.close_menu();
                                    }
                                    if ui.button("🖥  Copy file path to terminal (Middle mouse click)").clicked() {
                                        context_terminal_paste_path = Some(path_clone.to_string_lossy().to_string());
                                        ui.close_menu();
                                    }
                                    if ui.button("ℹ️  Properties").clicked() {
                                        context_properties = Some(entry_clone);
                                        ui.close_menu();
                                    }
                                    if ui.button("🔒  Permissions").clicked() {
                                        context_chmod = Some(path_clone.clone());
                                        ui.close_menu();
                                    }
                                }
                            });
                        }

                        // Background empty space for Folder Context Menu & Rubber-band Start
                        let avail_h = ui.available_height().max(50.0);
                        let (bg_rect, bg_resp) = ui.allocate_exact_size(vec2(table_w, avail_h), Sense::click_and_drag());
                        if bg_resp.clicked() && !ctrl_held && !shift_held {
                            clear_sel = true;
                        }
                        if bg_resp.drag_started_by(egui::PointerButton::Primary) {
                            explorer.drag_select_start = pointer_pos.or(Some(bg_rect.min));
                        }
                        bg_resp.context_menu(|ui| {
                            if ui.button("⬆  Parent directory").clicked() {
                                go_parent = true;
                                ui.close_menu();
                            }
                            ui.separator();
                            if ui.button("📁  New directory...").clicked() {
                                context_new_folder = true;
                                ui.close_menu();
                            }
                            if ui.button("📄  New empty file...").clicked() {
                                context_new_file = true;
                                ui.close_menu();
                            }
                            ui.separator();
                            if ui.button("🔄  Refresh current folder").clicked() {
                                context_refresh = true;
                                ui.close_menu();
                            }
                            if ui.button("📤  Upload file(s)...").clicked() {
                                context_upload = true;
                                ui.close_menu();
                            }
                            if ui.button("📁  Upload folder...").clicked() {
                                context_upload_folder = true;
                                ui.close_menu();
                            }
                            ui.separator();
                            if ui.button("📋  Copy current folder path").clicked() {
                                context_copy_text = Some(explorer.current_path.to_string_lossy().to_string());
                                ui.close_menu();
                            }
                            let hidden_label = if explorer.show_hidden {
                                "👁  Hide hidden files (.*)"
                            } else {
                                "👁  Show hidden files (.*)"
                            };
                            if ui.button(hidden_label).clicked() {
                                context_toggle_hidden = true;
                                ui.close_menu();
                            }
                            if ui.button("🖥  Open terminal here").clicked() {
                                context_terminal_here = Some(explorer.current_path.to_string_lossy().to_string());
                                ui.close_menu();
                            }
                        });
                    });
                });

                // Rubber-band (Marquee) drag multi-selection & Click selection state updates
                if let Some(start_pos) = explorer.drag_select_start {
                    if primary_down
                        && let Some(curr_pos) = pointer_pos
                            && start_pos.distance(curr_pos) > 4.0 {
                                let sel_rect = Rect::from_two_pos(start_pos, curr_pos);
                                let fg = ui.ctx().layer_painter(egui::LayerId::new(
                                    egui::Order::Foreground,
                                    egui::Id::new("sftp_marquee_select"),
                                ));
                                fg.rect(
                                    sel_rect,
                                    Rounding::same(2.0),
                                    Color32::from_rgba_unmultiplied(56, 189, 248, 35),
                                    Stroke::new(1.0_f32, Color32::from_rgb(56, 189, 248)),
                                );

                                let min_y = sel_rect.top();
                                let max_y = sel_rect.bottom();
                                let mut swept_paths = Vec::new();
                                for (r_rect, p_buf) in &row_bounds {
                                    if r_rect.bottom() >= min_y && r_rect.top() <= max_y {
                                        swept_paths.push(p_buf.clone());
                                    }
                                }
                                if !ctrl_held {
                                    explorer.selected_files = swept_paths;
                                    explorer.selected_file = explorer.selected_files.last().cloned();
                                } else {
                                    for sp in swept_paths {
                                        if !explorer.selected_files.contains(&sp) {
                                            explorer.selected_files.push(sp.clone());
                                            explorer.selected_file = Some(sp);
                                        }
                                    }
                                }
                            }
                    if primary_released || !primary_down {
                        explorer.drag_select_start = None;
                    }
                }

                // Ctrl+A & Delete keyboard shortcuts when SFTP panel has no active text input
                if !ui.ctx().wants_keyboard_input() {
                    if ui.ctx().input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::A)) {
                        explorer.selected_files = explorer.entries.iter().map(|e| e.path.clone()).collect();
                        explorer.selected_file = explorer.selected_files.last().cloned();
                    }
                    if ui.ctx().input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Delete)) {
                        let sel = explorer.get_selected_paths();
                        if sel.len() > 1 {
                            explorer.pending_delete = None;
                            explorer.pending_delete_many = sel;
                        } else if let Some(first) = sel.into_iter().next() {
                            explorer.pending_delete_many.clear();
                            explorer.pending_delete = Some(first);
                        }
                    }
                }

                if clear_sel {
                    explorer.clear_selection();
                }
                if let Some(p) = click_single {
                    explorer.select_single(p);
                }
                if let Some(p) = click_toggle {
                    explorer.toggle_select(p);
                }
                if let Some(p) = click_range {
                    explorer.select_range_to(p);
                }

                if go_parent {
                    explorer.go_up();
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
                    explorer.pending_delete_many.clear();
                    explorer.pending_delete = Some(target);
                }
                if !context_del_many.is_empty() {
                    explorer.pending_delete = None;
                    explorer.pending_delete_many = context_del_many;
                }
                if let Some(target) = context_rename {
                    explorer.open_rename(target);
                }
                if let Some(entry) = context_properties {
                    explorer.open_properties(entry);
                }
                if let Some(target) = context_download {
                    let name = target
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "download".to_string());
                    let is_dir = explorer.entries.iter().any(|e| e.path == target && e.is_dir);
                    if is_dir {
                        if let Some(dest_dir) = rfd::FileDialog::new().set_title("Select Destination Folder").pick_folder() {
                            let final_dest = dest_dir.join(&name);
                            explorer.download_file(&target, final_dest);
                        }
                    } else if let Some(dest) = rfd::FileDialog::new().set_file_name(&name).save_file() {
                        explorer.download_file(&target, dest);
                    }
                }
                if !context_download_many.is_empty()
                    && let Some(dest_dir) = rfd::FileDialog::new().set_title("Select Destination Folder").pick_folder() {
                        for path in context_download_many {
                            let name = path
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_else(|| "download".to_string());
                            explorer.download_file(&path, dest_dir.join(name));
                        }
                    }
                if context_new_folder {
                    explorer.show_new_folder = true;
                    explorer.new_folder_name = "new_folder".to_string();
                }
                if context_new_file {
                    explorer.show_new_file = true;
                    explorer.new_file_name = "new_file.txt".to_string();
                }
                if context_refresh {
                    explorer.refresh();
                }
                if context_upload
                    && let Some(files) = rfd::FileDialog::new().set_title("Select Files to Upload").pick_files() {
                        for f in files {
                            explorer.upload_file(f);
                        }
                    }
                if context_upload_folder
                    && let Some(folder) = rfd::FileDialog::new().set_title("Select Folder to Upload").pick_folder() {
                        explorer.upload_folder(folder);
                    }
                if let Some(text) = context_copy_text {
                    crate::terminal::widget::copy_text_to_clipboard(ui, &text);
                    explorer.status_message = Some("Copied to clipboard".into());
                }
                if context_toggle_hidden {
                    explorer.show_hidden = !explorer.show_hidden;
                    explorer.refresh();
                }
                if let Some(dir_str) = context_terminal_here {
                    explorer.requested_cd_terminal = Some(dir_str);
                }
                if let Some(paste_path) = context_terminal_paste_path {
                    explorer.requested_terminal_paste = Some(paste_path);
                    explorer.status_message = Some("Copied path to terminal".into());
                }
                if let Some((target, mode)) = context_open_external {
                    explorer.open_external(&target, mode);
                }
                if let Some(target) = context_compare
                    && let Some(other) = rfd::FileDialog::new().set_title("Select file to compare with").pick_file() {
                        explorer.open_compare(&target, other);
                    }
            });

        // 4. NATIVE OS DRAG-AND-DROP OVERLAY & HANDLER (Files & Folders from Windows Explorer / OS)
        let table_rect = table_frame_resp.response.rect;
        let hovered_files_count = ui.ctx().input(|i| i.raw.hovered_files.len());
        if hovered_files_count > 0 {
            let fg_painter = ui.ctx().layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("sftp_drag_drop_overlay"),
            ));
            fg_painter.rect(
                table_rect,
                Rounding::same(6.0),
                Color32::from_rgba_unmultiplied(15, 23, 42, 220),
                Stroke::new(2.0_f32, Color32::from_rgb(56, 189, 248)),
            );

            let center = table_rect.center();
            let title_text = if explorer.is_remote {
                format!("📥 Drop {} item(s) to Upload", hovered_files_count)
            } else {
                format!("📥 Drop {} item(s) to Copy here", hovered_files_count)
            };
            let dest_str = explorer.current_path.to_string_lossy();
            let sub_text = if dest_str.len() > 36 {
                format!("Target: ...{}", &dest_str[dest_str.len().saturating_sub(33)..])
            } else {
                format!("Target: {}", dest_str)
            };

            fg_painter.text(
                pos2(center.x, center.y - 10.0),
                Align2::CENTER_CENTER,
                title_text,
                FontId::proportional(13.5),
                Color32::from_rgb(56, 189, 248),
            );
            fg_painter.text(
                pos2(center.x, center.y + 12.0),
                Align2::CENTER_CENTER,
                sub_text,
                FontId::monospace(11.0),
                Color32::from_rgb(201, 209, 217),
            );
        }
    });

    // Process OS dropped files if dropped over the SFTP sidebar (or when SFTP panel is active)
    let dropped_paths: Vec<PathBuf> = ui.ctx().input(|i| {
        if i.raw.dropped_files.is_empty() {
            return Vec::new();
        }
        let drop_zone = sidebar_resp.response.rect.expand(16.0);
        let in_sftp_zone = match i.pointer.interact_pos().or_else(|| i.pointer.hover_pos()) {
            Some(pos) => drop_zone.contains(pos),
            None => true, // Windows OLE Drop fallback when pointer coordinates aren't reported on same frame
        };
        if in_sftp_zone {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect()
        } else {
            Vec::new()
        }
    });

    if !dropped_paths.is_empty() {
        explorer.handle_dropped_paths(dropped_paths);
    }

    // Built-in Quick Editor (Modular Kibi-style code/text editor)
    if explorer.editor.open {
        let action = explorer.editor.show(ui.ctx());
        match action {
            crate::editor::EditorAction::Save { path, content, remote_label } => {
                explorer.save_editor_file(path, content, remote_label);
            }
            crate::editor::EditorAction::Close | crate::editor::EditorAction::None => {}
        }
    }

    // Modal Permissions (chmod) - RustyBox TFORMCHMOD
    if explorer.chmod_open {
        let mut open = explorer.chmod_open;
        let mut close_chmod = false;
        let target_name = explorer
            .chmod_target
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Selected item".to_string());

        egui::Window::new(format!("Changing permissions for \"{}\"", target_name))
            .open(&mut open)
            .collapsible(false)
            .default_size(Vec2::new(380.0, 250.0))
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ui.ctx(), |ui| {
                ui.group(|ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new("Permissions").strong());
                    ui.add_space(4.0);
                    ui.columns(3, |cols| {
                        cols[0].vertical(|ui| {
                            ui.label(RichText::new("User (owner)").strong());
                            crate::session::ui::rusty_checkbox(ui, &mut explorer.chmod_u_r, "Read (r)");
                            crate::session::ui::rusty_checkbox(ui, &mut explorer.chmod_u_w, "Write (w)");
                            crate::session::ui::rusty_checkbox(ui, &mut explorer.chmod_u_x, "Execute (x)");
                        });

                        cols[1].vertical(|ui| {
                            ui.label(RichText::new("Group").strong());
                            crate::session::ui::rusty_checkbox(ui, &mut explorer.chmod_g_r, "Read (r)");
                            crate::session::ui::rusty_checkbox(ui, &mut explorer.chmod_g_w, "Write (w)");
                            crate::session::ui::rusty_checkbox(ui, &mut explorer.chmod_g_x, "Execute (x)");
                        });

                        cols[2].vertical(|ui| {
                            ui.label(RichText::new("Others").strong());
                            crate::session::ui::rusty_checkbox(ui, &mut explorer.chmod_o_r, "Read (r)");
                            crate::session::ui::rusty_checkbox(ui, &mut explorer.chmod_o_w, "Write (w)");
                            crate::session::ui::rusty_checkbox(ui, &mut explorer.chmod_o_x, "Execute (x)");
                        });
                    });
                });

                let u = (if explorer.chmod_u_r { 4 } else { 0 })
                    + (if explorer.chmod_u_w { 2 } else { 0 })
                    + (if explorer.chmod_u_x { 1 } else { 0 });
                let g = (if explorer.chmod_g_r { 4 } else { 0 })
                    + (if explorer.chmod_g_w { 2 } else { 0 })
                    + (if explorer.chmod_g_x { 1 } else { 0 });
                let o = (if explorer.chmod_o_r { 4 } else { 0 })
                    + (if explorer.chmod_o_w { 2 } else { 0 })
                    + (if explorer.chmod_o_x { 1 } else { 0 });
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!("Octal mode: 0{:01}{:01}{:01}", u, g, o))
                        .monospace()
                        .strong(),
                );
                ui.add_space(6.0);
                ui.separator();
                ui.add_space(4.0);
                let (ok_clicked, cancel_clicked) =
                    crate::session::ui::rusty_ok_cancel_buttons(ui, "✔ OK", "✖ Cancel");
                if ok_clicked {
                    explorer.apply_chmod();
                    close_chmod = true;
                }
                if cancel_clicked {
                    close_chmod = true;
                }
            });
        explorer.chmod_open = open && !close_chmod;
    }

    // Modal Rename Item
    if explorer.show_rename {
        let mut open = explorer.show_rename;
        let mut close_rename = false;
        let mut do_rename = false;
        egui::Window::new("🏷 Rename Item")
            .open(&mut open)
            .collapsible(false)
            .default_size(Vec2::new(320.0, 130.0))
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ui.ctx(), |ui| {
                ui.label(RichText::new("Enter new name:").strong());
                let resp = ui.add(TextEdit::singleline(&mut explorer.rename_new_name).desired_width(f32::INFINITY));
                if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    do_rename = true;
                }
                ui.add_space(6.0);
                ui.separator();
                ui.add_space(4.0);
                let (ok, cancel) = crate::session::ui::rusty_ok_cancel_buttons(ui, "✔ Rename", "✖ Cancel");
                if ok || do_rename {
                    if let Some(target) = explorer.rename_target.clone() {
                        let name = explorer.rename_new_name.clone();
                        explorer.rename_entry(&target, &name);
                    }
                    close_rename = true;
                }
                if cancel {
                    close_rename = true;
                }
            });
        explorer.show_rename = open && !close_rename;
    }

    // Modal Properties
    if explorer.properties_open {
        let mut open = explorer.properties_open;
        let mut close_prop = false;
        if let Some(entry) = explorer.properties_target.clone() {
            egui::Window::new(format!("ℹ Properties - {}", entry.name))
                .open(&mut open)
                .collapsible(false)
                .default_size(Vec2::new(380.0, 220.0))
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ui.ctx(), |ui| {
                    ui.group(|ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Name:").strong().color(Color32::from_rgb(139, 148, 158)));
                            ui.label(RichText::new(&entry.name).color(Color32::WHITE));
                        });
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Path:").strong().color(Color32::from_rgb(139, 148, 158)));
                            ui.label(RichText::new(entry.path.to_string_lossy().to_string()).monospace());
                        });
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Type:").strong().color(Color32::from_rgb(139, 148, 158)));
                            ui.label(if entry.is_dir { "Directory / Folder" } else { "Regular File" });
                        });
                        if !entry.is_dir {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Size:").strong().color(Color32::from_rgb(139, 148, 158)));
                                ui.label(format!("{} bytes ({})", entry.size, format_file_size(entry.size)));
                            });
                        }
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Permissions:").strong().color(Color32::from_rgb(139, 148, 158)));
                            ui.label(RichText::new(&entry.permissions).monospace());
                        });
                        if let Some(mod_time) = entry.modified
                            && let Ok(dur) = mod_time.duration_since(std::time::UNIX_EPOCH)
                                && let Some(dt) = chrono::DateTime::from_timestamp(dur.as_secs() as i64, 0) {
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new("Modified:").strong().color(Color32::from_rgb(139, 148, 158)));
                                        ui.label(dt.format("%Y-%m-%d %H:%M:%S UTC").to_string());
                                    });
                                }
                    });
                    ui.add_space(8.0);
                    if ui.button("✖ Close").clicked() {
                        close_prop = true;
                    }
                });
        }
        explorer.properties_open = open && !close_prop;
    }

    // Modal Diff / Compare Viewer
    if explorer.diff_open {
        let mut open = explorer.diff_open;
        let mut close_diff = false;
        egui::Window::new(format!("🔍 Compare Files: {} vs {}", explorer.diff_file_a, explorer.diff_file_b))
            .open(&mut open)
            .collapsible(false)
            .default_size(Vec2::new(760.0, 500.0))
            .resizable(true)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ui.ctx(), |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("File A: {}", explorer.diff_file_a)).strong().color(Color32::from_rgb(56, 189, 248)));
                    ui.separator();
                    ui.label(RichText::new(format!("File B: {}", explorer.diff_file_b)).strong().color(Color32::from_rgb(163, 230, 53)));
                });
                ui.separator();

                let avail_h = (ui.available_height() - 40.0).max(200.0);
                let half_w = ((ui.available_width() - 20.0) / 2.0).max(100.0);

                ui.horizontal(|ui| {
                    // Left Column: File A
                    ui.vertical(|ui| {
                        ui.set_width(half_w);
                        ScrollArea::vertical()
                            .id_salt("diff_scroll_a")
                            .max_height(avail_h)
                            .show(ui, |ui| {
                                TextEdit::multiline(&mut explorer.diff_content_a)
                                    .font(egui::TextStyle::Monospace)
                                    .desired_width(half_w)
                                    .interactive(false)
                                    .show(ui);
                            });
                    });
                    ui.separator();
                    // Right Column: File B
                    ui.vertical(|ui| {
                        ui.set_width(half_w);
                        ScrollArea::vertical()
                            .id_salt("diff_scroll_b")
                            .max_height(avail_h)
                            .show(ui, |ui| {
                                TextEdit::multiline(&mut explorer.diff_content_b)
                                    .font(egui::TextStyle::Monospace)
                                    .desired_width(half_w)
                                    .interactive(false)
                                    .show(ui);
                            });
                    });
                });

                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("✖ Close").clicked() {
                        close_diff = true;
                    }
                });
            });
        explorer.diff_open = open && !close_diff;
    }

    // Modal Delete Confirmation Popup (Single or Multi-Select)
    let has_single_del = explorer.pending_delete.is_some();
    let has_many_del = !explorer.pending_delete_many.is_empty();
    if has_single_del || has_many_del {
        let mut open = true;
        let mut confirmed = false;
        let mut cancelled = false;

        egui::Window::new("⚠️ Xác nhận xóa (Confirm Delete)")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_size(Vec2::new(380.0, 175.0))
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ui.ctx(), |ui| {
                ui.add_space(4.0);
                if has_many_del {
                    let count = explorer.pending_delete_many.len();
                    ui.label(
                        RichText::new(format!("Bạn có chắc chắn muốn xóa {} mục đã chọn?", count))
                            .strong()
                            .size(13.0)
                            .color(Color32::from_rgb(248, 113, 113)),
                    );
                    ui.add_space(6.0);
                    egui::Frame::none()
                        .fill(Color32::from_rgb(13, 17, 23))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 54, 61)))
                        .rounding(Rounding::same(4.0))
                        .inner_margin(Margin::symmetric(8.0, 6.0))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ScrollArea::vertical().max_height(110.0).show(ui, |ui| {
                                for p in &explorer.pending_delete_many {
                                    let name = p
                                        .file_name()
                                        .map(|n| n.to_string_lossy().to_string())
                                        .unwrap_or_else(|| p.display().to_string());
                                    ui.label(RichText::new(format!("• {}", name)).monospace().size(11.5));
                                }
                            });
                        });
                } else if let Some(ref del_target) = explorer.pending_delete {
                    let del_name = del_target
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| del_target.display().to_string());
                    ui.label(
                        RichText::new(format!("Bạn có chắc chắn muốn xóa '{}'?", del_name))
                            .strong()
                            .size(13.0)
                            .color(Color32::from_rgb(248, 113, 113)),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(del_target.to_string_lossy())
                            .monospace()
                            .size(11.0)
                            .color(Color32::from_rgb(139, 148, 158)),
                    );
                }

                ui.add_space(6.0);
                ui.label(
                    RichText::new("Hành động này không thể hoàn tác (This action cannot be undone).")
                        .size(11.0)
                        .weak(),
                );
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);

                if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    confirmed = true;
                }
                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    cancelled = true;
                }

                let (ok, cancel) = crate::session::ui::rusty_ok_cancel_buttons(ui, "🗑 Xóa (Delete)", "✖ Hủy (Cancel)");
                if ok {
                    confirmed = true;
                }
                if cancel {
                    cancelled = true;
                }
            });

        if confirmed {
            if has_many_del {
                let targets = std::mem::take(&mut explorer.pending_delete_many);
                explorer.delete_entries(&targets);
            } else if let Some(target) = explorer.pending_delete.take() {
                explorer.delete_entry(&target);
            }
        } else if !open || cancelled {
            explorer.pending_delete = None;
            explorer.pending_delete_many.clear();
        }
    }

    // Modal File Conflict Resolution Popup
    if let Some(conflict) = explorer.pending_conflict.clone() {
        let mut open = true;
        let mut do_overwrite = false;
        let mut do_rename = false;
        let mut do_skip = false;

        egui::Window::new("⚠️ Trùng tên tệp (File Already Exists)")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_size(Vec2::new(410.0, 175.0))
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ui.ctx(), |ui| {
                let action = if conflict.is_upload {
                    "thư mục đích trên Server (Upload destination)"
                } else {
                    "thư mục tải về trên máy (Download location)"
                };
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!("Tệp '{}' đã tồn tại trong {}!", conflict.file_name, action))
                        .strong()
                        .size(12.5)
                        .color(Color32::from_rgb(251, 191, 36)),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new(conflict.dest_path.to_string_lossy())
                        .monospace()
                        .size(11.0)
                        .color(Color32::from_rgb(139, 148, 158)),
                );
                if !explorer.pending_conflicts.is_empty() {
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new(format!("(+{} tệp trùng tên khác đang chờ xử lý)", explorer.pending_conflicts.len()))
                            .size(10.5)
                            .weak(),
                    );
                }
                ui.add_space(10.0);
                ui.separator();
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button(RichText::new("🔄 Ghi đè (Overwrite)").strong().color(Color32::from_rgb(248, 113, 113))).clicked() {
                        do_overwrite = true;
                    }
                    if ui.button(RichText::new("📝 Đổi tên (Keep both)").strong().color(Color32::from_rgb(56, 189, 248))).clicked() {
                        do_rename = true;
                    }
                    if ui.button("⏭ Bỏ qua (Skip)").clicked() {
                        do_skip = true;
                    }
                });
            });

        if do_overwrite {
            explorer.resolve_conflict_overwrite();
        } else if do_rename {
            explorer.resolve_conflict_rename();
        } else if do_skip || !open {
            explorer.resolve_conflict_skip();
        }
    }

    // Modal Create New Folder Popup
    if explorer.show_new_folder {
        let mut open = explorer.show_new_folder;
        let mut close_dlg = false;
        let mut do_create = false;
        egui::Window::new("📁 Tạo thư mục mới (New Directory)")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_size(Vec2::new(340.0, 135.0))
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ui.ctx(), |ui| {
                ui.label(RichText::new("Nhập tên thư mục mới:").strong());
                let resp = ui.add(
                    TextEdit::singleline(&mut explorer.new_folder_name)
                        .desired_width(f32::INFINITY)
                        .hint_text("new_folder"),
                );
                if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    do_create = true;
                }
                ui.add_space(6.0);
                ui.separator();
                ui.add_space(4.0);
                let (ok, cancel) = crate::session::ui::rusty_ok_cancel_buttons(ui, "✔ Tạo (Create)", "✖ Hủy");
                if ok || do_create {
                    let name = explorer.new_folder_name.clone();
                    explorer.create_folder(&name);
                    close_dlg = true;
                }
                if cancel {
                    close_dlg = true;
                }
            });
        explorer.show_new_folder = open && !close_dlg;
    }

    // Modal Create New File Popup
    if explorer.show_new_file {
        let mut open = explorer.show_new_file;
        let mut close_dlg = false;
        let mut do_create = false;
        egui::Window::new("📄 Tạo tệp mới (New Empty File)")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_size(Vec2::new(340.0, 135.0))
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ui.ctx(), |ui| {
                ui.label(RichText::new("Nhập tên tệp mới:").strong());
                let resp = ui.add(
                    TextEdit::singleline(&mut explorer.new_file_name)
                        .desired_width(f32::INFINITY)
                        .hint_text("new_file.txt"),
                );
                if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    do_create = true;
                }
                ui.add_space(6.0);
                ui.separator();
                ui.add_space(4.0);
                let (ok, cancel) = crate::session::ui::rusty_ok_cancel_buttons(ui, "✔ Tạo (Create)", "✖ Hủy");
                if ok || do_create {
                    let name = explorer.new_file_name.clone();
                    explorer.create_file(&name);
                    close_dlg = true;
                }
                if cancel {
                    close_dlg = true;
                }
            });
        explorer.show_new_file = open && !close_dlg;
    }

    if explorer.show_sudo_password_dialog {
        let open = explorer.show_sudo_password_dialog;
        let mut close_dlg = false;
        let mut do_confirm = false;
        egui::Window::new(RichText::new("🛡️ Xác thực Sudo (Root Mode)").strong().color(Color32::from_rgb(251, 146, 60)))
            .collapsible(false)
            .resizable(false)
            .default_size(Vec2::new(360.0, 160.0))
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ui.ctx(), |ui| {
                ui.label(RichText::new("SFTP đang kích hoạt quyền Root (sudo -S).").size(12.0).color(Color32::from_rgb(209, 217, 224)));
                ui.label(RichText::new("Vui lòng nhập mật khẩu sudo trên máy chủ:").strong().size(12.5));
                let resp = ui.add(
                    TextEdit::singleline(&mut explorer.sudo_password_input)
                        .password(true)
                        .desired_width(f32::INFINITY)
                        .hint_text("Mật khẩu sudo..."),
                );
                if !resp.has_focus() {
                    resp.request_focus();
                }
                if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    do_confirm = true;
                }
                ui.add_space(4.0);
                ui.checkbox(&mut explorer.sudo_password_remember, "Ghi nhớ mật khẩu sudo cho phiên làm việc này");
                ui.add_space(6.0);
                ui.separator();
                ui.add_space(4.0);
                let (ok, cancel) = crate::session::ui::rusty_ok_cancel_buttons(ui, "🛡️ Kích hoạt Sudo", "✖ Hủy");
                if ok || do_confirm {
                    explorer.confirm_sudo_password();
                    close_dlg = true;
                }
                if cancel {
                    close_dlg = true;
                }
            });
        explorer.show_sudo_password_dialog = open && !close_dlg;
    }
}

fn draw_sort_arrow(p: &egui::Painter, x: f32, y: f32, ascending: bool) {
    if ascending {
        p.add(egui::Shape::convex_polygon(
            vec![
                pos2(x - 3.2, y + 2.0),
                pos2(x + 3.2, y + 2.0),
                pos2(x, y - 2.5),
            ],
            Color32::from_rgb(139, 148, 158),
            Stroke::NONE,
        ));
    } else {
        p.add(egui::Shape::convex_polygon(
            vec![
                pos2(x - 3.2, y - 2.0),
                pos2(x + 3.2, y - 2.0),
                pos2(x, y + 2.5),
            ],
            Color32::from_rgb(139, 148, 158),
            Stroke::NONE,
        ));
    }
}

fn draw_mini_folder(p: &egui::Painter, center: egui::Pos2, color: Color32) {
    let fx = center.x - 6.5;
    let fy = center.y;
    let tab_rect = Rect::from_min_size(pos2(fx, fy - 5.0), vec2(5.5, 2.2));
    let body_rect = Rect::from_min_size(pos2(fx, fy - 3.0), vec2(13.0, 8.2));
    p.rect_filled(
        tab_rect,
        Rounding {
            nw: 1.4,
            ne: 1.4,
            sw: 0.0,
            se: 0.0,
        },
        Color32::from_rgb(217, 119, 6),
    );
    p.rect_filled(body_rect, Rounding::same(1.6), color);
}

fn draw_mini_file(p: &egui::Painter, center: egui::Pos2, color: Color32) {
    let doc = Rect::from_center_size(center, vec2(9.0, 11.2));
    p.rect(
        doc,
        Rounding::same(1.4),
        Color32::from_rgb(33, 38, 45),
        Stroke::new(1.1_f32, color),
    );
    let ls = Stroke::new(1.0_f32, color);
    p.line_segment(
        [pos2(center.x - 2.4, center.y - 2.0), pos2(center.x + 2.4, center.y - 2.0)],
        ls,
    );
    p.line_segment(
        [pos2(center.x - 2.4, center.y + 1.0), pos2(center.x + 1.8, center.y + 1.0)],
        ls,
    );
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
