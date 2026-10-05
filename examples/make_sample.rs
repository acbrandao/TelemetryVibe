//! Generates demo media: a synthetic ride (`sample_ride.fit`) and a matching test video
//! (`sample_ride.mp4`) whose creation time lines up 60 s into the ride, so automatic sync works.
//!
//! `cargo run --release --example make_sample -- [output_dir] [video_seconds]`

use std::path::PathBuf;

use chrono::{TimeZone, Utc};
use telemetryvibe::sample::{make_test_video, synthetic_ride_fit};
use telemetryvibe::video::ffmpeg;

fn main() {
    let dir = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "sample".into()));
    let seconds: f64 = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(60.0);
    std::fs::create_dir_all(&dir).expect("create output dir");
    let start = Utc
        .with_ymd_and_hms(2026, 5, 16, 9, 30, 0)
        .single()
        .expect("date");
    let fit = synthetic_ride_fit(start.timestamp(), 900);
    let fit_path = dir.join("sample_ride.fit");
    std::fs::write(&fit_path, fit).expect("write fit");
    println!("wrote {}", fit_path.display());

    let Some((ff, _)) = ffmpeg::locate(None) else {
        eprintln!("FFmpeg not found; skipping the sample video");
        return;
    };
    let video_start = start + chrono::Duration::seconds(60);
    let video_path = dir.join("sample_ride.mp4");
    match make_test_video(
        &ff,
        &video_path,
        seconds,
        1920,
        1080,
        30,
        &video_start.to_rfc3339(),
    ) {
        Ok(true) => println!("wrote {} (starts 60 s into the ride)", video_path.display()),
        _ => eprintln!("failed to create the sample video"),
    }
}
