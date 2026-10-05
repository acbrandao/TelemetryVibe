//! Timecode formatting and parsing helpers.

/// Formats seconds as `HH:MM:SS.mmm`.
pub fn format_timecode(seconds: f64) -> String {
    let neg = seconds < 0.0;
    let ms_total = (seconds.abs() * 1000.0).round() as u64;
    let ms = ms_total % 1000;
    let s = (ms_total / 1000) % 60;
    let m = (ms_total / 60_000) % 60;
    let h = ms_total / 3_600_000;
    format!("{}{h:02}:{m:02}:{s:02}.{ms:03}", if neg { "-" } else { "" })
}

/// Formats seconds compactly for ruler labels (`M:SS` or `H:MM:SS`).
pub fn format_ruler(seconds: f64) -> String {
    let neg = seconds < 0.0;
    let total = seconds.abs();
    let whole = total.floor() as u64;
    let frac = total - whole as f64;
    let s = whole % 60;
    let m = (whole / 60) % 60;
    let h = whole / 3600;
    let sign = if neg { "-" } else { "" };
    let base = if h > 0 {
        format!("{sign}{h}:{m:02}:{s:02}")
    } else {
        format!("{sign}{m}:{s:02}")
    };
    if frac > 0.001 {
        format!("{base}.{:01}", (frac * 10.0).floor() as u32)
    } else {
        base
    }
}

/// Formats a synchronization offset as `+MM:SS.mmm` (or `+H:MM:SS.mmm` beyond an hour).
pub fn format_offset(seconds: f64) -> String {
    let sign = if seconds < 0.0 { '-' } else { '+' };
    let ms_total = (seconds.abs() * 1000.0).round() as u64;
    let ms = ms_total % 1000;
    let s = (ms_total / 1000) % 60;
    let m = (ms_total / 60_000) % 60;
    let h = ms_total / 3_600_000;
    if h > 0 {
        format!("{sign}{h}:{m:02}:{s:02}.{ms:03}")
    } else {
        format!("{sign}{m:02}:{s:02}.{ms:03}")
    }
}

/// Parses offsets such as `+00:03.250`, `-12.8`, `1:02:03.5`, `3`.
pub fn parse_offset(input: &str) -> Option<f64> {
    let s = input.trim();
    if s.is_empty() {
        return None;
    }
    let (sign, body) = match s.as_bytes()[0] {
        b'+' => (1.0, &s[1..]),
        b'-' => (-1.0, &s[1..]),
        _ => (1.0, s),
    };
    let parts: Vec<&str> = body.split(':').collect();
    if parts.is_empty() || parts.len() > 3 {
        return None;
    }
    let mut total = 0.0;
    for (i, part) in parts.iter().enumerate() {
        let part = part.trim();
        if part.is_empty() {
            return None;
        }
        let v: f64 = part.parse().ok()?;
        if !v.is_finite() || v < 0.0 {
            return None;
        }
        // Only the last component may carry a fraction.
        if i + 1 < parts.len() && v.fract() != 0.0 {
            return None;
        }
        total = total * 60.0 + v;
    }
    Some(sign * total)
}

/// Wall-clock time in the local timezone: `HH:MM:SS`, or with milliseconds `HH:MM:SS.mmm`.
///
/// Note: chrono only accepts `%.3f`/`%.6f`/`%.9f`/`%.f` fractional specifiers; anything else
/// makes `to_string()` panic, so all wall-clock labels go through this tested helper.
pub fn format_wall_time(t: chrono::DateTime<chrono::Utc>, millis: bool) -> String {
    let local = t.with_timezone(&chrono::Local);
    if millis {
        local.format("%H:%M:%S%.3f").to_string()
    } else {
        local.format("%H:%M:%S").to_string()
    }
}

/// Formats a duration in a human friendly way (`1h 02m`, `4m 32s`, `12.4s`).
pub fn format_duration_human(seconds: f64) -> String {
    if seconds >= 3600.0 {
        let h = (seconds / 3600.0).floor();
        let m = ((seconds - h * 3600.0) / 60.0).floor();
        format!("{h:.0}h {m:02.0}m")
    } else if seconds >= 60.0 {
        let m = (seconds / 60.0).floor();
        let s = (seconds - m * 60.0).floor();
        format!("{m:.0}m {s:02.0}s")
    } else {
        format!("{seconds:.1}s")
    }
}

/// Formats a byte count (`1.4 GB`).
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = bytes as f64;
    let mut unit = 0;
    while v >= 1024.0 && unit < UNITS.len() - 1 {
        v /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{v:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timecode_formats() {
        assert_eq!(format_timecode(272.24), "00:04:32.240");
        assert_eq!(format_timecode(3723.5), "01:02:03.500");
        assert_eq!(format_timecode(-1.5), "-00:00:01.500");
    }

    #[test]
    fn offset_roundtrip() {
        assert_eq!(format_offset(3.25), "+00:03.250");
        assert_eq!(format_offset(-12.8), "-00:12.800");
        assert_eq!(format_offset(3723.5), "+1:02:03.500");
        for v in [3.25, -12.8, 0.0, 3723.5, -0.001] {
            let parsed = parse_offset(&format_offset(v)).unwrap();
            assert!((parsed - v).abs() < 1e-9, "{v} -> {parsed}");
        }
    }

    #[test]
    fn offset_parse_variants() {
        assert_eq!(parse_offset("3"), Some(3.0));
        assert_eq!(parse_offset("-12.8"), Some(-12.8));
        assert_eq!(parse_offset("1:02:03.5"), Some(3723.5));
        assert_eq!(parse_offset(" +00:03.250 "), Some(3.25));
        assert_eq!(parse_offset(""), None);
        assert_eq!(parse_offset("abc"), None);
        assert_eq!(parse_offset("1.5:00"), None);
        assert_eq!(parse_offset("1:2:3:4"), None);
    }

    #[test]
    fn wall_time_formats() {
        use chrono::TimeZone;
        let t = chrono::Utc
            .timestamp_opt(1_778_923_800, 123_456_789)
            .unwrap();
        let ms = format_wall_time(t, true);
        let s = format_wall_time(t, false);
        assert_eq!(ms.len(), "00:00:00.123".len());
        assert!(ms.ends_with(".123"), "{ms}");
        assert_eq!(s.len(), "00:00:00".len());
        assert!(ms.starts_with(&s));
    }

    #[test]
    fn bytes_format() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1536), "1.5 KB");
    }
}
