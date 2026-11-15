use super::syntax::{SyntaxRules, SyntaxType};
use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontId};
use std::sync::Arc;

pub struct ThemeColors {
    pub default_text: Color32,
    pub keyword: Color32,
    pub type_builtin: Color32,
    pub string: Color32,
    pub number: Color32,
    pub comment: Color32,
    pub search_bg: Color32,
    pub active_search_bg: Color32,
}

impl Default for ThemeColors {
    fn default() -> Self {
        Self {
            default_text: Color32::from_rgb(226, 232, 240),      // Slate 200
            keyword: Color32::from_rgb(198, 120, 221),           // OneDark Purple
            type_builtin: Color32::from_rgb(86, 182, 194),       // Cyan / Teal
            string: Color32::from_rgb(152, 195, 121),             // Sage Green
            number: Color32::from_rgb(209, 154, 102),             // Soft Orange
            comment: Color32::from_rgb(100, 116, 139),           // Slate 500
            search_bg: Color32::from_rgba_unmultiplied(234, 179, 8, 80),  // Muted Amber
            active_search_bg: Color32::from_rgba_unmultiplied(245, 158, 11, 200), // Bright Amber
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddedLanguage {
    None,
    JavaScript,
    Css,
    Php,
    RubyErb,
}

pub fn create_layout_job(
    text: &str,
    syntax: SyntaxType,
    font_size: f32,
    search_query: &str,
    active_match_idx: usize,
    word_wrap: bool,
    wrap_width: f32,
) -> LayoutJob {
    let mut job = LayoutJob::default();
    if word_wrap {
        job.wrap.max_width = wrap_width;
    }

    let base_rules = syntax.rules();
    let theme = ThemeColors::default();
    let font_id = FontId::monospace(font_size);

    let trimmed_search = search_query.trim();
    let has_search = !trimmed_search.is_empty();

    // Collect search match byte ranges if search is active
    let search_ranges: Vec<(usize, usize)> = if has_search {
        let needle = if base_rules.case_insensitive {
            trimmed_search.to_lowercase()
        } else {
            trimmed_search.to_string()
        };
        let haystack = if base_rules.case_insensitive {
            text.to_lowercase()
        } else {
            text.to_string()
        };

        haystack
            .match_indices(&needle)
            .map(|(start, matched_str)| (start, start + matched_str.len()))
            .collect()
    } else {
        Vec::new()
    };

    let mut in_multiline_comment = false;
    let mut embedded_lang = EmbeddedLanguage::None;
    let supports_embedding = matches!(
        syntax,
        SyntaxType::HtmlXml | SyntaxType::Php | SyntaxType::Ruby
    );

    for line in text.split_inclusive('\n') {
        highlight_line(
            line,
            supports_embedding,
            &base_rules,
            &theme,
            &font_id,
            &mut in_multiline_comment,
            &mut embedded_lang,
            &mut job,
        );
    }

    // Apply search highlight backgrounds if any
    if !search_ranges.is_empty() {
        apply_search_highlights(&mut job, &search_ranges, active_match_idx, &theme);
    }

    job
}

fn starts_with_ci(chars: &[char], prefix: &str) -> bool {
    let p_chars: Vec<char> = prefix.chars().collect();
    if chars.len() < p_chars.len() {
        return false;
    }
    for (a, b) in chars[..p_chars.len()].iter().zip(p_chars.iter()) {
        if !a.eq_ignore_ascii_case(b) {
            return false;
        }
    }
    true
}

#[allow(clippy::too_many_arguments)]
fn highlight_line(
    line: &str,
    supports_embedding: bool,
    base_rules: &SyntaxRules,
    theme: &ThemeColors,
    font_id: &FontId,
    in_multiline_comment: &mut bool,
    embedded_lang: &mut EmbeddedLanguage,
    job: &mut LayoutJob,
) {
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut i = 0;

    let fmt = |color: Color32| TextFormat {
        font_id: font_id.clone(),
        color,
        background: Color32::TRANSPARENT,
        ..Default::default()
    };

    // Render subtle indent guides for leading spaces (at every 2-space step)
    if !*in_multiline_comment {
        while i < len && (chars[i] == ' ' || chars[i] == '\t') {
            let is_indent_step = i % 2 == 0;
            let bg = if is_indent_step {
                Color32::from_rgba_unmultiplied(148, 163, 184, 38) // Crisp subtle indent guide marker
            } else {
                Color32::TRANSPARENT
            };
            let mut format = fmt(theme.default_text);
            format.background = bg;
            if chars[i] == '\t' {
                job.append("\t", 0.0, format);
            } else {
                job.append(" ", 0.0, format);
            }
            i += 1;
        }
    }

    while i < len {
        // --- 0. Handle HTML embedded mode transitions (Script, Style, PHP, ERB) ---
        if supports_embedding {
            match *embedded_lang {
                EmbeddedLanguage::None => {
                    // Check <script ...>
                    if starts_with_ci(&chars[i..], "<script") {
                        let after = i + 7;
                        if after >= len || chars[after] == '>' || chars[after].is_whitespace() || chars[after] == '/' {
                            let start = i;
                            while i < len && chars[i] != '>' {
                                i += 1;
                            }
                            if i < len && chars[i] == '>' {
                                i += 1;
                            }
                            let seg: String = chars[start..i].iter().collect();
                            job.append(&seg, 0.0, fmt(theme.keyword));
                            *embedded_lang = EmbeddedLanguage::JavaScript;
                            continue;
                        }
                    }

                    // Check <style ...>
                    if starts_with_ci(&chars[i..], "<style") {
                        let after = i + 6;
                        if after >= len || chars[after] == '>' || chars[after].is_whitespace() || chars[after] == '/' {
                            let start = i;
                            while i < len && chars[i] != '>' {
                                i += 1;
                            }
                            if i < len && chars[i] == '>' {
                                i += 1;
                            }
                            let seg: String = chars[start..i].iter().collect();
                            job.append(&seg, 0.0, fmt(theme.keyword));
                            *embedded_lang = EmbeddedLanguage::Css;
                            continue;
                        }
                    }

                    // Check <?php or <?=
                    if starts_with_ci(&chars[i..], "<?php") {
                        let seg: String = chars[i..i + 5].iter().collect();
                        job.append(&seg, 0.0, fmt(theme.keyword));
                        i += 5;
                        *embedded_lang = EmbeddedLanguage::Php;
                        continue;
                    } else if starts_with_ci(&chars[i..], "<?=") {
                        let seg: String = chars[i..i + 3].iter().collect();
                        job.append(&seg, 0.0, fmt(theme.keyword));
                        i += 3;
                        *embedded_lang = EmbeddedLanguage::Php;
                        continue;
                    }

                    // Check <% ERB (e.g. <%, <%=, <%-, <%#)
                    if chars[i..].starts_with(&['<', '%']) {
                        let start = i;
                        i += 2;
                        if i < len && (chars[i] == '=' || chars[i] == '-' || chars[i] == '#') {
                            i += 1;
                        }
                        let seg: String = chars[start..i].iter().collect();
                        job.append(&seg, 0.0, fmt(theme.keyword));
                        *embedded_lang = EmbeddedLanguage::RubyErb;
                        continue;
                    }
                }

                EmbeddedLanguage::JavaScript => {
                    // Check </script>
                    if starts_with_ci(&chars[i..], "</script") {
                        let start = i;
                        while i < len && chars[i] != '>' {
                            i += 1;
                        }
                        if i < len && chars[i] == '>' {
                            i += 1;
                        }
                        let seg: String = chars[start..i].iter().collect();
                        job.append(&seg, 0.0, fmt(theme.keyword));
                        *embedded_lang = EmbeddedLanguage::None;
                        continue;
                    }
                }

                EmbeddedLanguage::Css => {
                    // Check </style>
                    if starts_with_ci(&chars[i..], "</style") {
                        let start = i;
                        while i < len && chars[i] != '>' {
                            i += 1;
                        }
                        if i < len && chars[i] == '>' {
                            i += 1;
                        }
                        let seg: String = chars[start..i].iter().collect();
                        job.append(&seg, 0.0, fmt(theme.keyword));
                        *embedded_lang = EmbeddedLanguage::None;
                        continue;
                    }
                }

                EmbeddedLanguage::Php => {
                    // Check ?>
                    if chars[i..].starts_with(&['?', '>']) {
                        let seg: String = chars[i..i + 2].iter().collect();
                        job.append(&seg, 0.0, fmt(theme.keyword));
                        i += 2;
                        *embedded_lang = EmbeddedLanguage::None;
                        continue;
                    }
                }

                EmbeddedLanguage::RubyErb => {
                    // Check -%> or %>
                    if chars[i..].starts_with(&['-', '%', '>']) {
                        let seg: String = chars[i..i + 3].iter().collect();
                        job.append(&seg, 0.0, fmt(theme.keyword));
                        i += 3;
                        *embedded_lang = EmbeddedLanguage::None;
                        continue;
                    } else if chars[i..].starts_with(&['%', '>']) {
                        let seg: String = chars[i..i + 2].iter().collect();
                        job.append(&seg, 0.0, fmt(theme.keyword));
                        i += 2;
                        *embedded_lang = EmbeddedLanguage::None;
                        continue;
                    }
                }
            }
        }

        // Active rules for current token
        let rules = match *embedded_lang {
            EmbeddedLanguage::JavaScript => SyntaxType::JavaScript.rules(),
            EmbeddedLanguage::Css => SyntaxType::Css.rules(),
            EmbeddedLanguage::Php => SyntaxType::Php.rules(),
            EmbeddedLanguage::RubyErb => SyntaxType::Ruby.rules(),
            EmbeddedLanguage::None => *base_rules,
        };

        let line_comment_chars: Option<Vec<char>> = rules.line_comment.map(|s| s.chars().collect());
        let multi_start_chars: Option<Vec<char>> = rules.multi_comment.map(|(s, _)| s.chars().collect());
        let multi_end_chars: Option<Vec<char>> = rules.multi_comment.map(|(_, e)| e.chars().collect());

        // 1. In existing multiline comment
        if *in_multiline_comment {
            if let Some(ref end_chars) = multi_end_chars {
                let start = i;
                let mut found_end = false;
                while i < len {
                    if i + end_chars.len() <= len && chars[i..i + end_chars.len()] == end_chars[..] {
                        i += end_chars.len();
                        *in_multiline_comment = false;
                        found_end = true;
                        break;
                    }
                    i += 1;
                }
                let seg: String = chars[start..i].iter().collect();
                job.append(&seg, 0.0, fmt(theme.comment));
                if !found_end {
                    return; // line ends inside multiline comment
                }
                continue;
            } else {
                *in_multiline_comment = false;
            }
        }

        // 2. Start of multiline comment
        if let Some(ref start_chars) = multi_start_chars
            && i + start_chars.len() <= len && chars[i..i + start_chars.len()] == start_chars[..] {
                let start = i;
                i += start_chars.len();
                *in_multiline_comment = true;

                // Check if end of multiline comment is on the same line
                if let Some(ref end_chars) = multi_end_chars {
                    while i < len {
                        if i + end_chars.len() <= len && chars[i..i + end_chars.len()] == end_chars[..] {
                            i += end_chars.len();
                            *in_multiline_comment = false;
                            break;
                        }
                        i += 1;
                    }
                }
                let seg: String = chars[start..i].iter().collect();
                job.append(&seg, 0.0, fmt(theme.comment));
                continue;
            }

        // 3. Single-line comment
        if let Some(ref prefix_chars) = line_comment_chars
            && i + prefix_chars.len() <= len && chars[i..i + prefix_chars.len()] == prefix_chars[..] {
                // If in ERB, stop before %>
                if *embedded_lang == EmbeddedLanguage::RubyErb {
                    let start = i;
                    while i < len && !chars[i..].starts_with(&['%', '>']) && !chars[i..].starts_with(&['-', '%', '>']) {
                        i += 1;
                    }
                    let seg: String = chars[start..i].iter().collect();
                    job.append(&seg, 0.0, fmt(theme.comment));
                    continue;
                } else {
                    let seg: String = chars[i..].iter().collect();
                    job.append(&seg, 0.0, fmt(theme.comment));
                    return;
                }
            }

        // 4. Strings: "..." or '...' or `...`
        if rules.highlight_strings && (chars[i] == '"' || chars[i] == '\'' || chars[i] == '`') {
            let quote = chars[i];
            let start = i;
            i += 1;
            while i < len {
                if chars[i] == '\\' && i + 1 < len {
                    i += 2; // skip escaped character
                    continue;
                }
                if chars[i] == quote {
                    i += 1;
                    break;
                }
                i += 1;
            }
            let seg: String = chars[start..i].iter().collect();
            job.append(&seg, 0.0, fmt(theme.string));
            continue;
        }

        // 5. Numbers
        if rules.highlight_numbers && chars[i].is_ascii_digit() {
            let start = i;
            let is_hex = i + 1 < len && chars[i] == '0' && (chars[i + 1] == 'x' || chars[i + 1] == 'X');
            if is_hex {
                i += 2;
                while i < len && chars[i].is_ascii_hexdigit() {
                    i += 1;
                }
            } else {
                while i < len && (chars[i].is_ascii_digit() || chars[i] == '.' || chars[i] == '_') {
                    i += 1;
                }
            }
            let seg: String = chars[start..i].iter().collect();
            job.append(&seg, 0.0, fmt(theme.number));
            continue;
        }

        // 6. Identifiers (keywords, types, variables)
        if chars[i].is_alphabetic() || chars[i] == '_' {
            let start = i;
            while i < len && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            let check_word = if rules.case_insensitive {
                word.to_lowercase()
            } else {
                word.clone()
            };

            if rules.keywords.iter().any(|&k| k == check_word) {
                job.append(&word, 0.0, fmt(theme.keyword));
            } else if rules.types_builtins.iter().any(|&t| t == check_word) {
                job.append(&word, 0.0, fmt(theme.type_builtin));
            } else {
                job.append(&word, 0.0, fmt(theme.default_text));
            }
            continue;
        }

        // 7. Fallback: Any other characters (operators, symbols, whitespaces, punctuation)
        // Advance `i` by at least 1 character to guarantee no infinite loops!
        let start = i;
        i += 1;
        while i < len {
            // Stop if an embedded transition or delimiter starts
            if supports_embedding {
                if *embedded_lang == EmbeddedLanguage::None {
                    if starts_with_ci(&chars[i..], "<script")
                        || starts_with_ci(&chars[i..], "<style")
                        || starts_with_ci(&chars[i..], "<?php")
                        || starts_with_ci(&chars[i..], "<?=")
                        || chars[i..].starts_with(&['<', '%'])
                    {
                        break;
                    }
                } else if *embedded_lang == EmbeddedLanguage::JavaScript {
                    if starts_with_ci(&chars[i..], "</script") {
                        break;
                    }
                } else if *embedded_lang == EmbeddedLanguage::Css {
                    if starts_with_ci(&chars[i..], "</style") {
                        break;
                    }
                } else if *embedded_lang == EmbeddedLanguage::Php {
                    if chars[i..].starts_with(&['?', '>']) {
                        break;
                    }
                } else if *embedded_lang == EmbeddedLanguage::RubyErb
                    && (chars[i..].starts_with(&['%', '>']) || chars[i..].starts_with(&['-', '%', '>'])) {
                        break;
                    }
            }

            // Stop if an identifier starts
            if chars[i].is_alphabetic() || chars[i] == '_' {
                break;
            }
            // Stop if a number starts
            if rules.highlight_numbers && chars[i].is_ascii_digit() {
                break;
            }
            // Stop if a string starts
            if rules.highlight_strings && (chars[i] == '"' || chars[i] == '\'' || chars[i] == '`') {
                break;
            }
            // Stop if a single line comment starts
            if let Some(ref prefix_chars) = line_comment_chars
                && i + prefix_chars.len() <= len && chars[i..i + prefix_chars.len()] == prefix_chars[..] {
                    break;
                }
            // Stop if a multiline comment starts
            if let Some(ref start_chars) = multi_start_chars
                && i + start_chars.len() <= len && chars[i..i + start_chars.len()] == start_chars[..] {
                    break;
                }
            i += 1;
        }
        let seg: String = chars[start..i].iter().collect();
        job.append(&seg, 0.0, fmt(theme.default_text));
    }
}

fn apply_search_highlights(
    job: &mut LayoutJob,
    search_ranges: &[(usize, usize)],
    active_idx: usize,
    theme: &ThemeColors,
) {
    if search_ranges.is_empty() {
        return;
    }

    for (match_i, &(m_start, m_end)) in search_ranges.iter().enumerate() {
        let is_active = match_i == active_idx;
        let bg_color = if is_active {
            theme.active_search_bg
        } else {
            theme.search_bg
        };

        for section in &mut job.sections {
            let s_start = section.byte_range.start;
            let s_end = section.byte_range.end;

            if s_start < m_end && s_end > m_start {
                section.format.background = bg_color;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn render_editor_layouter(
    ctx: &egui::Context,
    text: &str,
    syntax: SyntaxType,
    font_size: f32,
    search_query: &str,
    active_match_idx: usize,
    word_wrap: bool,
    wrap_width: f32,
) -> Arc<egui::Galley> {
    let job = create_layout_job(
        text,
        syntax,
        font_size,
        search_query,
        active_match_idx,
        word_wrap,
        wrap_width,
    );
    ctx.fonts(|f| f.layout_job(job))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_highlight_plain_text_and_quotes_no_infinite_loop() {
        let test_texts = [
            "source \"https://rubygems.org\"\ngem 'rails'\n",
            "# Markdown header\nSome \"quoted text\" and `backticks` and [links](url)\n",
            "<% @users.each do |user| %>\n  <p><%= user.name %></p>\n<% end %>\n",
            "a = 1 + 2 * (3 / 4) !@#$%^&*()_+-=[]{}|;':\",./<>?",
            "",
            "\"unclosed string",
            "/* unclosed comment",
        ];

        for text in &test_texts {
            for syntax in &[SyntaxType::PlainText, SyntaxType::Markdown, SyntaxType::HtmlXml, SyntaxType::Ruby] {
                let job = create_layout_job(text, *syntax, 13.0, "", 0, false, 800.0);
                assert!(!job.sections.is_empty() || text.is_empty());
            }
        }
    }

    #[test]
    fn test_embedded_html_scripts_and_styles() {
        let html_content = r#"
<!DOCTYPE html>
<html>
<head>
    <style>
        body { display: flex; margin: 0; }
    </style>
</head>
<body>
    <div class="container">
        <% @items.each do |item| %>
            <p><%= item.title %></p>
        <% end %>
        <?php echo "Hello from PHP"; ?>
    </div>
    <script>
        const app = "RustyBox";
        function init() {
            console.log(app);
        }
    </script>
</body>
</html>
"#;

        let job = create_layout_job(html_content, SyntaxType::HtmlXml, 13.0, "", 0, false, 800.0);
        assert!(!job.sections.is_empty());

        // Verify sections contain keywords from embedded JS, CSS, and ERB
        let has_js_keyword = job.sections.iter().any(|s| {
            let text = &html_content[s.byte_range.start..s.byte_range.end];
            text == "const" || text == "function"
        });
        assert!(has_js_keyword, "Should highlight JS 'const' or 'function' inside <script>");

        let has_css_keyword = job.sections.iter().any(|s| {
            let text = &html_content[s.byte_range.start..s.byte_range.end];
            text == "flex"
        });
        assert!(has_css_keyword, "Should highlight CSS 'flex' inside <style>");

        let has_erb_keyword = job.sections.iter().any(|s| {
            let text = &html_content[s.byte_range.start..s.byte_range.end];
            text == "do" || text == "end"
        });
        assert!(has_erb_keyword, "Should highlight ERB 'do' or 'end' inside <% %>");
    }
}
