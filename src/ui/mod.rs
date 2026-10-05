//! User interface. Panels are thin views over [`GaugeApp`]; edits go through commands.
//!
//! ```text
//! ┌ menu ─────────────────────────────────────────────────────┐
//! │ toolbar: Import Video · Import GPS · Sync · Template · Render
//! ├ media/layers ┬────────── video canvas ──────────┬ library/inspector ┤
//! ├──────────────┴────────── timeline ──────────────┴────────────────────┤
//! └ status bar ────────────────────────────────────────────────┘
//! ```

pub mod canvas;
pub mod gauge_library;
pub mod inspector;
pub mod media_panel;
pub mod render_panel;
pub mod settings;
pub mod sync_panel;
pub mod theme;
pub mod timeline;
pub mod widgets;

use egui::{Align, Color32, Layout, RichText};

use crate::app::application::{GaugeApp, PendingAction, ProxyState, RightTab, ToastKind};
use crate::app::commands::Command;
use crate::project::autosave::Theme;
use crate::project::model::PreviewQuality;
use crate::telemetry::UnitSystem;

pub fn layout(app: &mut GaugeApp, ui: &mut egui::Ui) {
    menu_bar(app, ui);
    toolbar(app, ui);
    status_bar(app, ui);

    egui::Panel::bottom("timeline")
        .resizable(true)
        .default_size(240.0)
        .min_size(170.0)
        .show(ui, |ui| timeline::show(app, ui));

    egui::Panel::left("media")
        .resizable(true)
        .default_size(260.0)
        .min_size(210.0)
        .show(ui, |ui| media_panel::show(app, ui));

    egui::Panel::right("inspector")
        .resizable(true)
        .default_size(310.0)
        .min_size(260.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut app.ui.right_tab, RightTab::Library, "Gauge Library");
                ui.selectable_value(&mut app.ui.right_tab, RightTab::Inspector, "Inspector");
            });
            ui.separator();
            match app.ui.right_tab {
                RightTab::Library => gauge_library::show(app, ui),
                RightTab::Inspector => inspector::show(app, ui),
            }
        });

    egui::CentralPanel::no_frame().show(ui, |ui| canvas::show(app, ui));

    let ctx = ui.ctx().clone();
    sync_panel::window(app, &ctx);
    render_panel::window(app, &ctx);
    render_panel::queue_window(app, &ctx);
    settings::window(app, &ctx);
    settings::ffmpeg_window(app, &ctx);
    settings::shortcuts_window(app, &ctx);
    settings::about_window(app, &ctx);
    dialogs(app, &ctx);
    toasts(app, &ctx);
}

