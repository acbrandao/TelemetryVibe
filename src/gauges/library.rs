//! Predefined gauges (the gauge library) and multi-gauge templates.

use super::model::*;
use crate::telemetry::units::{to_display, to_si};
use crate::telemetry::{Metric, Track, UnitPref, UnitSystem};
use crate::utils::nice_ceil;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PresetId {
    AnalogSpeed,
    DigitalSpeed,
    SpeedTape,
    SpeedBar,
    SpeedGraph,
    ModernSpeed,
    SportSpeed,
    MotorsportSpeed,
    AviationSpeed,
    DigitalPace,
    DigitalAltitude,
    AltitudeTape,
    ElevationGraph,
    DigitalGrade,
    AnalogHr,
    DigitalHr,
    HrBar,
    HrZone,
    HrGraph,
    HrPulse,
    HrEkg,
    HrZoneDial,
    HrLedRing,
    DigitalCadence,
    CircularCadence,
    CadenceBar,
    DigitalPower,
    PowerGauge,
    PowerZone,
    PowerGraph,
    PowerZoneDial,
    PowerLedRing,
    PowerPeakDial,
    PowerSportDial,
    DigitalTemperature,
    TemperatureBar,
    DigitalDistance,
    DistanceProgress,
    HeadingTape,
    GradientTape,
    FullRouteMap,
    MovingRoute,
    TrailMap,
    CloseUpMap,
    Date,
    TimeOfDay,
    ElapsedTime,
    RecordingTime,
    LapTime,
    TextLabel,
    ImageLogo,
}

impl PresetId {
    pub const ALL: &'static [PresetId] = &[
        PresetId::AnalogSpeed,
        PresetId::DigitalSpeed,
        PresetId::SpeedTape,
        PresetId::SpeedBar,
        PresetId::SpeedGraph,
        PresetId::ModernSpeed,
        PresetId::SportSpeed,
        PresetId::MotorsportSpeed,
        PresetId::AviationSpeed,
        PresetId::DigitalPace,
        PresetId::DigitalAltitude,
        PresetId::AltitudeTape,
        PresetId::ElevationGraph,
        PresetId::DigitalGrade,
        PresetId::AnalogHr,
        PresetId::DigitalHr,
        PresetId::HrBar,
        PresetId::HrZone,
        PresetId::HrGraph,
        PresetId::HrPulse,
        PresetId::HrEkg,
        PresetId::HrZoneDial,
        PresetId::HrLedRing,
        PresetId::DigitalCadence,
        PresetId::CircularCadence,
        PresetId::CadenceBar,
        PresetId::DigitalPower,
        PresetId::PowerGauge,
        PresetId::PowerZone,
        PresetId::PowerGraph,
        PresetId::PowerZoneDial,
        PresetId::PowerLedRing,
        PresetId::PowerPeakDial,
        PresetId::PowerSportDial,
        PresetId::DigitalTemperature,
        PresetId::TemperatureBar,
        PresetId::DigitalDistance,
        PresetId::DistanceProgress,
        PresetId::HeadingTape,
        PresetId::GradientTape,
        PresetId::FullRouteMap,
        PresetId::MovingRoute,
        PresetId::TrailMap,
        PresetId::CloseUpMap,
        PresetId::Date,
        PresetId::TimeOfDay,
        PresetId::ElapsedTime,
        PresetId::RecordingTime,
        PresetId::LapTime,
        PresetId::TextLabel,
        PresetId::ImageLogo,
    ];

    pub fn name(self) -> &'static str {
        match self {
            PresetId::AnalogSpeed => "Analog speedometer",
            PresetId::DigitalSpeed => "Digital speed",
            PresetId::SpeedTape => "Speed tape",
            PresetId::SpeedBar => "Speed bar",
            PresetId::SpeedGraph => "Speed graph",
            PresetId::ModernSpeed => "Modern speed dial",
            PresetId::SportSpeed => "Sport speed dial",
            PresetId::MotorsportSpeed => "Motorsport dial",
            PresetId::AviationSpeed => "Aviation dial",
            PresetId::DigitalPace => "Digital pace",
            PresetId::DigitalAltitude => "Digital altitude",
            PresetId::AltitudeTape => "Altitude tape",
            PresetId::ElevationGraph => "Elevation graph",
            PresetId::DigitalGrade => "Digital gradient",
            PresetId::AnalogHr => "Analog heart rate",
            PresetId::DigitalHr => "Digital heart rate",
            PresetId::HrBar => "Heart rate bar",
            PresetId::HrZone => "Heart rate zone",
            PresetId::HrGraph => "Heart rate graph",
            PresetId::HrPulse => "Heart rate with beating heart",
            PresetId::HrEkg => "Heart rate EKG monitor",
            PresetId::HrZoneDial => "Heart rate zone dial",
            PresetId::HrLedRing => "Heart rate LED ring",
            PresetId::DigitalCadence => "Digital cadence",
            PresetId::CircularCadence => "Circular cadence",
            PresetId::CadenceBar => "Cadence bar",
            PresetId::DigitalPower => "Digital power",
            PresetId::PowerGauge => "Power gauge",
            PresetId::PowerZone => "Power zone",
            PresetId::PowerGraph => "Power graph",
            PresetId::PowerZoneDial => "Power zone dial",
            PresetId::PowerLedRing => "Power LED ring",
            PresetId::PowerPeakDial => "Power peak-hold dial",
            PresetId::PowerSportDial => "Power sport dial",
            PresetId::DigitalTemperature => "Digital temperature",
            PresetId::TemperatureBar => "Temperature bar",
            PresetId::DigitalDistance => "Digital distance",
            PresetId::DistanceProgress => "Distance progress",
            PresetId::HeadingTape => "Heading tape",
            PresetId::GradientTape => "Gradient tape",
            PresetId::FullRouteMap => "Full route map",
            PresetId::MovingRoute => "Moving route",
            PresetId::TrailMap => "Trail map",
            PresetId::CloseUpMap => "Close-up moving map",
            PresetId::Date => "Date",
            PresetId::TimeOfDay => "Time",
            PresetId::ElapsedTime => "Elapsed time",
            PresetId::RecordingTime => "Recording time",
            PresetId::LapTime => "Lap time",
            PresetId::TextLabel => "Text",
            PresetId::ImageLogo => "Image / logo",
        }
    }

    /// Primary metric of the preset (used to flag missing data in the library).
    pub fn metric(self) -> Option<Metric> {
        use PresetId::*;
        Some(match self {
            AnalogSpeed | DigitalSpeed | SpeedTape | SpeedBar | SpeedGraph | ModernSpeed
            | SportSpeed | MotorsportSpeed | AviationSpeed => Metric::Speed,
            DigitalPace => Metric::Pace,
            DigitalAltitude | AltitudeTape | ElevationGraph => Metric::Altitude,
            AnalogHr | DigitalHr | HrBar | HrZone | HrGraph | HrPulse | HrEkg | HrZoneDial
            | HrLedRing => Metric::HeartRate,
            DigitalCadence | CircularCadence | CadenceBar => Metric::Cadence,
            DigitalPower | PowerGauge | PowerZone | PowerGraph | PowerZoneDial | PowerLedRing
            | PowerPeakDial | PowerSportDial => Metric::Power,
            DigitalTemperature | TemperatureBar => Metric::Temperature,
            DigitalDistance | DistanceProgress => Metric::Distance,
            HeadingTape => Metric::Heading,
            GradientTape | DigitalGrade => Metric::Grade,
            FullRouteMap | MovingRoute | TrailMap | CloseUpMap => Metric::Latitude,
            _ => return None,
        })
    }
}

