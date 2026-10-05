//! The eframe application: owns state, background workers and the UI frame loop.
//!
//! Heavy work (FFmpeg detection, probing, FIT parsing, decoding, thumbnails, proxies, rendering)
//! runs on worker threads which report back through a channel; the UI thread only applies
//! results and uploads textures.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, unbounded};
use egui::{Key, KeyboardShortcut, Modifiers, TextureHandle, TextureOptions};

use super::commands::Command;
use super::state::AppState;
use crate::gauges::library::{PresetId, Template, apply_template, make_preset};
use crate::gauges::model::GaugeId;
use crate::project::Project;
use crate::project::autosave::{self, AppSettings, Recovery};
use crate::project::model::{self as pmodel, MediaRef};
use crate::project::templates::TemplateStore;
use crate::render::compositor::rasterize_gauge;
use crate::render::export::RenderQueue;
use crate::telemetry::{self, Track, sync};
use crate::ui;
use crate::utils::paths::AppPaths;
use crate::video::decoder::{DecoderConfig, VideoEngine, VideoFrame, preview_size};
use crate::video::encoder::TimeRange;
use crate::video::ffmpeg::{self, FfmpegInfo};
use crate::video::thumbnails::{self, Thumbnail};
use crate::video::{VideoInfo, probe, proxy};

/// Results from background work.
pub enum BgEvent {
    Ffmpeg(Result<FfmpegInfo, String>),
    VideoProbed(PathBuf, Result<VideoInfo, String>),
    TrackLoaded(PathBuf, Result<Box<Track>, String>),
    Thumb(u64, Thumbnail),
    ProxyProgress(f32),
    ProxyDone(PathBuf, Result<PathBuf, String>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Warning,
    Error,
}

pub struct Toast {
    pub kind: ToastKind,
    pub text: String,
    pub until: Instant,
}

#[derive(Default)]
pub enum ProxyState {
    #[default]
    None,
    Generating {
        progress: f32,
        cancel: Arc<AtomicBool>,
    },
    Ready {
        path: PathBuf,
        fps: f64,
    },
}

pub struct GaugeTexture {
    pub tex: TextureHandle,
    pub key: (u64, i64, u32),
    pub margin: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RightTab {
    #[default]
    Library,
    Inspector,
}

/// Transient UI state (not saved in projects).
#[derive(Default)]
pub struct UiState {
    pub right_tab: RightTab,
    pub show_sync: bool,
    pub show_render: bool,
    pub show_queue: bool,
    pub show_settings: bool,
    pub show_ffmpeg_info: bool,
    pub show_shortcuts: bool,
    pub show_about: bool,
    pub toasts: Vec<Toast>,
    pub canvas: ui::canvas::CanvasState,
    pub timeline: ui::timeline::TimelineState,
    pub sync: ui::sync_panel::SyncState,
    pub render: ui::render_panel::RenderDialogState,
    pub recovery: Option<Recovery>,
    pub confirm_discard: Option<PendingAction>,
    pub offset_text: String,
    pub offset_text_focused: bool,
    pub template_replace: bool,
    /// Name being edited in a user template's context menu: (original, edited).
    pub template_rename: Option<(String, String)>,
    pub close_confirmed: bool,
}

/// An action waiting for "discard unsaved changes?" confirmation.
#[derive(Clone, Debug)]
pub enum PendingAction {
    New,
    Open(Option<PathBuf>),
    Quit,
}

pub struct GaugeApp {
    pub(crate) paths: AppPaths,
    pub(crate) settings: AppSettings,
    pub(crate) state: AppState,
    pub(crate) ffmpeg: Option<Arc<FfmpegInfo>>,
    pub(crate) ffmpeg_error: Option<String>,
    pub(crate) ffmpeg_detecting: bool,
    pub(crate) engine: VideoEngine,
    pub(crate) queue: RenderQueue,
    pub(crate) bg_tx: Sender<BgEvent>,
    bg_rx: Receiver<BgEvent>,
    pub(crate) recent: Vec<PathBuf>,
    pub(crate) templates: TemplateStore,
    pub(crate) ui: UiState,
    pub(crate) egui_ctx: egui::Context,

    // Media loading.
    pub(crate) loading_video: Option<PathBuf>,
    pub(crate) loading_track: Option<PathBuf>,

    // Preview.
    pub(crate) video_tex: Option<TextureHandle>,
    pub(crate) video_frame_t: Option<f64>,
    pending_frames: VecDeque<VideoFrame>,
    pub(crate) playing: bool,
    play_anchor: Option<(Instant, f64)>,
    last_seek: Option<i64>,
    pub(crate) decoder_cfg: Option<DecoderConfig>,
    /// Physical pixel size of the video area on screen (for decode sizing).
    pub(crate) canvas_px: (f32, f32),
    pub(crate) gauge_tex: HashMap<GaugeId, GaugeTexture>,
    pub(crate) thumbs: Vec<(f64, TextureHandle)>,
    thumb_epoch: u64,
    thumb_cancel: Arc<AtomicBool>,
    pub(crate) proxy: ProxyState,
    pub(crate) theme_applied: Option<autosave::Theme>,
    pub(crate) ui_icons: HashMap<PresetId, TextureHandle>,
    last_title: String,
    pub(crate) dev_script: Option<super::devscript::DevScript>,
}

impl GaugeApp {
    pub fn new(cc: &eframe::CreationContext<'_>, open_path: Option<PathBuf>) -> Self {
        let paths = AppPaths::resolve();
        let settings = AppSettings::load(&paths);
        ui::theme::install_fonts(&cc.egui_ctx);
        ui::theme::apply(&cc.egui_ctx, settings.theme);
        let (bg_tx, bg_rx) = unbounded();
        let ctx = cc.egui_ctx.clone();
        let engine = VideoEngine::new({
            let ctx = ctx.clone();
            move || ctx.request_repaint()
        });
        let queue = RenderQueue::new({
            let ctx = ctx.clone();
            move || ctx.request_repaint()
        });
        let recovery = autosave::begin_session(&paths);
        let recent = autosave::load_recent(&paths);
        let templates = TemplateStore::load(TemplateStore::file_path(&paths.config_dir));
        let mut app = Self {
            paths,
            settings: settings.clone(),
            state: AppState::default(),
            ffmpeg: None,
            ffmpeg_error: None,
            ffmpeg_detecting: false,
            engine,
            queue,
            bg_tx,
            bg_rx,
            recent,
            templates,
            ui: UiState {
                recovery,
                ..Default::default()
            },
            egui_ctx: ctx,
            loading_video: None,
            loading_track: None,
            video_tex: None,
            video_frame_t: None,
            pending_frames: VecDeque::new(),
            playing: false,
            play_anchor: None,
            last_seek: None,
            decoder_cfg: None,
            canvas_px: (1280.0, 720.0),
            gauge_tex: HashMap::new(),
            thumbs: Vec::new(),
            thumb_epoch: 0,
            thumb_cancel: Arc::new(AtomicBool::new(false)),
            proxy: ProxyState::None,
            theme_applied: Some(settings.theme),
            ui_icons: HashMap::new(),
            last_title: String::new(),
            dev_script: super::devscript::DevScript::from_env(),
        };
        app.detect_ffmpeg();
        if let Some(p) = open_path {
            app.open_project_path(&p);
        }
        app
    }

