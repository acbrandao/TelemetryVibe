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
    DigitalVmg,
    DigitalPace,
    DigitalAltitude,
    AltitudeTape,
    ElevationGraph,
    DigitalGrade,
    AviationAltitude,
    VerticalSpeedDial,
    DigitalVerticalSpeed,
    DigitalElevationGain,
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
    Compass,
    AttitudeIndicator,
    WindDial,
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
    DigitalGForce,
    DigitalLapDelta,
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
        PresetId::DigitalVmg,
        PresetId::DigitalPace,
        PresetId::DigitalAltitude,
        PresetId::AltitudeTape,
        PresetId::ElevationGraph,
        PresetId::DigitalGrade,
        PresetId::AviationAltitude,
        PresetId::VerticalSpeedDial,
        PresetId::DigitalVerticalSpeed,
        PresetId::DigitalElevationGain,
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
        PresetId::Compass,
        PresetId::AttitudeIndicator,
        PresetId::WindDial,
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
        PresetId::DigitalGForce,
        PresetId::DigitalLapDelta,
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
            PresetId::DigitalVmg => "Digital VMG",
            PresetId::AviationAltitude => "Altimeter",
            PresetId::VerticalSpeedDial => "Vertical speed indicator",
            PresetId::DigitalVerticalSpeed => "Digital vertical speed",
            PresetId::DigitalElevationGain => "Digital elevation gain",
            PresetId::Compass => "Compass",
            PresetId::AttitudeIndicator => "Artificial horizon",
            PresetId::WindDial => "Wind",
            PresetId::DigitalGForce => "Digital G-force",
            PresetId::DigitalLapDelta => "Lap delta",
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
            DigitalVmg => Metric::Vmg,
            DigitalAltitude | AltitudeTape | ElevationGraph | AviationAltitude => Metric::Altitude,
            VerticalSpeedDial | DigitalVerticalSpeed => Metric::VerticalSpeed,
            DigitalElevationGain => Metric::ElevationGain,
            DigitalGForce => Metric::GForce,
            DigitalLapDelta => Metric::LapDelta,
            AnalogHr | DigitalHr | HrBar | HrZone | HrGraph | HrPulse | HrEkg | HrZoneDial
            | HrLedRing => Metric::HeartRate,
            DigitalCadence | CircularCadence | CadenceBar => Metric::Cadence,
            DigitalPower | PowerGauge | PowerZone | PowerGraph | PowerZoneDial | PowerLedRing
            | PowerPeakDial | PowerSportDial => Metric::Power,
            DigitalTemperature | TemperatureBar => Metric::Temperature,
            DigitalDistance | DistanceProgress => Metric::Distance,
            HeadingTape | Compass | AttitudeIndicator => Metric::Heading,
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
                DigitalVmg,
            ],
        ),
        (
            "Altitude",
            vec![
                DigitalAltitude,
                AltitudeTape,
                AviationAltitude,
                ElevationGraph,
                DigitalElevationGain,
                VerticalSpeedDial,
                DigitalVerticalSpeed,
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
            vec![
                FullRouteMap,
                MovingRoute,
                TrailMap,
                CloseUpMap,
                Compass,
                HeadingTape,
                AttitudeIndicator,
                WindDial,
            ],
        ),
        ("Motorsport", vec![DigitalGForce, DigitalLapDelta]),
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
        Metric::ElevationGain => (
            0.0,
            nice_max(stats.map(|s| s.max).unwrap_or(f64::NAN), 500.0),
        ),
        Metric::Vmg => default_range(Metric::Speed, track, units),
        Metric::GForce => (0.0, 2.0),
        Metric::LapDelta => (-5.0, 5.0),
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
        distance_from_sync: false,
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
        AviationSpeed | AviationAltitude | VerticalSpeedDial => base(
            id,
            name,
            analog(DialStyle::Aviation, 300.0),
            metric,
            dial_size,
            dial_size,
            video,
        ),
        Compass => base(
            id,
            name,
            analog(DialStyle::Compass, 360.0),
            metric,
            dial_size,
            dial_size,
            video,
        ),
        AttitudeIndicator => base(
            id,
            name,
            analog(DialStyle::Horizon, 360.0),
            metric,
            dial_size,
            dial_size,
            video,
        ),
        WindDial => base(
            id,
            name,
            analog(DialStyle::Wind, 360.0),
            Metric::Custom,
            dial_size,
            dial_size,
            video,
        ),
        DigitalSpeed | DigitalPace | DigitalAltitude | DigitalHr | DigitalCadence
        | DigitalPower | DigitalTemperature | DigitalDistance | DigitalVmg
        | DigitalVerticalSpeed | DigitalElevationGain | DigitalGForce | DigitalLapDelta => {
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
        AviationAltitude => {
            g.style.background = Rgba::rgb(5, 5, 5);
            g.style.background_opacity = 0.85;
        }
        AttitudeIndicator => {
            g.style.label = "ATT".into();
            g.style.accent = Rgba::rgb(255, 200, 40);
            g.smoothing = 0.5;
        }
        WindDial => {
            // Wind comes from instruments, as custom fields (shows "--" when absent).
            g.custom_key = "wind_direction".into();
            g.min = 0.0;
            g.max = 360.0;
            g.style.label = "WIND".into();
            g.style.primary = Rgba::rgb(0, 200, 230);
            g.style.accent = Rgba::rgb(255, 90, 70);
        }
        VerticalSpeedDial | DigitalVerticalSpeed => {
            // Symmetric around zero so level flight points straight up.
            let fallback = if units == UnitSystem::Metric {
                300.0
            } else {
                1000.0
            };
            let peak = track
                .and_then(|t| t.stats.get(Metric::VerticalSpeed))
                .map(|s| s.min.abs().max(s.max.abs()))
                .filter(|v| *v > 0.0)
                .map(|v| nice_ceil(to_display(Metric::VerticalSpeed, v, units) * 1.1))
                .unwrap_or(fallback);
            let lim = to_si(Metric::VerticalSpeed, peak, units);
            g.min = -lim;
            g.max = lim;
            if preset == VerticalSpeedDial {
                g.style.background = Rgba::rgb(5, 5, 5);
                g.style.background_opacity = 0.85;
                // -limit, -limit/2, 0, +limit/2, +limit: round numbers for any nice limit.
                if let GaugeKind::Analog { major_ticks, .. } = &mut g.kind {
                    *major_ticks = 4;
                }
            }
        }
        DigitalVmg => {
            // Negative while moving away from the destination.
            g.min = -g.max;
        }
        DigitalLapDelta => {
            // Green while ahead of the best lap, red while behind.
            g.zones = vec![
                Zone {
                    from: -1.0e6,
                    color: Rgba::rgb(70, 210, 110),
                    label: "Ahead".into(),
                },
                Zone {
                    from: 0.0,
                    color: Rgba::rgb(240, 70, 60),
                    label: "Behind".into(),
                },
            ];
            g.zone_targets.number = true;
            g.zone_targets.bar = false;
            g.zone_targets.arc = false;
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
    if fit_digital_height(&mut g, dh, track, units) {
        g.placement.x = ((video.0 - g.placement.w) / 2.0).round();
        g.placement.y = ((video.1 - g.placement.h) / 2.0).round();
    }
    g
}

/// Gives a digital-style gauge (readout, icon readout, EKG, time) height `h` and the width its
/// content needs at that height. Returns false, changing nothing, for other kinds.
fn fit_digital_height(g: &mut Gauge, h: f32, track: Option<&Track>, units: UnitSystem) -> bool {
    if !matches!(
        g.kind,
        GaugeKind::Digital { .. } | GaugeKind::Ekg { .. } | GaugeKind::Time { .. }
    ) {
        return false;
    }
    g.placement.h = h.round();
    let sync = crate::telemetry::SyncSettings::default();
    let ctx = super::RenderCtx {
        track,
        sync: &sync,
        video_t: 0.0,
        units,
    };
    if let Some(nw) = super::digital::natural_width(g, &ctx) {
        g.placement.w = nw;
    }
    true
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Template {
    Cycling,
    Running,
    Sailing,
    Hiking,
    Motorsport,
    Aviation,
}

impl Template {
    pub const ALL: [Template; 6] = [
        Template::Cycling,
        Template::Running,
        Template::Sailing,
        Template::Hiking,
        Template::Motorsport,
        Template::Aviation,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Template::Cycling => "Cycling",
            Template::Running => "Running",
            Template::Sailing => "Sailing",
            Template::Hiking => "Hiking",
            Template::Motorsport => "Motorsport",
            Template::Aviation => "Aviation",
        }
    }
    pub fn description(self) -> &'static str {
        match self {
            Template::Cycling => {
                "Speedometer over a bottom row of power, distance, grade, cadence and heart rate; elevation profile, close-up map"
            }
            Template::Running => {
                "EKG heart rate over a bottom row of pace, distance, elapsed time, grade, altitude and cadence; elevation profile, close-up map"
            }
            Template::Sailing => {
                "Left-edge column of GPS track, VMG, heel, distance, speed dial, wind rose and compass; heading tape"
            }
            Template::Hiking => {
                "Large elevation and distance; elevation gain, grade, vertical speed, speed; compass, elevation profile"
            }
            Template::Motorsport => {
                "Speedometer and tachometer with throttle, brake, gear, G-force and heart rate; track map, lap time and delta"
            }
            Template::Aviation => {
                "HUD: speed, altitude and vertical-speed tapes, ground speed, artificial horizon; altitude profile, heading tape, GPS route"
            }
        }
    }
}

/// Per-activity look shared by every gauge of a template: one panel color and transparency, one
/// corner radius, one label color and one accent, so a template reads as a single system.
#[derive(Clone, Copy)]
struct Skin {
    panel: Rgba,
    opacity: f32,
    radius: f32,
    /// Labels, ticks and scale text.
    label: Rgba,
    accent: Rgba,
}

impl Template {
    fn skin(self) -> Skin {
        match self {
            Template::Cycling => Skin {
                panel: Rgba::rgb(12, 14, 18),
                opacity: 0.5,
                radius: 0.12,
                label: Rgba::rgba(255, 255, 255, 165),
                accent: Rgba::rgb(255, 106, 61),
            },
            Template::Running => Skin {
                panel: Rgba::rgb(10, 14, 20),
                opacity: 0.5,
                radius: 0.12,
                label: Rgba::rgba(200, 255, 120, 190),
                accent: Rgba::rgb(170, 240, 60),
            },
            Template::Sailing => Skin {
                panel: Rgba::rgb(6, 22, 40),
                opacity: 0.6,
                radius: 0.18,
                label: Rgba::rgba(150, 220, 255, 190),
                accent: Rgba::rgb(0, 200, 230),
            },
            Template::Hiking => Skin {
                panel: Rgba::rgb(20, 22, 14),
                opacity: 0.5,
                radius: 0.14,
                label: Rgba::rgba(255, 225, 160, 185),
                accent: Rgba::rgb(240, 180, 60),
            },
            Template::Motorsport => Skin {
                panel: Rgba::rgb(6, 6, 8),
                opacity: 0.62,
                radius: 0.06,
                label: Rgba::rgba(255, 255, 255, 160),
                accent: Rgba::rgb(255, 40, 30),
            },
            Template::Aviation => Skin {
                panel: Rgba::rgb(0, 0, 0),
                opacity: 0.4,
                radius: 0.08,
                label: Rgba::rgba(120, 255, 160, 200),
                accent: Rgba::rgb(90, 240, 140),
            },
        }
    }
}

fn apply_skin(g: &mut Gauge, skin: &Skin) {
    let st = &mut g.style;
    st.background = skin.panel;
    st.border_width = 0.0;
    match g.kind {
        // Dial faces stay a little more opaque so their scales read over busy footage.
        GaugeKind::Analog { .. } => st.background_opacity = skin.opacity.max(0.6),
        GaugeKind::Map { .. } => {
            st.background_opacity = skin.opacity;
            st.corner_radius = skin.radius;
            return;
        }
        _ => {
            st.background_opacity = skin.opacity;
            st.corner_radius = skin.radius;
            st.accent = skin.accent;
        }
    }
    st.secondary = skin.label;
}

/// Bottom edge of the edge strip templates.
fn bottom_e(lay: &Layout<'_>) -> f32 {
    lay.video.1 - lay.edge()
}

/// Readout height (in units) of the edge strip templates.
const STRIP_TILE: f32 = 0.088;
/// Taller readouts of the motorsport dash, a third of its speedometer.
const DASH_TILE: f32 = 0.11;

/// Shared template geometry. Everything is sized from one unit `u` — the height of the largest
/// 16:9 frame that fits the video — so a template scales with the resolution and keeps its
/// proportions on any aspect ratio. Gauges form tight clusters along the edges and the bottom
/// of the frame, leaving its middle clear.
struct Layout<'a> {
    video: (f32, f32),
    track: Option<&'a Track>,
    units: UnitSystem,
    skin: Skin,
    u: f32,
    /// Margin to the video edge.
    m: f32,
    /// Space between gauges inside a cluster.
    gap: f32,
}

