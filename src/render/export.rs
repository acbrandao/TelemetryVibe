//! Final render jobs and the render queue.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crossbeam_channel::{Sender, bounded, unbounded};
use parking_lot::Mutex;
use rayon::prelude::*;

use super::compositor::{overlay_region, render_overlay};
use crate::gauges::RenderCtx;
use crate::gauges::model::Gauge;
use crate::telemetry::{SyncSettings, Track, UnitSystem};
use crate::video::encoder::{
    OutputGeometry, RenderSettings, TimeRange, estimated_size, export_args, output_geometry,
};
use crate::video::ffmpeg::{EncoderFamily, FfmpegInfo, command};
use crate::video::probe::VideoInfo;

/// Everything needed to render, snapshotted when the job is queued.
#[derive(Clone)]
pub struct ExportSpec {
    pub output: PathBuf,
    pub info: VideoInfo,
    pub settings: RenderSettings,
    pub gauges: Vec<Gauge>,
    pub track: Option<Arc<Track>>,
    pub sync: SyncSettings,
    pub units: UnitSystem,
    pub ffmpeg: Arc<FfmpegInfo>,
    /// Section of the video to render (timeline trim); `None` renders the whole video.
    pub range: Option<TimeRange>,
}

impl ExportSpec {
    /// The trim range, validated against the source duration.
    pub fn effective_range(&self) -> Option<TimeRange> {
        self.range.and_then(|r| r.normalized(self.info.duration))
    }

    /// Video time of the first rendered frame.
    pub fn start_time(&self) -> f64 {
        self.effective_range().map_or(0.0, |r| r.start)
    }

