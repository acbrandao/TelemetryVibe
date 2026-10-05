//! Normalized telemetry data model.
//!
//! Every parser produces [`RawSample`]s which [`TrackBuilder`] turns into a columnar [`Track`]:
//! sorted, de-duplicated, with derived channels (distance, speed, heading, grade, pace ...),
//! statistics, a projected route and detected events used for synchronization.
//! Missing values are stored as `NaN` and never assumed to exist.

use std::collections::BTreeMap;

use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};

use super::interpolation::{InterpKind, sample_channel};
use super::smoothing;

/// A telemetry quantity a gauge can bind to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Metric {
    Speed,
    Pace,
    Altitude,
    /// Altitude reported by the GPS receiver (FIT `gps_metadata`), when the file has it.
    /// `Altitude` is the device's primary altitude (barometric on most Garmin devices).
    GpsAltitude,
    Distance,
    HeartRate,
    Cadence,
    Power,
    Temperature,
    Heading,
    Grade,
    VerticalSpeed,
    /// Cumulative climbing since the start (m), derived from altitude.
    ElevationGain,
    /// Velocity made good toward the end of the recording (m/s), derived from GPS.
    Vmg,
    /// Combined lateral + longitudinal acceleration (g), derived from GPS speed and heading.
    GForce,
    /// Seconds behind (+) or ahead of (-) the fastest lap at the same distance into the lap.
    LapDelta,
    Latitude,
    Longitude,
    /// A non-standard field, identified by the gauge's `custom_key`.
    Custom,
}

impl Metric {
    pub const BUILTIN: [Metric; 18] = [
        Metric::Speed,
        Metric::Pace,
        Metric::GpsAltitude,
        Metric::Altitude,
        Metric::Distance,
        Metric::HeartRate,
        Metric::Cadence,
        Metric::Power,
        Metric::Temperature,
        Metric::Heading,
        Metric::Grade,
        Metric::VerticalSpeed,
        Metric::ElevationGain,
        Metric::Vmg,
        Metric::GForce,
        Metric::LapDelta,
        Metric::Latitude,
        Metric::Longitude,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Metric::Speed => "Speed",
            Metric::Pace => "Pace",
            Metric::Altitude => "Altitude",
            Metric::GpsAltitude => "GPS Altitude",
            Metric::Distance => "Distance",
            Metric::HeartRate => "Heart Rate",
            Metric::Cadence => "Cadence",
            Metric::Power => "Power",
            Metric::Temperature => "Temperature",
            Metric::Heading => "Heading",
            Metric::Grade => "Gradient",
            Metric::VerticalSpeed => "Vertical Speed",
            Metric::ElevationGain => "Elevation Gain",
            Metric::Vmg => "VMG",
            Metric::GForce => "G-Force",
            Metric::LapDelta => "Lap Delta",
            Metric::Latitude => "Latitude",
            Metric::Longitude => "Longitude",
            Metric::Custom => "Custom",
        }
    }

    /// Short uppercase label used on gauges.
    pub fn short_label(self) -> &'static str {
        match self {
            Metric::Speed => "SPEED",
            Metric::Pace => "PACE",
            Metric::Altitude | Metric::GpsAltitude => "ALTITUDE",
            Metric::Distance => "DISTANCE",
            Metric::HeartRate => "HEART RATE",
            Metric::Cadence => "CADENCE",
            Metric::Power => "POWER",
            Metric::Temperature => "TEMP",
            Metric::Heading => "HEADING",
            Metric::Grade => "GRADE",
            Metric::VerticalSpeed => "VERT SPEED",
            Metric::ElevationGain => "ELEV GAIN",
            Metric::Vmg => "VMG",
            Metric::GForce => "G-FORCE",
            Metric::LapDelta => "LAP DELTA",
            Metric::Latitude => "LAT",
            Metric::Longitude => "LON",
            Metric::Custom => "",
        }
    }

    /// How values of this metric are interpolated between samples.
    pub fn interp_kind(self) -> InterpKind {
        match self {
            Metric::Heading => InterpKind::Circular,
            Metric::Altitude | Metric::GpsAltitude => InterpKind::Cubic,
            _ => InterpKind::Linear,
        }
    }

    /// Default number of decimals for display.
    pub fn default_decimals(self) -> u8 {
        match self {
            Metric::Speed | Metric::Vmg | Metric::Distance | Metric::Grade | Metric::Temperature => {
                1
            }
            Metric::GForce | Metric::LapDelta => 2,
            Metric::Latitude | Metric::Longitude => 5,
            _ => 0,
        }
    }

    /// Altitude-like metrics share units, ranges and formatting.
    pub fn is_altitude(self) -> bool {
        matches!(self, Metric::Altitude | Metric::GpsAltitude)
    }

    /// The source to use for altitude gauges: GPS altitude when the track has it.
    pub fn preferred_altitude(track: Option<&Track>) -> Metric {
        if track.is_some_and(|t| t.has(Metric::GpsAltitude)) {
            Metric::GpsAltitude
        } else {
            Metric::Altitude
        }
    }
}

/// One raw sample as produced by a file parser. All values are canonical SI units.
#[derive(Clone, Debug, Default)]
pub struct RawSample {
    /// Seconds since the Unix epoch (fractional).
    pub time: f64,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub altitude: Option<f64>,
    /// Altitude from the GPS receiver (m), when recorded separately.
    pub gps_altitude: Option<f64>,
    /// m/s
    pub speed: Option<f64>,
    /// m, cumulative
    pub distance: Option<f64>,
    pub heart_rate: Option<f64>,
    pub cadence: Option<f64>,
    pub power: Option<f64>,
    /// °C
    pub temperature: Option<f64>,
    /// degrees clockwise from north
    pub heading: Option<f64>,
    /// percent
    pub grade: Option<f64>,
    /// Any other numeric fields found in the source file.
    pub extra: Vec<(String, f64)>,
}