impl<'a> Layout<'a> {
    fn new(t: Template, video: (f32, f32), track: Option<&'a Track>, units: UnitSystem) -> Self {
        let u = video.1.min(video.0 * 9.0 / 16.0).max(100.0);
        Self {
            video,
            track,
            units,
            skin: t.skin(),
            u,
            m: u * 0.035,
            gap: u * 0.01,
        }
    }

    fn right(&self) -> f32 {
        self.video.0 - self.m
    }

    fn bottom(&self) -> f32 {
        self.video.1 - self.m
    }

    /// A preset in the template's look, at `w`×`h`.
    fn sized(&self, p: PresetId, id: GaugeId, w: f32, h: f32) -> Gauge {
        let mut g = make_preset(p, id, self.video, self.track, self.units);
        apply_skin(&mut g, &self.skin);
        g.placement.w = w;
        g.placement.h = h;
        g
    }

    /// A square dial (or map) of side `size` units.
    fn dial(&self, p: PresetId, id: GaugeId, size: f32) -> Gauge {
        self.sized(p, id, size * self.u, size * self.u)
    }

    /// A readout `h` units tall, as wide as its content needs.
    fn readout(&self, p: PresetId, id: GaugeId, h: f32) -> Gauge {
        let mut g = self.sized(p, id, 0.0, 0.0);
        self.fit(&mut g, h * self.u);
        g
    }

