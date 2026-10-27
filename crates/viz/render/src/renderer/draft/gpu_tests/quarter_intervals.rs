//! Timestamp interval geometry, not GPU-active time or utilization; one device/queue sample.
use crate::timing::RawTimestampSnapshot;

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct IntervalEvidence {
    pub sample_id: u64,
    pub pass_mask: u8,
    pub timestamp_period_nanos: f32,
    pub raw_pass_pairs: [Option<[u64; 2]>; 8],
    pub individual_pass_micros: [Option<f64>; 8],
    pub pass_duration_sum_micros: f64,
    pub pass_interval_union_micros: f64,
    pub pass_interval_span_micros: f64,
}
pub(super) fn intervals(raw: RawTimestampSnapshot) -> Result<IntervalEvidence, &'static str> {
    if !raw.period_nanos.is_finite() || raw.period_nanos <= 0.0 || raw.mask == 0 {
        return Err("missing mask or invalid timestamp period");
    }
    let mut result = IntervalEvidence {
        sample_id: raw.sample_id,
        pass_mask: raw.mask,
        timestamp_period_nanos: raw.period_nanos,
        raw_pass_pairs: [None; 8],
        individual_pass_micros: [None; 8],
        pass_duration_sum_micros: 0.0,
        pass_interval_union_micros: 0.0,
        pass_interval_span_micros: 0.0,
    };
    let micros = |ticks: u64| ticks as f64 * f64::from(raw.period_nanos) / 1000.0;
    let mut pairs = Vec::new();
    for index in 0..8 {
        if raw.mask & (1 << index) == 0 {
            continue;
        }
        let start = u64::from_le_bytes(raw.bytes[index * 16..index * 16 + 8].try_into().unwrap());
        let end = u64::from_le_bytes(
            raw.bytes[index * 16 + 8..index * 16 + 16]
                .try_into()
                .unwrap(),
        );
        if start == u64::MAX || end == u64::MAX || end <= start {
            return Err("sentinel, reversed or zero timestamp interval");
        }
        result.raw_pass_pairs[index] = Some([start, end]);
        result.individual_pass_micros[index] = Some(micros(end - start));
        result.pass_duration_sum_micros += micros(end - start);
        pairs.push((start, end));
    }
    pairs.sort_unstable();
    let mut active = pairs[0];
    let mut union = 0_u64;
    let first = active.0;
    for &(start, end) in &pairs[1..] {
        if start <= active.1 {
            active.1 = active.1.max(end);
        } else {
            union += active.1 - active.0;
            active = (start, end);
        }
    }
    union += active.1 - active.0;
    result.pass_interval_union_micros = micros(union);
    result.pass_interval_span_micros = micros(active.1 - first);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn raw(pairs: &[(u64, u64)]) -> RawTimestampSnapshot {
        let mut result = RawTimestampSnapshot {
            sample_id: 17,
            mask: 0,
            period_nanos: 1000.0,
            bytes: [0; 128],
        };
        for (index, &(start, end)) in pairs.iter().enumerate() {
            result.mask |= 1 << index;
            result.bytes[index * 16..index * 16 + 8].copy_from_slice(&start.to_le_bytes());
            result.bytes[index * 16 + 8..index * 16 + 16].copy_from_slice(&end.to_le_bytes());
        }
        result
    }
    #[test]
    fn union_deduplicates_nested_overlapping_and_touching_passes_without_summing_them() {
        let result = intervals(raw(&[(100, 200), (110, 150), (190, 250), (250, 275)])).unwrap();
        assert_eq!(result.sample_id, 17);
        assert_eq!(result.pass_duration_sum_micros, 225.0);
        assert_eq!(result.pass_interval_union_micros, 175.0);
        assert_eq!(result.pass_interval_span_micros, 175.0);
        assert_eq!(result.individual_pass_micros[1], Some(40.0));
    }
    #[test]
    fn union_excludes_gaps_while_span_includes_them_and_period_scales_both() {
        let mut raw = raw(&[(300, 350), (100, 200)]);
        raw.period_nanos = 2.0;
        let result = intervals(raw).unwrap();
        assert_eq!(result.pass_interval_union_micros, 0.3);
        assert_eq!(result.pass_interval_span_micros, 0.5);
    }
    #[test]
    fn empty_zero_reversed_sentinel_and_invalid_period_are_unavailable() {
        for pairs in [vec![], vec![(10, 10)], vec![(20, 10)], vec![(10, u64::MAX)]] {
            assert!(intervals(raw(&pairs)).is_err());
        }
        for period in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            let mut raw = raw(&[(10, 20)]);
            raw.period_nanos = period;
            assert!(intervals(raw).is_err());
        }
    }
}
