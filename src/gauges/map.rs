//! Minimal vector route map with current position, direction arrow and trail.

use super::model::{FontWeight, Gauge, Rgba, RouteMode};
use super::scene::{self, Cap, HAlign, Scene, VAlign};
use super::{RenderCtx, panel};
use crate::telemetry::model::Route;
use crate::telemetry::units::{to_display, unit_label};
use crate::telemetry::{Metric, Track, UnitSystem};

#[derive(Clone, Copy, Debug)]
pub struct MapOpts {
    pub mode: RouteMode,
    pub trail_seconds: f32,
    pub route_width: f32,
    pub marker_size: f32,
    pub show_arrow: bool,
    pub heading_up: bool,
    pub zoom_radius: f32,
    pub speed_colors: bool,
}

/// Marker color: the speed zone color when speed coloring is on, else the accent color.
fn marker_color(g: &Gauge, track: &Track, gps_t: f64, o: &MapOpts) -> Rgba {
    if o.speed_colors
        && let Some(v) = track.value(Metric::Speed, gps_t)
        && let Some((_, z)) = g.zone_for(v)
    {
        return z.color;
    }
    g.style.accent
}

/// Draws the position marker (direction arrow or dot) at (mx, my).
fn draw_marker(
    s: &mut Scene,
    g: &Gauge,
    o: &MapOpts,
    mx: f32,
    my: f32,
    screen_heading: f32,
    color: Rgba,
) {
    let st = &g.style;
    let ms = o.marker_size.max(1.0);
    if o.show_arrow {
        // Heading 0 = north = screen up (-90° in screen angles).
        let a = screen_heading - 90.0;
        let tip = scene::polar(mx, my, ms * 1.6, a);
        let left = scene::polar(mx, my, ms, a + 140.0);
        let right = scene::polar(mx, my, ms, a - 140.0);
        let notch = scene::polar(mx, my, ms * 0.35, a + 180.0);
        s.glow_fill(
            scene::polygon(&[tip, left, notch, right]),
            color,
            st.glow.max(0.3),
            ms * 0.5,
        );
        s.stroke(
            scene::polygon(&[tip, left, notch, right]),
            st.text,
            (ms * 0.15).max(1.0),
            Cap::Round,
        );
    } else {
        s.glow_fill(scene::circle(mx, my, ms), color, st.glow.max(0.3), ms * 0.6);
        s.stroke(
            scene::circle(mx, my, ms),
            st.text,
            (ms * 0.25).max(1.0),
            Cap::Butt,
        );
    }
}