    /// Refits a readout to height `h` pixels (after its binding or style changed). False,
    /// changing nothing, for gauges that are not readouts.
    fn fit(&self, g: &mut Gauge, h: f32) -> bool {
        fit_digital_height(g, h, self.track, self.units)
    }

    /// Full-activity elevation profile filling the bottom edge from `x` to the right margin.
    fn profile(&self, id: GaugeId, x: f32) -> Gauge {
        let h = 0.085 * self.u;
        let mut g = self.sized(PresetId::ElevationGraph, id, self.right() - x, h);
        if let GaugeKind::Graph {
            grid, line_width, ..
        } = &mut g.kind
        {
            *grid = false;
            *line_width = (self.u * 0.0022).max(1.5);
        }
        g.style.label = "ELEVATION".into();
        g.style.font_scale = 1.5;
        g.move_to(x, self.bottom() - h);
        g
    }

    /// Margin of the edge-hugging strip templates (cycling, running).
    fn edge(&self) -> f32 {
        0.012 * self.u
    }

    /// Elevation profile `w` units wide in the bottom-right corner of an edge strip, taller
    /// than its tiles.
    fn edge_profile(&self, id: GaugeId, w: f32) -> Gauge {
        let (w, h) = (w * self.u, 0.123 * self.u);
        let mut g = self.sized(PresetId::ElevationGraph, id, w, h);
        if let GaugeKind::Graph {
            grid, line_width, ..
        } = &mut g.kind
        {
            *grid = false;
            *line_width = (self.u * 0.0022).max(1.5);
        }
        g.style.label = "ELEVATION".into();
        g.style.font_scale = 1.25;
        let e = self.edge();
        g.move_to(self.video.0 - e - w, self.video.1 - e - h);
        g
    }

