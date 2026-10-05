//! Rendering: the shared vector rasterizer, text, overlay composition and FFmpeg export.
//!
//! Preview and final render use the same gauge scenes and rasterizer ([`raster::draw_scene`]),
//! so the exported video matches what the editor shows.

pub mod compositor;
pub mod export;
pub mod raster;
pub mod text;
