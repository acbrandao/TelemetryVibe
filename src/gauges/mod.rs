//! Modular gauge system.
//!
//! ```text
//! Telemetry ──► data binding (metric + smoothing + units) ──► builder ──► Scene ──► rasterizer
//! ```
//!
//! Each gauge kind lives in its own module and exposes a `build` function producing a
//! [`scene::Scene`]. [`build_scene`] dispatches on [`model::GaugeKind`]; adding a gauge type
//! does not touch telemetry, preview or export code.

pub mod analog;
pub mod digital;
pub mod ekg;
pub mod graph;
pub mod library;
pub mod map;
pub mod model;
pub mod progress;
pub mod scene;
pub mod tape;

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use parking_lot::Mutex;

use crate::telemetry::units::{format_value, to_display, unit_label};
use crate::telemetry::{Metric, SyncSettings, Track, UnitSystem};
use model::{Gauge, GaugeKind, Rgba};
use scene::Scene;

/// Everything a gauge needs to render one instant.
#[derive(Clone, Copy)]
pub struct RenderCtx<'a> {
    pub track: Option<&'a Track>,
    pub sync: &'a SyncSettings,
    /// Seconds from the start of the video.
    pub video_t: f64,
    pub units: UnitSystem,
}

impl RenderCtx<'_> {
    /// Track time corresponding to the current video time.
    pub fn gps_t(&self) -> f64 {
        self.sync.video_to_gps(self.video_t)
    }
}

/// Builds the vector scene for a gauge at the context's instant.
pub fn build_scene(g: &Gauge, ctx: &RenderCtx<'_>) -> Scene {
    let mut scene = match &g.kind {
        GaugeKind::Analog {
            dial,
            sweep,
            major_ticks,
            minor_ticks,
            show_readout,
        } => analog::build(
            g,
            ctx,
            *dial,
            *sweep,
            *major_ticks,
            *minor_ticks,
            *show_readout,
        ),
        GaugeKind::Digital { align, icon } => digital::build_readout(g, ctx, *align, *icon),
        GaugeKind::Tape {
            orientation,
            span,
            major_step,
            minor_ticks,
        } => tape::build(g, ctx, *orientation, *span, *major_step, *minor_ticks),
        GaugeKind::Bar {
            orientation,
            segments,
            rounded,
            thickness,
            show_value,
        } => progress::build_bar(
            g,
            ctx,
            *orientation,
            *segments,
            *rounded,
            *thickness,
            *show_value,
        ),
        GaugeKind::Zone { show_value } => progress::build_zone(g, ctx, *show_value),
        GaugeKind::Graph {
            window,
            full_activity,
            line_width,
            fill,
            grid,
            auto_scale,
            show_value,
        } => graph::build(
            g,
            ctx,
            graph::GraphOpts {
                window: *window,
                full_activity: *full_activity,
                line_width: *line_width,
                fill: *fill,
                grid: *grid,
                auto_scale: *auto_scale,
                show_value: *show_value,
            },
        ),
        GaugeKind::Map {
            mode,
            trail_seconds,
            route_width,
            marker_size,
            show_arrow,
            heading_up,
            zoom_radius,
            speed_colors,
        } => map::build(
            g,
            ctx,
            map::MapOpts {
                mode: *mode,
                trail_seconds: *trail_seconds,
                route_width: *route_width,
                marker_size: *marker_size,
                show_arrow: *show_arrow,
                heading_up: *heading_up,
                zoom_radius: *zoom_radius,
                speed_colors: *speed_colors,
            },
        ),
        GaugeKind::Time {
            format,
            tz_offset_minutes,
        } => digital::build_time(g, ctx, *format, *tz_offset_minutes),
        GaugeKind::Text { text, align } => digital::build_text(g, ctx, text, *align),
        GaugeKind::Image { path } => digital::build_image(g, path),
        GaugeKind::Ekg {
            beats,
            window,
            show_value,
            grid,
        } => ekg::build(g, ctx, *beats, *window, *show_value, *grid),
    };
    if g.style.shadow && scene.shadow.is_none() {
        scene.shadow = Some((g.placement.w.min(g.placement.h) * 0.012).max(1.5));
    }
    scene
}

