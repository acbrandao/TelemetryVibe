//! Scrolling EKG (heart monitor) trace in sync with the heart rate.
//!
//! Beats are placed with the cumulative beat count derived from the heart-rate channel, so
//! there is exactly one PQRST complex per beat at the recorded rate, the head of the trace is
//! the current instant, and every frame is computed statelessly (scrubbing, preview and
//! export agree).

use super::model::{Gauge, Rgba, TextAlign};
use super::scene::{self, Cap, Scene};
use super::{
    RenderCtx, format_gauge_value, gauge_unit_label, gauge_value, panel, widest_value_template,
};
use crate::telemetry::Metric;
use crate::telemetry::model::HEART_BEATS;

/// One PQRST complex as a function of the offset from its R peak, in seconds.
/// Amplitudes are relative to the R peak (1.0); durations are realistic and do not stretch
/// with the heart rate (only the gap between beats does).
pub fn pqrst(x: f64) -> f64 {
    let g = |center: f64, width: f64, amp: f64| amp * (-((x - center) / width).powi(2)).exp();
    g(-0.16, 0.025, 0.12) // P
        + g(-0.025, 0.008, -0.12) // Q
        + g(0.0, 0.010, 1.0) // R
        + g(0.028, 0.009, -0.25) // S
        + g(0.24, 0.045, 0.28) // T
}

/// Trace value at track time `t`: contributions of the previous and the next R peak.
fn trace_value(ctx: &RenderCtx<'_>, t: f64) -> f64 {
    let Some(track) = ctx.track else { return 0.0 };
    let (Some(beats), Some(bpm)) = (
        track.extra_value(HEART_BEATS, t, 0.0),
        track.value(Metric::HeartRate, t).filter(|b| *b > 20.0),
    ) else {
        return 0.0;
    };
    let period = 60.0 / bpm;
    let phase = beats.rem_euclid(1.0);
    pqrst(phase * period) + pqrst(-(1.0 - phase) * period)
}

/// Width of the trace strip (with its left padding) beside the number, from the gauge height.
pub fn strip_inset(h: f32) -> f32 {
    h * 0.1 + h * 1.5
}

/// Seconds of trace for `beats` heartbeats at the current heart rate (`fallback` without one).
fn visible_seconds(ctx: &RenderCtx<'_>, beats: f32, fallback: f32) -> f64 {
    let bpm = ctx
        .track
        .and_then(|t| t.value(Metric::HeartRate, ctx.gps_t()))
        .filter(|b| *b > 20.0);
    match bpm {
        Some(b) => (beats.max(0.5) as f64 * 60.0 / b).clamp(0.5, 15.0),
        None => fallback.max(0.5) as f64,
    }
}

pub fn build(
    g: &Gauge,
    ctx: &RenderCtx<'_>,
    beats: f32,
    fallback_window: f32,
    show_value: bool,
    grid: bool,
) -> Scene {
    let mut s = Scene::new();
    let st = &g.style;
    let (w, h) = (g.placement.w, g.placement.h);
    panel(&mut s, g, w, h, None);

    // Compact monitor strip on the left; the number uses the shared digital layout beside it.
    let pad = h * 0.1;
    let (x0, x1) = if show_value {
        (pad, strip_inset(h))
    } else {
        (pad, (w - pad).max(pad + 1.0))
    };
    let (top, bottom) = (pad, h - pad);
    s.fill(
        scene::rounded_rect(x0, top, x1 - x0, bottom - top, h * 0.06),
        Rgba::BLACK.with_alpha_mul(0.35),
    );
    let inner = h * 0.05;
    let (tx0, tx1) = (x0 + inner, x1 - inner);
    let base_y = top + (bottom - top) * 0.7;
    let amp = (bottom - top) * 0.58;
    let window = visible_seconds(ctx, beats, fallback_window);
    let gps_t = ctx.gps_t();

    // Monitor grid: major lines every second, minor every 0.2 s (scrolling with the trace).
    if grid {
        let px_per_s = (tx1 - tx0) as f64 / window;
        let first = ((gps_t - window) / 0.2).ceil() as i64;
        let last = (gps_t / 0.2).floor() as i64;
        for k in first..=last {
            let x = tx1 - ((gps_t - k as f64 * 0.2) * px_per_s) as f32;
            let major = k.rem_euclid(5) == 0;
            s.stroke(
                scene::line(x, top + inner, x, bottom - inner),
                st.primary.with_alpha_mul(if major { 0.16 } else { 0.07 }),
                if major { 1.2 } else { 0.8 },
                Cap::Butt,
            );
        }
        for i in 1..4 {
            let y = top + (bottom - top) * i as f32 / 4.0;
            s.stroke(
                scene::line(tx0, y, tx1, y),
                st.primary.with_alpha_mul(0.07),
                0.8,
                Cap::Butt,
            );
        }
    }

    // Trace: sampled finely enough to resolve the ~10 ms R peak, older parts fading out.
    let n = ((window / 0.003) as usize).clamp(200, 3000);
    let pts: Vec<(f32, f32)> = (0..=n)
        .map(|i| {
            let f = i as f64 / n as f64;
            let v = trace_value(ctx, gps_t - window * (1.0 - f));
            (tx0 + (tx1 - tx0) * f as f32, base_y - (v as f32) * amp)
        })
        .collect();
    let line_w = (h * 0.022).max(1.2);
    let chunks = 5;
    let per = n / chunks;
    for c in 0..chunks {
        let from = c * per;
        let to = if c + 1 == chunks {
            n
        } else {
            (c + 1) * per + 1
        };
        let path = scene::polyline(&pts[from..=to.min(n)]);
        if c + 1 == chunks {
            s.glow_stroke(path, st.primary, line_w, Cap::Round, st.glow);
        } else {
            let alpha = 0.25 + 0.75 * (c + 1) as f32 / chunks as f32;
            s.stroke(path, st.primary.with_alpha_mul(alpha), line_w, Cap::Round);
        }
    }
    // Bright head at the current instant.
    if let Some(&(hx, hy)) = pts.last() {
        s.glow_fill(
            scene::circle(hx, hy, line_w * 1.5),
            st.primary,
            st.glow.max(0.5),
            line_w * 2.5,
        );
    }

    if show_value {
        let value = gauge_value(g, ctx);
        let text = format_gauge_value(g, ctx, value);
        let template = widest_value_template(g, ctx);
        let unit = if st.show_units {
            gauge_unit_label(g, ctx)
        } else {
            ""
        };
        let label = st.show_label.then(|| g.label());
        super::digital::value_block(
            &mut s,
            g,
            &text,
            &template,
            unit,
            label,
            TextAlign::Left,
            st.primary,
            x1,
        );
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pqrst_shape() {
        assert!((pqrst(0.0) - 1.0).abs() < 0.02, "R peak");
        assert!(pqrst(0.028) < -0.1, "S dip");
        assert!(pqrst(0.24) > 0.2, "T wave");
        assert!(pqrst(0.6).abs() < 1e-3, "flat between beats");
    }
}