/// Library categories for the UI.
pub fn categories() -> Vec<(&'static str, Vec<PresetId>)> {
    use PresetId::*;
    vec![
        (
            "Speed",
            vec![
                AnalogSpeed,
                DigitalSpeed,
                SpeedTape,
                SpeedBar,
                SpeedGraph,
                ModernSpeed,
                SportSpeed,
                MotorsportSpeed,
                AviationSpeed,
                DigitalPace,
            ],
        ),
        (
            "Altitude",
            vec![
                DigitalAltitude,
                AltitudeTape,
                ElevationGraph,
                GradientTape,
                DigitalGrade,
            ],
        ),
        (
            "Heart Rate",
            vec![
                AnalogHr, DigitalHr, HrPulse, HrEkg, HrZoneDial, HrLedRing, HrBar, HrZone, HrGraph,
            ],
        ),
        ("Cadence", vec![DigitalCadence, CircularCadence, CadenceBar]),
        (
            "Power",
            vec![
                DigitalPower,
                PowerGauge,
                PowerZoneDial,
                PowerLedRing,
                PowerPeakDial,
                PowerSportDial,
                PowerZone,
                PowerGraph,
            ],
        ),
        ("Temperature", vec![DigitalTemperature, TemperatureBar]),
        ("Distance", vec![DigitalDistance, DistanceProgress]),
        (
            "Route",
            vec![FullRouteMap, MovingRoute, TrailMap, CloseUpMap, HeadingTape],
        ),
        (
            "Time",
            vec![Date, TimeOfDay, ElapsedTime, RecordingTime, LapTime],
        ),
        ("Text & Branding", vec![TextLabel, ImageLogo]),
    ]
}

