//! End-to-end: synthetic FIT + generated video → auto sync → gauges → FFmpeg render → verify.
//! Skipped automatically when FFmpeg is not installed.

use std::path::PathBuf;
use std::sync::Arc;

use chrono::{TimeZone, Utc};
use telemetryvibe::gauges::library::{Template, apply_template};
use telemetryvibe::gauges::model::GaugeId;
use telemetryvibe::render::export::{ExportSpec, JobStatus, RenderJob};
use telemetryvibe::sample::{make_test_video, synthetic_ride_fit};
use telemetryvibe::telemetry::{self, Metric, SyncSettings, UnitSystem, sync};
use telemetryvibe::video::decoder::{DecoderConfig, VideoEngine};
use telemetryvibe::video::encoder::{RenderSettings, ResolutionPreset, TimeRange};
use telemetryvibe::video::{ffmpeg, probe};

fn workdir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("telemetryvibe-e2e-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn full_workflow_renders_mp4_with_audio_and_overlay() {
    let Ok(ff) = ffmpeg::detect(None) else {
        eprintln!("FFmpeg not available, skipping");
        return;
    };
    let ff = Arc::new(ff);
    let dir = workdir("render");
    let start = Utc.with_ymd_and_hms(2026, 5, 16, 9, 30, 0).unwrap();

    // Telemetry.
    let fit_path = dir.join("ride.fit");
    std::fs::write(&fit_path, synthetic_ride_fit(start.timestamp(), 300)).unwrap();
    let track = Arc::new(telemetry::load_file(&fit_path).unwrap());

    // Video recorded 30 s into the ride.
    let video_path = dir.join("clip.mp4");
    let vstart = start + chrono::Duration::seconds(30);
    assert!(
        make_test_video(
            &ff.ffmpeg,
            &video_path,
            3.0,
            640,
            360,
            30,
            &vstart.to_rfc3339()
        )
        .unwrap()
    );
    let info = probe::probe(&ff, &video_path).unwrap();
    assert_eq!((info.width, info.height), (640, 360));
    assert!(info.has_audio);

    // Automatic synchronization from metadata.
    let offset = sync::offset_from_creation_time(info.creation_time.unwrap(), &track);
    assert!((offset - 30.0).abs() < 0.01, "offset {offset}");
    let sync = SyncSettings {
        offset,
        ..Default::default()
    };
    assert!(track.value(Metric::Speed, sync.video_to_gps(1.0)).is_some());

    // Gauges from a template.
    let mut n = 0;
    let gauges = apply_template(
        Template::Cycling,
        (info.width as f32, info.height as f32),
        Some(&track),
        UnitSystem::Metric,
        &mut || {
            n += 1;
            GaugeId(n)
        },
    );

    // Render.
    let output = dir.join("out.mp4");
    let job = RenderJob::new(ExportSpec {
        output: output.clone(),
        info: info.clone(),
        settings: RenderSettings {
            resolution: ResolutionPreset::Original,
            ..Default::default()
        },
        gauges: gauges.clone(),
        track: Some(track.clone()),
        sync: sync.clone(),
        units: UnitSystem::Metric,
        ffmpeg: ff.clone(),
        range: None,
    });
    job.run(&|| {});
    assert_eq!(job.status(), JobStatus::Done, "{:?}", job.status());
    assert!(output.exists());

    // Verify output: same size, duration, audio preserved.
    let out = probe::probe(&ff, &output).unwrap();
    assert_eq!((out.width, out.height), (640, 360));
    assert!(
        (out.duration - 3.0).abs() < 0.2,
        "duration {}",
        out.duration
    );
    assert!(out.has_audio, "audio must be preserved");

    // The overlay changed pixels in the gauge area (bottom-left dial) compared to the source.
    let grab_at = |path: &PathBuf, t: &str| -> Vec<u8> {
        let o = std::process::Command::new(&ff.ffmpeg)
            .args(["-v", "error", "-ss", t, "-i"])
            .arg(path)
            .args(["-frames:v", "1", "-f", "rawvideo", "-pix_fmt", "rgb24", "-"])
            .output()
            .unwrap();
        o.stdout
    };
    let grab = |path: &PathBuf| grab_at(path, "1.5");
    let a = grab(&video_path);
    let b = grab(&output);
    assert_eq!(a.len(), b.len());
    let px = |buf: &[u8], x: usize, y: usize| {
        let i = (y * 640 + x) * 3;
        buf[i..i + 3].to_vec()
    };
    let diff = (0..60)
        .flat_map(|x| (260..340).map(move |y| (x, y)))
        .filter(|&(x, y)| {
            let (p, q) = (px(&a, x, y), px(&b, x, y));
            p.iter()
                .zip(q.iter())
                .map(|(m, n)| (*m as i32 - *n as i32).abs())
                .sum::<i32>()
                > 60
        })
        .count();
    assert!(diff > 500, "overlay not visible ({diff} changed pixels)");

    // Trimmed render: only 1.0 s – 2.5 s, with gauges showing the values of that section.
    let trimmed = dir.join("trimmed.mp4");
    let job = RenderJob::new(ExportSpec {
        output: trimmed.clone(),
        info: info.clone(),
        settings: RenderSettings::default(),
        gauges,
        track: Some(track.clone()),
        sync,
        units: UnitSystem::Metric,
        ffmpeg: ff.clone(),
        range: Some(TimeRange {
            start: 1.0,
            end: 2.5,
        }),
    });
    assert!((job.spec.duration() - 1.5).abs() < 1e-9);
    job.run(&|| {});
    assert_eq!(job.status(), JobStatus::Done, "{:?}", job.status());
    let out = probe::probe(&ff, &trimmed).unwrap();
    assert!(
        (out.duration - 1.5).abs() < 0.15,
        "trimmed duration {}",
        out.duration
    );
    assert!(out.has_audio, "audio is trimmed and kept");
    // 0.5 s into the trimmed file is video time 1.5 s: it must match the full render there
    // (same source frame, same gauge values), up to encoder noise.
    let c = grab_at(&trimmed, "0.5");
    assert_eq!(b.len(), c.len());
    let mean_diff = b
        .iter()
        .zip(c.iter())
        .map(|(m, n)| (*m as i32 - *n as i32).unsigned_abs() as u64)
        .sum::<u64>() as f64
        / b.len() as f64;
    assert!(
        mean_diff < 4.0,
        "trimmed frame differs (mean {mean_diff:.2})"
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn decoder_engine_seeks_and_plays() {
    let Some((ffmpeg_path, _)) = ffmpeg::locate(None) else {
        return;
    };
    let dir = workdir("decode");
    let video = dir.join("v.mp4");
    assert!(
        make_test_video(
            &ffmpeg_path,
            &video,
            2.0,
            320,
            180,
            30,
            "2026-01-01T00:00:00Z"
        )
        .unwrap()
    );
    let engine = VideoEngine::new(|| {});
    engine.configure(DecoderConfig {
        ffmpeg: ffmpeg_path.clone(),
        source: video.clone(),
        fps: 30.0,
        duration: 2.0,
        out_w: 160,
        out_h: 90,
        hwaccel_args: vec![],
        hwaccel_for_seeks: false,
    });
    let generation = engine.seek(1.0);
    let frame = loop {
        let f = engine
            .frames
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("frame");
        if f.generation == generation {
            break f;
        }
    };
    assert!((frame.t - 1.0).abs() < 1e-9);
    assert_eq!(frame.image.size, [160, 90]);

    // Cached neighbour: instant.
    let generation = engine.seek(1.0 + 1.0 / 30.0);
    let f = engine
        .frames
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("frame");
    assert_eq!(f.generation, generation);

    let generation = engine.play(0.5);
    let mut got = 0;
    let mut last_t = -1.0;
    while got < 10 {
        let f = engine
            .frames
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("play frame");
        if f.generation != generation {
            continue;
        }
        assert!(f.t > last_t);
        last_t = f.t;
        got += 1;
    }
    engine.stop();
    drop(engine);
    std::fs::remove_dir_all(&dir).ok();
}
