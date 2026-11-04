use super::pty_session::PtySession;
use super::vt_screen::{vt_to_egui_color, TerminalTheme};
use egui::{
    pos2, vec2, Color32, Event, FontFamily, FontId, Key, Rect, Response, RichText, Sense, Stroke, Ui,
};

pub struct TerminalWidget<'a> {
    session: &'a mut PtySession,
    pub font_size: f32,
    pub search_query: Option<&'a str>,
    pub theme: TerminalTheme,
    pub tab_id: String,
}

pub enum TerminalAction {
    None,
    MultiLinePaste(String),
    SaveBufferToFile,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    OpenSearch,
    RestartTab,
    CloseTab,
}

impl<'a> TerminalWidget<'a> {
    pub fn new(tab_id: &str, session: &'a mut PtySession) -> Self {
        Self {
            session,
            font_size: 14.0,
            search_query: None,
            theme: TerminalTheme::RustyBoxDark,
            tab_id: tab_id.to_string(),
        }
    }

    pub fn with_font_size(mut self, size: f32) -> Self {
        self.font_size = size.max(9.0).min(32.0);
        self
    }

    pub fn with_search(mut self, query: Option<&'a str>) -> Self {
        self.search_query = query;
        self
    }

    pub fn with_theme(mut self, theme: TerminalTheme) -> Self {
        self.theme = theme;
        self
    }

    pub fn ui(
        self,
        ui: &mut Ui,
        broadcast_sender: Option<&crossbeam_channel::Sender<Vec<u8>>>,
    ) -> (Response, TerminalAction) {
        let available_size = ui.available_size();
        let (rect, response) = ui.allocate_exact_size(
            available_size,
            Sense::click_and_drag().union(Sense::focusable_noninteractive()),
        );

        let mut action = TerminalAction::None;

        // Dynamic Monospace font setup
        let font_id = FontId::new(self.font_size, FontFamily::Monospace);
        let char_width = ui.fonts(|f| f.glyph_width(&font_id, 'M')).max(6.0);
        let row_height = ui.fonts(|f| f.row_height(&font_id)).max(12.0);

        let cols = ((rect.width() / char_width).floor() as u16).max(20);
        let rows = ((rect.height() / row_height).floor() as u16).max(5);

        // Resize PTY if dimensions changed
        {
            let (cur_r_nz, cur_c_nz) = {
                let p = self.session.parser.lock();
                p.screen().size()
            };
            if cur_r_nz.get() != rows || cur_c_nz.get() != cols {
                self.session.resize(rows, cols);
            }
        }

        let painter = ui.painter_at(rect);

        // Terminal background with selected theme
        painter.rect_filled(rect, 0.0, self.theme.default_bg());

        let term_id = egui::Id::new("terminal_focus").with(&self.tab_id);
        let sel_id = egui::Id::new("term_selection").with(&self.tab_id);
        let drag_origin_id = egui::Id::new("term_drag_origin").with(&self.tab_id);
        let init_focus_id = egui::Id::new("term_init_focus").with(&self.tab_id);

        let pos_to_cell = |pos: egui::Pos2| -> (usize, usize) {
            let r = (((pos.y - rect.top()) / row_height).floor().max(0.0) as usize)
                .min(rows.saturating_sub(1) as usize);
            let c = (((pos.x - rect.left()) / char_width).floor().max(0.0) as usize)
                .min(cols.saturating_sub(1) as usize);
            (r, c)
        };

        let mut selection: Option<((usize, usize), (usize, usize))> =
            ui.data(|d| d.get_temp(sel_id));
        let mut drag_origin: Option<egui::Pos2> = ui.data(|d| d.get_temp(drag_origin_id));

        if response.drag_started() {
            if let Some(pos) = response.interact_pointer_pos() {
                if rect.contains(pos) {
                    drag_origin = Some(pos);
                    selection = None;
                }
            }
        } else if response.dragged() {
            if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
                if let Some(origin) = drag_origin {
                    // Only begin selection once dragged past a 5px threshold so normal clicks never create accidental selections
                    if pos.distance(origin) >= 5.0 {
                        let start_cell = pos_to_cell(origin);
                        let end_cell = pos_to_cell(pos);
                        selection = Some((start_cell, end_cell));
                    }
                }
            }
        }

        if response.drag_stopped() {
            drag_origin = None;
            if let Some((start, end)) = selection {
                if start != end {
                    let parser = self.session.parser.lock();
                    let text = extract_selection_text(parser.screen(), start, end);
                    if !text.is_empty() {
                        if let Ok(mut cb) = arboard::Clipboard::new() {
                            let _ = cb.set_text(&text);
                        }
                        ui.output_mut(|o| o.copied_text = text);
                    }
                }
            }
        }

        if response.clicked() && !response.dragged() {
            selection = None;
            drag_origin = None;
        }

        ui.data_mut(|d| {
            d.insert_temp(sel_id, selection);
            d.insert_temp(drag_origin_id, drag_origin);
        });

        // Auto-focus newly opened terminal tab on first render
        let already_initialized: bool = ui.data(|d| d.get_temp(init_focus_id)).unwrap_or(false);
        if !already_initialized {
            ui.data_mut(|d| d.insert_temp(init_focus_id, true));
            ui.memory_mut(|m| {
                if let Some(fid) = m.focused() {
                    if fid != response.id && fid != term_id {
                        m.surrender_focus(fid);
                    }
                }
                m.request_focus(response.id);
            });
        }

        // Clicking or pressing inside terminal immediately steals focus from any TextEdit
        let pointer_in_terminal = ui.rect_contains_pointer(rect);
        let pointer_pressed = ui.input(|i| i.pointer.any_pressed() || i.pointer.any_down());
        if response.clicked() || (pointer_in_terminal && pointer_pressed) {
            ui.memory_mut(|m| {
                if let Some(fid) = m.focused() {
                    if fid != response.id && fid != term_id {
                        m.surrender_focus(fid);
                    }
                }
                m.request_focus(response.id);
            });
        }

        // Active terminal tab owns focus unless an external TextEdit (like search or quick connect) is actively focused
        let other_widget_focused = ui.memory(|m| {
            if let Some(fid) = m.focused() {
                fid != response.id && fid != term_id
            } else {
                false
            }
        });

        let has_focus = !other_widget_focused;

        // Lock Tab, Escape, and Arrow keys to the terminal when focused so egui never cycles focus away on Tab autocomplete
        if has_focus {
            ui.memory_mut(|m| {
                if m.focused().is_none() {
                    m.request_focus(response.id);
                }
                if m.focused() == Some(response.id) {
                    m.set_focus_lock_filter(
                        response.id,
                        egui::EventFilter {
                            tab: true,
                            horizontal_arrows: true,
                            vertical_arrows: true,
                            escape: true,
                        },
                    );
                }
            });
        }

