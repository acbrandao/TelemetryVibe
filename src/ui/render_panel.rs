//! Render settings dialog and render queue window.

use std::path::PathBuf;

use egui::{Grid, RichText};

use super::theme;
use super::widgets::prop_label;
use crate::app::application::{GaugeApp, ToastKind};
use crate::app::commands::Command;
use crate::render::export::{ExportSpec, JobStatus, RenderJob};
use crate::utils::timecode::{format_bytes, format_duration_human, format_timecode};
use crate::video::encoder::{
    FpsPreset, Quality, ResolutionPreset, estimated_size, output_geometry,
};
use crate::video::ffmpeg::{Codec, HwPreference};

#[derive(Default)]
pub struct RenderDialogState {
    pub output: Option<PathBuf>,
}

fn default_output(app: &GaugeApp) -> Option<PathBuf> {
    let v = app.state.video.as_ref()?;
    let stem = v.path.file_stem()?.to_string_lossy().to_string();
    let dir = v.path.parent()?.to_path_buf();
    let mut p = dir.join(format!("{stem}_gauges.mp4"));
    let mut n = 2;
    while p.exists() {
        p = dir.join(format!("{stem}_gauges_{n}.mp4"));
        n += 1;
    }
    Some(p)
}

pub fn window(app: &mut GaugeApp, ctx: &egui::Context) {
    let mut open = app.ui.show_render;
    let mut close = false;
    egui::Window::new("Render Video")
        .open(&mut open)
        .default_width(420.0)
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| close = contents(app, ui));
    app.ui.show_render = open && !close;
}

