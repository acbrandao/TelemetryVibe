//! Simplified NLE timeline: transport, ruler, video/GPS/gauge tracks, scrubbing, zoom,
//! and direct drag of the GPS track to synchronize.

use egui::{
    Align, Color32, CursorIcon, Layout, Pos2, Rect, RichText, Sense, Shape, Stroke, StrokeKind,
    pos2, vec2,
};

use super::theme;
use crate::app::application::GaugeApp;
use crate::app::commands::Command;
use crate::telemetry::Metric;
use crate::utils::nice_step;
use crate::utils::timecode::{
    format_duration_human, format_offset, format_ruler, format_timecode, format_wall_time,
    parse_offset,
};
use crate::video::encoder::TimeRange;

#[derive(Clone, Copy, Debug)]
enum TlDrag {
    Scrub,
    Gps {
        start_offset: f64,
        start_x: f32,
    },
    /// Dragging the render range in (start) or out (end) point.
    TrimIn,
    TrimOut,
}

#[derive(Default)]
pub struct TimelineState {
    drag: Option<TlDrag>,
}

const HEADER_W: f32 = 84.0;
const RULER_H: f32 = 22.0;
const VIDEO_H: f32 = 50.0;
const GPS_H: f32 = 46.0;
const GAUGES_H: f32 = 24.0;
const GAP: f32 = 6.0;
/// Horizontal grab distance for trim handles (points).
const HANDLE_GRAB: f32 = 6.0;
const HANDLE_TAB_W: f32 = 9.0;

pub fn show(app: &mut GaugeApp, ui: &mut egui::Ui) {
    transport(app, ui);
    ui.add_space(4.0);
    tracks(app, ui);
}

fn transport(app: &mut GaugeApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        let b = |t: &str| egui::Button::new(RichText::new(t).size(15.0)).min_size(vec2(30.0, 26.0));
        if ui.add(b("⏮")).on_hover_text("Beginning (Home)").clicked() {
            app.set_playhead(0.0);
        }
        if ui
            .add(b("⏴"))
            .on_hover_text("Previous frame (← or ,)")
            .clicked()
        {
            app.step_frames(-1);
        }
        let play = if app.playing { "⏸" } else { "▶" };
        if ui
            .add(
                egui::Button::new(RichText::new(play).size(16.0).color(Color32::WHITE))
                    .fill(theme::ACCENT)
                    .min_size(vec2(38.0, 26.0)),
            )
            .on_hover_text("Play / Pause (Space)")
            .clicked()
        {
            app.toggle_play();
        }
        if ui.add(b("⏹")).on_hover_text("Stop").clicked() {
            app.stop();
        }
        if ui
            .add(b("⏵"))
            .on_hover_text("Next frame (→ or .)")
            .clicked()
        {
            app.step_frames(1);
        }
        if ui.add(b("⏭")).on_hover_text("End (End)").clicked() {
            let d = app.state.duration();
            app.set_playhead(d);
        }
        ui.add_space(8.0);
        ui.label(
            RichText::new(format!("Video {}", format_timecode(app.state.playhead)))
                .monospace()
                .size(13.0),
        );
        ui.label(
            RichText::new(format!("/ {}", format_timecode(app.state.duration())))
                .monospace()
                .size(13.0)
                .weak(),
        );
        if let Some(track) = &app.state.track {
            let gt = app.state.gps_time();
            let wall = format_wall_time(track.wall_time(gt), true);
            ui.label(
                RichText::new(format!("↔  GPS {wall}"))
                    .monospace()
                    .size(13.0)
                    .color(theme::GPS_COLOR),
            );
        }
        ui.separator();
        offset_editor(app, ui);

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let tl = &mut app.state.project.timeline;
            let mut zoom = tl.zoom;
            ui.add(
                egui::Slider::new(&mut zoom, 1.0..=200.0)
                    .logarithmic(true)
                    .show_value(false),
            )
            .on_hover_text("Timeline zoom (⌘/Ctrl + scroll)");
            ui.label("🔍");
            if (zoom - tl.zoom).abs() > 1e-4 {
                let dur = app.state.duration();
                let center = app.state.playhead;
                let tl = &mut app.state.project.timeline;
                tl.zoom = zoom;
                let span = dur / zoom as f64;
                tl.scroll = (center - span / 2.0).clamp(0.0, (dur - span).max(0.0));
            }
        });
    });
}

