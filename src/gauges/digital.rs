//! Digital readouts, time displays, text and image overlays.

use chrono::{Duration, Local, Offset};

use super::model::{DigitalIcon, FontWeight, Gauge, GaugeKind, Rgba, TextAlign, TimeFormat};
use super::scene::{self, HAlign, Scene, VAlign};
use super::{
    RenderCtx, format_gauge_value, gauge_unit_label, gauge_value, load_image, panel,
    widest_value_template, zone_color,
};
use crate::render::text::fonts;
use crate::utils::timecode::format_timecode;

fn h_align(a: TextAlign) -> HAlign {
    match a {
        TextAlign::Left => HAlign::Left,
        TextAlign::Center => HAlign::Center,
        TextAlign::Right => HAlign::Right,
    }
}

/// Height-derived layout shared by every digital readout. The number size depends only on the
/// gauge height (and font scale), so readouts of the same height show equally sized numbers;
/// presets get a width that fits their content (see [`natural_width`]).
struct BlockMetrics {
    pad: f32,
    label_size: f32,
    /// Top of the number area.
    top: f32,
    avail_h: f32,
    /// Number font size before any width-fitting.
    size: f32,
}

fn block_metrics(g: &Gauge, width: f32, has_label: bool) -> BlockMetrics {
    let st = &g.style;
    let h = g.placement.h;
    let cap = fonts().cap_height_em();
    let pad = (h * 0.12).min(width * 0.08);
    let fs = st.font_scale.max(0.1);
    let label_size = (h * 0.17 * fs).max(1.0);
    let top = pad
        + if has_label {
            label_size * cap + h * 0.1
        } else {
            0.0
        };
    let avail_h = (h - pad - top).max(1.0);
    BlockMetrics {
        pad,
        label_size,
        top,
        avail_h,
        size: avail_h / cap * fs.min(1.0),
    }
}

const UNIT_RATIO: f32 = 0.36;

/// Width of number + unit at font `size`, using the wider of `text` and `template`.
fn number_width(g: &Gauge, text: &str, template: &str, unit: &str, size: f32) -> f32 {
    let f = fonts();
    let weight = g.style.font_weight;
    let tw = f
        .measure(template, size, weight, true)
        .max(f.measure(text, size, weight, true));
    let gap = if unit.is_empty() { 0.0 } else { size * 0.12 };
    tw + gap + f.measure(unit, size * UNIT_RATIO, FontWeight::Bold, false)
}

/// Icon column width (padding + icon) for readouts with an icon.
pub fn icon_inset(h: f32) -> f32 {
    h * 0.12 + h * 0.62
}

/// Template for a time readout, so its width does not change as digits change.
fn time_template(format: TimeFormat, text: &str) -> String {
    match format {
        TimeFormat::Date => "00 May 0000".to_string(),
        TimeFormat::TimeOfDay => "00:00:00".to_string(),
        _ => text
            .chars()
            .map(|c| if c.is_ascii_digit() { '0' } else { c })
            .collect(),
    }
}

/// Width a digital-style gauge (readout, icon readout, EKG, time) needs so its number is not
/// shrunk to fit, i.e. is sized by the gauge height alone. `None` for other kinds.
pub fn natural_width(g: &Gauge, ctx: &RenderCtx<'_>) -> Option<f32> {
    let h = g.placement.h;
    let st = &g.style;
    let unit = || {
        if st.show_units {
            gauge_unit_label(g, ctx)
        } else {
            ""
        }
    };
    let (left, template, unit) = match &g.kind {
        GaugeKind::Digital { icon, .. } => (
            if *icon == DigitalIcon::None {
                0.0
            } else {
                icon_inset(h)
            },
            widest_value_template(g, ctx),
            unit(),
        ),
        GaugeKind::Ekg { show_value, .. } => {
            if !*show_value {
                return Some((h * 2.6).round());
            }
            (
                super::ekg::strip_inset(h),
                widest_value_template(g, ctx),
                unit(),
            )
        }
        GaugeKind::Time { format, .. } => {
            let sample = match format {
                TimeFormat::VideoElapsed => "00:00",
                _ => "00:00:00",
            };
            (0.0, time_template(*format, sample), "")
        }
        _ => return None,
    };
    let m = block_metrics(g, f32::INFINITY, st.show_label);
    let content = number_width(g, &template, &template, unit, m.size);
    Some((left + m.pad * 2.0 + content * 1.04).ceil())
}

