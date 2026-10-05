//! Synchronization window: visual event sync with a route map and an interactive speed graph of
//! the whole recording, with laps and multi-sport segments marked on both.

use egui::{Color32, FontId, Pos2, Rect, Response, RichText, Sense, Shape, Stroke, pos2, vec2};

use super::theme;
use crate::app::application::{GaugeApp, ToastKind};
use crate::app::commands::Command;
use crate::telemetry::model::EventKind;
use crate::telemetry::units::{to_display, unit_label};
use crate::telemetry::{Metric, Track};
use crate::utils::timecode::{format_offset, format_timecode, format_wall_time};

/// Font size for times and offsets in this panel.
const BIG: f32 = 20.0;
const MEDIUM: f32 = 15.0;
const VIEW_H: f32 = 300.0;
const LAP_COLOR: Color32 = theme::ACCENT_2;
/// Per-sport colors for multi-sport recordings.
const SESSION_COLORS: [Color32; 5] = [
    theme::GPS_COLOR,
    Color32::from_rgb(200, 120, 255),
    theme::GAUGE_COLOR,
    theme::OK,
    Color32::from_rgb(240, 110, 170),
];

const MAX_MAP_ZOOM: f32 = 64.0;

#[derive(Default)]
pub struct SyncState {
    /// GPS time picked on the graph or map (seconds from track start).
    pub picked_gps: Option<f64>,
    /// Route map magnification (values below 1 mean "fit the whole route").
    pub map_zoom: f32,
    /// Route map center in route meters (`None` = middle of the route).
    pub map_center: Option<[f64; 2]>,
}

