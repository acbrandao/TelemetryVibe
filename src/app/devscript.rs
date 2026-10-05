//! Developer automation for visual checks (not used in normal operation).
//!
//! `TELEMETRYVIBE_DEV_SCRIPT="wait=3;template=Cycling;trim=5,20;playhead=20;select=1;shot=/tmp/a.png;quit"`
//! Steps run in order; `wait=<s>` pauses, `shot=<png>` saves a window screenshot.

use std::time::{Duration, Instant};

use super::application::{GaugeApp, RightTab};
use crate::gauges::library::Template;

pub struct DevScript {
    steps: Vec<(String, String)>,
    next_at: Instant,
    waiting_shot: Option<String>,
}

impl DevScript {
    pub fn from_env() -> Option<Self> {
        let s = std::env::var("TELEMETRYVIBE_DEV_SCRIPT").ok()?;
        let steps = s
            .split(';')
            .filter(|p| !p.trim().is_empty())
            .map(|p| {
                let mut it = p.splitn(2, '=');
                (
                    it.next().unwrap_or("").trim().to_string(),
                    it.next().unwrap_or("").trim().to_string(),
                )
            })
            .collect();
        Some(Self {
            steps,
            next_at: Instant::now() + Duration::from_secs(1),
            waiting_shot: None,
        })
    }

    pub fn tick(app: &mut GaugeApp, ctx: &egui::Context) {
        let Some(mut ds) = app.dev_script.take() else {
            return;
        };
        ctx.request_repaint_after(Duration::from_millis(50));
        // Collect a pending screenshot.
        if let Some(path) = ds.waiting_shot.clone() {
            let img = ctx.input(|i| {
                i.raw.events.iter().find_map(|e| match e {
                    egui::Event::Screenshot { image, .. } => Some(image.clone()),
                    _ => None,
                })
            });
            if let Some(img) = img {
                save_png(&img, &path);
                ds.waiting_shot = None;
                ds.next_at = Instant::now() + Duration::from_millis(200);
            }
            app.dev_script = Some(ds);
            return;
        }
        if Instant::now() < ds.next_at || ds.steps.is_empty() {
            app.dev_script = Some(ds);
            return;
        }
        let (k, v) = ds.steps.remove(0);
        let mut delay = 0.4;
        match k.as_str() {
            "wait" => delay = v.parse().unwrap_or(1.0),
            "template" => {
                if let Some(t) = Template::ALL
                    .into_iter()
                    .find(|t| t.name().eq_ignore_ascii_case(&v))
                {
                    app.apply_template(t);
                }
            }
            "theme" => {
                if let Some(t) = crate::project::autosave::Theme::ALL.into_iter().find(|t| {
                    t.label()
                        .replace(' ', "")
                        .eq_ignore_ascii_case(&v.replace(' ', ""))
                }) {
                    app.settings.theme = t;
                }
            }
            "clear_gauges" => app.clear_gauges(),
            "import_gps" => app.import_telemetry(std::path::PathBuf::from(&v)),
            "user_template" => app.apply_user_template(&v),
            "save_template" => app.save_current_template(),
            "trim" => {
                // trim=<in>,<out> in seconds; trim=0 clears.
                let parts: Vec<f64> = v.split(',').filter_map(|p| p.trim().parse().ok()).collect();
                match parts.as_slice() {
                    [a, b] => app.set_trim(*a, *b),
                    _ => app.clear_trim(),
                }
            }
            "playhead" => app.set_playhead(v.parse().unwrap_or(0.0)),
            "play" => app.play(),
            "pause" => app.pause(),
            "select" => {
                let i: usize = v.parse().unwrap_or(1);
                if let Some(g) = app.state.project.gauges.get(i.saturating_sub(1)) {
                    let id = g.id;
                    app.state.select(id, false);
                    app.ui.right_tab = RightTab::Inspector;
                }
            }
            "deselect" => app.state.selection.clear(),
            "library" => app.ui.right_tab = RightTab::Library,
            "sync" => app.ui.show_sync = v != "0",
            "pick_gps" => app.ui.sync.picked_gps = v.parse().ok(),
            "mark_video" => {
                let gps = app.state.project.sync.gps_mark;
                app.state.execute(super::commands::Command::SetSyncMarks {
                    video: Some(app.state.playhead),
                    gps,
                });
            }
            "render" => app.ui.show_render = v != "0",
            "queue" => app.ui.show_queue = v != "0",
            "units" => {
                let u = if v.eq_ignore_ascii_case("imperial") {
                    crate::telemetry::UnitSystem::Imperial
                } else {
                    crate::telemetry::UnitSystem::Metric
                };
                app.state.execute(super::commands::Command::SetUnits(u));
            }
            "render_now" => {
                if let (Some(info), Some(ff)) = (app.state.video.clone(), app.ffmpeg.clone()) {
                    let job =
                        crate::render::export::RenderJob::new(crate::render::export::ExportSpec {
                            output: std::path::PathBuf::from(&v),
                            info,
                            settings: app.state.project.render.clone(),
                            gauges: app.state.project.gauges.clone(),
                            track: app.state.track.clone(),
                            sync: app.state.project.sync.clone(),
                            units: app.state.project.units,
                            ffmpeg: ff,
                            range: app.state.project.trim,
                        });
                    app.queue.add(job);
                    app.queue.start();
                    app.ui.show_queue = true;
                }
            }
            "await_render" => {
                if app.queue.is_running() {
                    ds.steps.insert(0, (k.clone(), v.clone()));
                    delay = 0.25;
                }
            }
            "shot" => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
                ds.waiting_shot = Some(v);
            }
            "quit" => {
                app.state.dirty = false;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            other => tracing::warn!("unknown dev step {other}"),
        }
        ds.next_at = Instant::now() + Duration::from_secs_f64(delay);
        app.dev_script = Some(ds);
    }
}

fn save_png(img: &egui::ColorImage, path: &str) {
    let [w, h] = img.size;
    let Some(mut pm) = tiny_skia::Pixmap::new(w as u32, h as u32) else {
        return;
    };
    for (dst, c) in pm.pixels_mut().iter_mut().zip(img.pixels.iter()) {
        *dst = tiny_skia::ColorU8::from_rgba(c.r(), c.g(), c.b(), 255).premultiply();
    }
    if let Err(e) = pm.save_png(path) {
        tracing::warn!("screenshot failed: {e}");
    } else {
        tracing::info!("screenshot saved to {path}");
    }
}