/// Lays out a big value with optional label and unit inside the gauge box, starting `left`
/// units from the left edge (space reserved for an icon or EKG strip).
#[allow(clippy::too_many_arguments)]
pub(crate) fn value_block(
    s: &mut Scene,
    g: &Gauge,
    text: &str,
    template: &str,
    unit: &str,
    label: Option<String>,
    align: TextAlign,
    number_color: Rgba,
    left: f32,
) {
    let st = &g.style;
    let w = g.placement.w - left;
    let f = fonts();
    let cap = f.cap_height_em();
    let m = block_metrics(g, w, label.is_some());
    let pad = m.pad;
    if let Some(label) = label {
        let lx = left
            + match align {
                TextAlign::Left => pad,
                TextAlign::Center => w / 2.0,
                TextAlign::Right => w - pad,
            };
        s.text(
            label,
            lx,
            pad,
            m.label_size,
            FontWeight::Bold,
            h_align(align),
            VAlign::Top,
            st.secondary,
        );
    }
    let mut size = m.size;
    // Shrink only when the box is too narrow (e.g. resized by the user).
    let max_w = w - pad * 2.0;
    let wneed = number_width(g, text, template, unit, size);
    if wneed > max_w && wneed > 0.0 {
        size *= max_w / wneed;
    }
    let gap = if unit.is_empty() { 0.0 } else { size * 0.12 };
    let baseline = m.top + m.avail_h / 2.0 + size * cap / 2.0;
    let tw = f.measure(text, size, st.font_weight, true);
    let uw = f.measure(unit, size * UNIT_RATIO, FontWeight::Bold, false);
    let total = tw + gap + uw;
    let x0 = left
        + match align {
            TextAlign::Left => pad,
            TextAlign::Center => (w - total) / 2.0,
            TextAlign::Right => w - pad - total,
        };
    s.text(
        text,
        x0,
        baseline,
        size,
        st.font_weight,
        HAlign::Left,
        VAlign::Baseline,
        number_color,
    );
    if !unit.is_empty() {
        s.text(
            unit,
            x0 + tw + gap,
            baseline,
            size * UNIT_RATIO,
            FontWeight::Bold,
            HAlign::Left,
            VAlign::Baseline,
            st.secondary.with_alpha(230),
        );
    }
}

pub fn build_readout(g: &Gauge, ctx: &RenderCtx<'_>, align: TextAlign, icon: DigitalIcon) -> Scene {
    let mut s = Scene::new();
    let value = gauge_value(g, ctx);
    let zc = zone_color(g, value);
    let (w, h) = (g.placement.w, g.placement.h);
    panel(&mut s, g, w, h, zc);
    // Icon column on the left.
    let left = match icon {
        DigitalIcon::None => 0.0,
        _ => {
            let (pad, size) = (h * 0.12, h * 0.62);
            let (cx, cy) = (pad + size / 2.0, h / 2.0);
            match icon {
                DigitalIcon::Heart => draw_heart(&mut s, g, ctx, cx, cy, size, value.is_some()),
                DigitalIcon::Slope => draw_slope(&mut s, g, cx, cy, size, value, zc),
                DigitalIcon::None => {}
            }
            icon_inset(h)
        }
    };
    let text = format_gauge_value(g, ctx, value);
    let template = widest_value_template(g, ctx);
    let unit = if g.style.show_units {
        gauge_unit_label(g, ctx)
    } else {
        ""
    };
    let color = match zc {
        Some(c) if g.zone_targets.number => c,
        _ => g.style.text,
    };
    let label = g.style.show_label.then(|| g.label());
    value_block(&mut s, g, &text, &template, unit, label, align, color, left);
    s
}

/// Pulse envelope for a "lub-dub" heartbeat at `phase` (0..1): a strong beat then a softer one.
pub fn heartbeat_pulse(phase: f64) -> f32 {
    let bump = |center: f64, width: f64| (1.0 - ((phase - center) / width).abs()).max(0.0);
    let p = bump(0.06, 0.12).powi(2) + 0.55 * bump(0.30, 0.10).powi(2);
    p.min(1.0) as f32
}

/// Heart outline (classic parametric heart), centered on (cx, cy), `size` wide.
fn heart_points(cx: f32, cy: f32, size: f32) -> Vec<(f32, f32)> {
    let k = size / 34.0;
    (0..64)
        .map(|i| {
            let t = i as f32 / 64.0 * std::f32::consts::TAU;
            let x = 16.0 * t.sin().powi(3);
            let y =
                13.0 * t.cos() - 5.0 * (2.0 * t).cos() - 2.0 * (3.0 * t).cos() - (4.0 * t).cos();
            // The curve spans y ≈ -17..12; shift so the shape is visually centered.
            (cx + x * k, cy - (y + 2.5) * k)
        })
        .collect()
}

