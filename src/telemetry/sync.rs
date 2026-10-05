//! Video ↔ GPS time synchronization.
//!
//! The relationship is a single offset: `gps_t = video_t + offset`, where `gps_t` is seconds
//! from the start of the telemetry recording and `video_t` seconds from the start of the video.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::model::Track;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SyncSettings {
    /// Seconds added to video time to obtain track time.
    pub offset: f64,
    /// Video time of a marked event (visual sync).
    #[serde(default)]
    pub video_mark: Option<f64>,
    /// Track time of the corresponding event.
    #[serde(default)]
    pub gps_mark: Option<f64>,
}

impl SyncSettings {
    pub fn video_to_gps(&self, video_t: f64) -> f64 {
        video_t + self.offset
    }

    pub fn gps_to_video(&self, gps_t: f64) -> f64 {
        gps_t - self.offset
    }

    /// Wall-clock GPS time at a given video time.
    pub fn wall_time(&self, track: &Track, video_t: f64) -> DateTime<Utc> {
        track.wall_time(self.video_to_gps(video_t))
    }

    /// Offset that aligns both marks, if both are set.
    pub fn offset_from_marks(&self) -> Option<f64> {
        Some(event_offset(self.video_mark?, self.gps_mark?))
    }
}

/// Offset from the video's recorded creation time (start of recording).
pub fn offset_from_creation_time(video_start: DateTime<Utc>, track: &Track) -> f64 {
    let video_unix =
        video_start.timestamp() as f64 + video_start.timestamp_subsec_nanos() as f64 * 1e-9;
    video_unix - track.start_unix
}

/// Offset making the video time `video_t` correspond to track time `gps_t`.
pub fn event_offset(video_t: f64, gps_t: f64) -> f64 {
    gps_t - video_t
}

/// Whether the given offset puts at least part of the video inside the recording.
pub fn overlaps(offset: f64, video_duration: f64, track: &Track) -> bool {
    let start = offset;
    let end = offset + video_duration;
    end > 0.0 && start < track.duration()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::telemetry::model::{RawSample, TrackBuilder};
    use chrono::TimeZone;

    fn track() -> Track {
        let mut b = TrackBuilder::new("t", "t");
        for i in 0..100 {
            b.samples.push(RawSample {
                time: 1_714_557_600.0 + i as f64,
                speed: Some(i as f64),
                ..Default::default()
            });
        }
        b.build().unwrap()
    }

    #[test]
    fn mapping_roundtrip() {
        let s = SyncSettings {
            offset: 3.25,
            ..Default::default()
        };
        assert_eq!(s.video_to_gps(10.0), 13.25);
        assert_eq!(s.gps_to_video(13.25), 10.0);
    }

    #[test]
    fn creation_time_offset() {
        let t = track();
        // Video started 12.5 s after the recording.
        let start = Utc.timestamp_opt(1_714_557_612, 500_000_000).unwrap();
        let off = offset_from_creation_time(start, &t);
        assert!((off - 12.5).abs() < 1e-9);
        let s = SyncSettings {
            offset: off,
            ..Default::default()
        };
        // Video t=0 shows the telemetry recorded at 12.5 s.
        assert!(
            (t.value(crate::telemetry::Metric::Speed, s.video_to_gps(0.0))
                .unwrap()
                - 12.5)
                .abs()
                < 1e-9
        );
        assert_eq!(s.wall_time(&t, 0.0), start);
    }

    #[test]
    fn marks() {
        let s = SyncSettings {
            offset: 0.0,
            video_mark: Some(4.0),
            gps_mark: Some(64.0),
        };
        assert_eq!(s.offset_from_marks(), Some(60.0));
        assert!(overlaps(60.0, 30.0, &track()));
        assert!(!overlaps(200.0, 30.0, &track()));
        assert!(!overlaps(-50.0, 30.0, &track()));
    }
}