/// A recorded lap.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Lap {
    /// Seconds from track start.
    pub start: f64,
    pub end: f64,
    pub distance: Option<f64>,
}

/// One sport segment of a recording (several in a multi-sport activity such as a triathlon).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Session {
    /// Seconds from track start.
    pub start: f64,
    pub end: f64,
    pub sport: Option<String>,
}

/// An automatically detected moment useful for visual synchronization.
#[derive(Clone, Debug)]
pub struct DetectedEvent {
    pub t: f64,
    pub kind: EventKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventKind {
    StartMoving,
    Stop,
    MaxSpeed,
    LapStart,
    /// Start of a new sport segment in a multi-sport activity.
    SportStart,
}

impl EventKind {
    pub fn label(self) -> &'static str {
        match self {
            EventKind::StartMoving => "Starts moving",
            EventKind::Stop => "Stops",
            EventKind::MaxSpeed => "Max speed",
            EventKind::LapStart => "Lap start",
            EventKind::SportStart => "Sport change",
        }
    }
}

/// Min/max/average of a channel.
#[derive(Clone, Copy, Debug, Default)]
pub struct ChannelStats {
    pub min: f64,
    pub max: f64,
    pub avg: f64,
    pub count: usize,
}

/// Summary statistics of a track.
#[derive(Clone, Debug, Default)]
pub struct TrackStats {
    pub per_metric: BTreeMap<Metric, ChannelStats>,
    pub elevation_gain: Option<f64>,
    pub elevation_loss: Option<f64>,
    pub total_distance: Option<f64>,
    pub moving_time: f64,
    pub duration: f64,
}

impl TrackStats {
    pub fn get(&self, m: Metric) -> Option<&ChannelStats> {
        self.per_metric.get(&m)
    }
}

/// Route projected into a local planar frame (meters), y pointing south (screen down).
#[derive(Clone, Debug)]
pub struct Route {
    /// Simplified display polyline (meters, origin top-left of the bounding box).
    pub points: Vec<[f64; 2]>,
    /// Track time of each point in `points`.
    pub times: Vec<f64>,
    pub width: f64,
    pub height: f64,
    lon_min: f64,
    lat_max: f64,
    cos_lat: f64,
}

const EARTH_R: f64 = 6_371_000.0;

impl Route {
    pub fn project(&self, lat: f64, lon: f64) -> [f64; 2] {
        [
            (lon - self.lon_min).to_radians() * self.cos_lat * EARTH_R,
            (self.lat_max - lat).to_radians() * EARTH_R,
        ]
    }

    /// Index of the last display point at or before `t`.
    pub fn index_at(&self, t: f64) -> usize {
        self.times.partition_point(|&x| x <= t).saturating_sub(1)
    }
}

/// Normalized, interpolatable telemetry snapshot at one instant (canonical units).
#[derive(Clone, Debug, Default)]
pub struct TelemetrySample {
    pub t: f64,
    pub timestamp: Option<DateTime<Utc>>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub altitude: Option<f64>,
    pub speed: Option<f64>,
    pub distance: Option<f64>,
    pub heart_rate: Option<f64>,
    pub cadence: Option<f64>,
    pub power: Option<f64>,
    pub temperature: Option<f64>,
    pub heading: Option<f64>,
    pub grade: Option<f64>,
}

/// A complete telemetry recording in columnar form.
#[derive(Clone, Debug)]
pub struct Track {
    pub name: String,
    pub format: String,
    /// Unix seconds of the first sample.
    pub start_unix: f64,
    /// Seconds since `start_unix`, strictly increasing.
    pub times: Vec<f64>,
    channels: BTreeMap<Metric, Vec<f64>>,
    pub extra: BTreeMap<String, Vec<f64>>,
    pub laps: Vec<Lap>,
    /// Sport segments; empty unless the file records sessions.
    pub sessions: Vec<Session>,
    pub stats: TrackStats,
    pub route: Option<Route>,
    pub events: Vec<DetectedEvent>,
    pub sport: Option<String>,
    pub device: Option<String>,
}

impl Track {
    pub fn duration(&self) -> f64 {
        self.times.last().copied().unwrap_or(0.0)
    }

    pub fn len(&self) -> usize {
        self.times.len()
    }

    pub fn is_empty(&self) -> bool {
        self.times.is_empty()
    }

    pub fn start_time(&self) -> DateTime<Utc> {
        unix_to_datetime(self.start_unix)
    }

    /// Wall-clock time at track time `t`.
    pub fn wall_time(&self, t: f64) -> DateTime<Utc> {
        unix_to_datetime(self.start_unix + t)
    }

    pub fn has(&self, m: Metric) -> bool {
        self.channels.contains_key(&m)
    }

    /// The channel that actually serves `m`: GPS altitude falls back to the primary altitude
    /// for recordings without it, so such gauges still show data.
    pub fn resolve(&self, m: Metric) -> Metric {
        if m == Metric::GpsAltitude && !self.has(m) {
            Metric::Altitude
        } else {
            m
        }
    }

    pub fn channel(&self, m: Metric) -> Option<&[f64]> {
        self.channels.get(&self.resolve(m)).map(Vec::as_slice)
    }

    /// Statistics of the channel serving `m` (see [`Track::resolve`]).
    pub fn stats_for(&self, m: Metric) -> Option<&ChannelStats> {
        self.stats.get(self.resolve(m))
    }

    pub fn available_metrics(&self) -> Vec<Metric> {
        self.channels.keys().copied().collect()
    }

    /// Raw interpolated value at track time `t`.
    pub fn value(&self, m: Metric, t: f64) -> Option<f64> {
        let m = self.resolve(m);
        let ch = self.channels.get(&m)?;
        sample_channel(&self.times, ch, t, m.interp_kind())
    }

