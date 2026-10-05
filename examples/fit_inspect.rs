//! Lists the message types, record fields and parsed channels of FIT files.
//! `cargo run --release --example fit_inspect -- ride.fit [more.fit …]`

use std::collections::BTreeMap;

use telemetryvibe::telemetry::{Metric, fit};

fn main() {
    for path in std::env::args().skip(1) {
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("{path}: {e}");
                continue;
            }
        };
        println!("== {path}");
        let Ok(decoded) = fit::decode(&bytes) else {
            println!("   not a FIT file");
            continue;
        };
        let mut globals: BTreeMap<u16, usize> = BTreeMap::new();
        let mut record_fields: BTreeMap<u8, usize> = BTreeMap::new();
        for m in &decoded.messages {
            *globals.entry(m.global).or_default() += 1;
            if m.global == 20 {
                for (n, v) in &m.fields {
                    if v.num().is_some() {
                        *record_fields.entry(*n).or_default() += 1;
                    }
                }
            }
        }
        println!("   messages (global: count): {globals:?}");
        println!("   record fields with data (num: count): {record_fields:?}");
        if std::env::var("FIT_SEQ").is_ok() {
            for m in decoded.messages.iter().skip(400).take(40) {
                println!("   {} ts={:?} {:?}", m.global, m.timestamp, m.fields);
            }
        }
        if std::env::var("FIT_DUMP").is_ok() {
            let mut shown = 0;
            for m in &decoded.messages {
                if (m.global == 160 || m.global == 20) && shown < 12 {
                    println!(
                        "   {} ts={:?} {:?}",
                        m.global,
                        m.timestamp,
                        m.fields
                            .iter()
                            .filter(|(n, _)| [0u8, 1, 2, 3, 4, 5, 6, 78].contains(n))
                            .collect::<Vec<_>>()
                    );
                    shown += 1;
                }
            }
        }
        match fit::parse(&bytes, &path) {
            Ok(t) => {
                println!("   channels: {:?}", t.available_metrics());
                println!("   extra: {:?}", t.extra.keys().collect::<Vec<_>>());
                println!(
                    "   elevation gain/loss: {:.0} / {:.0} m, grade {:?}",
                    t.stats.elevation_gain.unwrap_or(f64::NAN),
                    t.stats.elevation_loss.unwrap_or(f64::NAN),
                    t.stats
                        .get(Metric::Grade)
                        .map(|s| (s.min.round(), s.max.round()))
                );
                if std::env::var("FIT_LAG").is_ok() {
                    // Best time lag between GPS and barometric altitude changes.
                    let mut best = (0.0, f64::INFINITY);
                    for lag10 in -50..=50 {
                        let lag = lag10 as f64 / 10.0;
                        let (mut err, mut n) = (0.0, 0);
                        let mut tt = 30.0;
                        while tt < t.duration() - 30.0 {
                            let d = |m: Metric, x: f64| {
                                Some(t.value(m, x + 5.0)? - t.value(m, x - 5.0)?)
                            };
                            if let (Some(a), Some(b)) =
                                (d(Metric::Altitude, tt), d(Metric::GpsAltitude, tt + lag))
                            {
                                err += (a - b).powi(2);
                                n += 1;
                            }
                            tt += 1.0;
                        }
                        if n > 0 && err / (n as f64) < best.1 {
                            best = (lag, err / n as f64);
                        }
                    }
                    println!("   best GPS-vs-baro lag: {:+.1} s", best.0);
                }
                for m in [Metric::Altitude, Metric::GpsAltitude] {
                    if let (Some(s), Some(ch)) = (t.stats.get(m), t.channel(m)) {
                        let n = ch.iter().filter(|v| !v.is_nan()).count();
                        println!(
                            "   {:<13} {n}/{} samples, {:.1} – {:.1} m (avg {:.1})",
                            m.label(),
                            ch.len(),
                            s.min,
                            s.max,
                            s.avg
                        );
                    }
                }
            }
            Err(e) => println!("   parse error: {e}"),
        }
    }
}