pub fn build(g: &Gauge, ctx: &RenderCtx<'_>, o: MapOpts) -> Scene {
    let mut s = Scene::new();
    let st = &g.style;
    let (w, h) = (g.placement.w, g.placement.h);
    panel(&mut s, g, w, h, None);
    let Some(track) = ctx.track else {
        s.text(
            "NO GPS",
            w / 2.0,
            h / 2.0,
            h.min(w) * 0.12,
            FontWeight::Bold,
            HAlign::Center,
            VAlign::Middle,
            st.secondary,
        );
        return s;
    };
    let Some(route) = track.route.as_ref() else {
        s.text(
            "NO ROUTE",
            w / 2.0,
            h / 2.0,
            h.min(w) * 0.12,
            FontWeight::Bold,
            HAlign::Center,
            VAlign::Middle,
            st.secondary,
        );
        return s;
    };
    let gps_t = ctx.gps_t().clamp(0.0, track.duration());
    if o.mode == RouteMode::CloseUp {
        build_close_up(&mut s, g, ctx, &o, track, route, gps_t);
        return s;
    }
    let pad = w.min(h) * 0.1 + o.marker_size;
    let (aw, ah) = (
        (w - pad * 2.0).max(1.0) as f64,
        (h - pad * 2.0).max(1.0) as f64,
    );
    let (rw, rh) = (route.width.max(1.0), route.height.max(1.0));

    let heading = track.value(Metric::Heading, gps_t).unwrap_or(0.0);
    let rot = if o.heading_up {
        -heading.to_radians()
    } else {
        0.0
    };
    // Heading-up must fit at any rotation, so use the bounding circle.
    let scale = if o.heading_up {
        aw.min(ah) / (rw * rw + rh * rh).sqrt()
    } else {
        (aw / rw).min(ah / rh)
    };
    let (cx, cy) = (w as f64 / 2.0, h as f64 / 2.0);
    let (rcx, rcy) = (rw / 2.0, rh / 2.0);
    let (sr, cr) = (rot.sin(), rot.cos());
    let to_screen = |p: [f64; 2]| -> (f32, f32) {
        let (dx, dy) = ((p[0] - rcx) * scale, (p[1] - rcy) * scale);
        (
            (cx + dx * cr - dy * sr) as f32,
            (cy + dx * sr + dy * cr) as f32,
        )
    };

    let current = match (
        track.value(Metric::Latitude, gps_t),
        track.value(Metric::Longitude, gps_t),
    ) {
        (Some(lat), Some(lon)) => route.project(lat, lon),
        _ => route.points[route.index_at(gps_t).min(route.points.len() - 1)],
    };
    let idx = route.index_at(gps_t);
    let mut traveled: Vec<(f32, f32)> = route.points[..=idx.min(route.points.len() - 1)]
        .iter()
        .map(|&p| to_screen(p))
        .collect();
    traveled.push(to_screen(current));

    let width = o.route_width.max(0.5);
    match o.mode {
        RouteMode::Full => {
            let all: Vec<(f32, f32)> = route.points.iter().map(|&p| to_screen(p)).collect();
            s.stroke(scene::polyline(&all), st.secondary, width, Cap::Round);
            s.glow_stroke(
                scene::polyline(&traveled),
                st.primary,
                width * 1.15,
                Cap::Round,
                st.glow,
            );
        }
        RouteMode::Progressive => {
            s.glow_stroke(
                scene::polyline(&traveled),
                st.primary,
                width,
                Cap::Round,
                st.glow,
            );
        }
        RouteMode::Trail | RouteMode::CloseUp => {
            let from = gps_t - o.trail_seconds.max(1.0) as f64;
            let start = route.index_at(from);
            let mut trail: Vec<(f32, f32)> = route.points[start..=idx.min(route.points.len() - 1)]
                .iter()
                .map(|&p| to_screen(p))
                .collect();
            trail.push(to_screen(current));
            s.glow_stroke(
                scene::polyline(&trail),
                st.primary,
                width,
                Cap::Round,
                st.glow,
            );
        }
    }

    // Marker.
    let (mx, my) = to_screen(current);
    let screen_heading = if o.heading_up { 0.0 } else { heading as f32 };
    let color = marker_color(g, track, gps_t, &o);
    draw_marker(&mut s, g, &o, mx, my, screen_heading, color);
    if st.show_label && !st.label.is_empty() {
        s.text(
            st.label.clone(),
            w * 0.06,
            h * 0.06,
            h * 0.08 * st.font_scale,
            FontWeight::Bold,
            HAlign::Left,
            VAlign::Top,
            st.secondary,
        );
    }
    s
}

/// Clips the segment p0–p1 to a rectangle (Liang–Barsky). Returns the visible part.
fn clip_segment(
    p0: (f32, f32),
    p1: (f32, f32),
    (x0, y0, x1, y1): (f32, f32, f32, f32),
) -> Option<((f32, f32), (f32, f32))> {
    let (dx, dy) = (p1.0 - p0.0, p1.1 - p0.1);
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for (p, q) in [
        (-dx, p0.0 - x0),
        (dx, x1 - p0.0),
        (-dy, p0.1 - y0),
        (dy, y1 - p0.1),
    ] {
        if p.abs() < 1e-9 {
            if q < 0.0 {
                return None;
            }
        } else {
            let r = q / p;
            if p < 0.0 {
                t0 = t0.max(r);
            } else {
                t1 = t1.min(r);
            }
            if t0 > t1 {
                return None;
            }
        }
    }
    Some((
        (p0.0 + dx * t0, p0.1 + dy * t0),
        (p0.0 + dx * t1, p0.1 + dy * t1),
    ))
}

