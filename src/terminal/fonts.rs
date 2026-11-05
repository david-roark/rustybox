use std::sync::Arc;

pub const FONT_NOTO_SANS_MONO: &str = "noto_sans_mono";
pub const FONT_UBUNTU_MONO: &str = "ubuntu_mono";
pub const FONT_DEJAVU_SANS_MONO: &str = "dejavu_sans_mono";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FontOption {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
}

pub const AVAILABLE_TERMINAL_FONTS: &[FontOption] = &[
    FontOption {
        id: FONT_NOTO_SANS_MONO,
        name: "Noto Sans Mono",
        description: "Full Unicode & Vietnamese diacritics coverage (Default)",
    },
    FontOption {
        id: FONT_UBUNTU_MONO,
        name: "Ubuntu Mono",
        description: "Crisp and compact monospace font",
    },
    FontOption {
        id: FONT_DEJAVU_SANS_MONO,
        name: "DejaVu Sans Mono",
        description: "Classic Linux terminal monospace font",
    },
];

pub fn terminal_font_display_name(id: &str) -> &'static str {
    match id {
        FONT_UBUNTU_MONO | "Ubuntu Mono" => "Ubuntu Mono",
        FONT_DEJAVU_SANS_MONO | "DejaVu Sans Mono" => "DejaVu Sans Mono",
        FONT_NOTO_SANS_MONO | "Noto Sans Mono" => "Noto Sans Mono",
        "" => "Default (Global Setting)",
        _ => "Noto Sans Mono",
    }
}

pub fn get_terminal_font_family(font_id: &str) -> egui::FontFamily {
    match font_id {
        FONT_UBUNTU_MONO | "Ubuntu Mono" => egui::FontFamily::Name(Arc::from("Ubuntu Mono")),
        FONT_DEJAVU_SANS_MONO | "DejaVu Sans Mono" => egui::FontFamily::Name(Arc::from("DejaVu Sans Mono")),
        FONT_NOTO_SANS_MONO | "Noto Sans Mono" => egui::FontFamily::Name(Arc::from("Noto Sans Mono")),
        _ => egui::FontFamily::Monospace,
    }
}

pub fn build_font_definitions(preferred_mono_font: &str) -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "noto_sans_mono".to_owned(),
        Arc::new(egui::FontData::from_static(include_bytes!("../../assets/fonts/NotoSansMono.ttf"))),
    );
    fonts.font_data.insert(
        "ubuntu_mono".to_owned(),
        Arc::new(egui::FontData::from_static(include_bytes!("../../assets/fonts/UbuntuMono.ttf"))),
    );
    fonts.font_data.insert(
        "dejavu_sans_mono".to_owned(),
        Arc::new(egui::FontData::from_static(include_bytes!("../../assets/fonts/DejaVuSansMono.ttf"))),
    );
    fonts.font_data.insert(
        "dejavu_sans".to_owned(),
        Arc::new(egui::FontData::from_static(include_bytes!("../../assets/fonts/DejaVuSans.ttf"))),
    );

    // Register named families so individual sessions can specify their exact font
    fonts.families.insert(
        egui::FontFamily::Name(Arc::from("Noto Sans Mono")),
        vec![
            "noto_sans_mono".to_owned(),
            "ubuntu_mono".to_owned(),
            "dejavu_sans_mono".to_owned(),
        ],
    );
    fonts.families.insert(
        egui::FontFamily::Name(Arc::from("Ubuntu Mono")),
        vec![
            "ubuntu_mono".to_owned(),
            "noto_sans_mono".to_owned(),
            "dejavu_sans_mono".to_owned(),
        ],
    );
    fonts.families.insert(
        egui::FontFamily::Name(Arc::from("DejaVu Sans Mono")),
        vec![
            "dejavu_sans_mono".to_owned(),
            "noto_sans_mono".to_owned(),
            "ubuntu_mono".to_owned(),
        ],
    );

    // Order egui's default Monospace family based on preferred_mono_font
    let mono = fonts.families.entry(egui::FontFamily::Monospace).or_default();
    let primary = match preferred_mono_font {
        FONT_UBUNTU_MONO | "Ubuntu Mono" => "ubuntu_mono",
        FONT_DEJAVU_SANS_MONO | "DejaVu Sans Mono" => "dejavu_sans_mono",
        _ => "noto_sans_mono",
    };

    mono.insert(0, "dejavu_sans_mono".to_owned());
    mono.insert(0, "ubuntu_mono".to_owned());
    mono.insert(0, "noto_sans_mono".to_owned());

    if let Some(pos) = mono.iter().position(|x| x == primary) {
        let item = mono.remove(pos);
        mono.insert(0, item);
    }

    // Put DejaVu Sans at top of Proportional family for crisp UI text
    let prop = fonts.families.entry(egui::FontFamily::Proportional).or_default();
    prop.insert(0, "dejavu_sans".to_owned());

    fonts
}

pub fn setup_terminal_fonts(ctx: &egui::Context, preferred_mono_font: &str) {
    let font_defs = build_font_definitions(preferred_mono_font);
    ctx.set_fonts(font_defs);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_display_names_and_fallbacks() {
        assert_eq!(terminal_font_display_name(FONT_NOTO_SANS_MONO), "Noto Sans Mono");
        assert_eq!(terminal_font_display_name(FONT_UBUNTU_MONO), "Ubuntu Mono");
        assert_eq!(terminal_font_display_name(FONT_DEJAVU_SANS_MONO), "DejaVu Sans Mono");
        assert_eq!(terminal_font_display_name(""), "Default (Global Setting)");
        assert_eq!(terminal_font_display_name("unknown_font"), "Noto Sans Mono");
    }

    #[test]
    fn test_get_terminal_font_family() {
        assert_eq!(
            get_terminal_font_family("ubuntu_mono"),
            egui::FontFamily::Name(Arc::from("Ubuntu Mono"))
        );
        assert_eq!(
            get_terminal_font_family("dejavu_sans_mono"),
            egui::FontFamily::Name(Arc::from("DejaVu Sans Mono"))
        );
        assert_eq!(
            get_terminal_font_family("noto_sans_mono"),
            egui::FontFamily::Name(Arc::from("Noto Sans Mono"))
        );
        assert_eq!(
            get_terminal_font_family(""),
            egui::FontFamily::Monospace
        );
    }

    #[test]
    fn test_build_font_definitions_primary_ordering() {
        let defs_noto = build_font_definitions("noto_sans_mono");
        let mono_noto = &defs_noto.families[&egui::FontFamily::Monospace];
        assert_eq!(mono_noto[0], "noto_sans_mono");

        let defs_ubuntu = build_font_definitions("ubuntu_mono");
        let mono_ubuntu = &defs_ubuntu.families[&egui::FontFamily::Monospace];
        assert_eq!(mono_ubuntu[0], "ubuntu_mono");

        let defs_dejavu = build_font_definitions("dejavu_sans_mono");
        let mono_dejavu = &defs_dejavu.families[&egui::FontFamily::Monospace];
        assert_eq!(mono_dejavu[0], "dejavu_sans_mono");
    }
}
