//! FFmpeg discovery and capability detection.
//!
//! The application drives FFmpeg as a subprocess (no linking), which keeps the build portable
//! and lets users swap in any compatible build. Lookup order: user-configured path, a copy
//! bundled next to the executable (or in the macOS `.app` Resources), `PATH`, then common
//! install locations (Homebrew etc. — GUI apps on macOS do not inherit the shell `PATH`).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

/// Hardware acceleration preference for encoding.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum HwPreference {
    #[default]
    Automatic,
    Cpu,
    Hardware,
}

impl HwPreference {
    pub const ALL: [HwPreference; 3] = [
        HwPreference::Automatic,
        HwPreference::Cpu,
        HwPreference::Hardware,
    ];
    pub fn label(self) -> &'static str {
        match self {
            HwPreference::Automatic => "Automatic",
            HwPreference::Cpu => "CPU (software)",
            HwPreference::Hardware => "Hardware",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Codec {
    #[default]
    H264,
    H265,
}

impl Codec {
    pub fn label(self) -> &'static str {
        match self {
            Codec::H264 => "H.264",
            Codec::H265 => "H.265 / HEVC",
        }
    }
}

/// Encoder families the application knows how to configure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EncoderFamily {
    Software,
    VideoToolbox,
    Nvenc,
    Qsv,
    Amf,
    MediaFoundation,
}

impl EncoderFamily {
    pub fn label(self) -> &'static str {
        match self {
            EncoderFamily::Software => "Software (x264/x265)",
            EncoderFamily::VideoToolbox => "Apple VideoToolbox",
            EncoderFamily::Nvenc => "NVIDIA NVENC",
            EncoderFamily::Qsv => "Intel Quick Sync",
            EncoderFamily::Amf => "AMD AMF",
            EncoderFamily::MediaFoundation => "Media Foundation",
        }
    }

    pub fn is_hardware(self) -> bool {
        self != EncoderFamily::Software
    }

    /// FFmpeg encoder name for a codec.
    pub fn encoder_name(self, codec: Codec) -> &'static str {
        match (self, codec) {
            (EncoderFamily::Software, Codec::H264) => "libx264",
            (EncoderFamily::Software, Codec::H265) => "libx265",
            (EncoderFamily::VideoToolbox, Codec::H264) => "h264_videotoolbox",
            (EncoderFamily::VideoToolbox, Codec::H265) => "hevc_videotoolbox",
            (EncoderFamily::Nvenc, Codec::H264) => "h264_nvenc",
            (EncoderFamily::Nvenc, Codec::H265) => "hevc_nvenc",
            (EncoderFamily::Qsv, Codec::H264) => "h264_qsv",
            (EncoderFamily::Qsv, Codec::H265) => "hevc_qsv",
            (EncoderFamily::Amf, Codec::H264) => "h264_amf",
            (EncoderFamily::Amf, Codec::H265) => "hevc_amf",
            (EncoderFamily::MediaFoundation, Codec::H264) => "h264_mf",
            (EncoderFamily::MediaFoundation, Codec::H265) => "hevc_mf",
        }
    }

    /// Preference order when several hardware encoders work.
    const HW_ORDER: [EncoderFamily; 5] = [
        EncoderFamily::VideoToolbox,
        EncoderFamily::Nvenc,
        EncoderFamily::Qsv,
        EncoderFamily::Amf,
        EncoderFamily::MediaFoundation,
    ];
}

/// Detected FFmpeg installation and capabilities.
#[derive(Clone, Debug)]
pub struct FfmpegInfo {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
    pub version: String,
    /// All H.264/HEVC encoders listed by the build.
    pub encoders: Vec<String>,
    pub hwaccels: Vec<String>,
    /// (family, codec) pairs verified by a test encode.
    pub working: Vec<(EncoderFamily, Codec)>,
}

