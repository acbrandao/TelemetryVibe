//! Final-render settings and FFmpeg command generation.
//!
//! The export pipeline decodes the original video once and composites a rasterized RGBA
//! overlay (piped through stdin) with the `overlay` filter, then encodes once:
//!
//! ```text
//! original ─► scale/fps ─┐
//!                        ├─► overlay ─► encoder (HW or x264/x265) ─► MP4 (+ original audio)
//! gauges (RGBA pipe) ────┘
//! ```
//! Only the bounding region covered by gauges is piped, which keeps the pipe small at 4K.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::ffmpeg::{Codec, EncoderFamily, HwPreference};
use super::probe::VideoInfo;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResolutionPreset {
    #[default]
    Original,
    R2160,
    R1440,
    R1080,
    R720,
}

impl ResolutionPreset {
    pub const ALL: [ResolutionPreset; 5] = [
        ResolutionPreset::Original,
        ResolutionPreset::R2160,
        ResolutionPreset::R1440,
        ResolutionPreset::R1080,
        ResolutionPreset::R720,
    ];
    pub fn label(self) -> &'static str {
        match self {
            ResolutionPreset::Original => "Original",
            ResolutionPreset::R2160 => "4K (2160p)",
            ResolutionPreset::R1440 => "1440p",
            ResolutionPreset::R1080 => "1080p",
            ResolutionPreset::R720 => "720p",
        }
    }
    fn short_side(self) -> Option<u32> {
        match self {
            ResolutionPreset::Original => None,
            ResolutionPreset::R2160 => Some(2160),
            ResolutionPreset::R1440 => Some(1440),
            ResolutionPreset::R1080 => Some(1080),
            ResolutionPreset::R720 => Some(720),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FpsPreset {
    #[default]
    Original,
    F60,
    F30,
    F24,
}

impl FpsPreset {
    pub const ALL: [FpsPreset; 4] = [
        FpsPreset::Original,
        FpsPreset::F60,
        FpsPreset::F30,
        FpsPreset::F24,
    ];
    pub fn label(self) -> &'static str {
        match self {
            FpsPreset::Original => "Original",
            FpsPreset::F60 => "60",
            FpsPreset::F30 => "30",
            FpsPreset::F24 => "24",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Quality {
    Draft,
    #[default]
    High,
    Maximum,
}

impl Quality {
    pub const ALL: [Quality; 3] = [Quality::Draft, Quality::High, Quality::Maximum];
    pub fn label(self) -> &'static str {
        match self {
            Quality::Draft => "Draft",
            Quality::High => "High",
            Quality::Maximum => "Maximum",
        }
    }
    /// Bits per pixel per frame used for bitrate-based encoders and size estimates.
    fn bpp(self) -> f64 {
        match self {
            Quality::Draft => 0.05,
            Quality::High => 0.11,
            Quality::Maximum => 0.22,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RenderSettings {
    pub codec: Codec,
    pub resolution: ResolutionPreset,
    pub fps: FpsPreset,
    pub quality: Quality,
    pub hw: HwPreference,
    pub include_audio: bool,
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            codec: Codec::H264,
            resolution: ResolutionPreset::Original,
            fps: FpsPreset::Original,
            quality: Quality::High,
            hw: HwPreference::Automatic,
            include_audio: true,
        }
    }
}

/// A section of the source video (seconds of video time) — the timeline trim range.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TimeRange {
    pub start: f64,
    pub end: f64,
}

impl TimeRange {
    /// Shortest section that can be rendered.
    pub const MIN_LEN: f64 = 0.1;

    pub fn duration(&self) -> f64 {
        (self.end - self.start).max(0.0)
    }

    /// Clamps the range to `0..=video_duration`. Returns `None` when it is empty, invalid,
    /// or covers (practically) the whole video, so "no trim" has a single representation.
    pub fn normalized(self, video_duration: f64) -> Option<TimeRange> {
        if !self.start.is_finite() || !self.end.is_finite() {
            return None;
        }
        let start = self.start.clamp(0.0, video_duration);
        let end = self.end.clamp(0.0, video_duration);
        if end - start < Self::MIN_LEN - 1e-9 {
            return None;
        }
        let whole = start <= 1e-3 && end >= video_duration - 1e-3;
        (!whole).then_some(TimeRange { start, end })
    }
}

/// Output frame geometry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OutputGeometry {
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    /// Output pixels per source pixel.
    pub scale: f32,
}

impl OutputGeometry {
    pub fn frame_count(&self, duration: f64) -> u64 {
        (duration * self.fps).ceil().max(1.0) as u64
    }
}

fn even(v: f64) -> u32 {
    ((v / 2.0).round() as u32 * 2).max(2)
}

pub fn output_geometry(info: &VideoInfo, s: &RenderSettings) -> OutputGeometry {
    let short = info.width.min(info.height);
    let target_short = s
        .resolution
        .short_side()
        .filter(|&t| t < short)
        .unwrap_or(short);
    let scale = target_short as f64 / short as f64;
    let (width, height) = if scale >= 0.9999 {
        (even(info.width as f64), even(info.height as f64))
    } else {
        (
            even(info.width as f64 * scale),
            even(info.height as f64 * scale),
        )
    };
    let fps = match s.fps {
        FpsPreset::Original => info.fps,
        FpsPreset::F60 => 60.0,
        FpsPreset::F30 => 30.0,
        FpsPreset::F24 => 24.0,
    };
    OutputGeometry {
        width,
        height,
        fps,
        scale: width as f32 / info.width as f32,
    }
}

/// Region of the output frame covered by the overlay (even-aligned).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OverlayRegion {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl OverlayRegion {
    /// Computes an even-aligned region from float bounds, clamped to the frame.
    pub fn from_bounds(
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        frame_w: u32,
        frame_h: u32,
    ) -> Option<Self> {
        let fx0 = (x0.floor().max(0.0) as u32) & !1;
        let fy0 = (y0.floor().max(0.0) as u32) & !1;
        let fx1 = ((x1.ceil().max(0.0) as u32 + 1) & !1).min(frame_w);
        let fy1 = ((y1.ceil().max(0.0) as u32 + 1) & !1).min(frame_h);
        (fx1 > fx0 + 1 && fy1 > fy0 + 1)
            .then(|| Self {
                x: fx0,
                y: fy0,
                w: (fx1 - fx0) & !1,
                h: (fy1 - fy0) & !1,
            })
            .filter(|r| r.w >= 2 && r.h >= 2)
    }
}

/// Rational frame rate string (`30000/1001` for NTSC rates).
pub fn fps_string(fps: f64) -> String {
    for (n, d) in [
        (24000, 1001),
        (30000, 1001),
        (60000, 1001),
        (120000, 1001),
        (48000, 1001),
    ] {
        if (fps - n as f64 / d as f64).abs() < 0.005 {
            return format!("{n}/{d}");
        }
    }
    if (fps - fps.round()).abs() < 0.001 {
        format!("{}", fps.round() as u64)
    } else {
        format!("{fps:.4}")
    }
}

/// Target bitrate (bits/s) for bitrate-driven encoders and for size estimates.
pub fn target_bitrate(geo: &OutputGeometry, s: &RenderSettings) -> u64 {
    let codec_factor = if s.codec == Codec::H265 { 0.6 } else { 1.0 };
    // Bitrate grows sub-linearly with frame rate.
    let fps_term = geo.fps.min(30.0) + (geo.fps - 30.0).max(0.0) * 0.5;
    (geo.width as f64 * geo.height as f64 * fps_term * s.quality.bpp() * codec_factor) as u64
}

pub fn estimated_size(geo: &OutputGeometry, s: &RenderSettings, duration: f64, audio: bool) -> u64 {
    let audio_bits = if audio && s.include_audio {
        192_000.0
    } else {
        0.0
    };
    ((target_bitrate(geo, s) as f64 + audio_bits) * duration / 8.0) as u64
}

fn audio_copyable(codec: Option<&str>) -> bool {
    matches!(
        codec,
        Some("aac" | "mp3" | "alac" | "ac3" | "eac3" | "opus" | "flac")
    )
}

/// Video encoder arguments for an encoder family.
pub fn encoder_args(enc: EncoderFamily, s: &RenderSettings, geo: &OutputGeometry) -> Vec<String> {
    let name = enc.encoder_name(s.codec).to_string();
    let br = target_bitrate(geo, s);
    let kbps = |b: u64| format!("{}k", b / 1000);
    let mut a = vec!["-c:v".to_string(), name];
    match enc {
        EncoderFamily::Software => {
            let (preset, crf) = match (s.codec, s.quality) {
                (Codec::H264, Quality::Draft) => ("veryfast", 26),
                (Codec::H264, Quality::High) => ("medium", 20),
                (Codec::H264, Quality::Maximum) => ("slow", 16),
                (Codec::H265, Quality::Draft) => ("veryfast", 28),
                (Codec::H265, Quality::High) => ("medium", 23),
                (Codec::H265, Quality::Maximum) => ("slow", 19),
            };
            a.extend([
                "-preset".into(),
                preset.into(),
                "-crf".into(),
                crf.to_string(),
            ]);
        }
        EncoderFamily::VideoToolbox => {
            a.extend([
                "-b:v".into(),
                kbps(br),
                "-maxrate".into(),
                kbps(br * 3 / 2),
                "-bufsize".into(),
                kbps(br * 2),
            ]);
            a.extend(["-allow_sw".into(), "1".into()]);
        }
        EncoderFamily::Nvenc => {
            let cq = match s.quality {
                Quality::Draft => 30,
                Quality::High => 23,
                Quality::Maximum => 18,
            };
            a.extend(["-preset", "p5", "-rc", "vbr", "-b:v", "0"].map(String::from));
            a.extend([
                "-cq".into(),
                cq.to_string(),
                "-maxrate".into(),
                kbps(br * 2),
            ]);
        }
        EncoderFamily::Qsv => {
            let q = match s.quality {
                Quality::Draft => 30,
                Quality::High => 23,
                Quality::Maximum => 18,
            };
            a.extend([
                "-global_quality".into(),
                q.to_string(),
                "-preset".into(),
                "medium".into(),
            ]);
        }
        EncoderFamily::Amf => {
            let q = match s.quality {
                Quality::Draft => 30,
                Quality::High => 22,
                Quality::Maximum => 18,
            };
            a.extend([
                "-rc".into(),
                "cqp".into(),
                "-qp_i".into(),
                q.to_string(),
                "-qp_p".into(),
                (q + 2).to_string(),
            ]);
            a.extend([
                "-quality".into(),
                if s.quality == Quality::Draft {
                    "speed"
                } else {
                    "quality"
                }
                .into(),
            ]);
        }
        EncoderFamily::MediaFoundation => {
            a.extend(["-b:v".into(), kbps(br), "-hw_encoding".into(), "1".into()]);
        }
    }
    if s.codec == Codec::H265 {
        // Required for HEVC playback in QuickTime / Apple devices.
        a.extend(["-tag:v".into(), "hvc1".into()]);
    }
    a
}

/// Builds the complete FFmpeg argument list for the final render.
///
/// With a `range`, only that section is rendered: input seeking (`-ss`/`-t` before `-i`) is
/// frame-accurate when transcoding and also trims the audio. Output timestamps start at zero, so
/// overlay frame `i` corresponds to video time `range.start + i / fps`.
#[allow(clippy::too_many_arguments)]
pub fn export_args(
    hwaccel: &[String],
    info: &VideoInfo,
    s: &RenderSettings,
    enc: EncoderFamily,
    geo: &OutputGeometry,
    overlay: Option<OverlayRegion>,
    range: Option<TimeRange>,
    output: &Path,
) -> Vec<String> {
    let fps = fps_string(geo.fps);
    let pix = if enc == EncoderFamily::Qsv {
        "nv12"
    } else {
        "yuv420p"
    };
    let mut a: Vec<String> = vec![
        "-hide_banner".into(),
        "-loglevel".into(),
        "error".into(),
        "-y".into(),
    ];
    if enc != EncoderFamily::Software || s.hw != HwPreference::Cpu {
        a.extend(hwaccel.iter().cloned());
    }
    if let Some(r) = range {
        a.extend([
            "-ss".into(),
            format!("{:.6}", r.start),
            "-t".into(),
            format!("{:.6}", r.duration()),
        ]);
    }
    a.extend(["-i".into(), info.path.to_string_lossy().to_string()]);
    let base = format!(
        "[0:v]setpts=PTS-STARTPTS,scale={}:{}:flags=lanczos,fps={fps}",
        geo.width, geo.height
    );
    match overlay {
        Some(r) => {
            a.extend([
                "-f".into(),
                "rawvideo".into(),
                "-pix_fmt".into(),
                "rgba".into(),
                "-s".into(),
                format!("{}x{}", r.w, r.h),
                "-framerate".into(),
                fps.clone(),
                "-i".into(),
                "pipe:0".into(),
            ]);
            a.extend([
                "-filter_complex".into(),
                format!(
                    "{base}[base];[base][1:v]overlay=x={}:y={}:eof_action=pass:format=auto,format={pix}[v]",
                    r.x, r.y
                ),
            ]);
        }
        None => {
            a.extend(["-filter_complex".into(), format!("{base},format={pix}[v]")]);
        }
    }
    a.extend(["-map".into(), "[v]".into()]);
    if s.include_audio && info.has_audio {
        a.extend(["-map".into(), "0:a:0?".into()]);
        if audio_copyable(info.audio_codec.as_deref()) {
            a.extend(["-c:a".into(), "copy".into()]);
        } else {
            a.extend(["-c:a".into(), "aac".into(), "-b:a".into(), "192k".into()]);
        }
    } else {
        a.push("-an".into());
    }
    a.extend(encoder_args(enc, s, geo));
    a.extend(["-movflags".into(), "+faststart".into()]);
    a.push(output.to_string_lossy().to_string());
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info() -> VideoInfo {
        VideoInfo {
            path: "/videos/ride.mov".into(),
            width: 3840,
            height: 2160,
            fps: 59.94,
            duration: 60.0,
            codec: "hevc".into(),
            profile: None,
            pix_fmt: None,
            bit_rate: None,
            has_audio: true,
            audio_codec: Some("aac".into()),
            file_size: 0,
            creation_time: None,
            rotation: 0,
        }
    }

    #[test]
    fn geometry_presets() {
        let i = info();
        let g = output_geometry(&i, &RenderSettings::default());
        assert_eq!((g.width, g.height), (3840, 2160));
        assert!((g.fps - 59.94).abs() < 1e-9);
        let g = output_geometry(
            &i,
            &RenderSettings {
                resolution: ResolutionPreset::R1080,
                fps: FpsPreset::F30,
                ..Default::default()
            },
        );
        assert_eq!((g.width, g.height, g.fps), (1920, 1080, 30.0));
        assert!((g.scale - 0.5).abs() < 1e-6);
        // No upscaling beyond the source.
        let mut small = i.clone();
        small.width = 1280;
        small.height = 720;
        let g = output_geometry(
            &small,
            &RenderSettings {
                resolution: ResolutionPreset::R2160,
                ..Default::default()
            },
        );
        assert_eq!((g.width, g.height), (1280, 720));
    }

    #[test]
    fn fps_strings() {
        assert_eq!(fps_string(59.94), "60000/1001");
        assert_eq!(fps_string(29.97003), "30000/1001");
        assert_eq!(fps_string(30.0), "30");
        assert_eq!(fps_string(25.0), "25");
    }

    #[test]
    fn overlay_region_even() {
        let r = OverlayRegion::from_bounds(11.3, 21.7, 301.2, 199.9, 1920, 1080).unwrap();
        assert_eq!(r.x % 2, 0);
        assert_eq!(r.y % 2, 0);
        assert_eq!(r.w % 2, 0);
        assert_eq!(r.h % 2, 0);
        assert!(r.x as f32 <= 11.3 && (r.x + r.w) as f32 >= 301.2);
        assert!(OverlayRegion::from_bounds(-50.0, -50.0, -10.0, -10.0, 100, 100).is_none());
        let clamped = OverlayRegion::from_bounds(1900.0, 0.0, 2100.0, 10.0, 1920, 1080).unwrap();
        assert!(clamped.x + clamped.w <= 1920);
    }

    #[test]
    fn export_command_with_overlay() {
        let i = info();
        let s = RenderSettings::default();
        let g = output_geometry(&i, &s);
        let r = OverlayRegion {
            x: 100,
            y: 200,
            w: 640,
            h: 360,
        };
        let args = export_args(
            &["-hwaccel".into(), "auto".into()],
            &i,
            &s,
            EncoderFamily::VideoToolbox,
            &g,
            Some(r),
            None,
            Path::new("/out/ride_gauges.mp4"),
        );
        let joined = args.join(" ");
        assert!(joined.contains("-i /videos/ride.mov"));
        assert!(
            joined.contains("-f rawvideo -pix_fmt rgba -s 640x360 -framerate 60000/1001 -i pipe:0")
        );
        assert!(joined.contains("overlay=x=100:y=200"));
        assert!(joined.contains("-map 0:a:0? -c:a copy"));
        assert!(joined.contains("-c:v h264_videotoolbox"));
        assert!(joined.contains("-movflags +faststart"));
        assert!(joined.ends_with("/out/ride_gauges.mp4"));
        assert!(joined.contains("-hwaccel auto"));
        assert!(!joined.contains("-ss "), "no trim → whole video");
    }

    #[test]
    fn export_command_trimmed() {
        let i = info();
        let s = RenderSettings::default();
        let g = output_geometry(&i, &s);
        let range = TimeRange {
            start: 12.5,
            end: 42.5,
        };
        let joined = export_args(
            &[],
            &i,
            &s,
            EncoderFamily::Software,
            &g,
            None,
            Some(range),
            Path::new("o.mp4"),
        )
        .join(" ");
        // Input seeking: the options must precede the source input.
        assert!(
            joined.contains("-ss 12.500000 -t 30.000000 -i /videos/ride.mov"),
            "{joined}"
        );
    }

    #[test]
    fn time_range_normalization() {
        let r = |start, end| TimeRange { start, end };
        assert_eq!(r(-5.0, 20.0).normalized(60.0), Some(r(0.0, 20.0)));
        assert_eq!(r(10.0, 99.0).normalized(60.0), Some(r(10.0, 60.0)));
        assert_eq!(r(0.0, 60.0).normalized(60.0), None, "whole video = no trim");
        assert_eq!(r(20.0, 20.05).normalized(60.0), None, "too short");
        assert_eq!(r(30.0, 10.0).normalized(60.0), None, "inverted");
        assert_eq!(r(f64::NAN, 10.0).normalized(60.0), None);
        assert!((r(10.0, 25.0).duration() - 15.0).abs() < 1e-12);
    }

    #[test]
    fn export_command_software_hevc_no_audio() {
        let mut i = info();
        i.audio_codec = Some("pcm_s16le".into());
        let s = RenderSettings {
            codec: Codec::H265,
            quality: Quality::Maximum,
            hw: HwPreference::Cpu,
            ..Default::default()
        };
        let g = output_geometry(&i, &s);
        let args = export_args(
            &["-hwaccel".into(), "auto".into()],
            &i,
            &s,
            EncoderFamily::Software,
            &g,
            None,
            None,
            Path::new("o.mp4"),
        )
        .join(" ");
        assert!(args.contains("-c:v libx265 -preset slow -crf 19 -tag:v hvc1"));
        assert!(
            args.contains("-c:a aac -b:a 192k"),
            "pcm must be transcoded for MP4"
        );
        assert!(!args.contains("pipe:0"));
        assert!(
            !args.contains("-hwaccel"),
            "CPU preference avoids hardware decode"
        );
        let s2 = RenderSettings {
            include_audio: false,
            ..s
        };
        let args2 = export_args(
            &[],
            &i,
            &s2,
            EncoderFamily::Software,
            &g,
            None,
            None,
            Path::new("o.mp4"),
        )
        .join(" ");
        assert!(args2.contains("-an"));
    }

    #[test]
    fn size_estimate_scales() {
        let i = info();
        let s = RenderSettings::default();
        let g = output_geometry(&i, &s);
        let draft = RenderSettings {
            quality: Quality::Draft,
            ..s.clone()
        };
        assert!(estimated_size(&g, &s, 60.0, true) > estimated_size(&g, &draft, 60.0, true));
    }
}
