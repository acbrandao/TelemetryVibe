//! Analog dials and speedometers.

use super::model::{DialStyle, Gauge, GaugeKind, Rgba};
use super::scene::{self, Cap, HAlign, Scene, VAlign};
use super::{
    RenderCtx, display_value, format_gauge_value, gauge_unit_label, gauge_value, zone_color,
};
use crate::utils::nice_step;

/// (sweep, start angle) in degrees.
pub fn angles(g: &Gauge) -> (f32, f32) {
    let sweep = match g.kind {
        GaugeKind::Analog { sweep, .. } => sweep.clamp(30.0, 360.0),
        _ => 270.0,
    };
    (sweep, 90.0 + (360.0 - sweep) / 2.0)
}

/// Needle angle for a canonical value (min when unknown).
pub fn needle_angle(g: &Gauge, value: Option<f64>) -> f32 {
    let (sweep, start) = angles(g);
    let f = value.map(|v| g.fraction(v)).unwrap_or(0.0) as f32;
    start + f * sweep
}

struct Look {
    face: bool,
    rim: bool,
    needle: bool,
    value_arc: bool,
    track_arc: bool,
    segmented: bool,
    big_center_number: bool,
    tick_len: f32,
    needle_width: f32,
}

fn look(d: DialStyle) -> Look {
    match d {
        DialStyle::Automotive => Look {
            face: true,
            rim: true,
            needle: true,
            value_arc: false,
            track_arc: false,
            segmented: false,
            big_center_number: false,
            tick_len: 0.11,
            needle_width: 0.05,
        },
        DialStyle::Motorsport => Look {
            face: true,
            rim: false,
            needle: true,
            value_arc: true,
            track_arc: true,
            segmented: false,
            big_center_number: false,
            tick_len: 0.08,
            needle_width: 0.035,
        },
        DialStyle::Aviation => Look {
            face: true,
            rim: true,
            needle: true,
            value_arc: false,
            track_arc: false,
            segmented: false,
            big_center_number: false,
            tick_len: 0.13,
            needle_width: 0.07,
        },
        DialStyle::Minimal => Look {
            face: false,
            rim: false,
            needle: true,
            value_arc: false,
            track_arc: true,
            segmented: false,
            big_center_number: false,
            tick_len: 0.06,
            needle_width: 0.025,
        },
        DialStyle::ModernDigital => Look {
            face: true,
            rim: false,
            needle: false,
            value_arc: true,
            track_arc: true,
            segmented: false,
            big_center_number: true,
            tick_len: 0.0,
            needle_width: 0.0,
        },
        DialStyle::Sport => Look {
            face: true,
            rim: false,
            needle: false,
            value_arc: false,
            track_arc: false,
            segmented: true,
            big_center_number: true,
            tick_len: 0.0,
            needle_width: 0.0,
        },
        // Drawn by their own builders below.
        DialStyle::ZoneArc | DialStyle::LedRing | DialStyle::PeakArc | DialStyle::Compass => {
            look(DialStyle::Minimal)
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn build(
    g: &Gauge,
    ctx: &RenderCtx<'_>,
    dial: DialStyle,
    _sweep: f32,
    major_ticks: u32,
    minor_ticks: u32,
    show_readout: bool,
) -> Scene {
    match dial {
        DialStyle::ZoneArc => return build_zone_arc(g, ctx),
        DialStyle::LedRing => return build_led_ring(g, ctx),
        DialStyle::PeakArc => return build_peak_arc(g, ctx, major_ticks),
        DialStyle::Compass => return build_compass(g, ctx),
        _ => {}
    }
    let mut s = Scene::new();
    let st = &g.style;
    let (w, h) = (g.placement.w, g.placement.h);
    let size = w.min(h);
    let (cx, cy) = (w / 2.0, h / 2.0);
    let r = size / 2.0 * 0.97;
    let lk = look(dial);
    let (sweep, start) = angles(g);
    let end = start + sweep;
    let value = gauge_value(g, ctx);
    let frac = value.map(|v| g.fraction(v) as f32);
    let zc = zone_color(g, value);
    let fs = st.font_scale.max(0.1);
    let glow = st.glow;

    // Face.
    if lk.face && st.background_opacity > 0.001 {
        let bg = match zc {
            Some(c) if g.zone_targets.background => c,
            _ => st.background,
        };
        s.fill(
            scene::circle(cx, cy, r),
            bg.with_alpha_mul(st.background_opacity),
        );
    }
    if lk.rim {
        s.stroke(
            scene::circle(cx, cy, r * 0.985),
            st.secondary,
            r * 0.03,
            Cap::Butt,
        );
    }
    if st.border_width > 0.0 {
        s.stroke(
            scene::circle(cx, cy, r - st.border_width / 2.0),
            st.border,
            st.border_width,
            Cap::Butt,
        );
    }

    // Zones as colored bands.
    if g.zone_targets.arc && !g.zones.is_empty() && !lk.segmented {
        let (r0, r1) = if dial == DialStyle::Aviation {
            (r * 0.80, r * 0.90)
        } else {
            (r * 0.84, r * 0.89)
        };
        for (i, z) in g.zones.iter().enumerate() {
            let f0 = g.fraction(z.from) as f32;
            let f1 = g.fraction(g.zone_end(i)) as f32;
            if f1 > f0 {
                s.fill(
                    scene::annular_sector(cx, cy, r0, r1, start + f0 * sweep, start + f1 * sweep),
                    z.color.with_alpha_mul(0.85),
                );
            }
        }
    }

    // Track and value arcs.
    let arc_w = if dial == DialStyle::ModernDigital {
        r * 0.11
    } else {
        r * 0.06
    };
    let arc_r = if dial == DialStyle::ModernDigital {
        r * 0.84
    } else {
        r * 0.93
    };
    if lk.track_arc {
        let col = if dial == DialStyle::Minimal {
            st.secondary
        } else {
            st.secondary.with_alpha_mul(0.25)
        };
        let width = if dial == DialStyle::Minimal {
            r * 0.02
        } else {
            arc_w
        };
        s.stroke(
            scene::arc(cx, cy, arc_r, start, end),
            col,
            width,
            Cap::Round,
        );
    }
    if lk.value_arc
        && let Some(f) = frac
        && f > 0.002
    {
        let col = match zc {
            Some(c) if g.zone_targets.arc => c,
            _ => {
                if dial == DialStyle::Motorsport {
                    st.accent
                } else {
                    st.primary
                }
            }
        };
        s.glow_stroke(
            scene::arc(cx, cy, arc_r, start, start + f * sweep),
            col,
            arc_w,
            Cap::Round,
            if dial == DialStyle::ModernDigital {
                glow.max(0.5)
            } else {
                glow
            },
        );
    }

    // Segmented arc (Sport).
    if lk.segmented {
        let n = 36;
        let seg = sweep / n as f32;
        for i in 0..n {
            let a0 = start + i as f32 * seg + seg * 0.12;
            let a1 = start + (i + 1) as f32 * seg - seg * 0.12;
            let center_f = (i as f32 + 0.5) / n as f32;
            let lit = frac.is_some_and(|f| center_f <= f);
            let seg_val = g.min + (g.max - g.min) * center_f as f64;
            let base = if g.zone_targets.arc {
                g.zone_for(seg_val)
                    .map(|(_, z)| z.color)
                    .unwrap_or(st.primary)
            } else {
                st.primary
            };
            let col = if lit {
                base
            } else {
                st.secondary.with_alpha_mul(0.18)
            };
            s.fill(
                scene::annular_sector(cx, cy, r * 0.74, r * 0.92, a0, a1),
                col,
            );
        }
    }

    // Ticks and labels.
    let dmin = display_value(g, ctx, g.min);
    let dmax = display_value(g, ctx, g.max);
    let majors = if major_ticks == 0 {
        let step = nice_step((dmax - dmin).abs(), 6.0);
        (((dmax - dmin).abs() / step).round() as u32).clamp(1, 20)
    } else {
        major_ticks.clamp(1, 40)
    };
    if st.show_ticks && lk.tick_len > 0.0 {
        let outer = r * 0.93;
        let minors = minor_ticks.min(10);
        for i in 0..=majors {
            let a = start + sweep * i as f32 / majors as f32;
            let (x0, y0) = scene::polar(cx, cy, outer, a);
            let (x1, y1) = scene::polar(cx, cy, outer - r * lk.tick_len, a);
            s.stroke(
                scene::line(x0, y0, x1, y1),
                st.secondary.with_alpha(255),
                r * 0.022,
                Cap::Butt,
            );
            if i < majors {
                for m in 1..minors {
                    let am = a + sweep / majors as f32 * m as f32 / minors as f32;
                    let (x0, y0) = scene::polar(cx, cy, outer, am);
                    let (x1, y1) = scene::polar(cx, cy, outer - r * lk.tick_len * 0.55, am);
                    s.stroke(
                        scene::line(x0, y0, x1, y1),
                        st.secondary,
                        r * 0.01,
                        Cap::Butt,
                    );
                }
            }
            // Numeric labels.
            let v = dmin + (dmax - dmin) * i as f64 / majors as f64;
            let label = if (dmax - dmin).abs() < 10.0 {
                format!("{v:.1}")
            } else {
                format!("{}", v.round() as i64)
            };
            let (lx, ly) = scene::polar(cx, cy, outer - r * lk.tick_len - r * 0.14, a);
            s.text(
                label,
                lx,
                ly,
                r * 0.13 * fs,
                super::model::FontWeight::Medium,
                HAlign::Center,
                VAlign::Middle,
                st.text,
            );
        }
    }

    // Label and readout.
    let unit = if st.show_units {
        gauge_unit_label(g, ctx)
    } else {
        ""
    };
    let text = format_gauge_value(g, ctx, value);
    let number_color = match zc {
        Some(c) if g.zone_targets.number => c,
        _ => st.text,
    };
    if lk.big_center_number {
        s.text(
            text,
            cx,
            cy,
            r * 0.42 * fs,
            st.font_weight,
            HAlign::Center,
            VAlign::Middle,
            number_color,
        );
        s.text(
            unit,
            cx,
            cy + r * 0.36,
            r * 0.12 * fs,
            super::model::FontWeight::Medium,
            HAlign::Center,
            VAlign::Middle,
            st.secondary.with_alpha(220),
        );
        if st.show_label {
            s.text(
                g.label(),
                cx,
                cy - r * 0.36,
                r * 0.1 * fs,
                super::model::FontWeight::Bold,
                HAlign::Center,
                VAlign::Middle,
                st.secondary,
            );
        }
    } else {
        if st.show_label {
            s.text(
                g.label(),
                cx,
                cy - r * 0.32,
                r * 0.09 * fs,
                super::model::FontWeight::Bold,
                HAlign::Center,
                VAlign::Middle,
                st.secondary,
            );
        }
        if show_readout {
            s.text(
                text,
                cx,
                cy + r * 0.40,
                r * 0.24 * fs,
                st.font_weight,
                HAlign::Center,
                VAlign::Middle,
                number_color,
            );
            s.text(
                unit,
                cx,
                cy + r * 0.62,
                r * 0.10 * fs,
                super::model::FontWeight::Medium,
                HAlign::Center,
                VAlign::Middle,
                st.secondary.with_alpha(220),
            );
        } else if !unit.is_empty() {
            s.text(
                unit,
                cx,
                cy + r * 0.45,
                r * 0.11 * fs,
                super::model::FontWeight::Medium,
                HAlign::Center,
                VAlign::Middle,
                st.secondary.with_alpha(220),
            );
        }
    }

    // Needle.
    if lk.needle {
        let a = needle_angle(g, value);
        let needle_color = match zc {
            Some(c) if g.zone_targets.needle => c,
            _ => {
                if matches!(dial, DialStyle::Automotive | DialStyle::Motorsport) {
                    st.accent
                } else {
                    st.primary
                }
            }
        };
        let len = r * 0.86;
        let tail = r * 0.16;
        let half = r * lk.needle_width / 2.0;
        let (tx, ty) = scene::polar(cx, cy, len, a);
        let (bx, by) = scene::polar(cx, cy, tail, a + 180.0);
        let (l1x, l1y) = scene::polar(cx, cy, half, a - 90.0);
        let (r1x, r1y) = scene::polar(cx, cy, half, a + 90.0);
        let (lbx, lby) = (bx + (l1x - cx) * 1.2, by + (l1y - cy) * 1.2);
        let (rbx, rby) = (bx + (r1x - cx) * 1.2, by + (r1y - cy) * 1.2);
        let tip_half = half * 0.25;
        let (tlx, tly) = (
            tx + (l1x - cx) / half * tip_half,
            ty + (l1y - cy) / half * tip_half,
        );
        let (trx, try_) = (
            tx + (r1x - cx) / half * tip_half,
            ty + (r1y - cy) / half * tip_half,
        );
        s.glow_fill(
            scene::polygon(&[(lbx, lby), (tlx, tly), (trx, try_), (rbx, rby)]),
            needle_color,
            glow,
            r * 0.04,
        );
        // Hub.
        s.fill(scene::circle(cx, cy, r * 0.085), Rgba::rgb(30, 32, 36));
        s.fill(scene::circle(cx, cy, r * 0.05), needle_color);
    }
    s
}

// ---------------------------------------------------------------- zone-coded dial styles

/// Center, radius and dial geometry shared by the zone-coded styles.
struct Dial {
    cx: f32,
    cy: f32,
    r: f32,
    start: f32,
    sweep: f32,
}

impl Dial {
    fn new(g: &Gauge) -> Self {
        let (w, h) = (g.placement.w, g.placement.h);
        let (sweep, start) = angles(g);
        Self {
            cx: w / 2.0,
            cy: h / 2.0,
            r: w.min(h) / 2.0 * 0.97,
            start,
            sweep,
        }
    }
    fn angle(&self, f: f32) -> f32 {
        self.start + f.clamp(0.0, 1.0) * self.sweep
    }
}

/// Zone spans as (from, to) range fractions with their colors; the primary color when the
/// gauge has no zones.
fn zone_spans(g: &Gauge) -> Vec<(f32, f32, Rgba)> {
    if g.zones.is_empty() {
        return vec![(0.0, 1.0, g.style.primary)];
    }
    let mut out = Vec::new();
    for (i, z) in g.zones.iter().enumerate() {
        let f0 = g.fraction(z.from) as f32;
        let f1 = g.fraction(g.zone_end(i)) as f32;
        if f1 > f0 {
            out.push((f0, f1, z.color));
        }
    }
    if out.is_empty() {
        out.push((0.0, 1.0, g.style.primary));
    }
    out
}

/// Color at a range fraction: the zone color there, else the primary color.
fn color_at(g: &Gauge, f: f32) -> Rgba {
    let v = g.min + (g.max - g.min) * f as f64;
    g.zone_for(v)
        .map(|(_, z)| z.color)
        .unwrap_or(g.style.primary)
}

fn dial_face(s: &mut Scene, g: &Gauge, d: &Dial, zc: Option<Rgba>) {
    let st = &g.style;
    if st.background_opacity > 0.001 {
        let bg = match zc {
            Some(c) if g.zone_targets.background => c,
            _ => st.background,
        };
        s.fill(
            scene::circle(d.cx, d.cy, d.r),
            bg.with_alpha_mul(st.background_opacity),
        );
    }
    if st.border_width > 0.0 {
        s.stroke(
            scene::circle(d.cx, d.cy, d.r - st.border_width / 2.0),
            st.border,
            st.border_width,
            Cap::Butt,
        );
    }
}

/// Label above, big number in the middle, unit and an optional sub line (zone name, peak)
/// below.
fn center_readout(
    s: &mut Scene,
    g: &Gauge,
    ctx: &RenderCtx<'_>,
    d: &Dial,
    value: Option<f64>,
    zc: Option<Rgba>,
    sub: Option<(String, Rgba)>,
) {
    let st = &g.style;
    let fs = st.font_scale.max(0.1);
    let (cx, cy, r) = (d.cx, d.cy, d.r);
    if st.show_label {
        s.text(
            g.label(),
            cx,
            cy - r * 0.34,
            r * 0.1 * fs,
            super::model::FontWeight::Bold,
            HAlign::Center,
            VAlign::Middle,
            st.secondary,
        );
    }
    let number_color = match zc {
        Some(c) if g.zone_targets.number => c,
        _ => st.text,
    };
    s.text(
        format_gauge_value(g, ctx, value),
        cx,
        cy,
        r * 0.40 * fs,
        st.font_weight,
        HAlign::Center,
        VAlign::Middle,
        number_color,
    );
    if st.show_units {
        s.text(
            gauge_unit_label(g, ctx),
            cx,
            cy + r * 0.27,
            r * 0.11 * fs,
            super::model::FontWeight::Medium,
            HAlign::Center,
            VAlign::Middle,
            st.secondary.with_alpha(220),
        );
    }
    if let Some((text, color)) = sub
        && !text.is_empty()
    {
        s.text(
            text,
            cx,
            cy + r * 0.46,
            r * 0.095 * fs,
            super::model::FontWeight::Bold,
            HAlign::Center,
            VAlign::Middle,
            color,
        );
    }
}

/// Name of the zone containing the value, in its color.
fn zone_name(g: &Gauge, value: Option<f64>) -> Option<(String, Rgba)> {
    g.zone_for(value?)
        .map(|(_, z)| (z.label.to_uppercase(), z.color))
}

/// Wide color-coded zone segments: bright up to the value, dimmed beyond it, with a pointer.
fn build_zone_arc(g: &Gauge, ctx: &RenderCtx<'_>) -> Scene {
    let mut s = Scene::new();
    let st = &g.style;
    let d = Dial::new(g);
    let value = gauge_value(g, ctx);
    let frac = value.map(|v| g.fraction(v) as f32);
    let zc = zone_color(g, value);
    dial_face(&mut s, g, &d, zc);

    let (r0, r1) = (d.r * 0.70, d.r * 0.88);
    let gap = 1.6;
    for (f0, f1, col) in zone_spans(g) {
        let a0 = d.angle(f0) + if f0 > 0.0 { gap / 2.0 } else { 0.0 };
        let a1 = d.angle(f1) - if f1 < 1.0 { gap / 2.0 } else { 0.0 };
        if a1 <= a0 {
            continue;
        }
        match frac {
            Some(f) => {
                let av = d.angle(f).clamp(a0, a1);
                if av > a0 {
                    s.glow_fill(
                        scene::annular_sector(d.cx, d.cy, r0, r1, a0, av),
                        col,
                        st.glow,
                        d.r * 0.05,
                    );
                }
                if a1 > av {
                    s.fill(
                        scene::annular_sector(d.cx, d.cy, r0, r1, av, a1),
                        col.with_alpha_mul(0.25),
                    );
                }
            }
            None => s.fill(
                scene::annular_sector(d.cx, d.cy, r0, r1, a0, a1),
                col.with_alpha_mul(0.35),
            ),
        }
    }

    // Values at the zone boundaries (and the ends of the scale).
    if st.show_ticks {
        let mut marks: Vec<f32> = zone_spans(g).iter().map(|z| z.0).collect();
        marks.push(1.0);
        marks.dedup_by(|a, b| (*a - *b).abs() < 0.04);
        for f in marks {
            let v = display_value(g, ctx, g.min + (g.max - g.min) * f as f64);
            let (lx, ly) = scene::polar(d.cx, d.cy, d.r * 0.61, d.angle(f));
            s.text(
                format!("{}", v.round() as i64),
                lx,
                ly,
                d.r * 0.075 * st.font_scale.max(0.1),
                super::model::FontWeight::Medium,
                HAlign::Center,
                VAlign::Middle,
                st.secondary,
            );
        }
    }

    // Pointer: a triangle just outside the band plus a line across it.
    if let Some(f) = frac {
        let a = d.angle(f);
        let (ix, iy) = scene::polar(d.cx, d.cy, r0 - d.r * 0.02, a);
        let (ox, oy) = scene::polar(d.cx, d.cy, r1 + d.r * 0.01, a);
        s.stroke(
            scene::line(ix, iy, ox, oy),
            st.text,
            d.r * 0.025,
            Cap::Round,
        );
        let tip = scene::polar(d.cx, d.cy, r1 + d.r * 0.015, a);
        let b1 = scene::polar(d.cx, d.cy, d.r * 0.99, a - 4.5);
        let b2 = scene::polar(d.cx, d.cy, d.r * 0.99, a + 4.5);
        s.fill(scene::polygon(&[tip, b1, b2]), st.text);
    }
    center_readout(&mut s, g, ctx, &d, value, zc, zone_name(g, value));
    s
}

/// A ring of LED dots colored by zone, lit up to the value; the leading dot glows brightest.
fn build_led_ring(g: &Gauge, ctx: &RenderCtx<'_>) -> Scene {
    let mut s = Scene::new();
    let st = &g.style;
    let d = Dial::new(g);
    let value = gauge_value(g, ctx);
    let frac = value.map(|v| g.fraction(v) as f32);
    let zc = zone_color(g, value);
    dial_face(&mut s, g, &d, zc);

    let n = 32;
    let ring = d.r * 0.82;
    let pitch = ring * d.sweep.to_radians() / n as f32;
    let dot = (pitch * 0.36).min(d.r * 0.06);
    let lit_count = frac.map(|f| (f * n as f32).round() as usize).unwrap_or(0);
    for i in 0..n {
        let f = (i as f32 + 0.5) / n as f32;
        let (x, y) = scene::polar(d.cx, d.cy, ring, d.angle(f));
        let col = color_at(g, f);
        if i < lit_count {
            let lead = i + 1 == lit_count;
            let rr = if lead { dot * 1.25 } else { dot };
            s.glow_fill(
                scene::circle(x, y, rr),
                col,
                st.glow.max(if lead { 0.8 } else { 0.4 }),
                dot * 0.9,
            );
            if lead {
                s.fill(scene::circle(x, y, rr * 0.45), Rgba::WHITE.with_alpha(200));
            }
        } else {
            s.fill(scene::circle(x, y, dot * 0.85), col.with_alpha_mul(0.16));
        }
    }
    center_readout(&mut s, g, ctx, &d, value, zc, zone_name(g, value));
    s
}

/// Eight-point compass direction for a heading in degrees.
fn cardinal(heading: f64) -> &'static str {
    const NAMES: [&str; 8] = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];
    NAMES[((heading.rem_euclid(360.0) + 22.5) / 45.0) as usize % 8]
}

/// Heading indicator: a compass card that turns so the current heading sits under the fixed
/// lubber mark at the top, with the heading and its cardinal direction in the middle.
fn build_compass(g: &Gauge, ctx: &RenderCtx<'_>) -> Scene {
    let mut s = Scene::new();
    let st = &g.style;
    let fs = st.font_scale.max(0.1);
    let d = Dial::new(g);
    let (cx, cy, r) = (d.cx, d.cy, d.r);
    let value = gauge_value(g, ctx);
    dial_face(&mut s, g, &d, None);
    s.stroke(
        scene::circle(cx, cy, r * 0.985),
        st.secondary,
        r * 0.03,
        Cap::Butt,
    );

    // Screen angle of a bearing on the card: the heading points straight up (270°).
    let heading = value.map(|v| v as f32).unwrap_or(0.0);
    let at = |bearing: f32| 270.0 + bearing - heading;
    if st.show_ticks {
        let outer = r * 0.93;
        for i in 0..72 {
            let b = i as f32 * 5.0;
            let (len, width, col) = if i % 6 == 0 {
                (0.14, 0.022, st.secondary.with_alpha(255))
            } else if i % 2 == 0 {
                (0.09, 0.014, st.secondary)
            } else {
                (0.05, 0.01, st.secondary)
            };
            let (x0, y0) = scene::polar(cx, cy, outer, at(b));
            let (x1, y1) = scene::polar(cx, cy, outer - r * len, at(b));
            s.stroke(scene::line(x0, y0, x1, y1), col, r * width, Cap::Butt);
        }
    }
    // Card labels every 30°: letters at the cardinal points, tens of degrees elsewhere.
    for i in 0..12 {
        let b = i as f32 * 30.0;
        let (text, size, color) = match i {
            0 => ("N".to_string(), 0.16, st.accent),
            3 => ("E".to_string(), 0.16, st.text),
            6 => ("S".to_string(), 0.16, st.text),
            9 => ("W".to_string(), 0.16, st.text),
            _ => ((i * 3).to_string(), 0.11, st.secondary.with_alpha(230)),
        };
        let (lx, ly) = scene::polar(cx, cy, r * 0.64, at(b));
        s.text(
            text,
            lx,
            ly,
            r * size * fs,
            super::model::FontWeight::Bold,
            HAlign::Center,
            VAlign::Middle,
            color,
        );
    }

    // Fixed lubber mark at the top.
    let tip = (cx, cy - r * 0.8);
    s.glow_fill(
        scene::polygon(&[
            tip,
            (cx - r * 0.07, cy - r * 0.97),
            (cx + r * 0.07, cy - r * 0.97),
        ]),
        st.primary,
        st.glow,
        r * 0.04,
    );

    // Center readout: heading in degrees and its cardinal direction.
    if st.show_label {
        s.text(
            g.label(),
            cx,
            cy - r * 0.27,
            r * 0.09 * fs,
            super::model::FontWeight::Bold,
            HAlign::Center,
            VAlign::Middle,
            st.secondary,
        );
    }
    let unit = if st.show_units {
        gauge_unit_label(g, ctx)
    } else {
        ""
    };
    s.text(
        format!("{}{unit}", format_gauge_value(g, ctx, value)),
        cx,
        cy,
        r * 0.30 * fs,
        st.font_weight,
        HAlign::Center,
        VAlign::Middle,
        st.text,
    );
    if let Some(v) = value {
        s.text(
            cardinal(v),
            cx,
            cy + r * 0.27,
            r * 0.12 * fs,
            super::model::FontWeight::Bold,
            HAlign::Center,
            VAlign::Middle,
            st.primary,
        );
    }
    s
}

/// Seconds covered by the peak-hold marker.
pub const PEAK_WINDOW: f64 = 10.0;

/// Highest value over the last [`PEAK_WINDOW`] seconds (sampled every 0.5 s; stateless, so
/// preview and export agree).
pub fn peak_value(g: &Gauge, ctx: &RenderCtx<'_>) -> Option<f64> {
    let steps = (PEAK_WINDOW / 0.5) as i32;
    (0..=steps)
        .filter_map(|k| {
            let c = RenderCtx {
                video_t: ctx.video_t - k as f64 * 0.5,
                ..*ctx
            };
            gauge_value(g, &c)
        })
        .reduce(f64::max)
}

/// Thick zone-colored value arc with ticks and a peak-hold marker for the last 10 seconds.
fn build_peak_arc(g: &Gauge, ctx: &RenderCtx<'_>, major_ticks: u32) -> Scene {
    let mut s = Scene::new();
    let st = &g.style;
    let d = Dial::new(g);
    let value = gauge_value(g, ctx);
    let frac = value.map(|v| g.fraction(v) as f32);
    let zc = zone_color(g, value);
    dial_face(&mut s, g, &d, zc);

    let (arc_r, arc_w) = (d.r * 0.78, d.r * 0.13);
    s.stroke(
        scene::arc(d.cx, d.cy, arc_r, d.start, d.start + d.sweep),
        st.secondary.with_alpha_mul(0.2),
        arc_w,
        Cap::Butt,
    );
    if let Some(f) = frac {
        for (f0, f1, col) in zone_spans(g) {
            let end = f1.min(f);
            if end > f0 {
                s.glow_stroke(
                    scene::arc(d.cx, d.cy, arc_r, d.angle(f0), d.angle(end)),
                    col,
                    arc_w,
                    Cap::Butt,
                    st.glow,
                );
            }
        }
    }

    // Ticks outside the arc, min/max labels at the ends.
    if st.show_ticks {
        let n = if major_ticks == 0 {
            10
        } else {
            major_ticks.clamp(2, 40)
        };
        for i in 0..=n {
            let a = d.angle(i as f32 / n as f32);
            let (x0, y0) = scene::polar(d.cx, d.cy, arc_r + arc_w * 0.65, a);
            let (x1, y1) = scene::polar(d.cx, d.cy, d.r * 0.97, a);
            s.stroke(
                scene::line(x0, y0, x1, y1),
                st.secondary,
                d.r * 0.015,
                Cap::Butt,
            );
        }
        for (f, v) in [(0.0, g.min), (1.0, g.max)] {
            let (lx, ly) = scene::polar(d.cx, d.cy, d.r * 0.58, d.angle(f));
            s.text(
                format!("{}", display_value(g, ctx, v).round() as i64),
                lx,
                ly,
                d.r * 0.08 * st.font_scale.max(0.1),
                super::model::FontWeight::Medium,
                HAlign::Center,
                VAlign::Middle,
                st.secondary,
            );
        }
    }

    // Peak hold.
    let peak = peak_value(g, ctx);
    if let Some(p) = peak {
        let a = d.angle(g.fraction(p) as f32);
        let (x0, y0) = scene::polar(d.cx, d.cy, arc_r - arc_w * 0.7, a);
        let (x1, y1) = scene::polar(d.cx, d.cy, arc_r + arc_w * 0.7, a);
        s.stroke(scene::line(x0, y0, x1, y1), st.text, d.r * 0.03, Cap::Round);
    }
    let sub = peak.map(|p| {
        (
            format!("PEAK {}", format_gauge_value(g, ctx, Some(p))),
            st.secondary,
        )
    });
    center_readout(&mut s, g, ctx, &d, value, zc, sub);
    s
}
