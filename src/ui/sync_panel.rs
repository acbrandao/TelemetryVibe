//! Synchronization window: automatic (metadata), manual offset, and visual event sync with an
//! interactive speed graph of the whole recording.

use egui::{Color32, Pos2, Rect, RichText, Sense, Shape, Stroke, pos2, vec2};

use super::theme;
use crate::app::application::GaugeApp;
use crate::app::commands::Command;
use crate::telemetry::Metric;
use crate::telemetry::units::{to_display, unit_label};
use crate::utils::timecode::{format_offset, format_timecode, format_wall_time};

#[derive(Default)]
pub struct SyncState {
    /// GPS time picked on the graph (seconds from track start).
    pub picked_gps: Option<f64>,
}

pub fn window(app: &mut GaugeApp, ctx: &egui::Context) {
    let mut open = app.ui.show_sync;
    egui::Window::new("Synchronize GPS with Video")
        .open(&mut open)
        .default_width(620.0)
        .resizable(true)
        .collapsible(false)
        .show(ctx, |ui| contents(app, ui));
    app.ui.show_sync = open;
}

fn contents(app: &mut GaugeApp, ui: &mut egui::Ui) {
    let Some(track) = app.state.track.clone() else {
        ui.label("Import a GPS file to synchronize.");
        if ui.button("Import GPS…").clicked() {
            app.import_dialog_gps();
        }
        return;
    };
    let offset = app.state.project.sync.offset;
    let units = app.state.project.units;
    let gps_now = app.state.gps_time();

    // Status line.
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!("Video {}", format_timecode(app.state.playhead))).monospace(),
        );
        ui.label("↔");
        let wall = format_wall_time(track.wall_time(gps_now), true);
        ui.label(
            RichText::new(format!("GPS {wall}"))
                .monospace()
                .color(theme::GPS_COLOR),
        );
        ui.label(
            RichText::new(format!("offset {}", format_offset(offset)))
                .monospace()
                .strong(),
        );
    });
    if let Some(v) = track.value(Metric::Speed, gps_now) {
        ui.label(
            RichText::new(format!(
                "Speed at playhead: {:.1} {}",
                to_display(Metric::Speed, v, units),
                unit_label(Metric::Speed, units)
            ))
            .weak(),
        );
    }
    ui.separator();

    // A — automatic.
    ui.label(RichText::new("1 · Automatic").strong());
    ui.horizontal(|ui| {
        let has_meta = app
            .state
            .video
            .as_ref()
            .and_then(|v| v.creation_time)
            .is_some();
        if ui
            .add_enabled(has_meta, egui::Button::new("Use video recording time"))
            .clicked()
        {
            app.state.undo.break_merge();
            app.try_auto_sync(true);
        }
        if !has_meta {
            ui.label(RichText::new("(no time metadata in this video)").weak());
        }
        ui.label("Timezone fix:");
        if ui.small_button("−1 h").clicked() {
            app.state.execute(Command::SetOffset(offset - 3600.0));
        }
        if ui.small_button("+1 h").clicked() {
            app.state.execute(Command::SetOffset(offset + 3600.0));
        }
        if ui
            .small_button("Align starts")
            .on_hover_text("Video start = GPS start")
            .clicked()
        {
            app.state.undo.break_merge();
            app.state.execute(Command::SetOffset(0.0));
        }
    });
    ui.add_space(4.0);

    // B — manual.
    ui.label(RichText::new("2 · Manual offset").strong());
    ui.horizontal(|ui| super::timeline::offset_editor(app, ui));
    ui.label(
        RichText::new("Tip: drag the GPS track in the timeline to slide it against the video.")
            .small()
            .weak(),
    );
    ui.add_space(4.0);

    // C — visual event sync.
    ui.label(RichText::new("3 · Match an event").strong());
    ui.label(RichText::new("Scrub the video to a recognizable moment (starting off, a stop, a turn), mark it, then click the same moment on the GPS graph below.").weak());
    ui.horizontal(|ui| {
        if ui
            .button(format!(
                "Mark video event @ {}",
                format_timecode(app.state.playhead)
            ))
            .clicked()
        {
            let gps = app.state.project.sync.gps_mark;
            app.state.execute(Command::SetSyncMarks {
                video: Some(app.state.playhead),
                gps,
            });
        }
        if let Some(vm) = app.state.project.sync.video_mark {
            ui.label(RichText::new(format!("video ✔ {}", format_timecode(vm))).color(theme::WARN));
            if ui.small_button("go").clicked() {
                app.set_playhead(vm);
            }
        }
    });

    // Detected events.
    let events: Vec<_> = track.events.iter().take(12).cloned().collect();
    if !events.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Detected:").weak());
            for e in events {
                let label = format!(
                    "{} {}",
                    e.kind.label(),
                    format_wall_time(track.wall_time(e.t), false)
                );
                if ui.small_button(label).clicked() {
                    app.ui.sync.picked_gps = Some(e.t);
                }
            }
        });
    }

    speed_graph(app, ui, &track);

    ui.horizontal(|ui| {
        let picked = app.ui.sync.picked_gps;
        if let Some(p) = picked {
            ui.label(
                RichText::new(format!(
                    "GPS event ✔ {}",
                    format_wall_time(track.wall_time(p), true)
                ))
                .color(theme::WARN),
            );
        }
        let can = app.state.project.sync.video_mark.is_some() && picked.is_some();
        if ui
            .add_enabled(
                can,
                egui::Button::new(RichText::new("Align event").strong()).fill(theme::ACCENT),
            )
            .clicked()
            && let (Some(vm), Some(gm)) = (app.state.project.sync.video_mark, picked)
        {
            app.state.undo.break_merge();
            app.state.execute(Command::SetSyncMarks {
                video: Some(vm),
                gps: Some(gm),
            });
            app.state
                .execute(Command::SetOffset(crate::telemetry::sync::event_offset(
                    vm, gm,
                )));
            app.set_playhead(vm);
        }
        if ui.small_button("Clear marks").clicked() {
            app.ui.sync.picked_gps = None;
            app.state.execute(Command::SetSyncMarks {
                video: None,
                gps: None,
            });
        }
    });
}

