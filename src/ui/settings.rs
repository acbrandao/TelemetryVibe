//! Settings, FFmpeg information, shortcuts and about windows.

use egui::{Grid, RichText};

use super::theme;
use super::widgets::prop_label;
use crate::app::application::{GaugeApp, ToastKind};
use crate::project::autosave::Theme;
use crate::utils::timecode::format_bytes;

pub fn window(app: &mut GaugeApp, ctx: &egui::Context) {
    let mut open = app.ui.show_settings;
    egui::Window::new("Settings").open(&mut open).default_width(460.0).collapsible(false).show(ctx, |ui| {
        super::widgets::section(ui, "FFmpeg");
        match (&app.ffmpeg, &app.ffmpeg_error) {
            (Some(ff), _) => {
                ui.label(RichText::new(ff.summary()).color(theme::OK));
                ui.label(RichText::new(ff.ffmpeg.display().to_string()).small().monospace());
            }
            (None, Some(e)) => {
                ui.colored_label(theme::WARN, e);
                ui.label("Install FFmpeg (e.g. `brew install ffmpeg` on macOS, or download a build for Windows) or locate the ffmpeg executable:");
            }
            _ => {
                ui.spinner();
            }
        }
        ui.horizontal(|ui| {
            let label = app.settings.ffmpeg_path.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "Automatic".into());
            prop_label(ui, "Location");
            ui.label(RichText::new(label).monospace().small());
        });
        ui.horizontal(|ui| {
            if ui.button("Browse…").clicked()
                && let Some(p) = rfd::FileDialog::new().set_title("Locate ffmpeg").pick_file()
            {
                app.settings.ffmpeg_path = Some(p);
                app.settings.save(&app.paths);
                app.detect_ffmpeg();
            }
            if ui.button("Use automatic").clicked() {
                app.settings.ffmpeg_path = None;
                app.settings.save(&app.paths);
                app.detect_ffmpeg();
            }
            if ui.button("Re-detect").clicked() {
                app.detect_ffmpeg();
            }
        });

        super::widgets::section(ui, "Preview");
        Grid::new("settings_grid").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
            prop_label(ui, "Hardware decoding");
            if ui.checkbox(&mut app.settings.hardware_decode, "").changed() {
                app.settings.save(&app.paths);
                app.reconfigure_decoder(true);
            }
            ui.end_row();
            prop_label(ui, "Auto-generate proxies");
            if ui.checkbox(&mut app.settings.auto_proxy, "").on_hover_text("For sources larger than 1440p").changed() {
                app.settings.save(&app.paths);
            }
            ui.end_row();
            prop_label(ui, "Theme");
            let before = app.settings.theme;
            egui::ComboBox::from_id_salt("theme")
                .selected_text(before.label())
                .show_ui(ui, |ui| {
                    for t in Theme::ALL {
                        ui.selectable_value(&mut app.settings.theme, t, t.label());
                    }
                });
            if before != app.settings.theme {
                app.settings.save(&app.paths);
            }
            ui.end_row();
        });

        super::widgets::section(ui, "Cache");
        ui.horizontal(|ui| {
            ui.label(format!("{} in {}", format_bytes(app.paths.cache_size()), app.paths.cache_dir.display()));
        });
        ui.horizontal(|ui| {
            if ui.button("Clear Cache").on_hover_text("Removes proxies, thumbnails and cached metadata").clicked() {
                app.cancel_proxy();
                match app.paths.clear_cache() {
                    Ok(()) => {
                        app.proxy = crate::app::application::ProxyState::None;
                        app.reconfigure_decoder(true);
                        app.toast(ToastKind::Success, "Cache cleared");
                    }
                    Err(e) => app.toast(ToastKind::Error, format!("Could not clear cache: {e}")),
                }
            }
            if ui.button("Open Cache Folder").clicked() {
                crate::platform::reveal_in_file_manager(&app.paths.cache_dir);
            }
            if ui.button("Open Logs").clicked() {
                crate::platform::reveal_in_file_manager(&app.paths.log_dir);
            }
        });
    });
    app.ui.show_settings = open;
}

