//! Renders every built-in template on a 1920×1080 frame into one PNG (visual regression aid).
//!
//! `cargo run --release --example template_sheet -- out.png`

use telemetryvibe::gauges::library::{Template, apply_template};
use telemetryvibe::gauges::model::GaugeId;
use telemetryvibe::gauges::{RenderCtx, build_scene};
use telemetryvibe::render::raster::{draw_scene, placement_transform};
use telemetryvibe::telemetry::model::{RawSample, TrackBuilder};
use telemetryvibe::telemetry::{SyncSettings, UnitSystem};

fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "template_sheet.png".into());
    let start = 1_714_557_600.0;
    let mut b = TrackBuilder::new("demo", "demo");
    for i in 0..600 {
        let t = i as f64;
        let rpm = 5000.0 + (t * 0.2).sin() * 3000.0;
        b.samples.push(RawSample {
            time: start + t,
            latitude: Some(45.0 + (t * 0.01).sin() * 0.01 + t * 0.00001),
            longitude: Some(7.0 + (t * 0.013).cos() * 0.015),
            altitude: Some(300.0 + (t * 0.02).sin() * 60.0),
            speed: Some(8.0 + (t * 0.05).sin() * 4.0),
            heart_rate: Some(140.0 + (t * 0.03).sin() * 25.0),
            cadence: Some(88.0 + (t * 0.1).sin() * 8.0),
            power: Some(230.0 + (t * 0.07).sin() * 80.0),
            extra: vec![
                ("rpm".into(), rpm),
                ("gear".into(), (rpm / 2000.0).floor() + 1.0),
                ("throttle".into(), ((t * 0.2).sin() * 60.0 + 40.0).clamp(0.0, 100.0)),
                ("brake".into(), ((t * 0.2).cos() * -80.0).clamp(0.0, 100.0)),
            ],
            ..Default::default()
        });
    }
    b.laps.push((start, start + 300.0, None));
    b.laps.push((start + 300.0, start + 600.0, None));
    let track = b.build().expect("track");
    let sync = SyncSettings::default();
    let ctx = RenderCtx {
        track: Some(&track),
        sync: &sync,
        video_t: 420.0,
        units: UnitSystem::Metric,
    };

    let (vw, vh) = (1920.0f32, 1080.0f32);
    let scale = 0.5;
    let (cw, ch) = ((vw * scale) as u32, (vh * scale) as u32);
    let cols = 2u32;
    let rows = (Template::ALL.len() as u32).div_ceil(cols);
    let pad = 8u32;
    let mut pm =
        tiny_skia::Pixmap::new((cw + pad) * cols + pad, (ch + pad) * rows + pad).expect("pixmap");
    pm.fill(tiny_skia::Color::from_rgba8(20, 20, 24, 255));
    let mut n = 0;
    for (i, t) in Template::ALL.iter().enumerate() {
        let ox = pad + (i as u32 % cols) * (cw + pad);
        let oy = pad + (i as u32 / cols) * (ch + pad);
        // Stand-in for the video: a sky-to-ground gradient.
        let paint_rect = |pm: &mut tiny_skia::Pixmap, y0: u32, h: u32, c: (u8, u8, u8)| {
            let r = tiny_skia::Rect::from_xywh(ox as f32, y0 as f32, cw as f32, h as f32)
                .expect("rect");
            let mut paint = tiny_skia::Paint::default();
            paint.set_color_rgba8(c.0, c.1, c.2, 255);
            pm.fill_rect(r, &paint, tiny_skia::Transform::identity(), None);
        };
        paint_rect(&mut pm, oy, ch / 2, (95, 135, 175));
        paint_rect(&mut pm, oy + ch / 2, ch - ch / 2, (85, 105, 70));
        let gauges = apply_template(*t, (vw, vh), Some(&track), UnitSystem::Metric, &mut || {
            n += 1;
            GaugeId(n)
        });
        for g in &gauges {
            let scene = build_scene(g, &ctx);
            let ts = placement_transform(&g.placement, scale, (-(ox as f32), -(oy as f32)));
            draw_scene(&mut pm.as_mut(), &scene, ts, g.opacity);
        }
    }
    pm.save_png(&out).expect("save png");
    println!("wrote {out}");
}
