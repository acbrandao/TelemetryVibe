//! Small shared helpers.

pub mod paths;
pub mod timecode;

/// Linear interpolation.
#[inline]
pub fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Picks a "nice" step (1, 2, 2.5, 5 × 10^n) so that `span / step` is close to `target_count`.
pub fn nice_step(span: f64, target_count: f64) -> f64 {
    if !(span.is_finite()) || span <= 0.0 || target_count <= 0.0 {
        return 1.0;
    }
    let raw = span / target_count;
    let mag = 10f64.powf(raw.log10().floor());
    let norm = raw / mag;
    let nice = if norm <= 1.0 {
        1.0
    } else if norm <= 2.0 {
        2.0
    } else if norm <= 2.5 {
        2.5
    } else if norm <= 5.0 {
        5.0
    } else {
        10.0
    };
    nice * mag
}

/// Rounds a maximum value up to a nice gauge limit (e.g. 47.3 → 50, 183 → 200).
pub fn nice_ceil(value: f64) -> f64 {
    if !value.is_finite() || value <= 0.0 {
        return 1.0;
    }
    let step = nice_step(value, 5.0);
    (value / step).ceil() * step
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nice_steps() {
        assert_eq!(nice_step(60.0, 6.0), 10.0);
        assert_eq!(nice_step(100.0, 4.0), 25.0);
        assert_eq!(nice_step(1.0, 5.0), 0.2);
        assert_eq!(nice_ceil(47.3), 50.0);
        assert_eq!(nice_ceil(183.0), 200.0);
    }
}
