//! Asynchronous preview decoding engine.
//!
//! A worker thread owns FFmpeg decode processes and never blocks the UI:
//!
//! * **Scrubbing** — seek requests are coalesced (latest wins). FFmpeg seeks to the nearest
//!   keyframe and decodes forward to the requested frame; a short burst of neighbouring frames
//!   is decoded in the same pass and cached so frame-stepping is instant.
//! * **Playback** — a streaming decode at preview resolution feeds a bounded ring buffer
//!   (back-pressure keeps memory flat). The UI clock picks frames from it.
//!
//! Frames are converted to GPU-ready images on the worker; the UI only uploads textures.
//! Every request carries a generation number so stale frames are discarded.

use std::io::Read;
use std::path::PathBuf;
use std::process::{Child, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::JoinHandle;

use crossbeam_channel::{Receiver, Sender, TryRecvError, bounded, unbounded};
use egui::ColorImage;
use parking_lot::Mutex;

use super::ffmpeg::command;
use super::frame_cache::FrameCache;

/// What to decode and at which preview size.
#[derive(Clone, Debug, PartialEq)]
pub struct DecoderConfig {
    pub ffmpeg: PathBuf,
    pub source: PathBuf,
    pub fps: f64,
    pub duration: f64,
    pub out_w: u32,
    pub out_h: u32,
    pub hwaccel_args: Vec<String>,
    /// Hardware decoding has a high per-process startup cost, so short seek bursts use it
    /// only for heavy sources (e.g. 4K HEVC without a proxy).
    pub hwaccel_for_seeks: bool,
}

impl DecoderConfig {
    fn index_of(&self, t: f64) -> u64 {
        let max = ((self.duration * self.fps).ceil() as u64).saturating_sub(1);
        ((t.max(0.0) * self.fps + 1e-6).floor() as u64).min(max)
    }

    fn time_of(&self, index: u64) -> f64 {
        index as f64 / self.fps
    }
}

#[derive(Debug)]
enum Cmd {
    Configure(DecoderConfig),
    Seek { t: f64, generation: u64 },
    Play { t: f64, generation: u64 },
    Stop,
    Shutdown,
}

/// A decoded preview frame.
#[derive(Clone)]
pub struct VideoFrame {
    /// Presentation time (seconds) of the frame in the decoded source.
    pub t: f64,
    pub generation: u64,
    pub image: Arc<ColorImage>,
}

/// Handle to the decode worker.
pub struct VideoEngine {
    tx: Sender<Cmd>,
    pub frames: Receiver<VideoFrame>,
    generation: AtomicU64,
    worker: Option<JoinHandle<()>>,
}

impl VideoEngine {
    pub fn new(repaint: impl Fn() + Send + Sync + 'static) -> Self {
        let (tx, rx) = unbounded();
        // Small ring buffer: playback stays a few frames ahead of the clock.
        let (ftx, frx) = bounded(6);
        let repaint: Arc<dyn Fn() + Send + Sync> = Arc::new(repaint);
        let worker = std::thread::Builder::new()
            .name("video-decoder".into())
            .spawn(move || Worker::new(rx, ftx, repaint).run())
            .ok();
        Self {
            tx,
            frames: frx,
            generation: AtomicU64::new(1),
            worker,
        }
    }

    /// Current generation; frames with another generation are stale.
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }

    fn next_generation(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub fn configure(&self, cfg: DecoderConfig) {
        self.next_generation();
        let _ = self.tx.send(Cmd::Configure(cfg));
    }

    /// Requests the frame at `t`. Returns the generation of the request.
    pub fn seek(&self, t: f64) -> u64 {
        let generation = self.next_generation();
        let _ = self.tx.send(Cmd::Seek { t, generation });
        generation
    }

    pub fn play(&self, t: f64) -> u64 {
        let generation = self.next_generation();
        let _ = self.tx.send(Cmd::Play { t, generation });
        generation
    }

    pub fn stop(&self) {
        self.next_generation();
        let _ = self.tx.send(Cmd::Stop);
    }
}

impl Drop for VideoEngine {
    fn drop(&mut self) {
        let _ = self.tx.send(Cmd::Shutdown);
        if let Some(w) = self.worker.take() {
            // Keep draining so a reader blocked on the full ring buffer can exit.
            while !w.is_finished() {
                while self.frames.try_recv().is_ok() {}
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            let _ = w.join();
        }
    }
}

struct Playback {
    child: Child,
    reader: Option<JoinHandle<()>>,
    stop: Arc<AtomicBool>,
}

impl Playback {
    fn stop(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(r) = self.reader.take() {
            // The reader exits on EOF/closed pipe; it may be blocked on a full channel which
            // the UI drains every frame, so this join is short.
            let _ = r.join();
        }
    }
}

struct Worker {
    rx: Receiver<Cmd>,
    out: Sender<VideoFrame>,
    repaint: Arc<dyn Fn() + Send + Sync>,
    cfg: Option<DecoderConfig>,
    cache: Arc<Mutex<FrameCache>>,
    playback: Option<Playback>,
    pending: Option<Cmd>,
}

impl Worker {
    fn new(
        rx: Receiver<Cmd>,
        out: Sender<VideoFrame>,
        repaint: Arc<dyn Fn() + Send + Sync>,
    ) -> Self {
        Self {
            rx,
            out,
            repaint,
            cfg: None,
            cache: Arc::new(Mutex::new(FrameCache::new(768 * 1024 * 1024))),
            playback: None,
            pending: None,
        }
    }

    fn stop_playback(&mut self) {
        if let Some(p) = self.playback.take() {
            p.stop();
        }
    }

    /// Takes the next command, coalescing queued seeks so only the newest is served.
    fn next_cmd(&mut self) -> Option<Cmd> {
        let first = match self.pending.take() {
            Some(c) => c,
            None => self.rx.recv().ok()?,
        };
        let mut cmd = first;
        while let Ok(next) = self.rx.try_recv() {
            match (&cmd, &next) {
                // A newer seek supersedes an older seek or stop.
                (Cmd::Seek { .. } | Cmd::Stop, Cmd::Seek { .. }) => cmd = next,
                _ => {
                    self.pending = Some(next);
                    break;
                }
            }
        }
        Some(cmd)
    }

    fn run(mut self) {
        while let Some(cmd) = self.next_cmd() {
            match cmd {
                Cmd::Shutdown => break,
                Cmd::Configure(cfg) => {
                    self.stop_playback();
                    if self.cfg.as_ref() != Some(&cfg) {
                        self.cache.lock().clear();
                    }
                    self.cfg = Some(cfg);
                }
                Cmd::Stop => self.stop_playback(),
                Cmd::Seek { t, generation } => {
                    self.stop_playback();
                    self.seek(t, generation);
                }
                Cmd::Play { t, generation } => {
                    self.stop_playback();
                    self.start_playback(t, generation);
                }
            }
        }
        self.stop_playback();
    }

    fn spawn_decode(
        cfg: &DecoderConfig,
        start_index: u64,
        max_frames: Option<u64>,
        hw: bool,
    ) -> std::io::Result<Child> {
        // Seek half a frame early so the first decoded frame is exactly `start_index`.
        let ss = (cfg.time_of(start_index) - 0.5 / cfg.fps).max(0.0);
        let mut cmd = command(&cfg.ffmpeg);
        cmd.args(["-hide_banner", "-loglevel", "error", "-nostdin"]);
        if hw {
            cmd.args(&cfg.hwaccel_args);
        }
        cmd.args(["-ss", &format!("{ss:.6}")]);
        cmd.arg("-i").arg(&cfg.source);
        if let Some(n) = max_frames {
            cmd.args(["-frames:v", &n.to_string()]);
        }
        cmd.args([
            "-an",
            "-sn",
            "-dn",
            "-vf",
            &format!("scale={}:{}:flags=fast_bilinear", cfg.out_w, cfg.out_h),
            "-pix_fmt",
            "rgba",
            "-f",
            "rawvideo",
            "pipe:1",
        ]);
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        cmd.spawn()
    }

    /// Sends a frame to the UI, waking it while the ring buffer is full of stale frames.
    fn deliver(&mut self, mut frame: VideoFrame) {
        loop {
            match self
                .out
                .send_timeout(frame, std::time::Duration::from_millis(30))
            {
                Ok(()) => {
                    (self.repaint)();
                    return;
                }
                Err(crossbeam_channel::SendTimeoutError::Timeout(f)) => {
                    (self.repaint)();
                    if !self.rx.is_empty() {
                        return;
                    }
                    frame = f;
                }
                Err(crossbeam_channel::SendTimeoutError::Disconnected(_)) => return,
            }
        }
    }

    fn to_image(buf: &[u8], w: u32, h: u32) -> Arc<ColorImage> {
        Arc::new(ColorImage::from_rgba_unmultiplied(
            [w as usize, h as usize],
            buf,
        ))
    }

    fn seek(&mut self, t: f64, generation: u64) {
        let Some(cfg) = self.cfg.clone() else { return };
        let target = cfg.index_of(t);
        let cached = self.cache.lock().get(target);
        if let Some(img) = cached {
            self.deliver(VideoFrame {
                t: cfg.time_of(target),
                generation,
                image: img,
            });
            return;
        }
        // Burst: a few frames before (for stepping back) and after the target.
        let start = target.saturating_sub(3);
        let count = 12;
        let mut child = match Self::spawn_decode(&cfg, start, Some(count), cfg.hwaccel_for_seeks) {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("failed to start decoder: {e}");
                return;
            }
        };
        let frame_bytes = cfg.out_w as usize * cfg.out_h as usize * 4;
        let mut buf = vec![0u8; frame_bytes];
        let mut sent = false;
        let mut last: Option<(u64, Arc<ColorImage>)> = None;
        if let Some(mut stdout) = child.stdout.take() {
            for k in 0..count {
                if stdout.read_exact(&mut buf).is_err() {
                    break;
                }
                let idx = start + k;
                let img = Self::to_image(&buf, cfg.out_w, cfg.out_h);
                self.cache.lock().insert(idx, img.clone());
                if idx == target {
                    self.deliver(VideoFrame {
                        t: cfg.time_of(idx),
                        generation,
                        image: img.clone(),
                    });
                    sent = true;
                }
                last = Some((idx, img));
                // Abort the remainder of the burst if the user moved on.
                if sent {
                    match self.rx.try_recv() {
                        Ok(cmd) => {
                            self.pending = Some(cmd);
                            break;
                        }
                        Err(TryRecvError::Empty) => {}
                        Err(TryRecvError::Disconnected) => break,
                    }
                }
            }
        }
        let _ = child.kill();
        let _ = child.wait();
        // Past the end of the stream: show the last decodable frame.
        if !sent && let Some((idx, img)) = last {
            self.deliver(VideoFrame {
                t: cfg.time_of(idx),
                generation,
                image: img,
            });
        }
    }

    fn start_playback(&mut self, t: f64, generation: u64) {
        let Some(cfg) = self.cfg.clone() else { return };
        let start = cfg.index_of(t);
        let mut child = match Self::spawn_decode(&cfg, start, None, true) {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("failed to start playback decoder: {e}");
                return;
            }
        };
        let Some(mut stdout) = child.stdout.take() else {
            let _ = child.kill();
            return;
        };
        let out = self.out.clone();
        let cache = self.cache.clone();
        let repaint = self.repaint.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let stop_flag = stop.clone();
        let reader = std::thread::Builder::new()
            .name("video-playback".into())
            .spawn(move || {
                let frame_bytes = cfg.out_w as usize * cfg.out_h as usize * 4;
                let mut buf = vec![0u8; frame_bytes];
                let mut idx = start;
                while stdout.read_exact(&mut buf).is_ok() {
                    let img = Self::to_image(&buf, cfg.out_w, cfg.out_h);
                    // Keep a sparse set of playback frames for instant scrub-back.
                    if idx % 4 == 0 {
                        cache.lock().insert(idx, img.clone());
                    }
                    let mut frame = VideoFrame {
                        t: cfg.time_of(idx),
                        generation,
                        image: img,
                    };
                    // Timed sends let a stop request through even if the UI is idle.
                    loop {
                        match out.send_timeout(frame, std::time::Duration::from_millis(40)) {
                            Ok(()) => break,
                            Err(crossbeam_channel::SendTimeoutError::Timeout(f)) => {
                                if stop_flag.load(Ordering::SeqCst) {
                                    return;
                                }
                                frame = f;
                            }
                            Err(crossbeam_channel::SendTimeoutError::Disconnected(_)) => return,
                        }
                    }
                    if stop_flag.load(Ordering::SeqCst) {
                        return;
                    }
                    repaint();
                    idx += 1;
                }
            })
            .ok();
        self.playback = Some(Playback {
            child,
            reader,
            stop,
        });
    }
}