fn speed_graph(app: &mut GaugeApp, ui: &mut egui::Ui, track: &crate::telemetry::Track) {
    let metric = if track.has(Metric::Speed) {
        Metric::Speed
    } else {
        Metric::Altitude
    };
    let (w, h) = (ui.available_width(), 130.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, ui.visuals().extreme_bg_color);
    let dur = track.duration().max(1.0);
    let x_of = |t: f64| rect.left() + (t / dur) as f32 * rect.width();
    let t_of = |x: f32| (((x - rect.left()) / rect.width()) as f64 * dur).clamp(0.0, dur);
    let (lo, hi) = track
        .stats
        .get(metric)
        .map(|s| (s.min.min(0.0), s.max.max(0.1)))
        .unwrap_or((0.0, 1.0));
    let y_of =
        |v: f64| rect.bottom() - 6.0 - ((v - lo) / (hi - lo)) as f32 * (rect.height() - 20.0);

    // Video coverage band.
    if let Some(v) = &app.state.video {
        let off = app.state.project.sync.offset;
        let r = Rect::from_min_max(
            pos2(x_of(off).max(rect.left()), rect.top()),
            pos2(x_of(off + v.duration).min(rect.right()), rect.bottom()),
        );
        if r.width() > 0.0 {
            painter.rect_filled(r, 0.0, theme::VIDEO_COLOR.gamma_multiply(0.25));
        }
    }
    let n = (rect.width() / 1.5) as usize;
    let pts: Vec<Pos2> = (0..=n)
        .filter_map(|i| {
            let t = dur * i as f64 / n as f64;
            track.value(metric, t).map(|v| pos2(x_of(t), y_of(v)))
        })
        .collect();
    painter.add(Shape::line(pts, Stroke::new(1.4, theme::GPS_COLOR)));
    painter.text(
        rect.left_top() + vec2(6.0, 4.0),
        egui::Align2::LEFT_TOP,
        format!(
            "{} over the whole recording — click to pick a GPS moment",
            metric.label()
        ),
        egui::FontId::proportional(11.0),
        ui.visuals().weak_text_color(),
    );

    // Current playhead mapped into GPS time.
    let gx = x_of(app.state.gps_time());
    painter.line_segment(
        [pos2(gx, rect.top()), pos2(gx, rect.bottom())],
        Stroke::new(1.5, theme::ACCENT),
    );
    if let Some(p) = app.ui.sync.picked_gps {
        let x = x_of(p);
        painter.line_segment(
            [pos2(x, rect.top()), pos2(x, rect.bottom())],
            Stroke::new(2.0, theme::WARN),
        );
    }
    for e in &track.events {
        let x = x_of(e.t);
        painter.circle_filled(pos2(x, rect.bottom() - 4.0), 2.5, Color32::from_gray(180));
    }
    if let Some(hp) = resp.hover_pos() {
        let t = t_of(hp.x);
        let label = format_wall_time(track.wall_time(t), false);
        painter.line_segment(
            [pos2(hp.x, rect.top()), pos2(hp.x, rect.bottom())],
            Stroke::new(1.0, Color32::from_white_alpha(60)),
        );
        painter.text(
            pos2(hp.x + 4.0, rect.bottom() - 16.0),
            egui::Align2::LEFT_BOTTOM,
            label,
            egui::FontId::monospace(10.0),
            Color32::WHITE,
        );
    }
    if (resp.clicked() || resp.dragged())
        && let Some(p) = resp.interact_pointer_pos()
    {
        app.ui.sync.picked_gps = Some(t_of(p.x));
    }
}