/// Default canonical range for a metric, adapted to the recording when available.
pub fn default_range(metric: Metric, track: Option<&Track>, units: UnitSystem) -> (f64, f64) {
    let stats = track.and_then(|t| t.stats_for(metric));
    // Choose limits that are round numbers in the display unit system.
    let nice_max = |si_max: f64, fallback_display: f64| {
        let d = si_max
            .is_finite()
            .then(|| to_display(metric, si_max, units));
        let dmax = d.map(|d| nice_ceil(d * 1.1)).unwrap_or(fallback_display);
        to_si(metric, dmax, units)
    };
    match metric {
        Metric::Speed => (
            0.0,
            nice_max(
                stats.map(|s| s.max).unwrap_or(f64::NAN),
                if units == UnitSystem::Metric {
                    60.0
                } else {
                    40.0
                },
            ),
        ),
        Metric::HeartRate => (
            60.0,
            stats
                .map(|s| nice_ceil(s.max.max(150.0) * 1.05))
                .unwrap_or(200.0),
        ),
        Metric::Cadence => (
            0.0,
            stats.map(|s| nice_ceil(s.max.max(100.0))).unwrap_or(150.0),
        ),
        Metric::Power => (
            0.0,
            stats
                .map(|s| nice_ceil(s.max.clamp(200.0, 2000.0)))
                .unwrap_or(600.0),
        ),
        Metric::Altitude | Metric::GpsAltitude => match stats {
            Some(s) => {
                let lo = to_display(metric, s.min, units);
                let hi = to_display(metric, s.max, units);
                let step = crate::utils::nice_step((hi - lo).max(10.0), 4.0);
                (
                    to_si(metric, (lo / step).floor() * step, units),
                    to_si(metric, (hi / step).ceil() * step, units),
                )
            }
            None => (
                0.0,
                to_si(
                    metric,
                    if units == UnitSystem::Metric {
                        1000.0
                    } else {
                        3000.0
                    },
                    units,
                ),
            ),
        },
        Metric::Temperature => (
            to_si(
                metric,
                if units == UnitSystem::Metric {
                    -10.0
                } else {
                    10.0
                },
                units,
            ),
            to_si(
                metric,
                if units == UnitSystem::Metric {
                    40.0
                } else {
                    100.0
                },
                units,
            ),
        ),
        Metric::Distance => (
            0.0,
            track
                .and_then(|t| t.stats.total_distance)
                .filter(|d| *d > 0.0)
                .unwrap_or(to_si(metric, 50.0, units)),
        ),
        Metric::Grade => (-20.0, 20.0),
        Metric::Heading => (0.0, 360.0),
        Metric::Pace => (
            to_si(metric, 3.0 * 60.0, units),
            to_si(metric, 10.0 * 60.0, units),
        ),
        Metric::VerticalSpeed => (-2.0, 2.0),
        _ => (0.0, stats.map(|s| nice_ceil(s.max)).unwrap_or(100.0)),
    }
}

/// Five heart-rate zones based on maximum heart rate.
pub fn hr_zones(max_hr: f64) -> Vec<Zone> {
    let z = |f: f64, c: Rgba, l: &str| Zone {
        from: (max_hr * f).round(),
        color: c,
        label: l.into(),
    };
    vec![
        Zone {
            from: 0.0,
            color: Rgba::rgb(110, 150, 200),
            label: "Recovery".into(),
        },
        z(0.6, Rgba::rgb(70, 180, 110), "Endurance"),
        z(0.7, Rgba::rgb(255, 200, 40), "Tempo"),
        z(0.8, Rgba::rgb(255, 130, 40), "Threshold"),
        z(0.9, Rgba::rgb(235, 55, 50), "Maximum"),
    ]
}

/// Six power zones based on FTP (Coggan model).
pub fn power_zones(ftp: f64) -> Vec<Zone> {
    let z = |f: f64, c: Rgba, l: &str| Zone {
        from: (ftp * f).round(),
        color: c,
        label: l.into(),
    };
    vec![
        Zone {
            from: 0.0,
            color: Rgba::rgb(150, 150, 160),
            label: "Recovery".into(),
        },
        z(0.55, Rgba::rgb(70, 140, 230), "Endurance"),
        z(0.75, Rgba::rgb(70, 190, 110), "Tempo"),
        z(0.90, Rgba::rgb(255, 205, 40), "Threshold"),
        z(1.05, Rgba::rgb(255, 130, 40), "VO2"),
        z(1.20, Rgba::rgb(235, 55, 50), "Anaerobic"),
    ]
}