    // ------------------------------------------------------------------ notifications

    pub fn toast(&mut self, kind: ToastKind, text: impl Into<String>) {
        let text = text.into();
        match kind {
            ToastKind::Error => tracing::error!("{text}"),
            ToastKind::Warning => tracing::warn!("{text}"),
            _ => tracing::info!("{text}"),
        }
        let secs = match kind {
            ToastKind::Error => 8,
            ToastKind::Warning => 6,
            _ => 3,
        };
        self.ui.toasts.push(Toast {
            kind,
            text,
            until: Instant::now() + Duration::from_secs(secs),
        });
    }

    // ------------------------------------------------------------------ FFmpeg

    pub fn detect_ffmpeg(&mut self) {
        self.ffmpeg_detecting = true;
        let custom = self.settings.ffmpeg_path.clone();
        let tx = self.bg_tx.clone();
        let ctx = self.egui_ctx.clone();
        std::thread::spawn(move || {
            let r = ffmpeg::detect(custom.as_deref()).map_err(|e| e.to_string());
            let _ = tx.send(BgEvent::Ffmpeg(r));
            ctx.request_repaint();
        });
    }

    // ------------------------------------------------------------------ media import

    pub fn import_video(&mut self, path: PathBuf) {
        let Some(ff) = self.ffmpeg.clone() else {
            self.toast(ToastKind::Error, "FFmpeg is required to import video. Install FFmpeg or set its location in Settings.");
            return;
        };
        self.loading_video = Some(path.clone());
        let tx = self.bg_tx.clone();
        let ctx = self.egui_ctx.clone();
        std::thread::spawn(move || {
            let r = probe::probe(&ff, &path).map_err(|e| e.to_string());
            let _ = tx.send(BgEvent::VideoProbed(path, r));
            ctx.request_repaint();
        });
    }

    pub fn import_telemetry(&mut self, path: PathBuf) {
        self.loading_track = Some(path.clone());
        let tx = self.bg_tx.clone();
        let ctx = self.egui_ctx.clone();
        std::thread::spawn(move || {
            let r = telemetry::load_file(&path).map(Box::new).map_err(|e| {
                tracing::error!(error = ?e, "telemetry load failed");
                e.to_string()
            });
            let _ = tx.send(BgEvent::TrackLoaded(path, r));
            ctx.request_repaint();
        });
    }

    pub fn import_dialog_video(&mut self) {
        let mut d = rfd::FileDialog::new()
            .set_title("Import Video")
            .add_filter("Video", &["mp4", "mov", "mkv", "m4v", "MP4", "MOV"]);
        if let Some(dir) = &self.settings.last_dir {
            d = d.set_directory(dir);
        }
        if let Some(p) = d.pick_file() {
            self.remember_dir(&p);
            self.import_video(p);
        }
    }

    pub fn import_dialog_gps(&mut self) {
        let exts = telemetry::supported_extensions();
        let mut all: Vec<String> = exts.iter().map(|s| s.to_string()).collect();
        all.extend(exts.iter().map(|s| s.to_uppercase()));
        let mut d = rfd::FileDialog::new()
            .set_title("Import GPS Data")
            .add_filter("GPS / telemetry", &all);
        if let Some(dir) = &self.settings.last_dir {
            d = d.set_directory(dir);
        }
        if let Some(p) = d.pick_file() {
            self.remember_dir(&p);
            self.import_telemetry(p);
        }
    }

    fn remember_dir(&mut self, p: &Path) {
        if let Some(dir) = p.parent() {
            self.settings.last_dir = Some(dir.to_path_buf());
            self.settings.save(&self.paths);
        }
    }

    pub fn handle_dropped_file(&mut self, path: PathBuf) {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if ext == pmodel::PROJECT_EXTENSION {
            self.request(PendingAction::Open(Some(path)));
        } else if telemetry::is_telemetry_file(&path) {
            self.import_telemetry(path);
        } else if probe::is_video_file(&path) {
            self.import_video(path);
        } else {
            self.toast(
                ToastKind::Warning,
                format!("Unsupported file: {}", path.display()),
            );
        }
    }

    fn on_video_loaded(&mut self, info: VideoInfo) {
        let name = info.file_name();
        self.pause();
        self.state.video = Some(info.clone());
        self.state.project.video = Some(MediaRef::new(&info.path));
        if self.state.project.name == "Untitled" {
            self.state.project.name = info
                .path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
        }
        self.state.playhead = self.state.playhead.min(info.duration);
        self.state.touch();
        self.state.autosaver.mark_dirty(true);
        self.video_tex = None;
        self.video_frame_t = None;
        self.gauge_tex.clear();

        // Proxy: reuse a cached one, or offer/auto-generate for heavy sources.
        self.proxy = ProxyState::None;
        let proxy_file = proxy::proxy_path(&self.paths.proxy_dir(), &info.path);
        if proxy_file.exists() {
            let (_, _, fps) = proxy::proxy_params(&info);
            self.proxy = ProxyState::Ready {
                path: proxy_file,
                fps,
            };
        } else if info.wants_proxy() && self.settings.auto_proxy {
            self.start_proxy();
        }
        self.reconfigure_decoder(true);
        self.start_thumbnails();
        self.toast(ToastKind::Success, format!("Video loaded: {name}"));
        self.try_auto_sync(false);
    }