/// Offset text field (`+00:03.250`) with nudge buttons.
pub fn offset_editor(app: &mut GaugeApp, ui: &mut egui::Ui) {
    ui.label("GPS offset");
    let offset = app.state.project.sync.offset;
    if !app.ui.offset_text_focused {
        app.ui.offset_text = format_offset(offset);
    }
    let resp = ui.add(
        egui::TextEdit::singleline(&mut app.ui.offset_text)
            .desired_width(96.0)
            .font(egui::TextStyle::Monospace),
    );
    app.ui.offset_text_focused = resp.has_focus();
    if resp.lost_focus() {
        match parse_offset(&app.ui.offset_text) {
            Some(v) => {
                app.state.undo.break_merge();
                app.state.execute(Command::SetOffset(v));
            }
            None => app.ui.offset_text = format_offset(offset),
        }
    }
    resp.on_hover_text("GPS time = video time + offset. Type e.g. +00:03.250 or -12.8");
    let fd = 1.0 / app.state.fps();
    for (label, d, tip) in [
        ("-1s", -1.0, "Shift GPS 1 s earlier"),
        ("-0.1", -0.1, "Shift GPS 100 ms earlier"),
        ("-1f", -fd, "Shift GPS one frame earlier"),
        ("+1f", fd, "Shift GPS one frame later"),
        ("+0.1", 0.1, "Shift GPS 100 ms later"),
        ("+1s", 1.0, "Shift GPS 1 s later"),
    ] {
        if ui.small_button(label).on_hover_text(tip).clicked() {
            app.state
                .execute(Command::SetOffset(app.state.project.sync.offset + d));
        }
    }
}