/// Gradient zones (percent): descents in blues, then flat, climbing, steep, very steep.
pub fn grade_zones() -> Vec<Zone> {
    let z = |from: f64, c: Rgba, l: &str| Zone {
        from,
        color: c,
        label: l.into(),
    };
    vec![
        z(-100.0, Rgba::rgb(80, 150, 255), "Steep descent"),
        z(-4.0, Rgba::rgb(140, 200, 255), "Descent"),
        z(-1.5, Rgba::rgb(110, 210, 120), "Flat"),
        z(3.0, Rgba::rgb(255, 210, 50), "Climb"),
        z(6.0, Rgba::rgb(255, 140, 40), "Steep"),
        z(10.0, Rgba::rgb(240, 60, 50), "Very steep"),
    ]
}

/// Speed zones (Normal / Fast / Very fast / Maximum) spread over the range.
pub fn speed_zones(max: f64) -> Vec<Zone> {
    vec![
        Zone {
            from: 0.0,
            color: Rgba::rgb(70, 180, 110),
            label: "Normal".into(),
        },
        Zone {
            from: max * 0.4,
            color: Rgba::rgb(255, 200, 40),
            label: "Fast".into(),
        },
        Zone {
            from: max * 0.7,
            color: Rgba::rgb(255, 130, 40),
            label: "Very fast".into(),
        },
        Zone {
            from: max * 0.85,
            color: Rgba::rgb(235, 55, 50),
            label: "Maximum".into(),
        },
    ]
}

fn base(
    id: GaugeId,
    name: &str,
    kind: GaugeKind,
    metric: Metric,
    w: f32,
    h: f32,
    video: (f32, f32),
) -> Gauge {
    Gauge {
        id,
        name: name.to_string(),
        kind,
        metric,
        custom_key: String::new(),
        placement: Placement {
            x: ((video.0 - w) / 2.0).round(),
            y: ((video.1 - h) / 2.0).round(),
            w: w.round(),
            h: h.round(),
            rotation: 0.0,
        },
        opacity: 1.0,
        visible: true,
        locked: false,
        group: None,
        min: 0.0,
        max: 100.0,
        units: UnitPref::Inherit,
        smoothing: 0.35,
        style: Style {
            decimals: metric.default_decimals(),
            ..Style::default()
        },
        zones: Vec::new(),
        zone_targets: ZoneTargets::default(),
        keyframes: Vec::new(),
    }
}

fn analog(dial: DialStyle, sweep: f32) -> GaugeKind {
    GaugeKind::Analog {
        dial,
        sweep,
        major_ticks: 0,
        minor_ticks: 5,
        show_readout: true,
    }
}

fn digital() -> GaugeKind {
    digital_icon(DigitalIcon::None)
}

fn digital_icon(icon: DigitalIcon) -> GaugeKind {
    GaugeKind::Digital {
        align: TextAlign::Left,
        icon,
    }
}

fn bar() -> GaugeKind {
    GaugeKind::Bar {
        orientation: Orientation::Horizontal,
        segments: 0,
        rounded: true,
        thickness: 0.7,
        show_value: true,
    }
}

fn graph(window: f32, full: bool) -> GaugeKind {
    GaugeKind::Graph {
        window,
        full_activity: full,
        line_width: 2.5,
        fill: true,
        grid: true,
        auto_scale: true,
        show_value: true,
    }
}

fn tape(orientation: Orientation, span: f64) -> GaugeKind {
    GaugeKind::Tape {
        orientation,
        span,
        major_step: 0.0,
        minor_ticks: 5,
    }
}

fn map(mode: RouteMode) -> GaugeKind {
    GaugeKind::Map {
        mode,
        trail_seconds: 120.0,
        route_width: 5.0,
        marker_size: 11.0,
        show_arrow: true,
        heading_up: false,
        zoom_radius: 300.0,
        speed_colors: false,
    }
}

fn time(format: TimeFormat) -> GaugeKind {
    GaugeKind::Time {
        format,
        tz_offset_minutes: super::digital::local_tz_offset_minutes(),
    }
}

