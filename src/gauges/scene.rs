//! Retained vector scene produced by gauge builders.
//!
//! Coordinates are gauge-local: `(0,0)` is the top-left of the gauge box and units are source
//! video pixels. Angles are degrees, clockwise from +x (screen convention).

use std::sync::Arc;

use tiny_skia::{Path, PathBuilder};

use super::model::{FontWeight, Rgba};

#[derive(Clone, Debug)]
pub enum Paint {
    Solid(Rgba),
    Linear {
        start: (f32, f32),
        end: (f32, f32),
        stops: Vec<(f32, Rgba)>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cap {
    Butt,
    Round,
    Square,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HAlign {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VAlign {
    /// `y` is the baseline.
    Baseline,
    /// `y` is the vertical middle of capital letters.
    Middle,
    /// `y` is the cap-height top.
    Top,
}

#[derive(Clone, Debug)]
pub struct TextPrim {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub size: f32,
    pub weight: FontWeight,
    pub h_align: HAlign,
    pub v_align: VAlign,
    pub color: Rgba,
    /// Equal-width digits so changing numbers do not jitter.
    pub tabular: bool,
    /// Rotation in degrees around (x, y).
    pub rotation: f32,
}

#[derive(Clone, Debug)]
pub enum Prim {
    Fill {
        path: Path,
        paint: Paint,
    },
    Stroke {
        path: Path,
        paint: Paint,
        width: f32,
        cap: Cap,
    },
    Text(TextPrim),
    Image {
        image: Arc<tiny_skia::Pixmap>,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    },
}

/// The renderable output of a gauge builder.
#[derive(Clone, Debug, Default)]
pub struct Scene {
    pub prims: Vec<Prim>,
    /// Drop shadow offset in local units (`None` = no shadow).
    pub shadow: Option<f32>,
}

impl Scene {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn fill(&mut self, path: Option<Path>, color: Rgba) {
        if let Some(path) = path
            && color.0[3] > 0
        {
            self.prims.push(Prim::Fill {
                path,
                paint: Paint::Solid(color),
            });
        }
    }

    pub fn fill_paint(&mut self, path: Option<Path>, paint: Paint) {
        if let Some(path) = path {
            self.prims.push(Prim::Fill { path, paint });
        }
    }

    pub fn stroke(&mut self, path: Option<Path>, color: Rgba, width: f32, cap: Cap) {
        if let Some(path) = path
            && color.0[3] > 0
            && width > 0.0
        {
            self.prims.push(Prim::Stroke {
                path,
                paint: Paint::Solid(color),
                width,
                cap,
            });
        }
    }

    /// Stroke with a soft glow underneath (`amount` 0..=1).
    pub fn glow_stroke(
        &mut self,
        path: Option<Path>,
        color: Rgba,
        width: f32,
        cap: Cap,
        amount: f32,
    ) {
        let Some(path) = path else { return };
        if amount > 0.01 {
            for (k, alpha) in [(4.0, 0.10), (2.6, 0.16), (1.8, 0.24)] {
                self.stroke(
                    Some(path.clone()),
                    color.with_alpha_mul(alpha * amount),
                    width * k,
                    cap,
                );
            }
        }
        self.stroke(Some(path), color, width, cap);
    }

    /// Filled shape with a glow halo.
    pub fn glow_fill(&mut self, path: Option<Path>, color: Rgba, amount: f32, halo: f32) {
        let Some(path) = path else { return };
        if amount > 0.01 {
            for (k, alpha) in [(3.0, 0.10), (2.0, 0.16), (1.0, 0.24)] {
                self.stroke(
                    Some(path.clone()),
                    color.with_alpha_mul(alpha * amount),
                    halo * k,
                    Cap::Round,
                );
            }
        }
        self.fill(Some(path), color);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn text(
        &mut self,
        text: impl Into<String>,
        x: f32,
        y: f32,
        size: f32,
        weight: FontWeight,
        h_align: HAlign,
        v_align: VAlign,
        color: Rgba,
    ) {
        let text = text.into();
        if text.is_empty() || size <= 0.5 || color.0[3] == 0 {
            return;
        }
        self.prims.push(Prim::Text(TextPrim {
            text,
            x,
            y,
            size,
            weight,
            h_align,
            v_align,
            color,
            tabular: true,
            rotation: 0.0,
        }));
    }

    pub fn push_text(&mut self, t: TextPrim) {
        if !t.text.is_empty() && t.size > 0.5 {
            self.prims.push(Prim::Text(t));
        }
    }
}

/// Converts an angle in degrees to a point on a circle.
#[inline]
pub fn polar(cx: f32, cy: f32, r: f32, deg: f32) -> (f32, f32) {
    let a = deg.to_radians();
    (cx + r * a.cos(), cy + r * a.sin())
}

pub fn rect(x: f32, y: f32, w: f32, h: f32) -> Option<Path> {
    let r = tiny_skia::Rect::from_xywh(x, y, w.max(0.01), h.max(0.01))?;
    Some(PathBuilder::from_rect(r))
}

pub fn rounded_rect(x: f32, y: f32, w: f32, h: f32, radius: f32) -> Option<Path> {
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    let r = radius.clamp(0.0, w.min(h) / 2.0);
    if r < 0.01 {
        return rect(x, y, w, h);
    }
    // Cubic approximation of quarter circles.
    let k = 0.552_284_8 * r;
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.cubic_to(x + w - r + k, y, x + w, y + r - k, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.cubic_to(x + w, y + h - r + k, x + w - r + k, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.cubic_to(x + r - k, y + h, x, y + h - r + k, x, y + h - r);
    pb.line_to(x, y + r);
    pb.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    pb.close();
    pb.finish()
}

pub fn circle(cx: f32, cy: f32, r: f32) -> Option<Path> {
    PathBuilder::from_circle(cx, cy, r.max(0.01))
}

/// Open arc polyline from `a0` to `a1` degrees.
pub fn arc(cx: f32, cy: f32, r: f32, a0: f32, a1: f32) -> Option<Path> {
    let span = a1 - a0;
    if span.abs() < 0.01 {
        return None;
    }
    let steps = ((span.abs() / 3.0).ceil() as usize).clamp(2, 240);
    let mut pb = PathBuilder::new();
    for i in 0..=steps {
        let a = a0 + span * i as f32 / steps as f32;
        let (x, y) = polar(cx, cy, r, a);
        if i == 0 {
            pb.move_to(x, y);
        } else {
            pb.line_to(x, y);
        }
    }
    pb.finish()
}

/// Closed annular sector between radii `r0 < r1`.
pub fn annular_sector(cx: f32, cy: f32, r0: f32, r1: f32, a0: f32, a1: f32) -> Option<Path> {
    let span = a1 - a0;
    if span.abs() < 0.01 {
        return None;
    }
    let steps = ((span.abs() / 3.0).ceil() as usize).clamp(2, 240);
    let mut pb = PathBuilder::new();
    for i in 0..=steps {
        let a = a0 + span * i as f32 / steps as f32;
        let (x, y) = polar(cx, cy, r1, a);
        if i == 0 {
            pb.move_to(x, y);
        } else {
            pb.line_to(x, y);
        }
    }
    for i in (0..=steps).rev() {
        let a = a0 + span * i as f32 / steps as f32;
        let (x, y) = polar(cx, cy, r0, a);
        pb.line_to(x, y);
    }
    pb.close();
    pb.finish()
}

pub fn line(x0: f32, y0: f32, x1: f32, y1: f32) -> Option<Path> {
    let mut pb = PathBuilder::new();
    pb.move_to(x0, y0);
    pb.line_to(x1, y1);
    pb.finish()
}

pub fn polygon(points: &[(f32, f32)]) -> Option<Path> {
    let mut pb = PathBuilder::new();
    for (i, &(x, y)) in points.iter().enumerate() {
        if i == 0 {
            pb.move_to(x, y);
        } else {
            pb.line_to(x, y);
        }
    }
    pb.close();
    pb.finish()
}

pub fn polyline(points: &[(f32, f32)]) -> Option<Path> {
    if points.len() < 2 {
        return None;
    }
    let mut pb = PathBuilder::new();
    for (i, &(x, y)) in points.iter().enumerate() {
        if i == 0 {
            pb.move_to(x, y);
        } else {
            pb.line_to(x, y);
        }
    }
    pb.finish()
}
