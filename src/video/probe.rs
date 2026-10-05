//! Video metadata via `ffprobe`.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::ffmpeg::{FfmpegInfo, command};

/// Metadata of an imported video.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VideoInfo {
    pub path: PathBuf,
    /// Display size (after applying rotation metadata).
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub duration: f64,
    pub codec: String,
    pub profile: Option<String>,
    pub pix_fmt: Option<String>,
    pub bit_rate: Option<u64>,
    pub has_audio: bool,
    pub audio_codec: Option<String>,
    pub file_size: u64,
    /// Recording start time from container metadata.
    pub creation_time: Option<DateTime<Utc>>,
    pub rotation: i32,
}

impl VideoInfo {
    pub fn file_name(&self) -> String {
        self.path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default()
    }

    pub fn frame_duration(&self) -> f64 {
        1.0 / self.fps.max(1.0)
    }

    pub fn frame_count(&self) -> u64 {
        (self.duration * self.fps).round().max(1.0) as u64
    }

    /// Time of the frame index (frame starts).
    pub fn frame_time(&self, index: u64) -> f64 {
        index as f64 / self.fps.max(1.0)
    }

    pub fn frame_index(&self, t: f64) -> u64 {
        ((t.max(0.0) * self.fps).floor() as u64).min(self.frame_count().saturating_sub(1))
    }

    pub fn resolution_label(&self) -> String {
        let p = self.width.min(self.height);
        let tag = match p {
            0..=719 => String::new(),
            720..=1079 => " (720p)".into(),
            1080..=1439 => " (1080p)".into(),
            1440..=2159 => " (1440p)".into(),
            2160..=2879 => " (4K)".into(),
            2880..=3383 => " (5K)".into(),
            3384..=4319 => " (6K)".into(),
            _ => " (8K)".into(),
        };
        format!("{}×{}{tag}", self.width, self.height)
    }