    /// Lays tiles `tile_h` units tall (as created) in one bottom-aligned row between `x0` and
    /// `limit`, with one gap between
    /// every neighbor (and before `limit` when `fill`ing up to a profile there). With `fill`
    /// the tiles widen and the gaps open up (to a limit) to span the space; otherwise they keep
    /// their natural width. When the tiles do not fit they all shrink to one smaller height.
    /// Tiles are never narrower than tall. Returns the row top, the gap and the tile height,
    /// for gauges stacked above.
    fn strip(
        &self,
        row: &mut [Gauge],
        tile_h: f32,
        x0: f32,
        limit: f32,
        fill: bool,
    ) -> (f32, f32, f32) {
        let e = self.edge();
        let n = row.len() as f32;
        let avail = limit - x0;
        let mut h = tile_h * self.u;
        let natural = row_width(row, 0.0);
        if natural + n * self.gap > avail {
            // Tile widths are proportional to their height.
            let k = (avail - n * self.gap) / natural;
            h *= k;
            for g in row.iter_mut() {
                if !self.fit(g, h) {
                    g.placement.w *= k;
                    g.placement.h = h;
                }
            }
        }
        for g in row.iter_mut() {
            g.placement.w = g.placement.w.max(h);
        }
        let natural = row_width(row, 0.0);
        let gap = if fill {
            ((avail - natural) / n).clamp(self.gap, 0.025 * self.u)
        } else {
            self.gap
        };
        if fill {
            spread(row, avail - gap, gap);
        }
        let top = self.video.1 - e - h;
        row_at(row, x0, top, gap);
        (top, gap, h)
    }