fn contents(app: &mut GaugeApp, ui: &mut egui::Ui) -> bool {
    let Some(info) = app.state.video.clone() else {
        ui.label("Import a video first.");
        return false;
    };
    let Some(ff) = app.ffmpeg.clone() else {
        ui.colored_label(
            theme::WARN,
            "FFmpeg is not available. Configure it in Settings.",
        );
        return false;
    };
    if app.ui.render.output.is_none() {
        app.ui.render.output = default_output(app);
    }
    let mut s = app.state.project.render.clone();
    let trim = app
        .state
        .project
        .trim
        .and_then(|r| r.normalized(info.duration));
    let duration = trim.map_or(info.duration, |r| r.duration());
    let mut clear_trim = false;
    Grid::new("render_grid")
        .num_columns(2)
        .spacing([12.0, 8.0])
        .show(ui, |ui| {
            prop_label(ui, "Format");
            ui.label("MP4");
            ui.end_row();
            prop_label(ui, "Codec");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut s.codec, Codec::H264, "H.264");
                ui.selectable_value(&mut s.codec, Codec::H265, "H.265");
            });
            ui.end_row();
            prop_label(ui, "Resolution");
            egui::ComboBox::from_id_salt("res")
                .selected_text(s.resolution.label())
                .show_ui(ui, |ui| {
                    for r in ResolutionPreset::ALL {
                        ui.selectable_value(&mut s.resolution, r, r.label());
                    }
                });
            ui.end_row();
            prop_label(ui, "Frame rate");
            egui::ComboBox::from_id_salt("fps")
                .selected_text(if s.fps == FpsPreset::Original {
                    format!("Original ({:.2})", info.fps)
                } else {
                    s.fps.label().to_string()
                })
                .show_ui(ui, |ui| {
                    for f in FpsPreset::ALL {
                        ui.selectable_value(&mut s.fps, f, f.label());
                    }
                });
            ui.end_row();
            prop_label(ui, "Quality");
            ui.horizontal(|ui| {
                for q in Quality::ALL {
                    ui.selectable_value(&mut s.quality, q, q.label());
                }
            });
            ui.end_row();
            prop_label(ui, "Acceleration");
            egui::ComboBox::from_id_salt("hw")
                .selected_text(s.hw.label())
                .show_ui(ui, |ui| {
                    for h in HwPreference::ALL {
                        ui.selectable_value(&mut s.hw, h, h.label());
                    }
                });
            ui.end_row();
            prop_label(ui, "Encoder");
            let enc = ff.choose_encoder(s.hw, s.codec);
            ui.label(
                RichText::new(format!("{} ({})", enc.label(), enc.encoder_name(s.codec))).color(
                    if enc.is_hardware() {
                        theme::OK
                    } else {
                        ui.visuals().text_color()
                    },
                ),
            );
            ui.end_row();
            prop_label(ui, "Range");
            ui.horizontal(|ui| match trim {
                Some(r) => {
                    ui.label(
                        RichText::new(format!(
                            "{} – {}",
                            format_timecode(r.start),
                            format_timecode(r.end)
                        ))
                        .monospace()
                        .color(theme::TRIM),
                    );
                    if ui
                        .small_button("Full video")
                        .on_hover_text("Clear the timeline trim and render the whole video")
                        .clicked()
                    {
                        clear_trim = true;
                    }
                }
                None => {
                    ui.label("Full video").on_hover_text(
                        "Drag the trim handles on the timeline (or press I / O) to render only a section",
                    );
                }
            });
            ui.end_row();
            prop_label(ui, "Audio");
            if info.has_audio {
                ui.checkbox(&mut s.include_audio, "Keep original audio");
            } else {
                ui.label(RichText::new("No audio in source").weak());
            }
            ui.end_row();
        });
    if s.hw == HwPreference::Hardware && !ff.has_hw_encoder() {
        ui.colored_label(
            theme::WARN,
            "No working hardware encoder was detected; software encoding will be used.",
        );
    }
    if s != app.state.project.render {
        app.state.execute(Command::SetRender(s.clone()));
    }
    if clear_trim {
        app.clear_trim();
    }

    let geo = output_geometry(&info, &s);
    ui.add_space(6.0);
    ui.label(
        RichText::new(format!(
            "{}×{} · {:.2} fps · {} · ≈ {}",
            geo.width,
            geo.height,
            geo.fps,
            format_duration_human(duration),
            format_bytes(estimated_size(&geo, &s, duration, info.has_audio))
        ))
        .weak(),
    );
    let visible = app
        .state
        .project
        .gauges
        .iter()
        .filter(|g| g.visible)
        .count();
    if visible == 0 {
        ui.colored_label(
            theme::WARN,
            "No visible gauges — the video will be re-encoded without overlay.",
        );
    }
    ui.separator();
    ui.horizontal(|ui| {
        prop_label(ui, "Output");
        let label = app
            .ui
            .render
            .output
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "—".into());
        ui.add(egui::Label::new(RichText::new(label).monospace().small()).truncate());
    });
    if ui.button("Choose output file…").clicked() {
        let mut d = rfd::FileDialog::new().add_filter("MP4 video", &["mp4"]);
        if let Some(p) = &app.ui.render.output {
            if let Some(dir) = p.parent() {
                d = d.set_directory(dir);
            }
            if let Some(n) = p.file_name() {
                d = d.set_file_name(n.to_string_lossy());
            }
        }
        if let Some(mut p) = d.save_file() {
            p.set_extension("mp4");
            app.ui.render.output = Some(p);
        }
    }
    ui.add_space(8.0);
    let mut close = false;
    ui.horizontal(|ui| {
        let ready = app.ui.render.output.is_some();
        let render =
            egui::Button::new(RichText::new("Render").strong().color(egui::Color32::WHITE))
                .fill(theme::ACCENT);
        let mut queue_it = |start: bool, app: &mut GaugeApp| {
            let Some(output) = app.ui.render.output.take() else {
                return;
            };
            if Some(&output) == app.state.video.as_ref().map(|v| &v.path) {
                app.toast(
                    ToastKind::Error,
                    "The output file cannot be the source video.",
                );
                return;
            }
            let job = RenderJob::new(ExportSpec {
                output,
                info: info.clone(),
                settings: s.clone(),
                gauges: app.state.project.gauges.clone(),
                track: app.state.track.clone(),
                sync: app.state.project.sync.clone(),
                units: app.state.project.units,
                ffmpeg: ff.clone(),
                range: trim,
            });
            app.queue.add(job);
            if start {
                app.queue.start();
            }
            app.ui.show_queue = true;
            close = true;
        };
        if ui.add_enabled(ready, render).clicked() {
            queue_it(true, app);
        }
        if ui
            .add_enabled(ready, egui::Button::new("Add to Queue"))
            .clicked()
        {
            queue_it(false, app);
        }
    });
    close
}

