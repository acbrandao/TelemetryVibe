//! Renders every library preset into a PNG contact sheet (visual regression aid).
//!
//! `cargo run --release --example gauge_sheet -- out.png`

use telemetryvibe::gauges::library::{PresetId, make_preset};
use telemetryvibe::gauges::model::GaugeId;
use telemetryvibe::gauges::{RenderCtx, build_scene};
use telemetryvibe::render::raster::{draw_scene, placement_transform};
use telemetryvibe::telemetry::model::{RawSample, TrackBuilder};
use telemetryvibe::telemetry::{SyncSettings, UnitSystem};

fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "gauge_sheet.png".into());
    let mut b = TrackBuilder::new("demo", "demo");
    for i in 0..600 {
        let t = i as f64;
        b.samples.push(RawSample {
            time: 1_714_557_600.0 + t,
            latitude: Some(45.0 + (t * 0.01).sin() * 0.01 + t * 0.00001),
            longitude: Some(7.0 + (t * 0.013).cos() * 0.015),
            altitude: Some(300.0 + (t * 0.02).sin() * 60.0),
            speed: Some(8.0 + (t * 0.05).sin() * 4.0),
            heart_rate: Some(140.0 + (t * 0.03).sin() * 25.0),
            cadence: Some(88.0 + (t * 0.1).sin() * 8.0),
            power: Some(230.0 + (t * 0.07).sin() * 80.0),
            temperature: Some(21.5),
            ..Default::default()
        });
    }
    let track = b.build().expect("track");
    let sync = SyncSettings::default();
    let ctx = RenderCtx {
        track: Some(&track),
        sync: &sync,
        video_t: 222.0,
        units: UnitSystem::Metric,
    };
    let cell = 330u32;
    let cols = 7u32;
    let rows = (PresetId::ALL.len() as u32).div_ceil(cols);
    let mut pm = tiny_skia::Pixmap::new(cell * cols, cell * rows).expect("pixmap");
    pm.fill(tiny_skia::Color::from_rgba8(70, 90, 110, 255));
    for (i, p) in PresetId::ALL.iter().enumerate() {
        let g = make_preset(
            *p,
            GaugeId(i as u64),
            (1920.0, 1080.0),
            Some(&track),
            UnitSystem::Metric,
        );
        let scene = build_scene(&g, &ctx);
        let scale = (cell as f32 * 0.9 / g.placement.w)
            .min(cell as f32 * 0.9 / g.placement.h)
            .min(1.0);
        let (cx, cy) = ((i as u32 % cols) * cell, (i as u32 / cols) * cell);
        let mut placement = g.placement;
        placement.x = (cx as f32 + (cell as f32 - g.placement.w * scale) / 2.0) / scale;
        placement.y = (cy as f32 + (cell as f32 - g.placement.h * scale) / 2.0) / scale;
        let ts = placement_transform(&placement, scale, (0.0, 0.0));
        draw_scene(&mut pm.as_mut(), &scene, ts, g.opacity);
    }
    pm.save_png(&out).expect("save png");
    println!("wrote {out}");
}
