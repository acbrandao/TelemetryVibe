//! The video canvas: video frame + gauge overlay with direct manipulation
//! (select, drag, resize, rotate, snapping guides, context menu, library drop target).

use egui::epaint::Mesh;
use egui::{
    Color32, CursorIcon, Pos2, Rect, Response, RichText, Sense, Shape, Stroke, StrokeKind, Vec2,
    emath::Rot2, pos2, vec2,
};

use super::theme;
use crate::app::application::{GaugeApp, RightTab};
use crate::app::commands::Command;
use crate::gauges::library::{PresetId, Template};
use crate::gauges::model::{Gauge, GaugeId, Placement};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    N,
    S,
    E,
    W,
    NE,
    NW,
    SE,
    SW,
    Rotate,
}

#[derive(Clone, Debug)]
enum DragKind {
    Move,
    Resize(Handle),
    Rotate,
}

#[derive(Clone, Debug)]
struct DragOp {
    kind: DragKind,
    origin: (f32, f32),
    originals: Vec<(GaugeId, Placement)>,
}

#[derive(Clone, Copy, Debug)]
enum Guide {
    V(f32),
    H(f32),
}

#[derive(Default)]
pub struct CanvasState {
    drag: Option<DragOp>,
    guides: Vec<Guide>,
}

struct View {
    rect: Rect,
    scale: f32,
}

impl View {
    fn to_screen(&self, x: f32, y: f32) -> Pos2 {
        pos2(
            self.rect.min.x + x * self.scale,
            self.rect.min.y + y * self.scale,
        )
    }
    fn to_video(&self, p: Pos2) -> (f32, f32) {
        (
            (p.x - self.rect.min.x) / self.scale,
            (p.y - self.rect.min.y) / self.scale,
        )
    }
}

const HANDLE: f32 = 7.0;
const ROT_OFFSET: f32 = 26.0;

fn handle_positions(p: &Placement, view: &View) -> Vec<(Handle, Pos2)> {
    let pts = [
        (Handle::NW, 0.0, 0.0),
        (Handle::N, 0.5, 0.0),
        (Handle::NE, 1.0, 0.0),
        (Handle::E, 1.0, 0.5),
        (Handle::SE, 1.0, 1.0),
        (Handle::S, 0.5, 1.0),
        (Handle::SW, 0.0, 1.0),
        (Handle::W, 0.0, 0.5),
    ];
    let mut out: Vec<(Handle, Pos2)> = pts
        .iter()
        .map(|&(h, fx, fy)| {
            let (x, y) = p.to_world(p.w * fx, p.h * fy);
            (h, view.to_screen(x, y))
        })
        .collect();
    // Rotation handle above the top edge (in screen space).
    let (tx, ty) = p.to_world(p.w * 0.5, 0.0);
    let (cx, cy) = p.center();
    let top = view.to_screen(tx, ty);
    let c = view.to_screen(cx, cy);
    let dir = (top - c).normalized();
    let dir = if dir.is_finite() {
        dir
    } else {
        vec2(0.0, -1.0)
    };
    out.push((Handle::Rotate, top + dir * ROT_OFFSET));
    out
}

fn hit_gauge(gauges: &[Gauge], x: f32, y: f32) -> Option<GaugeId> {
    gauges
        .iter()
        .rev()
        .find(|g| g.visible && g.placement.contains(x, y))
        .map(|g| g.id)
}