pub fn queue_window(app: &mut GaugeApp, ctx: &egui::Context) {
    let mut open = app.ui.show_queue;
    egui::Window::new("Render Queue")
        .open(&mut open)
        .default_width(560.0)
        .collapsible(true)
        .show(ctx, |ui| {
            let jobs = app.queue.jobs();
            if jobs.is_empty() {
                ui.label(RichText::new("No render jobs. Use Render Video to add one.").weak());
            }
            for job in &jobs {
                let p = job.progress.lock().clone();
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let name = job
                        .spec
                        .output
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(name).strong());
                        let color = match p.status {
                            JobStatus::Done => theme::OK,
                            JobStatus::Failed(_) => theme::ERROR,
                            JobStatus::Cancelled => ui.visuals().weak_text_color(),
                            _ => ui.visuals().text_color(),
                        };
                        ui.label(RichText::new(p.status.label()).color(color));
                    });
                    let section = job
                        .spec
                        .effective_range()
                        .map(|r| {
                            format!(
                                " (section {} – {})",
                                format_timecode(r.start),
                                format_timecode(r.end)
                            )
                        })
                        .unwrap_or_default();
                    ui.label(
                        RichText::new(format!(
                            "{}{section} · {}×{} · {} · {} · ≈ {}",
                            format_duration_human(job.spec.duration()),
                            job.geometry.width,
                            job.geometry.height,
                            job.spec.settings.codec.label(),
                            job.encoder.encoder_name(job.spec.settings.codec),
                            format_bytes(job.estimated_size)
                        ))
                        .small()
                        .weak(),
                    );
                    let frac = if p.total_frames > 0 {
                        p.frames_done as f32 / p.total_frames as f32
                    } else {
                        0.0
                    };
                    let eta = p
                        .eta
                        .map(|e| format!(" · ETA {}", format_duration_human(e.as_secs_f64())))
                        .unwrap_or_default();
                    let text = match p.status {
                        JobStatus::Running | JobStatus::Paused => {
                            format!("{:.1}% · {:.1} fps{eta}", frac * 100.0, p.encode_fps)
                        }
                        JobStatus::Done => format!(
                            "Finished in {}",
                            format_duration_human(p.elapsed.as_secs_f64())
                        ),
                        _ => format!("{:.0}%", frac * 100.0),
                    };
                    ui.add(egui::ProgressBar::new(frac).text(text));
                    ui.horizontal(|ui| {
                        if p.status.is_active() {
                            let paused = job.is_paused();
                            if ui.button(if paused { "Resume" } else { "Pause" }).clicked() {
                                job.set_paused(!paused);
                            }
                        }
                        if !p.status.is_finished() && ui.button("Cancel").clicked() {
                            job.cancel();
                        }
                        if ui.button("Open Folder").clicked() {
                            let target = if job.spec.output.exists() {
                                job.spec.output.clone()
                            } else {
                                job.spec
                                    .output
                                    .parent()
                                    .map(|d| d.to_path_buf())
                                    .unwrap_or_default()
                            };
                            crate::platform::reveal_in_file_manager(&target);
                        }
                        if p.status.is_finished() && ui.small_button("Remove").clicked() {
                            app.queue.remove(job.id);
                        }
                    });
                });
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let pending = jobs.iter().any(|j| j.status() == JobStatus::Queued);
                if ui
                    .add_enabled(
                        pending && !app.queue.is_running(),
                        egui::Button::new("Start Queue"),
                    )
                    .clicked()
                {
                    app.queue.start();
                }
                if ui.button("Clear Finished").clicked() {
                    app.queue.clear_finished();
                }
            });
        });
    app.ui.show_queue = open;
}