        // Mouse Wheel: Ctrl + Wheel = Zoom, Wheel alone = Scrollback (or Up/Down in alternate screen)
        if response.hovered() {
            let (ctrl_held, scroll_y) =
                ui.input(|i| (i.modifiers.ctrl || i.modifiers.command, i.raw_scroll_delta.y));
            if ctrl_held {
                if scroll_y > 1.0 {
                    action = TerminalAction::ZoomIn;
                } else if scroll_y < -1.0 {
                    action = TerminalAction::ZoomOut;
                }
            } else if scroll_y.abs() > 0.5 {
                let lines = ((scroll_y.abs() / row_height).ceil() as usize).clamp(1, 5);
                let mut parser = self.session.parser.lock();
                if parser.screen().alternate_screen() {
                    drop(parser);
                    let seq: &[u8] = if scroll_y > 0.0 { b"\x1b[A" } else { b"\x1b[B" };
                    for _ in 0..lines {
                        let _ = self.session.write_bytes(seq);
                    }
                } else {
                    let cur_sb = parser.screen().scrollback();
                    let new_sb = if scroll_y > 0.0 {
                        cur_sb.saturating_add(lines)
                    } else {
                        cur_sb.saturating_sub(lines)
                    };
                    parser.screen_mut().set_scrollback(new_sb);
                    selection = None;
                    ui.data_mut(|d| d.insert_temp(sel_id, selection));
                }
            }
        }

        // Right-Click Context Menu (MobaXterm TFORMRIGHTCLICKMENU)
        response.context_menu(|ui| {
            ui.set_min_width(210.0);

            let has_selection = selection.map(|(s, e)| s != e).unwrap_or(false);
            if has_selection {
                if ui.button("📄 Copy Selection").clicked() {
                    if let Some((s, e)) = selection {
                        let parser = self.session.parser.lock();
                        let text = extract_selection_text(parser.screen(), s, e);
                        if !text.is_empty() {
                            if let Ok(mut cb) = arboard::Clipboard::new() {
                                let _ = cb.set_text(&text);
                            }
                            ui.output_mut(|o| o.copied_text = text);
                        }
                    }
                    selection = None;
                    ui.data_mut(|d| d.insert_temp(sel_id, selection));
                    ui.close_menu();
                }
            }

            if ui.button("📋 Paste Clipboard").clicked() {
                let text_opt = arboard::Clipboard::new()
                    .ok()
                    .and_then(|mut cb| cb.get_text().ok())
                    .or_else(|| {
                        let t = ui.output(|o| o.copied_text.clone());
                        if t.is_empty() {
                            None
                        } else {
                            Some(t)
                        }
                    });

                if let Some(text) = text_opt {
                    if !text.is_empty() {
                        if text.contains('\n') {
                            action = TerminalAction::MultiLinePaste(text);
                        } else {
                            let bracketed = {
                                let mut p = self.session.parser.lock();
                                p.screen_mut().set_scrollback(0);
                                p.screen().bracketed_paste()
                            };
                            let bytes = format_paste_bytes(&text, bracketed);
                            let _ = self.session.write_bytes(&bytes);
                            if let Some(sender) = broadcast_sender {
                                let _ = sender.send(bytes);
                            }
                        }
                    }
                }
                selection = None;
                ui.data_mut(|d| d.insert_temp(sel_id, selection));
                ui.close_menu();
            }

            if ui.button("📄 Copy All Terminal Text").clicked() {
                let mut all_text = String::new();
                let parser = self.session.parser.lock();
                let screen = parser.screen();
                let (screen_rows, screen_cols) = screen.size();
                for r in 0..screen_rows.get() {
                    let mut row_str = String::new();
                    for c in 0..screen_cols.get() {
                        if let Some(cell) = screen.cell(r, c) {
                            row_str.push_str(cell.contents());
                        } else {
                            row_str.push(' ');
                        }
                    }
                    all_text.push_str(row_str.trim_end());
                    all_text.push('\n');
                }
                if let Ok(mut cb) = arboard::Clipboard::new() {
                    let _ = cb.set_text(&all_text);
                }
                ui.output_mut(|o| o.copied_text = all_text);
                ui.close_menu();
            }
            ui.separator();
            if ui
                .button("🛑 Send Ctrl+C (Interrupt Process)")
                .clicked()
            {
                self.session.parser.lock().screen_mut().set_scrollback(0);
                let _ = self.session.send_sigint();
                if let Some(sender) = broadcast_sender {
                    let _ = sender.send(b"\x03".to_vec());
                }
                selection = None;
                ui.data_mut(|d| d.insert_temp(sel_id, selection));
                ui.close_menu();
            }
            if ui.button("🔍 Find in Terminal (Ctrl+F)").clicked() {
                action = TerminalAction::OpenSearch;
                ui.close_menu();
            }
            if ui.button("🔎 Zoom In (Ctrl +)").clicked() {
                action = TerminalAction::ZoomIn;
                ui.close_menu();
            }
            if ui.button("🔍 Zoom Out (Ctrl -)").clicked() {
                action = TerminalAction::ZoomOut;
                ui.close_menu();
            }
            ui.separator();
            if ui.button("🧹 Clear Terminal Screen").clicked() {
                self.session.parser.lock().screen_mut().set_scrollback(0);
                #[cfg(target_os = "windows")]
                let _ = self.session.write_bytes(b"cls\r\n");
                #[cfg(not(target_os = "windows"))]
                let _ = self.session.write_bytes(b"clear\n");
                ui.close_menu();
            }
            if ui.button("💾 Save Buffer to File...").clicked() {
                action = TerminalAction::SaveBufferToFile;
                ui.close_menu();
            }
            ui.separator();
            ui.label(
                RichText::new(format!("Size: {} cols x {} rows", cols, rows))
                    .weak()
                    .size(11.0),
            );
        });