    fn on_track_loaded(&mut self, track: Track, path: PathBuf) {
        let summary = format!("GPS loaded: {} ({} samples)", track.name, track.len());
        self.state.track = Some(Arc::new(track));
        self.state.project.telemetry = Some(MediaRef::new(&path));
        self.state.touch();
        self.state.autosaver.mark_dirty(true);
        self.toast(ToastKind::Success, summary);
        self.try_auto_sync(false);
    }

    /// Applies metadata-based sync. Unless `force`, only when no offset was set yet.
    pub fn try_auto_sync(&mut self, force: bool) -> bool {
        let (Some(video), Some(track)) = (&self.state.video, &self.state.track) else {
            return false;
        };
        if !force && self.state.project.sync.offset != 0.0 {
            return false;
        }
        let Some(created) = video.creation_time else {
            if force {
                self.toast(ToastKind::Warning, "This video has no recording time metadata. Use event sync or the offset controls.");
            }
            return false;
        };
        let offset = sync::offset_from_creation_time(created, track);
        if sync::overlaps(offset, video.duration, track) {
            self.state.execute(Command::SetOffset(offset));
            self.toast(
                ToastKind::Success,
                format!(
                    "Synchronized automatically (offset {})",
                    crate::utils::timecode::format_offset(offset)
                ),
            );
            true
        } else {
            // Many action cameras store local time labelled as UTC; try whole-hour corrections.
            for h in 1..=14 {
                for sign in [-1.0, 1.0] {
                    let o = offset + sign * h as f64 * 3600.0;
                    if sync::overlaps(o, video.duration, track) {
                        self.state.execute(Command::SetOffset(o));
                        self.toast(
                            ToastKind::Info,
                            format!("Synchronized using metadata with a {}{}h timezone correction. Verify in Sync.", if sign > 0.0 { "+" } else { "-" }, h),
                        );
                        return true;
                    }
                }
            }
            if force {
                self.toast(ToastKind::Warning, "The video's recording time is outside the GPS recording. Use event sync instead.");
            }
            false
        }
    }

    // ------------------------------------------------------------------ thumbnails & proxy

    fn start_thumbnails(&mut self) {
        self.thumb_cancel.store(true, Ordering::Relaxed);
        self.thumbs.clear();
        let (Some(ff), Some(info)) = (self.ffmpeg.clone(), self.state.video.clone()) else {
            return;
        };
        self.thumb_epoch += 1;
        let epoch = self.thumb_epoch;
        let cancel = Arc::new(AtomicBool::new(false));
        self.thumb_cancel = cancel.clone();
        let tx = self.bg_tx.clone();
        let ctx = self.egui_ctx.clone();
        let dir = self.paths.thumbnail_dir();
        std::thread::Builder::new()
            .name("thumbnails".into())
            .spawn(move || {
                let count = (info.duration / 2.0).clamp(8.0, 120.0) as usize;
                thumbnails::generate(&ff.ffmpeg, &info, count, &dir, &cancel, |t| {
                    let _ = tx.send(BgEvent::Thumb(epoch, t));
                    ctx.request_repaint();
                });
            })
            .ok();
    }

    pub fn start_proxy(&mut self) {
        let (Some(ff), Some(info)) = (self.ffmpeg.clone(), self.state.video.clone()) else {
            return;
        };
        if matches!(self.proxy, ProxyState::Generating { .. }) {
            return;
        }
        let cancel = Arc::new(AtomicBool::new(false));
        self.proxy = ProxyState::Generating {
            progress: 0.0,
            cancel: cancel.clone(),
        };
        let out = proxy::proxy_path(&self.paths.proxy_dir(), &info.path);
        let tx = self.bg_tx.clone();
        let ctx = self.egui_ctx.clone();
        std::thread::Builder::new()
            .name("proxy".into())
            .spawn(move || {
                let mut last = Instant::now();
                let r = proxy::generate(&ff, &info, &out, &cancel, |p| {
                    if last.elapsed() > Duration::from_millis(200) {
                        let _ = tx.send(BgEvent::ProxyProgress(p));
                        ctx.request_repaint();
                        last = Instant::now();
                    }
                })
                .map_err(|e| e.to_string());
                let _ = tx.send(BgEvent::ProxyDone(info.path.clone(), r));
                ctx.request_repaint();
            })
            .ok();
    }

    pub fn cancel_proxy(&mut self) {
        if let ProxyState::Generating { cancel, .. } = &self.proxy {
            cancel.store(true, Ordering::Relaxed);
        }
    }

    // ------------------------------------------------------------------ preview decoding

    /// (Re)configures the decoder for the current source, canvas size and quality.
    pub fn reconfigure_decoder(&mut self, force: bool) {
        let (Some(ff), Some(info)) = (self.ffmpeg.clone(), self.state.video.clone()) else {
            return;
        };
        let pv = &self.state.project.preview;
        let (source, fps, using_proxy) = match (&self.proxy, pv.use_proxy) {
            (ProxyState::Ready { path, fps }, true) => (path.clone(), *fps, true),
            _ => (info.path.clone(), info.fps, false),
        };
        let (w, h) = preview_size(
            info.width,
            info.height,
            self.canvas_px.0,
            self.canvas_px.1,
            pv.quality.factor(),
        );
        let cfg = DecoderConfig {
            ffmpeg: ff.ffmpeg.clone(),
            source,
            fps,
            duration: info.duration,
            out_w: w,
            out_h: h,
            hwaccel_args: if self.settings.hardware_decode {
                ff.hwaccel_args()
            } else {
                Vec::new()
            },
            hwaccel_for_seeks: !using_proxy && info.width.max(info.height) > 2560,
        };
        if !force && self.decoder_cfg.as_ref() == Some(&cfg) {
            return;
        }
        tracing::debug!(?cfg, "decoder configured");
        self.decoder_cfg = Some(cfg.clone());
        self.engine.configure(cfg);
        self.pending_frames.clear();
        self.last_seek = None;
        if self.playing {
            self.engine.play(self.state.playhead);
            self.play_anchor = Some((Instant::now(), self.state.playhead));
        } else {
            self.request_frame();
        }
    }

