pub mod layouter;
pub mod syntax;

use self::layouter::render_editor_layouter;
use self::syntax::{detect_syntax, SyntaxType};
use egui::{vec2, Color32, FontId, RichText, ScrollArea, TextEdit, Vec2};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorAction {
    None,
    Save {
        path: PathBuf,
        content: String,
        remote_label: Option<String>,
    },
    Close,
}

pub struct Editor {
    pub open: bool,
    pub file_path: Option<PathBuf>,
    pub remote_label: Option<String>,
    pub content: String,
    pub original_content: String,
    pub status: Option<String>,
    pub font_size: f32,
    pub word_wrap: bool,
    pub show_search: bool,
    pub search_query: String,
    pub search_matches: usize,
    pub active_match_idx: usize,
    pub syntax: SyntaxType,
    pub show_unsaved_confirm: bool,
}

impl Default for Editor {
    fn default() -> Self {
        Self {
            open: false,
            file_path: None,
            remote_label: None,
            content: String::new(),
            original_content: String::new(),
            status: None,
            font_size: 13.0,
            word_wrap: false,
            show_search: false,
            search_query: String::new(),
            search_matches: 0,
            active_match_idx: 0,
            syntax: SyntaxType::PlainText,
            show_unsaved_confirm: false,
        }
    }
}