        // Render terminal contents with Syntax Highlighting & Search Matching
        {
            let parser = self.session.parser.lock();
            let screen = parser.screen();
            let (screen_rows_nz, screen_cols_nz) = screen.size();
            let screen_rows = screen_rows_nz.get();
            let screen_cols = screen_cols_nz.get();
            let (cursor_row, cursor_col) = screen.cursor_position();
            let cursor_hidden = screen.hide_cursor();
            let scrollback_offset = screen.scrollback();
            let visible_cols = screen_cols.min(cols) as usize;

            let search_term = self
                .search_query
                .map(|q| q.trim().to_lowercase())
                .filter(|q| !q.is_empty());

            for r in 0..screen_rows.min(rows) {
                // Collect row string once per row to compute syntax & search highlights
                let mut row_str = String::with_capacity(visible_cols);
                for c in 0..visible_cols {
                    if let Some(cell) = screen.cell(r, c as u16) {
                        let s = cell.contents();
                        if s.is_empty() {
                            row_str.push(' ');
                        } else {
                            row_str.push_str(s);
                        }
                    } else {
                        row_str.push(' ');
                    }
                }

                // Precompute syntax highlight colors once per row (UTF-8 char-index accurate)
                let row_syntax_colors = compute_row_syntax_highlights(&row_str, visible_cols);

                // Check search highlight occurrences
                let mut search_matches: Vec<std::ops::Range<usize>> = Vec::new();
                if let Some(ref query) = search_term {
                    let lower_row = row_str.to_lowercase();
                    let q_len = query.chars().count();
                    let mut start = 0;
                    while let Some(pos) = lower_row[start..].find(query) {
                        let actual_idx = start + pos;
                        let char_idx = lower_row[..actual_idx].chars().count();
                        search_matches.push(char_idx..(char_idx + q_len));
                        start = actual_idx + query.len();
                    }
                }

                for c in 0..screen_cols.min(cols) {
                    if let Some(cell) = screen.cell(r, c) {
                        let cell_x = rect.min.x + (c as f32) * char_width;
                        let cell_y = rect.min.y + (r as f32) * row_height;
                        let cell_rect =
                            Rect::from_min_size(pos2(cell_x, cell_y), vec2(char_width, row_height));

                        // Check if current cell is selected or inside search match
                        let is_selected_cell = is_cell_selected(r as usize, c as usize, selection);
                        let is_searched =
                            search_matches.iter().any(|range| range.contains(&(c as usize)));

                        let cell_fg_raw = vt_to_egui_color(cell.fgcolor(), true, self.theme);
                        let cell_bg_raw = vt_to_egui_color(cell.bgcolor(), false, self.theme);
                        let (base_fg, base_bg) = if cell.inverse() {
                            (cell_bg_raw, cell_fg_raw)
                        } else {
                            (cell_fg_raw, cell_bg_raw)
                        };

                        // Custom background
                        let bg_color = if is_selected_cell {
                            Color32::from_rgb(30, 95, 175) // Vibrant selection blue
                        } else if is_searched {
                            Color32::from_rgb(180, 140, 20) // Yellow highlight for search
                        } else {
                            base_bg
                        };

                        if bg_color != self.theme.default_bg() || is_selected_cell || cell.inverse()
                        {
                            painter.rect_filled(cell_rect, 0.0, bg_color);
                        }

                        if cell.underline() {
                            let y = cell_rect.bottom() - 1.5;
                            painter.line_segment(
                                [pos2(cell_rect.left(), y), pos2(cell_rect.right(), y)],
                                Stroke::new(1.0_f32, base_fg),
                            );
                        }

                        // Text & Syntax Highlighting
                        let contents = cell.contents();
                        if !contents.is_empty() && contents != " " {
                            let mut fg_color = if is_selected_cell {
                                Color32::WHITE
                            } else {
                                base_fg
                            };

                            // Apply MobaXterm-style smart syntax coloring if default color
                            if fg_color == self.theme.default_fg()
                                && !is_selected_cell
                                && !cell.inverse()
                            {
                                if let Some(Some(hl_col)) = row_syntax_colors.get(c as usize) {
                                    fg_color = *hl_col;
                                }
                            }

                            painter.text(
                                pos2(cell_x, cell_y),
                                egui::Align2::LEFT_TOP,
                                contents,
                                font_id.clone(),
                                fg_color,
                            );
                        }
                    }
                }
            }

            // Cursor drawing: solid blinking when focused, hollow outline when unfocused (hidden while viewing scrollback)
            if !cursor_hidden && scrollback_offset == 0 {
                let cur_x = rect.min.x + (cursor_col as f32) * char_width;
                let cur_y = rect.min.y + (cursor_row as f32) * row_height;
                let cur_rect =
                    Rect::from_min_size(pos2(cur_x, cur_y), vec2(char_width, row_height));
                if has_focus {
                    let blink = (ui.input(|i| i.time) * 2.0).fract() < 0.5;
                    if blink {
                        painter.rect_filled(
                            cur_rect,
                            1.0,
                            Color32::from_rgba_premultiplied(230, 230, 240, 160),
                        );
                    }
                } else {
                    painter.rect_stroke(
                        cur_rect,
                        1.0,
                        Stroke::new(
                            1.0_f32,
                            Color32::from_rgba_premultiplied(180, 180, 200, 120),
                        ),
                    );
                }
            }

            // Floating Scrollback Indicator Pill when viewing history
            if scrollback_offset > 0 {
                let badge_text = format!(
                    "📜 History: -{} lines (Scroll down or press any key)",
                    scrollback_offset
                );
                let badge_w = 300.0_f32.min(rect.width() - 16.0);
                let badge_rect = Rect::from_min_size(
                    pos2(rect.right() - badge_w - 8.0, rect.top() + 6.0),
                    vec2(badge_w, 22.0),
                );
                painter.rect(
                    badge_rect,
                    5.0,
                    Color32::from_rgba_unmultiplied(30, 41, 59, 230),
                    Stroke::new(1.0_f32, Color32::from_rgb(56, 189, 248)),
                );
                painter.text(
                    badge_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    badge_text,
                    FontId::proportional(11.0),
                    Color32::from_rgb(186, 230, 253),
                );
            }
        }

