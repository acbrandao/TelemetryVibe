//! Garmin FIT file parser.
//!
//! Implements the FIT binary protocol directly: file headers (12/14 byte), definition and
//! data messages, compressed timestamp headers, big/little endian architectures, developer
//! fields (named through `field_description` messages), chained FIT files and CRC checking.
//! Truncated files (e.g. a device that lost power) are parsed up to the last complete message.
//!
//! Known `record` fields are mapped to the normalized model; every other numeric field is
//! preserved as an extra channel so it can be bound to custom gauges later.

use std::collections::HashMap;

use super::model::{RawSample, Track, TrackBuilder};

/// Seconds between the Unix epoch and the FIT epoch (1989-12-31T00:00:00Z).
pub const FIT_EPOCH_OFFSET: f64 = 631_065_600.0;

#[derive(Debug, thiserror::Error)]
pub enum FitError {
    #[error("file is too short to be a FIT file")]
    TooShort,
    #[error("missing .FIT signature")]
    BadSignature,
    #[error("unsupported FIT header size {0}")]
    BadHeader(u8),
    #[error("data message references undefined local message type {0}")]
    UndefinedLocal(u8),
    #[error("file contains no record messages with timestamps")]
    NoRecords,
}

/// Global message numbers used by the profile mapping.
pub mod mesg {
    pub const FILE_ID: u16 = 0;
    pub const SESSION: u16 = 18;
    pub const LAP: u16 = 19;
    pub const RECORD: u16 = 20;
    pub const GPS_METADATA: u16 = 160;
    pub const FIELD_DESCRIPTION: u16 = 206;
}

/// Base type numbers (low 5 bits of the base type byte).
pub mod base {
    pub const ENUM: u8 = 0x00;
    pub const SINT8: u8 = 0x01;
    pub const UINT8: u8 = 0x02;
    pub const SINT16: u8 = 0x83;
    pub const UINT16: u8 = 0x84;
    pub const SINT32: u8 = 0x85;
    pub const UINT32: u8 = 0x86;
    pub const STRING: u8 = 0x07;
    pub const FLOAT32: u8 = 0x88;
    pub const FLOAT64: u8 = 0x89;
    pub const UINT8Z: u8 = 0x0A;
    pub const UINT16Z: u8 = 0x8B;
    pub const UINT32Z: u8 = 0x8C;
    pub const BYTE: u8 = 0x0D;
    pub const SINT64: u8 = 0x8E;
    pub const UINT64: u8 = 0x8F;
    pub const UINT64Z: u8 = 0x90;
}

const ELEMENT_SIZE: [usize; 17] = [1, 1, 1, 2, 2, 4, 4, 1, 4, 8, 1, 2, 4, 1, 8, 8, 8];

/// A decoded field value.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Num(f64),
    Text(String),
    Invalid,
}

impl Value {
    pub fn num(&self) -> Option<f64> {
        match self {
            Value::Num(v) => Some(*v),
            _ => None,
        }
    }
    pub fn text(&self) -> Option<&str> {
        match self {
            Value::Text(s) => Some(s),
            _ => None,
        }
    }
}

/// A decoded data message.
#[derive(Clone, Debug)]
pub struct Message {
    pub global: u16,
    pub fields: Vec<(u8, Value)>,
    /// Developer fields keyed by (developer data index, field number).
    pub dev_fields: Vec<((u8, u8), Value)>,
    /// Timestamp (FIT seconds), from field 253 or a compressed header.
    pub timestamp: Option<u32>,
}

impl Message {
    pub fn get(&self, num: u8) -> Option<f64> {
        self.fields
            .iter()
            .find(|(n, _)| *n == num)
            .and_then(|(_, v)| v.num())
    }
    pub fn get_text(&self, num: u8) -> Option<&str> {
        self.fields
            .iter()
            .find(|(n, _)| *n == num)
            .and_then(|(_, v)| v.text())
    }
}

#[derive(Clone, Debug)]
struct FieldDef {
    num: u8,
    size: u8,
    base: u8,
}

#[derive(Clone, Debug)]
struct DevFieldDef {
    num: u8,
    size: u8,
    dev_index: u8,
}

#[derive(Clone, Debug)]
struct MsgDef {
    global: u16,
    big_endian: bool,
    fields: Vec<FieldDef>,
    dev_fields: Vec<DevFieldDef>,
}

#[derive(Clone, Debug)]
struct DevFieldDesc {
    name: String,
    base: u8,
    scale: f64,
    offset: f64,
}

/// Result of decoding the raw message stream.
#[derive(Debug, Default)]
pub struct Decoded {
    pub messages: Vec<Message>,
    pub crc_ok: bool,
    pub truncated: bool,
    dev_desc: HashMap<(u8, u8), DevFieldDesc>,
}

const CRC_TABLE: [u16; 16] = [
    0x0000, 0xCC01, 0xD801, 0x1400, 0xF001, 0x3C00, 0x2800, 0xE401, 0xA001, 0x6C00, 0x7800, 0xB401,
    0x5000, 0x9C01, 0x8801, 0x4400,
];