/// A heart that beats at the current heart rate (phase from the cumulative beat count).
fn draw_heart(
    s: &mut Scene,
    g: &Gauge,
    ctx: &RenderCtx<'_>,
    cx: f32,
    cy: f32,
    size: f32,
    has_value: bool,
) {
    let st = &g.style;
    let beat = if has_value {
        ctx.track
            .and_then(|t| t.beat_phase(ctx.gps_t()))
            .map(heartbeat_pulse)
            .unwrap_or(0.0)
    } else {
        0.0
    };
    let scale = 0.80 + 0.22 * beat;
    let pts = heart_points(cx, cy, size * scale);
    let color = if has_value {
        st.primary.with_alpha_mul(0.55 + 0.45 * beat)
    } else {
        st.secondary.with_alpha_mul(0.5)
    };
    s.glow_fill(
        scene::polygon(&pts),
        color,
        (st.glow.max(0.35) * beat).min(1.0),
        size * 0.18,
    );
    // Small glint on the upper-left lobe for a little depth.
    let k = size * scale;
    s.fill(
        scene::circle(cx - k * 0.2, cy - k * 0.2, k * 0.07),
        Rgba::WHITE.with_alpha_mul(0.25 + 0.3 * beat),
    );
}

/// A wedge whose slope follows the gradient (exaggerated ×6 so everyday grades are visible).
fn draw_slope(
    s: &mut Scene,
    g: &Gauge,
    cx: f32,
    cy: f32,
    size: f32,
    value: Option<f64>,
    zc: Option<Rgba>,
) {
    let st = &g.style;
    let grade = value.unwrap_or(0.0) as f32;
    let angle = (grade / 100.0 * 6.0).atan().clamp(-0.85, 0.85);
    let half = size / 2.0;
    let rise = (half * 2.0 * angle.tan()).clamp(-size * 0.8, size * 0.8);
    let base_y = cy + size * 0.3;
    let (x0, x1) = (cx - half, cx + half);
    let pts = if rise >= 0.0 {
        vec![(x0, base_y), (x1, base_y), (x1, base_y - rise)]
    } else {
        vec![(x0, base_y + rise), (x0, base_y), (x1, base_y)]
    };
    let color = zc.unwrap_or(st.primary);
    // Ground line, then the wedge.
    s.stroke(
        scene::line(x0, base_y, x1, base_y),
        st.secondary,
        (size * 0.05).max(1.0),
        scene::Cap::Round,
    );
    if rise.abs() > 0.5 {
        s.glow_fill(scene::polygon(&pts), color, st.glow, size * 0.1);
    }
}

fn format_elapsed(seconds: f64) -> String {
    let neg = seconds < 0.0;
    let total = seconds.abs().floor() as u64;
    let (h, m, sec) = (total / 3600, (total / 60) % 60, total % 60);
    let sign = if neg { "-" } else { "" };
    if h > 0 {
        format!("{sign}{h}:{m:02}:{sec:02}")
    } else {
        format!("{sign}{m:02}:{sec:02}")
    }
}

/// System timezone offset in minutes (used as the default for new time gauges).
pub fn local_tz_offset_minutes() -> i32 {
    Local::now().offset().fix().local_minus_utc() / 60
}

pub fn time_text(ctx: &RenderCtx<'_>, format: TimeFormat, tz_offset_minutes: i32) -> String {
    let gps_t = ctx.gps_t();
    match format {
        TimeFormat::VideoElapsed => format_elapsed(ctx.video_t),
        TimeFormat::RecordingElapsed => match ctx.track {
            Some(_) => format_elapsed(gps_t),
            None => "--:--".into(),
        },
        TimeFormat::LapTime => match ctx.track.and_then(|t| t.lap_at(gps_t)) {
            Some((_, lap)) => format_elapsed(gps_t - lap.start),
            None => match ctx.track {
                Some(_) => format_elapsed(gps_t),
                None => "--:--".into(),
            },
        },
        TimeFormat::Date | TimeFormat::TimeOfDay => match ctx.track {
            Some(track) => {
                let wall = track.wall_time(gps_t) + Duration::minutes(tz_offset_minutes as i64);
                if format == TimeFormat::Date {
                    wall.format("%d %b %Y").to_string()
                } else {
                    wall.format("%H:%M:%S").to_string()
                }
            }
            None => {
                if format == TimeFormat::Date {
                    "-- --- ----".into()
                } else {
                    format_timecode(ctx.video_t)[..8].to_string()
                }
            }
        },
    }
}