        // Process keyboard inputs when terminal has focus
        if has_focus {
            let mut input_bytes: Vec<u8> = Vec::new();
            let mut paste_text_to_check: Option<String> = None;
            let mut copy_or_sigint_handled = false;
            let mut paste_handled = false;
            let mut scroll_delta_lines: isize = 0;

            let (events, current_modifiers) = ui.input(|i| (i.events.clone(), i.modifiers));
            for event in &events {
                match event {
                    Event::Text(text) => {
                        if !current_modifiers.ctrl && !current_modifiers.command {
                            if current_modifiers.alt {
                                for ch in text.chars() {
                                    input_bytes.push(0x1b);
                                    let mut buf = [0u8; 4];
                                    input_bytes
                                        .extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                                }
                            } else {
                                input_bytes.extend_from_slice(text.as_bytes());
                            }
                        }
                    }
                        Event::Key {
                            key,
                            pressed: true,
                            modifiers,
                            ..
                        } => {
                            let ctrl = modifiers.ctrl || modifiers.command;
                            let alt = modifiers.alt;
                            let shift = modifiers.shift;

                            match key {
                                Key::Enter => {
                                    if !self.session.is_alive() {
                                        action = TerminalAction::RestartTab;
                                    } else if alt {
                                        input_bytes.extend_from_slice(b"\x1b\r");
                                    } else {
                                        input_bytes.extend_from_slice(b"\r");
                                    }
                                }
                                Key::Backspace => {
                                    if alt {
                                        input_bytes.extend_from_slice(b"\x1b\x7f");
                                    } else if ctrl {
                                        input_bytes.extend_from_slice(b"\x17"); // Ctrl+Backspace: delete previous word
                                    } else {
                                        #[cfg(target_os = "windows")]
                                        {
                                            input_bytes.extend_from_slice(b"\x08");
                                        }
                                        #[cfg(not(target_os = "windows"))]
                                        {
                                            input_bytes.extend_from_slice(b"\x7f");
                                        }
                                    }
                                }
                                Key::Tab => {
                                    if shift {
                                        input_bytes.extend_from_slice(b"\x1b[Z"); // Shift+Tab (Backtab)
                                    } else {
                                        input_bytes.extend_from_slice(b"\t");
                                    }
                                }
                                Key::Escape => input_bytes.extend_from_slice(b"\x1b"),
                                Key::ArrowUp => {
                                    if shift && !ctrl && !alt {
                                        scroll_delta_lines += 3;
                                    } else if ctrl {
                                        input_bytes.extend_from_slice(b"\x1b[1;5A");
                                    } else if alt {
                                        input_bytes.extend_from_slice(b"\x1b[1;3A");
                                    } else {
                                        input_bytes.extend_from_slice(b"\x1b[A");
                                    }
                                }
                                Key::ArrowDown => {
                                    if shift && !ctrl && !alt {
                                        scroll_delta_lines -= 3;
                                    } else if ctrl {
                                        input_bytes.extend_from_slice(b"\x1b[1;5B");
                                    } else if alt {
                                        input_bytes.extend_from_slice(b"\x1b[1;3B");
                                    } else {
                                        input_bytes.extend_from_slice(b"\x1b[B");
                                    }
                                }
                                Key::ArrowRight => {
                                    if ctrl {
                                        input_bytes.extend_from_slice(b"\x1b[1;5C"); // Jump word right
                                    } else if alt {
                                        input_bytes.extend_from_slice(b"\x1b[1;3C");
                                    } else {
                                        input_bytes.extend_from_slice(b"\x1b[C");
                                    }
                                }
                                Key::ArrowLeft => {
                                    if ctrl {
                                        input_bytes.extend_from_slice(b"\x1b[1;5D"); // Jump word left
                                    } else if alt {
                                        input_bytes.extend_from_slice(b"\x1b[1;3D");
                                    } else {
                                        input_bytes.extend_from_slice(b"\x1b[D");
                                    }
                                }
                                Key::Home => input_bytes.extend_from_slice(b"\x1b[H"),
                                Key::End => input_bytes.extend_from_slice(b"\x1b[F"),
                                Key::PageUp => {
                                    if shift {
                                        scroll_delta_lines += (rows as isize / 2).max(5);
                                    } else {
                                        input_bytes.extend_from_slice(b"\x1b[5~");
                                    }
                                }
                                Key::PageDown => {
                                    if shift {
                                        scroll_delta_lines -= (rows as isize / 2).max(5);
                                    } else {
                                        input_bytes.extend_from_slice(b"\x1b[6~");
                                    }
                                }
                                Key::Delete => {
                                    if ctrl {
                                        input_bytes.extend_from_slice(b"\x1b[3;5~");
                                    } else {
                                        input_bytes.extend_from_slice(b"\x1b[3~");
                                    }
                                }
                                // Function Keys F1 - F12 (essential for htop, mc, byobu, nmtui)
                                Key::F1 => input_bytes.extend_from_slice(b"\x1bOP"),
                                Key::F2 => input_bytes.extend_from_slice(b"\x1bOQ"),
                                Key::F3 => input_bytes.extend_from_slice(b"\x1bOR"),
                                Key::F4 => input_bytes.extend_from_slice(b"\x1bOS"),
                                Key::F5 => input_bytes.extend_from_slice(b"\x1b[15~"),
                                Key::F6 => input_bytes.extend_from_slice(b"\x1b[17~"),
                                Key::F7 => input_bytes.extend_from_slice(b"\x1b[18~"),
                                Key::F8 => input_bytes.extend_from_slice(b"\x1b[19~"),
                                Key::F9 => input_bytes.extend_from_slice(b"\x1b[20~"),
                                Key::F10 => input_bytes.extend_from_slice(b"\x1b[21~"),
                                Key::F11 => input_bytes.extend_from_slice(b"\x1b[23~"),
                                Key::F12 => input_bytes.extend_from_slice(b"\x1b[24~"),

                                Key::Insert if shift => {
                                    if !paste_handled {
                                        paste_handled = true;
                                        let text_opt = arboard::Clipboard::new()
                                            .ok()
                                            .and_then(|mut cb| cb.get_text().ok());
                                        if let Some(text) = text_opt {
                                            if text.contains('\n') {
                                                paste_text_to_check = Some(text);
                                            } else {
                                                let bracketed = self
                                                    .session
                                                    .parser
                                                    .lock()
                                                    .screen()
                                                    .bracketed_paste();
                                                input_bytes.extend_from_slice(&format_paste_bytes(
                                                    &text, bracketed,
                                                ));
                                            }
                                        }
                                    }
                                }
                                Key::Insert if ctrl => {
                                    // Ctrl+Insert: Copy
                                    let parser = self.session.parser.lock();
                                    let text = if let Some((s, e)) = selection {
                                        extract_selection_text(parser.screen(), s, e)
                                    } else {
                                        String::new()
                                    };
                                    if !text.is_empty() {
                                        if let Ok(mut cb) = arboard::Clipboard::new() {
                                            let _ = cb.set_text(&text);
                                        }
                                        ui.output_mut(|o| o.copied_text = text);
                                    }
                                    selection = None;
                                    ui.data_mut(|d| d.insert_temp(sel_id, selection));
                                }
                                Key::Insert => input_bytes.extend_from_slice(b"\x1b[2~"),
                                Key::F if ctrl && !shift => action = TerminalAction::OpenSearch,
                                Key::Plus if ctrl => action = TerminalAction::ZoomIn,
                                Key::Equals if ctrl => action = TerminalAction::ZoomIn,
                                Key::Minus if ctrl => action = TerminalAction::ZoomOut,
                                Key::Num0 if ctrl => action = TerminalAction::ZoomReset,
                                Key::C if ctrl && shift => {
                                    // Ctrl+Shift+C: Copy Selection
                                    copy_or_sigint_handled = true;
                                    let parser = self.session.parser.lock();
                                    let text = if let Some((s, e)) = selection {
                                        extract_selection_text(parser.screen(), s, e)
                                    } else {
                                        String::new()
                                    };
                                    if !text.is_empty() {
                                        if let Ok(mut cb) = arboard::Clipboard::new() {
                                            let _ = cb.set_text(&text);
                                        }
                                        ui.output_mut(|o| o.copied_text = text);
                                    }
                                    selection = None;
                                    ui.data_mut(|d| d.insert_temp(sel_id, selection));
                                }
                                Key::V if ctrl && shift => {
                                    // Ctrl+Shift+V: Paste from Clipboard
                                    if !paste_handled {
                                        paste_handled = true;
                                        let text_opt = arboard::Clipboard::new()
                                            .ok()
                                            .and_then(|mut cb| cb.get_text().ok());
                                        if let Some(text) = text_opt {
                                            if text.contains('\n') {
                                                paste_text_to_check = Some(text);
                                            } else {
                                                let bracketed = self
                                                    .session
                                                    .parser
                                                    .lock()
                                                    .screen()
                                                    .bracketed_paste();
                                                input_bytes.extend_from_slice(&format_paste_bytes(
                                                    &text, bracketed,
                                                ));
                                            }
                                        }
                                    }
                                }
                                Key::C if ctrl => {
                                    if !copy_or_sigint_handled {
                                        copy_or_sigint_handled = true;
                                        let has_active_selection =
                                            selection.map(|(s, e)| s != e).unwrap_or(false);
                                        if has_active_selection {
                                            let parser = self.session.parser.lock();
                                            let text = if let Some((s, e)) = selection {
                                                extract_selection_text(parser.screen(), s, e)
                                            } else {
                                                String::new()
                                            };
                                            if !text.is_empty() {
                                                if let Ok(mut cb) = arboard::Clipboard::new() {
                                                    let _ = cb.set_text(&text);
                                                }
                                                ui.output_mut(|o| o.copied_text = text);
                                            }
                                            // Always clear selection after copying so subsequent Ctrl+C immediately sends SIGINT
                                            selection = None;
                                            ui.data_mut(|d| d.insert_temp(sel_id, selection));
                                        } else {
                                            // Standard Ctrl+C: Send SIGINT (\x03) to foreground process (docker logs, ping, tail -f, etc.)
                                            self.session.parser.lock().screen_mut().set_scrollback(0);
                                            let _ = self.session.send_sigint();
                                            ui.ctx().request_repaint();
                                            if let Some(sender) = broadcast_sender {
                                                let _ = sender.send(b"\x03".to_vec());
                                            }
                                        }
                                    }
                                }
                                Key::W if ctrl && (shift || !self.session.is_alive()) => {
                                    action = TerminalAction::CloseTab;
                                }
                                _ => {
                                    if let Some(seq) =
                                        encode_terminal_key_sequence(*key, ctrl, alt, shift)
                                    {
                                        input_bytes.extend_from_slice(seq);
                                    }
                                }
                            }
                        }
                        Event::Copy => {
                            if !copy_or_sigint_handled {
                                copy_or_sigint_handled = true;
                                let has_active_selection =
                                    selection.map(|(s, e)| s != e).unwrap_or(false);
                                if has_active_selection {
                                    let parser = self.session.parser.lock();
                                    let text = if let Some((s, e)) = selection {
                                        extract_selection_text(parser.screen(), s, e)
                                    } else {
                                        String::new()
                                    };
                                    if !text.is_empty() {
                                        if let Ok(mut cb) = arboard::Clipboard::new() {
                                            let _ = cb.set_text(&text);
                                        }
                                        ui.output_mut(|o| o.copied_text = text);
                                    }
                                    // Clear selection after copying so subsequent Ctrl+C sends SIGINT
                                    selection = None;
                                    ui.data_mut(|d| d.insert_temp(sel_id, selection));
                                } else {
                                    // No selection: Ctrl+C sends SIGINT (\x03) to interrupt running command (docker logs, ping, etc.)
                                    self.session.parser.lock().screen_mut().set_scrollback(0);
                                    let _ = self.session.send_sigint();
                                    ui.ctx().request_repaint();
                                    if let Some(sender) = broadcast_sender {
                                        let _ = sender.send(b"\x03".to_vec());
                                    }
                                }
                            }
                        }
                        Event::Cut => {
                            let has_active_selection =
                                selection.map(|(s, e)| s != e).unwrap_or(false);
                            if has_active_selection {
                                let parser = self.session.parser.lock();
                                let text = if let Some((s, e)) = selection {
                                    extract_selection_text(parser.screen(), s, e)
                                } else {
                                    String::new()
                                };
                                if !text.is_empty() {
                                    if let Ok(mut cb) = arboard::Clipboard::new() {
                                        let _ = cb.set_text(&text);
                                    }
                                    ui.output_mut(|o| o.copied_text = text);
                                }
                                selection = None;
                                ui.data_mut(|d| d.insert_temp(sel_id, selection));
                            }
                        }
                        Event::Paste(text) => {
                            if !paste_handled {
                                paste_handled = true;
                                if text.contains('\n') {
                                    paste_text_to_check = Some(text.clone());
                                } else {
                                    let bracketed =
                                        self.session.parser.lock().screen().bracketed_paste();
                                    input_bytes
                                        .extend_from_slice(&format_paste_bytes(text, bracketed));
                                }
                            }
                        }
                        _ => {}
                    }
                }

            if scroll_delta_lines != 0 {
                let mut parser = self.session.parser.lock();
                let cur_sb = parser.screen().scrollback();
                let new_sb = if scroll_delta_lines > 0 {
                    cur_sb.saturating_add(scroll_delta_lines as usize)
                } else {
                    cur_sb.saturating_sub((-scroll_delta_lines) as usize)
                };
                parser.screen_mut().set_scrollback(new_sb);
                selection = None;
                ui.data_mut(|d| d.insert_temp(sel_id, selection));
            }

            if let Some(text) = paste_text_to_check {
                selection = None;
                ui.data_mut(|d| d.insert_temp(sel_id, selection));
                action = TerminalAction::MultiLinePaste(text);
            }

            if !input_bytes.is_empty() {
                // Any keyboard input clears stale selection and snaps scrollback to live bottom
                if selection.is_some() {
                    selection = None;
                    ui.data_mut(|d| d.insert_temp(sel_id, selection));
                }
                self.session.parser.lock().screen_mut().set_scrollback(0);
                let _ = self.session.write_bytes(&input_bytes);
                ui.ctx().request_repaint();
                if let Some(sender) = broadcast_sender {
                    let _ = sender.send(input_bytes);
                }
            }
        }

