use super::pty_session::PtySession;
use super::vt_screen::vt_to_egui_color;
use egui::{
    pos2, vec2, Color32, Event, FontFamily, FontId, Key, Rect, Response, Sense, Ui,
};

pub struct TerminalWidget<'a> {
    session: &'a mut PtySession,
    pub font_size: f32,
    pub search_query: Option<&'a str>,
}

impl<'a> TerminalWidget<'a> {
    pub fn new(_id_source: impl std::hash::Hash, session: &'a mut PtySession) -> Self {
        Self {
            session,
            font_size: 14.0,
            search_query: None,
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

    pub fn ui(self, ui: &mut Ui, broadcast_sender: Option<&crossbeam_channel::Sender<Vec<u8>>>) -> Response {
        let available_size = ui.available_size();
        let (rect, response) = ui.allocate_exact_size(available_size, Sense::click_and_drag());

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

        // Terminal background (MobaXterm deep dark theme)
        painter.rect_filled(rect, 0.0, Color32::from_rgb(16, 18, 22));

        // Focus handling
        if response.clicked() {
            ui.memory_mut(|m| m.request_focus(response.id));
        }
        let has_focus = ui.memory(|m| m.has_focus(response.id));

        // Render terminal contents with Syntax Highlighting & Search Matching
        {
            let parser = self.session.parser.lock();
            let screen = parser.screen();
            let (screen_rows_nz, screen_cols_nz) = screen.size();
            let screen_rows = screen_rows_nz.get();
            let screen_cols = screen_cols_nz.get();
            let (cursor_row, cursor_col) = screen.cursor_position();
            let cursor_hidden = screen.hide_cursor();

            let search_term = self.search_query.map(|q| q.trim().to_lowercase()).filter(|q| !q.is_empty());

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
                        let cell_rect = Rect::from_min_size(pos2(cell_x, cell_y), vec2(char_width, row_height));

                        // Check if current cell is inside a search match
                        let is_searched = search_matches.iter().any(|range| range.contains(&(c as usize)));

                        // Custom background
                        let bg_color = if is_searched {
                            Color32::from_rgb(180, 140, 20) // Yellow highlight for search
                        } else {
                            vt_to_egui_color(cell.bgcolor(), false)
                        };

                        if bg_color != Color32::from_rgb(16, 18, 22) {
                            painter.rect_filled(cell_rect, 0.0, bg_color);
                        }

                        // Text & Syntax Highlighting
                        let contents = cell.contents();
                        if !contents.is_empty() && contents != " " {
                            let mut fg_color = vt_to_egui_color(cell.fgcolor(), true);

                            // Apply MobaXterm-style smart syntax coloring if default color
                            if fg_color == Color32::from_rgb(220, 220, 225) {
                                // Check if cell is in an IP address or Error/Success keyword
                                let col_idx = c as usize;
                                if is_in_keyword(&row_str, col_idx, &["error", "failed", "fatal", "exception", "err"]) {
                                    fg_color = Color32::from_rgb(255, 90, 90); // Red
                                } else if is_in_keyword(&row_str, col_idx, &["success", "ok", "passed", "running", "active"]) {
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

            // Cursor drawing
            if !cursor_hidden && has_focus {
                let cur_x = rect.min.x + (cursor_col as f32) * char_width;
                let cur_y = rect.min.y + (cursor_row as f32) * row_height;
                let cur_rect = Rect::from_min_size(pos2(cur_x, cur_y), vec2(char_width, row_height));
                let blink = (ui.input(|i| i.time) * 2.0).fract() < 0.5;
                if blink {
                    painter.rect_filled(cur_rect, 1.0, Color32::from_rgba_premultiplied(230, 230, 240, 160));
                }
            }
        }

        // Process keyboard inputs when terminal has focus
        if has_focus {
            let mut input_bytes: Vec<u8> = Vec::new();

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
                                Key::Enter => input_bytes.extend_from_slice(b"\r"),
                                Key::Backspace => input_bytes.extend_from_slice(b"\x7f"),
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
                                Key::Insert => input_bytes.extend_from_slice(b"\x1b[2~"),
                                Key::A if ctrl => input_bytes.extend_from_slice(b"\x01"), // Ctrl+A
                                Key::B if ctrl => input_bytes.extend_from_slice(b"\x02"), // Ctrl+B
                                Key::C if ctrl => input_bytes.extend_from_slice(b"\x03"), // Ctrl+C (SIGINT)
                                Key::D if ctrl => input_bytes.extend_from_slice(b"\x04"), // Ctrl+D (EOF)
                                Key::E if ctrl => input_bytes.extend_from_slice(b"\x05"), // Ctrl+E
                                Key::K if ctrl => input_bytes.extend_from_slice(b"\x0b"), // Ctrl+K
                                Key::L if ctrl => input_bytes.extend_from_slice(b"\x0c"), // Ctrl+L (Clear)
                                Key::U if ctrl => input_bytes.extend_from_slice(b"\x15"), // Ctrl+U
                                Key::W if ctrl => input_bytes.extend_from_slice(b"\x17"), // Ctrl+W
                                Key::Z if ctrl => input_bytes.extend_from_slice(b"\x1a"), // Ctrl+Z (SIGTSTP)
                                _ => {}
                            }
                        }
                        Event::Paste(text) => {
                            input_bytes.extend_from_slice(text.as_bytes());
                        }
                        _ => {}
                    }
                }
            });

            if !input_bytes.is_empty() {
                let _ = self.session.write_bytes(&input_bytes);
                if let Some(sender) = broadcast_sender {
                    let _ = sender.send(input_bytes);
                }
            }
        }

        response
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
                let before_ok = actual == 0 || !lower.as_bytes()[actual - 1].is_ascii_alphanumeric();
                let after_ok = actual + kw.len() >= lower.len() || !lower.as_bytes()[actual + kw.len()].is_ascii_alphanumeric();
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
    while right + 1 < chars.len() && (chars[right + 1].is_ascii_digit() || chars[right + 1] == '.') {
        right += 1;
    }
    let token: String = chars[left..=right].iter().collect();
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() == 4 && parts.iter().all(|p| p.parse::<u8>().is_ok()) {
        return true;
    }
    false
}