fn tracks(app: &mut GaugeApp, ui: &mut egui::Ui) {
    let avail = ui.available_rect_before_wrap();
    let height = (RULER_H + VIDEO_H + GPS_H + GAUGES_H + GAP * 4.0).min(avail.height());
    let full = Rect::from_min_size(avail.min, vec2(avail.width(), height));
    ui.allocate_rect(full, Sense::hover());
    let painter = ui.painter_at(full);
    let area = Rect::from_min_max(pos2(full.left() + HEADER_W, full.top()), full.max);
    let dur = app.state.duration();
    let zoom = app.state.project.timeline.zoom.max(1.0) as f64;
    let span = dur / zoom;
    let start = app
        .state
        .project
        .timeline
        .scroll
        .clamp(0.0, (dur - span).max(0.0));
    app.state.project.timeline.scroll = start;
    let x_of = |t: f64| area.left() + ((t - start) / span) as f32 * area.width();
    let t_of = |x: f32| start + ((x - area.left()) / area.width()) as f64 * span;

    let ruler = Rect::from_min_size(area.min, vec2(area.width(), RULER_H));
    let row = |i: usize| -> Rect {
        let mut y = full.top() + RULER_H + GAP;
        let heights = [VIDEO_H, GPS_H, GAUGES_H];
        for h in heights.iter().take(i) {
            y += h + GAP;
        }
        Rect::from_min_size(pos2(area.left(), y), vec2(area.width(), heights[i]))
    };
    let (video_row, gps_row, gauges_row) = (row(0), row(1), row(2));
    let (trim_in, trim_out) = app.trim_bounds();
    let trimmed = app.state.project.trim.is_some();
    let has_video = app.state.video.is_some();
    let weak = ui.visuals().weak_text_color();
    let text = ui.visuals().text_color();
    let track_bg = ui.visuals().extreme_bg_color;

    // Headers.
    for (r, name, color) in [
        (video_row, "VIDEO", theme::VIDEO_COLOR),
        (gps_row, "GPS", theme::GPS_COLOR),
        (gauges_row, "GAUGES", theme::GAUGE_COLOR),
    ] {
        let hr = Rect::from_min_max(
            pos2(full.left() + 6.0, r.top()),
            pos2(area.left() - 6.0, r.bottom()),
        );
        painter.rect_filled(
            Rect::from_min_size(hr.min, vec2(3.0, hr.height())),
            1.0,
            color,
        );
        painter.text(
            pos2(hr.left() + 10.0, hr.center().y),
            egui::Align2::LEFT_CENTER,
            name,
            egui::FontId::proportional(11.0),
            text,
        );
    }

    trim_buttons(
        app,
        ui,
        Rect::from_min_max(
            pos2(full.left() + 4.0, ruler.top() + 1.0),
            pos2(area.left() - 4.0, ruler.bottom() - 1.0),
        ),
    );

    // Ruler.
    let step = nice_step(span, (area.width() / 90.0).max(1.0) as f64);
    let mut t = (start / step).floor() * step;
    while t <= start + span {
        let x = x_of(t);
        if x >= area.left() - 1.0 {
            painter.line_segment(
                [pos2(x, ruler.bottom() - 7.0), pos2(x, ruler.bottom())],
                Stroke::new(1.0, weak),
            );
            painter.text(
                pos2(x + 3.0, ruler.top() + 2.0),
                egui::Align2::LEFT_TOP,
                format_ruler(t),
                egui::FontId::proportional(10.5),
                weak,
            );
            let minor = x_of(t + step / 2.0);
            painter.line_segment(
                [
                    pos2(minor, ruler.bottom() - 3.0),
                    pos2(minor, ruler.bottom()),
                ],
                Stroke::new(1.0, weak),
            );
        }
        t += step;
    }

    // Video track with thumbnails.
    let vx0 = x_of(0.0).max(area.left());
    let vx1 = x_of(dur).min(area.right());
    let vbar = Rect::from_min_max(pos2(vx0, video_row.top()), pos2(vx1, video_row.bottom()));
    painter.rect_filled(video_row, 4.0, track_bg);
    if app.state.video.is_some() {
        painter.rect_filled(vbar, 4.0, theme::VIDEO_COLOR.gamma_multiply(0.35));
        if !app.thumbs.is_empty() {
            let (tw, th) = app.thumbs[0].1.size_vec2().into();
            let tile_h = video_row.height() - 4.0;
            let tile_w = tile_h * tw / th.max(1.0);
            let mut x = vbar.left();
            let thumb_painter = painter.with_clip_rect(vbar.shrink(1.0));
            while x < vbar.right() {
                let tt = t_of(x + tile_w / 2.0);
                if let Some((_, tex)) = app
                    .thumbs
                    .iter()
                    .min_by(|a, b| (a.0 - tt).abs().total_cmp(&(b.0 - tt).abs()))
                {
                    let r =
                        Rect::from_min_size(pos2(x, video_row.top() + 2.0), vec2(tile_w, tile_h));
                    thumb_painter.image(
                        tex.id(),
                        r,
                        Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                        Color32::WHITE,
                    );
                }
                x += tile_w;
            }
        }
        if let Some(v) = &app.state.video {
            painter.text(
                vbar.left_top() + vec2(6.0, 3.0),
                egui::Align2::LEFT_TOP,
                v.file_name(),
                egui::FontId::proportional(10.5),
                Color32::WHITE,
            );
        }
    } else {
        painter.text(
            video_row.left_center() + vec2(8.0, 0.0),
            egui::Align2::LEFT_CENTER,
            "No video — drop one here",
            egui::FontId::proportional(11.0),
            weak,
        );
    }

    // GPS track: positioned by offset (video t = gps t - offset).
    painter.rect_filled(gps_row, 4.0, track_bg);
    let mut gps_bar: Option<Rect> = None;
    if let Some(track) = app.state.track.clone() {
        let offset = app.state.project.sync.offset;
        let g0 = x_of(-offset);
        let g1 = x_of(track.duration() - offset);
        let bar = Rect::from_min_max(
            pos2(g0.max(area.left()), gps_row.top()),
            pos2(g1.min(area.right()), gps_row.bottom()),
        );
        if bar.width() > 0.0 {
            painter.rect_filled(bar, 4.0, theme::GPS_COLOR.gamma_multiply(0.28));
            painter.rect_stroke(
                bar,
                4.0,
                Stroke::new(1.0, theme::GPS_COLOR.gamma_multiply(0.8)),
                StrokeKind::Inside,
            );
            // Speed sparkline.
            if let Some(stats) = track.stats.get(Metric::Speed) {
                let max = stats.max.max(0.1);
                let n = (bar.width() / 2.0).max(2.0) as usize;
                let pts: Vec<Pos2> = (0..=n)
                    .filter_map(|i| {
                        let x = bar.left() + bar.width() * i as f32 / n as f32;
                        let gt = t_of(x) + offset;
                        let v = track.value(Metric::Speed, gt)?;
                        Some(pos2(
                            x,
                            bar.bottom() - 4.0 - (v / max) as f32 * (bar.height() - 16.0),
                        ))
                    })
                    .collect();
                painter.add(Shape::line(pts, Stroke::new(1.3, theme::GPS_COLOR)));
            }
            let label = format!(
                "{}  ·  drag to sync  ·  offset {}",
                track.name,
                format_offset(offset)
            );
            painter.with_clip_rect(bar).text(
                bar.left_top() + vec2(6.0, 3.0),
                egui::Align2::LEFT_TOP,
                label,
                egui::FontId::proportional(10.5),
                text,
            );
            gps_bar = Some(bar);
            // Laps.
            for lap in track.laps.iter().skip(1) {
                let x = x_of(lap.start - offset);
                if bar.x_range().contains(x) {
                    painter.line_segment(
                        [pos2(x, bar.top()), pos2(x, bar.bottom())],
                        Stroke::new(1.0, theme::GPS_COLOR.gamma_multiply(0.6)),
                    );
                }
            }
        }
        // Sync marks.
        if let Some(vm) = app.state.project.sync.video_mark {
            marker(&painter, pos2(x_of(vm), video_row.bottom()), theme::WARN);
        }
        if let Some(gm) = app.state.project.sync.gps_mark {
            marker(
                &painter,
                pos2(x_of(gm - offset), gps_row.bottom()),
                theme::WARN,
            );
        }
    } else {
        painter.text(
            gps_row.left_center() + vec2(8.0, 0.0),
            egui::Align2::LEFT_CENTER,
            "No GPS data — drop a FIT/GPX file",
            egui::FontId::proportional(11.0),
            weak,
        );
    }

    // Gauges track.
    painter.rect_filled(gauges_row, 4.0, track_bg);
    let n = app
        .state
        .project
        .gauges
        .iter()
        .filter(|g| g.visible)
        .count();
    if n > 0 {
        let r = Rect::from_min_max(pos2(vx0, gauges_row.top()), pos2(vx1, gauges_row.bottom()));
        painter.rect_filled(r, 4.0, theme::GAUGE_COLOR.gamma_multiply(0.35));
        let names: Vec<&str> = app
            .state
            .project
            .gauges
            .iter()
            .filter(|g| g.visible)
            .map(|g| g.name.as_str())
            .collect();
        painter.with_clip_rect(r).text(
            r.left_center() + vec2(6.0, 0.0),
            egui::Align2::LEFT_CENTER,
            format!(
                "{n} gauge{} · {}",
                if n == 1 { "" } else { "s" },
                names.join(", ")
            ),
            egui::FontId::proportional(10.5),
            text,
        );
    }

    // Outside-video shading.
    if x_of(dur) < area.right() {
        painter.rect_filled(
            Rect::from_min_max(pos2(x_of(dur).max(area.left()), ruler.bottom()), area.max),
            0.0,
            Color32::from_black_alpha(60),
        );
    }

    // Trim (render range): shade what will not be rendered, draw the draggable in/out handles.
    let x_in = x_of(trim_in);
    let x_out = x_of(trim_out);
    if has_video {
        let tracks_top = ruler.bottom();
        let shade = Color32::from_black_alpha(140);
        if trimmed {
            if x_in > area.left() {
                painter.rect_filled(
                    Rect::from_min_max(
                        pos2(area.left(), tracks_top),
                        pos2(x_in.min(area.right()), full.bottom()),
                    ),
                    0.0,
                    shade,
                );
            }
            let video_end = x_of(dur).min(area.right());
            if x_out < video_end {
                painter.rect_filled(
                    Rect::from_min_max(
                        pos2(x_out.max(area.left()), tracks_top),
                        pos2(video_end, full.bottom()),
                    ),
                    0.0,
                    shade,
                );
            }
            // Band on the ruler marking the rendered section.
            let band = Rect::from_min_max(
                pos2(x_in.max(area.left()), ruler.bottom() - 4.0),
                pos2(x_out.min(area.right()), ruler.bottom()),
            );
            if band.width() > 0.0 {
                painter.rect_filled(band, 0.0, theme::TRIM.gamma_multiply(0.8));
            }
        }
        let active = app.ui.timeline.drag;
        let hovered = ui
            .ctx()
            .pointer_hover_pos()
            .and_then(|p| trim_handle_at(p, x_in, x_out, ruler.top(), full.bottom(), area));
        for (x, is_in) in [(x_in, true), (x_out, false)] {
            if x < area.left() - 1.0 || x > area.right() + 1.0 {
                continue;
            }
            let this = if is_in {
                TlDrag::TrimIn
            } else {
                TlDrag::TrimOut
            };
            let hot = matches!(
                (active, this),
                (Some(TlDrag::TrimIn), TlDrag::TrimIn) | (Some(TlDrag::TrimOut), TlDrag::TrimOut)
            ) || (active.is_none()
                && hovered
                    .is_some_and(|h| std::mem::discriminant(&h) == std::mem::discriminant(&this)));
            let color = if trimmed || hot {
                theme::TRIM
            } else {
                theme::TRIM.gamma_multiply(0.55)
            };
            painter.line_segment(
                [pos2(x, ruler.top()), pos2(x, full.bottom())],
                Stroke::new(if hot { 2.5 } else { 1.5 }, color),
            );
            // Bracket tab on the ruler, pointing into the rendered section.
            let tab = if is_in {
                Rect::from_min_max(pos2(x, ruler.top()), pos2(x + HANDLE_TAB_W, ruler.bottom()))
            } else {
                Rect::from_min_max(pos2(x - HANDLE_TAB_W, ruler.top()), pos2(x, ruler.bottom()))
            };
            let cr = if is_in {
                egui::CornerRadius {
                    nw: 0,
                    sw: 0,
                    ne: 3,
                    se: 3,
                }
            } else {
                egui::CornerRadius {
                    nw: 3,
                    sw: 3,
                    ne: 0,
                    se: 0,
                }
            };
            painter.rect_filled(tab, cr, color);
            for dx in [-1.5, 1.5] {
                let gx = tab.center().x + dx;
                painter.line_segment(
                    [pos2(gx, tab.top() + 6.0), pos2(gx, tab.bottom() - 6.0)],
                    Stroke::new(1.0, Color32::from_black_alpha(150)),
                );
            }
            if hot {
                let t = if is_in { trim_in } else { trim_out };
                let label = format!(
                    "{} {}  ·  {}",
                    if is_in { "Start" } else { "End" },
                    format_timecode(t),
                    format_duration_human(trim_out - trim_in)
                );
                let font = egui::FontId::proportional(10.5);
                let galley = painter.layout_no_wrap(label, font, Color32::BLACK);
                let size = galley.size() + vec2(10.0, 4.0);
                let mut lx = if is_in {
                    tab.right() + 4.0
                } else {
                    tab.left() - 4.0 - size.x
                };
                lx = lx.clamp(area.left(), (area.right() - size.x).max(area.left()));
                let lr = Rect::from_min_size(pos2(lx, ruler.top() + 1.0), size);
                painter.rect_filled(lr, 3.0, theme::TRIM);
                painter.galley(lr.min + vec2(5.0, 2.0), galley, Color32::BLACK);
            }
        }
    }

    // Playhead.
    let px = x_of(app.state.playhead);
    if area.x_range().contains(px) {
        painter.line_segment(
            [pos2(px, ruler.top()), pos2(px, full.bottom())],
            Stroke::new(1.5, theme::ACCENT),
        );
        painter.add(Shape::convex_polygon(
            vec![
                pos2(px - 6.0, ruler.top()),
                pos2(px + 6.0, ruler.top()),
                pos2(px, ruler.top() + 9.0),
            ],
            theme::ACCENT,
            Stroke::NONE,
        ));
    }

    // Interaction.
    let resp = ui.interact(area, ui.id().with("timeline_area"), Sense::click_and_drag());
    let handle_at = |p: Pos2| {
        has_video
            .then(|| trim_handle_at(p, x_in, x_out, ruler.top(), full.bottom(), area))
            .flatten()
    };
    if let Some(hp) = resp.hover_pos()
        && app.ui.timeline.drag.is_none()
        && (handle_at(hp).is_some() || gps_bar.is_some_and(|b| b.contains(hp)))
    {
        ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
    }
    if resp.drag_started()
        && let Some(origin) = ui.ctx().input(|i| i.pointer.press_origin())
    {
        app.ui.timeline.drag = if let Some(h) = handle_at(origin) {
            app.state.undo.break_merge();
            Some(h)
        } else if gps_bar.is_some_and(|b| b.contains(origin)) {
            app.state.undo.break_merge();
            Some(TlDrag::Gps {
                start_offset: app.state.project.sync.offset,
                start_x: origin.x,
            })
        } else {
            Some(TlDrag::Scrub)
        };
    }
    if resp.dragged()
        && let Some(p) = resp.interact_pointer_pos()
    {
        match app.ui.timeline.drag {
            Some(TlDrag::Scrub) => app.set_playhead(t_of(p.x)),
            Some(TlDrag::Gps {
                start_offset,
                start_x,
            }) => {
                ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
                // Dragging the GPS bar right makes it start later in video time: offset decreases.
                let dt = ((p.x - start_x) / area.width()) as f64 * span;
                let mut new = start_offset - dt;
                // Shift gives 10× finer control.
                if ui.ctx().input(|i| i.modifiers.shift) {
                    new = start_offset - dt * 0.1;
                }
                new = (new * 1000.0).round() / 1000.0;
                app.state.execute(Command::SetOffset(new));
            }
            Some(drag @ (TlDrag::TrimIn | TlDrag::TrimOut)) => {
                ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
                let mut t = t_of(p.x);
                // Snap to the start/end of the GPS recording (render only where data exists).
                if let Some(track) = &app.state.track {
                    let offset = app.state.project.sync.offset;
                    for edge in [-offset, track.duration() - offset] {
                        if (x_of(edge) - p.x).abs() < HANDLE_GRAB {
                            t = edge;
                        }
                    }
                }
                let t = app.snap_to_frame(t.clamp(0.0, dur));
                let (tin, tout) = app.trim_bounds();
                let (a, b) = if matches!(drag, TlDrag::TrimIn) {
                    (t.min(tout - TimeRange::MIN_LEN).max(0.0), tout)
                } else {
                    (tin, t.max(tin + TimeRange::MIN_LEN).min(dur))
                };
                app.set_trim(a, b);
                // Show the frame at the handle being dragged.
                app.set_playhead(if matches!(drag, TlDrag::TrimIn) { a } else { b });
            }
            None => {}
        }
    }
    if resp.drag_stopped() {
        app.ui.timeline.drag = None;
    }
    if resp.clicked()
        && let Some(p) = resp.interact_pointer_pos()
    {
        app.set_playhead(t_of(p.x));
    }
    if resp.hovered() {
        let (scroll, zoom_mod, hp) = ui.ctx().input(|i| {
            (
                i.smooth_scroll_delta,
                i.modifiers.command,
                i.pointer.hover_pos(),
            )
        });
        let tl = &mut app.state.project.timeline;
        if zoom_mod && scroll.y != 0.0 {
            let anchor_t = hp.map(|p| t_of(p.x)).unwrap_or(app.state.playhead);
            let new_zoom = (tl.zoom * (1.0 + scroll.y * 0.004)).clamp(1.0, 200.0);
            let new_span = dur / new_zoom as f64;
            let frac = hp
                .map(|p| ((p.x - area.left()) / area.width()) as f64)
                .unwrap_or(0.5);
            tl.zoom = new_zoom;
            tl.scroll = (anchor_t - frac * new_span).clamp(0.0, (dur - new_span).max(0.0));
        } else if tl.zoom > 1.0 && (scroll.x != 0.0 || scroll.y != 0.0) {
            let d = if scroll.x != 0.0 { scroll.x } else { scroll.y };
            tl.scroll =
                (tl.scroll - (d / area.width()) as f64 * span).clamp(0.0, (dur - span).max(0.0));
        }
    }
    // Keep the playhead visible during playback.
    if app.playing && (app.state.playhead > start + span || app.state.playhead < start) {
        app.state.project.timeline.scroll = app.state.playhead.clamp(0.0, (dur - span).max(0.0));
    }
}