    /// Length of the rendered output in seconds.
    pub fn duration(&self) -> f64 {
        self.effective_range()
            .map_or(self.info.duration, |r| r.duration())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum JobStatus {
    Queued,
    Running,
    Paused,
    Done,
    Failed(String),
    Cancelled,
}

impl JobStatus {
    pub fn label(&self) -> String {
        match self {
            JobStatus::Queued => "Queued".into(),
            JobStatus::Running => "Rendering".into(),
            JobStatus::Paused => "Paused".into(),
            JobStatus::Done => "Done".into(),
            JobStatus::Failed(e) => format!("Failed: {e}"),
            JobStatus::Cancelled => "Cancelled".into(),
        }
    }
    pub fn is_active(&self) -> bool {
        matches!(self, JobStatus::Running | JobStatus::Paused)
    }
    pub fn is_finished(&self) -> bool {
        matches!(
            self,
            JobStatus::Done | JobStatus::Failed(_) | JobStatus::Cancelled
        )
    }
}

#[derive(Clone, Debug)]
pub struct JobProgress {
    pub status: JobStatus,
    pub frames_done: u64,
    pub total_frames: u64,
    pub encode_fps: f64,
    pub eta: Option<Duration>,
    pub elapsed: Duration,
}

pub struct RenderJob {
    pub id: u64,
    pub spec: ExportSpec,
    pub encoder: EncoderFamily,
    pub geometry: OutputGeometry,
    pub estimated_size: u64,
    pub progress: Mutex<JobProgress>,
    cancel: AtomicBool,
    pause: AtomicBool,
}

static NEXT_JOB: AtomicU64 = AtomicU64::new(1);

impl RenderJob {
    pub fn new(spec: ExportSpec) -> Self {
        let encoder = spec
            .ffmpeg
            .choose_encoder(spec.settings.hw, spec.settings.codec);
        let geometry = output_geometry(&spec.info, &spec.settings);
        let estimated_size = estimated_size(
            &geometry,
            &spec.settings,
            spec.duration(),
            spec.info.has_audio,
        );
        let total = geometry.frame_count(spec.duration());
        Self {
            id: NEXT_JOB.fetch_add(1, Ordering::Relaxed),
            spec,
            encoder,
            geometry,
            estimated_size,
            progress: Mutex::new(JobProgress {
                status: JobStatus::Queued,
                frames_done: 0,
                total_frames: total,
                encode_fps: 0.0,
                eta: None,
                elapsed: Duration::ZERO,
            }),
            cancel: AtomicBool::new(false),
            pause: AtomicBool::new(false),
        }
    }

    pub fn status(&self) -> JobStatus {
        self.progress.lock().status.clone()
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
        let mut p = self.progress.lock();
        if p.status == JobStatus::Queued {
            p.status = JobStatus::Cancelled;
        }
    }

    pub fn set_paused(&self, paused: bool) {
        self.pause.store(paused, Ordering::SeqCst);
    }

    pub fn is_paused(&self) -> bool {
        self.pause.load(Ordering::SeqCst)
    }

    fn set_status(&self, s: JobStatus) {
        self.progress.lock().status = s;
    }

    /// Runs the job to completion (blocking).
    pub fn run(&self, repaint: &(dyn Fn() + Send + Sync)) {
        self.set_status(JobStatus::Running);
        let result = self.run_inner(repaint);
        let status = match result {
            Ok(()) => JobStatus::Done,
            Err(e) if self.cancel.load(Ordering::SeqCst) => {
                tracing::info!(job = self.id, "render cancelled ({e})");
                JobStatus::Cancelled
            }
            Err(e) => {
                tracing::error!(job = self.id, "render failed: {e}");
                JobStatus::Failed(e)
            }
        };
        self.set_status(status);
        repaint();
    }

    fn run_inner(&self, repaint: &(dyn Fn() + Send + Sync)) -> Result<(), String> {
        let spec = &self.spec;
        let geo = self.geometry;
        let region = overlay_region(&spec.gauges, geo.scale, geo.width, geo.height);
        let stem = spec
            .output
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "render".into());
        let tmp = spec.output.with_file_name(format!("{stem}.rendering.mp4"));
        let mut args = export_args(
            &spec.ffmpeg.hwaccel_args(),
            &spec.info,
            &spec.settings,
            self.encoder,
            &geo,
            region,
            spec.effective_range(),
            &tmp,
        );
        // Progress reporting on stdout.
        let out_pos = args.len() - 1;
        args.splice(
            out_pos..out_pos,
            ["-progress".to_string(), "pipe:1".into(), "-nostats".into()],
        );
        tracing::info!(job = self.id, encoder = ?self.encoder, ?region, "starting render: ffmpeg {}", args.join(" "));

        let mut child = command(&spec.ffmpeg.ffmpeg)
            .args(&args)
            .stdin(if region.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("could not start FFmpeg ({e})"))?;

        // Collect stderr for error reporting.
        let stderr_tail = Arc::new(Mutex::new(Vec::<String>::new()));
        let stderr_thread = child.stderr.take().map(|err| {
            let tail = stderr_tail.clone();
            std::thread::spawn(move || {
                for line in BufReader::new(err).lines().map_while(Result::ok) {
                    tracing::warn!(target: "ffmpeg", "{line}");
                    let mut t = tail.lock();
                    t.push(line);
                    if t.len() > 20 {
                        t.remove(0);
                    }
                }
            })
        });
        // Encoded position from -progress.
        let encoded_us = Arc::new(AtomicU64::new(0));
        let stdout_thread = child.stdout.take().map(|out| {
            let pos = encoded_us.clone();
            std::thread::spawn(move || {
                for line in BufReader::new(out).lines().map_while(Result::ok) {
                    if let Some(v) = line.strip_prefix("out_time_us=")
                        && let Ok(us) = v.trim().parse::<i64>()
                    {
                        pos.store(us.max(0) as u64, Ordering::Relaxed);
                    }
                }
            })
        });

        let total = self.progress.lock().total_frames;
        let started = Instant::now();
        let update = |job: &RenderJob| {
            let enc_frames = (encoded_us.load(Ordering::Relaxed) as f64 / 1e6 * geo.fps) as u64;
            let mut p = job.progress.lock();
            p.frames_done = enc_frames.min(total);
            p.elapsed = started.elapsed();
            let secs = p.elapsed.as_secs_f64();
            p.encode_fps = if secs > 0.0 {
                p.frames_done as f64 / secs
            } else {
                0.0
            };
            p.eta = (p.encode_fps > 0.1)
                .then(|| Duration::from_secs_f64((total - p.frames_done) as f64 / p.encode_fps));
        };

        let mut write_error: Option<String> = None;
        if let (Some(region), Some(mut stdin)) = (region, child.stdin.take()) {
            // Producer renders chunks of frames in parallel; the writer streams them in order.
            let (tx, rx) = bounded::<Vec<Vec<u8>>>(2);
            let gauges = spec.gauges.clone();
            let track = spec.track.clone();
            let sync = spec.sync.clone();
            let units = spec.units;
            let fps = geo.fps;
            let scale = geo.scale;
            let t0 = spec.start_time();
            let frames = total + 2;
            let producer = std::thread::spawn(move || {
                const CHUNK: u64 = 16;
                let mut start = 0;
                while start < frames {
                    let end = (start + CHUNK).min(frames);
                    let chunk: Vec<Vec<u8>> = (start..end)
                        .into_par_iter()
                        .map(|i| {
                            let ctx = RenderCtx {
                                track: track.as_deref(),
                                sync: &sync,
                                video_t: t0 + i as f64 / fps,
                                units,
                            };
                            render_overlay(&gauges, &ctx, scale, region)
                        })
                        .collect();
                    if tx.send(chunk).is_err() {
                        return;
                    }
                    start = end;
                }
            });
            let mut last_update = Instant::now();
            'outer: for chunk in rx.iter() {
                for frame in chunk {
                    while self.pause.load(Ordering::SeqCst) && !self.cancel.load(Ordering::SeqCst) {
                        self.set_status(JobStatus::Paused);
                        repaint();
                        std::thread::sleep(Duration::from_millis(100));
                    }
                    if self.progress.lock().status == JobStatus::Paused {
                        self.set_status(JobStatus::Running);
                    }
                    if self.cancel.load(Ordering::SeqCst) {
                        let _ = child.kill();
                        break 'outer;
                    }
                    if let Err(e) = stdin.write_all(&frame) {
                        // FFmpeg stops reading once the main video ends; that is normal.
                        if e.kind() != std::io::ErrorKind::BrokenPipe {
                            write_error = Some(e.to_string());
                        }
                        break 'outer;
                    }
                    if last_update.elapsed() > Duration::from_millis(150) {
                        update(self);
                        repaint();
                        last_update = Instant::now();
                    }
                }
            }
            drop(rx);
            drop(stdin);
            let _ = producer.join();
        } else {
            while child.try_wait().map(|s| s.is_none()).unwrap_or(false) {
                if self.cancel.load(Ordering::SeqCst) {
                    let _ = child.kill();
                    break;
                }
                update(self);
                repaint();
                std::thread::sleep(Duration::from_millis(200));
            }
        }

        let status = child.wait().map_err(|e| e.to_string())?;
        if let Some(t) = stdout_thread {
            let _ = t.join();
        }
        if let Some(t) = stderr_thread {
            let _ = t.join();
        }
        update(self);
        if self.cancel.load(Ordering::SeqCst) {
            let _ = std::fs::remove_file(&tmp);
            return Err("cancelled".into());
        }
        if !status.success() {
            let _ = std::fs::remove_file(&tmp);
            let tail = stderr_tail.lock().join("\n");
            return Err(if tail.is_empty() {
                format!("FFmpeg exited with {status}")
            } else {
                tail.lines().last().unwrap_or("FFmpeg error").to_string()
            });
        }
        if let Some(e) = write_error {
            let _ = std::fs::remove_file(&tmp);
            return Err(format!("failed to stream overlay: {e}"));
        }
        std::fs::rename(&tmp, &spec.output)
            .map_err(|e| format!("could not finalize output ({e})"))?;
        {
            let mut p = self.progress.lock();
            p.frames_done = p.total_frames;
            p.eta = Some(Duration::ZERO);
        }
        tracing::info!(job = self.id, output = %spec.output.display(), elapsed = ?started.elapsed(), "render finished");
        Ok(())
    }
}