/// FIT CRC-16.
pub fn crc16(mut crc: u16, bytes: &[u8]) -> u16 {
    for &b in bytes {
        let mut tmp = CRC_TABLE[(crc & 0xF) as usize];
        crc = (crc >> 4) & 0x0FFF;
        crc = crc ^ tmp ^ CRC_TABLE[(b & 0xF) as usize];
        tmp = CRC_TABLE[(crc & 0xF) as usize];
        crc = (crc >> 4) & 0x0FFF;
        crc = crc ^ tmp ^ CRC_TABLE[((b >> 4) & 0xF) as usize];
    }
    crc
}

/// Quick check whether bytes look like a FIT file.
pub fn sniff(bytes: &[u8]) -> bool {
    bytes.len() >= 12 && &bytes[8..12] == b".FIT"
}

fn read_value(data: &[u8], base_type: u8, big_endian: bool) -> Value {
    let idx = (base_type & 0x1F) as usize;
    if idx == 7 {
        let end = data.iter().position(|&b| b == 0).unwrap_or(data.len());
        return match std::str::from_utf8(&data[..end]) {
            Ok(s) if !s.is_empty() => Value::Text(s.to_string()),
            _ => Value::Invalid,
        };
    }
    let Some(&esize) = ELEMENT_SIZE.get(idx) else {
        return Value::Invalid;
    };
    if data.len() < esize {
        return Value::Invalid;
    }
    let d = &data[..esize];
    macro_rules! rd {
        ($t:ty, $n:expr) => {{
            let mut a = [0u8; $n];
            a.copy_from_slice(d);
            if big_endian {
                <$t>::from_be_bytes(a)
            } else {
                <$t>::from_le_bytes(a)
            }
        }};
    }
    let v: Option<f64> = match idx {
        0 | 2 | 13 => (d[0] != 0xFF).then_some(d[0] as f64),
        1 => (d[0] != 0x7F).then_some(d[0] as i8 as f64),
        10 => (d[0] != 0).then_some(d[0] as f64),
        3 => {
            let x = rd!(i16, 2);
            (x != 0x7FFF).then_some(x as f64)
        }
        4 => {
            let x = rd!(u16, 2);
            (x != 0xFFFF).then_some(x as f64)
        }
        11 => {
            let x = rd!(u16, 2);
            (x != 0).then_some(x as f64)
        }
        5 => {
            let x = rd!(i32, 4);
            (x != 0x7FFF_FFFF).then_some(x as f64)
        }
        6 => {
            let x = rd!(u32, 4);
            (x != 0xFFFF_FFFF).then_some(x as f64)
        }
        12 => {
            let x = rd!(u32, 4);
            (x != 0).then_some(x as f64)
        }
        8 => {
            let bits = rd!(u32, 4);
            let x = f32::from_bits(bits);
            (bits != 0xFFFF_FFFF && x.is_finite()).then_some(x as f64)
        }
        9 => {
            let bits = rd!(u64, 8);
            let x = f64::from_bits(bits);
            (bits != u64::MAX && x.is_finite()).then_some(x)
        }
        14 => {
            let x = rd!(i64, 8);
            (x != i64::MAX).then_some(x as f64)
        }
        15 => {
            let x = rd!(u64, 8);
            (x != u64::MAX).then_some(x as f64)
        }
        16 => {
            let x = rd!(u64, 8);
            (x != 0).then_some(x as f64)
        }
        _ => None,
    };
    v.map(Value::Num).unwrap_or(Value::Invalid)
}

/// Decodes all messages from a (possibly chained) FIT file.
pub fn decode(bytes: &[u8]) -> Result<Decoded, FitError> {
    if bytes.len() < 12 {
        return Err(FitError::TooShort);
    }
    let mut out = Decoded {
        crc_ok: true,
        ..Default::default()
    };
    let mut pos = 0usize;
    let mut first = true;
    while pos + 12 <= bytes.len() {
        let header_size = bytes[pos];
        if header_size != 12 && header_size != 14 {
            if first {
                return Err(FitError::BadHeader(header_size));
            }
            break;
        }
        if pos + header_size as usize > bytes.len() {
            break;
        }
        if &bytes[pos + 8..pos + 12] != b".FIT" {
            if first {
                return Err(FitError::BadSignature);
            }
            break;
        }
        let data_size = u32::from_le_bytes([
            bytes[pos + 4],
            bytes[pos + 5],
            bytes[pos + 6],
            bytes[pos + 7],
        ]) as usize;
        let data_start = pos + header_size as usize;
        let mut data_end = data_start + data_size;
        if data_end > bytes.len() {
            out.truncated = true;
            data_end = bytes.len();
        } else if data_end + 2 <= bytes.len() {
            let stored = u16::from_le_bytes([bytes[data_end], bytes[data_end + 1]]);
            if crc16(0, &bytes[pos..data_end]) != stored {
                out.crc_ok = false;
            }
        } else {
            out.crc_ok = false;
        }
        match decode_records(&bytes[data_start..data_end], &mut out) {
            Ok(()) => {}
            Err(e) if out.messages.is_empty() => return Err(e),
            Err(e) => {
                tracing::warn!("FIT stream stopped early: {e}");
                out.truncated = true;
                break;
            }
        }
        first = false;
        pos = data_end + 2;
    }
    Ok(out)
}