fn menu_bar(app: &mut GaugeApp, ui: &mut egui::Ui) {
    egui::Panel::top("menu").show(ui, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("New Project          ⌘N").clicked() {
                    app.request(PendingAction::New);
                    ui.close();
                }
                if ui.button("Open Project…        ⌘O").clicked() {
                    app.request(PendingAction::Open(None));
                    ui.close();
                }
                ui.menu_button("Open Recent", |ui| {
                    if app.recent.is_empty() {
                        ui.label(RichText::new("No recent projects").weak());
                    }
                    for p in app.recent.clone() {
                        let label = p
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default();
                        if ui
                            .button(label)
                            .on_hover_text(p.display().to_string())
                            .clicked()
                        {
                            app.request(PendingAction::Open(Some(p)));
                            ui.close();
                        }
                    }
                });
                ui.separator();
                if ui.button("Save                 ⌘S").clicked() {
                    app.save();
                    ui.close();
                }
                if ui.button("Save As…            ⇧⌘S").clicked() {
                    app.save_as();
                    ui.close();
                }
                ui.separator();
                if ui.button("Import Video…        ⌘I").clicked() {
                    app.import_dialog_video();
                    ui.close();
                }
                if ui.button("Import GPS Data…    ⇧⌘I").clicked() {
                    app.import_dialog_gps();
                    ui.close();
                }
                ui.separator();
                if ui.button("Settings…").clicked() {
                    app.ui.show_settings = true;
                    ui.close();
                }
                ui.separator();
                if ui.button("Quit").clicked() {
                    app.request(PendingAction::Quit);
                    ui.close();
                }
            });
            ui.menu_button("Edit", |ui| {
                let undo = app
                    .state
                    .undo
                    .undo_label()
                    .map(|l| format!("Undo {l}"))
                    .unwrap_or_else(|| "Undo".into());
                if ui
                    .add_enabled(
                        app.state.undo.can_undo(),
                        egui::Button::new(format!("{undo}   ⌘Z")),
                    )
                    .clicked()
                {
                    app.state.undo();
                    ui.close();
                }
                let redo = app
                    .state
                    .undo
                    .redo_label()
                    .map(|l| format!("Redo {l}"))
                    .unwrap_or_else(|| "Redo".into());
                if ui
                    .add_enabled(
                        app.state.undo.can_redo(),
                        egui::Button::new(format!("{redo}   ⇧⌘Z")),
                    )
                    .clicked()
                {
                    app.state.redo();
                    ui.close();
                }
                ui.separator();
                let has_sel = !app.state.selection.is_empty();
                if ui
                    .add_enabled(has_sel, egui::Button::new("Duplicate   ⌘D"))
                    .clicked()
                {
                    app.state.duplicate_selection();
                    ui.close();
                }
                if ui
                    .add_enabled(has_sel, egui::Button::new("Delete   ⌫"))
                    .clicked()
                {
                    app.state.delete_selection();
                    ui.close();
                }
                if ui.button("Select All   ⌘A").clicked() {
                    app.state.selection = app.state.project.gauges.iter().map(|g| g.id).collect();
                    ui.close();
                }
                if ui
                    .add_enabled(
                        !app.state.project.gauges.is_empty(),
                        egui::Button::new("Clear All Gauges"),
                    )
                    .clicked()
                {
                    app.clear_gauges();
                    ui.close();
                }
                ui.separator();
                if ui
                    .add_enabled(
                        app.state.selection.len() > 1,
                        egui::Button::new("Group   ⌘G"),
                    )
                    .clicked()
                {
                    inspector::group_selection(app);
                    ui.close();
                }
                if ui
                    .add_enabled(has_sel, egui::Button::new("Ungroup   ⇧⌘G"))
                    .clicked()
                {
                    inspector::ungroup_selection(app);
                    ui.close();
                }
            });
            ui.menu_button("View", |ui| {
                ui.label(RichText::new("Preview quality").weak());
                for q in PreviewQuality::ALL {
                    if ui
                        .radio(app.state.project.preview.quality == q, q.label())
                        .clicked()
                    {
                        set_preview_quality(app, q);
                        ui.close();
                    }
                }
                ui.separator();
                let mut use_proxy = app.state.project.preview.use_proxy;
                if ui
                    .checkbox(&mut use_proxy, "Use proxy for preview")
                    .changed()
                {
                    let mut pv = app.state.project.preview.clone();
                    pv.use_proxy = use_proxy;
                    app.state.execute(Command::SetPreview(pv));
                    app.reconfigure_decoder(true);
                }
                ui.separator();
                ui.menu_button("Theme", |ui| {
                    for t in Theme::ALL {
                        if ui.radio(app.settings.theme == t, t.label()).clicked() {
                            app.settings.theme = t;
                            app.settings.save(&app.paths);
                            ui.close();
                        }
                    }
                });
                ui.separator();
                if ui.button("Toggle Full Screen").clicked() {
                    let fs = ui.ctx().input(|i| i.viewport().fullscreen.unwrap_or(false));
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fs));
                    ui.close();
                }
            });
            ui.menu_button("Project", |ui| {
                ui.label(RichText::new("Units").weak());
                for u in [UnitSystem::Metric, UnitSystem::Imperial] {
                    if ui.radio(app.state.project.units == u, u.label()).clicked() {
                        app.state.execute(Command::SetUnits(u));
                        ui.close();
                    }
                }
                ui.separator();
                if ui.button("Synchronize GPS…").clicked() {
                    app.ui.show_sync = true;
                    ui.close();
                }
                if ui.button("Auto-sync from video metadata").clicked() {
                    app.try_auto_sync(true);
                    ui.close();
                }
                ui.separator();
                ui.menu_button("Template", |ui| gauge_library::template_menu(app, ui));
                ui.separator();
                let proxy_label = match &app.proxy {
                    ProxyState::Generating { .. } => "Cancel Proxy Generation",
                    ProxyState::Ready { .. } => "Rebuild Proxy",
                    ProxyState::None => "Generate Proxy",
                };
                if ui
                    .add_enabled(app.state.video.is_some(), egui::Button::new(proxy_label))
                    .clicked()
                {
                    match &app.proxy {
                        ProxyState::Generating { .. } => app.cancel_proxy(),
                        ProxyState::Ready { path, .. } => {
                            let _ = std::fs::remove_file(path);
                            app.proxy = ProxyState::None;
                            app.start_proxy();
                        }
                        ProxyState::None => app.start_proxy(),
                    }
                    ui.close();
                }
            });
            ui.menu_button("Render", |ui| {
                if ui.button("Render Video…   ⌘R").clicked() {
                    app.ui.show_render = true;
                    ui.close();
                }
                if ui.button("Render Queue").clicked() {
                    app.ui.show_queue = true;
                    ui.close();
                }
            });
            ui.menu_button("Help", |ui| {
                if ui.button("Keyboard Shortcuts").clicked() {
                    app.ui.show_shortcuts = true;
                    ui.close();
                }
                if ui.button("FFmpeg Information").clicked() {
                    app.ui.show_ffmpeg_info = true;
                    ui.close();
                }
                if ui.button("Open Logs").clicked() {
                    crate::platform::reveal_in_file_manager(&app.paths.log_dir);
                    ui.close();
                }
                ui.separator();
                if ui.button("About TelemetryVibe").clicked() {
                    app.ui.show_about = true;
                    ui.close();
                }
            });
        });
    });
}

