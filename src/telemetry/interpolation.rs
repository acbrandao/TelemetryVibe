//! Per-metric interpolation between telemetry samples.
//!
//! GPS devices typically record at 1–10 Hz while video runs at 24–120 fps, so every
//! gauge value is interpolated between the surrounding valid samples. Gaps are bridged
//! up to [`MAX_GAP`]; short holds at the edges avoid flicker at the recording boundaries.

/// Interpolation strategy for a metric.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InterpKind {
    /// Straight line between neighbors (speed, power, cadence, HR...).
    Linear,
    /// Shortest-arc interpolation for angles in degrees (heading).
    Circular,
    /// Cubic Hermite (Catmull-Rom style) for smooth curves (altitude).
    Cubic,
    /// Hold the previous sample.
    Step,
}

/// Samples further apart than this are not bridged.
pub const MAX_GAP: f64 = 60.0;
/// How long a lone value is held next to a gap or at the edges.
pub const HOLD: f64 = 2.0;
/// Number of neighbors scanned for a valid value around NaN runs.
const SCAN: usize = 256;

/// Returns the interpolated channel value at time `t`, or `None` when no data is near.
pub fn sample_channel(times: &[f64], values: &[f64], t: f64, kind: InterpKind) -> Option<f64> {
    let n = times.len().min(values.len());
    if n == 0 || !t.is_finite() {
        return None;
    }
    if t < times[0] - HOLD || t > times[n - 1] + HOLD {
        return None;
    }
    // Index of the last sample at or before t.
    let idx = times[..n].partition_point(|&x| x <= t);
    let prev = (idx > 0)
        .then(|| find_valid_back(values, idx - 1))
        .flatten();
    let next = find_valid_fwd(values, idx, n);

    match (prev, next) {
        (Some(a), Some(b)) if a == b => Some(values[a]),
        (Some(a), Some(b)) => {
            let (ta, tb) = (times[a], times[b]);
            if tb - ta > MAX_GAP {
                return hold(times, values, t, prev, next);
            }
            let f = ((t - ta) / (tb - ta)).clamp(0.0, 1.0);
            let (va, vb) = (values[a], values[b]);
            Some(match kind {
                InterpKind::Linear => va + (vb - va) * f,
                InterpKind::Step => va,
                InterpKind::Circular => lerp_angle(va, vb, f),
                InterpKind::Cubic => {
                    let before = (a > 0).then(|| find_valid_back(values, a - 1)).flatten();
                    let after = find_valid_fwd(values, b + 1, n);
                    let dt = tb - ta;
                    let slope = (vb - va) / dt;
                    let m0 = before
                        .filter(|&i| ta - times[i] <= MAX_GAP)
                        .map(|i| (vb - values[i]) / (tb - times[i]))
                        .unwrap_or(slope);
                    let m1 = after
                        .filter(|&i| times[i] - tb <= MAX_GAP)
                        .map(|i| (values[i] - va) / (times[i] - ta))
                        .unwrap_or(slope);
                    hermite(va, vb, m0 * dt, m1 * dt, f)
                }
            })
        }
        _ => hold(times, values, t, prev, next),
    }
}

fn hold(
    times: &[f64],
    values: &[f64],
    t: f64,
    prev: Option<usize>,
    next: Option<usize>,
) -> Option<f64> {
    if let Some(a) = prev
        && (t - times[a]).abs() <= HOLD
    {
        return Some(values[a]);
    }
    if let Some(b) = next
        && (times[b] - t).abs() <= HOLD
    {
        return Some(values[b]);
    }
    None
}

fn find_valid_back(values: &[f64], from: usize) -> Option<usize> {
    let lo = from.saturating_sub(SCAN);
    (lo..=from).rev().find(|&i| !values[i].is_nan())
}

fn find_valid_fwd(values: &[f64], from: usize, n: usize) -> Option<usize> {
    let hi = (from + SCAN).min(n);
    (from..hi).find(|&i| !values[i].is_nan())
}

/// Interpolates angles in degrees along the shortest arc, result in `0..360`.
pub fn lerp_angle(a: f64, b: f64, f: f64) -> f64 {
    let mut d = (b - a) % 360.0;
    if d > 180.0 {
        d -= 360.0;
    } else if d < -180.0 {
        d += 360.0;
    }
    (a + d * f).rem_euclid(360.0)
}

fn hermite(p0: f64, p1: f64, m0: f64, m1: f64, t: f64) -> f64 {
    let t2 = t * t;
    let t3 = t2 * t;
    (2.0 * t3 - 3.0 * t2 + 1.0) * p0
        + (t3 - 2.0 * t2 + t) * m0
        + (-2.0 * t3 + 3.0 * t2) * p1
        + (t3 - t2) * m1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_between_samples() {
        let times = [0.0, 1.0, 2.0];
        let vals = [0.0, 10.0, 20.0];
        assert_eq!(
            sample_channel(&times, &vals, 0.5, InterpKind::Linear),
            Some(5.0)
        );
        // 30 fps frame between 10 Hz samples.
        let v = sample_channel(&times, &vals, 1.0 + 1.0 / 30.0, InterpKind::Linear).unwrap();
        assert!((v - 10.333333).abs() < 1e-4);
        assert_eq!(
            sample_channel(&times, &vals, 2.0, InterpKind::Linear),
            Some(20.0)
        );
    }

    #[test]
    fn bridges_missing_values() {
        let times = [0.0, 1.0, 2.0, 3.0];
        let vals = [0.0, f64::NAN, f64::NAN, 30.0];
        assert_eq!(
            sample_channel(&times, &vals, 1.5, InterpKind::Linear),
            Some(15.0)
        );
    }

    #[test]
    fn large_gaps_are_not_bridged() {
        let times = [0.0, 1000.0];
        let vals = [5.0, 7.0];
        assert_eq!(
            sample_channel(&times, &vals, 1.0, InterpKind::Linear),
            Some(5.0)
        );
        assert_eq!(
            sample_channel(&times, &vals, 500.0, InterpKind::Linear),
            None
        );
    }

    #[test]
    fn edges() {
        let times = [10.0, 11.0];
        let vals = [1.0, 2.0];
        assert_eq!(
            sample_channel(&times, &vals, 9.0, InterpKind::Linear),
            Some(1.0)
        );
        assert_eq!(sample_channel(&times, &vals, 5.0, InterpKind::Linear), None);
        assert_eq!(
            sample_channel(&times, &vals, 12.5, InterpKind::Linear),
            Some(2.0)
        );
        assert_eq!(sample_channel(&[], &[], 0.0, InterpKind::Linear), None);
    }

    #[test]
    fn circular_wraps() {
        let times = [0.0, 1.0];
        let vals = [350.0, 10.0];
        let v = sample_channel(&times, &vals, 0.5, InterpKind::Circular).unwrap();
        assert!(v.abs() < 1e-9 || (v - 360.0).abs() < 1e-9, "{v}");
        let v = sample_channel(&times, &vals, 0.25, InterpKind::Circular).unwrap();
        assert!((v - 355.0).abs() < 1e-9);
    }

    #[test]
    fn cubic_passes_through_samples_and_is_smooth() {
        let times = [0.0, 1.0, 2.0, 3.0];
        let vals = [0.0, 1.0, 4.0, 9.0];
        assert_eq!(
            sample_channel(&times, &vals, 1.0, InterpKind::Cubic),
            Some(1.0)
        );
        let v = sample_channel(&times, &vals, 1.5, InterpKind::Cubic).unwrap();
        assert!((v - 2.25).abs() < 0.2, "{v}");
    }
}
