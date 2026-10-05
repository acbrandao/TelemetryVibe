//! Overlay composition shared by preview and export.

use egui::ColorImage;
use tiny_skia::{Pixmap, Transform};

use super::raster::{demultiply_in_place, draw_scene, placement_transform};
use crate::gauges::model::Gauge;
use crate::gauges::{RenderCtx, build_scene};
use crate::video::encoder::OverlayRegion;

/// Extra pixels around a gauge for shadows and glow (gauge-local units).
pub fn gauge_margin(g: &Gauge) -> f32 {
    let base = g.placement.w.max(g.placement.h);
    let glow = if g.style.glow > 0.0 { 0.06 } else { 0.0 };
    (base * (0.025 + glow)).max(4.0)
}

/// Union of all visible gauge bounds in output pixels.
pub fn overlay_region(
    gauges: &[Gauge],
    scale: f32,
    frame_w: u32,
    frame_h: u32,
) -> Option<OverlayRegion> {
    let mut b: Option<(f32, f32, f32, f32)> = None;
    for g in gauges.iter().filter(|g| g.visible && g.opacity > 0.0) {
        let (x, y, w, h) = g.placement.bounds();
        let m = gauge_margin(g);
        let r = (
            (x - m) * scale,
            (y - m) * scale,
            (x + w + m) * scale,
            (y + h + m) * scale,
        );
        b = Some(match b {
            None => r,
            Some(a) => (a.0.min(r.0), a.1.min(r.1), a.2.max(r.2), a.3.max(r.3)),
        });
    }
    let (x0, y0, x1, y1) = b?;
    OverlayRegion::from_bounds(x0, y0, x1, y1, frame_w, frame_h)
}

/// Renders all visible gauges into a straight-alpha RGBA buffer covering `region`.
pub fn render_overlay(
    gauges: &[Gauge],
    ctx: &RenderCtx<'_>,
    scale: f32,
    region: OverlayRegion,
) -> Vec<u8> {
    let Some(mut pm) = Pixmap::new(region.w, region.h) else {
        return vec![0; (region.w * region.h * 4) as usize];
    };
    {
        let mut pmm = pm.as_mut();
        for g in gauges.iter().filter(|g| g.visible && g.opacity > 0.0) {
            let scene = build_scene(g, ctx);
            let ts = placement_transform(&g.placement, scale, (region.x as f32, region.y as f32));
            draw_scene(&mut pmm, &scene, ts, g.opacity);
        }
    }
    let mut data = pm.take();
    demultiply_in_place(&mut data);
    data
}

/// Rasterized gauge for the interactive preview.
pub struct PreviewRaster {
    pub image: ColorImage,
    /// Margin in gauge-local units on each side of the gauge box.
    pub margin: f32,
}

/// Rasterizes one gauge (unrotated) at `scale` screen pixels per video pixel.
/// The UI draws it as a rotated textured quad; export rotates the vectors instead.
pub fn rasterize_gauge(g: &Gauge, ctx: &RenderCtx<'_>, scale: f32) -> Option<PreviewRaster> {
    let margin = gauge_margin(g);
    let w = ((g.placement.w + margin * 2.0) * scale).ceil() as u32;
    let h = ((g.placement.h + margin * 2.0) * scale).ceil() as u32;
    if w == 0 || h == 0 || w > 8192 || h > 8192 {
        return None;
    }
    let mut pm = Pixmap::new(w, h)?;
    let scene = build_scene(g, ctx);
    let ts = Transform::from_scale(scale, scale).pre_translate(margin, margin);
    draw_scene(&mut pm.as_mut(), &scene, ts, 1.0);
    let image = ColorImage::from_rgba_premultiplied([w as usize, h as usize], pm.data());
    Some(PreviewRaster { image, margin })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gauges::library::{PresetId, make_preset};
    use crate::gauges::model::GaugeId;
    use crate::telemetry::{SyncSettings, UnitSystem};

    #[test]
    fn region_covers_gauges() {
        let mut a = make_preset(
            PresetId::DigitalSpeed,
            GaugeId(1),
            (1920.0, 1080.0),
            None,
            UnitSystem::Metric,
        );
        a.placement.x = 100.0;
        a.placement.y = 100.0;
        let mut b = a.clone();
        b.placement.x = 1500.0;
        b.placement.y = 900.0;
        let r = overlay_region(&[a.clone(), b.clone()], 1.0, 1920, 1080).unwrap();
        assert!(r.x <= 100 && r.y <= 100);
        assert!(r.x + r.w >= (1500.0 + b.placement.w) as u32 || r.x + r.w == 1920);
        // Hidden gauges are excluded.
        b.visible = false;
        let r2 = overlay_region(&[a, b], 1.0, 1920, 1080).unwrap();
        assert!(r2.w < r.w);
        assert!(overlay_region(&[], 1.0, 1920, 1080).is_none());
    }

    #[test]
    fn overlay_frame_has_content_and_matches_preview() {
        let g = make_preset(
            PresetId::DigitalSpeed,
            GaugeId(1),
            (1920.0, 1080.0),
            None,
            UnitSystem::Metric,
        );
        let sync = SyncSettings::default();
        let ctx = RenderCtx {
            track: None,
            sync: &sync,
            video_t: 0.0,
            units: UnitSystem::Metric,
        };
        let region = overlay_region(std::slice::from_ref(&g), 1.0, 1920, 1080).unwrap();
        let data = render_overlay(std::slice::from_ref(&g), &ctx, 1.0, region);
        assert_eq!(data.len(), (region.w * region.h * 4) as usize);
        let opaque = data.as_chunks::<4>().0.iter().filter(|p| p[3] > 0).count();
        assert!(opaque > 1000);
        let preview = rasterize_gauge(&g, &ctx, 1.0).unwrap();
        let p_opaque = preview.image.pixels.iter().filter(|p| p.a() > 0).count();
        // Same gauge, same scale: coverage within a few percent.
        let ratio = opaque as f64 / p_opaque as f64;
        assert!((0.95..1.05).contains(&ratio), "{ratio}");
    }
}
