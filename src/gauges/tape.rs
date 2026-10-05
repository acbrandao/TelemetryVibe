//! HUD-style scrolling tapes (altitude, speed, heading, gradient).
//! The current value stays centered while the scale moves behind it.

use super::model::{FontWeight, Gauge, Orientation, Rgba};
use super::scene::{self, Cap, HAlign, Scene, VAlign};
use super::{RenderCtx, display_value, format_gauge_value, gauge_value, panel};
use crate::telemetry::Metric;
use crate::utils::nice_step;

fn compass_label(deg: f64) -> Option<&'static str> {
    let d = (deg.rem_euclid(360.0)).round() as i64 % 360;
    Some(match d {
        0 => "N",
        45 => "NE",
        90 => "E",
        135 => "SE",
        180 => "S",
        225 => "SW",
        270 => "W",
        315 => "NW",
        _ => return None,
    })
}

pub fn build(
    g: &Gauge,
    ctx: &RenderCtx<'_>,
    orientation: Orientation,
    span: f64,
    major_step: f64,
    minor_ticks: u32,
) -> Scene {
    let mut s = Scene::new();
    let st = &g.style;
    let (w, h) = (g.placement.w, g.placement.h);
    panel(&mut s, g, w, h, None);
    let value = gauge_value(g, ctx);
    let heading = g.metric == Metric::Heading;
    let span = if span > 0.0 { span } else { 100.0 };
    let v = value
        .map(|v| display_value(g, ctx, v))
        .unwrap_or_else(|| display_value(g, ctx, g.min));
    let major = if major_step > 0.0 {
        major_step
    } else if heading {
        nice_step(span, 4.0).max(5.0)
    } else {
        nice_step(span, 5.0)
    };
    let minors = minor_ticks.clamp(1, 10);
    let minor = major / minors as f64;
    let vertical = orientation == Orientation::Vertical;
    let len = if vertical { h } else { w };
    let cross = if vertical { w } else { h };
    let ppu = len as f64 / span;
    let center = len / 2.0;
    let fs = st.font_scale.max(0.1);
    let label_size = (cross * 0.2).min(len * 0.065) * fs;

    let first = ((v - span / 2.0) / minor).floor() as i64;
    let last = ((v + span / 2.0) / minor).ceil() as i64;
    for k in first..=last {
        let tv = k as f64 * minor;
        let pos = center - ((tv - v) * ppu) as f32 * if vertical { 1.0 } else { -1.0 };
        if pos < 0.0 || pos > len {
            continue;
        }
        // Fade toward the ends.
        let d = ((pos - center).abs() / center).min(1.0);
        let fade = 1.0 - d.powi(3);
        let is_major = k.rem_euclid(minors as i64) == 0;
        let tick = if is_major { cross * 0.2 } else { cross * 0.1 };
        let color = st.secondary.with_alpha_mul(fade);
        let lw = if is_major {
            cross * 0.025
        } else {
            cross * 0.015
        };
        if vertical {
            s.stroke(
                scene::line(0.0, pos, tick, pos),
                color,
                lw.max(1.0),
                Cap::Butt,
            );
        } else {
            s.stroke(
                scene::line(pos, h, pos, h - tick),
                color,
                lw.max(1.0),
                Cap::Butt,
            );
        }
        if is_major {
            let label = if heading {
                compass_label(tv)
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("{}", tv.rem_euclid(360.0).round() as i64))
            } else if major < 1.0 {
                format!("{tv:.1}")
            } else {
                format!("{}", tv.round() as i64)
            };
            if vertical {
                s.text(
                    label,
                    cross * 0.27,
                    pos,
                    label_size,
                    FontWeight::Medium,
                    HAlign::Left,
                    VAlign::Middle,
                    st.text.with_alpha_mul(fade),
                );
            } else {
                s.text(
                    label,
                    pos,
                    h - cross * 0.3,
                    label_size,
                    FontWeight::Medium,
                    HAlign::Center,
                    VAlign::Baseline,
                    st.text.with_alpha_mul(fade),
                );
            }
        }
    }

    // Current value box.
    let text = format_gauge_value(g, ctx, value);
    let box_color = Rgba::rgb(10, 12, 16).with_alpha_mul(0.92);
    if vertical {
        let bh = (h * 0.13).max(label_size * 1.8);
        let x0 = cross * 0.12;
        let pts = [
            (x0, center),
            (x0 + bh * 0.35, center - bh / 2.0),
            (w - 2.0, center - bh / 2.0),
            (w - 2.0, center + bh / 2.0),
            (x0 + bh * 0.35, center + bh / 2.0),
        ];
        s.fill(scene::polygon(&pts), box_color);
        s.stroke(
            scene::polygon(&pts),
            st.primary,
            (cross * 0.025).max(1.0),
            Cap::Butt,
        );
        s.text(
            text,
            (x0 + bh * 0.35 + w - 2.0) / 2.0,
            center,
            bh * 0.55 * fs,
            st.font_weight,
            HAlign::Center,
            VAlign::Middle,
            st.text,
        );
    } else {
        let bw = (w * 0.24).max(label_size * 4.0);
        let bh = h * 0.5;
        let y1 = h - cross * 0.22;
        let pts = [
            (center - bw / 2.0, 2.0),
            (center + bw / 2.0, 2.0),
            (center + bw / 2.0, bh),
            (center + bh * 0.2, bh),
            (center, y1),
            (center - bh * 0.2, bh),
            (center - bw / 2.0, bh),
        ];
        s.fill(scene::polygon(&pts), box_color);
        s.stroke(
            scene::polygon(&pts),
            st.primary,
            (cross * 0.02).max(1.0),
            Cap::Butt,
        );
        s.text(
            text,
            center,
            bh / 2.0 + 1.0,
            bh * 0.55 * fs,
            st.font_weight,
            HAlign::Center,
            VAlign::Middle,
            st.text,
        );
    }
    if st.show_label {
        let lbl = g.label();
        if vertical {
            s.text(
                lbl,
                w / 2.0,
                label_size * 0.4 + 2.0,
                label_size * 0.8,
                FontWeight::Bold,
                HAlign::Center,
                VAlign::Top,
                st.accent,
            );
        } else {
            s.text(
                lbl,
                6.0,
                4.0,
                label_size * 0.8,
                FontWeight::Bold,
                HAlign::Left,
                VAlign::Top,
                st.accent,
            );
        }
    }
    s
}