pub fn window(app: &mut GaugeApp, ctx: &egui::Context) {
    let mut open = app.ui.show_sync;
    egui::Window::new("Synchronize GPS with Video")
        .open(&mut open)
        .default_width(900.0)
        .resizable(true)
        .collapsible(false)
        // Scroll rather than overflow on short screens.
        .vscroll(true)
        .show(ctx, |ui| contents(app, ui));
    // `contents` may close the window itself (Sync button).
    if !open {
        app.ui.show_sync = false;
    }
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

    // Status: large readouts of the current mapping.
    ui.horizontal(|ui| {
        // Video time with step buttons; the playhead only moves here or on the timeline.
        let fd = 1.0 / app.state.fps();
        step_button(app, ui, "−1s", -1.0, "Move the video back 1 second");
        step_button(app, ui, "−", -fd, "Move the video back one frame");
        readout(
            ui,
            "VIDEO",
            &format_timecode(app.state.playhead),
            ui.visuals().text_color(),
        );
        step_button(app, ui, "+", fd, "Move the video forward one frame");
        step_button(app, ui, "+1s", 1.0, "Move the video forward 1 second");
        ui.add_space(12.0);
        readout(
            ui,
            "GPS",
            &format_wall_time(track.wall_time(gps_now), true),
            theme::GPS_COLOR,
        );
        ui.add_space(12.0);
        readout(ui, "OFFSET", &format_offset(offset), theme::ACCENT);
        if let Some(v) = track.value(Metric::Speed, gps_now) {
            ui.add_space(12.0);
            readout(
                ui,
                "SPEED",
                &format!(
                    "{:.1} {}",
                    to_display(Metric::Speed, v, units),
                    unit_label(Metric::Speed, units)
                ),
                ui.visuals().weak_text_color(),
            );
        }
    });
    ui.separator();

    ui.label(RichText::new("Match an event").strong().size(MEDIUM + 1.0));
    ui.label(RichText::new("Pick a recognizable moment (a lap or transition, starting off, a stop, a turn) on the GPS graph, the map or the detected events. Move the video to the same moment with the buttons above or the timeline, then press Sync to link the two.").weak());

    // Detected events (laps, sport changes, starts/stops) in a dropdown.
    let events = event_labels(&track);
    if !events.is_empty() {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Detected:").size(MEDIUM));
            let picked = app.ui.sync.picked_gps;
            let selected = events
                .iter()
                .find(|(t, _)| picked.is_some_and(|p| (p - t).abs() < 1e-6))
                .map(|(_, l)| l.clone())
                .unwrap_or_else(|| format!("Jump to an event… ({})", events.len()));
            let mut chosen = None;
            egui::ComboBox::from_id_salt("sync_detected_events")
                .selected_text(RichText::new(selected).monospace().size(MEDIUM))
                .width(340.0)
                .height(360.0)
                .show_ui(ui, |ui| {
                    for (t, label) in &events {
                        let on = picked.is_some_and(|p| (p - t).abs() < 1e-6);
                        if ui
                            .selectable_label(on, RichText::new(label).monospace().size(MEDIUM))
                            .clicked()
                        {
                            chosen = Some(*t);
                        }
                    }
                });
            if let Some(t) = chosen {
                app.ui.sync.picked_gps = Some(t);
                center_map_on(app, &track, t);
            }
        });
    }
    ui.add_space(4.0);

    // Route map + speed graph, sharing hover and pick.
    // One block is reserved for both views so the layout below always starts underneath them.
    let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), VIEW_H), Sense::hover());
    let gap = 8.0;
    let map = track.route.is_some().then(|| {
        let side = VIEW_H.min(row.width() * 0.4);
        let rect = Rect::from_min_size(row.min, vec2(side, VIEW_H));
        (
            rect,
            ui.interact(rect, ui.id().with("sync_map"), Sense::click_and_drag()),
        )
    });
    let graph_left = map.as_ref().map_or(row.left(), |(r, _)| r.right() + gap);
    let graph_rect = Rect::from_min_max(pos2(graph_left, row.top()), row.max);
    let graph = (
        graph_rect,
        ui.interact(
            graph_rect,
            ui.id().with("sync_graph"),
            Sense::click_and_drag(),
        ),
    );
    if let Some((rect, resp)) = &map {
        map_navigation(app, ui, &track, *rect, resp);
    }
    let map_view = map
        .as_ref()
        .and_then(|(r, _)| MapView::new(&track, *r, &app.ui.sync));
    let graph_view = GraphView::new(&track, graph.0);

    let mut hover_t = graph.1.hover_pos().map(|p| graph_view.t_of(p.x));
    if let (Some(mv), Some((_, resp))) = (&map_view, &map)
        && hover_t.is_none()
    {
        hover_t = resp.hover_pos().and_then(|p| mv.nearest_time(&track, p));
    }
    // Picking only selects a GPS moment; the offset and playhead change on Sync.
    if let Some(p) = picked_pos(&graph.1) {
        app.ui.sync.picked_gps = Some(graph_view.t_of(p.x));
    }
    if let (Some(mv), Some((_, resp))) = (&map_view, &map)
        && let Some(p) = picked_pos(resp)
        && let Some(t) = mv.nearest_time(&track, p)
    {
        app.ui.sync.picked_gps = Some(t);
    }
    let video_span = app
        .state
        .video
        .as_ref()
        .map(|v| (offset, offset + v.duration));
    let marks = Marks {
        playhead: gps_now,
        picked: app.ui.sync.picked_gps,
        hover: hover_t,
        video_span,
    };
    if let Some(mv) = &map_view {
        mv.paint(ui, &track, &marks);
    }
    graph_view.paint(ui, &track, &marks, units);
    legend(ui, &track);
    ui.add_space(4.0);
    ui.separator();

    // Sync timestamps and actions, directly under the map and chart.
    let picked = app.ui.sync.picked_gps;
    ui.horizontal(|ui| {
        readout(
            ui,
            "VIDEO TIME",
            &format_timecode(app.state.playhead),
            ui.visuals().text_color(),
        );
        ui.label(RichText::new("↔").size(BIG));
        match picked {
            Some(p) => {
                readout(
                    ui,
                    "PICKED GPS",
                    &format_wall_time(track.wall_time(p), true),
                    theme::WARN,
                );
                ui.add_space(12.0);
                let new = crate::telemetry::sync::event_offset(app.state.playhead, p);
                readout(ui, "OFFSET ON SYNC", &format_offset(new), theme::ACCENT);
            }
            None => readout(
                ui,
                "PICKED GPS",
                "none — Sync keeps the current offset",
                ui.visuals().weak_text_color(),
            ),
        }
    });
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        let sync = egui::Button::new(
            RichText::new("✔ Sync")
                .strong()
                .size(BIG)
                .color(Color32::WHITE),
        )
        .fill(theme::ACCENT)
        .min_size(vec2(140.0, 38.0));
        if ui
            .add(sync)
            .on_hover_text("Link the picked GPS moment to the current video time, save and close")
            .clicked()
        {
            if let Some(gm) = picked {
                let vm = app.state.playhead;
                app.state.undo.break_merge();
                app.state.execute(Command::SetSyncMarks {
                    video: Some(vm),
                    gps: Some(gm),
                });
                app.state
                    .execute(Command::SetOffset(crate::telemetry::sync::event_offset(
                        vm, gm,
                    )));
            }
            let off = app.state.project.sync.offset;
            app.toast(
                ToastKind::Success,
                format!("Synchronized (offset {})", format_offset(off)),
            );
            app.ui.show_sync = false;
        }
        ui.add_space(8.0);
        if ui
            .add_enabled(
                picked.is_some(),
                egui::Button::new(RichText::new("Clear pick").size(MEDIUM))
                    .min_size(vec2(0.0, 38.0)),
            )
            .clicked()
        {
            app.ui.sync.picked_gps = None;
            app.state.execute(Command::SetSyncMarks {
                video: None,
                gps: None,
            });
        }
    });
}

