use egui::Color32;
use vt100::Color;

pub fn vt_to_egui_color(color: Color, is_fg: bool) -> Color32 {
    match color {
        Color::Default => {
            if is_fg {
                Color32::from_rgb(220, 220, 225) // Bright light gray
            } else {
                Color32::from_rgb(18, 20, 24) // Deep dark terminal background
            }
        }
        Color::Idx(idx) => ansi_256_to_color32(idx),
        Color::Rgb(r, g, b) => Color32::from_rgb(r, g, b),
    }
}

pub fn ansi_256_to_color32(idx: u8) -> Color32 {
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
