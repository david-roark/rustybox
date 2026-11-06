#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod network_tools;
mod session;
mod sftp;
mod terminal;
mod tunneling;
mod snippets;

use app::RustyBoxApp;
use eframe::egui::ViewportBuilder;

#[cfg(target_os = "linux")]
fn ensure_linux_desktop_integration() {
    let Ok(exe_path) = std::env::current_exe() else { return; };
    let Some(data_dir) = dirs::data_local_dir() else { return; };

    // 1. Install standard 8-bit RGBA icon with an absolute path for instant desktop loading
    let icon_dir = data_dir.join("icons");
    let icon_file = icon_dir.join("rustybox.png");
    if std::fs::create_dir_all(&icon_dir).is_ok() {
        let png_bytes = include_bytes!("../assets/icon.png");
        let _ = std::fs::write(&icon_file, png_bytes);
    }

    // Also mirror to hicolor standard theme directory
    let hicolor_dir = data_dir.join("icons/hicolor/512x512/apps");
    if std::fs::create_dir_all(&hicolor_dir).is_ok() {
        let png_bytes = include_bytes!("../assets/icon.png");
        let _ = std::fs::write(hicolor_dir.join("rustybox.png"), png_bytes);
    }

    // 2. Clean up any redundant desktop files to prevent duplicate entries in app launcher
    let apps_dir = data_dir.join("applications");
    let duplicate_desktop = apps_dir.join("rust-remote-toolbox.desktop");
    if duplicate_desktop.exists() {
        let _ = std::fs::remove_file(duplicate_desktop);
    }

    // 3. Register the single official desktop entry
    let desktop_file = apps_dir.join("rustybox.desktop");
    if std::fs::create_dir_all(&apps_dir).is_ok() {
        let exe_str = exe_path.to_string_lossy();
        let icon_str = icon_file.to_string_lossy();
        let exe_name = exe_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("rustybox");

        let content = format!(
            "[Desktop Entry]\n\
            Version=1.0\n\
            Type=Application\n\
            Name=RustyBox\n\
            Comment=Remote Toolbox & Terminal\n\
            Exec=\"{}\" %U\n\
            Icon={}\n\
            Terminal=false\n\
            Categories=Utility;Network;\n\
            StartupWMClass={}\n",
            exe_str, icon_str, exe_name
        );
        let _ = std::fs::write(&desktop_file, content);
    }
}

fn main() -> eframe::Result<()> {
    // If invoked as an SSH_ASKPASS helper by OpenSSH, output the password and exit immediately
    if let Ok(pwd) = std::env::var("RUSTYBOX_SSH_PASS") {
        use std::io::Write;
        let mut out = std::io::stdout();
        let _ = out.write_all(pwd.trim().as_bytes());
        let _ = out.write_all(b"\n");
        let _ = out.flush();
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    ensure_linux_desktop_integration();

    let icon_bytes = include_bytes!("../assets/icon_rgba_128.bin");
    let app_icon = egui::IconData {
        rgba: icon_bytes.to_vec(),
        width: 128,
        height: 128,
    };

    let native_options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_app_id("rustybox")
            .with_title("RustyBox v1.3-85 - Remote Toolbox & Terminal")
            .with_icon(app_icon)
            .with_maximized(true)
            .with_min_inner_size([700.0, 480.0]),
        ..Default::default()
    };

    eframe::run_native(
        "RustyBox",
        native_options,
        Box::new(|cc| Ok(Box::new(RustyBoxApp::new(cc)))),
    )
}