/// Moves the video playhead by `d` seconds.
fn step_button(app: &mut GaugeApp, ui: &mut egui::Ui, text: &str, d: f64, tip: &str) {
    if ui
        .add(egui::Button::new(RichText::new(text).size(MEDIUM)).min_size(vec2(36.0, 30.0)))
        .on_hover_text(tip)
        .clicked()
    {
        app.set_playhead(app.state.playhead + d);
    }
}

/// Dropdown entries for the detected events: (GPS time, label), laps numbered.
fn event_labels(track: &Track) -> Vec<(f64, String)> {
    let mut lap = 1;
    let mut session = 0;
    track
        .events
        .iter()
        .map(|e| {
            let name = match e.kind {
                EventKind::LapStart => {
                    lap += 1;
                    format!("Lap {lap} start")
                }
                EventKind::SportStart => {
                    session += 1;
                    let sport = track
                        .sessions
                        .get(session)
                        .and_then(|s| s.sport.as_deref())
                        .unwrap_or("Next sport");
                    format!("→ {sport}")
                }
                k => k.label().to_string(),
            };
            let wall = format_wall_time(track.wall_time(e.t), false);
            (e.t, format!("{wall}  {name}"))
        })
        .collect()
}

/// Scrolls the route map to the position at GPS time `t`, zooming in if it shows the whole route.
fn center_map_on(app: &mut GaugeApp, track: &Track, t: f64) {
    let Some(route) = track.route.as_ref() else {
        return;
    };
    let p = match (
        track.value(Metric::Latitude, t),
        track.value(Metric::Longitude, t),
    ) {
        (Some(lat), Some(lon)) => route.project(lat, lon),
        _ => match route.points.get(route.index_at(t)) {
            Some(&p) => p,
            None => return,
        },
    };
    let s = &mut app.ui.sync;
    s.map_zoom = s.map_zoom.max(4.0);
    s.map_center = Some(p);
}

