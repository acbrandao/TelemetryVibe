//! Unit systems and conversion between canonical (SI) values and display values.
//!
//! All telemetry is stored canonically:
//! speed m/s, altitude/distance m, temperature °C, pace s/km, grade %, heading °.

use serde::{Deserialize, Serialize};

use super::model::Metric;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnitSystem {
    #[default]
    Metric,
    Imperial,
}

impl UnitSystem {
    pub fn label(self) -> &'static str {
        match self {
            UnitSystem::Metric => "Metric",
            UnitSystem::Imperial => "Imperial",
        }
    }
}

/// Per-gauge unit preference. Gauges inherit the project setting by default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnitPref {
    #[default]
    Inherit,
    Metric,
    Imperial,
}

impl UnitPref {
    pub fn resolve(self, global: UnitSystem) -> UnitSystem {
        match self {
            UnitPref::Inherit => global,
            UnitPref::Metric => UnitSystem::Metric,
            UnitPref::Imperial => UnitSystem::Imperial,
        }
    }
}

const M_PER_MILE: f64 = 1609.344;
const M_PER_FOOT: f64 = 0.3048;
const KMH_PER_MS: f64 = 3.6;
const MPH_PER_MS: f64 = 3600.0 / M_PER_MILE;

/// Converts a canonical value to its display value.
pub fn to_display(metric: Metric, si: f64, units: UnitSystem) -> f64 {
    match (metric, units) {
        (Metric::Speed | Metric::Vmg, UnitSystem::Metric) => si * KMH_PER_MS,
        (Metric::Speed | Metric::Vmg, UnitSystem::Imperial) => si * MPH_PER_MS,
        (Metric::Altitude | Metric::GpsAltitude | Metric::ElevationGain, UnitSystem::Imperial) => si / M_PER_FOOT,
        (Metric::Distance, UnitSystem::Metric) => si / 1000.0,
        (Metric::Distance, UnitSystem::Imperial) => si / M_PER_MILE,
        (Metric::Temperature, UnitSystem::Imperial) => si * 9.0 / 5.0 + 32.0,
        (Metric::Pace, UnitSystem::Imperial) => si * M_PER_MILE / 1000.0,
        (Metric::VerticalSpeed, UnitSystem::Imperial) => si / M_PER_FOOT * 60.0,
        (Metric::VerticalSpeed, UnitSystem::Metric) => si * 60.0,
        _ => si,
    }
}

/// Converts a display value back to canonical.
pub fn to_si(metric: Metric, display: f64, units: UnitSystem) -> f64 {
    match (metric, units) {
        (Metric::Speed | Metric::Vmg, UnitSystem::Metric) => display / KMH_PER_MS,
        (Metric::Speed | Metric::Vmg, UnitSystem::Imperial) => display / MPH_PER_MS,
        (Metric::Altitude | Metric::GpsAltitude | Metric::ElevationGain, UnitSystem::Imperial) => display * M_PER_FOOT,
        (Metric::Distance, UnitSystem::Metric) => display * 1000.0,
        (Metric::Distance, UnitSystem::Imperial) => display * M_PER_MILE,
        (Metric::Temperature, UnitSystem::Imperial) => (display - 32.0) * 5.0 / 9.0,
        (Metric::Pace, UnitSystem::Imperial) => display * 1000.0 / M_PER_MILE,
        (Metric::VerticalSpeed, UnitSystem::Imperial) => display * M_PER_FOOT / 60.0,
        (Metric::VerticalSpeed, UnitSystem::Metric) => display / 60.0,
        _ => display,
    }
}

/// Converts a canonical *difference* (e.g. a tape span) — temperature offsets are not shifted.
pub fn delta_to_display(metric: Metric, si: f64, units: UnitSystem) -> f64 {
    if metric == Metric::Temperature {
        if units == UnitSystem::Imperial {
            si * 9.0 / 5.0
        } else {
            si
        }
    } else {
        to_display(metric, si, units)
    }
}

