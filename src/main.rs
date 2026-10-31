mod app;
mod network_tools;
mod session;
mod sftp;
mod terminal;
mod tunneling;

use app::RustyBoxApp;
use eframe::egui::ViewportBuilder;

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_title("RustyBox - Portable Remote Toolbox & Terminal")
            .with_inner_size([1200.0, 780.0])
            .with_min_inner_size([700.0, 480.0]),
        ..Default::default()
    };

    eframe::run_native(
        "RustyBox",
        native_options,
        Box::new(|cc| Ok(Box::new(RustyBoxApp::new(cc)))),
    )
}