/// Zoom buttons, scroll-wheel zoom and right/middle-drag panning for the route map.
fn map_navigation(
    app: &mut GaugeApp,
    ui: &mut egui::Ui,
    track: &Track,
    rect: Rect,
    resp: &Response,
) {
    let Some(route) = track.route.as_ref() else {
        return;
    };
    let fit = |s: &SyncState| MapView::new(track, rect, s);
    let zoom_by = |s: &mut SyncState, factor: f32, anchor: Option<Pos2>| {
        let Some(before) = fit(s) else { return };
        let anchor = anchor.unwrap_or(before.inner_center);
        let fixed = before.to_route(anchor);
        s.map_zoom = (s.map_zoom.max(1.0) * factor).clamp(1.0, MAX_MAP_ZOOM);
        let scale = before.base_scale * s.map_zoom;
        let d = (anchor - before.inner_center) / scale;
        s.map_center = Some([fixed[0] - d.x as f64, fixed[1] - d.y as f64]);
    };

    // Scroll to zoom around the cursor.
    if let Some(hp) = resp.hover_pos() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll.abs() > 0.0 {
            // Zoom the map instead of scrolling the window.
            ui.input_mut(|i| i.smooth_scroll_delta = egui::Vec2::ZERO);
            zoom_by(&mut app.ui.sync, (scroll / 200.0).exp(), Some(hp));
        }
    }
    // Pan with the secondary or middle button (primary picks).
    if (resp.dragged_by(egui::PointerButton::Secondary)
        || resp.dragged_by(egui::PointerButton::Middle))
        && let Some(view) = fit(&app.ui.sync)
    {
        let d = resp.drag_delta() / view.scale;
        let c = view.center;
        app.ui.sync.map_center = Some([c[0] - d.x as f64, c[1] - d.y as f64]);
    }

    // Overlay buttons.
    let size = vec2(26.0, 26.0);
    let mut pos = pos2(rect.right() - 6.0 - size.x, rect.top() + 6.0);
    let mut button = |ui: &mut egui::Ui, text: &str, tip: &str| {
        let r = Rect::from_min_size(pos, size);
        pos.y += size.y + 4.0;
        ui.put(
            r,
            egui::Button::new(RichText::new(text).size(16.0).strong()),
        )
        .on_hover_text(tip)
        .clicked()
    };
    if button(ui, "+", "Zoom in (or scroll on the map)") {
        zoom_by(&mut app.ui.sync, 2.0, None);
    }
    if button(ui, "−", "Zoom out") {
        zoom_by(&mut app.ui.sync, 0.5, None);
    }
    if button(ui, "⌖", "Center on the playhead") {
        let t = app.state.gps_time();
        if let Some(view) = fit(&app.ui.sync)
            && let Some(p) = view.pos_at(track, t)
        {
            app.ui.sync.map_center = Some(view.to_route(p));
        }
    }
    if button(ui, "⟲", "Show the whole route") {
        app.ui.sync.map_zoom = 1.0;
        app.ui.sync.map_center = None;
    }
    if app.ui.sync.map_zoom <= 1.0 {
        app.ui.sync.map_center = None;
    }
    // Keep the center on the route's bounding box.
    if let Some(c) = &mut app.ui.sync.map_center {
        c[0] = c[0].clamp(0.0, route.width);
        c[1] = c[1].clamp(0.0, route.height);
    }
}

fn readout(ui: &mut egui::Ui, label: &str, value: &str, color: Color32) {
    ui.vertical(|ui| {
        ui.label(RichText::new(label).small().weak());
        ui.label(
            RichText::new(value)
                .monospace()
                .size(BIG)
                .strong()
                .color(color),
        );
    });
}

fn picked_pos(resp: &Response) -> Option<Pos2> {
    if resp.clicked() || resp.dragged_by(egui::PointerButton::Primary) {
        resp.interact_pointer_pos()
    } else {
        None
    }
}

fn session_color(i: usize) -> Color32 {
    SESSION_COLORS[i % SESSION_COLORS.len()]
}

/// Index of the sport segment containing `t` (0 when the track has none).
fn session_at(track: &Track, t: f64) -> usize {
    track
        .sessions
        .partition_point(|s| s.start <= t)
        .saturating_sub(1)
}

struct Marks {
    playhead: f64,
    picked: Option<f64>,
    hover: Option<f64>,
    video_span: Option<(f64, f64)>,
}

fn legend(ui: &mut egui::Ui, track: &Track) {
    ui.horizontal_wrapped(|ui| {
        let item = |ui: &mut egui::Ui, color: Color32, text: &str| {
            ui.label(RichText::new("■").color(color));
            ui.label(RichText::new(text).small().weak());
            ui.add_space(6.0);
        };
        item(ui, theme::ACCENT, "playhead");
        item(ui, theme::WARN, "picked GPS moment");
        item(ui, theme::VIDEO_COLOR, "video coverage");
        if track.laps.len() > 1 {
            item(ui, LAP_COLOR, "lap start (solid) / stop (dashed)");
        }
        if track.sessions.len() > 1 {
            for (i, s) in track.sessions.iter().enumerate() {
                item(
                    ui,
                    session_color(i),
                    s.sport.as_deref().unwrap_or("Segment"),
                );
            }
        }
    });
}

/// The route fitted into a rectangle, zoomed and centered as in [`SyncState`].
struct MapView {
    rect: Rect,
    inner_center: Pos2,
    /// Pixels per meter when the whole route fits.
    base_scale: f32,
    scale: f32,
    /// Route point (meters) shown at `inner_center`.
    center: [f64; 2],
}