fn decode_records(data: &[u8], out: &mut Decoded) -> Result<(), FitError> {
    let mut defs: HashMap<u8, MsgDef> = HashMap::new();
    let mut last_ts: Option<u32> = None;
    let mut p = 0usize;
    while p < data.len() {
        let h = data[p];
        p += 1;
        if h & 0x80 != 0 {
            // Compressed timestamp header.
            let local = (h >> 5) & 0x03;
            let offset = (h & 0x1F) as u32;
            let ts = last_ts.map(|last| {
                let base = last & !0x1F;
                if offset >= (last & 0x1F) {
                    base + offset
                } else {
                    base + offset + 0x20
                }
            });
            if let Some(t) = ts {
                last_ts = Some(t);
            }
            let def = defs.get(&local).ok_or(FitError::UndefinedLocal(local))?;
            let (msg, used) =
                read_data(&data[p..], def, &out.dev_desc).ok_or(FitError::TooShort)?;
            p += used;
            let mut msg = msg;
            if msg.timestamp.is_none() {
                msg.timestamp = ts;
            }
            handle_message(msg, out, &mut last_ts);
        } else if h & 0x40 != 0 {
            let local = h & 0x0F;
            let has_dev = h & 0x20 != 0;
            if p + 5 > data.len() {
                return Err(FitError::TooShort);
            }
            let big_endian = data[p + 1] == 1;
            let global = if big_endian {
                u16::from_be_bytes([data[p + 2], data[p + 3]])
            } else {
                u16::from_le_bytes([data[p + 2], data[p + 3]])
            };
            let nfields = data[p + 4] as usize;
            p += 5;
            if p + nfields * 3 > data.len() {
                return Err(FitError::TooShort);
            }
            let fields = (0..nfields)
                .map(|i| FieldDef {
                    num: data[p + i * 3],
                    size: data[p + i * 3 + 1],
                    base: data[p + i * 3 + 2],
                })
                .collect();
            p += nfields * 3;
            let mut dev_fields = Vec::new();
            if has_dev {
                if p >= data.len() {
                    return Err(FitError::TooShort);
                }
                let ndev = data[p] as usize;
                p += 1;
                if p + ndev * 3 > data.len() {
                    return Err(FitError::TooShort);
                }
                for i in 0..ndev {
                    dev_fields.push(DevFieldDef {
                        num: data[p + i * 3],
                        size: data[p + i * 3 + 1],
                        dev_index: data[p + i * 3 + 2],
                    });
                }
                p += ndev * 3;
            }
            defs.insert(
                local,
                MsgDef {
                    global,
                    big_endian,
                    fields,
                    dev_fields,
                },
            );
        } else {
            let local = h & 0x0F;
            let def = defs.get(&local).ok_or(FitError::UndefinedLocal(local))?;
            let (msg, used) =
                read_data(&data[p..], def, &out.dev_desc).ok_or(FitError::TooShort)?;
            p += used;
            handle_message(msg, out, &mut last_ts);
        }
    }
    Ok(())
}

fn read_data(
    data: &[u8],
    def: &MsgDef,
    dev_desc: &HashMap<(u8, u8), DevFieldDesc>,
) -> Option<(Message, usize)> {
    let mut p = 0usize;
    let mut msg = Message {
        global: def.global,
        fields: Vec::with_capacity(def.fields.len()),
        dev_fields: Vec::new(),
        timestamp: None,
    };
    for f in &def.fields {
        let size = f.size as usize;
        let bytes = data.get(p..p + size)?;
        let v = read_value(bytes, f.base, def.big_endian);
        if f.num == 253
            && let Value::Num(ts) = v
        {
            msg.timestamp = Some(ts as u32);
        }
        msg.fields.push((f.num, v));
        p += size;
    }
    for f in &def.dev_fields {
        let size = f.size as usize;
        let bytes = data.get(p..p + size)?;
        let key = (f.dev_index, f.num);
        let v = match dev_desc.get(&key) {
            Some(d) => match read_value(bytes, d.base, def.big_endian) {
                Value::Num(x) => Value::Num(x / d.scale - d.offset),
                other => other,
            },
            None => Value::Invalid,
        };
        msg.dev_fields.push((key, v));
        p += size;
    }
    Some((msg, p))
}

fn handle_message(msg: Message, out: &mut Decoded, last_ts: &mut Option<u32>) {
    if let Some(ts) = msg.timestamp {
        *last_ts = Some(ts);
    }
    if msg.global == mesg::FIELD_DESCRIPTION
        && let (Some(idx), Some(num), Some(bt)) = (msg.get(0), msg.get(1), msg.get(2))
    {
        let name = msg
            .get_text(3)
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("dev_{}_{}", idx as u8, num as u8));
        let scale = msg.get(6).filter(|s| *s > 0.0).unwrap_or(1.0);
        let offset = msg.get(7).unwrap_or(0.0);
        out.dev_desc.insert(
            (idx as u8, num as u8),
            DevFieldDesc {
                name,
                base: bt as u8,
                scale,
                offset,
            },
        );
    }
    out.messages.push(msg);
}