fn set_preview_quality(app: &mut GaugeApp, q: PreviewQuality) {
    let mut pv = app.state.project.preview.clone();
    pv.quality = q;
    app.state.execute(Command::SetPreview(pv));
    app.reconfigure_decoder(true);
}

fn toolbar(app: &mut GaugeApp, ui: &mut egui::Ui) {
    egui::Panel::top("toolbar")
        .frame(egui::Frame::side_top_panel(ui.style()).inner_margin(egui::Margin::symmetric(10, 6)))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let big = |t: &str| {
                    egui::Button::new(RichText::new(t).size(14.0)).min_size(egui::vec2(0.0, 28.0))
                };
                if ui
                    .add(big("🎬  Import Video"))
                    .on_hover_text("Import a video (or drop it on the window)")
                    .clicked()
                {
                    app.import_dialog_video();
                }
                if ui
                    .add(big("📍  Import GPS"))
                    .on_hover_text("Import FIT / GPX / TCX telemetry")
                    .clicked()
                {
                    app.import_dialog_gps();
                }
                if ui
                    .add(big("⏱  Sync"))
                    .on_hover_text("Synchronize GPS with video")
                    .clicked()
                {
                    app.ui.show_sync = !app.ui.show_sync;
                }
                egui::ComboBox::from_id_salt("template_combo")
                    .selected_text(RichText::new("▦  Template").size(14.0))
                    .show_ui(ui, |ui| gauge_library::template_menu(app, ui));
                ui.separator();
                ui.label("Preview");
                let current = app.state.project.preview.quality;
                for q in PreviewQuality::ALL {
                    if ui.selectable_label(current == q, q.label()).clicked() && current != q {
                        set_preview_quality(app, q);
                    }
                }
                ui.separator();
                let units = app.state.project.units;
                for u in [UnitSystem::Metric, UnitSystem::Imperial] {
                    if ui.selectable_label(units == u, u.label()).clicked() && units != u {
                        app.state.execute(Command::SetUnits(u));
                    }
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let render = egui::Button::new(
                        RichText::new("⏺  Render Video")
                            .size(14.0)
                            .color(Color32::WHITE),
                    )
                    .fill(theme::ACCENT)
                    .min_size(egui::vec2(0.0, 28.0));
                    if ui.add_enabled(app.state.video.is_some(), render).clicked() {
                        app.ui.show_render = true;
                    }
                    let n = app
                        .queue
                        .jobs()
                        .iter()
                        .filter(|j| !j.status().is_finished())
                        .count();
                    if n > 0 && ui.button(format!("Queue ({n})")).clicked() {
                        app.ui.show_queue = true;
                    }
                });
            });
        });
}

