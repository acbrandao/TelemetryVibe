//! Progress bars and effort/zone gauges.

use super::model::{FontWeight, Gauge, Orientation};
use super::scene::{self, HAlign, Scene, VAlign};
use super::{RenderCtx, format_gauge_value, gauge_unit_label, gauge_value, panel, zone_color};

#[allow(clippy::too_many_arguments)]
pub fn build_bar(
    g: &Gauge,
    ctx: &RenderCtx<'_>,
    orientation: Orientation,
    segments: u32,
    rounded: bool,
    thickness: f32,
    show_value: bool,
) -> Scene {
    let mut s = Scene::new();
    let st = &g.style;
    let (w, h) = (g.placement.w, g.placement.h);
    let value = gauge_value(g, ctx);
    let frac = value.map(|v| g.fraction(v) as f32).unwrap_or(0.0);
    let zc = zone_color(g, value);
    panel(&mut s, g, w, h, zc);
    let fs = st.font_scale.max(0.1);
    let pad = w.min(h) * 0.12;
    let unit = if st.show_units {
        gauge_unit_label(g, ctx)
    } else {
        ""
    };
    let value_text = if unit.is_empty() {
        format_gauge_value(g, ctx, value)
    } else {
        format!("{} {}", format_gauge_value(g, ctx, value), unit)
    };
    let has_text = st.show_label || show_value;
    let vertical = orientation == Orientation::Vertical;

    // Text area and bar rect.
    let (bx, by, bw, bh) = if vertical {
        let text_h = if has_text { h * 0.16 } else { 0.0 };
        let ts = (w * 0.2).min(h * 0.07) * fs;
        if st.show_label {
            s.text(
                g.label(),
                w / 2.0,
                pad,
                ts,
                FontWeight::Bold,
                HAlign::Center,
                VAlign::Top,
                st.secondary,
            );
        }
        if show_value {
            s.text(
                value_text.clone(),
                w / 2.0,
                h - pad,
                ts * 1.2,
                st.font_weight,
                HAlign::Center,
                VAlign::Baseline,
                st.text,
            );
        }
        let avail_w = w - pad * 2.0;
        let bar_w = (avail_w * thickness.clamp(0.05, 1.0)).max(1.0);
        let top = pad + if st.show_label { text_h * 0.6 } else { 0.0 };
        let bottom = h - pad - if show_value { text_h * 0.7 } else { 0.0 };
        ((w - bar_w) / 2.0, top, bar_w, (bottom - top).max(1.0))
    } else {
        let text_h = if has_text { h * 0.42 } else { 0.0 };
        let ts = text_h * 0.62 * fs;
        if st.show_label {
            s.text(
                g.label(),
                pad,
                pad,
                ts,
                FontWeight::Bold,
                HAlign::Left,
                VAlign::Top,
                st.secondary,
            );
        }
        if show_value {
            s.text(
                value_text,
                w - pad,
                pad,
                ts * 1.15,
                st.font_weight,
                HAlign::Right,
                VAlign::Top,
                st.text,
            );
        }
        let avail_h = h - pad * 2.0 - text_h;
        let bar_h = (avail_h * thickness.clamp(0.05, 1.0)).max(1.0);
        (
            pad,
            pad + text_h + (avail_h - bar_h) / 2.0,
            w - pad * 2.0,
            bar_h,
        )
    };
    let cross = if vertical { bw } else { bh };
    let radius = if rounded { cross / 2.0 } else { cross * 0.1 };
    let fill_color = match zc {
        Some(c) if g.zone_targets.bar => c,
        _ => st.primary,
    };

    if segments >= 2 {
        let n = segments.min(100);
        let along = if vertical { bh } else { bw };
        let seg = along / n as f32;
        let gap = seg * 0.18;
        for i in 0..n {
            let center_f = (i as f32 + 0.5) / n as f32;
            let lit = center_f <= frac && value.is_some();
            let seg_val = g.min + (g.max - g.min) * center_f as f64;
            let base = if g.zone_targets.bar {
                g.zone_for(seg_val)
                    .map(|(_, z)| z.color)
                    .unwrap_or(st.primary)
            } else {
                st.primary
            };
            let col = if lit {
                base
            } else {
                st.secondary.with_alpha_mul(0.22)
            };
            let r = if rounded {
                (seg - gap).min(cross) * 0.3
            } else {
                0.0
            };
            let path = if vertical {
                let y = by + bh - (i as f32 + 1.0) * seg + gap / 2.0;
                scene::rounded_rect(bx, y, bw, seg - gap, r)
            } else {
                scene::rounded_rect(bx + i as f32 * seg + gap / 2.0, by, seg - gap, bh, r)
            };
            s.fill(path, col);
        }
    } else {
        s.fill(
            scene::rounded_rect(bx, by, bw, bh, radius),
            st.secondary.with_alpha_mul(0.25),
        );
        if frac > 0.001 && value.is_some() {
            let path = if vertical {
                let fh = (bh * frac).max(cross.min(bh));
                scene::rounded_rect(bx, by + bh - fh, bw, fh, radius)
            } else {
                let fw = (bw * frac).max(cross.min(bw));
                scene::rounded_rect(bx, by, fw, bh, radius)
            };
            s.glow_fill(path, fill_color, st.glow, cross * 0.3);
        }
    }
    s
}

