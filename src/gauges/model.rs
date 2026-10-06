//! Gauge definitions: the serializable description of every overlay element.
//!
//! A [`Gauge`] is pure data. Rendering is done by the per-kind builders in the sibling modules,
//! which turn a gauge plus a telemetry instant into a vector [`Scene`](super::scene::Scene).
//! The same definitions drive the interactive preview and the final export.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::telemetry::{Metric, UnitPref};

/// Stable gauge identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct GaugeId(pub u64);

/// Straight-alpha sRGB color, serialized as `#RRGGBBAA`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rgba(pub [u8; 4]);

impl Rgba {
    pub const WHITE: Rgba = Rgba([255, 255, 255, 255]);
    pub const BLACK: Rgba = Rgba([0, 0, 0, 255]);
    pub const TRANSPARENT: Rgba = Rgba([0, 0, 0, 0]);

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Rgba([r, g, b, 255])
    }

    #[allow(clippy::self_named_constructors)]
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Rgba([r, g, b, a])
    }

    /// Multiplies alpha by `f` (0..=1).
    pub fn with_alpha_mul(self, f: f32) -> Self {
        let mut c = self;
        c.0[3] = (self.0[3] as f32 * f.clamp(0.0, 1.0)).round() as u8;
        c
    }

    pub fn with_alpha(self, a: u8) -> Self {
        Rgba([self.0[0], self.0[1], self.0[2], a])
    }

    pub fn to_hex(self) -> String {
        let [r, g, b, a] = self.0;
        format!("#{r:02X}{g:02X}{b:02X}{a:02X}")
    }

    pub fn from_hex(s: &str) -> Option<Self> {
        let h = s.trim().trim_start_matches('#');
        let p = |i: usize| u8::from_str_radix(h.get(i..i + 2)?, 16).ok();
        match h.len() {
            6 => Some(Rgba([p(0)?, p(2)?, p(4)?, 255])),
            8 => Some(Rgba([p(0)?, p(2)?, p(4)?, p(6)?])),
            _ => None,
        }
    }
}

impl Serialize for Rgba {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for Rgba {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Rgba::from_hex(&s).ok_or_else(|| serde::de::Error::custom(format!("invalid color {s}")))
    }
}

/// Placement in source-video pixel coordinates. Rotation (degrees) is about the center.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Placement {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    #[serde(default)]
    pub rotation: f32,
}

impl Placement {
    pub fn center(&self) -> (f32, f32) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    /// Axis-aligned bounds of the rotated box.
    pub fn bounds(&self) -> (f32, f32, f32, f32) {
        let (cx, cy) = self.center();
        let r = self.rotation.to_radians();
        let (s, c) = (r.sin().abs(), r.cos().abs());
        let bw = self.w * c + self.h * s;
        let bh = self.w * s + self.h * c;
        (cx - bw / 2.0, cy - bh / 2.0, bw, bh)
    }

    /// Whether a point (video px) lies inside the rotated box.
    pub fn contains(&self, px: f32, py: f32) -> bool {
        let (lx, ly) = self.to_local(px, py);
        (0.0..=self.w).contains(&lx) && (0.0..=self.h).contains(&ly)
    }

    /// Converts a video-space point to local (unrotated, origin top-left) coordinates.
    pub fn to_local(&self, px: f32, py: f32) -> (f32, f32) {
        let (cx, cy) = self.center();
        let r = -self.rotation.to_radians();
        let (dx, dy) = (px - cx, py - cy);
        let lx = dx * r.cos() - dy * r.sin();
        let ly = dx * r.sin() + dy * r.cos();
        (lx + self.w / 2.0, ly + self.h / 2.0)
    }