/// Creates a gauge from a preset, centered in a video of `video` size (pixels).
pub fn make_preset(
    preset: PresetId,
    id: GaugeId,
    video: (f32, f32),
    track: Option<&Track>,
    units: UnitSystem,
) -> Gauge {
    use PresetId::*;
    let hgt = video.1.max(100.0);
    let dial_size = hgt * 0.27;
    let (dw, dh) = (hgt * 0.30, hgt * 0.13);
    let (bw, bh) = (hgt * 0.36, hgt * 0.1);
    let (gw, gh) = (hgt * 0.40, hgt * 0.17);
    let map_size = hgt * 0.30;
    let name = preset.name();
    let metric = preset.metric().unwrap_or(Metric::Speed);
    let metric = match metric {
        Metric::Latitude => Metric::Speed,
        // Altitude gauges default to GPS altitude when the recording has it.
        Metric::Altitude => Metric::preferred_altitude(track),
        m => m,
    };

    let mut g = match preset {
        AnalogSpeed | AnalogHr | PowerGauge | CircularCadence => base(
            id,
            name,
            analog(DialStyle::Automotive, 270.0),
            metric,
            dial_size,
            dial_size,
            video,
        ),
        ModernSpeed => base(
            id,
            name,
            analog(DialStyle::ModernDigital, 260.0),
            metric,
            dial_size,
            dial_size,
            video,
        ),
        SportSpeed => base(
            id,
            name,
            analog(DialStyle::Sport, 270.0),
            metric,
            dial_size,
            dial_size,
            video,
        ),
        MotorsportSpeed => base(
            id,
            name,
            analog(DialStyle::Motorsport, 240.0),
            metric,
            dial_size,
            dial_size,
            video,
        ),
        AviationSpeed => base(
            id,
            name,
            analog(DialStyle::Aviation, 300.0),
            metric,
            dial_size,
            dial_size,
            video,
        ),
        DigitalSpeed | DigitalPace | DigitalAltitude | DigitalHr | DigitalCadence
        | DigitalPower | DigitalTemperature | DigitalDistance => {
            base(id, name, digital(), metric, dw, dh, video)
        }
        HrEkg => base(
            id,
            name,
            GaugeKind::Ekg {
                beats: 3.0,
                window: 3.0,
                show_value: true,
                grid: true,
            },
            metric,
            dw,
            dh,
            video,
        ),
        HrPulse => base(
            id,
            name,
            digital_icon(DigitalIcon::Heart),
            metric,
            dw * 1.15,
            dh,
            video,
        ),
        DigitalGrade => base(
            id,
            name,
            digital_icon(DigitalIcon::Slope),
            metric,
            dw * 1.15,
            dh,
            video,
        ),
        HrZoneDial | PowerZoneDial => base(
            id,
            name,
            analog(DialStyle::ZoneArc, 260.0),
            metric,
            dial_size,
            dial_size,
            video,
        ),
        HrLedRing | PowerLedRing => base(
            id,
            name,
            analog(DialStyle::LedRing, 280.0),
            metric,
            dial_size,
            dial_size,
            video,
        ),
        PowerPeakDial => base(
            id,
            name,
            analog(DialStyle::PeakArc, 260.0),
            metric,
            dial_size,
            dial_size,
            video,
        ),
        PowerSportDial => base(
            id,
            name,
            analog(DialStyle::Sport, 270.0),
            metric,
            dial_size,
            dial_size,
            video,
        ),
        SpeedBar | HrBar | CadenceBar | TemperatureBar | DistanceProgress => {
            base(id, name, bar(), metric, bw, bh, video)
        }
        SpeedTape | AltitudeTape | GradientTape => base(
            id,
            name,
            tape(Orientation::Vertical, 0.0),
            metric,
            hgt * 0.13,
            hgt * 0.42,
            video,
        ),
        HeadingTape => base(
            id,
            name,
            tape(Orientation::Horizontal, 90.0),
            Metric::Heading,
            hgt * 0.5,
            hgt * 0.1,
            video,
        ),
        HrZone | PowerZone => base(
            id,
            name,
            GaugeKind::Zone { show_value: true },
            metric,
            hgt * 0.36,
            hgt * 0.17,
            video,
        ),
        SpeedGraph | HrGraph | PowerGraph => {
            base(id, name, graph(30.0, false), metric, gw, gh, video)
        }
        ElevationGraph => base(id, name, graph(30.0, true), metric, gw, gh, video),
        FullRouteMap => base(
            id,
            name,
            map(RouteMode::Full),
            Metric::Latitude,
            map_size,
            map_size,
            video,
        ),
        MovingRoute => base(
            id,
            name,
            map(RouteMode::Progressive),
            Metric::Latitude,
            map_size,
            map_size,
            video,
        ),
        TrailMap => base(
            id,
            name,
            map(RouteMode::Trail),
            Metric::Latitude,
            map_size,
            map_size,
            video,
        ),
        CloseUpMap => {
            let mut kind = map(RouteMode::CloseUp);
            if let GaugeKind::Map {
                heading_up,
                speed_colors,
                route_width,
                marker_size,
                ..
            } = &mut kind
            {
                *heading_up = true;
                *speed_colors = true;
                *route_width = (hgt * 0.009).max(3.0);
                *marker_size = (hgt * 0.013).max(6.0);
            }
            // Bound to speed so the zone editor shows (and converts) speed values.
            base(id, name, kind, Metric::Speed, map_size, map_size, video)
        }
        Date => base(
            id,
            name,
            time(TimeFormat::Date),
            Metric::Speed,
            hgt * 0.3,
            hgt * 0.11,
            video,
        ),
        TimeOfDay => base(
            id,
            name,
            time(TimeFormat::TimeOfDay),
            Metric::Speed,
            hgt * 0.26,
            hgt * 0.11,
            video,
        ),
        ElapsedTime => base(
            id,
            name,
            time(TimeFormat::VideoElapsed),
            Metric::Speed,
            hgt * 0.26,
            hgt * 0.11,
            video,
        ),
        RecordingTime => base(
            id,
            name,
            time(TimeFormat::RecordingElapsed),
            Metric::Speed,
            hgt * 0.26,
            hgt * 0.11,
            video,
        ),
        LapTime => base(
            id,
            name,
            time(TimeFormat::LapTime),
            Metric::Speed,
            hgt * 0.26,
            hgt * 0.11,
            video,
        ),
        TextLabel => {
            let mut g = base(
                id,
                name,
                GaugeKind::Text {
                    text: "Your text".into(),
                    align: TextAlign::Center,
                },
                Metric::Speed,
                hgt * 0.4,
                hgt * 0.08,
                video,
            );
            g.style.background_opacity = 0.0;
            g.style.font_weight = FontWeight::Black;
            g
        }
        ImageLogo => {
            let mut g = base(
                id,
                name,
                GaugeKind::Image {
                    path: String::new(),
                },
                Metric::Speed,
                hgt * 0.15,
                hgt * 0.15,
                video,
            );
            g.style.shadow = false;
            g
        }
    };

    if g.kind.uses_metric() {
        let (lo, hi) = default_range(g.metric, track, units);
        g.min = lo;
        g.max = hi;
    }
    match preset {
        DistanceProgress => {
            g.style.label = "PROGRESS".into();
        }
        HeadingTape => {
            g.min = 0.0;
            g.max = 360.0;
        }
        GradientTape => {
            if let GaugeKind::Tape { span, .. } = &mut g.kind {
                *span = 20.0;
            }
            g.style.decimals = 1;
        }
        SpeedTape => {
            if let GaugeKind::Tape { span, .. } = &mut g.kind {
                *span = 40.0;
            }
        }
        AltitudeTape => {
            if let GaugeKind::Tape { span, .. } = &mut g.kind {
                *span = if units == UnitSystem::Metric {
                    100.0
                } else {
                    300.0
                };
            }
        }
        MotorsportSpeed => {
            g.zones = vec![Zone {
                from: g.max * 0.85,
                color: Rgba::rgb(235, 40, 40),
                label: "Redline".into(),
            }];
            g.style.accent = Rgba::rgb(255, 60, 30);
        }
        SportSpeed => {
            g.zones = speed_zones(g.max);
        }
        AviationSpeed => {
            g.zones = speed_zones(g.max);
            g.style.background = Rgba::rgb(5, 5, 5);
            g.style.background_opacity = 0.85;
        }
        ModernSpeed => {
            g.style.primary = Rgba::rgb(0, 210, 255);
            g.style.glow = 0.6;
        }
        HrZone | HrBar | AnalogHr | HrGraph | DigitalHr | HrPulse | HrEkg | HrZoneDial
        | HrLedRing => {
            let max_hr = track
                .and_then(|t| t.stats.get(Metric::HeartRate))
                .map(|s| s.max.max(160.0))
                .unwrap_or(190.0);
            g.zones = hr_zones(max_hr);
            g.style.primary = Rgba::rgb(255, 70, 80);
            if preset == HrBar
                && let GaugeKind::Bar { segments, .. } = &mut g.kind
            {
                *segments = 20;
            }
            if preset == HrEkg {
                // Classic monitor look: green trace on near-black.
                g.style.primary = Rgba::rgb(60, 255, 120);
                g.style.background = Rgba::rgb(4, 10, 6);
                g.style.background_opacity = 0.85;
                g.style.glow = 0.6;
                g.style.label = "HR".into();
                g.smoothing = 0.0;
            }
            if preset == DigitalHr || preset == HrGraph || preset == HrPulse || preset == HrEkg {
                g.zone_targets.number = false;
                g.zones.clear();
            }
            if preset == HrPulse {
                g.style.primary = Rgba::rgb(240, 40, 60);
                g.style.glow = 0.5;
            }
            if preset == HrLedRing {
                g.style.background = Rgba::rgb(6, 6, 8);
                g.style.background_opacity = 0.8;
            }
        }
        PowerZone | PowerGauge | PowerGraph | DigitalPower | PowerZoneDial | PowerLedRing
        | PowerPeakDial | PowerSportDial => {
            let ftp = track
                .and_then(|t| t.stats.get(Metric::Power))
                .map(|s| (s.avg * 1.25).clamp(150.0, 400.0))
                .unwrap_or(250.0);
            if preset != DigitalPower && preset != PowerGraph {
                g.zones = power_zones(ftp);
            }
            g.style.primary = Rgba::rgb(255, 200, 40);
            if preset == PowerLedRing {
                g.style.background = Rgba::rgb(6, 6, 8);
                g.style.background_opacity = 0.8;
            }
            if preset == PowerPeakDial {
                g.style.glow = 0.35;
            }
        }
        DigitalGrade => {
            g.style.decimals = 1;
            g.zones = grade_zones();
            g.zone_targets.number = true;
            g.zone_targets.bar = false;
            g.zone_targets.arc = false;
        }
        CadenceBar | CircularCadence | DigitalCadence => {
            g.style.primary = Rgba::rgb(80, 200, 255);
        }
        ElevationGraph => {
            g.style.primary = Rgba::rgb(120, 220, 120);
            g.smoothing = 0.0;
        }
        FullRouteMap | MovingRoute | TrailMap | CloseUpMap => {
            g.style.primary = Rgba::rgb(255, 120, 40);
            g.style.secondary = Rgba::rgba(255, 255, 255, 110);
            g.style.corner_radius = 0.12;
            g.style.glow = 0.4;
            if preset == CloseUpMap {
                let max = track
                    .and_then(|t| t.stats.get(Metric::Speed))
                    .map(|s| s.max)
                    .filter(|m| *m > 1.0)
                    .unwrap_or(15.0);
                g.zones = speed_zones(max);
                g.min = 0.0;
                g.max = max;
                g.style.primary = Rgba::rgb(120, 200, 255);
                g.style.background_opacity = 0.65;
            }
        }
        _ => {}
    }
    // Digital-style gauges (readouts, icon readouts, EKG, time) share one height and get a
    // width that fits their content, so their numbers come out the same size.
    if matches!(
        g.kind,
        GaugeKind::Digital { .. } | GaugeKind::Ekg { .. } | GaugeKind::Time { .. }
    ) {
        g.placement.h = dh.round();
        let sync = crate::telemetry::SyncSettings::default();
        let ctx = super::RenderCtx {
            track,
            sync: &sync,
            video_t: 0.0,
            units,
        };
        if let Some(nw) = super::digital::natural_width(&g, &ctx) {
            g.placement.w = nw;
        }
        g.placement.x = ((video.0 - g.placement.w) / 2.0).round();
        g.placement.y = ((video.1 - g.placement.h) / 2.0).round();
    }
    g
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Template {
    Cycling,
    Running,
    Sailing,
    Hiking,
    Motorsport,
}

impl Template {
    pub const ALL: [Template; 5] = [
        Template::Cycling,
        Template::Running,
        Template::Sailing,
        Template::Hiking,
        Template::Motorsport,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Template::Cycling => "Cycling",
            Template::Running => "Running",
            Template::Sailing => "Sailing",
            Template::Hiking => "Hiking",
            Template::Motorsport => "Motorsport",
        }
    }
    pub fn description(self) -> &'static str {
        match self {
            Template::Cycling => "Speed, cadence, power, heart rate, map",
            Template::Running => "Pace, heart rate, distance, route",
            Template::Sailing => "Speed, heading, distance, route",
            Template::Hiking => "Altitude, distance, heart rate, route",
            Template::Motorsport => "Speed, RPM, track, lap time",
        }
    }
}

