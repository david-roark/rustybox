use super::pty_session::PtySession;
use super::vt_screen::vt_to_egui_color;
use egui::{
    pos2, vec2, Color32, Event, FontFamily, FontId, Key, Rect, Response, Sense, Ui,
};

pub struct TerminalWidget<'a> {
    session: &'a mut PtySession,
    id_source: egui::Id,
    pub is_focused: bool,
}

impl<'a> TerminalWidget<'a> {
    pub fn new(id_source: impl std::hash::Hash, session: &'a mut PtySession) -> Self {
        Self {
            session,
            id_source: egui::Id::new(id_source),
            is_focused: false,
        }
    }

    pub fn ui(self, ui: &mut Ui, broadcast_sender: Option<&crossbeam_channel::Sender<Vec<u8>>>) -> Response {
        let available_size = ui.available_size();
        let (rect, response) = ui.allocate_exact_size(available_size, Sense::click_and_drag());

        // Monospace font setup
        let font_id = FontId::new(14.0, FontFamily::Monospace);
        let char_width = ui.fonts(|f| f.glyph_width(&font_id, 'M')).max(7.5);
        let row_height = ui.fonts(|f| f.row_height(&font_id)).max(16.0);

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

        // Terminal background
        painter.rect_filled(rect, 0.0, Color32::from_rgb(18, 20, 26));

        // Focus handling
        if response.clicked() {
            ui.memory_mut(|m| m.request_focus(response.id));
        }
        let has_focus = ui.memory(|m| m.has_focus(response.id));

        // Render terminal contents
        {
            let parser = self.session.parser.lock();
            let screen = parser.screen();
            let (screen_rows_nz, screen_cols_nz) = screen.size();
            let screen_rows = screen_rows_nz.get();
            let screen_cols = screen_cols_nz.get();
            let (cursor_row, cursor_col) = screen.cursor_position();
            let cursor_hidden = screen.hide_cursor();

            for r in 0..screen_rows.min(rows) {
                for c in 0..screen_cols.min(cols) {
                    if let Some(cell) = screen.cell(r, c) {
                        let cell_x = rect.min.x + (c as f32) * char_width;
                        let cell_y = rect.min.y + (r as f32) * row_height;
                        let cell_rect = Rect::from_min_size(pos2(cell_x, cell_y), vec2(char_width, row_height));

                        // Custom background
                        let bg_color = vt_to_egui_color(cell.bgcolor(), false);
                        if bg_color != Color32::from_rgb(18, 20, 26) {
                            painter.rect_filled(cell_rect, 0.0, bg_color);
                        }

                        // Text
                        let contents = cell.contents();
                        if !contents.is_empty() && contents != " " {
                            let fg_color = vt_to_egui_color(cell.fgcolor(), true);
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