    /// Converts a local point to video space.
    pub fn to_world(&self, lx: f32, ly: f32) -> (f32, f32) {
        let (cx, cy) = self.center();
        let r = self.rotation.to_radians();
        let (dx, dy) = (lx - self.w / 2.0, ly - self.h / 2.0);
        (
            cx + dx * r.cos() - dy * r.sin(),
            cy + dx * r.sin() + dy * r.cos(),
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FontWeight {
    Regular,
    Medium,
    #[default]
    Bold,
    Black,
}

impl FontWeight {
    pub const ALL: [FontWeight; 4] = [
        FontWeight::Regular,
        FontWeight::Medium,
        FontWeight::Bold,
        FontWeight::Black,
    ];
    pub fn label(self) -> &'static str {
        match self {
            FontWeight::Regular => "Regular",
            FontWeight::Medium => "Medium",
            FontWeight::Bold => "Bold",
            FontWeight::Black => "Black",
        }
    }
}

/// Shared appearance options. Not every kind uses every property.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Style {
    /// Needle, numbers, traveled route, graph line.
    pub primary: Rgba,
    /// Ticks, scale labels, untraveled route, bar track.
    pub secondary: Rgba,
    /// Highlights and red-line.
    pub accent: Rgba,
    pub text: Rgba,
    pub background: Rgba,
    pub background_opacity: f32,
    pub border: Rgba,
    pub border_width: f32,
    /// Fraction of the smaller side (0 = square corners, 0.5 = pill).
    pub corner_radius: f32,
    pub font_scale: f32,
    pub font_weight: FontWeight,
    pub shadow: bool,
    /// Glow amount 0..=1.
    pub glow: f32,
    pub decimals: u8,
    pub show_label: bool,
    /// Custom label text; empty uses the metric name.
    pub label: String,
    pub show_units: bool,
    pub prefix: String,
    pub suffix: String,
    pub show_ticks: bool,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            primary: Rgba::WHITE,
            secondary: Rgba::rgba(255, 255, 255, 150),
            accent: Rgba::rgb(255, 82, 48),
            text: Rgba::WHITE,
            background: Rgba::rgb(12, 14, 18),
            background_opacity: 0.55,
            border: Rgba::rgba(255, 255, 255, 40),
            border_width: 0.0,
            corner_radius: 0.18,
            font_scale: 1.0,
            font_weight: FontWeight::Bold,
            shadow: true,
            glow: 0.0,
            decimals: 0,
            show_label: true,
            label: String::new(),
            show_units: true,
            prefix: String::new(),
            suffix: String::new(),
            show_ticks: true,
        }
    }
}

/// A colored value band. Zones start at `from` (canonical units) and end at the next zone.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Zone {
    pub from: f64,
    pub color: Rgba,
    #[serde(default)]
    pub label: String,
}

/// Which visual parts zones recolor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ZoneTargets {
    pub arc: bool,
    pub needle: bool,
    pub number: bool,
    pub bar: bool,
    pub background: bool,
}

