//! Timeline thumbnail strip generation (keyframe-only decode, cached on disk).

use std::io::{Read, Write};
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use egui::ColorImage;

use super::ffmpeg::command;
use super::probe::VideoInfo;

pub const THUMB_H: u32 = 72;

/// A thumbnail and the video time it represents.
#[derive(Clone)]
pub struct Thumbnail {
    pub t: f64,
    pub image: Arc<ColorImage>,
}

pub fn thumb_size(info: &VideoInfo) -> (u32, u32) {
    let w = ((THUMB_H as f64 * info.width as f64 / info.height.max(1) as f64 / 2.0).round() as u32
        * 2)
    .max(16);
    (w, THUMB_H)
}

/// Generates up to ~`count` thumbnails evenly over the video. Calls `on_thumb` for each one.
/// Results are cached in `cache_dir`. Returns early when `cancel` is set.
pub fn generate(
    ffmpeg: &Path,
    info: &VideoInfo,
    count: usize,
    cache_dir: &Path,
    cancel: &AtomicBool,
    mut on_thumb: impl FnMut(Thumbnail),
) {
    let (w, h) = thumb_size(info);
    let count = count.max(2);
    let interval = (info.duration / count as f64).max(0.5);
    let key = crate::utils::paths::file_cache_key(&info.path);
    let cache_file = cache_dir.join(format!("{key}_{count}_{w}x{h}.thumbs"));
    let frame_bytes = (w * h * 4) as usize;

    if let Ok(data) = std::fs::read(&cache_file)
        && data.len() >= frame_bytes
        && data.len() % frame_bytes == 0
    {
        for (i, chunk) in data.chunks_exact(frame_bytes).enumerate() {
            on_thumb(Thumbnail {
                t: i as f64 * interval,
                image: Arc::new(ColorImage::from_rgba_unmultiplied(
                    [w as usize, h as usize],
                    chunk,
                )),
            });
        }
        return;
    }

    let child = command(ffmpeg)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-skip_frame",
            "nokey",
        ])
        .arg("-i")
        .arg(&info.path)
        .args([
            "-an",
            "-sn",
            "-vf",
            &format!("fps=1/{interval:.4},scale={w}:{h}:flags=fast_bilinear"),
            "-fps_mode",
            "vfr",
            "-pix_fmt",
            "rgba",
            "-f",
            "rawvideo",
            "pipe:1",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();
    let Ok(mut child) = child else {
        tracing::warn!("thumbnail generation failed to start");
        return;
    };
    let mut all = Vec::new();
    if let Some(mut out) = child.stdout.take() {
        let mut buf = vec![0u8; frame_bytes];
        let mut i = 0;
        while out.read_exact(&mut buf).is_ok() {
            if cancel.load(Ordering::Relaxed) {
                let _ = child.kill();
                let _ = child.wait();
                return;
            }
            on_thumb(Thumbnail {
                t: i as f64 * interval,
                image: Arc::new(ColorImage::from_rgba_unmultiplied(
                    [w as usize, h as usize],
                    &buf,
                )),
            });
            all.extend_from_slice(&buf);
            i += 1;
        }
    }
    let ok = child.wait().map(|s| s.success()).unwrap_or(false);
    if ok && !all.is_empty() {
        let _ = std::fs::create_dir_all(cache_dir);
        if let Ok(mut f) = std::fs::File::create(&cache_file) {
            let _ = f.write_all(&all);
        }
    }
}