    /// Large sources benefit from a proxy for smooth preview.
    pub fn wants_proxy(&self) -> bool {
        self.width.max(self.height) > 2560 || (self.fps > 61.0 && self.height > 1080)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProbeError {
    #[error("Unable to read this video file.")]
    Launch(#[source] std::io::Error),
    #[error("Unable to decode this video using the available codecs.")]
    Failed(String),
    #[error("This file does not contain a video stream.")]
    NoVideo,
}

fn parse_rate(s: &str) -> Option<f64> {
    let mut it = s.split('/');
    let n: f64 = it.next()?.parse().ok()?;
    let d: f64 = it.next().map(|d| d.parse().ok()).unwrap_or(Some(1.0))?;
    (d > 0.0 && n > 0.0).then(|| n / d)
}

fn num(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

/// Parses ffprobe JSON output into [`VideoInfo`].
pub fn parse_probe_json(path: &Path, json: &str, file_size: u64) -> Result<VideoInfo, ProbeError> {
    let v: Value = serde_json::from_str(json).map_err(|e| ProbeError::Failed(e.to_string()))?;
    let streams = v["streams"].as_array().cloned().unwrap_or_default();
    let video = streams
        .iter()
        .find(|s| s["codec_type"] == "video" && s["disposition"]["attached_pic"] != 1)
        .ok_or(ProbeError::NoVideo)?;
    let audio = streams.iter().find(|s| s["codec_type"] == "audio");
    let mut width = video["width"].as_u64().unwrap_or(0) as u32;
    let mut height = video["height"].as_u64().unwrap_or(0) as u32;
    if width == 0 || height == 0 {
        return Err(ProbeError::Failed("invalid dimensions".into()));
    }
    // Rotation from display matrix side data or legacy tag.
    let mut rotation = video["tags"]["rotate"]
        .as_str()
        .and_then(|r| r.parse::<i32>().ok())
        .unwrap_or(0);
    if let Some(sd) = video["side_data_list"].as_array() {
        for d in sd {
            if let Some(r) = num(&d["rotation"]) {
                rotation = r.round() as i32;
            }
        }
    }
    if rotation.rem_euclid(180) == 90 {
        std::mem::swap(&mut width, &mut height);
    }
    let fps = video["avg_frame_rate"]
        .as_str()
        .and_then(parse_rate)
        .or_else(|| video["r_frame_rate"].as_str().and_then(parse_rate))
        .filter(|f| *f > 1.0 && *f < 1000.0)
        .unwrap_or(30.0);
    let duration = num(&v["format"]["duration"])
        .or_else(|| num(&video["duration"]))
        .unwrap_or(0.0);
    let creation_time = v["format"]["tags"]["creation_time"]
        .as_str()
        .or_else(|| video["tags"]["creation_time"].as_str())
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.with_timezone(&Utc));
    Ok(VideoInfo {
        path: path.to_path_buf(),
        width,
        height,
        fps,
        duration,
        codec: video["codec_name"]
            .as_str()
            .unwrap_or("unknown")
            .to_string(),
        profile: video["profile"].as_str().map(str::to_string),
        pix_fmt: video["pix_fmt"].as_str().map(str::to_string),
        bit_rate: num(&v["format"]["bit_rate"]).map(|b| b as u64),
        has_audio: audio.is_some(),
        audio_codec: audio
            .and_then(|a| a["codec_name"].as_str())
            .map(str::to_string),
        file_size,
        creation_time,
        rotation,
    })
}

/// Runs ffprobe on a file.
pub fn probe(ff: &FfmpegInfo, path: &Path) -> Result<VideoInfo, ProbeError> {
    let out = command(&ff.ffprobe)
        .args([
            "-v",
            "error",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
        ])
        .arg(path)
        .stdin(Stdio::null())
        .output()
        .map_err(ProbeError::Launch)?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).to_string();
        tracing::error!(file = %path.display(), "ffprobe failed: {err}");
        return Err(ProbeError::Failed(err));
    }
    let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let info = parse_probe_json(path, &String::from_utf8_lossy(&out.stdout), size)?;
    tracing::info!(?info, "video probed");
    Ok(info)
}

pub fn is_video_file(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some("mp4" | "mov" | "mkv" | "m4v" | "avi" | "mts" | "insv" | "lrv" | "360")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const JSON: &str = r#"{
      "streams": [
        {"codec_type":"video","codec_name":"hevc","profile":"Main 10","width":3840,"height":2160,
         "avg_frame_rate":"60000/1001","r_frame_rate":"60000/1001","pix_fmt":"yuv420p10le",
         "side_data_list":[{"side_data_type":"Display Matrix","rotation":-90}]},
        {"codec_type":"audio","codec_name":"aac"}
      ],
      "format": {"duration":"125.458","bit_rate":"60000000","tags":{"creation_time":"2024-05-01T10:00:12.000000Z"}}
    }"#;

    #[test]
    fn parses_probe() {
        let i = parse_probe_json(Path::new("/x/clip.MP4"), JSON, 123).unwrap();
        assert_eq!((i.width, i.height), (2160, 3840));
        assert!((i.fps - 59.94).abs() < 0.01);
        assert!((i.duration - 125.458).abs() < 1e-9);
        assert_eq!(i.codec, "hevc");
        assert!(i.has_audio);
        assert_eq!(i.audio_codec.as_deref(), Some("aac"));
        assert_eq!(
            i.creation_time.unwrap().to_rfc3339(),
            "2024-05-01T10:00:12+00:00"
        );
        assert!(i.wants_proxy());
        assert_eq!(i.file_name(), "clip.MP4");
        assert_eq!(i.frame_index(1.0), 59);
    }

    #[test]
    fn no_video_stream() {
        let j = r#"{"streams":[{"codec_type":"audio"}],"format":{}}"#;
        assert!(matches!(
            parse_probe_json(Path::new("a"), j, 0),
            Err(ProbeError::NoVideo)
        ));
    }
}
