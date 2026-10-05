//! Historical graphs: rolling window (e.g. last 30 s of heart rate) or full-activity profile
//! (e.g. elevation) with the current position highlighted.

use super::model::{FontWeight, Gauge};
use super::scene::{self, Cap, HAlign, Paint, Scene, VAlign};
use super::{
    RenderCtx, display_value, format_gauge_value, gauge_unit_label, gauge_value, panel,
    relative_to_sync,
};
use crate::telemetry::Metric;

#[derive(Clone, Copy, Debug)]
pub struct GraphOpts {
    pub window: f32,
    pub full_activity: bool,
    pub line_width: f32,
    pub fill: bool,
    pub grid: bool,
    pub auto_scale: bool,
    pub show_value: bool,
}

fn sample(g: &Gauge, ctx: &RenderCtx<'_>, t: f64) -> Option<f64> {
    let track = ctx.track?;
    let v = if g.metric == Metric::Custom {
        track.extra_value(&g.custom_key, t, g.smoothing)
    } else {
        // Smoothing per point is expensive on long windows; the raw value suffices for shape.
        if g.smoothing > 0.0 && !matches!(g.metric, Metric::Heading) {
            track.value_smoothed(g.metric, t, g.smoothing * 0.6)
        } else {
            track.value(g.metric, t)
        }
        .map(|v| relative_to_sync(g, ctx, v))
    }?;
    Some(display_value(g, ctx, v))
}

pub fn build(g: &Gauge, ctx: &RenderCtx<'_>, o: GraphOpts) -> Scene {
    let mut s = Scene::new();
    let st = &g.style;
    let (w, h) = (g.placement.w, g.placement.h);
    panel(&mut s, g, w, h, None);
    let fs = st.font_scale.max(0.1);
    let pad = w.min(h) * 0.08;
    let head = if st.show_label || o.show_value {
        h * 0.16 * fs
    } else {
        0.0
    };
    let value = gauge_value(g, ctx);
    if st.show_label {
        s.text(
            g.label(),
            pad,
            pad,
            head * 0.8,
            FontWeight::Bold,
            HAlign::Left,
            VAlign::Top,
            st.secondary,
        );
    }
    if o.show_value {
        let unit = if st.show_units {
            gauge_unit_label(g, ctx)
        } else {
            ""
        };
        s.text(
            format!("{} {}", format_gauge_value(g, ctx, value), unit)
                .trim()
                .to_string(),
            w - pad,
            pad,
            head,
            st.font_weight,
            HAlign::Right,
            VAlign::Top,
            st.text,
        );
    }
    let px0 = pad;
    let px1 = w - pad;
    let py0 = pad + if head > 0.0 { head + h * 0.06 } else { 0.0 };
    let py1 = h - pad;
    if px1 <= px0 || py1 <= py0 {
        return s;
    }

    let gps_t = ctx.gps_t();
    let (t0, t1) = match (o.full_activity, ctx.track) {
        (true, Some(track)) => (0.0, track.duration().max(1.0)),
        _ => (gps_t - o.window.max(1.0) as f64, gps_t),
    };
    let n = ((px1 - px0) / 2.0).clamp(24.0, 240.0) as usize;
    let pts: Vec<(f64, Option<f64>)> = (0..=n)
        .map(|i| {
            let t = t0 + (t1 - t0) * i as f64 / n as f64;
            (t, sample(g, ctx, t))
        })
        .collect();

    let (mut lo, mut hi) = if o.auto_scale {
        pts.iter()
            .filter_map(|p| p.1)
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| {
                (a.min(v), b.max(v))
            })
    } else {
        (display_value(g, ctx, g.min), display_value(g, ctx, g.max))
    };
    if !lo.is_finite() || !hi.is_finite() {
        lo = display_value(g, ctx, g.min);
        hi = display_value(g, ctx, g.max);
    }
    if o.auto_scale {
        let span = (hi - lo).max(1.0);
        lo -= span * 0.1;
        hi += span * 0.1;
    }
    if (hi - lo).abs() < 1e-9 {
        hi = lo + 1.0;
    }
    let map_y = |v: f64| py1 - ((v - lo) / (hi - lo)).clamp(0.0, 1.0) as f32 * (py1 - py0);
    let map_x = |t: f64| px0 + ((t - t0) / (t1 - t0)) as f32 * (px1 - px0);

    if o.grid {
        for i in 0..=3 {
            let y = py0 + (py1 - py0) * i as f32 / 3.0;
            s.stroke(
                scene::line(px0, y, px1, y),
                st.secondary.with_alpha_mul(0.18),
                1.0,
                Cap::Butt,
            );
        }
        let ls = head.max(h * 0.1) * 0.55;
        s.text(
            format!("{}", hi.round() as i64),
            px1,
            py0 + 2.0,
            ls,
            FontWeight::Medium,
            HAlign::Right,
            VAlign::Top,
            st.secondary.with_alpha_mul(0.7),
        );
        s.text(
            format!("{}", lo.round() as i64),
            px1,
            py1 - 2.0,
            ls,
            FontWeight::Medium,
            HAlign::Right,
            VAlign::Baseline,
            st.secondary.with_alpha_mul(0.7),
        );
    }

    // Contiguous runs (gaps break the line).
    let mut runs: Vec<Vec<(f32, f32, f64)>> = vec![Vec::new()];
    for &(t, v) in &pts {
        match v {
            Some(v) => runs
                .last_mut()
                .map(|r| r.push((map_x(t), map_y(v), t)))
                .unwrap_or(()),
            None => {
                if runs.last().is_some_and(|r| !r.is_empty()) {
                    runs.push(Vec::new());
                }
            }
        }
    }
    let lw = o.line_width.max(0.5) * (h / 120.0).max(0.6);
    for run in runs.iter().filter(|r| r.len() >= 2) {
        let line: Vec<(f32, f32)> = run.iter().map(|p| (p.0, p.1)).collect();
        if o.fill {
            let mut poly = line.clone();
            poly.push((line[line.len() - 1].0, py1));
            poly.push((line[0].0, py1));
            s.fill_paint(
                scene::polygon(&poly),
                Paint::Linear {
                    start: (0.0, py0),
                    end: (0.0, py1),
                    stops: vec![
                        (0.0, st.primary.with_alpha_mul(0.45)),
                        (1.0, st.primary.with_alpha_mul(0.02)),
                    ],
                },
            );
        }
        if o.full_activity {
            let traveled: Vec<(f32, f32)> = run
                .iter()
                .filter(|p| p.2 <= gps_t)
                .map(|p| (p.0, p.1))
                .collect();
            s.stroke(
                scene::polyline(&line),
                st.secondary.with_alpha_mul(0.6),
                lw,
                Cap::Round,
            );
            s.glow_stroke(
                scene::polyline(&traveled),
                st.primary,
                lw * 1.2,
                Cap::Round,
                st.glow,
            );
        } else {
            s.glow_stroke(scene::polyline(&line), st.primary, lw, Cap::Round, st.glow);
        }
    }

    // Current position marker.
    if let Some(v) = sample(g, ctx, gps_t) {
        let x = map_x(gps_t.clamp(t0, t1));
        let y = map_y(v);
        if o.full_activity {
            s.stroke(
                scene::line(x, py0, x, py1),
                st.text.with_alpha_mul(0.5),
                1.0,
                Cap::Butt,
            );
        }
        s.fill(scene::circle(x, y, lw * 2.2), st.accent);
        s.stroke(scene::circle(x, y, lw * 2.2), st.text, lw * 0.6, Cap::Butt);
    }
    s
}
