// Hide the console window for release builds on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;

use telemetryvibe::app::application::GaugeApp;
use telemetryvibe::utils::paths::AppPaths;

fn init_logging(paths: &AppPaths) -> tracing_appender::non_blocking::WorkerGuard {
    use tracing_subscriber::prelude::*;
    let file = tracing_appender::rolling::daily(&paths.log_dir, "telemetryvibe.log");
    let (writer, guard) = tracing_appender::non_blocking(file);
    let filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        tracing_subscriber::EnvFilter::new("info,wgpu=warn,naga=warn,eframe=warn,egui_wgpu=warn")
    });
    tracing_subscriber::registry()
        .with(filter)
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(writer)
                .with_ansi(false),
        )
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();
    guard
}

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // Developer helper used by the macOS bundle script.
    if let Some(i) = args.iter().position(|a| a == "--write-icons") {
        let dir = args
            .get(i + 1)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("icons"));
        if let Err(e) = telemetryvibe::platform::write_icon_pngs(&dir) {
            eprintln!("failed to write icons: {e}");
            std::process::exit(1);
        }
        return Ok(());
    }

    let paths = AppPaths::resolve();
    let _guard = init_logging(&paths);
    std::panic::set_hook(Box::new(|info| {
        let bt = std::backtrace::Backtrace::force_capture();
        tracing::error!("panic: {info}\n{bt}");
    }));
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        os = std::env::consts::OS,
        arch = std::env::consts::ARCH,
        "TelemetryVibe starting"
    );

    let open_path = args.iter().find(|a| !a.starts_with('-')).map(PathBuf::from);
    let icon = egui::IconData {
        rgba: telemetryvibe::platform::icon_rgba(256),
        width: 256,
        height: 256,
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("TelemetryVibe")
            .with_inner_size([1480.0, 920.0])
            .with_min_inner_size([1000.0, 640.0])
            .with_drag_and_drop(true)
            .with_icon(icon),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native(
        "TelemetryVibe",
        options,
        Box::new(move |cc| Ok(Box::new(GaugeApp::new(cc, open_path)))),
    )
}
