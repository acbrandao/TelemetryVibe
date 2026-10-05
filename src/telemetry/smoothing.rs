//! Smoothing filters.
//!
//! * [`gaussian_smooth`] pre-filters a whole channel (used for altitude and derived values).
//! * [`smoothed`] is the per-gauge visual low-pass filter controlled by the 0–100 % smoothing
//!   slider. It is *stateless*: the value at time `t` is a Gaussian-weighted average of the
//!   interpolated signal around `t`. This makes preview and final render produce identical
//!   values regardless of playback order, scrubbing or frame rate.

use super::interpolation::{InterpKind, sample_channel};

/// Converts the 0..=1 smoothing amount into the Gaussian sigma in seconds.
pub fn sigma_for_amount(amount: f32) -> f64 {
    let a = amount.clamp(0.0, 1.0) as f64;
    // Quadratic response: subtle at low values, up to 2.5 s at 100 %.
    a * a * 2.5
}

/// Visual smoothing of a channel at time `t`.
pub fn smoothed(
    times: &[f64],
    values: &[f64],
    t: f64,
    kind: InterpKind,
    amount: f32,
) -> Option<f64> {
    let sigma = sigma_for_amount(amount);
    if sigma < 0.02 {
        return sample_channel(times, values, t, kind);
    }
    const TAPS: i32 = 12;
    let step = 2.5 * sigma / TAPS as f64;
    let mut wsum = 0.0;
    let mut acc = 0.0;
    let (mut sx, mut sy) = (0.0, 0.0);
    for k in -TAPS..=TAPS {
        let dt = k as f64 * step;
        let Some(v) = sample_channel(times, values, t + dt, kind) else {
            continue;
        };
        let w = (-0.5 * (dt / sigma).powi(2)).exp();
        wsum += w;
        if kind == InterpKind::Circular {
            let r = v.to_radians();
            sx += w * r.cos();
            sy += w * r.sin();
        } else {
            acc += w * v;
        }
    }
    if wsum <= 0.0 {
        return None;
    }
    // Do not invent values where the raw signal has none.
    sample_channel(times, values, t, kind)?;
    Some(if kind == InterpKind::Circular {
        (sy.atan2(sx).to_degrees() + 360.0) % 360.0
    } else {
        acc / wsum
    })
}

/// Gaussian smoothing of an irregularly sampled channel (NaNs are skipped and preserved).
pub fn gaussian_smooth(times: &[f64], values: &[f64], sigma: f64) -> Vec<f64> {
    let n = times.len().min(values.len());
    let mut out = vec![f64::NAN; n];
    if sigma <= 0.0 {
        out.copy_from_slice(&values[..n]);
        return out;
    }
    let reach = 3.0 * sigma;
    let mut lo = 0;
    let mut hi = 0;
    for i in 0..n {
        if values[i].is_nan() {
            continue;
        }
        let t = times[i];
        while lo < n && times[lo] < t - reach {
            lo += 1;
        }
        if hi < i {
            hi = i;
        }
        while hi + 1 < n && times[hi + 1] <= t + reach {
            hi += 1;
        }
        let (mut acc, mut wsum) = (0.0, 0.0);
        for j in lo..=hi {
            let v = values[j];
            if v.is_nan() {
                continue;
            }
            let d = (times[j] - t) / sigma;
            let w = (-0.5 * d * d).exp();
            acc += w * v;
            wsum += w;
        }
        out[i] = if wsum > 0.0 { acc / wsum } else { values[i] };
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_amount_is_raw() {
        let times = [0.0, 1.0, 2.0];
        let vals = [0.0, 10.0, 0.0];
        assert_eq!(
            smoothed(&times, &vals, 1.0, InterpKind::Linear, 0.0),
            Some(10.0)
        );
    }

    #[test]
    fn smoothing_reduces_spikes() {
        let times: Vec<f64> = (0..20).map(|i| i as f64).collect();
        let mut vals = vec![0.0; 20];
        vals[10] = 100.0;
        let raw = smoothed(&times, &vals, 10.0, InterpKind::Linear, 0.0).unwrap();
        let smooth = smoothed(&times, &vals, 10.0, InterpKind::Linear, 1.0).unwrap();
        assert!(smooth < raw * 0.5, "{smooth}");
        assert!(smooth > 0.0);
    }

    #[test]
    fn deterministic() {
        let times: Vec<f64> = (0..50).map(|i| i as f64 * 0.5).collect();
        let vals: Vec<f64> = times.iter().map(|t| (t * 0.7f64).sin() * 10.0).collect();
        let a = smoothed(&times, &vals, 7.3, InterpKind::Linear, 0.7);
        let b = smoothed(&times, &vals, 7.3, InterpKind::Linear, 0.7);
        assert_eq!(a, b);
    }

    #[test]
    fn circular_smoothing_wraps() {
        let times = [0.0, 1.0, 2.0, 3.0, 4.0];
        let vals = [358.0, 359.0, 0.0, 1.0, 2.0];
        let v = smoothed(&times, &vals, 2.0, InterpKind::Circular, 0.8).unwrap();
        assert!(!(5.0..=355.0).contains(&v), "{v}");
    }

    #[test]
    fn gaussian_preserves_constant() {
        let times: Vec<f64> = (0..10).map(|i| i as f64).collect();
        let vals = vec![5.0; 10];
        let s = gaussian_smooth(&times, &vals, 2.0);
        assert!(s.iter().all(|v| (v - 5.0).abs() < 1e-9));
    }
}