        (response, action)
    }
}

fn encode_terminal_key_sequence(
    key: Key,
    ctrl: bool,
    alt: bool,
    shift: bool,
) -> Option<&'static [u8]> {
    if ctrl && !alt && !shift {
        return match key {
            Key::A => Some(b"\x01"),            // Ctrl+A (Start of line)
            Key::B => Some(b"\x02"),            // Ctrl+B (tmux prefix / back)
            Key::C => Some(b"\x03"),            // Ctrl+C (SIGINT)
            Key::D => Some(b"\x04"),            // Ctrl+D (EOF / exit)
            Key::E => Some(b"\x05"),            // Ctrl+E (End of line)
            Key::G => Some(b"\x07"),            // Ctrl+G (Cancel search / bell)
            Key::H => Some(b"\x08"),            // Ctrl+H (Backspace)
            Key::J => Some(b"\n"),              // Ctrl+J (Newline)
            Key::K => Some(b"\x0b"),            // Ctrl+K (Kill to end of line)
            Key::L => Some(b"\x0c"),            // Ctrl+L (Clear screen)
            Key::N => Some(b"\x0e"),            // Ctrl+N (Next history)
            Key::O => Some(b"\x0f"),            // Ctrl+O
            Key::P => Some(b"\x10"),            // Ctrl+P (Prev history)
            Key::Q => Some(b"\x11"),            // Ctrl+Q (XON)
            Key::R => Some(b"\x12"),            // Ctrl+R (Reverse search)
            Key::S => Some(b"\x13"),            // Ctrl+S (XOFF / search)
            Key::U => Some(b"\x15"),            // Ctrl+U (Kill line before cursor)
            Key::W => Some(b"\x17"),            // Ctrl+W (Delete word before cursor)
            Key::X => Some(b"\x18"),            // Ctrl+X (nano exit / emacs)
            Key::Y => Some(b"\x19"),            // Ctrl+Y (Yank)
            Key::Z => Some(b"\x1a"),            // Ctrl+Z (SIGTSTP suspend)
            Key::Backslash => Some(b"\x1c"),    // Ctrl+\ (SIGQUIT force quit)
            Key::CloseBracket => Some(b"\x1d"), // Ctrl+] (Telnet escape)
            Key::Space => Some(b"\x00"),        // Ctrl+Space (NUL / mark)
            Key::Backspace => Some(b"\x17"),    // Ctrl+Backspace (Delete previous word)
            Key::Delete => Some(b"\x1b[3;5~"),  // Ctrl+Delete
            Key::ArrowUp => Some(b"\x1b[1;5A"),
            Key::ArrowDown => Some(b"\x1b[1;5B"),
            Key::ArrowRight => Some(b"\x1b[1;5C"),
            Key::ArrowLeft => Some(b"\x1b[1;5D"),
            _ => None,
        };
    }

    match key {
        Key::F1 => Some(b"\x1bOP"),
        Key::F2 => Some(b"\x1bOQ"),
        Key::F3 => Some(b"\x1bOR"),
        Key::F4 => Some(b"\x1bOS"),
        Key::F5 => Some(b"\x1b[15~"),
        Key::F6 => Some(b"\x1b[17~"),
        Key::F7 => Some(b"\x1b[18~"),
        Key::F8 => Some(b"\x1b[19~"),
        Key::F9 => Some(b"\x1b[20~"),
        Key::F10 => Some(b"\x1b[21~"),
        Key::F11 => Some(b"\x1b[23~"),
        Key::F12 => Some(b"\x1b[24~"),
        _ => None,
    }
}