    /// Called by the canvas with its on-screen video size in physical pixels.
    pub fn update_canvas_size(&mut self, w: f32, h: f32) {
        let (ow, oh) = self.canvas_px;
        let changed = (w / ow - 1.0).abs() > 0.2 || (h / oh - 1.0).abs() > 0.2;
        self.canvas_px = (w, h);
        if changed && self.state.video.is_some() {
            self.reconfigure_decoder(false);
        }
    }

    fn request_frame(&mut self) {
        if self.state.video.is_none() {
            return;
        }
        let fps = self.decoder_cfg.as_ref().map(|c| c.fps).unwrap_or(30.0);
        let idx = (self.state.playhead * fps + 1e-6).floor() as i64;
        if self.last_seek == Some(idx) {
            return;
        }
        self.last_seek = Some(idx);
        self.pending_frames.clear();
        self.engine.seek(self.state.playhead);
    }

    pub fn set_playhead(&mut self, t: f64) {
        let t = t.clamp(0.0, self.state.duration());
        if self.playing {
            self.pause();
        }
        self.state.playhead = t;
        self.request_frame();
    }

    pub fn step_frames(&mut self, n: i64) {
        let fd = 1.0 / self.state.fps();
        // Snap to frame grid.
        let idx = (self.state.playhead / fd + 1e-6).floor() as i64 + n;
        self.set_playhead(idx.max(0) as f64 * fd);
    }

    pub fn play(&mut self) {
        if self.playing {
            return;
        }
        if self.state.playhead >= self.state.duration() - 0.05 {
            self.state.playhead = 0.0;
        }
        self.playing = true;
        self.play_anchor = Some((Instant::now(), self.state.playhead));
        self.pending_frames.clear();
        if self.state.video.is_some() {
            self.engine.play(self.state.playhead);
        }
    }

    pub fn pause(&mut self) {
        if !self.playing {
            return;
        }
        self.playing = false;
        self.play_anchor = None;
        self.engine.stop();
        self.last_seek = None;
        self.request_frame();
    }

    pub fn toggle_play(&mut self) {
        if self.playing {
            self.pause();
        } else {
            self.play();
        }
    }

    pub fn stop(&mut self) {
        self.pause();
        self.set_playhead(0.0);
    }

    fn update_playback(&mut self) {
        if self.playing
            && let Some((start, t0)) = self.play_anchor
        {
            let t = t0 + start.elapsed().as_secs_f64();
            let dur = self.state.duration();
            if t >= dur {
                self.state.playhead = dur;
                self.pause();
            } else {
                self.state.playhead = t;
            }
        }
        // Intake decoded frames.
        let generation = self.engine.generation();
        while self.pending_frames.len() < 4 {
            match self.engine.frames.try_recv() {
                Ok(f) if f.generation == generation => self.pending_frames.push_back(f),
                Ok(f) => {
                    tracing::trace!(
                        stale = f.generation,
                        current = generation,
                        "dropping stale frame"
                    );
                    continue;
                }
                Err(_) => break,
            }
        }
        let mut show: Option<VideoFrame> = None;
        if self.playing {
            let fd = self
                .decoder_cfg
                .as_ref()
                .map(|c| 1.0 / c.fps)
                .unwrap_or(1.0 / 30.0);
            while let Some(f) = self.pending_frames.front() {
                if f.t <= self.state.playhead + fd * 0.5 {
                    show = self.pending_frames.pop_front();
                } else {
                    break;
                }
            }
        } else {
            show = self.pending_frames.drain(..).next_back();
        }
        if let Some(f) = show {
            let img = (*f.image).clone();
            match &mut self.video_tex {
                Some(t) => t.set(img, TextureOptions::LINEAR),
                None => {
                    self.video_tex = Some(self.egui_ctx.load_texture(
                        "video",
                        img,
                        TextureOptions::LINEAR,
                    ))
                }
            }
            self.video_frame_t = Some(f.t);
        }
    }

    // ------------------------------------------------------------------ gauge previews

    /// Re-rasterizes gauges whose content changed (in parallel) and uploads textures.
    pub(crate) fn update_gauge_textures(&mut self, scale: f32) {
        use rayon::prelude::*;
        let time_key = (self.state.playhead * 1000.0).round() as i64;
        let scale_key = (scale * 1000.0).round() as u32;
        let rev = self.state.revision;
        let ctx = self.state.render_ctx();
        let todo: Vec<&crate::gauges::model::Gauge> = self
            .state
            .project
            .gauges
            .iter()
            .filter(|g| g.visible)
            .filter(|g| {
                let time_dep = !matches!(g.kind, crate::gauges::model::GaugeKind::Image { .. });
                let key = (rev, if time_dep { time_key } else { 0 }, scale_key);
                self.gauge_tex.get(&g.id).is_none_or(|t| t.key != key)
            })
            .collect();
        if todo.is_empty() {
            return;
        }
        let rendered: Vec<(
            GaugeId,
            bool,
            Option<crate::render::compositor::PreviewRaster>,
        )> = todo
            .par_iter()
            .map(|g| {
                let time_dep = !matches!(g.kind, crate::gauges::model::GaugeKind::Image { .. });
                (g.id, time_dep, rasterize_gauge(g, &ctx, scale))
            })
            .collect();
        for (id, time_dep, raster) in rendered {
            let Some(r) = raster else { continue };
            let key = (rev, if time_dep { time_key } else { 0 }, scale_key);
            match self.gauge_tex.get_mut(&id) {
                Some(t) => {
                    t.tex.set(r.image, TextureOptions::LINEAR);
                    t.key = key;
                    t.margin = r.margin;
                }
                None => {
                    let tex = self.egui_ctx.load_texture(
                        format!("gauge-{}", id.0),
                        r.image,
                        TextureOptions::LINEAR,
                    );
                    self.gauge_tex.insert(
                        id,
                        GaugeTexture {
                            tex,
                            key,
                            margin: r.margin,
                        },
                    );
                }
            }
        }
        let ids: Vec<GaugeId> = self.state.project.gauges.iter().map(|g| g.id).collect();
        self.gauge_tex.retain(|id, _| ids.contains(id));
    }