/// Instantiates a template. `next_id` is called for every new gauge.
pub fn apply_template(
    t: Template,
    video: (f32, f32),
    track: Option<&Track>,
    units: UnitSystem,
    next_id: &mut dyn FnMut() -> GaugeId,
) -> Vec<Gauge> {
    use PresetId::*;
    let (vw, vh) = video;
    let m = vh * 0.035;
    let mut out = Vec::new();
    let mut add = |p: PresetId, x: f32, y: f32, out: &mut Vec<Gauge>| {
        let mut g = make_preset(p, next_id(), video, track, units);
        g.placement.x = x.round();
        g.placement.y = y.round();
        out.push(g);
    };
    // Helpers to read the preset sizes.
    let size = |p: PresetId| {
        let g = make_preset(p, GaugeId(0), video, track, units);
        (g.placement.w, g.placement.h)
    };
    match t {
        Template::Cycling => {
            let (dw, dh) = size(AnalogSpeed);
            add(AnalogSpeed, m, vh - dh - m, &mut out);
            let (rw, rh) = size(DigitalPower);
            let x = m * 2.0 + dw;
            add(DigitalPower, x, vh - m - rh * 3.0 - m, &mut out);
            add(DigitalCadence, x, vh - m - rh * 2.0 - m / 2.0, &mut out);
            add(DigitalHr, x, vh - m - rh, &mut out);
            let _ = rw;
            let (mw, _) = size(FullRouteMap);
            add(FullRouteMap, vw - mw - m, m, &mut out);
            let (gw, gh) = size(ElevationGraph);
            add(ElevationGraph, vw - gw - m, vh - gh - m, &mut out);
        }
        Template::Running => {
            let (_, rh) = size(DigitalPace);
            add(DigitalPace, m, vh - m - rh * 3.0 - m, &mut out);
            add(DigitalHr, m, vh - m - rh * 2.0 - m / 2.0, &mut out);
            add(DigitalDistance, m, vh - m - rh, &mut out);
            let (mw, _) = size(TrailMap);
            add(TrailMap, vw - mw - m, m, &mut out);
            let (zw, zh) = size(HrZone);
            add(HrZone, vw - zw - m, vh - zh - m, &mut out);
        }
        Template::Sailing => {
            let (dw, dh) = size(AviationSpeed);
            add(AviationSpeed, m, vh - dh - m, &mut out);
            let (hw, _) = size(HeadingTape);
            add(HeadingTape, (vw - hw) / 2.0, m, &mut out);
            let (_, rh) = size(DigitalDistance);
            add(DigitalDistance, m * 2.0 + dw, vh - rh - m, &mut out);
            let (mw, _) = size(FullRouteMap);
            add(FullRouteMap, vw - mw - m, m, &mut out);
        }
        Template::Hiking => {
            let (_, rh) = size(DigitalAltitude);
            add(DigitalAltitude, m, vh - m - rh * 3.0 - m, &mut out);
            add(DigitalDistance, m, vh - m - rh * 2.0 - m / 2.0, &mut out);
            add(DigitalHr, m, vh - m - rh, &mut out);
            let (gw, gh) = size(ElevationGraph);
            add(ElevationGraph, vw - gw - m, vh - gh - m, &mut out);
            let (mw, _) = size(FullRouteMap);
            add(FullRouteMap, vw - mw - m, m, &mut out);
        }
        Template::Motorsport => {
            let (dw, dh) = size(MotorsportSpeed);
            add(MotorsportSpeed, m, vh - dh - m, &mut out);
            // RPM from a custom telemetry field (shows "--" if absent).
            add(MotorsportSpeed, m * 2.0 + dw, vh - dh - m, &mut out);
            if let Some(rpm) = out.last_mut() {
                rpm.name = "RPM".into();
                rpm.metric = Metric::Custom;
                rpm.custom_key = "rpm".into();
                rpm.min = 0.0;
                rpm.max = 10000.0;
                rpm.zones = vec![Zone {
                    from: 8500.0,
                    color: Rgba::rgb(235, 40, 40),
                    label: "Redline".into(),
                }];
                rpm.style.label = "RPM".into();
            }
            let (mw, _) = size(FullRouteMap);
            add(FullRouteMap, vw - mw - m, m, &mut out);
            let (lw, lh) = size(LapTime);
            add(LapTime, vw - lw - m, vh - lh - m, &mut out);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn templates_fit_inside_video() {
        let mut n = 0;
        let mut next = || {
            n += 1;
            GaugeId(n)
        };
        for t in Template::ALL {
            let gauges = apply_template(t, (1920.0, 1080.0), None, UnitSystem::Metric, &mut next);
            assert!(gauges.len() >= 4, "{t:?}");
            for g in &gauges {
                let p = g.placement;
                assert!(p.x >= 0.0 && p.y >= 0.0, "{t:?} {}", g.name);
                assert!(
                    p.x + p.w <= 1920.5 && p.y + p.h <= 1080.5,
                    "{t:?} {}",
                    g.name
                );
            }
        }
    }

    #[test]
    fn ranges_are_round_in_display_units() {
        let (lo, hi) = default_range(Metric::Speed, None, UnitSystem::Imperial);
        assert_eq!(lo, 0.0);
        assert!((to_display(Metric::Speed, hi, UnitSystem::Imperial) - 40.0).abs() < 1e-9);
    }

    #[test]
    fn zones_are_sorted() {
        for zones in [hr_zones(190.0), power_zones(250.0), speed_zones(20.0)] {
            assert!(zones.windows(2).all(|w| w[0].from < w[1].from));
        }
    }
}