pub fn show(app: &mut GaugeApp, ui: &mut egui::Ui) {
    let full = ui.available_rect_before_wrap();
    let painter = ui.painter_at(full);
    painter.rect_filled(full, 0.0, theme::canvas_bg(app.settings.theme));

    let (vw, vh) = app.state.video_size();
    let margin = 16.0;
    let avail = full.shrink(margin);
    let scale = (avail.width() / vw).min(avail.height() / vh).max(0.01);
    let vrect = Rect::from_center_size(avail.center(), vec2(vw * scale, vh * scale));
    let view = View { rect: vrect, scale };
    let ppp = ui.ctx().pixels_per_point();
    app.update_canvas_size(vrect.width() * ppp, vrect.height() * ppp);

    let response = ui.interact(full, ui.id().with("canvas"), Sense::click_and_drag());

    // Video frame (or nearest thumbnail while a precise frame decodes, or placeholder).
    painter.rect_filled(vrect, 0.0, Color32::BLACK);
    let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
    let far_from_frame = app
        .video_frame_t
        .is_none_or(|t| (t - app.state.playhead).abs() > 0.6);
    let thumb = if far_from_frame && !app.playing {
        app.thumbs
            .iter()
            .min_by(|a, b| {
                (a.0 - app.state.playhead)
                    .abs()
                    .total_cmp(&(b.0 - app.state.playhead).abs())
            })
            .map(|t| t.1.id())
    } else {
        None
    };
    if let Some(tid) = thumb {
        painter.image(tid, vrect, uv, Color32::WHITE);
    } else if let Some(tex) = &app.video_tex {
        painter.image(tex.id(), vrect, uv, Color32::WHITE);
    } else {
        placeholder(app, &painter, vrect);
    }

    // Gauges.
    app.update_gauge_textures(scale * ppp);
    for g in app.state.project.gauges.iter().filter(|g| g.visible) {
        let Some(t) = app.gauge_tex.get(&g.id) else {
            continue;
        };
        let p = &g.placement;
        let (cx, cy) = p.center();
        let c = view.to_screen(cx, cy);
        let size = vec2(
            (p.w + t.margin * 2.0) * scale,
            (p.h + t.margin * 2.0) * scale,
        );
        let a = (g.opacity.clamp(0.0, 1.0) * 255.0) as u8;
        let mut mesh = Mesh::with_texture(t.tex.id());
        mesh.add_rect_with_uv(
            Rect::from_center_size(c, size),
            uv,
            Color32::from_rgba_premultiplied(a, a, a, a),
        );
        if p.rotation != 0.0 {
            mesh.rotate(Rot2::from_angle(p.rotation.to_radians()), c);
        }
        painter.add(Shape::mesh(mesh));
    }

    // Out-of-range telemetry badge.
    if let Some(track) = &app.state.track {
        let gt = app.state.gps_time();
        if gt < -1.0 || gt > track.duration() + 1.0 {
            let r = painter.text(
                vrect.left_top() + vec2(10.0, 10.0),
                egui::Align2::LEFT_TOP,
                "No GPS data at this time — check Sync",
                egui::FontId::proportional(12.0),
                theme::WARN,
            );
            painter.rect_stroke(
                r.expand(5.0),
                4.0,
                Stroke::new(1.0, theme::WARN),
                StrokeKind::Outside,
            );
        }
    }

    interact(app, ui, &response, &view);
    draw_selection(app, &painter, &view);

    // Library drag-and-drop target highlight.
    if response.dnd_hover_payload::<PresetId>().is_some() {
        painter.rect_stroke(
            vrect,
            0.0,
            Stroke::new(2.0, theme::ACCENT),
            StrokeKind::Inside,
        );
    }
    if let Some(preset) = response.dnd_release_payload::<PresetId>()
        && let Some(pos) = response.hover_pos()
    {
        let (x, y) = view.to_video(pos);
        app.add_preset(*preset, Some((x, y)));
    }
    let hovering_files = ui.ctx().input(|i| !i.raw.hovered_files.is_empty());
    if hovering_files {
        painter.rect_filled(full, 0.0, Color32::from_black_alpha(140));
        painter.text(
            full.center(),
            egui::Align2::CENTER_CENTER,
            "Drop video, GPS or project files",
            egui::FontId::proportional(22.0),
            Color32::WHITE,
        );
    }

    // Template chooser when nothing is placed yet.
    if app.state.project.gauges.is_empty()
        && (app.state.video.is_some() || app.state.track.is_some())
    {
        template_picker(app, ui, vrect);
    }
}