    // ------------------------------------------------------------------ gauges

    pub fn add_preset(&mut self, preset: PresetId, center: Option<(f32, f32)>) {
        let id = self.state.project.alloc_gauge_id();
        let size = self.state.video_size();
        let mut g = make_preset(
            preset,
            id,
            size,
            self.state.track.as_deref(),
            self.state.project.units,
        );
        match center {
            Some((cx, cy)) => {
                g.placement.x = (cx - g.placement.w / 2.0).round();
                g.placement.y = (cy - g.placement.h / 2.0).round();
            }
            None => {
                // Cascade new gauges so they don't stack exactly.
                let n = self.state.project.gauges.len() as f32 % 8.0;
                g.placement.x += n * 24.0 - 84.0;
                g.placement.y += n * 24.0 - 84.0;
            }
        }
        if let (Some(m), Some(track)) = (preset.metric(), &self.state.track)
            && m != crate::telemetry::Metric::Latitude
            && !track.has(m)
        {
            self.toast(ToastKind::Warning, telemetry::missing_metric_message(m));
        }
        if matches!(
            preset,
            PresetId::FullRouteMap | PresetId::MovingRoute | PresetId::TrailMap
        ) && self.state.track.as_ref().is_some_and(|t| t.route.is_none())
        {
            self.toast(
                ToastKind::Warning,
                "This GPS recording does not contain position data.",
            );
        }
        self.state.execute(Command::AddGauges(vec![g]));
        self.state.selection = vec![id];
        self.ui.right_tab = RightTab::Inspector;
    }

    pub fn apply_template(&mut self, t: Template) {
        let size = self.state.video_size();
        let track = self.state.track.clone();
        let units = self.state.project.units;
        let replace = self.ui.template_replace || self.state.project.gauges.is_empty();
        let project = &mut self.state.project;
        let mut next = || project.alloc_gauge_id();
        let gauges = apply_template(t, size, track.as_deref(), units, &mut next);
        let ids: Vec<GaugeId> = gauges.iter().map(|g| g.id).collect();
        if replace {
            self.state.execute(Command::ReplaceGauges(gauges));
        } else {
            self.state.execute(Command::AddGauges(gauges));
        }
        self.state.selection = ids;
        // Set after the command so undo restores the previous base template.
        self.state.project.template = Some(t.name().to_string());
        self.toast(ToastKind::Success, format!("{} template applied", t.name()));
    }

    /// Removes every gauge (one undo step).
    pub fn clear_gauges(&mut self) {
        let n = self.state.project.gauges.len();
        if n == 0 {
            return;
        }
        self.state.execute(Command::ClearGauges);
        self.state.selection.clear();
        // A fresh layout no longer derives from the previous template.
        self.state.project.template = None;
        self.toast(
            ToastKind::Info,
            format!(
                "Cleared {n} gauge{} — Undo (⌘Z) restores them",
                if n == 1 { "" } else { "s" }
            ),
        );
    }

    /// Saves the current gauge layout and settings as a user template, named automatically
    /// after the template it started from.
    pub fn save_current_template(&mut self) {
        if self.state.project.gauges.is_empty() {
            self.toast(
                ToastKind::Warning,
                "Add some gauges before saving a template.",
            );
            return;
        }
        let base = self.state.project.template.clone();
        let size = self.state.video_size();
        let gauges = self.state.project.gauges.clone();
        let name = self.templates.add(base.as_deref(), size, gauges);
        match self.templates.save() {
            Ok(()) => self.toast(
                ToastKind::Success,
                format!("Template saved as \"{name}\" (in the Template menu)"),
            ),
            Err(e) => {
                self.templates.remove(&name);
                self.toast(
                    ToastKind::Error,
                    format!("Could not save the template: {e}"),
                );
            }
        }
    }

    pub fn apply_user_template(&mut self, name: &str) {
        let Some(t) = self.templates.find(name).cloned() else {
            return;
        };
        let size = self.state.video_size();
        let replace = self.ui.template_replace || self.state.project.gauges.is_empty();
        let project = &mut self.state.project;
        let mut next_group = project.next_group_id;
        let gauges = {
            let mut next = || project.alloc_gauge_id();
            t.instantiate(size, &mut next, &mut || {
                let g = next_group;
                next_group += 1;
                g
            })
        };
        project.next_group_id = next_group;
        let ids: Vec<GaugeId> = gauges.iter().map(|g| g.id).collect();
        if replace {
            self.state.execute(Command::ReplaceGauges(gauges));
        } else {
            self.state.execute(Command::AddGauges(gauges));
        }
        self.state.selection = ids;
        self.state.project.template = Some(t.based_on.clone().unwrap_or(t.name.clone()));
        use crate::telemetry::Metric;
        let missing = self.state.track.as_ref().and_then(|track| {
            t.gauges
                .iter()
                .map(|g| g.metric)
                .find(|m| !matches!(m, Metric::Latitude | Metric::Custom) && !track.has(*m))
        });
        if let Some(m) = missing {
            self.toast(ToastKind::Warning, telemetry::missing_metric_message(m));
        }
        self.toast(ToastKind::Success, format!("{} template applied", t.name));
    }

    pub fn delete_user_template(&mut self, name: &str) {
        if self.templates.remove(name) {
            if let Err(e) = self.templates.save() {
                self.toast(ToastKind::Error, format!("Could not update templates: {e}"));
            } else {
                self.toast(ToastKind::Info, format!("Template \"{name}\" deleted"));
            }
        }
    }