impl MapView {
    fn new(track: &Track, rect: Rect, state: &SyncState) -> Option<Self> {
        let route = track.route.as_ref()?;
        if route.points.len() < 2 {
            return None;
        }
        let inner = rect.shrink(14.0);
        let (w, h) = (route.width.max(1.0), route.height.max(1.0));
        let base_scale = (inner.width() / w as f32).min(inner.height() / h as f32);
        Some(Self {
            rect,
            inner_center: inner.center(),
            base_scale,
            scale: base_scale * state.map_zoom.clamp(1.0, MAX_MAP_ZOOM),
            center: state.map_center.unwrap_or([w / 2.0, h / 2.0]),
        })
    }

    fn to_screen(&self, p: [f64; 2]) -> Pos2 {
        self.inner_center
            + vec2(
                (p[0] - self.center[0]) as f32,
                (p[1] - self.center[1]) as f32,
            ) * self.scale
    }

    fn to_route(&self, p: Pos2) -> [f64; 2] {
        let d = (p - self.inner_center) / self.scale;
        [self.center[0] + d.x as f64, self.center[1] + d.y as f64]
    }

    /// Screen position of the track at `t`.
    fn pos_at(&self, track: &Track, t: f64) -> Option<Pos2> {
        let route = track.route.as_ref()?;
        match (
            track.value(Metric::Latitude, t),
            track.value(Metric::Longitude, t),
        ) {
            (Some(lat), Some(lon)) => Some(self.to_screen(route.project(lat, lon))),
            _ => route
                .points
                .get(route.index_at(t))
                .map(|&p| self.to_screen(p)),
        }
    }

    /// Track time of the route point nearest to a screen position.
    fn nearest_time(&self, track: &Track, p: Pos2) -> Option<f64> {
        let route = track.route.as_ref()?;
        route
            .points
            .iter()
            .zip(&route.times)
            .map(|(&q, &t)| (self.to_screen(q).distance_sq(p), t))
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, t)| t)
    }

    fn paint(&self, ui: &egui::Ui, track: &Track, marks: &Marks) {
        let Some(route) = track.route.as_ref() else {
            return;
        };
        let painter = ui.painter_at(self.rect);
        painter.rect_filled(self.rect, 4.0, ui.visuals().extreme_bg_color);
        let pts: Vec<Pos2> = route.points.iter().map(|&p| self.to_screen(p)).collect();

        // Video coverage underlay.
        if let Some((a, b)) = marks.video_span {
            let covered: Vec<Pos2> = pts
                .iter()
                .zip(&route.times)
                .filter(|&(_, &t)| t >= a && t <= b)
                .map(|(p, _)| *p)
                .collect();
            if covered.len() > 1 {
                painter.add(Shape::line(
                    covered,
                    Stroke::new(7.0, theme::VIDEO_COLOR.gamma_multiply(0.5)),
                ));
            }
        }

        // Route, colored per sport segment.
        let mut run: Vec<Pos2> = Vec::new();
        let mut run_session = session_at(track, route.times[0]);
        for (p, &t) in pts.iter().zip(&route.times) {
            let s = session_at(track, t);
            if s != run_session && !run.is_empty() {
                let last = *run.last().unwrap();
                run.push(*p);
                painter.add(Shape::line(
                    std::mem::take(&mut run),
                    Stroke::new(2.0, session_color(run_session)),
                ));
                run.push(last);
                run_session = s;
            }
            run.push(*p);
        }
        if run.len() > 1 {
            painter.add(Shape::line(
                run,
                Stroke::new(2.0, session_color(run_session)),
            ));
        }

        // Start / finish.
        painter.circle_filled(pts[0], 5.0, theme::OK);
        painter.circle_filled(*pts.last().unwrap(), 5.0, theme::ERROR);

        // Laps; labels are skipped where they would overlap one already drawn.
        let mut labeled: Vec<Pos2> = Vec::new();
        for (i, lap) in track.laps.iter().enumerate().skip(1) {
            if let Some(p) = self.pos_at(track, lap.start) {
                painter.circle(
                    p,
                    4.0,
                    ui.visuals().extreme_bg_color,
                    Stroke::new(1.5, LAP_COLOR),
                );
                if labeled.iter().any(|q| q.distance(p) < 22.0) {
                    continue;
                }
                labeled.push(p);
                painter.text(
                    p + vec2(6.0, -6.0),
                    egui::Align2::LEFT_BOTTOM,
                    format!("L{}", i + 1),
                    FontId::proportional(12.0),
                    LAP_COLOR,
                );
            }
        }
        // Sport changes.
        for (i, s) in track.sessions.iter().enumerate().skip(1) {
            if let Some(p) = self.pos_at(track, s.start) {
                let c = session_color(i);
                painter.add(Shape::convex_polygon(
                    vec![
                        p + vec2(0.0, -7.0),
                        p + vec2(7.0, 0.0),
                        p + vec2(0.0, 7.0),
                        p + vec2(-7.0, 0.0),
                    ],
                    c,
                    Stroke::new(1.0, Color32::WHITE),
                ));
                painter.text(
                    p + vec2(9.0, 6.0),
                    egui::Align2::LEFT_TOP,
                    s.sport.as_deref().unwrap_or("Segment"),
                    FontId::proportional(12.0),
                    c,
                );
            }
        }

        if let Some(p) = marks.hover.and_then(|t| self.pos_at(track, t)) {
            painter.circle_stroke(p, 6.0, Stroke::new(1.5, Color32::from_white_alpha(160)));
        }
        if let Some(p) = marks.picked.and_then(|t| self.pos_at(track, t)) {
            painter.circle(p, 6.0, theme::WARN, Stroke::new(1.5, Color32::BLACK));
        }
        if let Some(p) = self.pos_at(track, marks.playhead) {
            painter.circle(p, 5.0, theme::ACCENT, Stroke::new(1.5, Color32::WHITE));
        }
        painter.text(
            self.rect.left_top() + vec2(6.0, 4.0),
            egui::Align2::LEFT_TOP,
            "Route — click to pick, right-drag to pan",
            FontId::proportional(12.0),
            ui.visuals().weak_text_color(),
        );
    }
}