    /// Close-up, north-up moving map with a scale bar in the top-right corner of an edge strip
    /// template, on a lighter panel.
    fn edge_map(&self, id: GaugeId, size: f32) -> Gauge {
        let mut g = self.dial(PresetId::CloseUpMap, id, size);
        if let GaugeKind::Map {
            speed_colors,
            heading_up,
            ..
        } = &mut g.kind
        {
            *speed_colors = false;
            *heading_up = false;
        }
        g.zones.clear();
        g.style.background_opacity = 0.35;
        g.style.primary = Rgba::rgb(255, 106, 61);
        let e = self.edge();
        g.move_to(self.video.0 - e - g.placement.w, 2.0 * e);
        g
    }

    fn top_right(&self, g: &mut Gauge) {
        let x = self.right() - g.placement.w;
        g.move_to(x, self.m);
    }
}

impl Gauge {
    fn move_to(&mut self, x: f32, y: f32) {
        self.placement.x = x;
        self.placement.y = y;
    }
}

/// Total width of gauges laid side by side.
fn row_width(gs: &[Gauge], gap: f32) -> f32 {
    gs.iter().map(|g| g.placement.w).sum::<f32>() + gap * gs.len().saturating_sub(1) as f32
}

/// Widens gauges in proportion to their width so the row spans `target` (never narrows).
fn spread(gs: &mut [Gauge], target: f32, gap: f32) {
    let w = row_width(gs, gap);
    let content = w - gap * gs.len().saturating_sub(1) as f32;
    if w < target && content > 0.0 {
        let k = (target - w + content) / content;
        for g in gs {
            g.placement.w *= k;
        }
    }
}

/// Lays gauges left to right from `x`, tops at `y`.
fn row_at(gs: &mut [Gauge], x: f32, y: f32, gap: f32) {
    let mut x = x;
    for g in gs {
        g.move_to(x, y);
        x += g.placement.w + gap;
    }
}

/// Stacks gauges downward from `y`, all as wide as `w`.
fn column_at(gs: &mut [Gauge], x: f32, y: f32, w: f32, gap: f32) {
    let mut y = y;
    for g in gs {
        g.placement.w = w;
        g.move_to(x, y);
        y += g.placement.h + gap;
    }
}

fn widest(gs: &[Gauge]) -> f32 {
    gs.iter().map(|g| g.placement.w).fold(0.0, f32::max)
}