/// Canonical value bound to the gauge at the current instant.
pub fn gauge_value(g: &Gauge, ctx: &RenderCtx<'_>) -> Option<f64> {
    let track = ctx.track?;
    let t = ctx.gps_t();
    if g.metric == Metric::Custom {
        track.extra_value(&g.custom_key, t, g.smoothing)
    } else {
        track
            .value_smoothed(g.metric, t, g.smoothing)
            .map(|v| relative_to_sync(g, ctx, v))
    }
}

/// Track distance at the sync start point (GPS time of the first video frame), clamped to the
/// recording.
pub fn sync_start_distance(track: &Track, sync: &SyncSettings) -> f64 {
    let t = sync.video_to_gps(0.0).clamp(0.0, track.duration());
    track.value(Metric::Distance, t).unwrap_or(0.0)
}

/// Rebases a canonical value on the sync start point when the gauge asks for it.
pub fn relative_to_sync(g: &Gauge, ctx: &RenderCtx<'_>, v: f64) -> f64 {
    match ctx.track {
        Some(track) if g.metric == Metric::Distance && g.distance_from_sync => {
            (v - sync_start_distance(track, ctx.sync)).max(0.0)
        }
        _ => v,
    }
}

/// Unit system in effect for this gauge.
pub fn gauge_units(g: &Gauge, ctx: &RenderCtx<'_>) -> UnitSystem {
    g.units.resolve(ctx.units)
}

pub fn display_value(g: &Gauge, ctx: &RenderCtx<'_>, si: f64) -> f64 {
    if g.metric == Metric::Custom {
        si
    } else {
        to_display(g.metric, si, gauge_units(g, ctx))
    }
}