    pub fn rename_user_template(&mut self, old: &str, new: &str) -> bool {
        if old == new.trim() {
            return true;
        }
        if !self.templates.rename(old, new) {
            self.toast(
                ToastKind::Warning,
                "Template names must be unique and not empty.",
            );
            return false;
        }
        if let Err(e) = self.templates.save() {
            self.toast(ToastKind::Error, format!("Could not update templates: {e}"));
        }
        true
    }

    // ------------------------------------------------------------------ trim

    /// The render range as (in, out), the whole video when untrimmed.
    pub fn trim_bounds(&self) -> (f64, f64) {
        let dur = self.state.duration();
        self.state
            .project
            .trim
            .and_then(|r| r.normalized(dur))
            .map_or((0.0, dur), |r| (r.start, r.end))
    }

    /// Sets the render range (normalized: a range covering the whole video clears the trim).
    pub fn set_trim(&mut self, start: f64, end: f64) {
        let dur = self.state.duration();
        let t = TimeRange { start, end }.normalized(dur);
        if t != self.state.project.trim {
            self.state.execute(Command::SetTrim(t));
        }
    }

    /// Snaps a time to the video frame grid.
    pub fn snap_to_frame(&self, t: f64) -> f64 {
        let fps = self.state.fps();
        (t * fps).round() / fps
    }

    /// Sets the in point at the playhead (the out point moves if it would be before it).
    pub fn set_trim_in(&mut self) {
        let (_, out) = self.trim_bounds();
        let t = self.snap_to_frame(self.state.playhead);
        let dur = self.state.duration();
        let out = if out - t < TimeRange::MIN_LEN {
            dur
        } else {
            out
        };
        self.state.undo.break_merge();
        self.set_trim(t, out);
    }

    /// Sets the out point at the playhead (the in point moves if it would be after it).
    pub fn set_trim_out(&mut self) {
        let (tin, _) = self.trim_bounds();
        let t = self.snap_to_frame(self.state.playhead);
        let tin = if t - tin < TimeRange::MIN_LEN {
            0.0
        } else {
            tin
        };
        self.state.undo.break_merge();
        self.set_trim(tin, t);
    }

    pub fn clear_trim(&mut self) {
        if self.state.project.trim.is_some() {
            self.state.undo.break_merge();
            self.state.execute(Command::SetTrim(None));
        }
    }

    // ------------------------------------------------------------------ project files

    pub fn request(&mut self, action: PendingAction) {
        if self.state.dirty && !matches!(action, PendingAction::Quit if self.ui.close_confirmed) {
            self.ui.confirm_discard = Some(action);
        } else {
            self.perform(action);
        }
    }

