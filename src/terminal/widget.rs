use super::highlight::HighlightRule;
use super::pty_session::PtySession;
use super::vt_screen::{apply_bold_dim, vt_to_egui_color, TerminalTheme};
use egui::{
    pos2, vec2, Color32, Event, FontFamily, FontId, Key, Rect, Response, RichText, Sense, Stroke, Ui,
};

pub struct TerminalWidget<'a> {
    session: &'a mut PtySession,
    pub font_size: f32,
    pub font_family: FontFamily,
    pub search_query: Option<&'a str>,
    pub theme: TerminalTheme,
    pub tab_id: String,
    pub is_active_pane: bool,
    pub backspace_sends_ctrl_h: bool,
    pub right_click_paste: bool,
    pub highlight_rules: Option<&'a [HighlightRule]>,
    pub highlight_enabled: bool,
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
    NextTab,
    PrevTab,
}

impl<'a> TerminalWidget<'a> {
    pub fn new(tab_id: &str, session: &'a mut PtySession) -> Self {
        Self {
            session,
            font_size: 14.0,
            font_family: FontFamily::Monospace,
            search_query: None,
            theme: TerminalTheme::RustyBoxDark,
            tab_id: tab_id.to_string(),
            is_active_pane: true,
            backspace_sends_ctrl_h: false,
            right_click_paste: false,
            highlight_rules: None,
            highlight_enabled: true,
        }
    }

    pub fn with_font_family(mut self, family: FontFamily) -> Self {
        self.font_family = family;
        self
    }

