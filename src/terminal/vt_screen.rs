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
        let trimmed = name.trim();
        for t in Self::all() {
            if t.name().eq_ignore_ascii_case(trimmed) {
                return Some(*t);
            }
        }
        match trimmed.to_ascii_lowercase().as_str() {
            "rustyboxdark" | "rustybox dark" | "default" => Some(TerminalTheme::RustyBoxDark),
            "dracula pro" | "dracula" => Some(TerminalTheme::Dracula),
            "solarizeddark" | "solarized dark" => Some(TerminalTheme::SolarizedDark),
            "solarizedlight" | "solarized light" => Some(TerminalTheme::SolarizedLight),
            "monokai" | "monokai classic" => Some(TerminalTheme::Monokai),
            "nord" | "nord arctic" => Some(TerminalTheme::Nord),
            "matrix" | "matrix green" => Some(TerminalTheme::Matrix),
            _ => None,
        }
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

pub fn ansi_256_to_color32(idx: u8, theme: TerminalTheme) -> Color32 {
    if idx < 16 {
        return match theme {
            TerminalTheme::Dracula => match idx {
                0 => Color32::from_rgb(33, 34, 44),     // Black
                1 => Color32::from_rgb(255, 85, 85),   // Red (#FF5555)
                2 => Color32::from_rgb(80, 250, 123),  // Green (#50FA7B)
                3 => Color32::from_rgb(241, 250, 140), // Yellow (#F1FA8C)
                4 => Color32::from_rgb(189, 147, 249), // Purple/Blue (#BD93F9)
                5 => Color32::from_rgb(255, 121, 198), // Pink/Magenta (#FF79C6)
                6 => Color32::from_rgb(139, 233, 253), // Cyan (#8BE9FD)
                7 => Color32::from_rgb(248, 248, 242), // White (#F8F8F2)
                8 => Color32::from_rgb(98, 114, 164),   // Bright Black / Comment (#6272A4)
                9 => Color32::from_rgb(255, 110, 110), // Bright Red
                10 => Color32::from_rgb(105, 255, 148), // Bright Green
                11 => Color32::from_rgb(255, 255, 165), // Bright Yellow
                12 => Color32::from_rgb(214, 172, 255), // Bright Purple
                13 => Color32::from_rgb(255, 146, 223), // Bright Pink
                14 => Color32::from_rgb(164, 255, 255), // Bright Cyan
                15 => Color32::from_rgb(255, 255, 255), // Bright White
                _ => unreachable!(),
            },
            TerminalTheme::Nord => match idx {
                0 => Color32::from_rgb(46, 52, 64),
                1 => Color32::from_rgb(191, 97, 106),
                2 => Color32::from_rgb(163, 190, 140),
                3 => Color32::from_rgb(235, 203, 139),
                4 => Color32::from_rgb(129, 161, 193),
                5 => Color32::from_rgb(180, 142, 173),
                6 => Color32::from_rgb(136, 192, 208),
                7 => Color32::from_rgb(229, 233, 240),
                8 => Color32::from_rgb(76, 86, 106),
                9 => Color32::from_rgb(191, 97, 106),
                10 => Color32::from_rgb(163, 190, 140),
                11 => Color32::from_rgb(235, 203, 139),
                12 => Color32::from_rgb(129, 161, 193),
                13 => Color32::from_rgb(180, 142, 173),
                14 => Color32::from_rgb(143, 188, 187),
                15 => Color32::from_rgb(236, 239, 244),
                _ => unreachable!(),
            },
            TerminalTheme::SolarizedDark | TerminalTheme::SolarizedLight => match idx {
                0 => Color32::from_rgb(7, 54, 66),
                1 => Color32::from_rgb(220, 50, 47),
                2 => Color32::from_rgb(133, 153, 0),
                3 => Color32::from_rgb(181, 137, 0),
                4 => Color32::from_rgb(38, 139, 210),
                5 => Color32::from_rgb(211, 54, 130),
                6 => Color32::from_rgb(42, 161, 152),
                7 => Color32::from_rgb(238, 232, 213),
                8 => Color32::from_rgb(0, 43, 54),
                9 => Color32::from_rgb(203, 75, 22),
                10 => Color32::from_rgb(88, 110, 117),
                11 => Color32::from_rgb(101, 123, 131),
                12 => Color32::from_rgb(131, 148, 150),
                13 => Color32::from_rgb(108, 113, 196),
                14 => Color32::from_rgb(147, 161, 161),
                15 => Color32::from_rgb(253, 246, 227),
                _ => unreachable!(),
            },
            TerminalTheme::Monokai => match idx {
                0 => Color32::from_rgb(39, 40, 34),
                1 => Color32::from_rgb(249, 38, 114),
                2 => Color32::from_rgb(166, 226, 46),
                3 => Color32::from_rgb(244, 191, 117),
                4 => Color32::from_rgb(102, 217, 239),
                5 => Color32::from_rgb(174, 129, 255),
                6 => Color32::from_rgb(161, 239, 228),
                7 => Color32::from_rgb(248, 248, 242),
                8 => Color32::from_rgb(117, 113, 94),
                9 => Color32::from_rgb(249, 38, 114),
                10 => Color32::from_rgb(166, 226, 46),
                11 => Color32::from_rgb(244, 191, 117),
                12 => Color32::from_rgb(102, 217, 239),
                13 => Color32::from_rgb(174, 129, 255),
                14 => Color32::from_rgb(161, 239, 228),
                15 => Color32::from_rgb(249, 249, 244),
                _ => unreachable!(),
            },
            TerminalTheme::Matrix => match idx {
                0 => Color32::from_rgb(5, 12, 5),
                1 => Color32::from_rgb(30, 180, 30),
                2 => Color32::from_rgb(80, 255, 80),
                3 => Color32::from_rgb(150, 255, 150),
                4 => Color32::from_rgb(20, 150, 20),
                5 => Color32::from_rgb(40, 200, 40),
                6 => Color32::from_rgb(100, 255, 100),
                7 => Color32::from_rgb(200, 255, 200),
                8 => Color32::from_rgb(15, 60, 15),
                9 => Color32::from_rgb(40, 200, 40),
                10 => Color32::from_rgb(120, 255, 120),
                11 => Color32::from_rgb(180, 255, 180),
                12 => Color32::from_rgb(30, 170, 30),
                13 => Color32::from_rgb(60, 220, 60),
                14 => Color32::from_rgb(140, 255, 140),
                15 => Color32::from_rgb(240, 255, 240),
                _ => unreachable!(),
            },
            TerminalTheme::RustyBoxDark => match idx {
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
                _ => unreachable!(),
            },
        };
    }
    match idx {
        0..=15 => unreachable!(),
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

/// Adjusts a terminal foreground color for VT100 `bold` (brightened) and `dim` (faint) attributes.
pub fn apply_bold_dim(color: Color32, bold: bool, dim: bool) -> Color32 {
    if bold && !dim {
        Color32::from_rgba_unmultiplied(
            color.r().saturating_add(36),
            color.g().saturating_add(36),
            color.b().saturating_add(36),
            color.a(),
        )
    } else if dim && !bold {
        Color32::from_rgba_unmultiplied(
            ((color.r() as u16 * 2) / 3) as u8,
            ((color.g() as u16 * 2) / 3) as u8,
            ((color.b() as u16 * 2) / 3) as u8,
            color.a(),
        )
    } else {
        color
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_theme_from_name_aliases() {
        assert_eq!(
            TerminalTheme::from_name("RustyBox Dark (Default)"),
            Some(TerminalTheme::RustyBoxDark)
        );
        assert_eq!(
            TerminalTheme::from_name("Dracula Pro"),
            Some(TerminalTheme::Dracula)
        );
        assert_eq!(
            TerminalTheme::from_name("solarized dark"),
            Some(TerminalTheme::SolarizedDark)
        );
        assert_eq!(
            TerminalTheme::from_name("Matrix Green"),
            Some(TerminalTheme::Matrix)
        );
    }

    #[test]
    fn test_apply_bold_dim_color_adjustments() {
        let base = Color32::from_rgb(120, 150, 180);
        let bold_c = apply_bold_dim(base, true, false);
        assert!(bold_c.r() > base.r() && bold_c.g() > base.g() && bold_c.b() > base.b());

        let dim_c = apply_bold_dim(base, false, true);
        assert!(dim_c.r() < base.r() && dim_c.g() < base.g() && dim_c.b() < base.b());

        assert_eq!(apply_bold_dim(base, false, false), base);
    }

    #[test]
    fn test_dracula_theme_colors() {
        let dracula = TerminalTheme::Dracula;
        assert_eq!(dracula.default_bg(), Color32::from_rgb(40, 42, 54));
        assert_eq!(dracula.default_fg(), Color32::from_rgb(248, 248, 242));
        assert_eq!(ansi_256_to_color32(1, dracula), Color32::from_rgb(255, 85, 85));
        assert_eq!(ansi_256_to_color32(2, dracula), Color32::from_rgb(80, 250, 123));
        assert_eq!(ansi_256_to_color32(4, dracula), Color32::from_rgb(189, 147, 249));
    }

    #[test]
    fn test_terminal_theme_new_aliases() {
        assert_eq!(TerminalTheme::from_name("Monokai Classic"), Some(TerminalTheme::Monokai));
        assert_eq!(TerminalTheme::from_name("Nord Arctic"), Some(TerminalTheme::Nord));
        assert_eq!(TerminalTheme::from_name("dracula"), Some(TerminalTheme::Dracula));
    }
}