pub fn gauge_unit_label(g: &Gauge, ctx: &RenderCtx<'_>) -> &'static str {
    if g.metric == Metric::Custom {
        ""
    } else {
        unit_label(g.metric, gauge_units(g, ctx))
    }
}

/// Formats a canonical value for this gauge (units, decimals, prefix/suffix).
pub fn format_gauge_value(g: &Gauge, ctx: &RenderCtx<'_>, si: Option<f64>) -> String {
    let body = match si {
        Some(v) => format_value(g.metric, display_value(g, ctx, v), g.style.decimals),
        None => "--".to_string(),
    };
    format!("{}{}{}", g.style.prefix, body, g.style.suffix)
}

/// Widest string the gauge can show, used to keep font sizes stable while values change.
pub fn widest_value_template(g: &Gauge, ctx: &RenderCtx<'_>) -> String {
    let a = format_gauge_value(g, ctx, Some(g.min));
    let b = format_gauge_value(g, ctx, Some(g.max));
    let mut s = if a.len() > b.len() { a } else { b };
    // Account for an extra digit beyond the configured range; custom fields (gear, RPM) come
    // with explicit ranges.
    if s.len() < 3 && g.metric != Metric::Custom {
        s = format!("{}{}", g.style.prefix, "0".repeat(3)) + &g.style.suffix;
    }
    s
}

/// Background panel shared by most gauges.
pub fn panel(scene: &mut Scene, g: &Gauge, w: f32, h: f32, zone_color: Option<Rgba>) {
    let st = &g.style;
    let bg = match zone_color {
        Some(c) if g.zone_targets.background => c,
        _ => st.background,
    };
    let radius = st.corner_radius.clamp(0.0, 0.5) * w.min(h);
    if st.background_opacity > 0.001 {
        scene.fill(
            scene::rounded_rect(0.0, 0.0, w, h, radius),
            bg.with_alpha_mul(st.background_opacity),
        );
    }
    if st.border_width > 0.0 {
        let bw = st.border_width;
        scene.stroke(
            scene::rounded_rect(
                bw / 2.0,
                bw / 2.0,
                w - bw,
                h - bw,
                (radius - bw / 2.0).max(0.0),
            ),
            st.border,
            bw,
            scene::Cap::Butt,
        );
    }
}

/// Color for the value considering zones.
pub fn zone_color(g: &Gauge, si: Option<f64>) -> Option<Rgba> {
    let v = si?;
    g.zone_for(v).map(|(_, z)| z.color)
}

static IMAGES: OnceLock<Mutex<HashMap<String, Option<Arc<tiny_skia::Pixmap>>>>> = OnceLock::new();

/// Loads (and caches) a PNG image overlay.
pub fn load_image(path: &str) -> Option<Arc<tiny_skia::Pixmap>> {
    let cache = IMAGES.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(v) = cache.lock().get(path) {
        return v.clone();
    }
    let img =
        std::fs::read(path)
            .ok()
            .and_then(|bytes| match tiny_skia::Pixmap::decode_png(&bytes) {
                Ok(p) => Some(Arc::new(p)),
                Err(e) => {
                    tracing::warn!("cannot decode image {path}: {e}");
                    None
                }
            });
    cache.lock().insert(path.to_string(), img.clone());
    img
}

#[cfg(test)]
mod tests {
    use super::library::{PresetId, make_preset};
    use super::*;
    use crate::telemetry::model::{RawSample, TrackBuilder};

    pub(crate) fn test_track() -> Track {
        let mut b = TrackBuilder::new("t", "t");
        for i in 0..120 {
            b.samples.push(RawSample {
                time: 1_714_557_600.0 + i as f64,
                latitude: Some(45.0 + (i as f64 * 0.05).sin() * 0.002),
                longitude: Some(7.0 + (i as f64 * 0.05).cos() * 0.003),
                altitude: Some(200.0 + i as f64 * 0.5),
                speed: Some(5.0 + (i % 20) as f64 * 0.5),
                heart_rate: Some(120.0 + i as f64 * 0.3),
                cadence: Some(85.0),
                power: Some(180.0 + (i % 30) as f64 * 5.0),
                temperature: Some(22.0),
                ..Default::default()
            });
        }
        b.build().unwrap()
    }

    #[test]
    fn every_preset_builds_a_scene() {
        let track = test_track();
        let sync = SyncSettings::default();
        let ctx = RenderCtx {
            track: Some(&track),
            sync: &sync,
            video_t: 30.0,
            units: UnitSystem::Metric,
        };
        let ctx_none = RenderCtx {
            track: None,
            sync: &sync,
            video_t: 30.0,
            units: UnitSystem::Imperial,
        };
        for preset in PresetId::ALL {
            let g = make_preset(
                *preset,
                model::GaugeId(1),
                (1920.0, 1080.0),
                Some(&track),
                UnitSystem::Metric,
            );
            let s = build_scene(&g, &ctx);
            if !matches!(g.kind, GaugeKind::Image { .. }) {
                assert!(!s.prims.is_empty(), "{preset:?} produced nothing");
            }
            // Without telemetry nothing panics.
            let _ = build_scene(&g, &ctx_none);
            // Rasterizes.
            let mut pm = tiny_skia::Pixmap::new(400, 400).unwrap();
            let ts = crate::render::raster::placement_transform(&g.placement, 0.3, (0.0, 0.0));
            crate::render::raster::draw_scene(&mut pm.as_mut(), &s, ts, g.opacity);
        }
    }

    #[test]
    fn value_binding_respects_units_and_sync() {
        let track = test_track();
        let sync = SyncSettings {
            offset: 10.0,
            ..Default::default()
        };
        let g = make_preset(
            PresetId::DigitalSpeed,
            model::GaugeId(1),
            (1920.0, 1080.0),
            Some(&track),
            UnitSystem::Metric,
        );
        let ctx = RenderCtx {
            track: Some(&track),
            sync: &sync,
            video_t: 0.0,
            units: UnitSystem::Metric,
        };
        let v = gauge_value(&g, &ctx).unwrap();
        // Track time 10 s → speed 5 + 10*0.5 = 10 m/s.
        assert!((v - 10.0).abs() < 1e-6, "{v}");
        assert_eq!(format_gauge_value(&g, &ctx, Some(v)), "36.0");
        let imperial = RenderCtx {
            units: UnitSystem::Imperial,
            ..ctx
        };
        assert_eq!(format_gauge_value(&g, &imperial, Some(v)), "22.4");
        assert_eq!(gauge_unit_label(&g, &imperial), "mph");
    }

    #[test]
    fn heart_beats_follow_heart_rate() {
        // Constant 120 bpm → 2 beats per second.
        let mut b = TrackBuilder::new("t", "t");
        for i in 0..60 {
            b.samples.push(RawSample {
                time: 1_714_557_600.0 + i as f64,
                heart_rate: Some(120.0),
                ..Default::default()
            });
        }
        let track = b.build().unwrap();
        let beats = |t: f64| track.extra_value(crate::telemetry::model::HEART_BEATS, t, 0.0);
        assert!((beats(10.0).unwrap() - 20.0).abs() < 1e-9);
        assert!((track.beat_phase(10.25).unwrap() - 0.5).abs() < 1e-9);
        assert!(track.beat_phase(10.5).unwrap() < 1e-9);
    }

    #[test]
    fn peak_hold_and_new_dials() {
        let track = test_track();
        let sync = SyncSettings::default();
        let ctx = RenderCtx {
            track: Some(&track),
            sync: &sync,
            video_t: 31.0,
            units: UnitSystem::Metric,
        };
        let mut g = make_preset(
            PresetId::PowerPeakDial,
            model::GaugeId(1),
            (1920.0, 1080.0),
            Some(&track),
            UnitSystem::Metric,
        );
        g.smoothing = 0.0;
        // Power ramps 180 + (i % 30) * 5 W and restarts at 30 s: at 31 s the 10 s window still
        // holds the 325 W top of the previous ramp (29 s) while the current value is 185 W.
        let peak = analog::peak_value(&g, &ctx).unwrap();
        let now = gauge_value(&g, &ctx).unwrap();
        assert!((now - 185.0).abs() < 1e-6, "{now}");
        assert!((peak - 325.0).abs() < 1e-6, "{peak}");
        assert!(!g.zones.is_empty(), "power dials are zone coded");
        for preset in [
            PresetId::HrZoneDial,
            PresetId::HrLedRing,
            PresetId::PowerLedRing,
        ] {
            let g = make_preset(
                preset,
                model::GaugeId(1),
                (1920.0, 1080.0),
                Some(&track),
                UnitSystem::Metric,
            );
            assert!(!g.zones.is_empty(), "{preset:?}");
            assert!(build_scene(&g, &ctx).prims.len() > 10, "{preset:?}");
        }
    }

    #[test]
    fn horizon_and_wind_dials() {
        let track = test_track();
        let sync = SyncSettings::default();
        let ctx = RenderCtx {
            track: Some(&track),
            sync: &sync,
            video_t: 30.0,
            units: UnitSystem::Metric,
        };
        let horizon = make_preset(
            PresetId::AttitudeIndicator,
            model::GaugeId(1),
            (1920.0, 1080.0),
            Some(&track),
            UnitSystem::Metric,
        );
        // Without pitch/roll fields the attitude comes from GPS: the test track climbs, so the
        // nose is up, and it circles, so it banks.
        let (pitch, roll) = analog::attitude(&horizon, &ctx).unwrap();
        assert!(pitch > 0.0, "{pitch}");
        assert!(roll.abs() > 0.1 && roll.abs() <= 60.0, "{roll}");
        assert!(build_scene(&horizon, &ctx).prims.len() > 20);

        let wind = make_preset(
            PresetId::WindDial,
            model::GaugeId(2),
            (1920.0, 1080.0),
            Some(&track),
            UnitSystem::Metric,
        );
        assert_eq!(wind.metric, Metric::Custom);
        assert_eq!(wind.custom_key, "wind_direction");
        // No wind fields: the rose still draws, with "--" for the speed.
        let texts: Vec<String> = build_scene(&wind, &ctx)
            .prims
            .iter()
            .filter_map(|p| match p {
                scene::Prim::Text(t) => Some(t.text.clone()),
                _ => None,
            })
            .collect();
        assert!(texts.iter().any(|t| t == "--"), "{texts:?}");
    }

    #[test]
    fn gradient_readout_is_zone_colored() {
        let g = make_preset(
            PresetId::DigitalGrade,
            model::GaugeId(1),
            (1920.0, 1080.0),
            None,
            UnitSystem::Metric,
        );
        assert_eq!(g.metric, Metric::Grade);
        assert_eq!(zone_color(&g, Some(0.0)), Some(Rgba::rgb(110, 210, 120)));
        assert_eq!(zone_color(&g, Some(12.0)), Some(Rgba::rgb(240, 60, 50)));
        assert_eq!(zone_color(&g, Some(-8.0)), Some(Rgba::rgb(80, 150, 255)));
    }

    #[test]
    fn close_up_map_colors_by_speed() {
        let track = test_track();
        let sync = SyncSettings::default();
        let g = make_preset(
            PresetId::CloseUpMap,
            model::GaugeId(1),
            (1920.0, 1080.0),
            Some(&track),
            UnitSystem::Metric,
        );
        assert!(matches!(
            g.kind,
            GaugeKind::Map {
                mode: model::RouteMode::CloseUp,
                speed_colors: true,
                ..
            }
        ));
        // Slow (5 m/s) and fast (14.5 m/s) moments use different marker colors.
        let colors_at = |t: f64| {
            let ctx = RenderCtx {
                track: Some(&track),
                sync: &sync,
                video_t: t,
                units: UnitSystem::Metric,
            };
            let s = build_scene(&g, &ctx);
            s.prims
                .iter()
                .filter_map(|p| match p {
                    scene::Prim::Fill {
                        paint: scene::Paint::Solid(c),
                        ..
                    } => Some(*c),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        let slow = g.zone_for(5.0).unwrap().1.color;
        let fast = g.zone_for(14.5).unwrap().1.color;
        assert_ne!(slow, fast);
        assert!(colors_at(40.0).contains(&slow));
        assert!(colors_at(59.0).contains(&fast));
    }

    #[test]
    fn digital_numbers_share_one_size() {
        let track = test_track();
        let sync = SyncSettings::default();
        let ctx = RenderCtx {
            track: Some(&track),
            sync: &sync,
            video_t: 30.0,
            units: UnitSystem::Metric,
        };
        let mut sizes = Vec::new();
        for preset in PresetId::ALL {
            let g = make_preset(
                *preset,
                model::GaugeId(1),
                (1920.0, 1080.0),
                Some(&track),
                UnitSystem::Metric,
            );
            if !matches!(
                g.kind,
                GaugeKind::Digital { .. } | GaugeKind::Ekg { .. } | GaugeKind::Time { .. }
            ) {
                continue;
            }
            let biggest = build_scene(&g, &ctx)
                .prims
                .iter()
                .filter_map(|p| match p {
                    scene::Prim::Text(t) => Some(t.size),
                    _ => None,
                })
                .fold(0.0f32, f32::max);
            sizes.push((*preset, biggest));
        }
        assert!(sizes.len() >= 15, "{sizes:?}");
        let reference = sizes[0].1;
        for (preset, size) in &sizes {
            assert!(
                (size - reference).abs() / reference < 0.01,
                "{preset:?}: {size} vs {reference}"
            );
        }
    }

    #[test]
    fn analog_needle_angle_tracks_value() {
        let g = make_preset(
            PresetId::AnalogSpeed,
            model::GaugeId(1),
            (1920.0, 1080.0),
            None,
            UnitSystem::Metric,
        );
        let (sweep, start) = analog::angles(&g);
        assert_eq!(sweep, 270.0);
        assert_eq!(start, 135.0);
        assert_eq!(analog::needle_angle(&g, Some(g.min)), 135.0);
        assert_eq!(analog::needle_angle(&g, Some(g.max)), 405.0);
        let mid = (g.min + g.max) / 2.0;
        assert_eq!(analog::needle_angle(&g, Some(mid)), 270.0);
        // Values outside the range are clamped.
        assert_eq!(analog::needle_angle(&g, Some(g.max * 3.0)), 405.0);
    }
}