/// Unit label for display (`km/h`, `mph`, `ft`, ...).
pub fn unit_label(metric: Metric, units: UnitSystem) -> &'static str {
    match (metric, units) {
        (Metric::Speed | Metric::Vmg, UnitSystem::Metric) => "km/h",
        (Metric::Speed | Metric::Vmg, UnitSystem::Imperial) => "mph",
        (Metric::Altitude | Metric::GpsAltitude | Metric::ElevationGain, UnitSystem::Metric) => "m",
        (Metric::Altitude | Metric::GpsAltitude | Metric::ElevationGain, UnitSystem::Imperial) => "ft",
        (Metric::Distance, UnitSystem::Metric) => "km",
        (Metric::Distance, UnitSystem::Imperial) => "mi",
        (Metric::Temperature, UnitSystem::Metric) => "°C",
        (Metric::Temperature, UnitSystem::Imperial) => "°F",
        (Metric::Pace, UnitSystem::Metric) => "/km",
        (Metric::Pace, UnitSystem::Imperial) => "/mi",
        (Metric::VerticalSpeed, UnitSystem::Metric) => "m/min",
        (Metric::VerticalSpeed, UnitSystem::Imperial) => "ft/min",
        (Metric::HeartRate, _) => "bpm",
        (Metric::Cadence, _) => "rpm",
        (Metric::Power, _) => "W",
        (Metric::Grade, _) => "%",
        (Metric::Heading, _) => "°",
        (Metric::GForce, _) => "g",
        (Metric::LapDelta, _) => "s",
        (Metric::Latitude, _) | (Metric::Longitude, _) => "°",
        (Metric::Custom, _) => "",
    }
}

/// Formats a display value with the given number of decimals, adding thousands separators.
/// Pace values are rendered as `m:ss`.
pub fn format_value(metric: Metric, display: f64, decimals: u8) -> String {
    if !display.is_finite() {
        return "--".to_string();
    }
    if metric == Metric::Pace {
        let total = display.round().max(0.0) as u64;
        return format!("{}:{:02}", total / 60, total % 60);
    }
    let decimals = decimals.min(6) as usize;
    let s = format!("{:.*}", decimals, display);
    // Lap delta always shows its sign: + behind the best lap, - ahead of it.
    if metric == Metric::LapDelta && !s.starts_with('-') {
        return format!("+{}", group_thousands(&s));
    }
    group_thousands(&s)
}

fn group_thousands(s: &str) -> String {
    let (sign, rest) = if let Some(r) = s.strip_prefix('-') {
        ("-", r)
    } else {
        ("", s)
    };
    let (int_part, frac) = match rest.find('.') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, ""),
    };
    if int_part.len() <= 3 {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len() + int_part.len() / 3);
    for (i, ch) in int_part.chars().enumerate() {
        if i > 0 && (int_part.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    format!("{sign}{out}{frac}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speed_conversions() {
        let ms = 10.0;
        assert!((to_display(Metric::Speed, ms, UnitSystem::Metric) - 36.0).abs() < 1e-9);
        assert!(
            (to_display(Metric::Speed, ms, UnitSystem::Imperial) - 22.369362920544).abs() < 1e-6
        );
        for u in [UnitSystem::Metric, UnitSystem::Imperial] {
            for m in [
                Metric::Speed,
                Metric::Altitude,
                Metric::GpsAltitude,
                Metric::Distance,
                Metric::Temperature,
                Metric::Pace,
                Metric::VerticalSpeed,
            ] {
                let back = to_si(m, to_display(m, 12.5, u), u);
                assert!((back - 12.5).abs() < 1e-9, "{m:?} {u:?}");
            }
        }
    }

    #[test]
    fn temperature() {
        assert!(
            (to_display(Metric::Temperature, 100.0, UnitSystem::Imperial) - 212.0).abs() < 1e-9
        );
        assert!(
            (delta_to_display(Metric::Temperature, 10.0, UnitSystem::Imperial) - 18.0).abs() < 1e-9
        );
    }

    #[test]
    fn formatting() {
        assert_eq!(format_value(Metric::Altitude, 1245.2, 0), "1,245");
        assert_eq!(format_value(Metric::Speed, 32.44, 1), "32.4");
        assert_eq!(format_value(Metric::Pace, 305.0, 0), "5:05");
        assert_eq!(format_value(Metric::Power, -1234567.0, 0), "-1,234,567");
        assert_eq!(format_value(Metric::Power, f64::NAN, 0), "--");
        assert_eq!(format_value(Metric::LapDelta, 1.234, 2), "+1.23");
        assert_eq!(format_value(Metric::LapDelta, -0.5, 2), "-0.50");
    }
}