/// Sequential render queue running on a background thread.
pub struct RenderQueue {
    jobs: Arc<Mutex<Vec<Arc<RenderJob>>>>,
    running: Arc<AtomicBool>,
    wake: Sender<()>,
    _worker: Option<JoinHandle<()>>,
}

impl RenderQueue {
    pub fn new(repaint: impl Fn() + Send + Sync + 'static) -> Self {
        let jobs: Arc<Mutex<Vec<Arc<RenderJob>>>> = Arc::new(Mutex::new(Vec::new()));
        let running = Arc::new(AtomicBool::new(false));
        let (wake, wake_rx) = unbounded::<()>();
        let (j, r) = (jobs.clone(), running.clone());
        let worker = std::thread::Builder::new()
            .name("render-queue".into())
            .spawn(move || {
                loop {
                    match wake_rx.recv_timeout(Duration::from_millis(500)) {
                        Ok(()) | Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
                        Err(crossbeam_channel::RecvTimeoutError::Disconnected) => return,
                    }
                    while r.load(Ordering::SeqCst) {
                        let next = j
                            .lock()
                            .iter()
                            .find(|job| job.status() == JobStatus::Queued)
                            .cloned();
                        match next {
                            Some(job) => job.run(&repaint),
                            None => {
                                r.store(false, Ordering::SeqCst);
                                repaint();
                            }
                        }
                    }
                }
            })
            .ok();
        Self {
            jobs,
            running,
            wake,
            _worker: worker,
        }
    }

    pub fn add(&self, job: RenderJob) -> Arc<RenderJob> {
        let job = Arc::new(job);
        self.jobs.lock().push(job.clone());
        job
    }

    pub fn start(&self) {
        self.running.store(true, Ordering::SeqCst);
        let _ = self.wake.send(());
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    pub fn jobs(&self) -> Vec<Arc<RenderJob>> {
        self.jobs.lock().clone()
    }

    pub fn active_job(&self) -> Option<Arc<RenderJob>> {
        self.jobs
            .lock()
            .iter()
            .find(|j| j.status().is_active())
            .cloned()
    }

    pub fn remove(&self, id: u64) {
        self.jobs
            .lock()
            .retain(|j| j.id != id || j.status().is_active());
    }

    pub fn clear_finished(&self) {
        self.jobs.lock().retain(|j| !j.status().is_finished());
    }

    pub fn cancel_all(&self) {
        for j in self.jobs.lock().iter() {
            if !j.status().is_finished() {
                j.cancel();
            }
        }
    }
}