/// Speed (or altitude) over the whole recording.
struct GraphView {
    rect: Rect,
    dur: f64,
    metric: Metric,
    lo: f64,
    hi: f64,
}

impl GraphView {
    fn new(track: &Track, rect: Rect) -> Self {
        let metric = if track.has(Metric::Speed) {
            Metric::Speed
        } else {
            Metric::Altitude
        };
        let (lo, hi) = track
            .stats
            .get(metric)
            .map(|s| (s.min.min(0.0), s.max.max(0.1)))
            .unwrap_or((0.0, 1.0));
        Self {
            rect,
            dur: track.duration().max(1.0),
            metric,
            lo,
            hi,
        }
    }

    fn x_of(&self, t: f64) -> f32 {
        self.rect.left() + (t / self.dur) as f32 * self.rect.width()
    }

    fn t_of(&self, x: f32) -> f64 {
        (((x - self.rect.left()) / self.rect.width()) as f64 * self.dur).clamp(0.0, self.dur)
    }

    fn y_of(&self, v: f64) -> f32 {
        let r = self.rect;
        r.bottom() - 8.0 - ((v - self.lo) / (self.hi - self.lo)) as f32 * (r.height() - 40.0)
    }

    fn vline(&self, painter: &egui::Painter, t: f64, stroke: Stroke) {
        let x = self.x_of(t);
        painter.line_segment(
            [pos2(x, self.rect.top()), pos2(x, self.rect.bottom())],
            stroke,
        );
    }

    fn paint(
        &self,
        ui: &egui::Ui,
        track: &Track,
        marks: &Marks,
        units: crate::telemetry::units::UnitSystem,
    ) {
        let rect = self.rect;
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 4.0, ui.visuals().extreme_bg_color);

        // Sport segments as tinted bands.
        if track.sessions.len() > 1 {
            for (i, s) in track.sessions.iter().enumerate() {
                let band = Rect::from_min_max(
                    pos2(self.x_of(s.start).max(rect.left()), rect.top()),
                    pos2(self.x_of(s.end).min(rect.right()), rect.bottom()),
                );
                if band.width() > 0.0 {
                    painter.rect_filled(band, 0.0, session_color(i).gamma_multiply(0.10));
                    painter.text(
                        pos2(band.left() + 4.0, rect.top() + 20.0),
                        egui::Align2::LEFT_TOP,
                        s.sport.as_deref().unwrap_or("Segment"),
                        FontId::proportional(13.0),
                        session_color(i),
                    );
                }
            }
        }

