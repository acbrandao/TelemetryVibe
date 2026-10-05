//! GPX and TCX parsers (XML based formats).

use quick_xml::Reader;
use quick_xml::events::Event;

use super::model::{RawSample, Track, TrackBuilder};

#[derive(Debug, thiserror::Error)]
pub enum XmlError {
    #[error("invalid XML: {0}")]
    Xml(String),
    #[error("file is not valid UTF-8")]
    Encoding,
    #[error("no timestamped track points")]
    NoPoints,
}

fn parse_time(s: &str) -> Option<f64> {
    let dt = chrono::DateTime::parse_from_rfc3339(s.trim())
        .ok()
        .or_else(|| {
            // Some exporters omit the timezone; assume UTC.
            chrono::NaiveDateTime::parse_from_str(s.trim(), "%Y-%m-%dT%H:%M:%S%.f")
                .ok()
                .map(|n| n.and_utc().fixed_offset())
        })?;
    Some(dt.timestamp() as f64 + dt.timestamp_subsec_nanos() as f64 * 1e-9)
}

fn num(s: &str) -> Option<f64> {
    s.trim().parse::<f64>().ok().filter(|v| v.is_finite())
}

pub fn sniff_gpx(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(2048)];
    String::from_utf8_lossy(head).contains("<gpx")
}

pub fn sniff_tcx(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(2048)];
    String::from_utf8_lossy(head).contains("TrainingCenterDatabase")
}

/// XML walk events.
enum Ev<'a> {
    Start(&'a str, &'a [(String, String)]),
    Text(&'a [String], &'a str),
    End(&'a str),
}

/// Walks XML elements, reporting start tags, text (with the element path) and end tags.
fn walk(bytes: &[u8], mut on: impl FnMut(Ev<'_>)) -> Result<(), XmlError> {
    let text = std::str::from_utf8(bytes).map_err(|_| XmlError::Encoding)?;
    let text = text.trim_start_matches('\u{feff}');
    let mut reader = Reader::from_str(text);
    let mut path: Vec<String> = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = e.local_name().as_ref().to_string();
                let attrs = collect_attrs(&e);
                on(Ev::Start(&name, &attrs));
                path.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = e.local_name().as_ref().to_string();
                let attrs = collect_attrs(&e);
                on(Ev::Start(&name, &attrs));
                on(Ev::End(&name));
            }
            Ok(Event::Text(t)) => {
                let s = t.xml10_content();
                if !s.trim().is_empty() {
                    on(Ev::Text(&path, &s));
                }
            }
            Ok(Event::CData(t)) => {
                let s: &str = &t;
                on(Ev::Text(&path, s));
            }
            Ok(Event::End(_)) => {
                if let Some(name) = path.pop() {
                    on(Ev::End(&name));
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => return Err(XmlError::Xml(e.to_string())),
        }
    }
    Ok(())
}

fn collect_attrs(e: &quick_xml::events::BytesStart<'_>) -> Vec<(String, String)> {
    e.attributes()
        .flatten()
        .map(|a| {
            (
                a.key.local_name().as_ref().to_string(),
                a.normalized_value(quick_xml::XmlVersion::Implicit1_0)
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
            )
        })
        .collect()
}

/// Parses GPX 1.0/1.1 including Garmin TrackPointExtension (hr, cad, atemp) and power.
pub fn parse_gpx(bytes: &[u8], name: &str) -> Result<Track, XmlError> {
    let mut builder = TrackBuilder::new(name, "GPX");
    let mut current: Option<RawSample> = None;
    let mut has_time = false;
    walk(bytes, |ev| match ev {
        Ev::Start(tag, attrs) => {
            if tag == "trkpt" || tag == "rtept" {
                let get = |k: &str| attrs.iter().find(|(a, _)| a == k).and_then(|(_, v)| num(v));
                current = Some(RawSample {
                    latitude: get("lat"),
                    longitude: get("lon"),
                    ..Default::default()
                });
                has_time = false;
            }
        }
        Ev::Text(path, text) => {
            let Some(s) = current.as_mut() else { return };
            let Some(tag) = path.last() else { return };
            match tag.as_str() {
                "time" => {
                    if let Some(t) = parse_time(text) {
                        s.time = t;
                        has_time = true;
                    }
                }
                "ele" => s.altitude = num(text),
                "hr" | "heartrate" => s.heart_rate = num(text),
                "cad" | "cadence" => s.cadence = num(text),
                "atemp" | "temp" | "wtemp" => s.temperature = num(text),
                "power" | "PowerInWatts" => s.power = num(text),
                "speed" => s.speed = num(text),
                "course" | "bearing" => s.heading = num(text),
                "distance" => s.distance = num(text),
                other => {
                    if let Some(v) = num(text)
                        && path.iter().any(|p| p == "extensions")
                    {
                        s.extra.push((other.to_string(), v));
                    }
                }
            }
        }
        Ev::End(tag) => {
            if (tag == "trkpt" || tag == "rtept")
                && let Some(s) = current.take()
                && has_time
            {
                builder.samples.push(s);
            }
        }
    })?;
    builder.build().ok_or(XmlError::NoPoints)
}