fn marker(painter: &egui::Painter, p: Pos2, c: Color32) {
    painter.add(Shape::convex_polygon(
        vec![p, p + vec2(-5.0, -8.0), p + vec2(5.0, -8.0)],
        c,
        Stroke::NONE,
    ));
}

/// Which trim handle (if any) is under `p`: within a few points of the in/out line, anywhere
/// from the ruler down. The nearer handle wins when both are close.
fn trim_handle_at(
    p: Pos2,
    x_in: f32,
    x_out: f32,
    top: f32,
    bottom: f32,
    area: Rect,
) -> Option<TlDrag> {
    if p.y < top
        || p.y > bottom
        || p.x < area.left() - HANDLE_GRAB
        || p.x > area.right() + HANDLE_GRAB
    {
        return None;
    }
    // The tabs extend inward, so bias each handle's hit zone towards its tab.
    let d_in = (p.x - (x_in + HANDLE_TAB_W / 2.0)).abs() - HANDLE_TAB_W / 2.0;
    let d_out = (p.x - (x_out - HANDLE_TAB_W / 2.0)).abs() - HANDLE_TAB_W / 2.0;
    let (d, h) = if d_in <= d_out {
        (d_in, TlDrag::TrimIn)
    } else {
        (d_out, TlDrag::TrimOut)
    };
    (d <= HANDLE_GRAB / 2.0).then_some(h)
}

/// Set-in / set-out / clear buttons in the ruler's header cell.
fn trim_buttons(app: &mut GaugeApp, ui: &mut egui::Ui, cell: Rect) {
    let has_video = app.state.video.is_some();
    let has_trim = app.state.project.trim.is_some();
    let w = (cell.width() / 3.0).floor();
    let buttons: [(&str, &str, bool); 3] = [
        ("[", "Set render start at the playhead (I)", has_video),
        ("]", "Set render end at the playhead (O)", has_video),
        ("×", "Clear trim: render the whole video (⌥X)", has_trim),
    ];
    for (i, (label, tip, enabled)) in buttons.into_iter().enumerate() {
        let r = Rect::from_min_size(
            pos2(cell.left() + i as f32 * w, cell.top()),
            vec2(w - 2.0, cell.height()),
        );
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(r));
        let btn = egui::Button::new(RichText::new(label).size(11.0).color(theme::TRIM))
            .min_size(r.size());
        if child
            .add_enabled(enabled, btn)
            .on_hover_text(tip)
            .on_disabled_hover_text(tip)
            .clicked()
        {
            match i {
                0 => app.set_trim_in(),
                1 => app.set_trim_out(),
                _ => app.clear_trim(),
            }
        }
    }
}