impl FfmpegInfo {
    /// Resolves the encoder to use for a preference and codec.
    pub fn choose_encoder(&self, pref: HwPreference, codec: Codec) -> EncoderFamily {
        let hw = EncoderFamily::HW_ORDER
            .into_iter()
            .find(|f| self.working.contains(&(*f, codec)));
        let sw_ok = self
            .encoders
            .iter()
            .any(|e| e == EncoderFamily::Software.encoder_name(codec));
        match pref {
            HwPreference::Cpu => EncoderFamily::Software,
            HwPreference::Hardware => hw.unwrap_or(EncoderFamily::Software),
            HwPreference::Automatic => match hw {
                Some(h) => h,
                None if sw_ok => EncoderFamily::Software,
                None => EncoderFamily::Software,
            },
        }
    }

    pub fn has_hw_encoder(&self) -> bool {
        self.working.iter().any(|(f, _)| f.is_hardware())
    }

    /// Hardware decode flag for FFmpeg input (`auto` falls back to software on failure).
    pub fn hwaccel_args(&self) -> Vec<String> {
        if self.hwaccels.is_empty() {
            Vec::new()
        } else {
            vec!["-hwaccel".into(), "auto".into()]
        }
    }

    pub fn summary(&self) -> String {
        let hw: Vec<&str> = {
            let mut v: Vec<&str> = self
                .working
                .iter()
                .filter(|(f, _)| f.is_hardware())
                .map(|(f, _)| f.label())
                .collect();
            v.dedup();
            v
        };
        if hw.is_empty() {
            format!("FFmpeg {} · software encoding", self.version)
        } else {
            format!("FFmpeg {} · {}", self.version, hw.join(", "))
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FfmpegError {
    #[error("FFmpeg was not found. Install FFmpeg or set its location in Settings.")]
    NotFound,
    #[error("FFmpeg could not be started: {0}")]
    Launch(String),
}

/// Creates a `Command` that never pops up a console window on Windows.
pub fn command(program: &Path) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

fn runs(path: &Path) -> bool {
    command(path)
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Finds `ffmpeg` and `ffprobe`.
pub fn locate(custom: Option<&Path>) -> Option<(PathBuf, PathBuf)> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(c) = custom {
        if c.is_dir() {
            candidates.push(c.to_path_buf());
        } else if let Some(parent) = c.parent() {
            candidates.push(parent.to_path_buf());
        }
    }
    if let Ok(exe_path) = std::env::current_exe()
        && let Some(dir) = exe_path.parent()
    {
        candidates.push(dir.to_path_buf());
        candidates.push(dir.join("ffmpeg"));
        candidates.push(dir.join("../Resources"));
        candidates.push(dir.join("../Resources/ffmpeg"));
    }
    if let Some(path) = std::env::var_os("PATH") {
        candidates.extend(std::env::split_paths(&path));
    }
    for extra in [
        "/opt/homebrew/bin",
        "/usr/local/bin",
        "/opt/local/bin",
        "/usr/bin",
        "C:\\ffmpeg\\bin",
        "C:\\Program Files\\ffmpeg\\bin",
    ] {
        candidates.push(PathBuf::from(extra));
    }
    for dir in candidates {
        let ff = dir.join(exe("ffmpeg"));
        let fp = dir.join(exe("ffprobe"));
        if ff.is_file() && fp.is_file() && runs(&ff) {
            return Some((ff, fp));
        }
    }
    None
}

fn run_capture(ffmpeg: &Path, args: &[&str]) -> Result<String, FfmpegError> {
    let out = command(ffmpeg)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| FfmpegError::Launch(e.to_string()))?;
    Ok(String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr))
}

/// Parses `ffmpeg -encoders` output, keeping H.264/HEVC video encoders.
pub fn parse_encoders(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|l| {
            let l = l.trim_start();
            let mut parts = l.split_whitespace();
            let flags = parts.next()?;
            let name = parts.next()?;
            (flags.starts_with('V')
                && (name.contains("264") || name.contains("hevc") || name.contains("265")))
            .then(|| name.to_string())
        })
        .collect()
}

