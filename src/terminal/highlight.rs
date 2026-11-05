use serde::{Deserialize, Serialize};

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HighlightRule {
    pub id: String,
    pub keyword: String,
    pub color_rgb: [u8; 3],
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub whole_word: bool,
    #[serde(default)]
    pub match_case: bool,
}

impl HighlightRule {
    pub fn new(keyword: &str, r: u8, g: u8, b: u8) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            keyword: keyword.to_string(),
            color_rgb: [r, g, b],
            enabled: true,
            whole_word: true,
            match_case: false,
        }
    }
}

pub fn default_highlight_rules() -> Vec<HighlightRule> {
    vec![
        // Error / Critical - Red
        HighlightRule::new("error", 255, 90, 90),
        HighlightRule::new("failed", 255, 90, 90),
        HighlightRule::new("fatal", 255, 90, 90),
        HighlightRule::new("exception", 255, 90, 90),
        HighlightRule::new("critical", 255, 90, 90),
        HighlightRule::new("panic", 255, 90, 90),
        HighlightRule::new("err", 255, 90, 90),

        // Warning - Orange / Yellow
        HighlightRule::new("warn", 255, 210, 60),
        HighlightRule::new("warning", 255, 210, 60),

        // Success - Green
        HighlightRule::new("success", 80, 230, 120),
        HighlightRule::new("ok", 80, 230, 120),
        HighlightRule::new("passed", 80, 230, 120),
        HighlightRule::new("running", 80, 230, 120),
        HighlightRule::new("active", 80, 230, 120),

        // Notice / Info - Cyan
        HighlightRule::new("notice", 80, 210, 245),
        HighlightRule::new("info", 80, 210, 245),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_rules_coverage() {
        let rules = default_highlight_rules();
        assert!(rules.iter().any(|r| r.keyword == "error"));
        assert!(rules.iter().any(|r| r.keyword == "warn"));
        assert!(rules.iter().any(|r| r.keyword == "success"));
    }
}