/// Names and scale/offset for non-core `record` fields preserved as extra channels.
const RECORD_EXTRAS: &[(u8, &str, f64, f64)] = &[
    (10, "resistance", 1.0, 0.0),
    (12, "cycle_length_m", 100.0, 0.0),
    (19, "total_cycles", 1.0, 0.0),
    (29, "accumulated_power", 1.0, 0.0),
    (30, "left_right_balance", 1.0, 0.0),
    (31, "gps_accuracy_m", 1.0, 0.0),
    (32, "vertical_speed", 1000.0, 0.0),
    (33, "calories", 1.0, 0.0),
    (39, "vertical_oscillation_mm", 10.0, 0.0),
    (40, "stance_time_percent", 100.0, 0.0),
    (41, "stance_time_ms", 10.0, 0.0),
    (43, "left_torque_effectiveness", 2.0, 0.0),
    (44, "right_torque_effectiveness", 2.0, 0.0),
    (45, "left_pedal_smoothness", 2.0, 0.0),
    (46, "right_pedal_smoothness", 2.0, 0.0),
    (47, "combined_pedal_smoothness", 2.0, 0.0),
    (54, "total_hemoglobin_conc", 100.0, 0.0),
    (57, "saturated_hemoglobin_percent", 10.0, 0.0),
    (81, "battery_soc", 2.0, 0.0),
    (83, "vertical_ratio", 100.0, 0.0),
    (84, "stance_time_balance", 100.0, 0.0),
    (85, "step_length_mm", 10.0, 0.0),
    (91, "absolute_pressure", 1.0, 0.0),
    (92, "depth_m", 1000.0, 0.0),
    (108, "respiration_rate", 100.0, 0.0),
    (114, "grit", 1.0, 0.0),
    (115, "flow", 1.0, 0.0),
    (136, "wrist_heart_rate", 1.0, 0.0),
    (139, "core_temperature", 100.0, 0.0),
];

/// Record fields mapped onto the core model (never stored as extras).
const RECORD_CORE: &[u8] = &[253, 0, 1, 2, 3, 4, 5, 6, 7, 9, 13, 52, 53, 73, 78];

fn sport_name(v: u8) -> &'static str {
    match v {
        1 => "Running",
        2 => "Cycling",
        4 => "Fitness equipment",
        5 => "Swimming",
        10 => "Training",
        11 => "Walking",
        13 => "Alpine skiing",
        17 => "Hiking",
        21 => "E-biking",
        32 => "Sailing",
        37 => "Stand-up paddleboarding",
        38 => "Surfing",
        43 => "Windsurfing",
        44 => "Kitesurfing",
        _ => "Generic",
    }
}

fn manufacturer_name(v: u16) -> String {
    match v {
        1 => "Garmin".into(),
        15 => "Dynastream".into(),
        32 => "Wahoo".into(),
        123 => "Polar".into(),
        255 => "Development".into(),
        260 => "Zwift".into(),
        265 => "Strava".into(),
        294 => "Coros".into(),
        other => format!("Manufacturer #{other}"),
    }
}

/// GPS-receiver altitude points `(FIT seconds, meters)` from `gps_metadata` messages
/// (`enhanced_altitude`, field 3), sorted by time.
///
/// Garmin devices usually write these without a timestamp of their own: one follows each
/// `record` (same second), and with smart recording several arrive at ~1 Hz between sparse
/// records. Untimed messages are therefore spaced one second apart (or tighter, if needed to
/// fit) ending one second before the next timestamped message. Messages that do carry a
/// timestamp (field 253, plus `timestamp_ms` in field 0) use it directly.
fn gps_altitude_points(messages: &[Message]) -> Vec<(f64, f64)> {
    let mut points: Vec<(f64, f64)> = Vec::new();
    let mut pending: Vec<f64> = Vec::new();
    let mut anchor: Option<f64> = None;
    let flush =
        |pending: &mut Vec<f64>, points: &mut Vec<(f64, f64)>, t0: Option<f64>, t1: Option<f64>| {
            let k = pending.len() as f64;
            for (i, alt) in pending.drain(..).enumerate() {
                let i = i as f64;
                let t = match (t0, t1) {
                    (Some(a), Some(b)) => {
                        let step = ((b - a) / k).clamp(0.0, 1.0);
                        b - (k - i) * step
                    }
                    (None, Some(b)) => b - (k - i),
                    (Some(a), None) => a + i,
                    (None, None) => continue,
                };
                points.push((t, alt));
            }
        };
    for m in messages {
        let alt = (m.global == mesg::GPS_METADATA)
            .then(|| m.get(3).map(|v| v / 5.0 - 500.0))
            .flatten();
        match m.timestamp {
            Some(ts) => {
                let ts = ts as f64;
                flush(&mut pending, &mut points, anchor, Some(ts));
                anchor = Some(ts);
                if let Some(a) = alt {
                    let ms = m.get(0).filter(|v| *v < 1000.0).unwrap_or(0.0);
                    points.push((ts + ms / 1000.0, a));
                }
            }
            None => {
                if let Some(a) = alt {
                    pending.push(a);
                }
            }
        }
    }
    flush(&mut pending, &mut points, anchor, None);
    points.retain(|(_, a)| (-500.0..9000.0).contains(a));
    points.sort_by(|a, b| a.0.total_cmp(&b.0));
    points
}