/// Parses Garmin Training Center XML.
pub fn parse_tcx(bytes: &[u8], name: &str) -> Result<Track, XmlError> {
    let mut builder = TrackBuilder::new(name, "TCX");
    let mut current: Option<RawSample> = None;
    let mut lap_start: Option<f64> = None;
    let mut lap_time: Option<f64> = None;
    let mut lap_dist: Option<f64> = None;
    let mut sport: Option<String> = None;
    walk(bytes, |ev| match ev {
        Ev::Start(tag, attrs) => match tag {
            "Trackpoint" => current = Some(RawSample::default()),
            "Lap" => {
                lap_start = attrs
                    .iter()
                    .find(|(k, _)| k == "StartTime")
                    .and_then(|(_, v)| parse_time(v));
                lap_time = None;
                lap_dist = None;
            }
            "Activity" => {
                sport = attrs
                    .iter()
                    .find(|(k, _)| k == "Sport")
                    .map(|(_, v)| v.clone());
            }
            _ => {}
        },
        Ev::Text(path, text) => {
            let Some(tag) = path.last() else { return };
            let parent = path.len().checked_sub(2).map(|i| path[i].as_str());
            if let Some(s) = current.as_mut() {
                match (parent, tag.as_str()) {
                    (_, "Time") => s.time = parse_time(text).unwrap_or(0.0),
                    (_, "LatitudeDegrees") => s.latitude = num(text),
                    (_, "LongitudeDegrees") => s.longitude = num(text),
                    (_, "AltitudeMeters") => s.altitude = num(text),
                    (_, "DistanceMeters") => s.distance = num(text),
                    (Some("HeartRateBpm"), "Value") => s.heart_rate = num(text),
                    (_, "Cadence") | (_, "RunCadence") => s.cadence = num(text),
                    (_, "Speed") => s.speed = num(text),
                    (_, "Watts") => s.power = num(text),
                    _ => {}
                }
            } else if parent == Some("Lap") {
                match tag.as_str() {
                    "TotalTimeSeconds" => lap_time = num(text),
                    "DistanceMeters" => lap_dist = num(text),
                    _ => {}
                }
            }
        }
        Ev::End(tag) => match tag {
            "Trackpoint" => {
                if let Some(s) = current.take()
                    && s.time > 0.0
                {
                    builder.samples.push(s);
                }
            }
            "Lap" => {
                if let Some(start) = lap_start.take() {
                    builder
                        .laps
                        .push((start, start + lap_time.unwrap_or(0.0), lap_dist));
                }
            }
            _ => {}
        },
    })?;
    builder.sport = sport;
    builder.build().ok_or(XmlError::NoPoints)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::telemetry::model::Metric;

    const GPX: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<gpx version="1.1" creator="test" xmlns="http://www.topografix.com/GPX/1/1"
     xmlns:gpxtpx="http://www.garmin.com/xmlschemas/TrackPointExtension/v1">
  <trk><name>Ride</name><trkseg>
    <trkpt lat="45.0000" lon="7.0000"><ele>100</ele><time>2024-05-01T10:00:00Z</time>
      <extensions><gpxtpx:TrackPointExtension><gpxtpx:hr>120</gpxtpx:hr><gpxtpx:cad>80</gpxtpx:cad></gpxtpx:TrackPointExtension></extensions></trkpt>
    <trkpt lat="45.0001" lon="7.0000"><ele>101</ele><time>2024-05-01T10:00:01Z</time>
      <extensions><gpxtpx:TrackPointExtension><gpxtpx:hr>122</gpxtpx:hr></gpxtpx:TrackPointExtension></extensions></trkpt>
    <trkpt lat="45.0002" lon="7.0000"><ele>102</ele><time>2024-05-01T10:00:02.500Z</time></trkpt>
  </trkseg></trk>
</gpx>"#;

    #[test]
    fn parses_gpx() {
        assert!(sniff_gpx(GPX.as_bytes()));
        let t = parse_gpx(GPX.as_bytes(), "ride.gpx").unwrap();
        assert_eq!(t.len(), 3);
        assert_eq!(t.value(Metric::HeartRate, 0.5), Some(121.0));
        assert!((t.duration() - 2.5).abs() < 1e-6);
        assert!(t.has(Metric::Speed));
        assert_eq!(t.start_time().to_rfc3339(), "2024-05-01T10:00:00+00:00");
    }

    #[test]
    fn parses_tcx() {
        let tcx = r#"<?xml version="1.0"?>
<TrainingCenterDatabase xmlns="http://www.garmin.com/xmlschemas/TrainingCenterDatabase/v2">
<Activities><Activity Sport="Running"><Lap StartTime="2024-05-01T10:00:00Z">
<TotalTimeSeconds>10</TotalTimeSeconds><DistanceMeters>30</DistanceMeters>
<Track>
<Trackpoint><Time>2024-05-01T10:00:00Z</Time><Position><LatitudeDegrees>45</LatitudeDegrees><LongitudeDegrees>7</LongitudeDegrees></Position><HeartRateBpm><Value>130</Value></HeartRateBpm></Trackpoint>
<Trackpoint><Time>2024-05-01T10:00:02Z</Time><Position><LatitudeDegrees>45.0001</LatitudeDegrees><LongitudeDegrees>7</LongitudeDegrees></Position><HeartRateBpm><Value>134</Value></HeartRateBpm></Trackpoint>
</Track></Lap></Activity></Activities></TrainingCenterDatabase>"#;
        assert!(sniff_tcx(tcx.as_bytes()));
        let t = parse_tcx(tcx.as_bytes(), "run.tcx").unwrap();
        assert_eq!(t.len(), 2);
        assert_eq!(t.value(Metric::HeartRate, 1.0), Some(132.0));
        assert_eq!(t.sport.as_deref(), Some("Running"));
        assert_eq!(t.laps.len(), 1);
    }

    #[test]
    fn rejects_empty() {
        assert!(parse_gpx(b"<gpx></gpx>", "x").is_err());
        assert!(parse_gpx(b"<gpx><trk", "x").is_err() || parse_gpx(b"<gpx><trk", "x").is_err());
    }
}
