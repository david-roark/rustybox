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

        let pos_to_cell = |pos: egui::Pos2| -> (usize, usize) {
            let r = (((pos.y - rect.top()) / row_height).floor().max(0.0) as usize).min(rows.saturating_sub(1) as usize);
            let c = (((pos.x - rect.left()) / char_width).floor().max(0.0) as usize).min(cols.saturating_sub(1) as usize);
            (r, c)
        };

        let mut selection: Option<((usize, usize), (usize, usize))> = ui.data(|d| d.get_temp(sel_id));

        if response.drag_started() {
            if let Some(pos) = response.interact_pointer_pos() {
                if rect.contains(pos) {
                    let cell = pos_to_cell(pos);
                    selection = Some((cell, cell));
                }
            }
        } else if response.dragged() {
            if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
                let cell = pos_to_cell(pos);
                if let Some((start, _)) = selection {
                    selection = Some((start, cell));
                }
            }
        }

        if response.drag_stopped() {
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
        }

        ui.data_mut(|d| d.insert_temp(sel_id, selection));

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

        // Active terminal tab owns focus unless an external TextEdit (like search) is actively focused
        let other_widget_focused = ui.memory(|m| {
            if let Some(fid) = m.focused() {
                fid != response.id && fid != term_id
            } else {
                false
            }
        });

        let has_focus = !other_widget_focused;

        // Ctrl + Mouse Wheel Zoom
        if response.hovered() && ui.input(|i| i.modifiers.ctrl) {
            let scroll_y = ui.input(|i| i.raw_scroll_delta.y);
            if scroll_y > 1.0 {
                action = TerminalAction::ZoomIn;
            } else if scroll_y < -1.0 {
                action = TerminalAction::ZoomOut;
            }
        }

        // Right-Click Context Menu (MobaXterm TFORMRIGHTCLICKMENU)
        response.context_menu(|ui| {
            ui.set_min_width(200.0);

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
                    ui.close_menu();
                }
            }

            if ui.button("📋 Paste Clipboard").clicked() {
                let text_opt = arboard::Clipboard::new().ok().and_then(|mut cb| cb.get_text().ok())
                    .or_else(|| {
                        let t = ui.output(|o| o.copied_text.clone());
                        if t.is_empty() { None } else { Some(t) }
                    });

                if let Some(text) = text_opt {
                    if !text.is_empty() {
                        if text.contains('\n') {
                            action = TerminalAction::MultiLinePaste(text);
                        } else {
                            let bytes = text.as_bytes().to_vec();
                            let _ = self.session.write_bytes(&bytes);
                            if let Some(sender) = broadcast_sender {
                                let _ = sender.send(bytes);
                            }
                        }
                    }
                }
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
            ui.label(RichText::new(format!("Size: {} cols x {} rows", cols, rows)).weak().size(11.0));
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

            let search_term = self
                .search_query
                .map(|q| q.trim().to_lowercase())
                .filter(|q| !q.is_empty());

            for r in 0..screen_rows.min(rows) {
                // Collect row string to check syntax highlighting keywords
                let mut row_str = String::with_capacity(screen_cols as usize);
                for c in 0..screen_cols.min(cols) {
                    if let Some(cell) = screen.cell(r, c) {
                        row_str.push_str(cell.contents());
                    } else {
                        row_str.push(' ');
                    }
                }

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

                        // Custom background
                        let bg_color = if is_selected_cell {
                            Color32::from_rgb(30, 95, 175) // Vibrant selection blue
                        } else if is_searched {
                            Color32::from_rgb(180, 140, 20) // Yellow highlight for search
                        } else {
                            vt_to_egui_color(cell.bgcolor(), false, self.theme)
                        };

                        if bg_color != self.theme.default_bg() || is_selected_cell {
                            painter.rect_filled(cell_rect, 0.0, bg_color);
                        }

                        // Text & Syntax Highlighting
                        let contents = cell.contents();
                        if !contents.is_empty() && contents != " " {
                            let mut fg_color = if is_selected_cell {
                                Color32::WHITE
                            } else {
                                vt_to_egui_color(cell.fgcolor(), true, self.theme)
                            };

                            // Apply MobaXterm-style smart syntax coloring if default color
                            if fg_color == self.theme.default_fg() && !is_selected_cell {
                                let col_idx = c as usize;
                                if is_in_keyword(
                                    &row_str,
                                    col_idx,
                                    &["error", "failed", "fatal", "exception", "err"],
                                ) {
                                    fg_color = Color32::from_rgb(255, 90, 90); // Red
                                } else if is_in_keyword(
                                    &row_str,
                                    col_idx,
                                    &["success", "ok", "passed", "running", "active"],
                                ) {
                                    fg_color = Color32::from_rgb(80, 230, 120); // Green
                                } else if is_in_keyword(&row_str, col_idx, &["warn", "warning"]) {
                                    fg_color = Color32::from_rgb(255, 210, 60); // Yellow
                                } else if is_ipv4_char(&row_str, col_idx) {
                                    fg_color = Color32::from_rgb(80, 210, 245); // Cyan for IP addresses
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

            // Cursor drawing: solid blinking when focused, hollow outline when unfocused
            if !cursor_hidden {
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
                        Stroke::new(1.0, Color32::from_rgba_premultiplied(180, 180, 200, 120)),
                    );
                }
            }
        }

        // Process keyboard inputs when terminal has focus
        if has_focus {
            let mut input_bytes: Vec<u8> = Vec::new();
            let mut paste_text_to_check: Option<String> = None;

            ui.input(|i| {
                for event in &i.events {
                    match event {
                        Event::Text(text) => {
                            if !i.modifiers.ctrl && !i.modifiers.command {
                                input_bytes.extend_from_slice(text.as_bytes());
                            }
                        }
                        Event::Key {
                            key,
                            pressed: true,
                            modifiers,
                            ..
                        } => {
                            let ctrl = modifiers.ctrl || modifiers.command;
                            match key {
                                Key::Enter => {
                                    if !self.session.running.load(std::sync::atomic::Ordering::Relaxed) {
                                        action = TerminalAction::RestartTab;
                                    } else {
                                        input_bytes.extend_from_slice(b"\r");
                                    }
                                }
                                Key::Backspace => {
                                    #[cfg(target_os = "windows")]
                                    {
                                        input_bytes.extend_from_slice(b"\x08");
                                    }
                                    #[cfg(not(target_os = "windows"))]
                                    {
                                        input_bytes.extend_from_slice(b"\x7f");
                                    }
                                }
                                Key::Tab => input_bytes.extend_from_slice(b"\t"),
                                Key::Escape => input_bytes.extend_from_slice(b"\x1b"),
                                Key::ArrowUp => input_bytes.extend_from_slice(b"\x1b[A"),
                                Key::ArrowDown => input_bytes.extend_from_slice(b"\x1b[B"),
                                Key::ArrowRight => input_bytes.extend_from_slice(b"\x1b[C"),
                                Key::ArrowLeft => input_bytes.extend_from_slice(b"\x1b[D"),
                                Key::Home => input_bytes.extend_from_slice(b"\x1b[H"),
                                Key::End => input_bytes.extend_from_slice(b"\x1b[F"),
                                Key::PageUp => input_bytes.extend_from_slice(b"\x1b[5~"),
                                Key::PageDown => input_bytes.extend_from_slice(b"\x1b[6~"),
                                Key::Delete => input_bytes.extend_from_slice(b"\x1b[3~"),
                                Key::Insert if modifiers.shift => {
                                    // Shift+Insert: Paste
                                    let text_opt = arboard::Clipboard::new().ok().and_then(|mut cb| cb.get_text().ok());
                                    if let Some(text) = text_opt {
                                        if text.contains('\n') {
                                            paste_text_to_check = Some(text);
                                        } else {
                                            input_bytes.extend_from_slice(text.as_bytes());
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
                                }
                                Key::Insert => input_bytes.extend_from_slice(b"\x1b[2~"),
                                Key::F if ctrl => action = TerminalAction::OpenSearch,
                                Key::Plus if ctrl => action = TerminalAction::ZoomIn,
                                Key::Equals if ctrl => action = TerminalAction::ZoomIn,
                                Key::Minus if ctrl => action = TerminalAction::ZoomOut,
                                Key::Num0 if ctrl => action = TerminalAction::ZoomReset,
                                Key::C if ctrl && modifiers.shift => {
                                    // Ctrl+Shift+C: Copy Selection or Screen Text
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
                                }
                                Key::V if ctrl && modifiers.shift => {
                                    // Ctrl+Shift+V: Paste from Clipboard
                                    let text_opt = arboard::Clipboard::new().ok().and_then(|mut cb| cb.get_text().ok());
                                    if let Some(text) = text_opt {
                                        if text.contains('\n') {
                                            paste_text_to_check = Some(text);
                                        } else {
                                            input_bytes.extend_from_slice(text.as_bytes());
                                        }
                                    }
                                }
                                Key::A if ctrl => input_bytes.extend_from_slice(b"\x01"), // Ctrl+A
                                Key::B if ctrl => input_bytes.extend_from_slice(b"\x02"), // Ctrl+B
                                Key::C if ctrl => input_bytes.extend_from_slice(b"\x03"), // Ctrl+C (SIGINT)
                                Key::D if ctrl => input_bytes.extend_from_slice(b"\x04"), // Ctrl+D (EOF)
                                Key::E if ctrl => input_bytes.extend_from_slice(b"\x05"), // Ctrl+E
                                Key::K if ctrl => input_bytes.extend_from_slice(b"\x0b"), // Ctrl+K
                                Key::L if ctrl => input_bytes.extend_from_slice(b"\x0c"), // Ctrl+L (Clear)
                                Key::U if ctrl => input_bytes.extend_from_slice(b"\x15"), // Ctrl+U
                                Key::W if ctrl => {
                                    if !self.session.running.load(std::sync::atomic::Ordering::Relaxed) {
                                        action = TerminalAction::CloseTab;
                                    } else {
                                        input_bytes.extend_from_slice(b"\x17");
                                    }
                                }
                                Key::Z if ctrl => input_bytes.extend_from_slice(b"\x1a"), // Ctrl+Z (SIGTSTP)
                                _ => {}
                            }
                        }
                        Event::Paste(text) => {
                            if text.contains('\n') {
                                paste_text_to_check = Some(text.clone());
                            } else {
                                input_bytes.extend_from_slice(text.as_bytes());
                            }
                        }
                        _ => {}
                    }
                }
            });

            if let Some(text) = paste_text_to_check {
                action = TerminalAction::MultiLinePaste(text);
            }

            if !input_bytes.is_empty() {
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

// Helper: check if a column position falls inside a keyword
fn is_in_keyword(row: &str, col: usize, keywords: &[&str]) -> bool {
    let lower = row.to_lowercase();
    for kw in keywords {
        let mut start = 0;
        while let Some(pos) = lower[start..].find(kw) {
            let actual = start + pos;
            if col >= actual && col < actual + kw.len() {
                // Word boundary check
                let before_ok =
                    actual == 0 || !lower.as_bytes()[actual - 1].is_ascii_alphanumeric();
                let after_ok = actual + kw.len() >= lower.len()
                    || !lower.as_bytes()[actual + kw.len()].is_ascii_alphanumeric();
                if before_ok && after_ok {
                    return true;
                }
            }
            start = actual + kw.len();
        }
    }
    false
}

// Helper: check if character is part of an IPv4 address pattern (e.g. 192.168.1.1)
fn is_ipv4_char(row: &str, col: usize) -> bool {
    let chars: Vec<char> = row.chars().collect();
    if col >= chars.len() || (!chars[col].is_ascii_digit() && chars[col] != '.') {
        return false;
    }
    // Expand left and right to get full token
    let mut left = col;
    while left > 0 && (chars[left - 1].is_ascii_digit() || chars[left - 1] == '.') {
        left -= 1;
    }
    let mut right = col;
    while right + 1 < chars.len() && (chars[right + 1].is_ascii_digit() || chars[right + 1] == '.')
    {
        right += 1;
    }
    let token: String = chars[left..=right].iter().collect();
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() == 4 && parts.iter().all(|p| p.parse::<u8>().is_ok()) {
        return true;
    }
    false
}

fn is_cell_selected(r: usize, c: usize, sel: Option<((usize, usize), (usize, usize))>) -> bool {
    let Some(((r1, c1), (r2, c2))) = sel else {
        return false;
    };
    if (r1, c1) == (r2, c2) {
        return false;
    }
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
    let (start, end) = if start <= end { (start, end) } else { (end, start) };
    let (screen_rows, screen_cols) = screen.size();
    let max_r = screen_rows.get() as usize;
    let max_c = screen_cols.get() as usize;

    let mut result = String::new();
    for r in start.0..=end.0.min(max_r.saturating_sub(1)) {
        let col_start = if r == start.0 { start.1 } else { 0 };
        let col_end = if r == end.0 { end.1 } else { max_c.saturating_sub(1) };
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