fn format_paste_bytes(text: &str, bracketed: bool) -> Vec<u8> {
    if bracketed {
        let mut out = Vec::with_capacity(text.len() + 12);
        out.extend_from_slice(b"\x1b[200~");
        out.extend_from_slice(text.as_bytes());
        out.extend_from_slice(b"\x1b[201~");
        out
    } else {
        text.as_bytes().to_vec()
    }
}

/// Precomputes MobaXterm-style keyword & IPv4 highlight colors once per row using UTF-8 character indices
fn compute_row_syntax_highlights(row: &str, max_cols: usize) -> Vec<Option<Color32>> {
    let chars: Vec<char> = row.chars().collect();
    let len = chars.len().min(max_cols);
    let mut colors = vec![None; max_cols];
    if len == 0 {
        return colors;
    }

    let lower_chars: Vec<char> = chars.iter().map(|c| c.to_ascii_lowercase()).collect();

    let apply_keywords =
        |colors: &mut [Option<Color32>], keywords: &[&str], color: Color32| {
            for kw in keywords {
                let kw_chars: Vec<char> = kw.chars().collect();
                let kw_len = kw_chars.len();
                if kw_len == 0 || kw_len > len {
                    continue;
                }
                let mut i = 0;
                while i + kw_len <= len {
                    if lower_chars[i..i + kw_len] == kw_chars[..] {
                        let before_ok = i == 0 || !lower_chars[i - 1].is_ascii_alphanumeric();
                        let after_ok = i + kw_len >= lower_chars.len()
                            || !lower_chars[i + kw_len].is_ascii_alphanumeric();
                        if before_ok && after_ok {
                            for slot in &mut colors[i..i + kw_len] {
                                if slot.is_none() {
                                    *slot = Some(color);
                                }
                            }
                            i += kw_len;
                            continue;
                        }
                    }
                    i += 1;
                }
            }
        };

    apply_keywords(
        &mut colors,
        &["error", "failed", "fatal", "exception", "err"],
        Color32::from_rgb(255, 90, 90),
    );
    apply_keywords(
        &mut colors,
        &["success", "ok", "passed", "running", "active"],
        Color32::from_rgb(80, 230, 120),
    );
    apply_keywords(
        &mut colors,
        &["warn", "warning"],
        Color32::from_rgb(255, 210, 60),
    );

    // IPv4 tokens (e.g. 192.168.1.1)
    let mut idx = 0;
    while idx < len {
        if chars[idx].is_ascii_digit() {
            let start = idx;
            while idx < len && (chars[idx].is_ascii_digit() || chars[idx] == '.') {
                idx += 1;
            }
            let token: String = chars[start..idx].iter().collect();
            let parts: Vec<&str> = token.split('.').collect();
            if parts.len() == 4 && parts.iter().all(|p| !p.is_empty() && p.parse::<u8>().is_ok()) {
                for slot in &mut colors[start..idx] {
                    if slot.is_none() {
                        *slot = Some(Color32::from_rgb(80, 210, 245));
                    }
                }
            }
        } else {
            idx += 1;
        }
    }

    colors
}

