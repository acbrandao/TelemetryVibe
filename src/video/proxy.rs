//! Proxy generation for large sources (e.g. 4K60 → 1080p30 with short GOP for fast seeking).
//! Preview decodes the proxy; the final render always uses the original.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};

use super::ffmpeg::{Codec, EncoderFamily, FfmpegInfo, HwPreference, command};
use super::probe::VideoInfo;

pub const PROXY_SHORT_SIDE: u32 = 1080;
pub const PROXY_MAX_FPS: f64 = 30.0;

pub fn proxy_path(cache_dir: &Path, source: &Path) -> PathBuf {
    let key = crate::utils::paths::file_cache_key(source);
    cache_dir.join(format!("{key}_proxy.mp4"))
}

/// Proxy dimensions and frame rate for a source.
pub fn proxy_params(info: &VideoInfo) -> (u32, u32, f64) {
    let short = info.width.min(info.height) as f64;
    let k = (PROXY_SHORT_SIDE as f64 / short).min(1.0);
    let even = |v: f64| ((v * k / 2.0).round() as u32 * 2).max(2);
    let fps = if info.fps > PROXY_MAX_FPS + 1.0 {
        // Integer division of the source rate keeps cadence smooth (59.94 → 29.97).
        info.fps / (info.fps / PROXY_MAX_FPS).round()
    } else {
        info.fps
    };
    (even(info.width as f64), even(info.height as f64), fps)
}

pub fn proxy_args(ff: &FfmpegInfo, info: &VideoInfo, out: &Path) -> Vec<String> {
    let (w, h, fps) = proxy_params(info);
    let enc = ff.choose_encoder(HwPreference::Automatic, Codec::H264);
    let mut a: Vec<String> = vec![
        "-hide_banner".into(),
        "-loglevel".into(),
        "error".into(),
        "-nostdin".into(),
        "-y".into(),
    ];
    a.extend(ff.hwaccel_args());
    a.extend(["-i".into(), info.path.to_string_lossy().to_string()]);
    a.extend([
        "-an".into(),
        "-sn".into(),
        "-vf".into(),
        format!("scale={w}:{h},fps={fps:.5},format=yuv420p"),
        "-g".into(),
        "15".into(),
        "-c:v".into(),
        enc.encoder_name(Codec::H264).into(),
    ]);
    match enc {
        EncoderFamily::Software => {
            a.extend(["-preset", "veryfast", "-crf", "23", "-tune", "fastdecode"].map(String::from))
        }
        _ => a.extend(["-b:v", "10M"].map(String::from)),
    }
    a.extend(["-progress".into(), "pipe:1".into(), "-nostats".into()]);
    a.push(out.to_string_lossy().to_string());
    a
}

#[derive(Debug, thiserror::Error)]
pub enum ProxyError {
    #[error("Proxy generation was cancelled.")]
    Cancelled,
    #[error("Proxy generation failed.")]
    Failed,
}

/// Generates a proxy, reporting progress 0..=1. Blocking; run on a worker thread.
pub fn generate(
    ff: &FfmpegInfo,
    info: &VideoInfo,
    out: &Path,
    cancel: &AtomicBool,
    mut progress: impl FnMut(f32),
) -> Result<PathBuf, ProxyError> {
    if let Some(dir) = out.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let tmp = out.with_extension("partial.mp4");
    let args = proxy_args(ff, info, &tmp);
    tracing::info!(?args, "generating proxy");
    let mut child = command(&ff.ffmpeg)
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| {
            tracing::error!("proxy spawn failed: {e}");
            ProxyError::Failed
        })?;
    if let Some(stdout) = child.stdout.take() {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if cancel.load(Ordering::Relaxed) {
                let _ = child.kill();
                let _ = child.wait();
                let _ = std::fs::remove_file(&tmp);
                return Err(ProxyError::Cancelled);
            }
            if let Some(us) = line
                .strip_prefix("out_time_us=")
                .or_else(|| line.strip_prefix("out_time_ms="))
                && let Ok(us) = us.trim().parse::<f64>()
                && info.duration > 0.0
            {
                progress(((us / 1e6) / info.duration).clamp(0.0, 1.0) as f32);
            }
        }
    }
    let ok = child.wait().map(|s| s.success()).unwrap_or(false);
    if !ok {
        let _ = std::fs::remove_file(&tmp);
        return Err(ProxyError::Failed);
    }
    std::fs::rename(&tmp, out).map_err(|_| ProxyError::Failed)?;
    progress(1.0);
    Ok(out.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(w: u32, h: u32, fps: f64) -> VideoInfo {
        VideoInfo {
            path: "/v.mp4".into(),
            width: w,
            height: h,
            fps,
            duration: 10.0,
            codec: "hevc".into(),
            profile: None,
            pix_fmt: None,
            bit_rate: None,
            has_audio: true,
            audio_codec: None,
            file_size: 0,
            creation_time: None,
            rotation: 0,
        }
    }

    #[test]
    fn params() {
        let (w, h, fps) = proxy_params(&info(3840, 2160, 59.94));
        assert_eq!((w, h), (1920, 1080));
        assert!((fps - 29.97).abs() < 0.01);
        let (w, h, fps) = proxy_params(&info(2160, 3840, 30.0));
        assert_eq!((w, h), (1080, 1920));
        assert_eq!(fps, 30.0);
        let (_, _, fps) = proxy_params(&info(3840, 2160, 120.0));
        assert_eq!(fps, 30.0);
    }
}