/// Binds a gauge to a non-standard field from data loggers (shows "--" when absent).
fn bind_custom(g: &mut Gauge, key: &str, label: &str, min: f64, max: f64) {
    g.name = label.to_string();
    g.metric = Metric::Custom;
    g.custom_key = key.into();
    g.min = min;
    g.max = max;
    g.zones.clear();
    g.style.label = label.to_uppercase();
    g.style.decimals = 0;
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
    let lay = Layout::new(t, video, track, units);
    let (u, m, gap) = (lay.u, lay.m, lay.gap);
    let bottom = lay.bottom();
    let mut out = Vec::new();
    match t {
        // Hugging the frame edges: one row of readouts across the bottom (power, distance,
        // grade, cadence, heart rate) ending in a taller elevation profile, the speedometer
        // sitting on the row's left end, and a close-up moving map top-right.
        Template::Cycling => {
            let profile = lay.edge_profile(next_id(), 0.42);
            let mut row = [
                lay.readout(DigitalPower, next_id(), STRIP_TILE),
                lay.readout(DigitalDistance, next_id(), STRIP_TILE),
                lay.readout(DigitalGrade, next_id(), STRIP_TILE),
                lay.readout(DigitalCadence, next_id(), STRIP_TILE),
                lay.readout(HrPulse, next_id(), STRIP_TILE),
            ];
            let (row_top, strip_gap, _) =
                lay.strip(&mut row, STRIP_TILE, lay.edge(), profile.placement.x, true);
            let d = 0.26 * u;
            let mut speed = lay.sized(SportSpeed, next_id(), d, d);
            speed.name = "Speedometer".into();
            speed.move_to(lay.edge(), row_top - strip_gap - d);
            out.push(speed);
            out.extend(row);
            out.extend([profile, lay.edge_map(next_id(), 0.22)]);
        }
        // Same edge strip as cycling: pace, distance, elapsed time, grade, altitude and
        // cadence ending in the elevation profile, with an EKG heart-rate monitor stacked on
        // the pace tile and a close-up moving map top-right.
        Template::Running => {
            let profile = lay.edge_profile(next_id(), 0.36);
            let mut ekg = lay.readout(HrEkg, next_id(), STRIP_TILE);
            let mut pace = lay.readout(DigitalPace, next_id(), STRIP_TILE);
            pace.placement.w = pace.placement.w.max(ekg.placement.w);
            let mut elapsed = lay.readout(RecordingTime, next_id(), STRIP_TILE);
            elapsed.name = "Elapsed time".into();
            elapsed.style.label = "ELAPSED".into();
            let mut row = [
                pace,
                lay.readout(DigitalDistance, next_id(), STRIP_TILE),
                elapsed,
                lay.readout(DigitalGrade, next_id(), STRIP_TILE),
                lay.readout(DigitalAltitude, next_id(), STRIP_TILE),
                lay.readout(DigitalCadence, next_id(), STRIP_TILE),
            ];
            let (row_top, strip_gap, tile_h) =
                lay.strip(&mut row, STRIP_TILE, lay.edge(), profile.placement.x, true);
            // As tall as the row and as wide as the pace tile beneath it.
            lay.fit(&mut ekg, tile_h);
            ekg.placement.w = row[0].placement.w;
            ekg.move_to(lay.edge(), row_top - strip_gap - ekg.placement.h);
            out.push(ekg);
            out.extend(row);
            out.extend([profile, lay.edge_map(next_id(), 0.22)]);
        }
        // One instrument column down the left edge: GPS track, then VMG, heel and distance at
        // the map's width, the speed dial, and the wind rose and heading compass side by side
        // at the bottom. A heading tape spans the top from the map to the right edge.
        Template::Sailing => {
            let e = lay.edge();
            let tile = 0.07;
            let mut heel = lay.readout(DigitalCadence, next_id(), tile);
            bind_custom(&mut heel, "heel", "Heel", -45.0, 45.0);
            heel.style.suffix = "°".into();
            heel.style.primary = Rgba::WHITE;
            lay.fit(&mut heel, tile * u);
            let mut tiles = [
                lay.readout(DigitalVmg, next_id(), tile),
                heel,
                lay.readout(DigitalDistance, next_id(), tile),
            ];
            let col_w = widest(&tiles).max(0.23 * u);

            let mut track_map = lay.sized(FullRouteMap, next_id(), col_w, col_w);
            track_map.name = "GPS track".into();
            track_map.move_to(e, e);
            let tiles_y = e + col_w + gap;
            column_at(&mut tiles, e, tiles_y, col_w, gap);

            // Dials from the bottom up: wind and heading, then speed above them.
            let c = 0.155 * u;
            let mut small = [
                lay.sized(WindDial, next_id(), c, c),
                lay.sized(Compass, next_id(), c, c),
            ];
            row_at(&mut small, e, bottom_e(&lay) - c, gap);
            let d = 0.25 * u;
            let mut speed = lay.sized(AviationSpeed, next_id(), d, d);
            speed.name = "Boat speed".into();
            speed.style.label = "SPEED".into();
            speed.move_to(e, bottom_e(&lay) - c - gap - d);

            let tape_x = e + col_w + gap;
            let mut heading = lay.sized(HeadingTape, next_id(), video.0 - e - tape_x, 0.075 * u);
            heading.style.primary = Rgba::WHITE;
            heading.style.accent = Rgba::rgb(255, 90, 70);
            heading.move_to(tape_x, e);

            out.extend([track_map, heading]);
            out.extend(tiles);
            out.push(speed);
            out.extend(small);
        }
        // Bottom-left: large elevation and distance with climbing and speed readouts above;
        // compass on its own in the top-right corner; elevation profile along the bottom.
        Template::Hiking => {
            let p = 0.13;
            let mut primary = [
                lay.readout(DigitalAltitude, next_id(), p),
                lay.readout(DigitalDistance, next_id(), p),
            ];
            let mut second = [
                lay.readout(DigitalElevationGain, next_id(), 0.065),
                lay.readout(DigitalGrade, next_id(), 0.065),
                lay.readout(DigitalVerticalSpeed, next_id(), 0.065),
                lay.readout(DigitalSpeed, next_id(), 0.065),
            ];
            let w = row_width(&primary, gap).max(row_width(&second, gap));
            spread(&mut primary, w, gap);
            spread(&mut second, w, gap);
            let top = bottom - p * u;
            row_at(&mut primary, m, top, gap);
            let second_y = top - gap - second[0].placement.h;
            row_at(&mut second, m, second_y, gap);
            out.extend(primary);
            out.extend(second);
            out.push(lay.profile(next_id(), m + w + 3.0 * gap));
            let mut compass = lay.dial(Compass, next_id(), 0.2);
            lay.top_right(&mut compass);
            out.push(compass);
        }
        // Edge-hugging race dash: a large speedometer in the bottom-left corner with the
        // tachometer beside it, then a bottom row of throttle and brake, gear, G-force and heart
        // rate. Close-up track map top-right with lap time and lap delta beneath it.
        Template::Motorsport => {
            let e = lay.edge();
            let tile = DASH_TILE * u;
            let d = 0.32 * u;
            let mut speed = lay.sized(MotorsportSpeed, next_id(), d, d);
            speed.name = "Speedometer".into();
            speed.move_to(e, bottom_e(&lay) - d);
            // RPM, gear, throttle and brake come from car data loggers, as custom fields.
            let r = 0.22 * u;
            let mut rpm = lay.sized(MotorsportSpeed, next_id(), r, r);
            bind_custom(&mut rpm, "rpm", "RPM", 0.0, 10000.0);
            rpm.zones = vec![Zone {
                from: 8500.0,
                color: Rgba::rgb(235, 40, 40),
                label: "Redline".into(),
            }];
            rpm.move_to(e + d + gap, bottom_e(&lay) - r);

            let pedal = |mut g: Gauge, key: &str, label: &str, color: Rgba| {
                bind_custom(&mut g, key, label, 0.0, 100.0);
                g.kind = GaugeKind::Bar {
                    orientation: Orientation::Horizontal,
                    segments: 0,
                    rounded: false,
                    thickness: 0.35,
                    show_value: false,
                };
                g.style.primary = color;
                g.placement.w = 1.9 * tile;
                g.placement.h = tile;
                g
            };
            let throttle = pedal(
                lay.sized(CadenceBar, next_id(), 0.0, 0.0),
                "throttle",
                "Throttle",
                Rgba::rgb(70, 210, 110),
            );
            let brake = pedal(
                lay.sized(CadenceBar, next_id(), 0.0, 0.0),
                "brake",
                "Brake",
                Rgba::rgb(240, 60, 50),
            );
            let mut gear = lay.readout(DigitalCadence, next_id(), DASH_TILE);
            bind_custom(&mut gear, "gear", "Gear", 0.0, 8.0);
            gear.style.primary = Rgba::WHITE;
            gear.style.show_units = false;
            gear.kind = GaugeKind::Digital {
                align: TextAlign::Center,
                icon: DigitalIcon::None,
            };
            lay.fit(&mut gear, tile);
            let mut hr = lay.readout(DigitalHr, next_id(), DASH_TILE);
            hr.style.primary = Rgba::WHITE;
            let mut row = [
                throttle,
                brake,
                gear,
                lay.readout(DigitalGForce, next_id(), DASH_TILE),
                hr,
            ];
            lay.strip(
                &mut row,
                DASH_TILE,
                e + d + gap + r + gap,
                video.0 - e,
                false,
            );
            out.extend([speed, rpm]);
            out.extend(row);

            let mut track_map = lay.edge_map(next_id(), 0.24);
            track_map.name = "Track map".into();
            let mut lap = [
                lay.readout(LapTime, next_id(), 0.075),
                lay.readout(DigitalLapDelta, next_id(), 0.075),
            ];
            let lap_w = widest(&lap).max(track_map.placement.w);
            track_map.placement.w = lap_w;
            track_map.move_to(video.0 - e - lap_w, track_map.placement.y);
            let lap_y = track_map.placement.y + track_map.placement.h + gap;
            column_at(&mut lap, video.0 - e - lap_w, lap_y, lap_w, gap);
            out.push(track_map);
            out.extend(lap);
        }
        // Edge-hugging HUD: speed tape on the left edge with ground speed at its foot,
        // altitude and vertical-speed tapes on the right edge, all three standing on the
        // bottom edge at one height; artificial horizon bottom-center; an altitude profile and
        // the heading tape side by side along the top; GPS route in the top-right corner.
        Template::Aviation => {
            let e = lay.edge();
            let bottom = bottom_e(&lay);
            let tape_h = 0.46 * u;
            let tape_top = bottom - tape_h;
            let mut airspeed = lay.sized(SpeedTape, next_id(), 0.1 * u, tape_h);
            airspeed.name = "Airspeed".into();
            airspeed.style.label = "SPD".into();
            let mut altitude = lay.sized(AltitudeTape, next_id(), 0.115 * u, tape_h);
            altitude.style.label = "ALT".into();
            let mut vsi = lay.sized(DigitalVerticalSpeed, next_id(), 0.095 * u, tape_h);
            vsi.name = "Vertical speed".into();
            vsi.style.label = "V/S".into();
            vsi.style.font_scale = 0.8;
            let span = display_span(&vsi, units);
            vsi.kind = tape(Orientation::Vertical, span);
            for g in [&mut airspeed, &mut altitude, &mut vsi] {
                g.style.primary = lay.skin.accent;
            }
            airspeed.move_to(e, tape_top);
            vsi.move_to(video.0 - e - vsi.placement.w, tape_top);
            altitude.move_to(vsi.placement.x - gap - altitude.placement.w, tape_top);
            let mut gs = lay.readout(DigitalSpeed, next_id(), 0.065);
            gs.name = "Ground speed".into();
            gs.style.label = "GS".into();
            gs.move_to(e + airspeed.placement.w + gap, bottom - gs.placement.h);

            let mut horizon = lay.dial(AttitudeIndicator, next_id(), 0.29);
            horizon.move_to(
                (video.0 - horizon.placement.w) / 2.0,
                bottom - horizon.placement.h,
            );

            // Top pair, centered: altitude profile and heading tape at one height.
            let top_h = 0.085 * u;
            let mut profile = lay.sized(ElevationGraph, next_id(), 0.4 * u, top_h);
            profile.name = "Altitude profile".into();
            profile.style.label = "ALTITUDE".into();
            let mut heading = lay.sized(HeadingTape, next_id(), 0.42 * u, top_h);
            heading.style.primary = lay.skin.accent;
            let mut top = [profile, heading];
            let x0 = (video.0 - row_width(&top, gap)) / 2.0;
            row_at(&mut top, x0, e, gap);

            let mut route = lay.dial(FullRouteMap, next_id(), 0.2);
            route.name = "GPS route".into();
            route.style.background_opacity = 0.35;
            route.move_to(video.0 - e - route.placement.w, e);
            out.extend([airspeed, gs, altitude, vsi, horizon]);
            out.extend(top);
            out.push(route);
        }
    }
    for g in &mut out {
        let p = &mut g.placement;
        p.x = p.x.round();
        p.y = p.y.round();
        p.w = p.w.round();
        p.h = p.h.round();
    }
    out
}