fn is_cell_selected(r: usize, c: usize, sel: Option<((usize, usize), (usize, usize))>) -> bool {
    let Some(((r1, c1), (r2, c2))) = sel else {
        return false;
    };
    if (r1, c1) == (r2, c2) {
        return false;
    };
    let (start, end) = if (r1, c1) <= (r2, c2) {
        ((r1, c1), (r2, c2))
    } else {
        ((r2, c2), (r1, c1))
    };
    if r < start.0 || r > end.0 {
        return false;
    }
    if start.0 == end.0 {
        return c >= start.1 && c <= end.1;
    }
    if r == start.0 {
        c >= start.1
    } else if r == end.0 {
        c <= end.1
    } else {
        true
    }
}

fn extract_selection_text(
    screen: &vt100::Screen,
    start: (usize, usize),
    end: (usize, usize),
) -> String {
    let (start, end) = if start <= end {
        (start, end)
    } else {
        (end, start)
    };
    let (screen_rows, screen_cols) = screen.size();
    let max_r = screen_rows.get() as usize;
    let max_c = screen_cols.get() as usize;

    let mut result = String::new();
    for r in start.0..=end.0.min(max_r.saturating_sub(1)) {
        let col_start = if r == start.0 { start.1 } else { 0 };
        let col_end = if r == end.0 {
            end.1
        } else {
            max_c.saturating_sub(1)
        };
        let mut row_str = String::new();
        for c in col_start..=col_end.min(max_c.saturating_sub(1)) {
            if let Some(cell) = screen.cell(r as u16, c as u16) {
                row_str.push_str(cell.contents());
            } else {
                row_str.push(' ');
            }
        }
        if r > start.0 {
            result.push('\n');
        }
        result.push_str(row_str.trim_end());
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_row_syntax_highlights_utf8_alignment() {
        // Row with multi-byte UTF-8 box drawing and Vietnamese characters before "ERROR" and IPv4
        let row = "─ ✔ Lỗi hệ thống: ERROR tại 192.168.1.10 status OK";
        let char_count = row.chars().count();
        let highlights = compute_row_syntax_highlights(row, char_count);

        let err_char_pos = row
            .char_indices()
            .enumerate()
            .find(|(_, (byte_idx, _))| row[*byte_idx..].starts_with("ERROR"))
            .map(|(char_idx, _)| char_idx)
            .unwrap();
        for i in err_char_pos..err_char_pos + 5 {
            assert_eq!(highlights[i], Some(Color32::from_rgb(255, 90, 90)));
        }

        let ip_char_pos = row
            .char_indices()
            .enumerate()
            .find(|(_, (byte_idx, _))| row[*byte_idx..].starts_with("192.168.1.10"))
            .map(|(char_idx, _)| char_idx)
            .unwrap();
        for i in ip_char_pos..ip_char_pos + 12 {
            assert_eq!(highlights[i], Some(Color32::from_rgb(80, 210, 245)));
        }
    }

    #[test]
    fn test_format_paste_bytes_bracketed() {
        assert_eq!(format_paste_bytes("docker ps", false), b"docker ps");
        assert_eq!(
            format_paste_bytes("docker ps", true),
            b"\x1b[200~docker ps\x1b[201~"
        );
    }

    #[test]
    fn test_encode_terminal_key_sequence_ctrl_and_function_keys() {
        // Common process control & readline shortcuts
        assert_eq!(
            encode_terminal_key_sequence(Key::C, true, false, false),
            Some(b"\x03".as_slice())
        );
        assert_eq!(
            encode_terminal_key_sequence(Key::Z, true, false, false),
            Some(b"\x1a".as_slice())
        );
        assert_eq!(
            encode_terminal_key_sequence(Key::D, true, false, false),
            Some(b"\x04".as_slice())
        );
        assert_eq!(
            encode_terminal_key_sequence(Key::Backslash, true, false, false),
            Some(b"\x1c".as_slice())
        );
        assert_eq!(
            encode_terminal_key_sequence(Key::R, true, false, false),
            Some(b"\x12".as_slice())
        );
        assert_eq!(
            encode_terminal_key_sequence(Key::W, true, false, false),
            Some(b"\x17".as_slice())
        );
        assert_eq!(
            encode_terminal_key_sequence(Key::U, true, false, false),
            Some(b"\x15".as_slice())
        );
        assert_eq!(
            encode_terminal_key_sequence(Key::K, true, false, false),
            Some(b"\x0b".as_slice())
        );
        assert_eq!(
            encode_terminal_key_sequence(Key::A, true, false, false),
            Some(b"\x01".as_slice())
        );
        assert_eq!(
            encode_terminal_key_sequence(Key::E, true, false, false),
            Some(b"\x05".as_slice())
        );
        assert_eq!(
            encode_terminal_key_sequence(Key::L, true, false, false),
            Some(b"\x0c".as_slice())
        );
        assert_eq!(
            encode_terminal_key_sequence(Key::ArrowLeft, true, false, false),
            Some(b"\x1b[1;5D".as_slice())
        );
        assert_eq!(
            encode_terminal_key_sequence(Key::ArrowRight, true, false, false),
            Some(b"\x1b[1;5C".as_slice())
        );

        // Function keys F1..F12 for TUI tools (htop, mc, byobu)
        assert_eq!(
            encode_terminal_key_sequence(Key::F1, false, false, false),
            Some(b"\x1bOP".as_slice())
        );
        assert_eq!(
            encode_terminal_key_sequence(Key::F4, false, false, false),
            Some(b"\x1bOS".as_slice())
        );
        assert_eq!(
            encode_terminal_key_sequence(Key::F5, false, false, false),
            Some(b"\x1b[15~".as_slice())
        );
        assert_eq!(
            encode_terminal_key_sequence(Key::F9, false, false, false),
            Some(b"\x1b[20~".as_slice())
        );
        assert_eq!(
            encode_terminal_key_sequence(Key::F10, false, false, false),
            Some(b"\x1b[21~".as_slice())
        );
        assert_eq!(
            encode_terminal_key_sequence(Key::F12, false, false, false),
            Some(b"\x1b[24~".as_slice())
        );
    }

    #[test]
    fn test_compute_row_syntax_highlights_word_boundaries_and_invalid_ips() {
        // Substrings inside words ("terror", "tokens") and invalid IPv4 ("999.999.999.999", "1.2.3") must NOT be highlighted
        let row = "terror tokens 999.999.999.999 1.2.3 WARN: job failed on 10.20.30.40";
        let highlights = compute_row_syntax_highlights(row, row.chars().count());

        // "terror" (0..6), "tokens" (7..13), "999.999.999.999" (14..29), "1.2.3" (30..35) should all be None
        for idx in 0..35 {
            assert_eq!(
                highlights[idx], None,
                "Unexpected highlight at char index {}",
                idx
            );
        }

        // "WARN" at 36..40 -> yellow
        for idx in 36..40 {
            assert_eq!(highlights[idx], Some(Color32::from_rgb(255, 210, 60)));
        }

        // "failed" at 46..52 -> red
        for idx in 46..52 {
            assert_eq!(highlights[idx], Some(Color32::from_rgb(255, 90, 90)));
        }

        // "10.20.30.40" at 56..67 -> cyan
        for idx in 56..67 {
            assert_eq!(highlights[idx], Some(Color32::from_rgb(80, 210, 245)));
        }
    }

    #[test]
    fn test_selection_bounds_and_extract_selection_text() {
        let rows = std::num::NonZeroU16::new(10).unwrap();
        let cols = std::num::NonZeroU16::new(40).unwrap();
        let mut parser = vt100::Parser::new(rows, cols, 100);
        parser.process(b"CONTAINER ID   IMAGE\r\na1b2c3d4e5f6   postgres:16\r\n");

        // Single-cell selection (start == end) must be treated as unselected so click jitter never blocks Ctrl+C
        assert!(!is_cell_selected(1, 0, Some(((1, 0), (1, 0)))));
        assert!(is_cell_selected(1, 5, Some(((1, 0), (1, 11)))));

        // Forward and reverse drag selection extraction
        let forward = extract_selection_text(parser.screen(), (1, 0), (1, 11));
        let reverse = extract_selection_text(parser.screen(), (1, 11), (1, 0));
        assert_eq!(forward, "a1b2c3d4e5f6");
        assert_eq!(reverse, "a1b2c3d4e5f6");

        let multiline = extract_selection_text(parser.screen(), (0, 0), (1, 11));
        assert_eq!(multiline, "CONTAINER ID   IMAGE\na1b2c3d4e5f6");
    }

    #[test]
    fn test_scrollback_and_vt100_inverse_underline_attributes() {
        let rows = std::num::NonZeroU16::new(5).unwrap();
        let cols = std::num::NonZeroU16::new(40).unwrap();
        let mut parser = vt100::Parser::new(rows, cols, 200);

        for i in 1..=20 {
            parser.process(format!("Log line {:02}\r\n", i).as_bytes());
        }
        assert!(parser.screen().scrollback_len() >= 15);
        assert_eq!(parser.screen().scrollback(), 0);

        // Scroll up 10 lines into scrollback history
        parser.screen_mut().set_scrollback(10);
        assert_eq!(parser.screen().scrollback(), 10);
        assert!(parser.screen().contents().contains("Log line 07"));

        // Reset scrollback to 0 (simulating keypress snap-to-bottom)
        parser.screen_mut().set_scrollback(0);
        assert_eq!(parser.screen().scrollback(), 0);

        // Verify VT100 inverse (\x1b[7m) and underline (\x1b[4m) attributes
        parser.process(b"\x1b[2J\x1b[H\x1b[7mHTOP_BAR\x1b[0m \x1b[4mLINK\x1b[0m");
        let cell_inv = parser.screen().cell(0, 0).unwrap();
        assert!(cell_inv.inverse());
        assert!(!cell_inv.underline());

        let cell_ul = parser.screen().cell(0, 9).unwrap();
        assert!(!cell_ul.inverse());
        assert!(cell_ul.underline());
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn test_terminal_widget_ui_ctrl_c_and_copy_no_deadlock() {
        let mut sess = PtySession::spawn_local(24, 80, Some("/bin/sh"), None)
            .expect("Failed to spawn local PTY session");
        let ctx = egui::Context::default();
        let tab_id = "test_tab_deadlock";
        let sel_id = egui::Id::new("term_selection").with(tab_id);

        // 1. Seed an active selection and press Ctrl+C (both Key::C + Event::Copy in same frame)
        //    Verify it does NOT deadlock egui::Context when calling output_mut / data_mut, and clears selection!
        ctx.data_mut(|d| {
            d.insert_temp(sel_id, Some(((0usize, 0usize), (0usize, 8usize))));
        });

        let mut raw_input = egui::RawInput::default();
        raw_input.modifiers = egui::Modifiers::CTRL;
        raw_input.events.push(Event::Key {
            key: Key::C,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::CTRL,
        });
        raw_input.events.push(Event::Copy);

        let _ = ctx.run(raw_input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let _ = TerminalWidget::new(tab_id, &mut sess).ui(ui, None);
            });
        });

        let sel_after_first_ctrl_c: Option<((usize, usize), (usize, usize))> =
            ctx.data(|d| d.get_temp(sel_id));
        assert_eq!(
            sel_after_first_ctrl_c, None,
            "Selection must be cleared after first Ctrl+C copy"
        );

        // 2. Start a foreground streaming loop (like `docker compose logs -f -n 10`) and press Ctrl+C via egui UI
        sess.write_str("sh -c 'trap \"echo UI_SIGINT_OK; exit 0\" INT; echo UI_STREAM_START; while :; do sleep 1; done'\n")
            .unwrap();
        for _ in 0..30 {
            std::thread::sleep(std::time::Duration::from_millis(40));
            if sess
                .parser
                .lock()
                .screen()
                .contents()
                .contains("UI_STREAM_START")
            {
                break;
            }
        }

        let mut raw_input_2 = egui::RawInput::default();
        raw_input_2.modifiers = egui::Modifiers::CTRL;
        raw_input_2.events.push(Event::Key {
            key: Key::C,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::CTRL,
        });
        raw_input_2.events.push(Event::Copy);

        let _ = ctx.run(raw_input_2, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let _ = TerminalWidget::new(tab_id, &mut sess).ui(ui, None);
            });
        });

        let mut stopped = false;
        for _ in 0..30 {
            std::thread::sleep(std::time::Duration::from_millis(40));
            if sess
                .parser
                .lock()
                .screen()
                .contents()
                .contains("UI_SIGINT_OK")
            {
                stopped = true;
                break;
            }
        }
        assert!(
            stopped,
            "Ctrl+C through TerminalWidget::ui failed to interrupt foreground streaming process"
        );
        sess.kill();
    }
}
