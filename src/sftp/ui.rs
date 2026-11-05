use super::FileExplorer;
use egui::{
    pos2, vec2, Align2, Color32, FontId, Margin, Rect, RichText, Rounding,
    ScrollArea, Sense, Stroke, TextEdit, Ui, Vec2,
};
use std::path::PathBuf;

pub fn render_sftp_sidebar(ui: &mut Ui, explorer: &mut FileExplorer) {
    explorer.poll();

    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 5.0;

        // Header indicator: Remote SFTP vs Local
        ui.horizontal(|ui| {
            if explorer.is_remote {
                let label = explorer.remote_label.as_deref().unwrap_or("VPS");
                ui.label(RichText::new("●").color(Color32::from_rgb(63, 185, 80)).size(12.0));
                ui.label(RichText::new(format!("SFTP: {}", label)).strong().size(11.5).color(Color32::from_rgb(88, 166, 255)));
                if explorer.is_loading {
                    ui.spinner();
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("💻 Local").on_hover_text("Switch to browsing local files").clicked() {
                        explorer.disconnect_remote();
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
            "Upload file to current folder",
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

            let is_active_toggle = i == 8 && explorer.show_hidden;
            let bg = if is_active_toggle {
                Color32::from_rgba_unmultiplied(31, 111, 235, 65)
            } else if resp.hovered() {
                Color32::from_rgb(48, 54, 61)
            } else {
                Color32::from_rgb(33, 38, 45)
            };
            let border = if is_active_toggle {
                Stroke::new(1.2, Color32::from_rgb(56, 189, 248))
            } else if resp.hovered() {
                Stroke::new(1.0, Color32::from_rgb(88, 166, 255))
            } else {
                Stroke::new(1.0, Color32::from_rgb(48, 54, 61))
            };

            let p = ui.painter();
            p.rect(r, Rounding::same(4.0), bg, border);
            let c = r.center();

            match i {
                0 => {
                    // Up folder (Golden folder + green up arrow)
                    draw_mini_folder(p, c, Color32::from_rgb(245, 158, 11));
                    let s = Stroke::new(1.6, Color32::from_rgb(52, 211, 153));
                    p.line_segment([pos2(c.x, c.y + 3.0), pos2(c.x, c.y - 2.5)], s);
                    p.line_segment([pos2(c.x - 2.5, c.y), pos2(c.x, c.y - 2.5)], s);
                    p.line_segment([pos2(c.x + 2.5, c.y), pos2(c.x, c.y - 2.5)], s);
                }
                1 => {
                    // Upload (Green upward arrow)
                    let s = Stroke::new(1.8, Color32::from_rgb(63, 185, 80));
                    p.line_segment([pos2(c.x, c.y + 3.5), pos2(c.x, c.y - 4.0)], s);
                    p.line_segment([pos2(c.x - 3.5, c.y - 0.5), pos2(c.x, c.y - 4.0)], s);
                    p.line_segment([pos2(c.x + 3.5, c.y - 0.5), pos2(c.x, c.y - 4.0)], s);
                    p.line_segment(
                        [pos2(c.x - 4.5, c.y + 5.0), pos2(c.x + 4.5, c.y + 5.0)],
                        Stroke::new(1.4, Color32::from_rgb(139, 148, 158)),
                    );
                }
                2 => {
                    // Download (Blue downward arrow)
                    let s = Stroke::new(1.8, Color32::from_rgb(56, 189, 248));
                    p.line_segment([pos2(c.x, c.y - 4.0), pos2(c.x, c.y + 3.5)], s);
                    p.line_segment([pos2(c.x - 3.5, c.y), pos2(c.x, c.y + 3.5)], s);
                    p.line_segment([pos2(c.x + 3.5, c.y), pos2(c.x, c.y + 3.5)], s);
                    p.line_segment(
                        [pos2(c.x - 4.5, c.y + 5.0), pos2(c.x + 4.5, c.y + 5.0)],
                        Stroke::new(1.4, Color32::from_rgb(139, 148, 158)),
                    );
                }
                3 => {
                    // Refresh (Cyan circular arc + arrowheads)
                    let col = Color32::from_rgb(88, 166, 255);
                    p.circle_stroke(c, 4.5, Stroke::new(1.5, col));
                    p.line_segment(
                        [pos2(c.x + 2.5, c.y - 4.5), pos2(c.x + 5.2, c.y - 4.5)],
                        Stroke::new(1.5, col),
                    );
                    p.line_segment(
                        [pos2(c.x + 4.5, c.y - 4.5), pos2(c.x + 4.5, c.y - 1.8)],
                        Stroke::new(1.5, col),
                    );
                }
                4 => {
                    // New Folder (Folder + green plus)
                    draw_mini_folder(p, pos2(c.x - 1.0, c.y), Color32::from_rgb(251, 191, 36));
                    let ps = Stroke::new(1.6, Color32::from_rgb(63, 185, 80));
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
                    let ps = Stroke::new(1.6, Color32::from_rgb(31, 111, 235));
                    p.line_segment([pos2(c.x - 2.8, c.y), pos2(c.x + 1.8, c.y)], ps);
                    p.line_segment([pos2(c.x - 0.5, c.y - 2.3), pos2(c.x - 0.5, c.y + 2.3)], ps);
                }
                6 => {
                    // Delete (Red X)
                    let s = Stroke::new(2.0, Color32::from_rgb(248, 81, 73));
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
                        Stroke::new(1.2, Color32::from_rgb(165, 180, 252)),
                    );
                    let ls = Stroke::new(1.2, Color32::from_rgb(165, 180, 252));
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
                        Stroke::new(1.4, Color32::from_rgb(251, 191, 36)),
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
                    if let Some(path) = rfd::FileDialog::new().pick_file() {
                        explorer.upload_file(path);
                    }
                }
                2 => {
                    if let Some(ref selected) = explorer.selected_file.clone() {
                        if let Some(dest_dir) = rfd::FileDialog::new().pick_folder() {
                            let file_name = selected
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_else(|| "downloaded_file".to_string());
                            explorer.download_file(&selected, dest_dir.join(file_name));
                        }
                    }
                }
                3 => explorer.refresh(),
                4 => explorer.show_new_folder = !explorer.show_new_folder,
                5 => explorer.show_new_file = !explorer.show_new_file,
                6 => {
                    if let Some(ref selected) = explorer.selected_file {
                        explorer.pending_delete = Some(selected.clone());
                    }
                }
                7 => {
                    if explorer.can_edit_selected() {
                        if let Some(selected) = explorer.selected_file.clone() {
                            explorer.open_editor(selected);
                        }
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

        // 2. PATH BAR WITH DROPDOWN HISTORY (Exact pixel-aligned row like `/home/ruby/ [▾]`)
        let path_h = 25.0;
        let (path_row, _) = ui.allocate_exact_size(vec2(ui.available_width(), path_h), Sense::hover());
        let drop_w = 24.0;
        let drop_rect = Rect::from_min_max(
            pos2(path_row.right() - drop_w, path_row.top()),
            path_row.max,
        );
        let input_rect = Rect::from_min_max(
            path_row.min,
            pos2(drop_rect.left() - 4.0, path_row.bottom()),
        );

        let path_resp = ui.put(
            input_rect,
            TextEdit::singleline(&mut explorer.path_input)
                .font(FontId::monospace(11.5))
                .margin(Margin::symmetric(7.0, 4.0))
                .desired_width((input_rect.width() - 16.0).max(60.0))
                .hint_text("/home/..."),
        );
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
                Stroke::new(1.0, Color32::from_rgb(48, 54, 61)),
            );
            let dc = drop_rect.center();
            let cs = Stroke::new(1.6, Color32::from_rgb(201, 209, 217));
            p.line_segment([pos2(dc.x - 3.5, dc.y - 1.5), pos2(dc.x, dc.y + 2.0)], cs);
            p.line_segment([pos2(dc.x, dc.y + 2.0), pos2(dc.x + 3.5, dc.y - 1.5)], cs);
        }
        let drop_resp = drop_resp.on_hover_text("Recent & Quick Paths");
        if drop_resp.clicked() {
            ui.memory_mut(|m| m.toggle_popup(drop_id));
        }

        let mut jump_to: Option<PathBuf> = None;
        egui::popup::popup_below_widget(
            ui,
            drop_id,
            &drop_resp,
            egui::PopupCloseBehavior::CloseOnClickOutside,
            |ui| {
                ui.set_min_width(210.0);
                ui.label(RichText::new("Quick Jump").size(11.0).weak());
                if let Some(home) = dirs::home_dir() {
                    if ui.button(format!("Home ({})", home.display())).clicked() {
                        jump_to = Some(home);
                    }
                }
                if ui.button("/ (Root)").clicked() {
                    jump_to = Some(PathBuf::from("/"));
                }
                if ui.button("/tmp").clicked() {
                    jump_to = Some(PathBuf::from("/tmp"));
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
        if let Some(target) = jump_to {
            explorer.set_path(target);
        }

        // Disk Usage Bar right below the Path Bar (RustyBox style)
        let (used_b, total_b, used_pct) = explorer.disk_usage_stats();
        let (disk_r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 14.0), Sense::hover());
        {
            let p = ui.painter();
            p.rect(
                disk_r,
                Rounding::same(3.0),
                Color32::from_rgb(13, 17, 23),
                Stroke::new(1.0, Color32::from_rgb(40, 46, 56)),
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

        // Inline input for New Folder / New File / Safe Delete Confirmation
        if explorer.show_new_folder {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Folder:").size(11.5));
                ui.add(TextEdit::singleline(&mut explorer.new_folder_name).desired_width(115.0));
                if ui.small_button("Create").clicked() {
                    let name = explorer.new_folder_name.clone();
                    explorer.create_folder(&name);
                    explorer.show_new_folder = false;
                }
                if ui.small_button("✕").clicked() {
                    explorer.show_new_folder = false;
                }
            });
        }

        if explorer.show_new_file {
            ui.horizontal(|ui| {
                ui.label(RichText::new("File:").size(11.5));
                ui.add(TextEdit::singleline(&mut explorer.new_file_name).desired_width(125.0));
                if ui.small_button("Create").clicked() {
                    let name = explorer.new_file_name.clone();
                    explorer.create_file(&name);
                    explorer.show_new_file = false;
                }
                if ui.small_button("✕").clicked() {
                    explorer.show_new_file = false;
                }
            });
        }

        if let Some(del_target) = explorer.pending_delete.clone() {
            let del_name = del_target
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| del_target.display().to_string());
            egui::Frame::none()
                .fill(Color32::from_rgba_unmultiplied(248, 81, 73, 30))
                .stroke(Stroke::new(1.0, Color32::from_rgb(248, 81, 73)))
                .rounding(Rounding::same(5.0))
                .inner_margin(Margin::symmetric(6.0, 4.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("Delete '{}'?", del_name))
                                .size(11.5)
                                .color(Color32::from_rgb(255, 180, 175)),
                        );
                        if ui.small_button("Confirm").clicked() {
                            explorer.delete_entry(&del_target);
                            explorer.pending_delete = None;
                        }
                        if ui.small_button("Cancel").clicked() {
                            explorer.pending_delete = None;
                        }
                    });
                });
        }

        if let Some(ref err) = explorer.error_message {
            ui.colored_label(Color32::from_rgb(248, 81, 73), RichText::new(err).size(11.0));
        }
        if let Some(ref msg) = explorer.status_message {
            ui.colored_label(Color32::from_rgb(63, 185, 80), RichText::new(msg).size(11.0));
        }

        // 3. RECESSED SFTP FILE LIST TABLE (With `Name ▼` Header + `..` Row + Vector Icons)
        let footer_reserve = 68.0;
        let table_h = (ui.available_height() - footer_reserve).max(120.0);

        egui::Frame::none()
            .fill(Color32::from_rgb(13, 17, 23))
            .stroke(Stroke::new(1.0, Color32::from_rgb(40, 46, 56)))
            .rounding(Rounding::same(6.0))
            .inner_margin(Margin::same(1.0))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());

                // Table Column Header (`Name ▼` | `Size`)
                let (hdr_rect, hdr_resp) =
                    ui.allocate_exact_size(vec2(ui.available_width(), 22.0), Sense::click());
                if hdr_resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if hdr_resp.on_hover_text("Click to toggle sort order (A-Z / Z-A)").clicked() {
                    explorer.toggle_sort_order();
                }

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
                    Stroke::new(1.0, Color32::from_rgb(48, 54, 61)),
                );
                p.text(
                    pos2(hdr_rect.left() + 8.0, hdr_rect.center().y),
                    Align2::LEFT_CENTER,
                    "Name",
                    FontId::proportional(11.5),
                    Color32::from_rgb(201, 209, 217),
                );
                // Sort indicator triangle next to `Name`
                let tri_x = hdr_rect.left() + 46.0;
                let tri_y = hdr_rect.center().y;
                if explorer.sort_ascending {
                    p.add(egui::Shape::convex_polygon(
                        vec![
                            pos2(tri_x - 3.2, tri_y - 1.8),
                            pos2(tri_x + 3.2, tri_y - 1.8),
                            pos2(tri_x, tri_y + 2.4),
                        ],
                        Color32::from_rgb(139, 148, 158),
                        Stroke::NONE,
                    ));
                } else {
                    p.add(egui::Shape::convex_polygon(
                        vec![
                            pos2(tri_x, tri_y - 2.4),
                            pos2(tri_x + 3.2, tri_y + 1.8),
                            pos2(tri_x - 3.2, tri_y + 1.8),
                        ],
                        Color32::from_rgb(139, 148, 158),
                        Stroke::NONE,
                    ));
                }
                p.text(
                    pos2(hdr_rect.right() - 8.0, hdr_rect.center().y),
                    Align2::RIGHT_CENTER,
                    "Size",
                    FontId::proportional(11.0),
                    Color32::from_rgb(139, 148, 158),
                );

                let list_h = (table_h - 26.0).max(90.0);
                let mut clicked_dir: Option<PathBuf> = None;
                let mut double_clicked_file: Option<PathBuf> = None;
                let mut context_chmod: Option<PathBuf> = None;
                let mut context_del: Option<PathBuf> = None;
                let mut go_parent = false;

                ScrollArea::vertical()
                    .id_salt("sftp_entries_scroll")
                    .max_height(list_h)
                    .min_scrolled_height(list_h)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 1.0;

                        // Parent directory `..` row (always at top just like RustyBox)
                        let (up_rect, up_resp) =
                            ui.allocate_exact_size(vec2(ui.available_width(), 22.0), Sense::click());
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
                        if up_resp
                            .on_hover_text("Double-click to go to parent directory")
                            .double_clicked()
                        {
                            go_parent = true;
                        }

                        for entry in &explorer.entries {
                            let is_selected = explorer.selected_file.as_ref() == Some(&entry.path);
                            let (row_rect, resp) =
                                ui.allocate_exact_size(vec2(ui.available_width(), 22.0), Sense::click());
                            if resp.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }

                            let p = ui.painter();
                            if is_selected {
                                p.rect(
                                    row_rect,
                                    Rounding::same(3.0),
                                    Color32::from_rgba_unmultiplied(31, 111, 235, 55),
                                    Stroke::new(1.0, Color32::from_rgb(56, 189, 248)),
                                );
                            } else if resp.hovered() {
                                p.rect_filled(
                                    row_rect,
                                    Rounding::same(3.0),
                                    Color32::from_rgb(26, 31, 38),
                                );
                            }

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

                            let size_str = if entry.is_dir {
                                String::new()
                            } else {
                                format_file_size(entry.size)
                            };
                            let right_reserve = if size_str.is_empty() { 8.0 } else { 56.0 };

                            let name_clip = Rect::from_min_max(
                                pos2(row_rect.left() + 26.0, row_rect.top()),
                                pos2((row_rect.right() - right_reserve).max(row_rect.left() + 40.0), row_rect.bottom()),
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

                            ui.painter().with_clip_rect(name_clip).text(
                                pos2(row_rect.left() + 26.0, row_rect.center().y),
                                Align2::LEFT_CENTER,
                                &entry.name,
                                FontId::proportional(12.0),
                                name_color,
                            );

                            if !size_str.is_empty() {
                                ui.painter().text(
                                    pos2(row_rect.right() - 6.0, row_rect.center().y),
                                    Align2::RIGHT_CENTER,
                                    &size_str,
                                    FontId::monospace(10.0),
                                    Color32::from_rgb(110, 118, 129),
                                );
                            }

                            if resp.clicked() {
                                explorer.selected_file = Some(entry.path.clone());
                            }
                            if resp.double_clicked() {
                                if entry.is_dir {
                                    clicked_dir = Some(entry.path.clone());
                                } else {
                                    double_clicked_file = Some(entry.path.clone());
                                }
                            }

                            let tip = format!(
                                "{}\nPermissions: {}\n{}",
                                entry.path.display(),
                                entry.permissions,
                                if entry.is_dir {
                                    "Type: Directory".to_string()
                                } else {
                                    format!("Size: {}", format_file_size(entry.size))
                                }
                            );
                            let path_clone = entry.path.clone();
                            let is_file_entry = !entry.is_dir;
                            resp.on_hover_text(tip).context_menu(|ui| {
                                if is_file_entry && ui.button("Open in Quick Editor").clicked() {
                                    double_clicked_file = Some(path_clone.clone());
                                    ui.close_menu();
                                }
                                if ui.button("Permissions (chmod)...").clicked() {
                                    context_chmod = Some(path_clone.clone());
                                    ui.close_menu();
                                }
                                ui.separator();
                                if ui.button("Delete...").clicked() {
                                    context_del = Some(path_clone.clone());
                                    ui.close_menu();
                                }
                            });
                        }
                    });

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
                    explorer.pending_delete = Some(target);
                }
            });

        // 4. RUSTYBOX SFTP FOOTER CONTROLS (Follow terminal folder + Remote monitoring checkboxes)
        ui.add_space(2.0);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.checkbox(
                &mut explorer.follow_terminal_folder,
                RichText::new("Follow terminal folder")
                    .size(11.5)
                    .color(Color32::from_rgb(201, 209, 217)),
            )
            .on_hover_text("Automatically sync SFTP folder when changing directory (cd) in terminal");

            ui.checkbox(
                &mut explorer.remote_monitoring_enabled,
                RichText::new("Remote monitoring")
                    .size(11.5)
                    .color(Color32::from_rgb(201, 209, 217)),
            )
            .on_hover_text("Display live CPU, RAM, Network, Uptime & HDD bar at bottom of terminal");
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

        let mut close_editor = false;
        egui::Window::new(format!("Quick Editor - {}", file_name))
            .open(&mut open)
            .collapsible(false)
            .default_size(Vec2::new(620.0, 460.0))
            .show(ui.ctx(), |ui| {
                ui.horizontal(|ui| {
                    if ui.button("💾 Save File").clicked() {
                        explorer.save_editor();
                    }
                    if ui.button("✖ Close").clicked() {
                        close_editor = true;
                    }
                    if let Some(ref st) = explorer.editor_status {
                        ui.label(RichText::new(st).color(Color32::from_rgb(63, 185, 80)));
                    }
                });
                ui.separator();
                ScrollArea::both().max_height(370.0).show(ui, |ui| {
                    ui.add(
                        TextEdit::multiline(&mut explorer.editor_content)
                            .font(egui::TextStyle::Monospace)
                            .desired_width(f32::INFINITY)
                            .code_editor(),
                    );
                });
            });
        explorer.editor_open = open && !close_editor;
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
        Stroke::new(1.1, color),
    );
    let ls = Stroke::new(1.0, color);
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