        // Video coverage band.
        if let Some((a, b)) = marks.video_span {
            let r = Rect::from_min_max(
                pos2(self.x_of(a).max(rect.left()), rect.top()),
                pos2(self.x_of(b).min(rect.right()), rect.bottom()),
            );
            if r.width() > 0.0 {
                painter.rect_filled(r, 0.0, theme::VIDEO_COLOR.gamma_multiply(0.25));
            }
        }

        // Laps: start = solid line + label; stop = dashed line when it doesn't meet the next lap.
        // Labels are skipped where laps are too close together to read.
        let mut last_label_x = f32::NEG_INFINITY;
        for (i, lap) in track.laps.iter().enumerate() {
            if i > 0 {
                self.vline(
                    &painter,
                    lap.start,
                    Stroke::new(1.2, LAP_COLOR.gamma_multiply(0.8)),
                );
            }
            let x = self.x_of(lap.start);
            if i > 0 && x - last_label_x >= 30.0 {
                last_label_x = x;
                painter.text(
                    pos2(x + 3.0, rect.top() + 36.0),
                    egui::Align2::LEFT_TOP,
                    format!("L{}", i + 1),
                    FontId::proportional(12.0),
                    LAP_COLOR,
                );
            }
            let next_start = track.laps.get(i + 1).map(|l| l.start).unwrap_or(self.dur);
            if (next_start - lap.end).abs() > 1.0 && lap.end > lap.start {
                let x = self.x_of(lap.end);
                painter.add(Shape::dashed_line(
                    &[pos2(x, rect.top()), pos2(x, rect.bottom())],
                    Stroke::new(1.2, LAP_COLOR.gamma_multiply(0.8)),
                    4.0,
                    3.0,
                ));
            }
        }
        // Sport changes.
        for (i, s) in track.sessions.iter().enumerate().skip(1) {
            self.vline(&painter, s.start, Stroke::new(2.5, session_color(i)));
        }

        // The curve.
        let n = (rect.width() / 1.5) as usize;
        let pts: Vec<Pos2> = (0..=n)
            .filter_map(|i| {
                let t = self.dur * i as f64 / n as f64;
                track
                    .value(self.metric, t)
                    .map(|v| pos2(self.x_of(t), self.y_of(v)))
            })
            .collect();
        painter.add(Shape::line(pts, Stroke::new(1.6, theme::GPS_COLOR)));
        painter.text(
            rect.left_top() + vec2(6.0, 4.0),
            egui::Align2::LEFT_TOP,
            format!(
                "{} over the whole recording — click to pick a GPS moment",
                self.metric.label()
            ),
            FontId::proportional(12.0),
            ui.visuals().weak_text_color(),
        );
        painter.text(
            rect.right_top() + vec2(-6.0, 4.0),
            egui::Align2::RIGHT_TOP,
            format!(
                "max {:.0} {}",
                to_display(self.metric, self.hi, units),
                unit_label(self.metric, units)
            ),
            FontId::proportional(12.0),
            ui.visuals().weak_text_color(),
        );

        for e in &track.events {
            painter.circle_filled(
                pos2(self.x_of(e.t), rect.bottom() - 4.0),
                2.5,
                Color32::from_gray(180),
            );
        }
        self.vline(&painter, marks.playhead, Stroke::new(1.8, theme::ACCENT));
        if let Some(p) = marks.picked {
            self.vline(&painter, p, Stroke::new(2.2, theme::WARN));
        }
        if let Some(t) = marks.hover {
            self.vline(&painter, t, Stroke::new(1.0, Color32::from_white_alpha(70)));
            let mut label = format_wall_time(track.wall_time(t), false);
            if let Some(v) = track.value(self.metric, t) {
                label += &format!(
                    "  {:.1} {}",
                    to_display(self.metric, v, units),
                    unit_label(self.metric, units)
                );
            }
            let x = self.x_of(t);
            let (anchor, dx) = if x > rect.right() - 160.0 {
                (egui::Align2::RIGHT_BOTTOM, -6.0)
            } else {
                (egui::Align2::LEFT_BOTTOM, 6.0)
            };
            painter.text(
                pos2(x + dx, rect.bottom() - 14.0),
                anchor,
                label,
                FontId::monospace(14.0),
                Color32::WHITE,
            );
        }
    }
}