    /// Interpolated value with visual smoothing (`amount` 0..=1, 0 = raw data).
    pub fn value_smoothed(&self, m: Metric, t: f64, amount: f32) -> Option<f64> {
        let m = self.resolve(m);
        let ch = self.channels.get(&m)?;
        smoothing::smoothed(&self.times, ch, t, m.interp_kind(), amount)
    }

    /// Interpolated value of a non-standard field.
    pub fn extra_value(&self, key: &str, t: f64, amount: f32) -> Option<f64> {
        let ch = self.extra.get(key)?;
        smoothing::smoothed(&self.times, ch, t, InterpKind::Linear, amount)
    }

    /// Phase within the current heartbeat (0 = beat starts, rising to 1), from the cumulative
    /// beat count. `None` without heart-rate data.
    pub fn beat_phase(&self, t: f64) -> Option<f64> {
        let ch = self.extra.get(HEART_BEATS)?;
        let beats = sample_channel(&self.times, ch, t, InterpKind::Linear)?;
        Some(beats.rem_euclid(1.0))
    }

    /// Full interpolated snapshot at `t`.
    pub fn sample_at(&self, t: f64) -> TelemetrySample {
        let in_range = !self.times.is_empty() && t >= -1.0 && t <= self.duration() + 1.0;
        TelemetrySample {
            t,
            timestamp: in_range.then(|| self.wall_time(t)),
            latitude: self.value(Metric::Latitude, t),
            longitude: self.value(Metric::Longitude, t),
            altitude: self.value(Metric::Altitude, t),
            speed: self.value(Metric::Speed, t),
            distance: self.value(Metric::Distance, t),
            heart_rate: self.value(Metric::HeartRate, t),
            cadence: self.value(Metric::Cadence, t),
            power: self.value(Metric::Power, t),
            temperature: self.value(Metric::Temperature, t),
            heading: self.value(Metric::Heading, t),
            grade: self.value(Metric::Grade, t),
        }
    }

    /// Current lap index at `t`, if laps were recorded.
    pub fn lap_at(&self, t: f64) -> Option<(usize, &Lap)> {
        self.laps
            .iter()
            .enumerate()
            .rev()
            .find(|(_, l)| t >= l.start)
    }
}

pub fn unix_to_datetime(unix: f64) -> DateTime<Utc> {
    let secs = unix.floor();
    let nanos = ((unix - secs) * 1e9).round().clamp(0.0, 999_999_999.0) as u32;
    Utc.timestamp_opt(secs as i64, nanos)
        .single()
        .unwrap_or_else(|| Utc.timestamp_opt(0, 0).single().unwrap_or_default())
}

/// Extra channel: cumulative heart beats since the start of the recording.
pub const HEART_BEATS: &str = "heart_beats";

/// Extra channel: heavily smoothed GPS altitude used for grade, vertical speed and gain/loss.
pub const GPS_ALTITUDE_TREND: &str = "gps_altitude_trend";

/// Builds a normalized [`Track`] from raw samples.
pub struct TrackBuilder {
    pub name: String,
    pub format: String,
    pub samples: Vec<RawSample>,
    /// Laps in absolute Unix seconds (start, end, distance).
    pub laps: Vec<(f64, f64, Option<f64>)>,
    /// Sport segments in absolute Unix seconds (start, end, sport).
    pub sessions: Vec<(f64, f64, Option<String>)>,
    pub sport: Option<String>,
    pub device: Option<String>,
}