pub fn ffmpeg_window(app: &mut GaugeApp, ctx: &egui::Context) {
    let mut open = app.ui.show_ffmpeg_info;
    egui::Window::new("FFmpeg Information")
        .open(&mut open)
        .default_width(420.0)
        .show(ctx, |ui| match &app.ffmpeg {
            Some(ff) => {
                Grid::new("ff_grid")
                    .num_columns(2)
                    .spacing([10.0, 6.0])
                    .show(ui, |ui| {
                        prop_label(ui, "Version");
                        ui.label(&ff.version);
                        ui.end_row();
                        prop_label(ui, "ffmpeg");
                        ui.label(
                            RichText::new(ff.ffmpeg.display().to_string())
                                .monospace()
                                .small(),
                        );
                        ui.end_row();
                        prop_label(ui, "ffprobe");
                        ui.label(
                            RichText::new(ff.ffprobe.display().to_string())
                                .monospace()
                                .small(),
                        );
                        ui.end_row();
                        prop_label(ui, "HW decoding");
                        ui.label(if ff.hwaccels.is_empty() {
                            "none".to_string()
                        } else {
                            ff.hwaccels.join(", ")
                        });
                        ui.end_row();
                        prop_label(ui, "Encoders");
                        ui.label(ff.encoders.join(", "));
                        ui.end_row();
                        prop_label(ui, "Verified");
                        ui.vertical(|ui| {
                            for (f, c) in &ff.working {
                                ui.label(
                                    RichText::new(format!(
                                        "✔ {} — {}",
                                        f.label(),
                                        f.encoder_name(*c)
                                    ))
                                    .color(
                                        if f.is_hardware() {
                                            theme::OK
                                        } else {
                                            ui.visuals().text_color()
                                        },
                                    ),
                                );
                            }
                        });
                        ui.end_row();
                    });
            }
            None => {
                ui.label("FFmpeg is not available.");
            }
        });
    app.ui.show_ffmpeg_info = open;
}

pub fn shortcuts_window(app: &mut GaugeApp, ctx: &egui::Context) {
    let mut open = app.ui.show_shortcuts;
    egui::Window::new("Keyboard Shortcuts")
        .open(&mut open)
        .default_width(360.0)
        .show(ctx, |ui| {
            Grid::new("keys")
                .num_columns(2)
                .striped(true)
                .spacing([16.0, 4.0])
                .show(ui, |ui| {
                    for (k, v) in [
                        ("Space", "Play / pause"),
                        ("← / →", "Previous / next frame (nudge gauge when selected)"),
                        ("⇧ ← / ⇧ →", "Back / forward 10 frames (nudge 10 px)"),
                        (", / .", "Previous / next frame"),
                        ("Home / End", "Beginning / end"),
                        ("I / O", "Set render start / end at the playhead"),
                        ("⌥X", "Clear trim (render the whole video)"),
                        ("Delete", "Delete selected gauge"),
                        ("Esc", "Deselect"),
                        ("⌘/Ctrl Z", "Undo"),
                        ("⌘/Ctrl ⇧ Z", "Redo"),
                        ("⌘/Ctrl D", "Duplicate"),
                        ("⌘/Ctrl A", "Select all gauges"),
                        ("⌘/Ctrl G / ⇧G", "Group / ungroup"),
                        ("⌘/Ctrl S", "Save"),
                        ("⌘/Ctrl ⇧ S", "Save as"),
                        ("⌘/Ctrl O", "Open project"),
                        ("⌘/Ctrl I / ⇧I", "Import video / GPS"),
                        ("⌘/Ctrl R", "Render"),
                        ("⇧ drag", "Constrain move / keep aspect / snap rotation"),
                        ("⌥/Alt drag", "Move without snapping"),
                        ("⌘/Ctrl click", "Add to selection"),
                        ("⌘/Ctrl scroll", "Zoom timeline"),
                    ] {
                        ui.label(RichText::new(k).monospace());
                        ui.label(v);
                        ui.end_row();
                    }
                });
        });
    app.ui.show_shortcuts = open;
}

pub fn about_window(app: &mut GaugeApp, ctx: &egui::Context) {
    let mut open = app.ui.show_about;
    egui::Window::new("About TelemetryVibe").open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
        ui.heading("TelemetryVibe");
        ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
        ui.label("GPS telemetry gauge overlays for video.");
        ui.add_space(6.0);
        ui.label(RichText::new("Built with Rust, egui/wgpu, tiny-skia and FFmpeg. Inter font (SIL Open Font License).").small().weak());
    });
    app.ui.show_about = open;
}
