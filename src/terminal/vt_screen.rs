use egui::Color32;
use vt100::Color;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminalTheme {
    RustyBoxDark,
    Dracula,
    SolarizedDark,
    SolarizedLight,
    Monokai,
    Nord,
    Matrix,
}

impl TerminalTheme {
    pub fn all() -> &'static [TerminalTheme] {
        &[
            TerminalTheme::RustyBoxDark,
            TerminalTheme::Dracula,
            TerminalTheme::SolarizedDark,
            TerminalTheme::SolarizedLight,
            TerminalTheme::Monokai,
            TerminalTheme::Nord,
            TerminalTheme::Matrix,
        ]
    }

    pub fn name(&self) -> &'static str {
        match self {
            TerminalTheme::RustyBoxDark => "RustyBox Dark (Default)",
            TerminalTheme::Dracula => "Dracula",
            TerminalTheme::SolarizedDark => "Solarized Dark",
            TerminalTheme::SolarizedLight => "Solarized Light",
            TerminalTheme::Monokai => "Monokai",
            TerminalTheme::Nord => "Nord",
            TerminalTheme::Matrix => "Matrix Green",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        for t in Self::all() {
            if t.name() == name {
                return Some(*t);
            }
        }
        None
    }

    pub fn default_bg(&self) -> Color32 {
        match self {
            TerminalTheme::RustyBoxDark => Color32::from_rgb(16, 18, 22),
            TerminalTheme::Dracula => Color32::from_rgb(40, 42, 54),
            TerminalTheme::SolarizedDark => Color32::from_rgb(0, 43, 54),
            TerminalTheme::SolarizedLight => Color32::from_rgb(253, 246, 227),
            TerminalTheme::Monokai => Color32::from_rgb(39, 40, 34),
            TerminalTheme::Nord => Color32::from_rgb(46, 52, 64),
            TerminalTheme::Matrix => Color32::from_rgb(5, 12, 5),
        }
    }

    pub fn default_fg(&self) -> Color32 {
        match self {
            TerminalTheme::RustyBoxDark => Color32::from_rgb(220, 220, 225),
            TerminalTheme::Dracula => Color32::from_rgb(248, 248, 242),
            TerminalTheme::SolarizedDark => Color32::from_rgb(131, 148, 150),
            TerminalTheme::SolarizedLight => Color32::from_rgb(101, 123, 131),
            TerminalTheme::Monokai => Color32::from_rgb(248, 248, 240),
            TerminalTheme::Nord => Color32::from_rgb(216, 222, 233),
            TerminalTheme::Matrix => Color32::from_rgb(80, 255, 80),
        }
    }
}

pub fn vt_to_egui_color(color: Color, is_fg: bool, theme: TerminalTheme) -> Color32 {
    match color {
        Color::Default => {
            if is_fg {
                theme.default_fg()
            } else {
                theme.default_bg()
            }
        }
        Color::Idx(idx) => ansi_256_to_color32(idx, theme),
        Color::Rgb(r, g, b) => Color32::from_rgb(r, g, b),
    }
}

pub fn ansi_256_to_color32(idx: u8, _theme: TerminalTheme) -> Color32 {
    // Standard 16 ANSI colors
    match idx {
        0 => Color32::from_rgb(0, 0, 0),        // Black
        1 => Color32::from_rgb(205, 49, 49),    // Red
        2 => Color32::from_rgb(13, 188, 121),   // Green
        3 => Color32::from_rgb(229, 229, 16),   // Yellow
        4 => Color32::from_rgb(36, 114, 200),   // Blue
        5 => Color32::from_rgb(188, 63, 188),   // Magenta
        6 => Color32::from_rgb(17, 168, 205),   // Cyan
        7 => Color32::from_rgb(229, 229, 229),  // White
        8 => Color32::from_rgb(102, 102, 102),  // Bright Black / Gray
        9 => Color32::from_rgb(241, 76, 76),    // Bright Red
        10 => Color32::from_rgb(35, 209, 139),  // Bright Green
        11 => Color32::from_rgb(245, 245, 67),  // Bright Yellow
        12 => Color32::from_rgb(59, 142, 234),  // Bright Blue
        13 => Color32::from_rgb(214, 112, 214), // Bright Magenta
        14 => Color32::from_rgb(41, 184, 219),  // Bright Cyan
        15 => Color32::from_rgb(255, 255, 255), // Bright White
        16..=231 => {
            // 6x6x6 color cube
            let n = idx - 16;
            let r = (n / 36) % 6;
            let g = (n / 6) % 6;
            let b = n % 6;
            let step = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
            Color32::from_rgb(step(r), step(g), step(b))
        }
        232..=255 => {
            // 24 grayscale levels
            let gray = 8 + (idx - 232) * 10;
            Color32::from_rgb(gray, gray, gray)
        }
    }
}
