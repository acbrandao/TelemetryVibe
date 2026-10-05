//! Scene rasterizer shared by the interactive preview and the final export.
//!
//! Both paths call [`draw_scene`] with the same gauge scenes, guaranteeing the preview matches
//! the rendered video. The preview rasterizes each gauge at display resolution and uploads it
//! as a GPU texture; the export rasterizes at output resolution and pipes frames to FFmpeg.

use tiny_skia::{
    FillRule, GradientStop, LineCap, LinearGradient, PixmapMut, PixmapPaint, Point, SpreadMode,
    Stroke, Transform,
};

use super::text::fonts;
use crate::gauges::model::{Placement, Rgba};
use crate::gauges::scene::{Cap, HAlign, Paint, Prim, Scene, TextPrim, VAlign};

fn color(c: Rgba, opacity: f32) -> tiny_skia::Color {
    let [r, g, b, a] = c.0;
    tiny_skia::Color::from_rgba8(
        r,
        g,
        b,
        (a as f32 * opacity).round().clamp(0.0, 255.0) as u8,
    )
}

fn make_paint(p: &Paint, opacity: f32, override_color: Option<Rgba>) -> tiny_skia::Paint<'static> {
    let mut paint = tiny_skia::Paint {
        anti_alias: true,
        ..Default::default()
    };
    if let Some(c) = override_color {
        paint.set_color(color(c, opacity));
        return paint;
    }
    match p {
        Paint::Solid(c) => paint.set_color(color(*c, opacity)),
        Paint::Linear { start, end, stops } => {
            let stops: Vec<GradientStop> = stops
                .iter()
                .map(|(pos, c)| GradientStop::new(*pos, color(*c, opacity)))
                .collect();
            match LinearGradient::new(
                Point::from_xy(start.0, start.1),
                Point::from_xy(end.0, end.1),
                stops,
                SpreadMode::Pad,
                Transform::identity(),
            ) {
                Some(shader) => paint.shader = shader,
                None => paint.set_color(color(Rgba::WHITE, opacity)),
            }
        }
    }
    paint
}

/// Local → pixel transform for a gauge placement, given a uniform output `scale` and an
/// output-space `origin` offset (used when rendering a cropped overlay region).
pub fn placement_transform(p: &Placement, scale: f32, origin: (f32, f32)) -> Transform {
    let (cx, cy) = p.center();
    Transform::from_translate(-origin.0, -origin.1)
        .pre_scale(scale, scale)
        .pre_translate(cx, cy)
        .pre_rotate(p.rotation)
        .pre_translate(-p.w / 2.0, -p.h / 2.0)
}

/// Draws a scene with the given local→pixel transform and global opacity.
pub fn draw_scene(pixmap: &mut PixmapMut<'_>, scene: &Scene, ts: Transform, opacity: f32) {
    if opacity <= 0.001 {
        return;
    }
    if let Some(offset) = scene.shadow {
        let shadow = Rgba::rgba(0, 0, 0, 110);
        let sts = ts.pre_translate(offset * 0.4, offset);
        for prim in &scene.prims {
            draw_prim(pixmap, prim, sts, opacity, Some(shadow));
        }
    }
    for prim in &scene.prims {
        draw_prim(pixmap, prim, ts, opacity, None);
    }
}

fn draw_prim(
    pixmap: &mut PixmapMut<'_>,
    prim: &Prim,
    ts: Transform,
    opacity: f32,
    shadow: Option<Rgba>,
) {
    match prim {
        Prim::Fill { path, paint } => {
            // Mostly-transparent fills (panels) cast no meaningful shadow.
            if shadow.is_some() && is_faint(paint) {
                return;
            }
            let shadow = shadow.map(|s| scale_shadow(s, paint));
            let p = make_paint(paint, opacity, shadow);
            pixmap.fill_path(path, &p, FillRule::Winding, ts, None);
        }
        Prim::Stroke {
            path,
            paint,
            width,
            cap,
        } => {
            if shadow.is_some() && is_faint(paint) {
                return;
            }
            let shadow = shadow.map(|s| scale_shadow(s, paint));
            let p = make_paint(paint, opacity, shadow);
            let stroke = Stroke {
                width: *width,
                line_cap: match cap {
                    Cap::Butt => LineCap::Butt,
                    Cap::Round => LineCap::Round,
                    Cap::Square => LineCap::Square,
                },
                line_join: tiny_skia::LineJoin::Round,
                ..Default::default()
            };
            pixmap.stroke_path(path, &p, &stroke, ts, None);
        }
        Prim::Text(t) => {
            if shadow.is_some() && t.color.0[3] < 90 {
                return;
            }
            draw_text(pixmap, t, ts, opacity, shadow);
        }
        Prim::Image { image, x, y, w, h } => {
            if shadow.is_some() {
                return;
            }
            let iw = image.width() as f32;
            let ih = image.height() as f32;
            if iw <= 0.0 || ih <= 0.0 {
                return;
            }
            let its = ts.pre_translate(*x, *y).pre_scale(w / iw, h / ih);
            let paint = PixmapPaint {
                opacity,
                quality: tiny_skia::FilterQuality::Bicubic,
                ..Default::default()
            };
            pixmap.draw_pixmap(0, 0, image.as_ref().as_ref(), &paint, its, None);
        }
    }
}