/// A gauge's full range in display units (tape span that shows the whole scale).
fn display_span(g: &Gauge, units: UnitSystem) -> f64 {
    let u = g.units.resolve(units);
    (to_display(g.metric, g.max, u) - to_display(g.metric, g.min, u)).abs()
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
        for (vw, vh) in [(1920.0, 1080.0), (1080.0, 1920.0), (1440.0, 1080.0)] {
            for t in Template::ALL {
                let gauges = apply_template(t, (vw, vh), None, UnitSystem::Metric, &mut next);
                assert!(gauges.len() >= 4, "{t:?}");
                for g in &gauges {
                    let p = g.placement;
                    assert!(p.x >= 0.0 && p.y >= 0.0, "{t:?} {vw}x{vh} {}", g.name);
                    assert!(
                        p.x + p.w <= vw + 0.5 && p.y + p.h <= vh + 0.5,
                        "{t:?} {vw}x{vh} {}",
                        g.name
                    );
                }
                // No gauge covers another.
                for (i, a) in gauges.iter().enumerate() {
                    for b in &gauges[i + 1..] {
                        let (p, q) = (a.placement, b.placement);
                        let overlap = p.x < q.x + q.w
                            && q.x < p.x + p.w
                            && p.y < q.y + q.h
                            && q.y < p.y + p.h;
                        assert!(!overlap, "{t:?} {vw}x{vh}: {} overlaps {}", a.name, b.name);
                    }
                }
            }
        }
    }

    #[test]
    fn templates_keep_the_center_clear() {
        let mut n = 0;
        let mut next = || {
            n += 1;
            GaugeId(n)
        };
        for (vw, vh) in [(1920.0, 1080.0), (1080.0, 1920.0), (1440.0, 1080.0)] {
            for t in Template::ALL {
                for g in apply_template(t, (vw, vh), None, UnitSystem::Metric, &mut next) {
                    let p = g.placement;
                    let covers_center = p.x < vw * 0.6
                        && p.x + p.w > vw * 0.4
                        && p.y < vh * 0.6
                        && p.y + p.h > vh * 0.4;
                    assert!(!covers_center, "{t:?} {vw}x{vh} {}", g.name);
                }
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