/// Parses `ffmpeg -hwaccels` output.
pub fn parse_hwaccels(text: &str) -> Vec<String> {
    text.lines()
        .skip_while(|l| !l.starts_with("Hardware acceleration methods"))
        .skip(1)
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Parses the version string from `ffmpeg -version`.
pub fn parse_version(text: &str) -> String {
    text.lines()
        .next()
        .and_then(|l| l.strip_prefix("ffmpeg version "))
        .and_then(|l| l.split_whitespace().next())
        .unwrap_or("unknown")
        .to_string()
}

fn test_encoder(ffmpeg: &Path, encoder: &str) -> bool {
    let pix = if encoder.ends_with("_qsv") {
        "nv12"
    } else {
        "yuv420p"
    };
    command(ffmpeg)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=c=black:s=256x256:r=30:d=0.2",
            "-frames:v",
            "3",
            "-pix_fmt",
            pix,
            "-c:v",
            encoder,
            "-f",
            "null",
            "-",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Locates FFmpeg and probes its capabilities (runs short test encodes; call off the UI thread).
pub fn detect(custom: Option<&Path>) -> Result<FfmpegInfo, FfmpegError> {
    let (ffmpeg, ffprobe) = locate(custom).ok_or(FfmpegError::NotFound)?;
    let version = parse_version(&run_capture(&ffmpeg, &["-hide_banner", "-version"])?);
    let version = if version == "unknown" {
        parse_version(&run_capture(&ffmpeg, &["-version"])?)
    } else {
        version
    };
    let encoders = parse_encoders(&run_capture(&ffmpeg, &["-hide_banner", "-encoders"])?);
    let hwaccels = parse_hwaccels(&run_capture(&ffmpeg, &["-hide_banner", "-hwaccels"])?);
    let mut working = Vec::new();
    for family in EncoderFamily::HW_ORDER {
        for codec in [Codec::H264, Codec::H265] {
            let name = family.encoder_name(codec);
            if encoders.iter().any(|e| e == name) && test_encoder(&ffmpeg, name) {
                working.push((family, codec));
            }
        }
    }
    for codec in [Codec::H264, Codec::H265] {
        if encoders
            .iter()
            .any(|e| e == EncoderFamily::Software.encoder_name(codec))
        {
            working.push((EncoderFamily::Software, codec));
        }
    }
    let info = FfmpegInfo {
        ffmpeg,
        ffprobe,
        version,
        encoders,
        hwaccels,
        working,
    };
    tracing::info!(
        ffmpeg = %info.ffmpeg.display(),
        version = %info.version,
        hwaccels = ?info.hwaccels,
        working = ?info.working,
        "FFmpeg detected"
    );
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_encoder_list() {
        let text = " V....D libx264              libx264 H.264 / AVC\n V....D h264_videotoolbox    VideoToolbox H.264 Encoder\n A....D aac                  AAC\n V....D libvpx               VP8\n V....D hevc_nvenc  NVIDIA NVENC hevc encoder\n";
        assert_eq!(
            parse_encoders(text),
            vec!["libx264", "h264_videotoolbox", "hevc_nvenc"]
        );
    }

    #[test]
    fn parses_hwaccels() {
        let text = "Hardware acceleration methods:\nvideotoolbox\n\n";
        assert_eq!(parse_hwaccels(text), vec!["videotoolbox"]);
    }

    #[test]
    fn parses_version() {
        assert_eq!(parse_version("ffmpeg version 8.1.2 Copyright (c)"), "8.1.2");
    }

    #[test]
    fn chooses_encoder() {
        let info = FfmpegInfo {
            ffmpeg: "ffmpeg".into(),
            ffprobe: "ffprobe".into(),
            version: "8".into(),
            encoders: vec!["libx264".into(), "h264_videotoolbox".into()],
            hwaccels: vec![],
            working: vec![
                (EncoderFamily::VideoToolbox, Codec::H264),
                (EncoderFamily::Software, Codec::H264),
            ],
        };
        assert_eq!(
            info.choose_encoder(HwPreference::Automatic, Codec::H264),
            EncoderFamily::VideoToolbox
        );
        assert_eq!(
            info.choose_encoder(HwPreference::Cpu, Codec::H264),
            EncoderFamily::Software
        );
        assert_eq!(
            info.choose_encoder(HwPreference::Hardware, Codec::H265),
            EncoderFamily::Software
        );
    }
}