/// GPS altitude at FIT time `t`: linear between neighbours up to 5 s apart, else the nearest
/// point within 1 s.
fn gps_altitude_at(points: &[(f64, f64)], t: f64) -> Option<f64> {
    let i = points.partition_point(|p| p.0 < t);
    let after = points.get(i);
    let before = i.checked_sub(1).and_then(|j| points.get(j));
    match (before, after) {
        (_, Some(&(ta, a))) if (ta - t).abs() < 1e-6 => Some(a),
        (Some(&(t0, a0)), Some(&(t1, a1))) if t1 - t0 <= 5.0 => {
            Some(a0 + (a1 - a0) * (t - t0) / (t1 - t0))
        }
        (b, a) => [b, a]
            .into_iter()
            .flatten()
            .filter(|p| (p.0 - t).abs() <= 1.0)
            .min_by(|x, y| (x.0 - t).abs().total_cmp(&(y.0 - t).abs()))
            .map(|p| p.1),
    }
}

/// Developer fields carrying GPS altitude under this name are used for the GPS altitude channel.
fn is_gps_altitude_name(name: &str) -> bool {
    let n = name.trim().to_ascii_lowercase().replace([' ', '-'], "_");
    n == "gps_altitude" || n == "gps_alt" || n == "gps_elevation"
}

/// Parses a FIT file into a normalized [`Track`].
pub fn parse(bytes: &[u8], name: &str) -> Result<Track, FitError> {
    let decoded = decode(bytes)?;
    if !decoded.crc_ok {
        tracing::warn!("FIT file {name}: CRC mismatch, continuing with decoded data");
    }
    if decoded.truncated {
        tracing::warn!("FIT file {name}: truncated, using messages decoded so far");
    }
    let mut builder = TrackBuilder::new(name, "Garmin FIT");
    let extras: HashMap<u8, (&str, f64, f64)> = RECORD_EXTRAS
        .iter()
        .map(|&(n, s, sc, off)| (n, (s, sc, off)))
        .collect();
    let gps_alt = gps_altitude_points(&decoded.messages);

    for msg in &decoded.messages {
        match msg.global {
            mesg::RECORD => {
                let Some(ts) = msg.timestamp else { continue };
                let semicircle = 180.0 / 2f64.powi(31);
                let lat = msg.get(0).map(|v| v * semicircle);
                let lon = msg.get(1).map(|v| v * semicircle);
                let (lat, lon) = match (lat, lon) {
                    (Some(a), Some(o))
                        if (-90.0..=90.0).contains(&a)
                            && (-180.0..=180.0).contains(&o)
                            && !(a == 0.0 && o == 0.0) =>
                    {
                        (Some(a), Some(o))
                    }
                    _ => (None, None),
                };
                let altitude = msg.get(78).or_else(|| msg.get(2)).map(|v| v / 5.0 - 500.0);
                let speed = msg.get(73).or_else(|| msg.get(6)).map(|v| v / 1000.0);
                let cadence = match (msg.get(4), msg.get(53)) {
                    (Some(c), Some(f)) => Some(c + f / 128.0),
                    (Some(c), None) => Some(c),
                    (None, _) => msg.get(52).map(|c| c / 256.0),
                };
                let mut sample = RawSample {
                    time: ts as f64 + FIT_EPOCH_OFFSET,
                    latitude: lat,
                    longitude: lon,
                    altitude,
                    gps_altitude: gps_altitude_at(&gps_alt, ts as f64),
                    speed,
                    distance: msg.get(5).map(|v| v / 100.0),
                    heart_rate: msg.get(3).filter(|v| *v > 0.0),
                    cadence,
                    power: msg.get(7),
                    temperature: msg.get(13),
                    heading: None,
                    grade: msg.get(9).map(|v| v / 100.0),
                    extra: Vec::new(),
                };
                for (num, v) in &msg.fields {
                    if RECORD_CORE.contains(num) {
                        continue;
                    }
                    let Some(x) = v.num() else { continue };
                    match extras.get(num) {
                        Some(&(key, scale, off)) => {
                            sample.extra.push((key.to_string(), x / scale - off))
                        }
                        None => sample.extra.push((format!("record_{num}"), x)),
                    }
                }
                for (key, v) in &msg.dev_fields {
                    if let (Some(x), Some(d)) = (v.num(), decoded.dev_desc.get(key)) {
                        if is_gps_altitude_name(&d.name) {
                            sample.gps_altitude = Some(x);
                        } else {
                            sample.extra.push((d.name.clone(), x));
                        }
                    }
                }
                builder.samples.push(sample);
            }
            mesg::LAP => {
                let start = msg.get(2);
                let end = msg.timestamp.map(|t| t as f64);
                let elapsed = msg.get(7).map(|v| v / 1000.0);
                if let Some(s) = start {
                    let e = end.or(elapsed.map(|el| s + el)).unwrap_or(s);
                    builder.laps.push((
                        s + FIT_EPOCH_OFFSET,
                        e + FIT_EPOCH_OFFSET,
                        msg.get(9).map(|v| v / 100.0),
                    ));
                }
            }
            mesg::SESSION => {
                let sport = msg.get(5).map(|sp| sport_name(sp as u8).to_string());
                if builder.sport.is_none() {
                    builder.sport = sport.clone();
                } else if sport.is_some() && builder.sport != sport {
                    builder.sport = Some("Multisport".to_string());
                }
                let start = msg.get(2);
                let end = msg.timestamp.map(|t| t as f64);
                let elapsed = msg.get(7).map(|v| v / 1000.0);
                if let Some(s) = start {
                    let e = end.or(elapsed.map(|el| s + el)).unwrap_or(s);
                    builder
                        .sessions
                        .push((s + FIT_EPOCH_OFFSET, e + FIT_EPOCH_OFFSET, sport));
                }
            }
            mesg::FILE_ID => {
                if let Some(m) = msg.get(1) {
                    builder.device = Some(manufacturer_name(m as u16));
                }
            }
            _ => {}
        }
    }
    builder
        .laps
        .sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    builder.build().ok_or(FitError::NoRecords)
}