pub fn build_time(g: &Gauge, ctx: &RenderCtx<'_>, format: TimeFormat, tz: i32) -> Scene {
    let mut s = Scene::new();
    panel(&mut s, g, g.placement.w, g.placement.h, None);
    let text = format!(
        "{}{}{}",
        g.style.prefix,
        time_text(ctx, format, tz),
        g.style.suffix
    );
    let template = time_template(format, &text);
    let label = g.style.show_label.then(|| {
        if g.style.label.is_empty() {
            match format {
                TimeFormat::Date => "DATE",
                TimeFormat::TimeOfDay => "TIME",
                TimeFormat::VideoElapsed => "ELAPSED",
                TimeFormat::RecordingElapsed => "RIDE TIME",
                TimeFormat::LapTime => "LAP",
            }
            .to_string()
        } else {
            g.style.label.clone()
        }
    });
    value_block(
        &mut s,
        g,
        &text,
        &template,
        "",
        label,
        TextAlign::Left,
        g.style.text,
        0.0,
    );
    s
}

/// Expands `{date}` and `{time}` placeholders.
fn expand_placeholders(text: &str, ctx: &RenderCtx<'_>) -> String {
    if !text.contains('{') {
        return text.to_string();
    }
    let tz = local_tz_offset_minutes();
    text.replace("{date}", &time_text(ctx, TimeFormat::Date, tz))
        .replace("{time}", &time_text(ctx, TimeFormat::TimeOfDay, tz))
}

pub fn build_text(g: &Gauge, ctx: &RenderCtx<'_>, text: &str, align: TextAlign) -> Scene {
    let mut s = Scene::new();
    let (w, h) = (g.placement.w, g.placement.h);
    panel(&mut s, g, w, h, None);
    let f = fonts();
    let text = expand_placeholders(text, ctx);
    let pad = h * 0.15;
    let mut size = (h - pad * 2.0) / f.cap_height_em() * g.style.font_scale.max(0.1);
    let tw = f.measure(&text, size, g.style.font_weight, false);
    if tw > w - pad * 2.0 && tw > 0.0 {
        size *= (w - pad * 2.0) / tw;
    }
    let x = match align {
        TextAlign::Left => pad,
        TextAlign::Center => w / 2.0,
        TextAlign::Right => w - pad,
    };
    let mut t = scene::TextPrim {
        text,
        x,
        y: h / 2.0,
        size,
        weight: g.style.font_weight,
        h_align: h_align(align),
        v_align: VAlign::Middle,
        color: g.style.text,
        tabular: false,
        rotation: 0.0,
    };
    if g.style.glow > 0.01 {
        // Soft glow behind the text.
        let mut halo = t.clone();
        halo.color = g.style.accent.with_alpha_mul(0.35 * g.style.glow);
        s.push_text(halo);
    }
    t.tabular = false;
    s.push_text(t);
    s
}

pub fn build_image(g: &Gauge, path: &str) -> Scene {
    let mut s = Scene::new();
    let (w, h) = (g.placement.w, g.placement.h);
    if let Some(img) = load_image(path) {
        let (iw, ih) = (img.width() as f32, img.height() as f32);
        let k = (w / iw).min(h / ih);
        let (dw, dh) = (iw * k, ih * k);
        s.prims.push(scene::Prim::Image {
            image: img,
            x: (w - dw) / 2.0,
            y: (h - dh) / 2.0,
            w: dw,
            h: dh,
        });
    } else {
        // Placeholder so the box remains visible and selectable.
        s.stroke(
            scene::rect(1.0, 1.0, w - 2.0, h - 2.0),
            g.style.secondary.with_alpha_mul(0.5),
            2.0,
            scene::Cap::Butt,
        );
        s.text(
            "IMAGE",
            w / 2.0,
            h / 2.0,
            h * 0.18,
            FontWeight::Bold,
            HAlign::Center,
            VAlign::Middle,
            g.style.secondary,
        );
    }
    s.shadow = None;
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elapsed_format() {
        assert_eq!(format_elapsed(65.4), "01:05");
        assert_eq!(format_elapsed(3725.0), "1:02:05");
        assert_eq!(format_elapsed(-5.0), "-00:05");
    }

    #[test]
    fn heartbeat_envelope() {
        assert!(heartbeat_pulse(0.06) > 0.95, "beat peak");
        assert!(heartbeat_pulse(0.30) > 0.5, "second beat");
        assert!(heartbeat_pulse(0.7) < 0.01, "rest between beats");
        assert!((0..100).all(|i| (0.0..=1.0).contains(&heartbeat_pulse(i as f64 / 100.0))));
    }
}