impl Default for ZoneTargets {
    fn default() -> Self {
        Self {
            arc: true,
            needle: false,
            number: false,
            bar: true,
            background: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DialStyle {
    #[default]
    Automotive,
    Motorsport,
    Aviation,
    Minimal,
    ModernDigital,
    Sport,
    /// Wide color-coded zone segments with a pointer and the current zone name.
    ZoneArc,
    /// A ring of LED dots colored by zone, lit up to the value.
    LedRing,
    /// Thick zone-colored value arc with a 10-second peak-hold marker.
    PeakArc,
    /// Rotating compass card with a fixed lubber line; for heading.
    Compass,
    /// Artificial horizon: pitch and bank from `pitch` / `roll` fields, else derived from GPS.
    Horizon,
    /// Heading-up wind rose: an arrow from the `wind_direction` field, `wind_speed` inside.
    Wind,
}

impl DialStyle {
    pub const ALL: [DialStyle; 12] = [
        DialStyle::Automotive,
        DialStyle::Motorsport,
        DialStyle::Aviation,
        DialStyle::Minimal,
        DialStyle::ModernDigital,
        DialStyle::Sport,
        DialStyle::ZoneArc,
        DialStyle::LedRing,
        DialStyle::PeakArc,
        DialStyle::Compass,
        DialStyle::Horizon,
        DialStyle::Wind,
    ];
    pub fn label(self) -> &'static str {
        match self {
            DialStyle::Automotive => "Automotive",
            DialStyle::Motorsport => "Motorsport",
            DialStyle::Aviation => "Aviation",
            DialStyle::Minimal => "Minimal",
            DialStyle::ModernDigital => "Modern digital",
            DialStyle::Sport => "Sport",
            DialStyle::ZoneArc => "Zone arc",
            DialStyle::LedRing => "LED ring",
            DialStyle::PeakArc => "Peak hold arc",
            DialStyle::Compass => "Compass",
            DialStyle::Horizon => "Artificial horizon",
            DialStyle::Wind => "Wind",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Orientation {
    #[default]
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RouteMode {
    /// Entire route, traveled part highlighted.
    #[default]
    Full,
    /// Only the traveled part.
    Progressive,
    /// Only the last `trail_seconds`.
    Trail,
    /// Zoomed-in map that follows the current position (`zoom_radius` meters around it).
    CloseUp,
}

impl RouteMode {
    pub const ALL: [RouteMode; 4] = [
        RouteMode::Full,
        RouteMode::Progressive,
        RouteMode::Trail,
        RouteMode::CloseUp,
    ];
    pub fn label(self) -> &'static str {
        match self {
            RouteMode::Full => "Full route",
            RouteMode::Progressive => "Progressive",
            RouteMode::Trail => "Trail",
            RouteMode::CloseUp => "Close-up (follows position)",
        }
    }
}

/// Optional animated icon beside a digital readout.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DigitalIcon {
    #[default]
    None,
    /// A heart that beats at the heart-rate value.
    Heart,
    /// A wedge tilted with the gradient.
    Slope,
}

impl DigitalIcon {
    pub const ALL: [DigitalIcon; 3] = [DigitalIcon::None, DigitalIcon::Heart, DigitalIcon::Slope];
    pub fn label(self) -> &'static str {
        match self {
            DigitalIcon::None => "None",
            DigitalIcon::Heart => "Beating heart",
            DigitalIcon::Slope => "Slope wedge",
        }
    }
}

fn default_ekg_beats() -> f32 {
    3.0
}

fn default_zoom_radius() -> f32 {
    300.0
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeFormat {
    Date,
    #[default]
    TimeOfDay,
    /// Elapsed since the start of the video.
    VideoElapsed,
    /// Elapsed since the start of the GPS recording.
    RecordingElapsed,
    /// Elapsed in the current lap.
    LapTime,
}

impl TimeFormat {
    pub const ALL: [TimeFormat; 5] = [
        TimeFormat::Date,
        TimeFormat::TimeOfDay,
        TimeFormat::VideoElapsed,
        TimeFormat::RecordingElapsed,
        TimeFormat::LapTime,
    ];
    pub fn label(self) -> &'static str {
        match self {
            TimeFormat::Date => "Date",
            TimeFormat::TimeOfDay => "Time of day",
            TimeFormat::VideoElapsed => "Elapsed (video)",
            TimeFormat::RecordingElapsed => "Recording time",
            TimeFormat::LapTime => "Lap time",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextAlign {
    Left,
    #[default]
    Center,
    Right,
}

/// Kind-specific options. Adding a gauge type means adding a variant here and a builder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum GaugeKind {
    Analog {
        dial: DialStyle,
        /// Sweep angle in degrees.
        sweep: f32,
        major_ticks: u32,
        minor_ticks: u32,
        show_readout: bool,
    },
    Digital {
        align: TextAlign,
        #[serde(default)]
        icon: DigitalIcon,
    },
    Tape {
        orientation: Orientation,
        /// Visible span in display units.
        span: f64,
        /// Major tick step in display units (0 = automatic).
        major_step: f64,
        minor_ticks: u32,
    },
    Bar {
        orientation: Orientation,
        /// 0 = continuous.
        segments: u32,
        rounded: bool,
        /// Bar thickness as a fraction of the cross dimension.
        thickness: f32,
        show_value: bool,
    },
    Zone {
        show_value: bool,
    },
    Graph {
        /// Rolling window in seconds; ignored for full-activity graphs.
        window: f32,
        full_activity: bool,
        line_width: f32,
        fill: bool,
        grid: bool,
        auto_scale: bool,
        show_value: bool,
    },
    Map {
        mode: RouteMode,
        trail_seconds: f32,
        route_width: f32,
        marker_size: f32,
        show_arrow: bool,
        heading_up: bool,
        /// Close-up mode: visible radius around the position, in meters.
        #[serde(default = "default_zoom_radius")]
        zoom_radius: f32,
        /// Color the position marker (and close-up trail) by speed, using the gauge's zones.
        #[serde(default)]
        speed_colors: bool,
    },
    Time {
        format: TimeFormat,
        /// Timezone offset in minutes applied to wall-clock values.
        tz_offset_minutes: i32,
    },
    Text {
        text: String,
        align: TextAlign,
    },
    Image {
        path: String,
    },
    /// Scrolling EKG trace with one heartbeat per beat of the heart-rate value.
    Ekg {
        /// Heartbeats visible in the strip (the time span follows the heart rate).
        #[serde(default = "default_ekg_beats")]
        beats: f32,
        /// Seconds of trace visible when there is no heart-rate value.
        window: f32,
        show_value: bool,
        grid: bool,
    },
}

impl GaugeKind {
    pub fn type_label(&self) -> &'static str {
        match self {
            GaugeKind::Analog { .. } => "Analog dial",
            GaugeKind::Digital { .. } => "Digital readout",
            GaugeKind::Tape { .. } => "Tape",
            GaugeKind::Bar { .. } => "Progress bar",
            GaugeKind::Zone { .. } => "Zone gauge",
            GaugeKind::Graph { .. } => "Graph",
            GaugeKind::Map { .. } => "Route map",
            GaugeKind::Time { .. } => "Time",
            GaugeKind::Text { .. } => "Text",
            GaugeKind::Image { .. } => "Image",
            GaugeKind::Ekg { .. } => "EKG monitor",
        }
    }

    /// Whether this kind displays a telemetry metric.
    pub fn uses_metric(&self) -> bool {
        !matches!(
            self,
            GaugeKind::Map { .. }
                | GaugeKind::Time { .. }
                | GaugeKind::Text { .. }
                | GaugeKind::Image { .. }
        )
    }

    /// Whether resizing should keep the aspect ratio by default.
    pub fn prefers_square(&self) -> bool {
        matches!(self, GaugeKind::Analog { .. })
    }
}

/// Future keyframe support: animatable properties over video time.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Keyframe {
    pub time: f64,
    pub property: AnimatedProperty,
    pub value: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnimatedProperty {
    X,
    Y,
    Width,
    Height,
    Opacity,
    Rotation,
    Visible,
}

fn default_true() -> bool {
    true
}
fn default_opacity() -> f32 {
    1.0
}

/// A complete gauge definition.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Gauge {
    pub id: GaugeId,
    pub name: String,
    pub kind: GaugeKind,
    /// Data source.
    pub metric: Metric,
    /// Field key when `metric == Metric::Custom`.
    #[serde(default)]
    pub custom_key: String,
    pub placement: Placement,
    #[serde(default = "default_opacity")]
    pub opacity: f32,
    #[serde(default = "default_true")]
    pub visible: bool,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub group: Option<u32>,
    /// Range in canonical units.
    pub min: f64,
    pub max: f64,
    #[serde(default)]
    pub units: UnitPref,
    /// Visual smoothing 0..=1.
    #[serde(default)]
    pub smoothing: f32,
    /// Distance gauges: count from the sync start point (the GPS position at the start of the
    /// video) instead of the start of the recording.
    #[serde(default)]
    pub distance_from_sync: bool,
    #[serde(default)]
    pub style: Style,
    #[serde(default)]
    pub zones: Vec<Zone>,
    #[serde(default)]
    pub zone_targets: ZoneTargets,
    /// Reserved for keyframe animation; gauges are static in this version.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keyframes: Vec<Keyframe>,
}

impl Gauge {
    /// Display label (custom or metric name).
    pub fn label(&self) -> String {
        if !self.style.label.is_empty() {
            self.style.label.clone()
        } else if self.metric == Metric::Custom {
            self.custom_key.to_uppercase()
        } else {
            self.metric.short_label().to_string()
        }
    }

    /// Zone containing `value` (canonical units).
    pub fn zone_for(&self, value: f64) -> Option<(usize, &Zone)> {
        self.zones
            .iter()
            .enumerate()
            .rev()
            .find(|(_, z)| value >= z.from)
    }

    /// Upper bound of zone `i` (next zone start or the gauge max).
    pub fn zone_end(&self, i: usize) -> f64 {
        self.zones.get(i + 1).map(|z| z.from).unwrap_or(self.max)
    }

    /// Fraction of the range covered by `value`, clamped to 0..=1.
    pub fn fraction(&self, value: f64) -> f64 {
        let span = self.max - self.min;
        if span.abs() < 1e-12 {
            return 0.0;
        }
        ((value - self.min) / span).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_roundtrip() {
        let c = Rgba::rgba(255, 82, 48, 200);
        assert_eq!(c.to_hex(), "#FF5230C8");
        assert_eq!(Rgba::from_hex("#FF5230C8"), Some(c));
        assert_eq!(Rgba::from_hex("ff5230"), Some(Rgba::rgb(255, 82, 48)));
        assert_eq!(Rgba::from_hex("zz"), None);
    }

    #[test]
    fn placement_transforms() {
        let p = Placement {
            x: 100.0,
            y: 100.0,
            w: 200.0,
            h: 100.0,
            rotation: 90.0,
        };
        let (wx, wy) = p.to_world(0.0, 0.0);
        let (lx, ly) = p.to_local(wx, wy);
        assert!(lx.abs() < 1e-3 && ly.abs() < 1e-3);
        // Rotated 90°, the box spans 100 wide and 200 tall around center (200,150).
        let (bx, by, bw, bh) = p.bounds();
        assert!((bw - 100.0).abs() < 1e-3 && (bh - 200.0).abs() < 1e-3);
        assert!((bx - 150.0).abs() < 1e-3 && (by - 50.0).abs() < 1e-3);
        assert!(p.contains(200.0, 60.0));
        assert!(!p.contains(110.0, 110.0));
    }
}
