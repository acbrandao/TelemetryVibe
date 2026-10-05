//! Left panel: media information (video + GPS) and the gauge layer list.

use egui::{RichText, ScrollArea};

use super::theme;
use super::widgets::{kv, section};
use crate::app::application::{GaugeApp, ProxyState, RightTab};
use crate::app::commands::Command;
use crate::gauges::model::Gauge;
use crate::telemetry::Metric;
use crate::telemetry::units::{format_value, to_display, unit_label};
use crate::utils::timecode::{format_bytes, format_duration_human};

pub fn show(app: &mut GaugeApp, ui: &mut egui::Ui) {
    ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            section(ui, "Video");
            video_card(app, ui);
            ui.add_space(4.0);
            section(ui, "GPS / Telemetry");
            gps_card(app, ui);
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                section(ui, "Layers");
                if !app.state.project.gauges.is_empty() {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .small_button("Clear all")
                            .on_hover_text("Remove every gauge (Undo restores them)")
                            .clicked()
                        {
                            app.clear_gauges();
                        }
                    });
                }
            });
            layers(app, ui);
        });
}

fn video_card(app: &mut GaugeApp, ui: &mut egui::Ui) {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.set_width(ui.available_width());
        match app.state.video.clone() {
            None => {
                if app.loading_video.is_some() {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Analyzing video…");
                    });
                } else {
                    ui.label(RichText::new("No video imported").weak());
                    if ui.button("Import Video…").clicked() {
                        app.import_dialog_video();
                    }
                }
            }
            Some(v) => {
                ui.label(RichText::new(v.file_name()).strong())
                    .on_hover_text(v.path.display().to_string());
                kv(ui, "Resolution", v.resolution_label());
                kv(
                    ui,
                    "Frame rate",
                    format!("{:.3} fps", v.fps).replace(".000", ""),
                );
                kv(ui, "Duration", format_duration_human(v.duration));
                kv(
                    ui,
                    "Codec",
                    format!(
                        "{}{}",
                        v.codec.to_uppercase(),
                        v.profile
                            .as_ref()
                            .map(|p| format!(" ({p})"))
                            .unwrap_or_default()
                    ),
                );
                kv(
                    ui,
                    "Audio",
                    v.audio_codec
                        .as_ref()
                        .map(|a| a.to_uppercase())
                        .unwrap_or_else(|| "None".into()),
                );
                kv(ui, "File size", format_bytes(v.file_size));
                if let Some(ct) = v.creation_time {
                    kv(
                        ui,
                        "Recorded",
                        ct.with_timezone(&chrono::Local)
                            .format("%Y-%m-%d %H:%M:%S")
                            .to_string(),
                    );
                }
                ui.add_space(2.0);
                match &app.proxy {
                    ProxyState::None => {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Proxy: none").weak());
                            if ui
                                .small_button("Generate")
                                .on_hover_text("Create a 1080p proxy for smoother preview")
                                .clicked()
                            {
                                app.start_proxy();
                            }
                        });
                    }
                    ProxyState::Generating { progress, .. } => {
                        let p = *progress;
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::ProgressBar::new(p)
                                    .desired_width(130.0)
                                    .text(format!("Proxy {:.0}%", p * 100.0)),
                            );
                            if ui.small_button("Cancel").clicked() {
                                app.cancel_proxy();
                            }
                        });
                    }
                    ProxyState::Ready { .. } => {
                        ui.label(
                            RichText::new(if app.state.project.preview.use_proxy {
                                "Proxy: in use"
                            } else {
                                "Proxy: ready (disabled)"
                            })
                            .color(theme::OK),
                        );
                    }
                }
                if ui.small_button("Replace…").clicked() {
                    app.import_dialog_video();
                }
            }
        }
    });
}