fn status_bar(app: &mut GaugeApp, ui: &mut egui::Ui) {
    egui::Panel::bottom("status")
        .frame(egui::Frame::side_top_panel(ui.style()).inner_margin(egui::Margin::symmetric(10, 3)))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 14.0;
                match (&app.ffmpeg, &app.ffmpeg_error, app.ffmpeg_detecting) {
                    (_, _, true) => {
                        ui.spinner();
                        ui.label("Detecting FFmpeg…");
                    }
                    (Some(ff), _, _) => {
                        if ui.link(RichText::new(ff.summary()).small()).clicked() {
                            app.ui.show_ffmpeg_info = true;
                        }
                    }
                    (None, Some(_), _)
                        if ui
                            .link(
                                RichText::new("⚠ FFmpeg not found — click to configure")
                                    .color(theme::WARN)
                                    .small(),
                            )
                            .clicked() =>
                    {
                        app.ui.show_settings = true;
                    }
                    _ => {}
                }
                if app.loading_video.is_some() {
                    ui.spinner();
                    ui.label(RichText::new("Analyzing video…").small());
                }
                if app.loading_track.is_some() {
                    ui.spinner();
                    ui.label(RichText::new("Parsing GPS…").small());
                }
                if let ProxyState::Generating { progress, .. } = &app.proxy {
                    ui.add(
                        egui::ProgressBar::new(*progress)
                            .desired_width(120.0)
                            .text(RichText::new(format!("Proxy {:.0}%", progress * 100.0)).small()),
                    );
                }
                if let Some(job) = app.queue.active_job() {
                    let p = job.progress.lock().clone();
                    let frac = if p.total_frames > 0 {
                        p.frames_done as f32 / p.total_frames as f32
                    } else {
                        0.0
                    };
                    let resp = ui.add(
                        egui::ProgressBar::new(frac).desired_width(160.0).text(
                            RichText::new(format!(
                                "Rendering {:.0}% · {:.0} fps",
                                frac * 100.0,
                                p.encode_fps
                            ))
                            .small(),
                        ),
                    );
                    if resp.interact(egui::Sense::click()).clicked() {
                        app.ui.show_queue = true;
                    }
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("{} units", app.state.project.units.label()))
                            .small()
                            .weak(),
                    );
                    if app.state.dirty {
                        ui.label(RichText::new("Unsaved changes").small().weak());
                    }
                });
            });
        });
}

fn dialogs(app: &mut GaugeApp, ctx: &egui::Context) {
    if let Some(action) = app.ui.confirm_discard.clone() {
        let mut choice: Option<u8> = None;
        egui::Modal::new(egui::Id::new("confirm_discard")).show(ctx, |ui| {
            ui.set_width(360.0);
            ui.heading("Unsaved changes");
            ui.label("Do you want to save the changes to this project?");
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Save").clicked() {
                    choice = Some(0);
                }
                if ui.button("Don't Save").clicked() {
                    choice = Some(1);
                }
                if ui.button("Cancel").clicked() {
                    choice = Some(2);
                }
            });
        });
        match choice {
            Some(0) => {
                app.ui.confirm_discard = None;
                if app.save() {
                    app.perform(action);
                }
            }
            Some(1) => {
                app.ui.confirm_discard = None;
                app.state.dirty = false;
                app.perform(action);
            }
            Some(2) => app.ui.confirm_discard = None,
            _ => {}
        }
    }

    if let Some(rec) = app.ui.recovery.clone() {
        let mut choice: Option<bool> = None;
        egui::Modal::new(egui::Id::new("recovery")).show(ctx, |ui| {
            ui.set_width(400.0);
            ui.heading("Recover unsaved project?");
            ui.label("TelemetryVibe did not shut down cleanly last time. An autosaved copy of your project is available.");
            ui.label(
                RichText::new(format!(
                    "Saved {}{}",
                    rec.meta.saved_at.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M"),
                    rec.meta.project_path.as_ref().map(|p| format!(" · {}", p.display())).unwrap_or_default()
                ))
                .weak(),
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Recover").clicked() {
                    choice = Some(true);
                }
                if ui.button("Discard").clicked() {
                    choice = Some(false);
                }
            });
        });
        match choice {
            Some(true) => {
                app.ui.recovery = None;
                app.recover(rec);
            }
            Some(false) => {
                app.ui.recovery = None;
                crate::project::autosave::discard_recovery(&app.paths);
            }
            None => {}
        }
    }
}

fn toasts(app: &mut GaugeApp, ctx: &egui::Context) {
    let now = std::time::Instant::now();
    app.ui.toasts.retain(|t| t.until > now);
    if app.ui.toasts.is_empty() {
        return;
    }
    ctx.request_repaint_after(std::time::Duration::from_millis(250));
    egui::Area::new(egui::Id::new("toasts"))
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-16.0, -36.0))
        .order(egui::Order::Foreground)
        .interactable(true)
        .show(ctx, |ui| {
            let mut remove = None;
            for (i, t) in app.ui.toasts.iter().enumerate().rev().take(5) {
                let (icon, color) = match t.kind {
                    ToastKind::Info => ("ℹ", theme::ACCENT_2),
                    ToastKind::Success => ("✔", theme::OK),
                    ToastKind::Warning => ("⚠", theme::WARN),
                    ToastKind::Error => ("✖", theme::ERROR),
                };
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_max_width(380.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(icon).color(color).strong());
                        ui.add(egui::Label::new(&t.text).wrap());
                        if ui.small_button("×").clicked() {
                            remove = Some(i);
                        }
                    });
                });
                ui.add_space(4.0);
            }
            if let Some(i) = remove {
                app.ui.toasts.remove(i);
            }
        });
}
