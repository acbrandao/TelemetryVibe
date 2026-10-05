//! Vector text: glyph outlines from the bundled Inter variable font converted to paths.
//!
//! Text is rendered as filled outlines through the same rasterizer as every other primitive,
//! which keeps preview and export identical and lets text rotate/scale freely.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use ab_glyph::{Font, FontVec, GlyphId, OutlineCurve, VariableFont};
use parking_lot::RwLock;
use tiny_skia::{Path, PathBuilder};

use crate::gauges::model::FontWeight;

static INTER: &[u8] = include_bytes!("../../assets/fonts/Inter.ttf");

type GlyphCache = RwLock<HashMap<(usize, GlyphId), Option<Arc<Path>>>>;

pub struct Fonts {
    faces: Vec<FontVec>,
    glyphs: GlyphCache,
    units_per_em: f32,
    cap_height: f32,
    digit_advance: Vec<f32>,
}

static FONTS: OnceLock<Fonts> = OnceLock::new();

/// Global font set (loaded on first use).
pub fn fonts() -> &'static Fonts {
    FONTS.get_or_init(Fonts::load)
}

fn weight_index(w: FontWeight) -> usize {
    match w {
        FontWeight::Regular => 0,
        FontWeight::Medium => 1,
        FontWeight::Bold => 2,
        FontWeight::Black => 3,
    }
}

/// Measured layout of a string.
pub struct Layout {
    /// (glyph, x offset in font units)
    pub glyphs: Vec<(GlyphId, f32)>,
    /// Advance width in font units.
    pub width: f32,
}

impl Fonts {
    fn load() -> Self {
        let mut faces = Vec::new();
        for wght in [400.0, 500.0, 700.0, 900.0] {
            match FontVec::try_from_vec(INTER.to_vec()) {
                Ok(mut f) => {
                    f.set_variation(b"wght", wght);
                    faces.push(f);
                }
                Err(e) => tracing::error!("failed to load bundled font: {e}"),
            }
        }
        if faces.is_empty() {
            // Fall back to egui's bundled font so text never panics.
            if let Ok(f) = FontVec::try_from_vec(
                egui::FontDefinitions::default()
                    .font_data
                    .values()
                    .next()
                    .map(|d| d.font.to_vec())
                    .unwrap_or_default(),
            ) {
                faces.push(f);
            }
        }
        let units_per_em = faces
            .first()
            .and_then(|f| f.units_per_em())
            .unwrap_or(2048.0);
        // Cap height from the outline of 'H'.
        let cap_height = faces
            .first()
            .and_then(|f| f.outline(f.glyph_id('H')))
            .map(|o| o.bounds.max.y.max(-o.bounds.min.y).abs())
            .filter(|h| *h > 0.0)
            .unwrap_or(units_per_em * 0.727);
        let digit_advance = faces
            .iter()
            .map(|f| {
                ('0'..='9')
                    .map(|c| f.h_advance_unscaled(f.glyph_id(c)))
                    .fold(0.0, f32::max)
            })
            .collect();
        Self {
            faces,
            glyphs: RwLock::new(HashMap::new()),
            units_per_em,
            cap_height,
            digit_advance,
        }
    }

    fn face_index(&self, w: FontWeight) -> usize {
        weight_index(w).min(self.faces.len().saturating_sub(1))
    }

    pub fn units_per_em(&self) -> f32 {
        self.units_per_em
    }

    /// Cap height as a fraction of the em size.
    pub fn cap_height_em(&self) -> f32 {
        self.cap_height / self.units_per_em
    }

    pub fn layout(&self, text: &str, weight: FontWeight, tabular: bool) -> Layout {
        let fi = self.face_index(weight);
        let Some(face) = self.faces.get(fi) else {
            return Layout {
                glyphs: Vec::new(),
                width: 0.0,
            };
        };
        let digit_adv = self.digit_advance.get(fi).copied().unwrap_or(0.0);
        let mut glyphs = Vec::with_capacity(text.len());
        let mut x = 0.0;
        let mut prev: Option<GlyphId> = None;
        for ch in text.chars() {
            let id = face.glyph_id(ch);
            let adv = face.h_advance_unscaled(id);
            if tabular && ch.is_ascii_digit() {
                glyphs.push((id, x + (digit_adv - adv) / 2.0));
                x += digit_adv;
                prev = None;
            } else {
                if let Some(p) = prev {
                    x += face.kern_unscaled(p, id);
                }
                glyphs.push((id, x));
                x += adv;
                prev = Some(id);
            }
        }
        Layout { glyphs, width: x }
    }

    /// Width of `text` at pixel `size`.
    pub fn measure(&self, text: &str, size: f32, weight: FontWeight, tabular: bool) -> f32 {
        self.layout(text, weight, tabular).width * size / self.units_per_em
    }

    /// Outline path of a glyph in font units (y up), cached.
    pub fn glyph_path(&self, weight: FontWeight, id: GlyphId) -> Option<Arc<Path>> {
        let fi = self.face_index(weight);
        if let Some(p) = self.glyphs.read().get(&(fi, id)) {
            return p.clone();
        }
        let path = self
            .faces
            .get(fi)
            .and_then(|f| f.outline(id))
            .and_then(|o| outline_to_path(&o.curves))
            .map(Arc::new);
        self.glyphs.write().insert((fi, id), path.clone());
        path
    }
}

fn outline_to_path(curves: &[OutlineCurve]) -> Option<Path> {
    let mut pb = PathBuilder::new();
    let mut last: Option<ab_glyph::Point> = None;
    let near = |a: ab_glyph::Point, b: ab_glyph::Point| {
        (a.x - b.x).abs() < 0.01 && (a.y - b.y).abs() < 0.01
    };
    for c in curves {
        let start = match c {
            OutlineCurve::Line(a, _)
            | OutlineCurve::Quad(a, _, _)
            | OutlineCurve::Cubic(a, _, _, _) => *a,
        };
        if last.is_none_or(|l| !near(l, start)) {
            if last.is_some() {
                pb.close();
            }
            pb.move_to(start.x, start.y);
        }
        match c {
            OutlineCurve::Line(_, b) => {
                pb.line_to(b.x, b.y);
                last = Some(*b);
            }
            OutlineCurve::Quad(_, b, c2) => {
                pb.quad_to(b.x, b.y, c2.x, c2.y);
                last = Some(*c2);
            }
            OutlineCurve::Cubic(_, b, c2, d) => {
                pb.cubic_to(b.x, b.y, c2.x, c2.y, d.x, d.y);
                last = Some(*d);
            }
        }
    }
    if last.is_some() {
        pb.close();
    }
    pb.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_loads_and_measures() {
        let f = fonts();
        assert!(f.units_per_em() > 0.0);
        let cap = f.cap_height_em();
        assert!(cap > 0.6 && cap < 0.8, "{cap}");
        let w1 = f.measure("100", 40.0, FontWeight::Bold, true);
        let w2 = f.measure("111", 40.0, FontWeight::Bold, true);
        assert!(
            (w1 - w2).abs() < 1e-3,
            "tabular digits must have equal width"
        );
        let bold = f.measure("Speed", 40.0, FontWeight::Black, false);
        let reg = f.measure("Speed", 40.0, FontWeight::Regular, false);
        assert!(bold > reg, "variable weight applied");
        assert!(
            f.glyph_path(
                FontWeight::Bold,
                f.layout("A", FontWeight::Bold, false).glyphs[0].0
            )
            .is_some()
        );
    }
}