fn placeholder(app: &GaugeApp, painter: &egui::Painter, r: Rect) {
    let c = Color32::from_gray(40);
    let step = 48.0;
    let mut x = r.left();
    while x < r.right() {
        painter.line_segment([pos2(x, r.top()), pos2(x, r.bottom())], Stroke::new(1.0, c));
        x += step;
    }
    let mut y = r.top();
    while y < r.bottom() {
        painter.line_segment([pos2(r.left(), y), pos2(r.right(), y)], Stroke::new(1.0, c));
        y += step;
    }
    let msg = if app.loading_video.is_some() {
        "Loading video…".to_string()
    } else if app.state.project.video.is_some() && app.ffmpeg.is_none() {
        "Waiting for FFmpeg…".to_string()
    } else {
        "Drop your video and GPS file here\nor use Import Video / Import GPS".to_string()
    };
    painter.text(
        r.center(),
        egui::Align2::CENTER_CENTER,
        msg,
        egui::FontId::proportional(18.0),
        Color32::from_gray(150),
    );
}

fn template_picker(app: &mut GaugeApp, ui: &mut egui::Ui, vrect: Rect) {
    let area = Rect::from_center_size(vrect.center(), vec2(460.0, 150.0));
    let mut chosen = None;
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(area));
    egui::Frame::popup(child.style()).show(&mut child, |ui| {
        ui.vertical_centered(|ui| {
            ui.label(RichText::new("Choose a gauge template").heading());
            ui.label(RichText::new("or drag gauges from the library on the right").weak());
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                for t in Template::ALL {
                    if ui
                        .button(RichText::new(t.name()).size(14.0))
                        .on_hover_text(t.description())
                        .clicked()
                    {
                        chosen = Some(t);
                    }
                }
            });
        });
    });
    if let Some(t) = chosen {
        app.apply_template(t);
    }
}

fn snap_targets(app: &GaugeApp, moving: &[GaugeId]) -> (Vec<f32>, Vec<f32>) {
    let (vw, vh) = app.state.video_size();
    let m = vh * 0.035;
    let mut xs = vec![0.0, m, vw / 2.0, vw - m, vw];
    let mut ys = vec![0.0, m, vh / 2.0, vh - m, vh];
    for g in app
        .state
        .project
        .gauges
        .iter()
        .filter(|g| g.visible && !moving.contains(&g.id))
    {
        let (x, y, w, h) = g.placement.bounds();
        xs.extend([x, x + w / 2.0, x + w]);
        ys.extend([y, y + h / 2.0, y + h]);
    }
    (xs, ys)
}

/// Finds the smallest correction aligning any of `edges` to any target within `threshold`.
fn best_snap(edges: [f32; 3], targets: &[f32], threshold: f32) -> Option<(f32, f32)> {
    let mut best: Option<(f32, f32)> = None;
    for e in edges {
        for &t in targets {
            let d = t - e;
            if d.abs() <= threshold && best.is_none_or(|(bd, _)| d.abs() < bd.abs()) {
                best = Some((d, t));
            }
        }
    }
    best
}

