//! TelemetryVibe — GPS telemetry gauge overlays for video.
//!
//! Module map:
//! * [`telemetry`] — FIT/GPX/TCX parsing, normalized model, interpolation, smoothing, sync.
//! * [`gauges`] — gauge definitions, per-kind builders producing vector scenes, library.
//! * [`render`] — shared rasterizer (preview + export), compositor, FFmpeg render jobs.
//! * [`video`] — FFmpeg discovery, probing, preview decoder, proxies, encoder arguments.
//! * [`project`] — `.gpsvideo` documents, autosave and recovery.
//! * [`app`] / [`ui`] — application state, commands/undo and the egui interface.

pub mod app;
pub mod gauges;
pub mod platform;
pub mod project;
pub mod render;
pub mod sample;
pub mod telemetry;
pub mod ui;
pub mod utils;
pub mod video;