impl Editor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open_file(&mut self, path: PathBuf, content: String, remote_label: Option<String>) {
        let detected = detect_syntax(Some(&path), &content);
        self.file_path = Some(path);
        self.remote_label = remote_label;
        self.content = content.clone();
        self.original_content = content;
        self.status = None;
        self.syntax = detected;
        self.show_search = false;
        self.search_query.clear();
        self.search_matches = 0;
        self.active_match_idx = 0;
        self.show_unsaved_confirm = false;
        self.open = true;
    }

    pub fn is_dirty(&self) -> bool {
        self.content != self.original_content
    }

    pub fn mark_saved(&mut self) {
        self.original_content = self.content.clone();
        let target = self
            .remote_label
            .as_deref()
            .unwrap_or("Local Disk");
        self.status = Some(format!("Saved successfully to {}!", target));
    }

    pub fn update_search(&mut self) {
        let q = self.search_query.trim();
        if !q.is_empty() {
            let rules = self.syntax.rules();
            if rules.case_insensitive {
                let needle = q.to_lowercase();
                let haystack = self.content.to_lowercase();
                self.search_matches = haystack.match_indices(&needle).count();
            } else {
                self.search_matches = self.content.match_indices(q).count();
            }
            if self.search_matches == 0 {
                self.active_match_idx = 0;
            } else if self.active_match_idx >= self.search_matches {
                self.active_match_idx = self.search_matches - 1;
            }
        } else {
            self.search_matches = 0;
            self.active_match_idx = 0;
        }
    }

    pub fn next_match(&mut self) {
        if self.search_matches > 0 {
            self.active_match_idx = (self.active_match_idx + 1) % self.search_matches;
        }
    }

    pub fn prev_match(&mut self) {
        if self.search_matches > 0 {
            if self.active_match_idx == 0 {
                self.active_match_idx = self.search_matches - 1;
            } else {
                self.active_match_idx -= 1;
            }
        }
    }

    pub fn clean_tabs_to_spaces(&mut self, spaces_per_tab: usize) -> usize {
        let count = self.content.matches('\t').count();
        if count > 0 {
            let replacement = " ".repeat(spaces_per_tab);
            self.content = self.content.replace('\t', &replacement);
            self.status = Some(format!("Cleaned {} Tab(s) → {} spaces!", count, spaces_per_tab));
        } else {
            self.status = Some("No tabs found (file is clean)".to_string());
        }
        count
    }

    pub fn show(&mut self, ctx: &egui::Context) -> EditorAction {
        if !self.open {
            return EditorAction::None;
        }

        let mut action = EditorAction::None;
        let mut trigger_save = false;

        // Global shortcuts inside editor
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::S)) {
            trigger_save = true;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::F)) {
            self.show_search = !self.show_search;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F3)) {
            self.next_match();
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::SHIFT, egui::Key::F3)) {
            self.prev_match();
        }

        // Close on Esc if no search bar open
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            if self.show_search {
                self.show_search = false;
            } else if self.is_dirty() {
                self.show_unsaved_confirm = true;
            } else {
                self.open = false;
                return EditorAction::Close;
            }
        }

        let file_name = self
            .file_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Untitled".to_string());

        let dirty_suffix = if self.is_dirty() { " ● (Unsaved)" } else { "" };
        let window_title = format!("📝 Quick Editor — {}{}", file_name, dirty_suffix);

        let screen = ctx.screen_rect();
        let fixed_h = (screen.height() * 0.80).max(300.0);
        let max_w = (screen.width() - 40.0).max(400.0);
        let def_w = (screen.width() * 0.80).clamp(520.0, 960.0).min(max_w);

        let mut window_open = self.open;
        let mut request_close = false;

        egui::Window::new(window_title)
            .id(egui::Id::new("builtin_quick_code_editor_window"))
            .open(&mut window_open)
            .collapsible(false)
            .resizable(true)
            .default_size(Vec2::new(def_w, fixed_h))
            .min_size(Vec2::new(420.0, fixed_h))
            .max_size(Vec2::new(max_w, fixed_h))
            .constrain_to(screen.shrink(16.0))
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(screen.center())
            .show(ctx, |ui| {
                // Toolbar
                ui.horizontal(|ui| {
                    if (ui.button("💾 Save (Ctrl+S)").clicked() || trigger_save)
                        && self.file_path.is_some()
                        && let Some(ref path) = self.file_path {
                            action = EditorAction::Save {
                                path: path.clone(),
                                content: self.content.clone(),
                                remote_label: self.remote_label.clone(),
                            };
                        }

                    if ui.button("🔍 Find (Ctrl+F)").clicked() {
                        self.show_search = !self.show_search;
                    }

                    let wrap_text = if self.word_wrap { "↩ Wrap: ON" } else { "➡ Wrap: OFF" };
                    if ui.button(wrap_text).on_hover_text("Toggle word wrapping").clicked() {
                        self.word_wrap = !self.word_wrap;
                    }

                    if ui.button("🧹 Clean Tabs").on_hover_text("Convert all Tab characters to 2 spaces (fixes indentation in YAML, Python, etc.)").clicked() {
                        self.clean_tabs_to_spaces(2);
                    }

                    // Font Zoom
                    if ui.small_button("➖").on_hover_text("Zoom out font").clicked() && self.font_size > 9.0 {
                        self.font_size -= 1.0;
                    }
                    ui.label(RichText::new(format!("{:.0}px", self.font_size)).size(11.0).monospace());
                    if ui.small_button("➕").on_hover_text("Zoom in font").clicked() && self.font_size < 28.0 {
                        self.font_size += 1.0;
                    }

                    // Syntax Selector / Info Pill
                    egui::ComboBox::from_id_salt("editor_syntax_select")
                        .selected_text(self.syntax.name())
                        .show_ui(ui, |ui| {
                            let list = [
                                SyntaxType::Bash,
                                SyntaxType::Yaml,
                                SyntaxType::Json,
                                SyntaxType::Toml,
                                SyntaxType::IniConf,
                                SyntaxType::Python,
                                SyntaxType::Rust,
                                SyntaxType::C,
                                SyntaxType::Sql,
                                SyntaxType::Markdown,
                                SyntaxType::HtmlXml,
                                SyntaxType::Ruby,
                                SyntaxType::JavaScript,
                                SyntaxType::Go,
                                SyntaxType::Php,
                                SyntaxType::Css,
                                SyntaxType::Lua,
                                SyntaxType::Java,
                                SyntaxType::PlainText,
                            ];
                            for s in list {
                                ui.selectable_value(&mut self.syntax, s, s.name());
                            }
                        });

                    if ui.button("✖ Close").clicked() {
                        request_close = true;
                    }

                    if let Some(ref st) = self.status {
                        ui.label(RichText::new(st).color(Color32::from_rgb(52, 211, 153)).strong());
                    }
                });

                // Search Panel
                if self.show_search {
                    ui.add_space(3.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Find:").size(11.5).strong());
                        let search_resp = ui.add(
                            TextEdit::singleline(&mut self.search_query)
                                .hint_text("Type keyword (Enter for next)...")
                                .desired_width(200.0),
                        );
                        if search_resp.changed() {
                            self.update_search();
                        }
                        if search_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            if ui.input(|i| i.modifiers.shift) {
                                self.prev_match();
                            } else {
                                self.next_match();
                            }
                        }

                        if ui.small_button("▲ Prev").on_hover_text("Previous match (Shift+F3)").clicked() {
                            self.prev_match();
                        }
                        if ui.small_button("▼ Next").on_hover_text("Next match (F3 or Enter)").clicked() {
                            self.next_match();
                        }

                        if !self.search_query.trim().is_empty() {
                            let match_color = if self.search_matches > 0 {
                                Color32::from_rgb(52, 211, 153)
                            } else {
                                Color32::from_rgb(248, 113, 113)
                            };
                            let match_text = if self.search_matches > 0 {
                                format!("{}/{} matches", self.active_match_idx + 1, self.search_matches)
                            } else {
                                "0 matches".to_string()
                            };
                            ui.label(RichText::new(match_text).color(match_color).size(11.0).monospace());
                        }

                        if ui.small_button("✖").on_hover_text("Close search").clicked() {
                            self.show_search = false;
                        }
                    });
                }

                ui.separator();

                // Compute line numbers
                let line_count = self.content.split('\n').count().max(1);
                let digits = line_count.to_string().len().max(2);
                let line_numbers_text = (1..=line_count)
                    .map(|n| format!("{:>width$}", n, width = digits))
                    .collect::<Vec<String>>()
                    .join("\n");

                let total_chars = self.content.len();
                let editor_font = FontId::monospace(self.font_size);
                let bottom_status_height = 28.0;
                let editor_avail_height = (ui.available_height() - bottom_status_height).max(120.0);

                ScrollArea::both()
                    .max_height(editor_avail_height)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.horizontal_top(|ui| {
                            // Left gutter: Line numbers
                            let line_w = (digits as f32 * 8.5 * (self.font_size / 13.0)).max(28.0);
                            ui.allocate_ui_with_layout(
                                vec2(line_w, 0.0),
                                egui::Layout::top_down(egui::Align::RIGHT),
                                |ui| {
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(&line_numbers_text)
                                                .font(editor_font.clone())
                                                .color(Color32::from_rgb(100, 116, 139)),
                                        )
                                        .selectable(false),
                                    );
                                },
                            );

                            ui.separator();

                            // Main code editor area with Kibi-style syntax layouter
                            let edit_width = if self.word_wrap {
                                (ui.available_width() - 8.0).max(100.0)
                            } else {
                                f32::INFINITY
                            };

                            let syntax = self.syntax;
                            let font_size = self.font_size;
                            let query = self.search_query.clone();
                            let active_idx = self.active_match_idx;
                            let word_wrap = self.word_wrap;

                            let mut layouter = move |ui: &egui::Ui, string: &str, wrap_width: f32| {
                                render_editor_layouter(
                                    ui.ctx(),
                                    string,
                                    syntax,
                                    font_size,
                                    &query,
                                    active_idx,
                                    word_wrap,
                                    wrap_width,
                                )
                            };

                            let edit_resp = ui.add(
                                TextEdit::multiline(&mut self.content)
                                    .id_salt("builtin_editor_multiline_content")
                                    .font(editor_font)
                                    .desired_width(edit_width)
                                    .desired_rows(10)
                                    .lock_focus(true)
                                    .layouter(&mut layouter),
                            );

                            if edit_resp.changed() {
                                self.status = None;
                                if self.show_search {
                                    self.update_search();
                                }
                            }
                        });
                    });

                // Status bar pinned at bottom
                ui.separator();
                ui.horizontal(|ui| {
                    let dirty_indicator = if self.is_dirty() {
                        RichText::new("● Modified").color(Color32::from_rgb(251, 191, 36)).size(11.0).strong()
                    } else {
                        RichText::new("✔ Saved").color(Color32::from_rgb(52, 211, 153)).size(11.0)
                    };
                    ui.label(dirty_indicator);
                    ui.separator();
                    ui.label(RichText::new(format!("Lines: {}  |  Length: {} bytes", line_count, total_chars)).size(11.0).weak());
                    ui.separator();
                    ui.label(RichText::new(format!("Syntax: {}", self.syntax.name())).size(11.0).color(Color32::from_rgb(148, 163, 184)));

                    let tab_count = self.content.matches('\t').count();
                    if tab_count > 0 {
                        ui.separator();
                        if ui.button(
                            RichText::new(format!("⚠️ {} Tab(s) (Click to clean)", tab_count))
                                .color(Color32::from_rgb(251, 191, 36))
                                .size(11.0)
                                .strong(),
                        ).on_hover_text("Convert all tabs into 2 spaces").clicked() {
                            self.clean_tabs_to_spaces(2);
                        }
                    }

                    if let Some(ref p) = self.file_path {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let target_desc = if let Some(ref lbl) = self.remote_label {
                                format!("{}: {}", lbl, p.to_string_lossy())
                            } else {
                                p.to_string_lossy().to_string()
                            };
                            ui.label(RichText::new(target_desc).size(10.5).color(Color32::from_rgb(148, 163, 184)));
                        });
                    }
                });
            });

        if !window_open || request_close {
            if self.is_dirty() {
                self.show_unsaved_confirm = true;
            } else {
                self.open = false;
                action = EditorAction::Close;
            }
        }

        // Unsaved changes confirmation dialog
        if self.show_unsaved_confirm {
            let mut close_dialog = false;
            let mut confirm_exit = false;
            let mut confirm_save = false;

            egui::Window::new("⚠️ Unsaved Changes")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label("You have unsaved changes. Do you want to save before closing?");
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("💾 Save & Close").clicked() {
                            confirm_save = true;
                            close_dialog = true;
                        }
                        if ui.button("Discard Changes").clicked() {
                            confirm_exit = true;
                            close_dialog = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close_dialog = true;
                        }
                    });
                });

            if confirm_save {
                if let Some(ref path) = self.file_path {
                    action = EditorAction::Save {
                        path: path.clone(),
                        content: self.content.clone(),
                        remote_label: self.remote_label.clone(),
                    };
                }
                self.open = false;
            } else if confirm_exit {
                self.open = false;
                action = EditorAction::Close;
            }

            if close_dialog {
                self.show_unsaved_confirm = false;
            }
        }

        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_editor_open_and_dirty_tracking() {
        let mut editor = Editor::new();
        assert!(!editor.open);
        assert!(!editor.is_dirty());

        let path = PathBuf::from("/etc/nginx/nginx.conf");
        let initial_text = "server {\n    listen 80;\n}".to_string();
        editor.open_file(path.clone(), initial_text.clone(), Some("vps-prod".to_string()));

        assert!(editor.open);
        assert_eq!(editor.file_path, Some(path));
        assert_eq!(editor.remote_label.as_deref(), Some("vps-prod"));
        assert!(!editor.is_dirty(), "Newly opened file should not be dirty");

        // Modify content
        editor.content.push_str("\n    # comment");
        assert!(editor.is_dirty(), "Content was changed, should be dirty");

        // Mark as saved
        editor.mark_saved();
        assert!(!editor.is_dirty(), "After mark_saved, should not be dirty");
        assert!(editor.status.is_some());
    }

    #[test]
    fn test_editor_search_and_navigation() {
        let mut editor = Editor::new();
        editor.content = "apple orange banana apple kiwi apple".to_string();

        editor.search_query = "apple".to_string();
        editor.update_search();

        assert_eq!(editor.search_matches, 3);
        assert_eq!(editor.active_match_idx, 0);

        editor.next_match();
        assert_eq!(editor.active_match_idx, 1);

        editor.next_match();
        assert_eq!(editor.active_match_idx, 2);

        editor.next_match();
        assert_eq!(editor.active_match_idx, 0, "Should wrap around to 0");

        editor.prev_match();
        assert_eq!(editor.active_match_idx, 2, "Should wrap back to last match");
    }

    #[test]
    fn test_clean_tabs_to_spaces() {
        let mut editor = Editor::new();
        editor.content = "name:\n\tage: 30\n\t\tcity: Hanoi\n".to_string();

        let count = editor.clean_tabs_to_spaces(2);
        assert_eq!(count, 3);
        assert_eq!(editor.content, "name:\n  age: 30\n    city: Hanoi\n");
        assert!(!editor.content.contains('\t'));
        assert!(editor.status.is_some());

        // Second clean should find 0 tabs
        let count2 = editor.clean_tabs_to_spaces(2);
        assert_eq!(count2, 0);
    }
}