fn is_faint(p: &Paint) -> bool {
    match p {
        Paint::Solid(c) => c.0[3] < 90,
        Paint::Linear { stops, .. } => stops.iter().all(|(_, c)| c.0[3] < 90),
    }
}

/// Shadows of translucent elements are proportionally lighter.
fn scale_shadow(s: Rgba, p: &Paint) -> Rgba {
    let a = match p {
        Paint::Solid(c) => c.0[3] as f32 / 255.0,
        Paint::Linear { stops, .. } => stops
            .iter()
            .map(|(_, c)| c.0[3] as f32 / 255.0)
            .fold(0.0, f32::max),
    };
    s.with_alpha_mul(a)
}

fn draw_text(
    pixmap: &mut PixmapMut<'_>,
    t: &TextPrim,
    ts: Transform,
    opacity: f32,
    shadow: Option<Rgba>,
) {
    let f = fonts();
    let layout = f.layout(&t.text, t.weight, t.tabular);
    let scale = t.size / f.units_per_em();
    let width = layout.width * scale;
    let x0 = match t.h_align {
        HAlign::Left => 0.0,
        HAlign::Center => -width / 2.0,
        HAlign::Right => -width,
    };
    let cap = f.cap_height_em() * t.size;
    let baseline = match t.v_align {
        VAlign::Baseline => 0.0,
        VAlign::Middle => cap / 2.0,
        VAlign::Top => cap,
    };
    let base_ts = ts.pre_translate(t.x, t.y).pre_rotate(t.rotation);
    let c = shadow
        .map(|s| s.with_alpha_mul(t.color.0[3] as f32 / 255.0))
        .unwrap_or(t.color);
    let paint = make_paint(&Paint::Solid(c), opacity, None);
    for (id, gx) in &layout.glyphs {
        if let Some(path) = f.glyph_path(t.weight, *id) {
            let gts = base_ts
                .pre_translate(x0 + gx * scale, baseline)
                .pre_scale(scale, -scale);
            pixmap.fill_path(&path, &paint, FillRule::Winding, gts, None);
        }
    }
}

/// Converts premultiplied RGBA (tiny-skia) into straight-alpha RGBA in place (for FFmpeg).
pub fn demultiply_in_place(data: &mut [u8]) {
    for px in data.as_chunks_mut::<4>().0 {
        let a = px[3] as u32;
        if a == 0 || a == 255 {
            continue;
        }
        for c in &mut px[..3] {
            *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gauges::model::FontWeight;
    use crate::gauges::scene;

    #[test]
    fn draws_shapes_and_text() {
        let mut pm = tiny_skia::Pixmap::new(100, 50).unwrap();
        let mut s = Scene::new();
        s.fill(scene::rect(0.0, 0.0, 10.0, 10.0), Rgba::rgb(255, 0, 0));
        s.text(
            "88",
            60.0,
            25.0,
            30.0,
            FontWeight::Bold,
            HAlign::Center,
            VAlign::Middle,
            Rgba::WHITE,
        );
        draw_scene(&mut pm.as_mut(), &s, Transform::identity(), 1.0);
        let px = pm.pixel(5, 5).unwrap();
        assert_eq!((px.red(), px.alpha()), (255, 255));
        // Some text pixels were painted around the center.
        let painted = (40..80)
            .flat_map(|x| (10..40).map(move |y| (x, y)))
            .filter(|&(x, y)| pm.pixel(x, y).unwrap().alpha() > 128)
            .count();
        assert!(painted > 50, "{painted}");
    }

    #[test]
    fn demultiply() {
        let mut d = [64u8, 32, 0, 128, 10, 10, 10, 0];
        demultiply_in_place(&mut d);
        assert_eq!(&d[..4], &[128, 64, 0, 128]);
    }

    #[test]
    fn transform_places_gauge() {
        let p = Placement {
            x: 10.0,
            y: 20.0,
            w: 30.0,
            h: 40.0,
            rotation: 0.0,
        };
        let ts = placement_transform(&p, 2.0, (0.0, 0.0));
        let mut pt = [Point::from_xy(0.0, 0.0)];
        ts.map_points(&mut pt);
        assert!((pt[0].x - 20.0).abs() < 1e-4 && (pt[0].y - 40.0).abs() < 1e-4);
    }
}