fn interact(app: &mut GaugeApp, ui: &egui::Ui, response: &Response, view: &View) {
    let (mods, press_origin, pointer) = ui.ctx().input(|i| {
        (
            i.modifiers,
            i.pointer.press_origin(),
            i.pointer.interact_pos(),
        )
    });

    // Hover cursor.
    if app.ui.canvas.drag.is_none()
        && let Some(hp) = response.hover_pos()
    {
        if let Some(g) = app.state.single_selection().filter(|g| !g.locked)
            && let Some((h, _)) = handle_positions(&g.placement, view)
                .into_iter()
                .find(|(_, p)| p.distance(hp) <= HANDLE + 3.0)
        {
            ui.ctx().set_cursor_icon(match h {
                Handle::N | Handle::S => CursorIcon::ResizeVertical,
                Handle::E | Handle::W => CursorIcon::ResizeHorizontal,
                Handle::NE | Handle::SW => CursorIcon::ResizeNeSw,
                Handle::NW | Handle::SE => CursorIcon::ResizeNwSe,
                Handle::Rotate => CursorIcon::Alias,
            });
        } else {
            let (x, y) = view.to_video(hp);
            if hit_gauge(&app.state.project.gauges, x, y).is_some() {
                ui.ctx().set_cursor_icon(CursorIcon::Grab);
            }
        }
    }

    if response.drag_started()
        && let Some(origin) = press_origin
    {
        let (ox, oy) = view.to_video(origin);
        let mut op: Option<DragOp> = None;
        if let Some(g) = app.state.single_selection().filter(|g| !g.locked)
            && let Some((h, _)) = handle_positions(&g.placement, view)
                .into_iter()
                .find(|(_, p)| p.distance(origin) <= HANDLE + 3.0)
        {
            op = Some(DragOp {
                kind: if h == Handle::Rotate {
                    DragKind::Rotate
                } else {
                    DragKind::Resize(h)
                },
                origin: (ox, oy),
                originals: vec![(g.id, g.placement)],
            });
        }
        if op.is_none() {
            match hit_gauge(&app.state.project.gauges, ox, oy) {
                Some(id) => {
                    if !app.state.selection.contains(&id) {
                        app.state.select(id, mods.command);
                    }
                    let originals: Vec<(GaugeId, Placement)> = app
                        .state
                        .selected_gauges()
                        .into_iter()
                        .filter(|g| !g.locked)
                        .map(|g| (g.id, g.placement))
                        .collect();
                    if !originals.is_empty() {
                        op = Some(DragOp {
                            kind: DragKind::Move,
                            origin: (ox, oy),
                            originals,
                        });
                    }
                }
                None => {
                    if !mods.command {
                        app.state.selection.clear();
                    }
                }
            }
        }
        if op.is_some() {
            app.state.undo.break_merge();
        }
        app.ui.canvas.drag = op;
    }

    if response.dragged()
        && let (Some(op), Some(p)) = (app.ui.canvas.drag.clone(), pointer)
    {
        ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
        let (px, py) = view.to_video(p);
        let threshold = 7.0 / view.scale;
        let mut guides = Vec::new();
        let mut updated: Vec<Gauge> = Vec::new();
        match op.kind {
            DragKind::Move => {
                let mut dx = px - op.origin.0;
                let mut dy = py - op.origin.1;
                if mods.shift {
                    if dx.abs() > dy.abs() {
                        dy = 0.0;
                    } else {
                        dx = 0.0;
                    }
                }
                if !mods.alt {
                    // Union bounds of the moving set.
                    let ids: Vec<GaugeId> = op.originals.iter().map(|o| o.0).collect();
                    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
                    for (_, pl) in &op.originals {
                        let (x, y, w, h) = pl.bounds();
                        x0 = x0.min(x);
                        y0 = y0.min(y);
                        x1 = x1.max(x + w);
                        y1 = y1.max(y + h);
                    }
                    let (xs, ys) = snap_targets(app, &ids);
                    if let Some((d, t)) =
                        best_snap([x0 + dx, (x0 + x1) / 2.0 + dx, x1 + dx], &xs, threshold)
                        && !(mods.shift && dx == 0.0)
                    {
                        dx += d;
                        guides.push(Guide::V(t));
                    }
                    if let Some((d, t)) =
                        best_snap([y0 + dy, (y0 + y1) / 2.0 + dy, y1 + dy], &ys, threshold)
                        && !(mods.shift && dy == 0.0)
                    {
                        dy += d;
                        guides.push(Guide::H(t));
                    }
                }
                for (id, pl) in &op.originals {
                    if let Some(g) = app.state.project.gauge(*id) {
                        let mut g = g.clone();
                        g.placement.x = (pl.x + dx).round();
                        g.placement.y = (pl.y + dy).round();
                        updated.push(g);
                    }
                }
            }
            DragKind::Resize(h) => {
                let (id, orig) = op.originals[0];
                if let Some(g) = app.state.project.gauge(id) {
                    let keep_aspect = mods.shift ^ g.kind.prefers_square();
                    let (lx, ly) = orig.to_local(px, py);
                    let (mut l, mut t, mut r, mut b) = (0.0f32, 0.0f32, orig.w, orig.h);
                    let min = 16.0;
                    match h {
                        Handle::E | Handle::NE | Handle::SE => r = lx.max(l + min),
                        Handle::W | Handle::NW | Handle::SW => l = lx.min(r - min),
                        _ => {}
                    }
                    match h {
                        Handle::S | Handle::SE | Handle::SW => b = ly.max(t + min),
                        Handle::N | Handle::NE | Handle::NW => t = ly.min(b - min),
                        _ => {}
                    }
                    if keep_aspect {
                        let aspect = orig.w / orig.h.max(1.0);
                        let (mut w, mut hh) = (r - l, b - t);
                        let corner = matches!(h, Handle::NE | Handle::NW | Handle::SE | Handle::SW);
                        if corner {
                            let s = (w / orig.w).max(hh / orig.h);
                            w = orig.w * s;
                            hh = orig.h * s;
                        } else if matches!(h, Handle::E | Handle::W) {
                            hh = w / aspect;
                        } else {
                            w = hh * aspect;
                        }
                        // Anchor the opposite side/corner.
                        match h {
                            Handle::W | Handle::NW | Handle::SW => l = r - w,
                            Handle::N | Handle::S => {
                                let cx = orig.w / 2.0;
                                l = cx - w / 2.0;
                            }
                            _ => {}
                        }
                        r = l + w;
                        match h {
                            Handle::N | Handle::NW | Handle::NE => t = b - hh,
                            Handle::E | Handle::W => {
                                let cy = orig.h / 2.0;
                                t = cy - hh / 2.0;
                            }
                            _ => {}
                        }
                        b = t + hh;
                    }
                    let (cxw, cyw) = orig.to_world((l + r) / 2.0, (t + b) / 2.0);
                    let mut g = g.clone();
                    g.placement.w = (r - l).round();
                    g.placement.h = (b - t).round();
                    g.placement.x = (cxw - g.placement.w / 2.0).round();
                    g.placement.y = (cyw - g.placement.h / 2.0).round();
                    updated.push(g);
                }
            }
            DragKind::Rotate => {
                let (id, orig) = op.originals[0];
                if let Some(g) = app.state.project.gauge(id) {
                    let (cx, cy) = orig.center();
                    let mut a = (py - cy).atan2(px - cx).to_degrees() + 90.0;
                    if mods.shift {
                        a = (a / 15.0).round() * 15.0;
                    } else if (a.rem_euclid(90.0)).min(90.0 - a.rem_euclid(90.0)) < 3.0 {
                        a = (a / 90.0).round() * 90.0;
                    }
                    let mut a = a.rem_euclid(360.0);
                    if a > 180.0 {
                        a -= 360.0;
                    }
                    let mut g = g.clone();
                    g.placement.rotation = a.round();
                    updated.push(g);
                }
            }
        }
        app.ui.canvas.guides = guides;
        if !updated.is_empty() {
            app.state.execute(Command::UpdateGauges {
                gauges: updated,
                merge: "canvas-drag",
            });
        }
    }

    if response.drag_stopped() {
        app.ui.canvas.drag = None;
        app.ui.canvas.guides.clear();
    }

    if (response.clicked() || response.secondary_clicked())
        && let Some(p) = response.interact_pointer_pos()
    {
        let (x, y) = view.to_video(p);
        match hit_gauge(&app.state.project.gauges, x, y) {
            Some(id) => {
                if response.secondary_clicked() {
                    if !app.state.selection.contains(&id) {
                        app.state.select(id, false);
                    }
                } else {
                    app.state.select(id, mods.command);
                }
                app.ui.right_tab = RightTab::Inspector;
            }
            None => {
                if !mods.command {
                    app.state.selection.clear();
                }
            }
        }
    }

    response.context_menu(|ui| {
        let has = !app.state.selection.is_empty();
        if !has {
            ui.label(RichText::new("No gauge selected").weak());
            if ui.button("Open Gauge Library").clicked() {
                app.ui.right_tab = RightTab::Library;
                ui.close();
            }
            return;
        }
        if ui.button("Duplicate   ⌘D").clicked() {
            app.state.duplicate_selection();
            ui.close();
        }
        if ui.button("Delete").clicked() {
            app.state.delete_selection();
            ui.close();
        }
        ui.separator();
        let all_locked = app.state.selected_gauges().iter().all(|g| g.locked);
        if ui
            .button(if all_locked { "Unlock" } else { "Lock" })
            .clicked()
        {
            set_flag(app, |g| g.locked = !all_locked);
            ui.close();
        }
        if ui.button("Hide").clicked() {
            set_flag(app, |g| g.visible = false);
            ui.close();
        }
        ui.separator();
        if let [id] = app.state.selection.as_slice() {
            let id = *id;
            if ui.button("Bring to Front").clicked() {
                app.state.execute(Command::Reorder {
                    id,
                    delta: i32::MAX / 2,
                });
                ui.close();
            }
            if ui.button("Send to Back").clicked() {
                app.state.execute(Command::Reorder {
                    id,
                    delta: -(i32::MAX / 2),
                });
                ui.close();
            }
        }
        if app.state.selection.len() > 1 && ui.button("Group").clicked() {
            super::inspector::group_selection(app);
            ui.close();
        }
        if app
            .state
            .selected_gauges()
            .iter()
            .any(|g| g.group.is_some())
            && ui.button("Ungroup").clicked()
        {
            super::inspector::ungroup_selection(app);
            ui.close();
        }
    });
}

