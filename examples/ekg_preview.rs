//! Renders digital-style gauges at true relative scale (EKG first, at two instants):
//! `cargo run --example ekg_preview -- out.png`
use telemetryvibe::gauges::library::{PresetId, make_preset};
use telemetryvibe::gauges::model::GaugeId;
use telemetryvibe::gauges::{RenderCtx, build_scene};
use telemetryvibe::telemetry::{SyncSettings, UnitSystem};

fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "digital.png".into());
    let track = telemetryvibe::telemetry::fit::parse(
        &telemetryvibe::sample::synthetic_ride_fit(1_778_923_800, 600),
        "demo",
    )
    .unwrap();
    let rows: Vec<(PresetId, f64)> = vec![
        (PresetId::HrEkg, 100.0),
        (PresetId::HrEkg, 100.3),
        (PresetId::HrPulse, 100.0),
        (PresetId::DigitalHr, 100.0),
        (PresetId::DigitalSpeed, 100.0),
        (PresetId::DigitalPace, 100.0),
        (PresetId::DigitalGrade, 100.0),
        (PresetId::DigitalPower, 100.0),
        (PresetId::ElapsedTime, 100.0),
        (PresetId::Date, 100.0),
    ];
    let k = 0.6;
    let sync = SyncSettings::default();
    let gauges: Vec<_> = rows
        .iter()
        .map(|(p, _)| {
            make_preset(
                *p,
                GaugeId(1),
                (1920.0, 1080.0),
                Some(&track),
                UnitSystem::Metric,
            )
        })
        .collect();
    let w = gauges.iter().map(|g| g.placement.w).fold(0.0, f32::max) * k + 20.0;
    let h: f32 = gauges.iter().map(|g| g.placement.h * k + 10.0).sum::<f32>() + 10.0;
    let mut pm = tiny_skia::Pixmap::new(w as u32, h as u32).unwrap();
    pm.fill(tiny_skia::Color::from_rgba8(70, 90, 110, 255));
    let mut y = 10.0;
    for (g, (_, t)) in gauges.iter().zip(&rows) {
        let ctx = RenderCtx {
            track: Some(&track),
            sync: &sync,
            video_t: *t,
            units: UnitSystem::Metric,
        };
        let s = build_scene(g, &ctx);
        let mut p = g.placement;
        p.x = 10.0 / k;
        p.y = y / k;
        let ts = telemetryvibe::render::raster::placement_transform(&p, k, (0.0, 0.0));
        telemetryvibe::render::raster::draw_scene(&mut pm.as_mut(), &s, ts, 1.0);
        y += g.placement.h * k + 10.0;
    }
    pm.save_png(&out).unwrap();
}