pub fn build_zone(g: &Gauge, ctx: &RenderCtx<'_>, show_value: bool) -> Scene {
    let mut s = Scene::new();
    let st = &g.style;
    let (w, h) = (g.placement.w, g.placement.h);
    let value = gauge_value(g, ctx);
    let current = value.and_then(|v| g.zone_for(v));
    panel(&mut s, g, w, h, current.map(|(_, z)| z.color));
    let fs = st.font_scale.max(0.1);
    let pad = h.min(w) * 0.1;
    let head = (h * 0.2).min(w * 0.065) * fs;

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
    if show_value {
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
    // Zone name.
    let zone_text = match current {
        Some((i, z)) if !z.label.is_empty() => format!("Z{} · {}", i + 1, z.label.to_uppercase()),
        Some((i, _)) => format!("ZONE {}", i + 1),
        None => "--".to_string(),
    };
    let name_y = pad + head + h * 0.08;
    s.text(
        zone_text,
        pad,
        name_y,
        head * 1.15,
        FontWeight::Black,
        HAlign::Left,
        VAlign::Top,
        current.map(|(_, z)| z.color).unwrap_or(st.text),
    );

    // Blocks.
    let n = g.zones.len().max(1);
    let blocks_top = name_y + head * 1.15 * 0.75 + h * 0.08;
    let blocks_h = (h - pad - blocks_top).max(2.0);
    let gap = w * 0.012;
    let bw = (w - pad * 2.0 - gap * (n as f32 - 1.0)) / n as f32;
    for (i, z) in g.zones.iter().enumerate() {
        let x = pad + i as f32 * (bw + gap);
        let active = current.is_some_and(|(ci, _)| ci == i);
        let (y, hh, col) = if active {
            (blocks_top, blocks_h, z.color)
        } else {
            (
                blocks_top + blocks_h * 0.3,
                blocks_h * 0.7,
                z.color.with_alpha_mul(0.3),
            )
        };
        s.fill(scene::rounded_rect(x, y, bw, hh, bw.min(hh) * 0.2), col);
        if active && let Some(v) = value {
            // Position within the zone.
            let z0 = z.from;
            let z1 = g.zone_end(i);
            let f = if z1 > z0 {
                ((v - z0) / (z1 - z0)).clamp(0.0, 1.0) as f32
            } else {
                0.5
            };
            let mx = x + bw * f;
            s.fill(scene::rect(mx - 1.5, y, 3.0, hh), st.text.with_alpha(230));
        }
    }
    s
}