    pub fn with_highlight_rules(mut self, rules: Option<&'a [HighlightRule]>, enabled: bool) -> Self {
        self.highlight_rules = rules;
        self.highlight_enabled = enabled;
        self
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

    pub fn with_active_pane(mut self, active: bool) -> Self {
        self.is_active_pane = active;
        self
    }

    pub fn with_backspace_ctrl_h(mut self, enabled: bool) -> Self {
        self.backspace_sends_ctrl_h = enabled;
        self
    }

    #[allow(dead_code)]
    pub fn with_right_click_paste(mut self, enabled: bool) -> Self {
        self.right_click_paste = enabled;
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
        let font_id = FontId::new(self.font_size, self.font_family.clone());
        let char_width = ui.fonts(|f| f.glyph_width(&font_id, 'M')).max(6.0);
        let row_height = ui.fonts(|f| f.row_height(&font_id)).max(12.0);

        let scrollbar_reserve = 10.0_f32;
        let text_area_w = (rect.width() - scrollbar_reserve).max(char_width * 20.0);
        let cols = ((text_area_w / char_width).floor() as u16).max(20);
        let rows = ((rect.height() / row_height).floor() as u16).max(5);

        // Resize PTY if dimensions changed
        let (
            scrollback_len,
            mut scrollback_offset,
            alternate_screen,
            mouse_mode,
            mouse_encoding,
            app_cursor,
        ) = {
            let mut p = self.session.parser.lock();
            let (cur_r_nz, cur_c_nz) = p.screen().size();
            if cur_r_nz.get() != rows || cur_c_nz.get() != cols {
                drop(p);
                self.session.resize(rows, cols);
                p = self.session.parser.lock();
            }
            let sc = p.screen();
            (
                sc.scrollback_len(),
                sc.scrollback(),
                sc.alternate_screen(),
                sc.mouse_protocol_mode(),
                sc.mouse_protocol_encoding(),
                sc.application_cursor(),
            )
        };

        let painter = ui.painter_at(rect);

        // Terminal background with selected theme
        painter.rect_filled(rect, 0.0, self.theme.default_bg());

        let term_id = egui::Id::new("terminal_focus").with(&self.tab_id);
        let sel_id = egui::Id::new("term_selection").with(&self.tab_id);
        let drag_origin_id = egui::Id::new("term_drag_origin").with(&self.tab_id);
        let sb_drag_id = egui::Id::new("term_sb_drag").with(&self.tab_id);
        let init_focus_id = egui::Id::new("term_init_focus").with(&self.tab_id);

        let pos_to_screen_cell = |pos: egui::Pos2| -> (usize, usize) {
            let r = (((pos.y - rect.top()) / row_height).floor().max(0.0) as usize)
                .min(rows.saturating_sub(1) as usize);
            let c = (((pos.x - rect.left()) / char_width).floor().max(0.0) as usize)
                .min(cols.saturating_sub(1) as usize);
            (r, c)
        };

        let pos_to_abs_cell = |pos: egui::Pos2, sb_offset: usize| -> (usize, usize) {
            let (screen_r, c) = pos_to_screen_cell(pos);
            let top_abs = scrollback_len.saturating_sub(sb_offset);
            (top_abs + screen_r, c)
        };

        let mut selection: Option<((usize, usize), (usize, usize))> =
            ui.data(|d| d.get_temp(sel_id));
        let mut drag_origin: Option<(usize, usize, egui::Pos2)> =
            ui.data(|d| d.get_temp(drag_origin_id));
        let mut dragging_scrollbar: bool = ui.data(|d| d.get_temp(sb_drag_id)).unwrap_or(false);

        let (hover_pos, shift_held, ctrl_held_now, middle_clicked, secondary_clicked, primary_clicked) =
            ui.input(|i| {
                (
                    i.pointer.hover_pos(),
                    i.modifiers.shift,
                    i.modifiers.ctrl || i.modifiers.command,
                    i.pointer.button_clicked(egui::PointerButton::Middle),
                    i.pointer.button_clicked(egui::PointerButton::Secondary),
                    i.pointer.button_clicked(egui::PointerButton::Primary),
                )
            });

        let scrollbar_rect = Rect::from_min_max(
            pos2(rect.right() - scrollbar_reserve, rect.top()),
            rect.max,
        );

        // Handle interactive vertical scrollbar click/drag or TUI mouse reporting / text selection
        let tui_captures_mouse = mouse_mode != vt100::MouseProtocolMode::None && !shift_held;

        if response.drag_started() {
            if let Some(pos) = response.interact_pointer_pos() {
                if !alternate_screen && scrollback_len > 0 && scrollbar_rect.contains(pos) {
                    dragging_scrollbar = true;
                } else if tui_captures_mouse {
                    let (sr, sc) = pos_to_screen_cell(pos);
                    if let Some(seq) = encode_mouse_event(
                        mouse_mode,
                        mouse_encoding,
                        0,
                        (sc as u16) + 1,
                        (sr as u16) + 1,
                        true,
                    ) {
                        let _ = self.session.write_bytes(&seq);
                    }
                } else if rect.contains(pos) {
                    let (abs_r, c) = pos_to_abs_cell(pos, scrollback_offset);
                    drag_origin = Some((abs_r, c, pos));
                    selection = None;
                }
            }
        } else if response.dragged() {
            if let Some(pos) = hover_pos {
                if dragging_scrollbar && !alternate_screen && scrollback_len > 0 {
                    let ratio = ((pos.y - rect.top()) / rect.height()).clamp(0.0, 1.0);
                    let new_sb = ((1.0 - ratio) * (scrollback_len as f32)).round() as usize;
                    let mut parser = self.session.parser.lock();
                    parser.screen_mut().set_scrollback(new_sb);
                    scrollback_offset = parser.screen().scrollback();
                } else if !tui_captures_mouse {
                    if let Some((orig_abs_r, orig_c, orig_pos)) = drag_origin {
                        // Auto-scroll when dragging near top/bottom edge to allow multi-page selection
                        if !alternate_screen && scrollback_len > 0 {
                            if pos.y < rect.top() + row_height * 0.75 {
                                let mut parser = self.session.parser.lock();
                                let new_sb = parser.screen().scrollback().saturating_add(1);
                                parser.screen_mut().set_scrollback(new_sb);
                                scrollback_offset = parser.screen().scrollback();
                            } else if pos.y > rect.bottom() - row_height * 0.75 {
                                let mut parser = self.session.parser.lock();
                                let new_sb = parser.screen().scrollback().saturating_sub(1);
                                parser.screen_mut().set_scrollback(new_sb);
                                scrollback_offset = parser.screen().scrollback();
                            }
                        }
                        if pos.distance(orig_pos) >= 5.0 || (pos_to_abs_cell(pos, scrollback_offset) != (orig_abs_r, orig_c)) {
                            let end_cell = pos_to_abs_cell(pos, scrollback_offset);
                            if (orig_abs_r, orig_c) != end_cell {
                                selection = Some(((orig_abs_r, orig_c), end_cell));
                            }
                        }
                    }
                }
            }
        }

        if response.drag_stopped() {
            if dragging_scrollbar {
                dragging_scrollbar = false;
            } else if tui_captures_mouse {
                if let Some(pos) = hover_pos.or_else(|| response.interact_pointer_pos()) {
                    let (sr, sc) = pos_to_screen_cell(pos);
                    if let Some(seq) = encode_mouse_event(
                        mouse_mode,
                        mouse_encoding,
                        0,
                        (sc as u16) + 1,
                        (sr as u16) + 1,
                        false,
                    ) {
                        let _ = self.session.write_bytes(&seq);
                    }
                }
            } else {
                drag_origin = None;
                if let Some((start, end)) = selection {
                    if start != end {
                        let mut parser = self.session.parser.lock();
                        let text =
                            extract_selection_text_with_scrollback(parser.screen_mut(), start, end);
                        drop(parser);
                        if !text.is_empty() {
                            copy_text_to_clipboard(ui, &text);
                        }
                    }
                }
            }
        }

        if response.triple_clicked() && !response.dragged() {
            if let Some(pos) = response.interact_pointer_pos() {
                if !tui_captures_mouse && rect.contains(pos) && !scrollbar_rect.contains(pos) {
                    let (abs_r, _) = pos_to_abs_cell(pos, scrollback_offset);
                    let mut parser = self.session.parser.lock();
                    let (_, screen_cols_nz) = parser.screen().size();
                    let max_c = (screen_cols_nz.get() as usize).saturating_sub(1);
                    let start = (abs_r, 0);
                    let end = (abs_r, max_c);
                    let text =
                        extract_selection_text_with_scrollback(parser.screen_mut(), start, end);
                    drop(parser);
                    if !text.is_empty() {
                        copy_text_to_clipboard(ui, &text);
                    }
                    selection = Some((start, end));
                    drag_origin = None;
                }
            }
        } else if response.double_clicked() && !response.dragged() {
            if let Some(pos) = response.interact_pointer_pos() {
                if !tui_captures_mouse && rect.contains(pos) && !scrollbar_rect.contains(pos) {
                    let (abs_r, c) = pos_to_abs_cell(pos, scrollback_offset);
                    let mut parser = self.session.parser.lock();
                    let (w_start, w_end) = find_word_bounds_in_row(parser.screen_mut(), abs_r, c);
                    let start = (abs_r, w_start);
                    let end = (abs_r, w_end);
                    let text =
                        extract_selection_text_with_scrollback(parser.screen_mut(), start, end);
                    drop(parser);
                    if !text.is_empty() {
                        copy_text_to_clipboard(ui, &text);
                    }
                    selection = Some((start, end));
                    drag_origin = None;
                }
            }
        } else if response.clicked() && !response.dragged() {
            if let Some(pos) = response.interact_pointer_pos() {
                if !alternate_screen && scrollback_len > 0 && scrollbar_rect.contains(pos) {
                    let ratio = ((pos.y - rect.top()) / rect.height()).clamp(0.0, 1.0);
                    let new_sb = ((1.0 - ratio) * (scrollback_len as f32)).round() as usize;
                    let mut parser = self.session.parser.lock();
                    parser.screen_mut().set_scrollback(new_sb);
                    scrollback_offset = parser.screen().scrollback();
                } else if tui_captures_mouse && !ctrl_held_now {
                    let (sr, sc) = pos_to_screen_cell(pos);
                    if let Some(press_seq) = encode_mouse_event(
                        mouse_mode,
                        mouse_encoding,
                        0,
                        (sc as u16) + 1,
                        (sr as u16) + 1,
                        true,
                    ) {
                        let _ = self.session.write_bytes(&press_seq);
                    }
                    if let Some(rel_seq) = encode_mouse_event(
                        mouse_mode,
                        mouse_encoding,
                        0,
                        (sc as u16) + 1,
                        (sr as u16) + 1,
                        false,
                    ) {
                        let _ = self.session.write_bytes(&rel_seq);
                    }
                }
            }
            selection = None;
            drag_origin = None;
        }

        ui.data_mut(|d| {
            d.insert_temp(sel_id, selection);
            d.insert_temp(drag_origin_id, drag_origin);
            d.insert_temp(sb_drag_id, dragging_scrollbar);
        });

        // Auto-focus newly opened active terminal pane on first render
        let already_initialized: bool = ui.data(|d| d.get_temp(init_focus_id)).unwrap_or(false);
        if !already_initialized {
            ui.data_mut(|d| d.insert_temp(init_focus_id, true));
            if self.is_active_pane {
                ui.memory_mut(|m| {
                    if let Some(fid) = m.focused() {
                        if fid != response.id && fid != term_id {
                            m.surrender_focus(fid);
                        }
                    }
                    m.request_focus(response.id);
                });
            }
        }

        // Clicking or pressing inside terminal immediately steals focus from any TextEdit or sibling split pane
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

        // Determine focus: if a widget is explicitly focused in egui memory, check if it's this pane;
        // if no widget is focused, only the active pane claims default focus.
        let has_focus = ui.memory(|m| {
            if let Some(fid) = m.focused() {
                fid == response.id || fid == term_id
            } else {
                self.is_active_pane
            }
        });

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

        // Middle-click paste (always enabled) or Right-click paste (when right_click_paste is enabled and Shift/Ctrl not held)
        let direct_right_click_paste =
            self.right_click_paste && !shift_held && !ctrl_held_now && secondary_clicked && pointer_in_terminal;
        let direct_middle_click_paste = middle_clicked && pointer_in_terminal;
        if direct_right_click_paste || direct_middle_click_paste {
            let text_opt = get_clipboard_text(ui, direct_middle_click_paste);
            if let Some(text) = text_opt {
                if !text.is_empty() {
                    if text.contains('\n') {
                        action = TerminalAction::MultiLinePaste(text);
                    } else if let Ok(bytes) = self.session.paste_text(&text) {
                        if let Some(sender) = broadcast_sender {
                            let _ = sender.send(bytes);
                        }
                    }
                }
            }
        }

        // Mouse Wheel: Ctrl + Wheel = Zoom, Wheel in TUI mouse mode = SGR 64/65, Wheel alone = Scrollback
        if response.hovered() {
            let scroll_y = ui.input(|i| i.raw_scroll_delta.y);
            if ctrl_held_now {
                if scroll_y > 1.0 {
                    action = TerminalAction::ZoomIn;
                } else if scroll_y < -1.0 {
                    action = TerminalAction::ZoomOut;
                }
            } else if scroll_y.abs() > 0.5 {
                let lines = ((scroll_y.abs() / row_height).ceil() as usize).clamp(1, 5);
                if tui_captures_mouse {
                    let pos = hover_pos.unwrap_or(rect.center());
                    let (sr, sc) = pos_to_screen_cell(pos);
                    let btn = if scroll_y > 0.0 { 64 } else { 65 };
                    for _ in 0..lines {
                        if let Some(seq) = encode_mouse_event(
                            mouse_mode,
                            mouse_encoding,
                            btn,
                            (sc as u16) + 1,
                            (sr as u16) + 1,
                            true,
                        ) {
                            let _ = self.session.write_bytes(&seq);
                        }
                    }
                } else if alternate_screen {
                    let seq: &[u8] = if scroll_y > 0.0 {
                        if app_cursor { b"\x1bOA" } else { b"\x1b[A" }
                    } else if app_cursor {
                        b"\x1bOB"
                    } else {
                        b"\x1b[B"
                    };
                    for _ in 0..lines {
                        let _ = self.session.write_bytes(seq);
                    }
                } else {
                    let mut parser = self.session.parser.lock();
                    let cur_sb = parser.screen().scrollback();
                    let new_sb = if scroll_y > 0.0 {
                        cur_sb.saturating_add(lines)
                    } else {
                        cur_sb.saturating_sub(lines)
                    };
                    parser.screen_mut().set_scrollback(new_sb);
                    scrollback_offset = parser.screen().scrollback();
                }
            }
        }

        // Right-Click Context Menu (MobaXterm TFORMRIGHTCLICKMENU)
        if !self.right_click_paste || shift_held || ctrl_held_now {
            response.context_menu(|ui| {
                ui.set_min_width(210.0);

                let has_selection = selection.map(|(s, e)| s != e).unwrap_or(false);
                if has_selection {
                    if ui.button("📄 Copy Selection").clicked() {
                        if let Some((s, e)) = selection {
                            let mut parser = self.session.parser.lock();
                            let text =
                                extract_selection_text_with_scrollback(parser.screen_mut(), s, e);
                            drop(parser);
                            if !text.is_empty() {
                                copy_text_to_clipboard(ui, &text);
                            }
                        }
                        selection = None;
                        ui.data_mut(|d| d.insert_temp(sel_id, selection));
                        ui.close_menu();
                    }
                }

                if ui.button("📋 Paste Clipboard").clicked() {
                    let text_opt = get_clipboard_text(ui, false);

                    if let Some(text) = text_opt {
                        if !text.is_empty() {
                            if text.contains('\n') {
                                action = TerminalAction::MultiLinePaste(text);
                            } else if let Ok(bytes) = self.session.paste_text(&text) {
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
                    let mut parser = self.session.parser.lock();
                    let all_text = extract_all_terminal_text(parser.screen_mut());
                    drop(parser);
                    if !all_text.is_empty() {
                        copy_text_to_clipboard(ui, &all_text);
                    }
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
        }

        // Render terminal contents with Syntax Highlighting, Wide Char support, URL detection & Search Matching
        let mut hovered_url: Option<String> = None;
        {
            let parser = self.session.parser.lock();
            let screen = parser.screen();
            let (screen_rows_nz, screen_cols_nz) = screen.size();
            let screen_rows = screen_rows_nz.get();
            let screen_cols = screen_cols_nz.get();
            let (cursor_row, cursor_col) = screen.cursor_position();
            let cursor_hidden = screen.hide_cursor();
            let visible_cols = screen_cols.min(cols) as usize;
            let top_abs_row = scrollback_len.saturating_sub(scrollback_offset);
            let hovered_screen_cell = hover_pos
                .filter(|p| rect.contains(*p))
                .map(pos_to_screen_cell);

            let search_term = self
                .search_query
                .map(|q| q.trim().to_lowercase())
                .filter(|q| !q.is_empty());

            for r in 0..screen_rows.min(rows) {
                let abs_row = top_abs_row + (r as usize);

                // Collect row string once per row to compute syntax, URL & search highlights
                let mut row_str = String::with_capacity(visible_cols);
                for c in 0..visible_cols {
                    if let Some(cell) = screen.cell(r, c as u16) {
                        if cell.is_wide_continuation() {
                            row_str.push(' ');
                        } else {
                            let s = cell.contents();
                            if s.is_empty() {
                                row_str.push(' ');
                            } else {
                                row_str.push_str(s);
                            }
                        }
                    } else {
                        row_str.push(' ');
                    }
                }

                // Precompute syntax highlight colors once per row (UTF-8 char-index accurate)
                let row_syntax_colors = compute_row_syntax_highlights_with_rules(
                    &row_str,
                    visible_cols,
                    self.highlight_rules,
                    self.highlight_enabled,
                );
                let row_urls = detect_row_urls(&row_str);

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
                        if cell.is_wide_continuation() {
                            continue;
                        }
                        let span_cols: u16 = if cell.is_wide() { 2 } else { 1 };
                        let cell_x = rect.min.x + (c as f32) * char_width;
                        let cell_y = rect.min.y + (r as f32) * row_height;
                        let cell_w = char_width * (span_cols as f32);
                        let cell_rect =
                            Rect::from_min_size(pos2(cell_x, cell_y), vec2(cell_w, row_height));

                        // Check if current cell (or its wide continuation) is selected or inside search match
                        let is_selected_cell = (0..span_cols).any(|dc| {
                            is_cell_selected(abs_row, (c + dc) as usize, selection)
                        });
                        let is_searched = (0..span_cols).any(|dc| {
                            search_matches
                                .iter()
                                .any(|range| range.contains(&((c + dc) as usize)))
                        });

                        let url_match = row_urls
                            .iter()
                            .find(|(u_start, u_end, _)| (c as usize) >= *u_start && (c as usize) < *u_end);
                        if let Some((_, _, url_str)) = url_match {
                            if hovered_screen_cell == Some((r as usize, c as usize)) {
                                hovered_url = Some(url_str.clone());
                            }
                        }

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

                        let url_underlined = url_match.is_some()
                            && ctrl_held_now
                            && hovered_screen_cell.map(|(hr, _)| hr == r as usize).unwrap_or(false);

                        if cell.underline() || url_underlined {
                            let y = cell_rect.bottom() - 1.5;
                            let ul_color = if url_underlined {
                                Color32::from_rgb(96, 165, 250)
                            } else {
                                base_fg
                            };
                            painter.line_segment(
                                [pos2(cell_rect.left(), y), pos2(cell_rect.right(), y)],
                                Stroke::new(1.0_f32, ul_color),
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

                            // Apply URL link color or MobaXterm-style smart syntax coloring if default color
                            if fg_color == self.theme.default_fg()
                                && !is_selected_cell
                                && !cell.inverse()
                            {
                                if url_match.is_some() {
                                    fg_color = Color32::from_rgb(96, 165, 250);
                                } else if let Some(Some(hl_col)) = row_syntax_colors.get(c as usize) {
                                    fg_color = *hl_col;
                                }
                            }

                            fg_color = apply_bold_dim(fg_color, cell.bold(), cell.dim());

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

            // Visual Scrollbar on the right edge when scrollback history is available
            if !alternate_screen && scrollback_len > 0 {
                let total_lines = (scrollback_len + screen_rows as usize).max(1) as f32;
                let track_rect = Rect::from_min_max(
                    pos2(rect.right() - 7.0, rect.top() + 2.0),
                    pos2(rect.right() - 1.5, rect.bottom() - 2.0),
                );
                painter.rect_filled(
                    track_rect,
                    3.0,
                    Color32::from_rgba_unmultiplied(30, 41, 59, 120),
                );

                let thumb_h = ((screen_rows as f32 / total_lines) * track_rect.height())
                    .clamp(22.0, track_rect.height());
                let scroll_ratio =
                    (top_abs_row as f32 / (scrollback_len.max(1) as f32)).clamp(0.0, 1.0);
                let thumb_y =
                    track_rect.top() + scroll_ratio * (track_rect.height() - thumb_h);
                let thumb_rect = Rect::from_min_size(
                    pos2(track_rect.left(), thumb_y),
                    vec2(track_rect.width(), thumb_h),
                );
                let thumb_color = if dragging_scrollbar || hover_pos.map(|p| scrollbar_rect.contains(p)).unwrap_or(false) {
                    Color32::from_rgb(56, 189, 248)
                } else {
                    Color32::from_rgba_unmultiplied(148, 163, 184, 160)
                };
                painter.rect_filled(thumb_rect, 3.0, thumb_color);
            }

            // Floating Scrollback Indicator Pill when viewing history
            if scrollback_offset > 0 {
                let badge_text = format!(
                    "📜 History: -{} lines (Scroll down or press any key)",
                    scrollback_offset
                );
                let badge_w = 300.0_f32.min(rect.width() - 20.0);
                let badge_rect = Rect::from_min_size(
                    pos2(rect.right() - badge_w - 12.0, rect.top() + 6.0),
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

        // Handle Ctrl+Hover & Ctrl+Click on detected URLs
        if let Some(url) = hovered_url {
            if ctrl_held_now {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                if primary_clicked {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(&url));
                }
            }
            response.clone().on_hover_text(format!("Ctrl+Click to open:\n{}", url));
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
                                } else if self.backspace_sends_ctrl_h || cfg!(target_os = "windows") {
                                    input_bytes.extend_from_slice(b"\x08");
                                } else {
                                    input_bytes.extend_from_slice(b"\x7f");
                                }
                            }
                            Key::Tab => {
                                if ctrl {
                                    action = if shift { TerminalAction::PrevTab } else { TerminalAction::NextTab };
                                } else if shift {
                                    input_bytes.extend_from_slice(b"\x1b[Z"); // Shift+Tab (Backtab)
                                } else {
                                    input_bytes.extend_from_slice(b"\t");
                                }
                            }
                            Key::Escape => input_bytes.extend_from_slice(b"\x1b"),
                            Key::ArrowUp if shift && !ctrl && !alt => {
                                scroll_delta_lines += 3;
                            }
                            Key::ArrowDown if shift && !ctrl && !alt => {
                                scroll_delta_lines -= 3;
                            }
                            Key::ArrowUp
                            | Key::ArrowDown
                            | Key::ArrowRight
                            | Key::ArrowLeft
                            | Key::Home
                            | Key::End => {
                                if let Some(seq) = encode_cursor_or_nav_key(
                                    *key, ctrl, alt, shift, app_cursor,
                                ) {
                                    input_bytes.extend_from_slice(seq);
                                }
                            }
                            Key::PageUp => {
                                if ctrl {
                                    action = TerminalAction::PrevTab;
                                } else if shift {
                                    scroll_delta_lines += (rows as isize / 2).max(5);
                                } else {
                                    input_bytes.extend_from_slice(b"\x1b[5~");
                                }
                            }
                            Key::PageDown => {
                                if ctrl {
                                    action = TerminalAction::NextTab;
                                } else if shift {
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
                                    let text_opt = get_clipboard_text(ui, true);
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
                                let mut parser = self.session.parser.lock();
                                let text = if let Some((s, e)) = selection {
                                    extract_selection_text_with_scrollback(
                                        parser.screen_mut(),
                                        s,
                                        e,
                                    )
                                } else {
                                    String::new()
                                };
                                drop(parser);
                                if !text.is_empty() {
                                    copy_text_to_clipboard(ui, &text);
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
                                let mut parser = self.session.parser.lock();
                                let text = if let Some((s, e)) = selection {
                                    extract_selection_text_with_scrollback(
                                        parser.screen_mut(),
                                        s,
                                        e,
                                    )
                                } else {
                                    String::new()
                                };
                                drop(parser);
                                if !text.is_empty() {
                                    copy_text_to_clipboard(ui, &text);
                                }
                                selection = None;
                                ui.data_mut(|d| d.insert_temp(sel_id, selection));
                            }
                            Key::V if ctrl && shift => {
                                // Ctrl+Shift+V: Paste from Clipboard
                                if !paste_handled {
                                    paste_handled = true;
                                    let text_opt = get_clipboard_text(ui, false);
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
                                        let mut parser = self.session.parser.lock();
                                        let text = if let Some((s, e)) = selection {
                                            extract_selection_text_with_scrollback(
                                                parser.screen_mut(),
                                                s,
                                                e,
                                            )
                                        } else {
                                            String::new()
                                        };
                                        drop(parser);
                                        if !text.is_empty() {
                                            copy_text_to_clipboard(ui, &text);
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
                                let mut parser = self.session.parser.lock();
                                let text = if let Some((s, e)) = selection {
                                    extract_selection_text_with_scrollback(
                                        parser.screen_mut(),
                                        s,
                                        e,
                                    )
                                } else {
                                    String::new()
                                };
                                drop(parser);
                                if !text.is_empty() {
                                    copy_text_to_clipboard(ui, &text);
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
                            let mut parser = self.session.parser.lock();
                            let text = if let Some((s, e)) = selection {
                                extract_selection_text_with_scrollback(parser.screen_mut(), s, e)
                            } else {
                                String::new()
                            };
                            drop(parser);
                            if !text.is_empty() {
                                copy_text_to_clipboard(ui, &text);
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

/// Encodes cursor & navigation keys (`ArrowUp/Down/Right/Left`, `Home`, `End`),
/// honoring `DECCKM` Application Cursor Keys mode (`\x1bOA`..`\x1bOF`) when unmodified.
pub fn encode_cursor_or_nav_key(
    key: Key,
    ctrl: bool,
    alt: bool,
    shift: bool,
    app_cursor: bool,
) -> Option<&'static [u8]> {
    match key {
        Key::ArrowUp => {
            if ctrl {
                Some(b"\x1b[1;5A")
            } else if alt {
                Some(b"\x1b[1;3A")
            } else if app_cursor && !shift {
                Some(b"\x1bOA")
            } else {
                Some(b"\x1b[A")
            }
        }
        Key::ArrowDown => {
            if ctrl {
                Some(b"\x1b[1;5B")
            } else if alt {
                Some(b"\x1b[1;3B")
            } else if app_cursor && !shift {
                Some(b"\x1bOB")
            } else {
                Some(b"\x1b[B")
            }
        }
        Key::ArrowRight => {
            if ctrl {
                Some(b"\x1b[1;5C")
            } else if alt {
                Some(b"\x1b[1;3C")
            } else if app_cursor && !shift {
                Some(b"\x1bOC")
            } else {
                Some(b"\x1b[C")
            }
        }
        Key::ArrowLeft => {
            if ctrl {
                Some(b"\x1b[1;5D")
            } else if alt {
                Some(b"\x1b[1;3D")
            } else if app_cursor && !shift {
                Some(b"\x1bOD")
            } else {
                Some(b"\x1b[D")
            }
        }
        Key::Home => {
            if app_cursor && !ctrl && !alt && !shift {
                Some(b"\x1bOH")
            } else {
                Some(b"\x1b[H")
            }
        }
        Key::End => {
            if app_cursor && !ctrl && !alt && !shift {
                Some(b"\x1bOF")
            } else {
                Some(b"\x1b[F")
            }
        }
        _ => None,
    }
}

/// Encodes mouse click/release/wheel events for TUI applications (`htop`, `tmux`, `mc`, `nvim`)
/// when `MouseProtocolMode` is active.
pub fn encode_mouse_event(
    mode: vt100::MouseProtocolMode,
    encoding: vt100::MouseProtocolEncoding,
    button: u8,
    col_1based: u16,
    row_1based: u16,
    pressed: bool,
) -> Option<Vec<u8>> {
    match mode {
        vt100::MouseProtocolMode::None => return None,
        vt100::MouseProtocolMode::Press if !pressed => return None,
        _ => {}
    }

    let c = col_1based.max(1);
    let r = row_1based.max(1);

    match encoding {
        vt100::MouseProtocolEncoding::Sgr => {
            let suffix = if pressed { 'M' } else { 'm' };
            Some(format!("\x1b[<{};{};{}{}", button, c, r, suffix).into_bytes())
        }
        vt100::MouseProtocolEncoding::Default | vt100::MouseProtocolEncoding::Utf8 => {
            let cb = if pressed { button } else { 3 }.saturating_add(32);
            let cx = (c.min(223) as u8).saturating_add(32);
            let cy = (r.min(223) as u8).saturating_add(32);
            Some(vec![0x1b, b'[', b'M', cb, cx, cy])
        }
    }
}

/// Detects `http://` and `https://` URLs on a single terminal row, returning `(char_start, char_end, url)`.
pub fn detect_row_urls(row: &str) -> Vec<(usize, usize, String)> {
    let chars: Vec<char> = row.chars().collect();
    let len = chars.len();
    let mut results = Vec::new();
    let mut i = 0;

    while i + 7 < len {
        let rest: String = chars[i..].iter().collect();
        let is_https = rest.starts_with("https://");
        let is_http = !is_https && rest.starts_with("http://");
        if is_https || is_http {
            let prefix_len = if is_https { 8 } else { 7 };
            let mut end = i + prefix_len;
            while end < len {
                let ch = chars[end];
                if ch.is_whitespace()
                    || matches!(ch, '<' | '>' | '"' | '\'' | '`' | '\0')
                {
                    break;
                }
                end += 1;
            }
            // Strip trailing punctuation that is typically not part of the URL
            while end > i + prefix_len
                && matches!(chars[end - 1], '.' | ',' | ';' | ':' | ')' | ']' | '}')
            {
                end -= 1;
            }
            if end > i + prefix_len {
                let url: String = chars[i..end].iter().collect();
                results.push((i, end, url));
                i = end;
                continue;
            }
        }
        i += 1;
    }

    results
}

pub fn format_paste_bytes(text: &str, bracketed: bool) -> Vec<u8> {
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
#[allow(dead_code)]
pub fn compute_row_syntax_highlights(row: &str, max_cols: usize) -> Vec<Option<Color32>> {
    compute_row_syntax_highlights_with_rules(row, max_cols, None, true)
}

/// Precomputes configurable keyword & IPv4 highlight colors once per row using UTF-8 character indices
pub fn compute_row_syntax_highlights_with_rules(
    row: &str,
    max_cols: usize,
    custom_rules: Option<&[HighlightRule]>,
    enabled: bool,
) -> Vec<Option<Color32>> {
    let mut colors = vec![None; max_cols];
    if !enabled || max_cols == 0 {
        return colors;
    }

    let chars: Vec<char> = row.chars().collect();
    let len = chars.len().min(max_cols);
    if len == 0 {
        return colors;
    }

    let lower_chars: Vec<char> = chars.iter().map(|c| c.to_ascii_lowercase()).collect();

    if let Some(rules) = custom_rules {
        for rule in rules {
            if !rule.enabled || rule.keyword.trim().is_empty() {
                continue;
            }
            let color = Color32::from_rgb(rule.color_rgb[0], rule.color_rgb[1], rule.color_rgb[2]);
            let kw = rule.keyword.trim();
            let kw_chars: Vec<char> = if rule.match_case {
                kw.chars().collect()
            } else {
                kw.to_ascii_lowercase().chars().collect()
            };
            let kw_len = kw_chars.len();
            if kw_len == 0 || kw_len > len {
                continue;
            }

            let mut i = 0;
            while i + kw_len <= len {
                let matches = if rule.match_case {
                    chars[i..i + kw_len] == kw_chars[..]
                } else {
                    lower_chars[i..i + kw_len] == kw_chars[..]
                };

                if matches {
                    let before_ok = if rule.whole_word {
                        i == 0 || (!lower_chars[i - 1].is_ascii_alphanumeric() && lower_chars[i - 1] != '_')
                    } else {
                        true
                    };
                    let after_ok = if rule.whole_word {
                        i + kw_len >= lower_chars.len()
                            || (!lower_chars[i + kw_len].is_ascii_alphanumeric() && lower_chars[i + kw_len] != '_')
                    } else {
                        true
                    };

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
    } else {
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
                            let before_ok = i == 0 || (!lower_chars[i - 1].is_ascii_alphanumeric() && lower_chars[i - 1] != '_');
                            let after_ok = i + kw_len >= lower_chars.len()
                                || (!lower_chars[i + kw_len].is_ascii_alphanumeric() && lower_chars[i + kw_len] != '_');
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
    }

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

#[allow(dead_code)]
pub fn extract_selection_text(
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
                if !cell.is_wide_continuation() {
                    row_str.push_str(cell.contents());
                }
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

/// Reads a single absolute row `abs_r` (`0 .. scrollback_len + screen_rows`) from `screen`.
fn read_abs_row_string(
    screen: &mut vt100::Screen,
    abs_r: usize,
    col_start: usize,
    col_end: usize,
) -> String {
    let (screen_rows_nz, screen_cols_nz) = screen.size();
    let screen_rows = screen_rows_nz.get() as usize;
    let screen_cols = screen_cols_nz.get() as usize;
    let sb_len = screen.scrollback_len();
    let total_rows = sb_len + screen_rows;
    if abs_r >= total_rows {
        return String::new();
    }

    let (target_sb, screen_r) = if abs_r < sb_len {
        (sb_len - abs_r, 0u16)
    } else {
        (0, (abs_r - sb_len) as u16)
    };
    screen.set_scrollback(target_sb);

    let mut row_str = String::new();
    let max_c = col_end.min(screen_cols.saturating_sub(1));
    for c in col_start..=max_c {
        if let Some(cell) = screen.cell(screen_r, c as u16) {
            if !cell.is_wide_continuation() {
                let s = cell.contents();
                if s.is_empty() {
                    row_str.push(' ');
                } else {
                    row_str.push_str(s);
                }
            }
        } else {
            row_str.push(' ');
        }
    }
    row_str
}

/// Extracts selected text across any absolute row range (`0 .. scrollback_len + screen_rows`),
/// preserving the user's current scrollback offset when finished.
pub fn extract_selection_text_with_scrollback(
    screen: &mut vt100::Screen,
    start: (usize, usize),
    end: (usize, usize),
) -> String {
    let (start, end) = if start <= end {
        (start, end)
    } else {
        (end, start)
    };
    let orig_sb = screen.scrollback();
    let (screen_rows_nz, screen_cols_nz) = screen.size();
    let total_rows = screen.scrollback_len() + (screen_rows_nz.get() as usize);
    let max_c = (screen_cols_nz.get() as usize).saturating_sub(1);

    let mut result = String::new();
    let end_r = end.0.min(total_rows.saturating_sub(1));
    for abs_r in start.0..=end_r {
        let col_start = if abs_r == start.0 { start.1 } else { 0 };
        let col_end = if abs_r == end.0 { end.1 } else { max_c };
        let row_str = read_abs_row_string(screen, abs_r, col_start, col_end);
        if abs_r > start.0 {
            result.push('\n');
        }
        result.push_str(row_str.trim_end());
    }
    screen.set_scrollback(orig_sb);
    result
}

/// Extracts the entire terminal text (all scrollback history + visible screen),
/// preserving the current scrollback offset.
pub fn extract_all_terminal_text(screen: &mut vt100::Screen) -> String {
    let orig_sb = screen.scrollback();
    let (screen_rows_nz, screen_cols_nz) = screen.size();
    let total_rows = screen.scrollback_len() + (screen_rows_nz.get() as usize);
    let max_c = (screen_cols_nz.get() as usize).saturating_sub(1);

    let mut all_text = String::new();
    for abs_r in 0..total_rows {
        let row_str = read_abs_row_string(screen, abs_r, 0, max_c);
        all_text.push_str(row_str.trim_end());
        all_text.push('\n');
    }
    screen.set_scrollback(orig_sb);
    all_text
}

/// Global persistent clipboard instance preventing X11/Wayland selection loss on drop.
static GLOBAL_CLIPBOARD: parking_lot::Mutex<Option<arboard::Clipboard>> = parking_lot::const_mutex(None);

/// Copies `text` to the OS clipboard, updating both `CLIPBOARD` and `PRIMARY` selections on Linux.
pub fn copy_text_to_clipboard(ui: &Ui, text: &str) {
    if text.is_empty() {
        return;
    }
    ui.output_mut(|o| o.copied_text = text.to_string());

    let mut guard = GLOBAL_CLIPBOARD.lock();
    if guard.is_none() {
        *guard = arboard::Clipboard::new().ok();
    }

    if let Some(cb) = guard.as_mut() {
        let set_res = {
            #[cfg(target_os = "linux")]
            {
                use arboard::{LinuxClipboardKind, SetExtLinux};
                let r1 = cb.set().clipboard(LinuxClipboardKind::Clipboard).text(text);
                let r2 = cb.set().clipboard(LinuxClipboardKind::Primary).text(text);
                r1.or(r2)
            }
            #[cfg(not(target_os = "linux"))]
            {
                cb.set_text(text)
            }
        };

        if set_res.is_err() {
            // Reconnect if clipboard server connection dropped
            if let Ok(mut new_cb) = arboard::Clipboard::new() {
                #[cfg(target_os = "linux")]
                {
                    use arboard::{LinuxClipboardKind, SetExtLinux};
                    let _ = new_cb.set().clipboard(LinuxClipboardKind::Clipboard).text(text);
                    let _ = new_cb.set().clipboard(LinuxClipboardKind::Primary).text(text);
                }
                #[cfg(not(target_os = "linux"))]
                {
                    let _ = new_cb.set_text(text);
                }
                *guard = Some(new_cb);
            }
        }
    }
}

/// Retrieves text from the OS clipboard.
/// On Linux, `prefer_primary = true` (e.g. for middle-click paste or Shift+Insert) queries `PRIMARY` first,
/// falling back to `CLIPBOARD`.
pub fn get_clipboard_text(ui: &Ui, prefer_primary: bool) -> Option<String> {
    let mut guard = GLOBAL_CLIPBOARD.lock();
    if guard.is_none() {
        *guard = arboard::Clipboard::new().ok();
    }

    let text_opt = if let Some(cb) = guard.as_mut() {
        #[cfg(target_os = "linux")]
        {
            use arboard::{GetExtLinux, LinuxClipboardKind};
            if prefer_primary {
                cb.get()
                    .clipboard(LinuxClipboardKind::Primary)
                    .text()
                    .or_else(|_| cb.get().clipboard(LinuxClipboardKind::Clipboard).text())
                    .ok()
            } else {
                cb.get()
                    .clipboard(LinuxClipboardKind::Clipboard)
                    .text()
                    .or_else(|_| cb.get().clipboard(LinuxClipboardKind::Primary).text())
                    .ok()
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = prefer_primary;
            cb.get_text().ok()
        }
    } else {
        None
    };

    text_opt.filter(|t| !t.is_empty()).or_else(|| {
        let t = ui.output(|o| o.copied_text.clone());
        if t.is_empty() {
            None
        } else {
            Some(t)
        }
    })
}

/// Finds the column range `(start_c, end_c)` of the word around `col` on absolute row `abs_r`.
pub fn find_word_bounds_in_row(
    screen: &mut vt100::Screen,
    abs_r: usize,
    col: usize,
) -> (usize, usize) {
    let (_screen_rows_nz, screen_cols_nz) = screen.size();
    let max_c = (screen_cols_nz.get() as usize).saturating_sub(1);
    let row_str = read_abs_row_string(screen, abs_r, 0, max_c);
    let chars: Vec<char> = row_str.chars().collect();
    if chars.is_empty() {
        return (col, col);
    }
    let c = col.min(chars.len().saturating_sub(1));
    let is_word_char = |ch: char| {
        ch.is_alphanumeric()
            || ch == '_'
            || ch == '-'
            || ch == '.'
            || ch == '/'
            || ch == ':'
            || ch == '@'
            || ch == '~'
    };
    let target_is_word = is_word_char(chars[c]);
    let target_is_space = chars[c].is_whitespace();

    let mut start_c = c;
    while start_c > 0 {
        let prev = chars[start_c - 1];
        if target_is_word && is_word_char(prev) {
            start_c -= 1;
        } else if target_is_space && prev.is_whitespace() {
            start_c -= 1;
        } else if !target_is_word && !target_is_space && !is_word_char(prev) && !prev.is_whitespace() {
            start_c -= 1;
        } else {
            break;
        }
    }

    let mut end_c = c;
    while end_c + 1 < chars.len() {
        let next = chars[end_c + 1];
        if target_is_word && is_word_char(next) {
            end_c += 1;
        } else if target_is_space && next.is_whitespace() {
            end_c += 1;
        } else if !target_is_word && !target_is_space && !is_word_char(next) && !next.is_whitespace() {
            end_c += 1;
        } else {
            break;
        }
    }

    (start_c, end_c)
}

/// Searches across the entire scrollback history and visible screen for `query` (case-insensitive),
/// returning a list of `(abs_row, col_start, col_end)` matches and preserving `screen.scrollback()`.
pub fn search_terminal_buffer(
    screen: &mut vt100::Screen,
    query: &str,
) -> Vec<(usize, usize, usize)> {
    let q_lower = query.trim().to_lowercase();
    if q_lower.is_empty() {
        return Vec::new();
    }
    let q_char_len = q_lower.chars().count();
    let orig_sb = screen.scrollback();
    let (screen_rows_nz, screen_cols_nz) = screen.size();
    let total_rows = screen.scrollback_len() + (screen_rows_nz.get() as usize);
    let max_c = (screen_cols_nz.get() as usize).saturating_sub(1);

    let mut matches = Vec::new();
    for abs_r in 0..total_rows {
        let row_str = read_abs_row_string(screen, abs_r, 0, max_c);
        let lower_row = row_str.to_lowercase();
        let mut start = 0;
        while let Some(pos) = lower_row[start..].find(&q_lower) {
            let actual_idx = start + pos;
            let char_idx = lower_row[..actual_idx].chars().count();
            matches.push((abs_r, char_idx, char_idx + q_char_len));
            start = actual_idx + q_lower.len();
        }
    }
    screen.set_scrollback(orig_sb);
    matches
}

/// Scrolls the terminal buffer so that `abs_row` is visible near the vertical center of the viewport.
pub fn scroll_to_abs_row(screen: &mut vt100::Screen, abs_row: usize) {
    let sb_len = screen.scrollback_len();
    let screen_rows = screen.size().0.get() as usize;
    let half_view = screen_rows / 2;
    let desired_top_abs = abs_row.saturating_sub(half_view).min(sb_len);
    let new_scrollback = sb_len.saturating_sub(desired_top_abs);
    screen.set_scrollback(new_scrollback);
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
    fn test_custom_highlight_rules_and_toggle() {
        let mut custom_rules = vec![
            HighlightRule::new("critical", 255, 0, 0),
            HighlightRule::new("timeout", 216, 112, 240),
        ];
        custom_rules.push(HighlightRule {
            id: "disabled_rule".to_string(),
            keyword: "ignored".to_string(),
            color_rgb: [100, 100, 100],
            enabled: false,
            whole_word: true,
            match_case: false,
        });

        let line = "Connection TIMEOUT: CRITICAL alert, ignored text";
        let cols = line.chars().count();

        // 1. With rules enabled
        let highlights = compute_row_syntax_highlights_with_rules(line, cols, Some(&custom_rules), true);
        // "TIMEOUT" at index 11..18 -> magenta
        for i in 11..18 {
            assert_eq!(highlights[i], Some(Color32::from_rgb(216, 112, 240)));
        }
        // "CRITICAL" at index 20..28 -> red
        for i in 20..28 {
            assert_eq!(highlights[i], Some(Color32::from_rgb(255, 0, 0)));
        }
        // "ignored" is disabled -> None
        for i in 36..43 {
            assert_eq!(highlights[i], None);
        }

        // 2. With global toggle disabled
        let disabled_hl = compute_row_syntax_highlights_with_rules(line, cols, Some(&custom_rules), false);
        for h in disabled_hl {
            assert_eq!(h, None);
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

    #[test]
    fn test_application_cursor_keys_mode_and_nav_encoding() {
        // Normal mode (DECCKM off)
        assert_eq!(
            encode_cursor_or_nav_key(Key::ArrowUp, false, false, false, false),
            Some(b"\x1b[A".as_slice())
        );
        assert_eq!(
            encode_cursor_or_nav_key(Key::ArrowDown, false, false, false, false),
            Some(b"\x1b[B".as_slice())
        );
        assert_eq!(
            encode_cursor_or_nav_key(Key::Home, false, false, false, false),
            Some(b"\x1b[H".as_slice())
        );
        assert_eq!(
            encode_cursor_or_nav_key(Key::End, false, false, false, false),
            Some(b"\x1b[F".as_slice())
        );

        // Application Cursor Keys mode (DECCKM on: vim, less, htop, tmux)
        assert_eq!(
            encode_cursor_or_nav_key(Key::ArrowUp, false, false, false, true),
            Some(b"\x1bOA".as_slice())
        );
        assert_eq!(
            encode_cursor_or_nav_key(Key::ArrowDown, false, false, false, true),
            Some(b"\x1bOB".as_slice())
        );
        assert_eq!(
            encode_cursor_or_nav_key(Key::ArrowRight, false, false, false, true),
            Some(b"\x1bOC".as_slice())
        );
        assert_eq!(
            encode_cursor_or_nav_key(Key::ArrowLeft, false, false, false, true),
            Some(b"\x1bOD".as_slice())
        );
        assert_eq!(
            encode_cursor_or_nav_key(Key::Home, false, false, false, true),
            Some(b"\x1bOH".as_slice())
        );
        assert_eq!(
            encode_cursor_or_nav_key(Key::End, false, false, false, true),
            Some(b"\x1bOF".as_slice())
        );

        // Ctrl+Arrow always sends word-jump CSI even when app_cursor is true
        assert_eq!(
            encode_cursor_or_nav_key(Key::ArrowLeft, true, false, false, true),
            Some(b"\x1b[1;5D".as_slice())
        );
    }

    #[test]
    fn test_mouse_protocol_encoding_sgr_and_default() {
        assert_eq!(
            encode_mouse_event(
                vt100::MouseProtocolMode::None,
                vt100::MouseProtocolEncoding::Sgr,
                0,
                10,
                5,
                true
            ),
            None
        );

        // SGR (1006) press & release
        assert_eq!(
            encode_mouse_event(
                vt100::MouseProtocolMode::PressRelease,
                vt100::MouseProtocolEncoding::Sgr,
                0,
                12,
                8,
                true
            ),
            Some(b"\x1b[<0;12;8M".to_vec())
        );
        assert_eq!(
            encode_mouse_event(
                vt100::MouseProtocolMode::PressRelease,
                vt100::MouseProtocolEncoding::Sgr,
                0,
                12,
                8,
                false
            ),
            Some(b"\x1b[<0;12;8m".to_vec())
        );

        // Default X10 mode press
        assert_eq!(
            encode_mouse_event(
                vt100::MouseProtocolMode::Press,
                vt100::MouseProtocolEncoding::Default,
                0,
                1,
                1,
                true
            ),
            Some(vec![0x1b, b'[', b'M', 32, 33, 33])
        );
    }

    #[test]
    fn test_detect_row_urls_and_wide_char_handling() {
        let row = "Visit https://github.com/rustybox/repo, or (http://10.0.0.1:8080/health).";
        let urls = detect_row_urls(row);
        assert_eq!(urls.len(), 2);
        assert_eq!(urls[0].2, "https://github.com/rustybox/repo");
        assert_eq!(urls[1].2, "http://10.0.0.1:8080/health");

        // Wide character (emoji / CJK) extraction should not duplicate continuation cells
        let rows = std::num::NonZeroU16::new(4).unwrap();
        let cols = std::num::NonZeroU16::new(30).unwrap();
        let mut parser = vt100::Parser::new(rows, cols, 50);
        parser.process("🐳 Docker Tiếng Việt\r\n".as_bytes());
        let extracted =
            extract_selection_text_with_scrollback(parser.screen_mut(), (0, 0), (0, 25));
        assert_eq!(extracted, "🐳 Docker Tiếng Việt");
    }

    #[test]
    fn test_scrollback_search_and_cross_page_selection() {
        let rows = std::num::NonZeroU16::new(5).unwrap();
        let cols = std::num::NonZeroU16::new(40).unwrap();
        let mut parser = vt100::Parser::new(rows, cols, 200);

        for i in 1..=25 {
            if i == 3 || i == 18 {
                parser.process(format!("Line {:02}: CRITICAL_TOKEN found\r\n", i).as_bytes());
            } else {
                parser.process(format!("Line {:02}: normal log output\r\n", i).as_bytes());
            }
        }

        // Search across the entire scrollback + live screen
        let matches = search_terminal_buffer(parser.screen_mut(), "critical_token");
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].0, 2); // Line 03 is at abs_row 2
        assert_eq!(matches[1].0, 17); // Line 18 is at abs_row 17

        // Jump viewport to first match (in scrollback history)
        scroll_to_abs_row(parser.screen_mut(), matches[0].0);
        assert!(parser.screen().scrollback() > 0);
        assert!(parser.screen().contents().contains("Line 03: CRITICAL_TOKEN"));

        // Extract multi-line selection spanning scrollback rows (abs_row 1..=3)
        let selected =
            extract_selection_text_with_scrollback(parser.screen_mut(), (1, 0), (3, 35));
        assert!(selected.contains("Line 02: normal log output"));
        assert!(selected.contains("Line 03: CRITICAL_TOKEN found"));
        assert!(selected.contains("Line 04: normal log output"));
    }

    #[test]
    fn test_find_word_bounds_and_clipboard_helpers() {
        let rows = std::num::NonZeroU16::new(5).unwrap();
        let cols = std::num::NonZeroU16::new(60).unwrap();
        let mut parser = vt100::Parser::new(rows, cols, 100);
        parser.process(b"curl https://api.example.com/v1 -H 'Auth: Bearer'\r\n");

        // Double-click on "https://api.example.com/v1" (col 5) should select the full URL token
        let (w_start, w_end) = find_word_bounds_in_row(parser.screen_mut(), 0, 5);
        let extracted_url =
            extract_selection_text_with_scrollback(parser.screen_mut(), (0, w_start), (0, w_end));
        assert_eq!(extracted_url, "https://api.example.com/v1");

        // Double-click on "curl" (col 1) should select "curl"
        let (c_start, c_end) = find_word_bounds_in_row(parser.screen_mut(), 0, 1);
        let extracted_cmd =
            extract_selection_text_with_scrollback(parser.screen_mut(), (0, c_start), (0, c_end));
        assert_eq!(extracted_cmd, "curl");

        // Test copy_text_to_clipboard and get_clipboard_text with egui context
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                copy_text_to_clipboard(ui, "rustybox_clipboard_test");
                let retrieved = get_clipboard_text(ui, false);
                assert_eq!(retrieved, Some("rustybox_clipboard_test".to_string()));
            });
        });
    }
}