fn set_flag(app: &mut GaugeApp, f: impl Fn(&mut Gauge)) {
    let gs: Vec<Gauge> = app
        .state
        .selected_gauges()
        .into_iter()
        .cloned()
        .map(|mut g| {
            f(&mut g);
            g
        })
        .collect();
    app.state.execute(Command::UpdateGauges {
        gauges: gs,
        merge: "flags",
    });
}

fn draw_selection(app: &GaugeApp, painter: &egui::Painter, view: &View) {
    let sel = app.state.selected_gauges();
    let single = sel.len() == 1;
    for g in &sel {
        let p = &g.placement;
        let corners: Vec<Pos2> = [(0.0, 0.0), (p.w, 0.0), (p.w, p.h), (0.0, p.h)]
            .iter()
            .map(|&(x, y)| {
                let (wx, wy) = p.to_world(x, y);
                view.to_screen(wx, wy)
            })
            .collect();
        let color = if g.locked {
            Color32::from_gray(150)
        } else {
            theme::ACCENT_2
        };
        painter.add(Shape::closed_line(corners.clone(), Stroke::new(1.5, color)));
        if single && !g.locked {
            let handles = handle_positions(p, view);
            if let (Some(top), Some(rot)) = (
                handles.iter().find(|h| h.0 == Handle::N),
                handles.iter().find(|h| h.0 == Handle::Rotate),
            ) {
                painter.line_segment([top.1, rot.1], Stroke::new(1.0, color));
            }
            for (h, pos) in handles {
                if h == Handle::Rotate {
                    painter.circle(pos, HANDLE * 0.8, Color32::WHITE, Stroke::new(1.5, color));
                } else {
                    let r = Rect::from_center_size(pos, Vec2::splat(HANDLE * 1.4));
                    painter.rect_filled(r, 1.5, Color32::WHITE);
                    painter.rect_stroke(r, 1.5, Stroke::new(1.0, color), StrokeKind::Outside);
                }
            }
        }
        if single {
            let label = format!(
                "X {:.0}  Y {:.0}  W {:.0}  H {:.0}{}{}",
                p.x,
                p.y,
                p.w,
                p.h,
                if p.rotation != 0.0 {
                    format!("  {:.0}°", p.rotation)
                } else {
                    String::new()
                },
                if g.locked { "  🔒" } else { "" }
            );
            let bottom = corners.iter().map(|c| c.y).fold(f32::MIN, f32::max);
            let cx = corners.iter().map(|c| c.x).sum::<f32>() / 4.0;
            let pos = pos2(cx, bottom + 18.0);
            let galley =
                painter.layout_no_wrap(label, egui::FontId::monospace(11.0), Color32::WHITE);
            let r = Rect::from_center_size(pos, galley.size() + vec2(10.0, 4.0));
            painter.rect_filled(r, 3.0, Color32::from_black_alpha(190));
            painter.galley(r.min + vec2(5.0, 2.0), galley, Color32::WHITE);
        }
    }
    for guide in &app.ui.canvas.guides {
        let s = Stroke::new(1.0, Color32::from_rgb(255, 60, 200));
        match *guide {
            Guide::V(x) => {
                let a = view.to_screen(x, 0.0);
                painter.line_segment(
                    [pos2(a.x, view.rect.top()), pos2(a.x, view.rect.bottom())],
                    s,
                );
            }
            Guide::H(y) => {
                let a = view.to_screen(0.0, y);
                painter.line_segment(
                    [pos2(view.rect.left(), a.y), pos2(view.rect.right(), a.y)],
                    s,
                );
            }
        }
    }
}
