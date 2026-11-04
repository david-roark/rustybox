mod app;
mod network_tools;
mod session;
mod sftp;
mod terminal;
mod tunneling;

use app::RustyBoxApp;
use eframe::egui::ViewportBuilder;

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

    let native_options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_title("RustyBox v1.3-44 - Remote Toolbox & Terminal")
            .with_maximized(true)
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([700.0, 480.0]),
        ..Default::default()
    };

    eframe::run_native(
        "RustyBox",
        native_options,
        Box::new(|cc| Ok(Box::new(RustyBoxApp::new(cc)))),
    )
}
