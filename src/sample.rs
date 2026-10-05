//! Synthetic sample data: a FIT ride and a matching test video (for demos and tests).

use std::path::Path;
use std::process::Stdio;

use crate::telemetry::fit::writer::{FitWriter, W};
use crate::telemetry::fit::{FIT_EPOCH_OFFSET, base, mesg};
use crate::video::ffmpeg::command;

fn semi(deg: f64) -> i32 {
    (deg * 2f64.powi(31) / 180.0) as i32
}

/// Generates a FIT activity: a loop ride with hills, sprints, a stop, HR/cadence/power/temp.
/// `start_unix` is the wall-clock start, `seconds` the duration, at 1 Hz.
pub fn synthetic_ride_fit(start_unix: i64, seconds: u32) -> Vec<u8> {
    let mut w = FitWriter::new();
    let fit_start = (start_unix as f64 - FIT_EPOCH_OFFSET) as u32;
    w.define(
        0,
        mesg::FILE_ID,
        &[
            (0, 1, base::ENUM),
            (1, 2, base::UINT16),
            (4, 4, base::UINT32),
        ],
    );
    w.data(0, &[W::U8(4), W::U16(1), W::U32(fit_start)]);
    w.define(
        1,
        mesg::RECORD,
        &[
            (253, 4, base::UINT32),
            (0, 4, base::SINT32),
            (1, 4, base::SINT32),
            (78, 4, base::UINT32),
            (73, 4, base::UINT32),
            (5, 4, base::UINT32),
            (3, 1, base::UINT8),
            (4, 1, base::UINT8),
            (7, 2, base::UINT16),
            (13, 1, base::SINT8),
        ],
    );
    let (lat0, lon0) = (45.0703, 7.6869);
    let mut dist = 0.0f64;
    let mut heading = 0.0f64;
    let (mut lat, mut lon) = (lat0, lon0);
    for i in 0..seconds {
        let t = i as f64;
        // Speed profile: ramp up, cruise with variation, a stop around 40 %, sprint near the end.
        let phase = t / seconds as f64;
        let mut speed = 9.0 + 3.0 * (t * 0.05).sin() + 1.5 * (t * 0.17).sin();
        if t < 8.0 {
            speed = t * 1.1;
        }
        if (0.40..0.43).contains(&phase) {
            speed = 0.0;
        }
        if phase > 0.85 {
            speed += 5.0;
        }
        let speed = speed.max(0.0);
        heading += 0.6 + 0.4 * (t * 0.03).sin();
        let hr = heading.to_radians();
        lat += speed * hr.cos() / 111_320.0;
        lon += speed * hr.sin() / (111_320.0 * lat0.to_radians().cos());
        dist += speed;
        let alt = 240.0 + 35.0 * (t * 0.012).sin() + 10.0 * (t * 0.05).sin();
        let heart = (125.0 + speed * 3.0 + 12.0 * (t * 0.02).sin()).clamp(60.0, 195.0);
        let cadence = if speed > 0.5 {
            82.0 + 8.0 * (t * 0.11).sin()
        } else {
            0.0
        };
        let power = if speed > 0.5 {
            170.0 + speed * 9.0 + 60.0 * (t * 0.09).sin()
        } else {
            0.0
        };
        w.data(
            1,
            &[
                W::U32(fit_start + i),
                W::I32(semi(lat)),
                W::I32(semi(lon)),
                W::U32(((alt + 500.0) * 5.0) as u32),
                W::U32((speed * 1000.0) as u32),
                W::U32((dist * 100.0) as u32),
                W::U8(heart as u8),
                W::U8(cadence as u8),
                W::U16(power.max(0.0) as u16),
                W::I8(19 + (t * 0.002) as i8),
            ],
        );
    }
    w.define(
        2,
        mesg::SESSION,
        &[(253, 4, base::UINT32), (5, 1, base::ENUM)],
    );
    w.data(2, &[W::U32(fit_start + seconds), W::U8(2)]);
    w.finish()
}

/// Creates a test video with `testsrc2` + a sine audio track and creation-time metadata.
pub fn make_test_video(
    ffmpeg: &Path,
    out: &Path,
    seconds: f64,
    width: u32,
    height: u32,
    fps: u32,
    creation_time_rfc3339: &str,
) -> std::io::Result<bool> {
    let status = command(ffmpeg)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
        ])
        .arg(format!(
            "testsrc2=size={width}x{height}:rate={fps}:duration={seconds}"
        ))
        .args(["-f", "lavfi", "-i"])
        .arg(format!(
            "sine=frequency=440:sample_rate=48000:duration={seconds}"
        ))
        .args([
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-pix_fmt",
            "yuv420p",
            "-g",
            &(fps * 2).to_string(),
        ])
        .args(["-c:a", "aac", "-b:a", "128k", "-shortest"])
        .args([
            "-metadata",
            &format!("creation_time={creation_time_rfc3339}"),
        ])
        .arg(out)
        .stdin(Stdio::null())
        .status()?;
    Ok(status.success())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::telemetry::Metric;

    #[test]
    fn synthetic_fit_parses() {
        let bytes = synthetic_ride_fit(1_714_557_600, 300);
        let t = crate::telemetry::fit::parse(&bytes, "s.fit").unwrap();
        assert_eq!(t.len(), 300);
        assert!(t.has(Metric::Power) && t.has(Metric::HeartRate) && t.route.is_some());
        assert!(
            t.events
                .iter()
                .any(|e| e.kind == crate::telemetry::model::EventKind::Stop)
        );
    }
}
