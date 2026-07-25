use serde::Serialize;
use std::time::Duration;

#[derive(Clone, Debug, Serialize)]
pub struct Distribution {
    pub samples: usize,
    pub minimum_microseconds: f64,
    pub p50_microseconds: f64,
    pub p95_microseconds: f64,
    pub p99_microseconds: f64,
    pub maximum_microseconds: f64,
    pub mean_microseconds: f64,
}

pub fn distribution(samples: &[Duration]) -> Option<Distribution> {
    if samples.is_empty() {
        return None;
    }
    let mut nanos = samples
        .iter()
        .map(|sample| sample.as_nanos().min(u128::from(u64::MAX)) as u64)
        .collect::<Vec<_>>();
    nanos.sort_unstable();
    let total = nanos.iter().map(|value| *value as u128).sum::<u128>();
    Some(Distribution {
        samples: nanos.len(),
        minimum_microseconds: micros(nanos[0]),
        p50_microseconds: micros(percentile(&nanos, 50)),
        p95_microseconds: micros(percentile(&nanos, 95)),
        p99_microseconds: micros(percentile(&nanos, 99)),
        maximum_microseconds: micros(*nanos.last().expect("samples are non-empty")),
        mean_microseconds: total as f64 / nanos.len() as f64 / 1_000.0,
    })
}

fn percentile(sorted: &[u64], percentile: usize) -> u64 {
    let rank = (percentile * sorted.len()).div_ceil(100);
    sorted[rank.saturating_sub(1).min(sorted.len() - 1)]
}

fn micros(nanos: u64) -> f64 {
    nanos as f64 / 1_000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distribution_uses_nearest_rank_percentiles() {
        let samples = (1..=100).map(Duration::from_micros).collect::<Vec<_>>();
        let result = distribution(&samples).unwrap();
        assert_eq!(result.minimum_microseconds, 1.0);
        assert_eq!(result.p50_microseconds, 50.0);
        assert_eq!(result.p95_microseconds, 95.0);
        assert_eq!(result.p99_microseconds, 99.0);
        assert_eq!(result.maximum_microseconds, 100.0);
        assert_eq!(result.mean_microseconds, 50.5);
    }

    #[test]
    fn empty_distribution_is_explicitly_absent() {
        assert!(distribution(&[]).is_none());
    }
}