impl TrackBuilder {
    pub fn new(name: impl Into<String>, format: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            format: format.into(),
            samples: Vec::new(),
            laps: Vec::new(),
            sessions: Vec::new(),
            sport: None,
            device: None,
        }
    }

    /// Returns `None` when there are no usable timestamped samples.
    pub fn build(mut self) -> Option<Track> {
        self.samples.retain(|s| s.time.is_finite() && s.time > 0.0);
        if self.samples.is_empty() {
            return None;
        }
        self.samples.sort_by(|a, b| {
            a.time
                .partial_cmp(&b.time)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let merged = merge_duplicates(std::mem::take(&mut self.samples));
        let start = merged[0].time;
        let times: Vec<f64> = merged.iter().map(|s| s.time - start).collect();
        let n = times.len();

        let mut channels: BTreeMap<Metric, Vec<f64>> = BTreeMap::new();
        let mut put = |m: Metric, f: &dyn Fn(&RawSample) -> Option<f64>| {
            let v: Vec<f64> = merged
                .iter()
                .map(|s| f(s).filter(|x| x.is_finite()).unwrap_or(f64::NAN))
                .collect();
            if v.iter().any(|x| !x.is_nan()) {
                channels.insert(m, v);
            }
        };
        put(Metric::Latitude, &|s| s.latitude);
        put(Metric::Longitude, &|s| s.longitude);
        put(Metric::Altitude, &|s| s.altitude);
        put(Metric::GpsAltitude, &|s| s.gps_altitude);
        put(Metric::Speed, &|s| s.speed);
        put(Metric::Distance, &|s| s.distance);
        put(Metric::HeartRate, &|s| s.heart_rate);
        put(Metric::Cadence, &|s| s.cadence);
        put(Metric::Power, &|s| s.power);
        put(Metric::Temperature, &|s| s.temperature);
        put(Metric::Heading, &|s| s.heading);
        put(Metric::Grade, &|s| s.grade);

        // Positions must come in pairs.
        if !(channels.contains_key(&Metric::Latitude) && channels.contains_key(&Metric::Longitude))
        {
            channels.remove(&Metric::Latitude);
            channels.remove(&Metric::Longitude);
        }

        let mut extra: BTreeMap<String, Vec<f64>> = BTreeMap::new();
        for (i, s) in merged.iter().enumerate() {
            for (k, v) in &s.extra {
                if v.is_finite() {
                    extra.entry(k.clone()).or_insert_with(|| vec![f64::NAN; n])[i] = *v;
                }
            }
        }

        derive_channels(&times, &mut channels, &mut extra);

        let laps: Vec<Lap> = self
            .laps
            .iter()
            .map(|&(s, e, d)| Lap {
                start: s - start,
                end: e - start,
                distance: d,
            })
            .collect();
        let mut sessions: Vec<Session> = self
            .sessions
            .iter()
            .map(|(s, e, sport)| Session {
                start: s - start,
                end: e - start,
                sport: sport.clone(),
            })
            .collect();
        sessions.sort_by(|a, b| {
            a.start
                .partial_cmp(&b.start)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        if let Some(delta) = channels
            .get(&Metric::Distance)
            .and_then(|dist| derive_lap_delta(&times, dist, &laps))
        {
            channels.insert(Metric::LapDelta, delta);
        }

        let mut track = Track {
            name: self.name,
            format: self.format,
            start_unix: start,
            times,
            channels,
            extra,
            laps,
            sessions,
            stats: TrackStats::default(),
            route: None,
            events: Vec::new(),
            sport: self.sport,
            device: self.device,
        };
        track.stats = compute_stats(&track);
        track.route = build_route(&track);
        track.events = detect_events(&track);
        Some(track)
    }
}

/// Samples sharing a timestamp are merged; later values win where present.
fn merge_duplicates(samples: Vec<RawSample>) -> Vec<RawSample> {
    let mut out: Vec<RawSample> = Vec::with_capacity(samples.len());
    for s in samples {
        if let Some(last) = out.last_mut()
            && (s.time - last.time).abs() < 1e-6
        {
            macro_rules! take {
                ($($f:ident),*) => { $( if s.$f.is_some() { last.$f = s.$f; } )* };
            }
            take!(
                latitude,
                longitude,
                altitude,
                gps_altitude,
                speed,
                distance,
                heart_rate,
                cadence,
                power,
                temperature,
                heading,
                grade
            );
            last.extra.extend(s.extra);
            continue;
        }
        out.push(s);
    }
    out
}

/// Haversine distance in meters.
pub fn haversine(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let (p1, p2) = (lat1.to_radians(), lat2.to_radians());
    let dp = p2 - p1;
    let dl = (lon2 - lon1).to_radians();
    let a = (dp / 2.0).sin().powi(2) + p1.cos() * p2.cos() * (dl / 2.0).sin().powi(2);
    2.0 * EARTH_R * a.sqrt().asin()
}

/// Initial bearing in degrees (0..360, clockwise from north).
pub fn bearing(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let (p1, p2) = (lat1.to_radians(), lat2.to_radians());
    let dl = (lon2 - lon1).to_radians();
    let y = dl.sin() * p2.cos();
    let x = p1.cos() * p2.sin() - p1.sin() * p2.cos() * dl.cos();
    (y.atan2(x).to_degrees() + 360.0) % 360.0
}

fn derive_channels(
    times: &[f64],
    channels: &mut BTreeMap<Metric, Vec<f64>>,
    extra: &mut BTreeMap<String, Vec<f64>>,
) {
    let n = times.len();
    let has_pos = channels.contains_key(&Metric::Latitude);

    // Distance from positions when the device did not record it.
    if !channels.contains_key(&Metric::Distance) && has_pos {
        let lat = &channels[&Metric::Latitude];
        let lon = &channels[&Metric::Longitude];
        let mut dist = vec![f64::NAN; n];
        let mut acc = 0.0;
        let mut last: Option<(f64, f64)> = None;
        for i in 0..n {
            if lat[i].is_nan() || lon[i].is_nan() {
                continue;
            }
            if let Some((la, lo)) = last {
                let d = haversine(la, lo, lat[i], lon[i]);
                // Ignore GPS teleports (> 100 m/s).
                if d < 100.0 * (times[i] - times[i.saturating_sub(1)]).max(1.0) {
                    acc += d;
                }
            }
            last = Some((lat[i], lon[i]));
            dist[i] = acc;
        }
        channels.insert(Metric::Distance, dist);
    }

    // Speed from distance when missing.
    if !channels.contains_key(&Metric::Speed)
        && let Some(dist) = channels.get(&Metric::Distance)
    {
        let mut speed = vec![f64::NAN; n];
        let mut lo = 0;
        let mut hi = 0;
        for i in 0..n {
            // Centered derivative over a ±2 s window.
            while lo < i && times[i] - times[lo] > 2.0 {
                lo += 1;
            }
            while hi + 1 < n && times[hi + 1] - times[i] <= 2.0 {
                hi += 1;
            }
            hi = hi.max(i);
            let (mut a, mut b) = (lo, hi);
            while a < i && dist[a].is_nan() {
                a += 1;
            }
            while b > i && dist[b].is_nan() {
                b -= 1;
            }
            if a < b && !dist[a].is_nan() && !dist[b].is_nan() {
                let dt = times[b] - times[a];
                if dt > 0.0 {
                    speed[i] = ((dist[b] - dist[a]) / dt).max(0.0);
                }
            }
        }
        channels.insert(Metric::Speed, speed);
    }

    // Heading from movement between positions.
    if !channels.contains_key(&Metric::Heading) && has_pos {
        let lat = &channels[&Metric::Latitude];
        let lon = &channels[&Metric::Longitude];
        let mut heading = vec![f64::NAN; n];
        let mut anchor: Option<usize> = None;
        let mut current = f64::NAN;
        for i in 0..n {
            if lat[i].is_nan() || lon[i].is_nan() {
                continue;
            }
            match anchor {
                None => anchor = Some(i),
                Some(a) => {
                    if haversine(lat[a], lon[a], lat[i], lon[i]) >= 3.0 {
                        current = bearing(lat[a], lon[a], lat[i], lon[i]);
                        anchor = Some(i);
                    }
                }
            }
            heading[i] = current;
        }
        // Backfill the start with the first known heading.
        if let Some(first) = heading.iter().copied().find(|h| !h.is_nan()) {
            for h in heading.iter_mut() {
                if h.is_nan() {
                    *h = first;
                } else {
                    break;
                }
            }
            channels.insert(Metric::Heading, heading);
        }
    }

    // Altitude is smoothed for display, grade and gain/loss; the raw values are preserved.
    if let Some(alt) = channels.get(&Metric::Altitude).cloned() {
        let smoothed = smoothing::gaussian_smooth(times, &alt, 2.0);
        extra.insert("altitude_raw".into(), alt);
        channels.insert(Metric::Altitude, smoothed);
    }
    // GPS altitude is noisier than barometric altitude: same light smoothing, raw kept.
    if let Some(alt) = channels.get(&Metric::GpsAltitude).cloned() {
        let smoothed = smoothing::gaussian_smooth(times, &alt, 2.0);
        extra.insert("gps_altitude_raw".into(), alt);
        channels.insert(Metric::GpsAltitude, smoothed);
    }

    // Derived climbing metrics (grade, vertical speed, gain/loss) follow the preferred altitude
    // source: GPS altitude when present. GPS altitude is noisy, so they use a more heavily
    // smoothed trend (σ = 6 s), kept as an extra channel; displayed values stay responsive.
    let climb_alt: Option<Vec<f64>> = match extra.get("gps_altitude_raw") {
        Some(raw) => {
            let trend = smoothing::gaussian_smooth(times, raw, 6.0);
            extra.insert(GPS_ALTITUDE_TREND.into(), trend.clone());
            Some(trend)
        }
        None => channels.get(&Metric::Altitude).cloned(),
    };

    // Grade over a ±25 m distance window.
    if !channels.contains_key(&Metric::Grade)
        && let (Some(alt), Some(dist)) = (climb_alt.as_ref(), channels.get(&Metric::Distance))
    {
        let mut grade = vec![f64::NAN; n];
        let (mut lo, mut hi) = (0usize, 0usize);
        for i in 0..n {
            if dist[i].is_nan() || alt[i].is_nan() {
                continue;
            }
            while lo < i && (dist[lo].is_nan() || dist[i] - dist[lo] > 25.0) {
                lo += 1;
            }
            hi = hi.max(i);
            while hi + 1 < n && (dist[hi + 1].is_nan() || dist[hi + 1] - dist[i] <= 25.0) {
                hi += 1;
            }
            let (mut a, mut b) = (lo, hi);
            while a < i && (alt[a].is_nan() || dist[a].is_nan()) {
                a += 1;
            }
            while b > i && (alt[b].is_nan() || dist[b].is_nan()) {
                b -= 1;
            }
            let run = dist[b] - dist[a];
            if run > 4.0 {
                grade[i] = ((alt[b] - alt[a]) / run * 100.0).clamp(-40.0, 40.0);
            }
        }
        let grade = smoothing::gaussian_smooth(times, &grade, 3.0);
        if grade.iter().any(|g| !g.is_nan()) {
            channels.insert(Metric::Grade, grade);
        }
    }

    // Vertical speed (m/s) from smoothed altitude.
    if let Some(alt) = &climb_alt {
        let mut vs = vec![f64::NAN; n];
        let (mut lo, mut hi) = (0usize, 0usize);
        for i in 0..n {
            while lo < i && times[i] - times[lo] > 5.0 {
                lo += 1;
            }
            hi = hi.max(i);
            while hi + 1 < n && times[hi + 1] - times[i] <= 5.0 {
                hi += 1;
            }
            let (mut a, mut b) = (lo, hi);
            while a < i && alt[a].is_nan() {
                a += 1;
            }
            while b > i && alt[b].is_nan() {
                b -= 1;
            }
            let dt = times[b] - times[a];
            if dt > 0.5 && !alt[a].is_nan() && !alt[b].is_nan() {
                vs[i] = (alt[b] - alt[a]) / dt;
            }
        }
        if vs.iter().any(|v| !v.is_nan()) {
            channels.insert(Metric::VerticalSpeed, vs);
        }
        let (gain, _) = climb_series(alt, climb_hysteresis(extra));
        if gain.iter().any(|v| !v.is_nan()) {
            channels.insert(Metric::ElevationGain, gain);
        }
    }

    if let Some(vmg) = derive_vmg(channels) {
        channels.insert(Metric::Vmg, vmg);
    }
    if let Some(g) = derive_g_force(times, channels) {
        channels.insert(Metric::GForce, g);
    }

    // Cumulative heart beats (trapezoidal integral of bpm / 60). The fractional part is the
    // phase within the current beat, so an animated heart stays in step with a changing heart
    // rate and every frame is computed statelessly (scrubbing and export agree).
    if let Some(hr) = channels.get(&Metric::HeartRate) {
        let mut beats = vec![f64::NAN; n];
        let mut acc = 0.0;
        let mut last: Option<(f64, f64)> = None;
        for i in 0..n {
            if hr[i].is_nan() || hr[i] <= 0.0 {
                continue;
            }
            if let Some((t0, h0)) = last {
                acc += (times[i] - t0) * (h0 + hr[i]) / 2.0 / 60.0;
            }
            last = Some((times[i], hr[i]));
            beats[i] = acc;
        }
        if last.is_some() {
            extra.insert(HEART_BEATS.into(), beats);
        }
    }

    // Pace (s/km) from speed.
    if let Some(speed) = channels.get(&Metric::Speed) {
        let pace: Vec<f64> = speed
            .iter()
            .map(|&v| if v > 0.5 { 1000.0 / v } else { f64::NAN })
            .collect();
        if pace.iter().any(|p| !p.is_nan()) {
            channels.insert(Metric::Pace, pace);
        }
    }
}

/// Hysteresis (m) for counting climbing: GPS altitude jitters more than barometric altitude.
fn climb_hysteresis(extra: &BTreeMap<String, Vec<f64>>) -> f64 {
    if extra.contains_key(GPS_ALTITUDE_TREND) {
        3.0
    } else {
        1.0
    }
}

/// Cumulative elevation gain at every sample (NaN before the first altitude) and the total
/// loss. Changes smaller than `hysteresis` are treated as noise, not climbing.
fn climb_series(alt: &[f64], hysteresis: f64) -> (Vec<f64>, f64) {
    let mut gain_series = vec![f64::NAN; alt.len()];
    let (mut gain, mut loss) = (0.0, 0.0);
    let mut reference: Option<f64> = None;
    for (i, &a) in alt.iter().enumerate() {
        if !a.is_nan() {
            match reference {
                None => reference = Some(a),
                Some(r) => {
                    if a - r >= hysteresis {
                        gain += a - r;
                        reference = Some(a);
                    } else if r - a >= hysteresis {
                        loss += r - a;
                        reference = Some(a);
                    }
                }
            }
        }
        if reference.is_some() {
            gain_series[i] = gain;
        }
    }
    (gain_series, loss)
}

/// Velocity made good toward the last recorded position: speed × cos(course − bearing).
fn derive_vmg(channels: &BTreeMap<Metric, Vec<f64>>) -> Option<Vec<f64>> {
    let lat = channels.get(&Metric::Latitude)?;
    let lon = channels.get(&Metric::Longitude)?;
    let speed = channels.get(&Metric::Speed)?;
    let heading = channels.get(&Metric::Heading)?;
    let end = (0..lat.len())
        .rev()
        .find(|&i| !lat[i].is_nan() && !lon[i].is_nan())?;
    let (elat, elon) = (lat[end], lon[end]);
    let vmg: Vec<f64> = (0..lat.len())
        .map(|i| {
            let (la, lo, v, h) = (lat[i], lon[i], speed[i], heading[i]);
            if la.is_nan() || lo.is_nan() || v.is_nan() || h.is_nan() {
                return f64::NAN;
            }
            // Within a few meters of the destination the bearing is meaningless.
            if haversine(la, lo, elat, elon) < 10.0 {
                return f64::NAN;
            }
            v * (h - bearing(la, lo, elat, elon)).to_radians().cos()
        })
        .collect();
    vmg.iter().any(|v| !v.is_nan()).then_some(vmg)
}

/// Combined acceleration in g: longitudinal from the change in speed, lateral from speed ×
/// turn rate. Centered differences over ±1 s, then lightly smoothed against GPS jitter.
fn derive_g_force(times: &[f64], channels: &BTreeMap<Metric, Vec<f64>>) -> Option<Vec<f64>> {
    const G: f64 = 9.80665;
    let speed = channels.get(&Metric::Speed)?;
    let heading = channels.get(&Metric::Heading)?;
    let n = times.len();
    let mut g = vec![f64::NAN; n];
    let (mut lo, mut hi) = (0usize, 0usize);
    for i in 0..n {
        while lo < i && times[i] - times[lo] > 1.0 {
            lo += 1;
        }
        hi = hi.max(i);
        while hi + 1 < n && times[hi + 1] - times[i] <= 1.0 {
            hi += 1;
        }
        let (a, b) = (lo, hi);
        let dt = times[b] - times[a];
        if dt < 0.5 || [speed[a], speed[b], heading[a], heading[b], speed[i]]
            .iter()
            .any(|v| v.is_nan())
        {
            continue;
        }
        let long = (speed[b] - speed[a]) / dt;
        // Shortest signed turn between the two headings.
        let turn = ((heading[b] - heading[a] + 540.0) % 360.0 - 180.0).to_radians();
        let lat = speed[i] * turn / dt;
        g[i] = ((long * long + lat * lat).sqrt() / G).min(5.0);
    }
    let g = smoothing::gaussian_smooth(times, &g, 1.0);
    g.iter().any(|v| !v.is_nan()).then_some(g)
}

/// Time gained or lost against the fastest complete lap, compared at equal distance into the
/// lap. Needs at least two laps and a distance channel.
fn derive_lap_delta(times: &[f64], dist: &[f64], laps: &[Lap]) -> Option<Vec<f64>> {
    if laps.len() < 2 {
        return None;
    }
    let dist_at = |t: f64| sample_channel(times, dist, t, InterpKind::Linear);
    // Fastest lap that covers some distance.
    let best = laps
        .iter()
        .filter(|l| l.end - l.start > 10.0)
        .filter(|l| matches!((dist_at(l.start), dist_at(l.end)), (Some(a), Some(b)) if b - a > 50.0))
        .min_by(|a, b| {
            (a.end - a.start)
                .partial_cmp(&(b.end - b.start))
                .unwrap_or(std::cmp::Ordering::Equal)
        })?;
    let d0 = dist_at(best.start)?;
    // (distance into lap, time into lap) along the best lap, distance non-decreasing.
    let mut profile: Vec<(f64, f64)> = vec![(0.0, 0.0)];
    for (i, &t) in times.iter().enumerate() {
        if t <= best.start || t > best.end || dist[i].is_nan() {
            continue;
        }
        let d = dist[i] - d0;
        if d >= profile.last().map_or(0.0, |p| p.0) {
            profile.push((d, t - best.start));
        }
    }
    let best_time_at = |d: f64| -> Option<f64> {
        let k = profile.partition_point(|p| p.0 < d);
        if k == 0 {
            return Some(0.0);
        }
        let (d1, t1) = *profile.get(k)?;
        let (d0, t0) = profile[k - 1];
        Some(if d1 > d0 {
            t0 + (t1 - t0) * (d - d0) / (d1 - d0)
        } else {
            t1
        })
    };
    let mut delta = vec![f64::NAN; times.len()];
    for (i, &t) in times.iter().enumerate() {
        let Some(lap) = laps.iter().rev().find(|l| t >= l.start && t <= l.end) else {
            continue;
        };
        let (Some(start_d), false) = (dist_at(lap.start), dist[i].is_nan()) else {
            continue;
        };
        if let Some(bt) = best_time_at(dist[i] - start_d) {
            delta[i] = (t - lap.start) - bt;
        }
    }
    delta.iter().any(|v| !v.is_nan()).then_some(delta)
}

fn channel_stats(values: &[f64]) -> Option<ChannelStats> {
    let mut s = ChannelStats {
        min: f64::INFINITY,
        max: f64::NEG_INFINITY,
        avg: 0.0,
        count: 0,
    };
    let mut sum = 0.0;
    for &v in values {
        if v.is_nan() {
            continue;
        }
        s.min = s.min.min(v);
        s.max = s.max.max(v);
        sum += v;
        s.count += 1;
    }
    (s.count > 0).then(|| {
        s.avg = sum / s.count as f64;
        s
    })
}

fn compute_stats(track: &Track) -> TrackStats {
    let mut stats = TrackStats {
        duration: track.duration(),
        ..Default::default()
    };
    for (&m, ch) in &track.channels {
        if let Some(s) = channel_stats(ch) {
            stats.per_metric.insert(m, s);
        }
    }
    // Averages of HR/power/cadence/speed are more meaningful while moving.
    if let Some(speed) = track.channel(Metric::Speed) {
        let mut moving = 0.0;
        for (w, v) in track.times.windows(2).zip(speed.iter().skip(1)) {
            let dt = w[1] - w[0];
            if *v > 0.8 && dt < 10.0 {
                moving += dt;
            }
        }
        stats.moving_time = moving;
        let moving_avg = |ch: &[f64]| -> Option<f64> {
            let (mut sum, mut n) = (0.0, 0usize);
            for (i, &v) in ch.iter().enumerate() {
                if !v.is_nan() && speed.get(i).is_some_and(|s| *s > 0.8) {
                    sum += v;
                    n += 1;
                }
            }
            (n > 0).then(|| sum / n as f64)
        };
        for m in [Metric::Speed, Metric::Cadence, Metric::Power] {
            if let (Some(ch), Some(st)) = (track.channel(m), stats.per_metric.get_mut(&m))
                && let Some(avg) = moving_avg(ch)
            {
                st.avg = avg;
            }
        }
    }
    // GPS altitude (preferred) uses its smoothed trend and a larger threshold against jitter.
    let (climb, hysteresis) = match track.extra.get(GPS_ALTITUDE_TREND) {
        Some(trend) => (Some(trend.as_slice()), 3.0),
        None => (track.channel(Metric::Altitude), 1.0),
    };
    if let Some(alt) = climb {
        let (gain, loss) = climb_series(alt, hysteresis);
        stats.elevation_gain = Some(gain.iter().rev().copied().find(|v| !v.is_nan()).unwrap_or(0.0));
        stats.elevation_loss = Some(loss);
    }
    if let Some(d) = track.channel(Metric::Distance) {
        stats.total_distance = d.iter().rev().copied().find(|v| !v.is_nan());
    }
    stats
}

fn build_route(track: &Track) -> Option<Route> {
    let lat = track.channel(Metric::Latitude)?;
    let lon = track.channel(Metric::Longitude)?;
    let valid: Vec<usize> = (0..lat.len())
        .filter(|&i| !lat[i].is_nan() && !lon[i].is_nan())
        .collect();
    if valid.len() < 2 {
        return None;
    }
    let (mut lat_min, mut lat_max, mut lon_min, mut lon_max) = (
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
    );
    for &i in &valid {
        lat_min = lat_min.min(lat[i]);
        lat_max = lat_max.max(lat[i]);
        lon_min = lon_min.min(lon[i]);
        lon_max = lon_max.max(lon[i]);
    }
    let cos_lat = ((lat_min + lat_max) / 2.0).to_radians().cos();
    let mut route = Route {
        points: Vec::new(),
        times: Vec::new(),
        width: (lon_max - lon_min).to_radians() * cos_lat * EARTH_R,
        height: (lat_max - lat_min).to_radians() * EARTH_R,
        lon_min,
        lat_max,
        cos_lat,
    };
    let diag = (route.width.powi(2) + route.height.powi(2)).sqrt();
    let min_step = (diag / 3000.0).max(0.5);
    let mut last: Option<[f64; 2]> = None;
    for (k, &i) in valid.iter().enumerate() {
        let p = route.project(lat[i], lon[i]);
        let keep = match last {
            None => true,
            Some(q) => {
                k + 1 == valid.len()
                    || ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2)).sqrt() >= min_step
            }
        };
        if keep {
            route.points.push(p);
            route.times.push(track.times[i]);
            last = Some(p);
        }
    }
    Some(route)
}

fn detect_events(track: &Track) -> Vec<DetectedEvent> {
    let mut events = Vec::new();
    if let Some(speed) = track.channel(Metric::Speed) {
        let smooth = smoothing::gaussian_smooth(&track.times, speed, 1.0);
        let mut moving = false;
        let mut candidate: Option<f64> = None;
        for (i, &v) in smooth.iter().enumerate() {
            if v.is_nan() {
                continue;
            }
            let t = track.times[i];
            let wants = if moving { v > 0.8 } else { v > 2.0 };
            if wants != moving {
                let since = *candidate.get_or_insert(t);
                if t - since >= 2.0 {
                    moving = wants;
                    events.push(DetectedEvent {
                        t: since,
                        kind: if moving {
                            EventKind::StartMoving
                        } else {
                            EventKind::Stop
                        },
                    });
                    candidate = None;
                }
            } else {
                candidate = None;
            }
        }
        if let Some((i, _)) = speed
            .iter()
            .enumerate()
            .filter(|(_, v)| !v.is_nan())
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        {
            events.push(DetectedEvent {
                t: track.times[i],
                kind: EventKind::MaxSpeed,
            });
        }
    }
    for lap in track.laps.iter().skip(1) {
        events.push(DetectedEvent {
            t: lap.start,
            kind: EventKind::LapStart,
        });
    }
    for s in track.sessions.iter().skip(1) {
        events.push(DetectedEvent {
            t: s.start,
            kind: EventKind::SportStart,
        });
    }
    events.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap_or(std::cmp::Ordering::Equal));
    events
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line_track(n: usize) -> Track {
        let mut b = TrackBuilder::new("test", "test");
        for i in 0..n {
            b.samples.push(RawSample {
                time: 1_700_000_000.0 + i as f64,
                // ~11.1 m per sample heading north.
                latitude: Some(45.0 + i as f64 * 0.0001),
                longitude: Some(7.0),
                altitude: Some(100.0 + i as f64),
                heart_rate: if i % 2 == 0 { Some(120.0) } else { None },
                ..Default::default()
            });
        }
        b.build().unwrap()
    }

    #[test]
    fn derives_distance_speed_heading() {
        let t = line_track(60);
        assert!(t.has(Metric::Distance));
        assert!(t.has(Metric::Speed));
        assert!(t.has(Metric::Heading));
        assert!(t.has(Metric::Grade));
        assert!(t.has(Metric::Pace));
        let v = t.value(Metric::Speed, 30.0).unwrap();
        assert!((v - 11.12).abs() < 0.2, "speed {v}");
        let h = t.value(Metric::Heading, 30.0).unwrap();
        assert!(!(1.0..=359.0).contains(&h), "heading {h}");
        let d = t.stats.total_distance.unwrap();
        assert!((d - 59.0 * 11.12).abs() < 5.0, "distance {d}");
    }

    #[test]
    fn derives_gain_vmg_and_g_force() {
        // Straight north at constant speed, climbing 1 m per sample.
        let t = line_track(60);
        let gain = t.value(Metric::ElevationGain, 59.0).unwrap();
        assert!((gain - t.stats.elevation_gain.unwrap()).abs() < 1e-9);
        assert!(gain > 40.0, "gain {gain}");
        assert!(t.value(Metric::ElevationGain, 10.0).unwrap() < gain);
        // Heading straight for the finish: VMG equals speed.
        let (vmg, speed) = (
            t.value(Metric::Vmg, 20.0).unwrap(),
            t.value(Metric::Speed, 20.0).unwrap(),
        );
        assert!((vmg - speed).abs() < 0.2, "vmg {vmg} speed {speed}");
        // No turning and no speed change: almost no acceleration.
        let g = t.value(Metric::GForce, 30.0).unwrap();
        assert!(g < 0.05, "g {g}");
    }

    #[test]
    fn lap_delta_against_the_best_lap() {
        // Two 100 s laps covering the same 500 m; the second is slower in its first half.
        let mut b = TrackBuilder::new("laps", "test");
        let t0 = 1_700_000_000.0;
        for i in 0..=200 {
            let s = i as f64;
            let dist = if i <= 100 {
                s * 5.0
            } else {
                let k = s - 100.0;
                500.0 + if k <= 50.0 { k * 4.0 } else { 200.0 + (k - 50.0) * 6.0 }
            };
            b.samples.push(RawSample {
                time: t0 + s,
                distance: Some(dist),
                ..Default::default()
            });
        }
        b.laps.push((t0, t0 + 100.0, Some(500.0)));
        b.laps.push((t0 + 100.0, t0 + 200.0, Some(500.0)));
        let t = b.build().unwrap();
        assert_eq!(t.value(Metric::LapDelta, 50.0), Some(0.0));
        // 200 m into lap 2 took 50 s; the best lap got there in 40 s.
        let d = t.value(Metric::LapDelta, 150.0).unwrap();
        assert!((d - 10.0).abs() < 0.01, "delta {d}");
    }

    #[test]
    fn missing_values_are_none() {
        let t = line_track(10);
        assert!(!t.has(Metric::Power));
        assert_eq!(t.value(Metric::Power, 3.0), None);
        // Heart rate is present every other sample; interpolation bridges the gap.
        assert_eq!(t.value(Metric::HeartRate, 3.0), Some(120.0));
        // Outside the recording.
        assert_eq!(t.value(Metric::HeartRate, 100.0), None);
    }

    #[test]
    fn duplicates_merge() {
        let mut b = TrackBuilder::new("t", "t");
        b.samples.push(RawSample {
            time: 10.0,
            heart_rate: Some(100.0),
            ..Default::default()
        });
        b.samples.push(RawSample {
            time: 10.0,
            power: Some(200.0),
            ..Default::default()
        });
        b.samples.push(RawSample {
            time: 11.0,
            power: Some(210.0),
            ..Default::default()
        });
        let t = b.build().unwrap();
        assert_eq!(t.len(), 2);
        assert_eq!(t.value(Metric::HeartRate, 0.0), Some(100.0));
        assert_eq!(t.value(Metric::Power, 0.0), Some(200.0));
    }

    #[test]
    fn route_projects() {
        let t = line_track(100);
        let r = t.route.as_ref().unwrap();
        assert!(r.height > 1000.0 && r.width < 1.0);
        assert!(r.points.len() >= 2);
    }

    #[test]
    fn empty_builder() {
        assert!(TrackBuilder::new("x", "y").build().is_none());
    }
}