/// Minimal FIT encoder. Used by tests and the sample-data generator.
pub mod writer {
    use super::crc16;

    /// A value written into a data message, encoded according to the field's base type.
    #[derive(Clone, Copy, Debug)]
    pub enum W {
        U8(u8),
        I8(i8),
        U16(u16),
        I16(i16),
        U32(u32),
        I32(i32),
    }

    impl W {
        fn bytes(self) -> Vec<u8> {
            match self {
                W::U8(v) => vec![v],
                W::I8(v) => vec![v as u8],
                W::U16(v) => v.to_le_bytes().to_vec(),
                W::I16(v) => v.to_le_bytes().to_vec(),
                W::U32(v) => v.to_le_bytes().to_vec(),
                W::I32(v) => v.to_le_bytes().to_vec(),
            }
        }
    }

    #[derive(Default)]
    pub struct FitWriter {
        data: Vec<u8>,
    }

    impl FitWriter {
        pub fn new() -> Self {
            Self::default()
        }

        /// Writes a definition message: fields are `(field number, size, base type)`.
        pub fn define(&mut self, local: u8, global: u16, fields: &[(u8, u8, u8)]) {
            self.data.push(0x40 | (local & 0x0F));
            self.data.push(0); // reserved
            self.data.push(0); // little endian
            self.data.extend_from_slice(&global.to_le_bytes());
            self.data.push(fields.len() as u8);
            for &(num, size, base) in fields {
                self.data.extend_from_slice(&[num, size, base]);
            }
        }

        /// Writes a data message; values must match the definition order and sizes.
        pub fn data(&mut self, local: u8, values: &[W]) {
            self.data.push(local & 0x0F);
            for v in values {
                self.data.extend(v.bytes());
            }
        }

        /// Writes a compressed-timestamp data message.
        pub fn data_compressed(&mut self, local: u8, time_offset: u8, values: &[W]) {
            self.data
                .push(0x80 | ((local & 0x03) << 5) | (time_offset & 0x1F));
            for v in values {
                self.data.extend(v.bytes());
            }
        }