/// Picks a preview decode size for a display area and quality factor (keeps aspect, even dims).
pub fn preview_size(
    video_w: u32,
    video_h: u32,
    max_w: f32,
    max_h: f32,
    quality: f32,
) -> (u32, u32) {
    let scale = (max_w / video_w as f32)
        .min(max_h / video_h as f32)
        .min(1.0)
        * quality.clamp(0.1, 1.0);
    let w = ((video_w as f32 * scale / 2.0).round() as u32 * 2).max(16);
    let h = ((video_h as f32 * scale / 2.0).round() as u32 * 2).max(16);
    (w, h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_mapping() {
        let cfg = DecoderConfig {
            ffmpeg: "ffmpeg".into(),
            source: "x".into(),
            fps: 30.0,
            duration: 10.0,
            out_w: 16,
            out_h: 16,
            hwaccel_args: vec![],
            hwaccel_for_seeks: false,
        };
        assert_eq!(cfg.index_of(0.0), 0);
        assert_eq!(cfg.index_of(1.0), 30);
        assert_eq!(cfg.index_of(1.0 / 30.0 * 7.0), 7);
        assert_eq!(cfg.index_of(100.0), 299);
        assert!((cfg.time_of(30) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn preview_sizes() {
        assert_eq!(preview_size(3840, 2160, 1280.0, 720.0, 1.0), (1280, 720));
        assert_eq!(preview_size(3840, 2160, 1280.0, 720.0, 0.5), (640, 360));
        assert_eq!(preview_size(640, 360, 1280.0, 720.0, 1.0), (640, 360));
        let (w, h) = preview_size(1080, 1920, 1000.0, 1000.0, 1.0);
        assert!(w % 2 == 0 && h % 2 == 0 && h <= 1000);
    }
}