fn gps_card(app: &mut GaugeApp, ui: &mut egui::Ui) {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.set_width(ui.available_width());
        match app.state.track.clone() {
            None => {
                if app.loading_track.is_some() {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Parsing GPS…");
                    });
                } else {
                    ui.label(RichText::new("No GPS data imported").weak());
                    if ui.button("Import GPS…").clicked() {
                        app.import_dialog_gps();
                    }
                }
            }
            Some(t) => {
                let units = app.state.project.units;
                ui.label(RichText::new(&t.name).strong());
                kv(ui, "Format", t.format.clone());
                if let Some(d) = &t.device {
                    kv(ui, "Device", d.clone());
                }
                if let Some(s) = &t.sport {
                    kv(ui, "Sport", s.clone());
                }
                kv(
                    ui,
                    "Start",
                    t.start_time()
                        .with_timezone(&chrono::Local)
                        .format("%Y-%m-%d %H:%M:%S")
                        .to_string(),
                );
                kv(ui, "Duration", format_duration_human(t.duration()));
                let hz = if t.duration() > 0.0 {
                    t.len() as f64 / t.duration()
                } else {
                    0.0
                };
                kv(ui, "Samples", format!("{} ({:.1} Hz)", t.len(), hz));
                if let Some(d) = t.stats.total_distance {
                    kv(
                        ui,
                        "Distance",
                        format!(
                            "{} {}",
                            format_value(
                                Metric::Distance,
                                to_display(Metric::Distance, d, units),
                                2
                            ),
                            unit_label(Metric::Distance, units)
                        ),
                    );
                }
                if let Some(s) = t.stats.get(Metric::Speed) {
                    kv(
                        ui,
                        "Max / avg speed",
                        format!(
                            "{:.1} / {:.1} {}",
                            to_display(Metric::Speed, s.max, units),
                            to_display(Metric::Speed, s.avg, units),
                            unit_label(Metric::Speed, units)
                        ),
                    );
                }
                if let (Some(g), Some(l)) = (t.stats.elevation_gain, t.stats.elevation_loss) {
                    kv(
                        ui,
                        "Elevation ↑ / ↓",
                        format!(
                            "{:.0} / {:.0} {}",
                            to_display(Metric::Altitude, g, units),
                            to_display(Metric::Altitude, l, units),
                            unit_label(Metric::Altitude, units)
                        ),
                    );
                }
                for m in [Metric::GpsAltitude, Metric::Altitude] {
                    let Some(s) = t.stats.get(m) else { continue };
                    kv(
                        ui,
                        if m == Metric::GpsAltitude {
                            "GPS altitude"
                        } else {
                            "Altitude"
                        },
                        format!(
                            "{:.0} – {:.0} {}",
                            to_display(m, s.min, units),
                            to_display(m, s.max, units),
                            unit_label(m, units)
                        ),
                    );
                }
                if let Some(s) = t.stats.get(Metric::HeartRate) {
                    kv(
                        ui,
                        "HR avg / max",
                        format!("{:.0} / {:.0} bpm", s.avg, s.max),
                    );
                }
                if let Some(s) = t.stats.get(Metric::Power) {
                    kv(
                        ui,
                        "Power avg / max",
                        format!("{:.0} / {:.0} W", s.avg, s.max),
                    );
                }
                ui.add_space(2.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(4.0, 2.0);
                    for m in [
                        Metric::Latitude,
                        Metric::Speed,
                        Metric::Altitude,
                        Metric::GpsAltitude,
                        Metric::Distance,
                        Metric::HeartRate,
                        Metric::Cadence,
                        Metric::Power,
                        Metric::Temperature,
                    ] {
                        let (txt, c) = if t.has(m) {
                            ("✔", theme::OK)
                        } else {
                            ("✖", ui.visuals().weak_text_color())
                        };
                        let name = if m == Metric::Latitude {
                            "GPS"
                        } else {
                            m.label()
                        };
                        ui.label(RichText::new(format!("{txt} {name}")).small().color(c));
                    }
                });
                if !t.extra.is_empty() {
                    ui.label(
                        RichText::new(format!("+ {} additional fields", t.extra.len()))
                            .small()
                            .weak(),
                    )
                    .on_hover_text(t.extra.keys().cloned().collect::<Vec<_>>().join("\n"));
                }
                if ui.small_button("Replace…").clicked() {
                    app.import_dialog_gps();
                }
            }
        }
    });
}

fn layers(app: &mut GaugeApp, ui: &mut egui::Ui) {
    if app.state.project.gauges.is_empty() {
        ui.label(
            RichText::new("No gauges yet. Pick a template or drag gauges from the library.").weak(),
        );
        return;
    }
    // Top-most first.
    let gauges: Vec<Gauge> = app.state.project.gauges.iter().rev().cloned().collect();
    for g in gauges {
        let selected = app.state.selection.contains(&g.id);
        ui.horizontal(|ui| {
            let eye = if g.visible { "👁" } else { "◌" };
            if ui.small_button(eye).on_hover_text("Show / hide").clicked() {
                let mut ng = g.clone();
                ng.visible = !g.visible;
                app.state.execute(Command::UpdateGauge(ng));
            }
            let lock = if g.locked { "🔒" } else { "🔓" };
            if ui
                .small_button(lock)
                .on_hover_text("Lock / unlock")
                .clicked()
            {
                let mut ng = g.clone();
                ng.locked = !g.locked;
                app.state.execute(Command::UpdateGauge(ng));
            }
            let group = g.group.map(|n| format!("  [G{n}]")).unwrap_or_default();
            let mut label = RichText::new(format!("{}{group}", g.name));
            if !g.visible {
                label = label.weak();
            }
            let resp = ui.selectable_label(selected, label);
            if resp.clicked() {
                let toggle = ui.input(|i| i.modifiers.command);
                app.state.select(g.id, toggle);
                app.ui.right_tab = RightTab::Inspector;
            }
        });
    }
}