        /// Produces the complete file with a 14-byte header and CRCs.
        pub fn finish(self) -> Vec<u8> {
            let mut out = Vec::with_capacity(self.data.len() + 16);
            out.push(14);
            out.push(0x20); // protocol 2.0
            out.extend_from_slice(&2132u16.to_le_bytes());
            out.extend_from_slice(&(self.data.len() as u32).to_le_bytes());
            out.extend_from_slice(b".FIT");
            let hcrc = crc16(0, &out[..12]);
            out.extend_from_slice(&hcrc.to_le_bytes());
            out.extend_from_slice(&self.data);
            let crc = crc16(0, &out);
            out.extend_from_slice(&crc.to_le_bytes());
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::writer::{FitWriter, W};
    use super::*;
    use crate::telemetry::model::Metric;

    const T0: u32 = 1_000_000_000; // FIT seconds (2021-09-08)

    fn deg_to_semi(d: f64) -> i32 {
        (d * 2f64.powi(31) / 180.0) as i32
    }

    fn sample_file() -> Vec<u8> {
        let mut w = FitWriter::new();
        w.define(
            0,
            mesg::FILE_ID,
            &[(0, 1, base::ENUM), (1, 2, base::UINT16)],
        );
        w.data(0, &[W::U8(4), W::U16(1)]);
        w.define(
            1,
            mesg::RECORD,
            &[
                (253, 4, base::UINT32),
                (0, 4, base::SINT32),
                (1, 4, base::SINT32),
                (78, 4, base::UINT32),
                (73, 4, base::UINT32),
                (5, 4, base::UINT32),
                (3, 1, base::UINT8),
                (4, 1, base::UINT8),
                (7, 2, base::UINT16),
                (13, 1, base::SINT8),
                (39, 2, base::UINT16),
            ],
        );
        for i in 0..10u32 {
            w.data(
                1,
                &[
                    W::U32(T0 + i),
                    W::I32(deg_to_semi(45.0 + i as f64 * 0.0001)),
                    W::I32(deg_to_semi(7.0)),
                    W::U32(((250.0 + i as f64 + 500.0) * 5.0) as u32),
                    W::U32(5000 + i * 100),
                    W::U32(i * 1110),
                    W::U8(if i == 5 { 0xFF } else { 140 + i as u8 }),
                    W::U8(90),
                    W::U16(200 + i as u16 * 10),
                    W::I8(21),
                    W::U16(85),
                ],
            );
        }
        // A record using a compressed timestamp header (local 2, time + 1 s).
        w.define(2, mesg::RECORD, &[(3, 1, base::UINT8)]);
        let offset = ((T0 + 10) & 0x1F) as u8;
        w.data_compressed(2, offset, &[W::U8(160)]);
        w.define(3, mesg::SESSION, &[(5, 1, base::ENUM)]);
        w.data(3, &[W::U8(2)]);
        w.finish()
    }

    #[test]
    fn crc_known_value() {
        // CRC of "123456789" with the FIT polynomial (CRC-16/ARC).
        assert_eq!(crc16(0, b"123456789"), 0xBB3D);
    }

    #[test]
    fn decodes_header_and_crc() {
        let bytes = sample_file();
        assert!(sniff(&bytes));
        let d = decode(&bytes).unwrap();
        assert!(d.crc_ok);
        assert!(!d.truncated);
        assert_eq!(
            d.messages
                .iter()
                .filter(|m| m.global == mesg::RECORD)
                .count(),
            11
        );
    }

    #[test]
    fn extracts_telemetry() {
        let track = parse(&sample_file(), "test.fit").unwrap();
        assert_eq!(track.len(), 11);
        assert_eq!(track.sport.as_deref(), Some("Cycling"));
        assert_eq!(track.device.as_deref(), Some("Garmin"));
        assert!((track.start_unix - (T0 as f64 + FIT_EPOCH_OFFSET)).abs() < 1e-9);
        let lat = track.value(Metric::Latitude, 2.0).unwrap();
        assert!((lat - 45.0002).abs() < 1e-6, "{lat}");
        let speed = track.value(Metric::Speed, 3.0).unwrap();
        assert!((speed - 5.3).abs() < 1e-9);
        let alt = track.value(Metric::Altitude, 0.0).unwrap();
        assert!((alt - 250.0).abs() < 1.5, "{alt}");
        assert_eq!(track.value(Metric::Power, 4.0), Some(240.0));
        assert_eq!(track.value(Metric::Cadence, 1.0), Some(90.0));
        assert_eq!(track.value(Metric::Temperature, 1.0), Some(21.0));
        // Invalid HR sample (0xFF) is bridged by interpolation.
        assert_eq!(track.value(Metric::HeartRate, 5.0), Some(145.0));
        // Compressed timestamp record.
        assert_eq!(track.value(Metric::HeartRate, 10.0), Some(160.0));
        // Distance in meters.
        assert!((track.value(Metric::Distance, 9.0).unwrap() - 99.9).abs() < 1e-9);
        // Extra field preserved.
        assert!(track.extra.contains_key("vertical_oscillation_mm"));
        assert!(
            (track
                .extra_value("vertical_oscillation_mm", 1.0, 0.0)
                .unwrap()
                - 8.5)
                .abs()
                < 1e-9
        );
    }

    /// Garmin layout: each record is followed by an untimed `gps_metadata`; with smart recording
    /// extra ones arrive at 1 Hz between sparse records. True GPS altitude = 100 + t meters.
    fn gps_metadata_file() -> Vec<u8> {
        let alt = |t: u32| W::U32(((100.0 + t as f64 + 500.0) * 5.0) as u32);
        let mut w = FitWriter::new();
        w.define(
            0,
            mesg::RECORD,
            &[(253, 4, base::UINT32), (78, 4, base::UINT32)],
        );
        // Barometric altitude is offset by +10 m so the two channels are distinguishable.
        let baro = |t: u32| W::U32(((110.0 + t as f64 + 500.0) * 5.0) as u32);
        w.define(1, mesg::GPS_METADATA, &[(3, 4, base::UINT32)]);
        for t in 0..5 {
            w.data(0, &[W::U32(T0 + t), baro(t)]);
            w.data(1, &[alt(t)]);
        }
        // Smart recording gap: GPS fixes at 5, 6, 7 without records.
        for t in 5..8 {
            w.data(1, &[alt(t)]);
        }
        w.data(0, &[W::U32(T0 + 8), baro(8)]);
        w.data(1, &[alt(8)]);
        w.finish()
    }

    #[test]
    fn gps_metadata_timing() {
        let d = decode(&gps_metadata_file()).unwrap();
        let pts = gps_altitude_points(&d.messages);
        let rel: Vec<(f64, f64)> = pts.iter().map(|&(t, a)| (t - T0 as f64, a)).collect();
        assert_eq!(rel.len(), 9);
        for (i, (t, a)) in rel.iter().enumerate() {
            assert!((t - i as f64).abs() < 1e-9, "point {i} at {t}");
            assert!((a - (100.0 + i as f64)).abs() < 0.21, "point {i} alt {a}");
        }
        // Interpolation helper.
        let mid = gps_altitude_at(&pts, T0 as f64 + 2.5).unwrap();
        assert!((mid - 102.5).abs() < 0.21);
        assert_eq!(gps_altitude_at(&pts, T0 as f64 + 20.0), None);
    }

    #[test]
    fn gps_altitude_channel() {
        let track = parse(&gps_metadata_file(), "gps.fit").unwrap();
        assert!(track.has(Metric::GpsAltitude));
        assert!(track.has(Metric::Altitude));
        let gps = track.value(Metric::GpsAltitude, 3.0).unwrap();
        let baro = track.value(Metric::Altitude, 3.0).unwrap();
        // Smoothed channels (both get the same light smoothing).
        assert!((gps - 103.0).abs() < 1.0, "gps {gps}");
        assert!((baro - 113.0).abs() < 1.0, "baro {baro}");
        // Raw GPS altitude is exact at record times.
        let raw = track.extra_value("gps_altitude_raw", 3.0, 0.0).unwrap();
        assert!((raw - 103.0).abs() < 0.21, "raw {raw}");
        let raw_gap_end = track.extra_value("gps_altitude_raw", 8.0, 0.0).unwrap();
        assert!((raw_gap_end - 108.0).abs() < 0.21, "raw {raw_gap_end}");
        assert_eq!(
            Metric::preferred_altitude(Some(&track)),
            Metric::GpsAltitude
        );
        // Altitude presets and templates default to GPS altitude.
        let g = crate::gauges::library::make_preset(
            crate::gauges::library::PresetId::DigitalAltitude,
            crate::gauges::model::GaugeId(1),
            (1920.0, 1080.0),
            Some(&track),
            crate::telemetry::UnitSystem::Metric,
        );
        assert_eq!(g.metric, Metric::GpsAltitude);
        // Range comes from the GPS altitude stats (~100–108 m), not the barometric 110–118 m.
        assert!(
            g.min <= 100.0 && g.max >= 107.0 && g.max < 110.0,
            "{} – {}",
            g.min,
            g.max
        );
    }

    #[test]
    fn gps_altitude_falls_back_to_altitude() {
        let track = parse(&sample_file(), "test.fit").unwrap();
        assert!(!track.has(Metric::GpsAltitude));
        assert_eq!(Metric::preferred_altitude(Some(&track)), Metric::Altitude);
        // A gauge bound to GPS altitude still shows the recording's altitude.
        assert_eq!(
            track.value(Metric::GpsAltitude, 2.0),
            track.value(Metric::Altitude, 2.0)
        );
        assert!(track.stats_for(Metric::GpsAltitude).is_some());
        assert!(!is_gps_altitude_name("altitude"));
        assert!(is_gps_altitude_name("GPS Altitude"));
    }

    #[test]
    fn truncated_file_still_parses() {
        let bytes = sample_file();
        let cut = &bytes[..bytes.len() - 40];
        let track = parse(cut, "cut.fit").unwrap();
        assert!(track.len() >= 5);
    }

    #[test]
    fn rejects_garbage() {
        assert!(matches!(parse(b"hello", "x"), Err(FitError::TooShort)));
        let mut bytes = sample_file();
        bytes[8] = b'X';
        assert!(matches!(parse(&bytes, "x"), Err(FitError::BadSignature)));
    }

    #[test]
    fn corrupt_crc_is_tolerated() {
        let mut bytes = sample_file();
        let n = bytes.len();
        bytes[n - 1] ^= 0xFF;
        let d = decode(&bytes).unwrap();
        assert!(!d.crc_ok);
        assert!(parse(&bytes, "x").is_ok());
    }

    #[test]
    fn big_endian_definition() {
        let mut data = Vec::new();
        // definition, local 0, big endian, record, 2 fields
        data.extend_from_slice(&[0x40, 0, 1, 0, 20, 2, 253, 4, 0x86, 7, 2, 0x84]);
        data.push(0);
        data.extend_from_slice(&(T0).to_be_bytes());
        data.extend_from_slice(&321u16.to_be_bytes());
        let mut file = vec![12, 0x10, 0, 0];
        file.extend_from_slice(&(data.len() as u32).to_le_bytes());
        file.extend_from_slice(b".FIT");
        file.extend_from_slice(&data);
        let crc = crc16(0, &file);
        file.extend_from_slice(&crc.to_le_bytes());
        let track = parse(&file, "be").unwrap();
        assert_eq!(track.value(Metric::Power, 0.0), Some(321.0));
    }
}
