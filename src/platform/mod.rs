//! The few platform-specific bits: revealing files in the OS file manager and the app icon.
//! Everything else in the application is cross-platform.

use std::path::Path;
use std::process::Command;

/// Opens the OS file manager with `path` selected (or opens the folder).
pub fn reveal_in_file_manager(path: &Path) {
    let result = if cfg!(target_os = "macos") {
        if path.is_file() {
            Command::new("open").arg("-R").arg(path).spawn()
        } else {
            Command::new("open").arg(path).spawn()
        }
    } else if cfg!(target_os = "windows") {
        if path.is_file() {
            Command::new("explorer")
                .arg(format!("/select,{}", path.display()))
                .spawn()
        } else {
            Command::new("explorer").arg(path).spawn()
        }
    } else {
        let dir = if path.is_file() {
            path.parent().unwrap_or(path)
        } else {
            path
        };
        Command::new("xdg-open").arg(dir).spawn()
    };
    if let Err(e) = result {
        tracing::warn!("could not open file manager: {e}");
    }
}

/// Renders the application icon (a stylized speedometer) as straight-alpha RGBA.
pub fn icon_rgba(size: u32) -> Vec<u8> {
    use crate::gauges::model::Rgba;
    use crate::gauges::scene::{self, Cap, Scene};
    let s = size as f32;
    let mut sc = Scene::new();
    let r = s * 0.5;
    let (cx, cy) = (s / 2.0, s / 2.0);
    sc.fill(
        scene::rounded_rect(s * 0.04, s * 0.04, s * 0.92, s * 0.92, s * 0.22),
        Rgba::rgb(22, 24, 30),
    );
    sc.stroke(
        scene::arc(cx, cy + s * 0.04, r * 0.62, 150.0, 390.0),
        Rgba::rgba(255, 255, 255, 60),
        s * 0.07,
        Cap::Round,
    );
    sc.stroke(
        scene::arc(cx, cy + s * 0.04, r * 0.62, 150.0, 320.0),
        Rgba::rgb(255, 106, 61),
        s * 0.07,
        Cap::Round,
    );
    let (tx, ty) = scene::polar(cx, cy + s * 0.04, r * 0.5, 305.0);
    sc.stroke(
        scene::line(cx, cy + s * 0.04, tx, ty),
        Rgba::WHITE,
        s * 0.045,
        Cap::Round,
    );
    sc.fill(scene::circle(cx, cy + s * 0.04, s * 0.06), Rgba::WHITE);
    // GPS route squiggle under the dial.
    let pts: Vec<(f32, f32)> = (0..=20)
        .map(|i| {
            let f = i as f32 / 20.0;
            (s * (0.28 + 0.44 * f), s * 0.8 + (f * 9.0).sin() * s * 0.03)
        })
        .collect();
    sc.stroke(
        scene::polyline(&pts),
        Rgba::rgb(80, 200, 255),
        s * 0.03,
        Cap::Round,
    );
    let mut pm = match tiny_skia::Pixmap::new(size, size) {
        Some(p) => p,
        None => return vec![0; (size * size * 4) as usize],
    };
    crate::render::raster::draw_scene(&mut pm.as_mut(), &sc, tiny_skia::Transform::identity(), 1.0);
    let mut data = pm.take();
    crate::render::raster::demultiply_in_place(&mut data);
    data
}

/// Writes icon PNGs (used by the macOS bundle script to build an `.icns`).
pub fn write_icon_pngs(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    for size in [16u32, 32, 64, 128, 256, 512, 1024] {
        let rgba = icon_rgba(size);
        let mut pm = tiny_skia::Pixmap::new(size, size).ok_or(std::io::ErrorKind::InvalidInput)?;
        // Re-premultiply for tiny-skia's PNG encoder.
        #[allow(clippy::chunks_exact_to_as_chunks)]
        for (dst, src) in pm.pixels_mut().iter_mut().zip(rgba.chunks_exact(4)) {
            *dst = tiny_skia::ColorU8::from_rgba(src[0], src[1], src[2], src[3]).premultiply();
        }
        pm.save_png(dir.join(format!("icon_{size}.png")))
            .map_err(|e| std::io::Error::other(e.to_string()))?;
    }
    Ok(())
}