/// Polylines grouped by color, merging consecutive segments that join up.
#[derive(Default)]
struct Runs(Vec<(Rgba, Vec<(f32, f32)>)>);

impl Runs {
    fn push(&mut self, color: Rgba, a: (f32, f32), b: (f32, f32)) {
        if let Some((c, pts)) = self.0.last_mut()
            && *c == color
            && pts
                .last()
                .is_some_and(|l| (l.0 - a.0).abs() < 0.01 && (l.1 - a.1).abs() < 0.01)
        {
            pts.push(b);
            return;
        }
        self.0.push((color, vec![a, b]));
    }
}

/// Zoomed-in map following the current position: the route ahead in the secondary color, the
/// traveled part in the primary (or speed zone) colors, a scale bar and a north pointer.
fn build_close_up(
    s: &mut Scene,
    g: &Gauge,
    ctx: &RenderCtx<'_>,
    o: &MapOpts,
    track: &Track,
    route: &Route,
    gps_t: f64,
) {
    let st = &g.style;
    let (w, h) = (g.placement.w, g.placement.h);
    let (Some(lat), Some(lon)) = (
        track.channel(Metric::Latitude),
        track.channel(Metric::Longitude),
    ) else {
        return;
    };
    let speed = track.channel(Metric::Speed);
    let current = match (
        track.value(Metric::Latitude, gps_t),
        track.value(Metric::Longitude, gps_t),
    ) {
        (Some(la), Some(lo)) => route.project(la, lo),
        _ => route.points[route.index_at(gps_t).min(route.points.len() - 1)],
    };
    let heading = track.value(Metric::Heading, gps_t).unwrap_or(0.0);
    let rot = if o.heading_up {
        -heading.to_radians()
    } else {
        0.0
    };
    let (sr, cr) = (rot.sin(), rot.cos());
    let view_r = (w.min(h) / 2.0 * 0.88) as f64;
    let scale = view_r / o.zoom_radius.max(10.0) as f64;
    // Heading-up shows more of the road ahead.
    let (cx, cy) = (
        w as f64 / 2.0,
        if o.heading_up {
            h as f64 * 0.62
        } else {
            h as f64 / 2.0
        },
    );
    let to_screen = |p: [f64; 2]| -> (f32, f32) {
        let (dx, dy) = ((p[0] - current[0]) * scale, (p[1] - current[1]) * scale);
        (
            (cx + dx * cr - dy * sr) as f32,
            (cy + dx * sr + dy * cr) as f32,
        )
    };
    let inset = o.route_width.max(w.min(h) * 0.035);
    let bounds = (inset, inset, w - inset, h - inset);
    let cur_screen = to_screen(current);

    // Segment colors.
    let traveled_color = |i: usize| -> Rgba {
        if o.speed_colors
            && let Some(v) = speed
                .and_then(|sp| sp.get(i))
                .copied()
                .filter(|v| !v.is_nan())
            && let Some((_, z)) = g.zone_for(v)
        {
            return z.color;
        }
        st.primary
    };
    let mut ahead = Runs::default();
    let mut behind = Runs::default();
    let mut prev: Option<(usize, (f32, f32))> = None;
    for i in 0..lat.len().min(lon.len()) {
        if lat[i].is_nan() || lon[i].is_nan() {
            continue;
        }
        let p = to_screen(route.project(lat[i], lon[i]));
        if let Some((j, q)) = prev {
            let (tj, ti) = (track.times[j], track.times[i]);
            let mut piece = |a: (f32, f32), b: (f32, f32), traveled: bool| {
                if let Some((ca, cb)) = clip_segment(a, b, bounds) {
                    if traveled {
                        behind.push(traveled_color(i), ca, cb);
                    } else {
                        ahead.push(st.secondary, ca, cb);
                    }
                }
            };
            if ti <= gps_t {
                piece(q, p, true);
            } else if tj >= gps_t {
                piece(q, p, false);
            } else {
                // The segment containing the current position.
                piece(q, cur_screen, true);
                piece(cur_screen, p, false);
            }
        }
        prev = Some((i, p));
    }
    let width = o.route_width.max(0.5);
    for (c, pts) in &ahead.0 {
        s.stroke(scene::polyline(pts), *c, width, Cap::Round);
    }
    for (c, pts) in &behind.0 {
        s.glow_stroke(scene::polyline(pts), *c, width * 1.15, Cap::Round, st.glow);
    }

    // Scale bar (bottom left).
    let units = ctx.units;
    let (unit_m, unit) = match units {
        UnitSystem::Metric => (1.0, "m"),
        UnitSystem::Imperial => (0.3048, "ft"),
    };
    let step_units = crate::utils::nice_step(o.zoom_radius as f64 / unit_m, 2.0);
    let bar_px = (step_units * unit_m * scale) as f32;
    let label = if units == UnitSystem::Metric && step_units >= 1000.0 {
        format!("{} km", step_units / 1000.0)
    } else if units == UnitSystem::Imperial && step_units >= 5280.0 {
        format!(
            "{} {}",
            to_display(Metric::Distance, step_units * unit_m, units),
            unit_label(Metric::Distance, units)
        )
    } else {
        format!("{step_units} {unit}")
    };
    let (bx, by) = (inset * 1.2, h - inset * 1.2);
    let sw = (w.min(h) * 0.012).max(1.0);
    s.stroke(scene::line(bx, by, bx + bar_px, by), st.text, sw, Cap::Butt);
    for x in [bx, bx + bar_px] {
        s.stroke(scene::line(x, by, x, by - sw * 4.0), st.text, sw, Cap::Butt);
    }
    s.text(
        label,
        bx + bar_px / 2.0,
        by - sw * 6.0,
        h * 0.055 * st.font_scale.max(0.1),
        FontWeight::Bold,
        HAlign::Center,
        VAlign::Baseline,
        st.text,
    );

    // North pointer (top right) when the map rotates.
    if o.heading_up {
        let (nx, ny) = (w - inset * 2.2, inset * 2.2);
        let a = -90.0 + rot.to_degrees() as f32;
        let k = w.min(h) * 0.045;
        let tip = scene::polar(nx, ny, k, a);
        let l = scene::polar(nx, ny, k * 0.7, a + 140.0);
        let r = scene::polar(nx, ny, k * 0.7, a - 140.0);
        s.fill(scene::polygon(&[tip, l, r]), Rgba::rgb(235, 70, 60));
        let (tx, ty) = scene::polar(nx, ny, k * 1.9, a + 180.0);
        s.text(
            "N",
            tx,
            ty,
            k * 1.1,
            FontWeight::Bold,
            HAlign::Center,
            VAlign::Middle,
            st.text,
        );
    }

    // Marker with a halo ring in the same (speed) color.
    let color = marker_color(g, track, gps_t, o);
    let ms = o.marker_size.max(1.0);
    s.stroke(
        scene::circle(cur_screen.0, cur_screen.1, ms * 2.3),
        color.with_alpha_mul(0.45),
        (ms * 0.3).max(1.0),
        Cap::Butt,
    );
    let screen_heading = if o.heading_up { 0.0 } else { heading as f32 };
    draw_marker(s, g, o, cur_screen.0, cur_screen.1, screen_heading, color);
    if st.show_label && !st.label.is_empty() {
        s.text(
            st.label.clone(),
            w * 0.06,
            h * 0.06,
            h * 0.08 * st.font_scale,
            FontWeight::Bold,
            HAlign::Left,
            VAlign::Top,
            st.secondary,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_clipping() {
        let b = (0.0, 0.0, 10.0, 10.0);
        assert_eq!(
            clip_segment((-5.0, 5.0), (15.0, 5.0), b),
            Some(((0.0, 5.0), (10.0, 5.0)))
        );
        assert_eq!(clip_segment((-5.0, -5.0), (-1.0, 20.0), b), None);
        assert_eq!(
            clip_segment((2.0, 2.0), (3.0, 3.0), b),
            Some(((2.0, 2.0), (3.0, 3.0)))
        );
    }
}
