//! Coverage in one completed query sample, never a sum of overlapping encoder intervals.
pub(super) fn union(intervals: &mut [(u64, u64)]) -> Option<(u64, bool)> {
    if intervals.is_empty() || intervals.iter().any(|(start, end)| end < start) {
        return None;
    }
    intervals.sort_unstable();
    let (mut start, mut end) = intervals[0];
    let mut ticks = 0_u64;
    let mut overlap = false;
    for &(next_start, next_end) in &intervals[1..] {
        if next_start <= end {
            overlap |= next_start < end && next_end > next_start;
            end = end.max(next_end);
        } else {
            ticks = ticks.checked_add(end - start)?;
            (start, end) = (next_start, next_end);
        }
    }
    Some((ticks.checked_add(end - start)?, overlap))
}

pub(super) fn micros(ticks: u64, period_nanos: f32) -> Option<u64> {
    if !period_nanos.is_finite() || period_nanos <= 0.0 {
        return None;
    }
    let micros = ticks as f64 * f64::from(period_nanos) / 1000.0;
    (micros.is_finite() && micros < u64::MAX as f64).then_some(micros as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicated_nested_and_partial_intervals_count_once_without_filling_gaps() {
        assert_eq!(union(&mut [(10, 30), (10, 30), (15, 20)]), Some((20, true)));
        assert_eq!(union(&mut [(20, 40), (10, 30), (50, 60)]), Some((40, true)));
        assert_eq!(union(&mut [(30, 40), (10, 20)]), Some((20, false)));
        assert_eq!(union(&mut [(10, 20), (20, 30)]), Some((20, false)));
        assert_eq!(union(&mut [(10, 20), (15, 15)]), Some((10, false)));
    }

    #[test]
    fn missing_reversed_wrapped_and_invalid_periods_are_unavailable() {
        assert_eq!(union(&mut []), None);
        assert_eq!(union(&mut [(30, 10)]), None);
        assert_eq!(union(&mut [(u64::MAX - 5, 3)]), None);
        for period in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert_eq!(micros(1000, period), None);
        }
        assert_eq!(micros(u64::MAX - 1, f32::MAX), None);
        assert_eq!(
            union(&mut [(u64::MAX - 50, u64::MAX - 10)]),
            Some((40, false))
        );
        assert_eq!(micros(40, 1000.0), Some(40));
    }
}