    pub fn perform(&mut self, action: PendingAction) {
        match action {
            PendingAction::New => self.new_project(),
            PendingAction::Open(Some(p)) => self.open_project_path(&p),
            PendingAction::Open(None) => {
                let mut d = rfd::FileDialog::new()
                    .add_filter("TelemetryVibe project", &[pmodel::PROJECT_EXTENSION]);
                if let Some(dir) = &self.settings.last_dir {
                    d = d.set_directory(dir);
                }
                if let Some(p) = d.pick_file() {
                    self.open_project_path(&p);
                }
            }
            PendingAction::Quit => {
                self.ui.close_confirmed = true;
                self.egui_ctx
                    .send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    fn reset_media(&mut self) {
        self.pause();
        self.thumb_cancel.store(true, Ordering::Relaxed);
        self.cancel_proxy();
        self.thumbs.clear();
        self.video_tex = None;
        self.video_frame_t = None;
        self.gauge_tex.clear();
        self.decoder_cfg = None;
        self.proxy = ProxyState::None;
        self.engine.stop();
    }

    pub fn new_project(&mut self) {
        self.reset_media();
        self.state = AppState::default();
        self.ui.offset_text.clear();
    }

    pub fn load_project(&mut self, project: Project, path: Option<PathBuf>) {
        self.reset_media();
        let video = project.video.clone();
        let tel = project.telemetry.clone();
        self.state = AppState {
            project,
            project_path: path,
            ..Default::default()
        };
        // Keep the saved offset: auto sync only applies to offset 0.
        if let Some(v) = video {
            if self.ffmpeg.is_none() && self.ffmpeg_detecting {
                // Imported once FFmpeg detection finishes.
            } else if v.path.exists() {
                self.import_video(v.path);
            } else {
                self.toast(
                    ToastKind::Warning,
                    format!("Video not found: {}", v.path.display()),
                );
            }
        }
        if let Some(t) = tel {
            if t.path.exists() {
                self.import_telemetry(t.path);
            } else {
                self.toast(
                    ToastKind::Warning,
                    format!("GPS file not found: {}", t.path.display()),
                );
            }
        }
    }

    pub fn open_project_path(&mut self, path: &Path) {
        match pmodel::load(path) {
            Ok(p) => {
                self.load_project(p, Some(path.to_path_buf()));
                autosave::add_recent(&self.paths, &mut self.recent, path);
                self.remember_dir(path);
                self.toast(ToastKind::Success, format!("Opened {}", path.display()));
            }
            Err(e) => {
                tracing::error!(error = ?e, path = %path.display(), "open failed");
                self.toast(ToastKind::Error, e.to_string());
            }
        }
    }

    pub fn save(&mut self) -> bool {
        match self.state.project_path.clone() {
            Some(p) => self.save_to(&p),
            None => self.save_as(),
        }
    }

    pub fn save_as(&mut self) -> bool {
        let name = format!("{}.{}", self.state.project.name, pmodel::PROJECT_EXTENSION);
        let mut d = rfd::FileDialog::new()
            .set_file_name(name)
            .add_filter("TelemetryVibe project", &[pmodel::PROJECT_EXTENSION]);
        if let Some(dir) = self
            .state
            .video
            .as_ref()
            .and_then(|v| v.path.parent().map(Path::to_path_buf))
            .or_else(|| self.settings.last_dir.clone())
        {
            d = d.set_directory(dir);
        }
        match d.save_file() {
            Some(mut p) => {
                if p.extension().is_none() {
                    p.set_extension(pmodel::PROJECT_EXTENSION);
                }
                self.save_to(&p)
            }
            None => false,
        }
    }

    fn save_to(&mut self, path: &Path) -> bool {
        match pmodel::save(&self.state.project, path) {
            Ok(()) => {
                self.state.project_path = Some(path.to_path_buf());
                self.state.dirty = false;
                autosave::add_recent(&self.paths, &mut self.recent, path);
                autosave::discard_recovery(&self.paths);
                self.toast(ToastKind::Success, "Project saved");
                true
            }
            Err(e) => {
                self.toast(ToastKind::Error, format!("Could not save the project: {e}"));
                false
            }
        }
    }

    pub fn recover(&mut self, r: Recovery) {
        match std::fs::read_to_string(&r.file)
            .map_err(|e| e.to_string())
            .and_then(|s| pmodel::from_json(&s).map_err(|e| e.to_string()))
        {
            Ok(p) => {
                self.load_project(p, r.meta.project_path.clone());
                self.state.dirty = true;
                self.toast(ToastKind::Success, "Project recovered");
            }
            Err(e) => self.toast(ToastKind::Error, format!("Recovery failed: {e}")),
        }
    }

    pub fn window_title(&self) -> String {
        let name = self
            .state
            .project_path
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| self.state.project.name.clone());
        format!(
            "{}{} — TelemetryVibe",
            name,
            if self.state.dirty { " •" } else { "" }
        )
    }

    // ------------------------------------------------------------------ events & shortcuts

    fn process_background(&mut self) {
        while let Ok(ev) = self.bg_rx.try_recv() {
            match ev {
                BgEvent::Ffmpeg(r) => {
                    self.ffmpeg_detecting = false;
                    match r {
                        Ok(info) => {
                            self.ffmpeg_error = None;
                            self.ffmpeg = Some(Arc::new(info));
                            // A video from an opened project may be waiting for FFmpeg.
                            if self.state.video.is_none()
                                && self.loading_video.is_none()
                                && let Some(v) = self.state.project.video.clone()
                                && v.path.exists()
                            {
                                self.import_video(v.path);
                            }
                            if self.state.video.is_some() {
                                self.reconfigure_decoder(true);
                            }
                        }
                        Err(e) => {
                            self.ffmpeg = None;
                            self.ffmpeg_error = Some(e.clone());
                            self.toast(ToastKind::Error, e);
                        }
                    }
                }
                BgEvent::VideoProbed(path, r) => {
                    if self.loading_video.as_ref() == Some(&path) {
                        self.loading_video = None;
                    }
                    match r {
                        Ok(info) => self.on_video_loaded(info),
                        Err(e) => self.toast(ToastKind::Error, format!("{e} ({})", path.display())),
                    }
                }
                BgEvent::TrackLoaded(path, r) => {
                    if self.loading_track.as_ref() == Some(&path) {
                        self.loading_track = None;
                    }
                    match r {
                        Ok(t) => self.on_track_loaded(*t, path),
                        Err(e) => self.toast(ToastKind::Error, e),
                    }
                }
                BgEvent::Thumb(epoch, t) => {
                    if epoch == self.thumb_epoch {
                        let tex = self.egui_ctx.load_texture(
                            format!("thumb-{}", self.thumbs.len()),
                            (*t.image).clone(),
                            TextureOptions::LINEAR,
                        );
                        self.thumbs.push((t.t, tex));
                    }
                }
                BgEvent::ProxyProgress(p) => {
                    if let ProxyState::Generating { progress, .. } = &mut self.proxy {
                        *progress = p;
                    }
                }
                BgEvent::ProxyDone(src, r) => {
                    if self.state.video.as_ref().map(|v| &v.path) != Some(&src) {
                        continue;
                    }
                    match r {
                        Ok(path) => {
                            let fps = self
                                .state
                                .video
                                .as_ref()
                                .map(|v| proxy::proxy_params(v).2)
                                .unwrap_or(30.0);
                            self.proxy = ProxyState::Ready { path, fps };
                            self.toast(
                                ToastKind::Success,
                                "Proxy ready — preview now uses the lightweight proxy",
                            );
                            self.reconfigure_decoder(true);
                        }
                        Err(e) => {
                            self.proxy = ProxyState::None;
                            if !e.contains("cancel") {
                                self.toast(ToastKind::Warning, e);
                            }
                        }
                    }
                }
            }
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        let cmd = Modifiers::COMMAND;
        let cmd_shift = Modifiers::COMMAND | Modifiers::SHIFT;
        let sc = |m: Modifiers, k: Key| KeyboardShortcut::new(m, k);
        // Global (work even when a text field has focus).
        let mut fired: Vec<&str> = Vec::new();
        ctx.input_mut(|i| {
            if i.consume_shortcut(&sc(cmd_shift, Key::S)) {
                fired.push("save_as");
            }
            if i.consume_shortcut(&sc(cmd, Key::S)) {
                fired.push("save");
            }
            if i.consume_shortcut(&sc(cmd, Key::O)) {
                fired.push("open");
            }
            if i.consume_shortcut(&sc(cmd, Key::N)) {
                fired.push("new");
            }
            if i.consume_shortcut(&sc(cmd, Key::R)) {
                fired.push("render");
            }
            if i.consume_shortcut(&sc(cmd, Key::I)) {
                fired.push("import_video");
            }
            if i.consume_shortcut(&sc(cmd_shift, Key::I)) {
                fired.push("import_gps");
            }
        });
        let text_focus = ctx.egui_wants_keyboard_input();
        if !text_focus {
            ctx.input_mut(|i| {
                if i.consume_shortcut(&sc(cmd_shift, Key::Z)) {
                    fired.push("redo");
                }
                if i.consume_shortcut(&sc(cmd, Key::Z)) {
                    fired.push("undo");
                }
                if i.consume_shortcut(&sc(cmd, Key::D)) {
                    fired.push("duplicate");
                }
                if i.consume_shortcut(&sc(cmd, Key::A)) {
                    fired.push("select_all");
                }
                if i.consume_shortcut(&sc(cmd_shift, Key::G)) {
                    fired.push("ungroup");
                }
                if i.consume_shortcut(&sc(cmd, Key::G)) {
                    fired.push("group");
                }
                if i.consume_key(Modifiers::NONE, Key::Space) {
                    fired.push("play");
                }
                if i.consume_key(Modifiers::NONE, Key::Home) {
                    fired.push("home");
                }
                if i.consume_key(Modifiers::NONE, Key::End) {
                    fired.push("end");
                }
                if i.consume_key(Modifiers::NONE, Key::Delete)
                    || i.consume_key(Modifiers::NONE, Key::Backspace)
                {
                    fired.push("delete");
                }
                if i.consume_key(Modifiers::NONE, Key::Escape) {
                    fired.push("deselect");
                }
                if i.consume_key(Modifiers::NONE, Key::Comma) {
                    fired.push("prev_frame");
                }
                if i.consume_key(Modifiers::NONE, Key::Period) {
                    fired.push("next_frame");
                }
                if i.consume_key(Modifiers::NONE, Key::I) {
                    fired.push("trim_in");
                }
                if i.consume_key(Modifiers::NONE, Key::O) {
                    fired.push("trim_out");
                }
                if i.consume_key(Modifiers::ALT, Key::X) {
                    fired.push("trim_clear");
                }
                for (key, name, shift_name) in [
                    (Key::ArrowLeft, "left", "left10"),
                    (Key::ArrowRight, "right", "right10"),
                    (Key::ArrowUp, "up", "up10"),
                    (Key::ArrowDown, "down", "down10"),
                ] {
                    if i.consume_key(Modifiers::SHIFT, key) {
                        fired.push(shift_name);
                    } else if i.consume_key(Modifiers::NONE, key) {
                        fired.push(name);
                    }
                }
            });
        }
        let has_sel = !self.state.selection.is_empty();
        for f in fired {
            match f {
                "save" => {
                    self.save();
                }
                "save_as" => {
                    self.save_as();
                }
                "open" => self.request(PendingAction::Open(None)),
                "new" => self.request(PendingAction::New),
                "render" => self.ui.show_render = true,
                "import_video" => self.import_dialog_video(),
                "import_gps" => self.import_dialog_gps(),
                "undo" => {
                    self.state.undo();
                }
                "redo" => {
                    self.state.redo();
                }
                "duplicate" => {
                    self.state.duplicate_selection();
                }
                "select_all" => {
                    self.state.selection = self.state.project.gauges.iter().map(|g| g.id).collect()
                }
                "group" => ui::inspector::group_selection(self),
                "ungroup" => ui::inspector::ungroup_selection(self),
                "play" => self.toggle_play(),
                "home" => self.set_playhead(0.0),
                "end" => {
                    let d = self.state.duration();
                    self.set_playhead(d);
                }
                "delete" => self.state.delete_selection(),
                "deselect" => self.state.selection.clear(),
                "prev_frame" => self.step_frames(-1),
                "next_frame" => self.step_frames(1),
                "trim_in" => self.set_trim_in(),
                "trim_out" => self.set_trim_out(),
                "trim_clear" => self.clear_trim(),
                // Arrows nudge the selected gauges; otherwise they step frames.
                "left" if has_sel => self.state.nudge_selection(-1.0, 0.0),
                "right" if has_sel => self.state.nudge_selection(1.0, 0.0),
                "up" if has_sel => self.state.nudge_selection(0.0, -1.0),
                "down" if has_sel => self.state.nudge_selection(0.0, 1.0),
                "left10" if has_sel => self.state.nudge_selection(-10.0, 0.0),
                "right10" if has_sel => self.state.nudge_selection(10.0, 0.0),
                "up10" if has_sel => self.state.nudge_selection(0.0, -10.0),
                "down10" if has_sel => self.state.nudge_selection(0.0, 10.0),
                "left" => self.step_frames(-1),
                "right" => self.step_frames(1),
                "left10" => self.step_frames(-10),
                "right10" => self.step_frames(10),
                _ => {}
            }
        }
    }

    fn handle_drops(&mut self, ctx: &egui::Context) {
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .collect()
        });
        // Telemetry after video so auto sync sees both.
        let (videos, others): (Vec<PathBuf>, Vec<PathBuf>) =
            dropped.into_iter().partition(|p| probe::is_video_file(p));
        for p in videos.into_iter().chain(others) {
            self.handle_dropped_file(p);
        }
    }

    fn autosave_tick(&mut self) {
        if self.state.autosaver.due() {
            let path = self.state.project_path.clone();
            self.state
                .autosaver
                .save(&self.paths, &self.state.project, path.as_deref());
        }
    }

    fn handle_close_request(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.viewport().close_requested())
            && self.state.dirty
            && !self.ui.close_confirmed
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.ui.confirm_discard = Some(PendingAction::Quit);
        }
    }
}

impl eframe::App for GaugeApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.theme_applied != Some(self.settings.theme) {
            ui::theme::apply(ctx, self.settings.theme);
            self.theme_applied = Some(self.settings.theme);
        }
        self.process_background();
        self.handle_drops(ctx);
        self.handle_close_request(ctx);
        self.update_playback();
        self.autosave_tick();
        if self.playing
            || self.queue.is_running()
            || matches!(self.proxy, ProxyState::Generating { .. })
        {
            ctx.request_repaint();
        }
        let title = self.window_title();
        if title != self.last_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.last_title = title;
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.handle_shortcuts(&ctx);
        ui::layout(self, ui);
        super::devscript::DevScript::tick(self, &ctx);
    }

    fn on_exit(&mut self) {
        self.queue.cancel_all();
        self.thumb_cancel.store(true, Ordering::Relaxed);
        self.cancel_proxy();
        autosave::end_session(&self.paths);
        self.settings.save(&self.paths);
        tracing::info!("clean exit");
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        ui::theme::canvas_bg(self.settings.theme).to_normalized_gamma_f32()
    }
}
