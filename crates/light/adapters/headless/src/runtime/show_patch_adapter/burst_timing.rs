//! Private numeric-only telemetry for the isolated manual-clock sync bench.
//! Production adapters never enable it; no transport response or persisted data changes.

use parking_lot::Mutex;
use std::{collections::BTreeMap, sync::OnceLock, time::Duration};

#[derive(Default)]
struct Phase {
    samples: u64,
    total_micros: u128,
    maximum_micros: u128,
}

#[derive(Default)]
struct Aggregate {
    transactions: u64,
    phases: BTreeMap<&'static str, Phase>,
}

impl Aggregate {
    fn record(&mut self, name: &'static str, elapsed: Duration) -> Option<serde_json::Value> {
        let phase = self.phases.entry(name).or_default();
        phase.samples += 1;
        phase.total_micros += elapsed.as_micros();
        phase.maximum_micros = phase.maximum_micros.max(elapsed.as_micros());
        if name != "total" {
            return None;
        }
        self.transactions += 1;
        if !self.transactions.is_multiple_of(100) {
            return None;
        }
        Some(serde_json::json!({
            "transactions": self.transactions,
            "phases": self.phases.iter().map(|(name, phase)| (*name, serde_json::json!({
                "samples": phase.samples,
                "total_micros": phase.total_micros,
                "maximum_micros": phase.maximum_micros,
            }))).collect::<BTreeMap<_, _>>()
        }))
    }
}

pub(super) fn record(phase: &'static str, elapsed: Duration) {
    static METRICS: OnceLock<Mutex<Aggregate>> = OnceLock::new();
    let summary = METRICS
        .get_or_init(Mutex::default)
        .lock()
        .record(phase, elapsed);
    if let Some(summary) = summary {
        tracing::info!(numeric_phases = %summary, "isolated sync transaction timings");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summaries_keep_stage_samples_and_total_distinct() {
        let mut metrics = Aggregate::default();
        for _ in 0..99 {
            assert!(metrics.record("commit", Duration::from_micros(7)).is_none());
            assert!(metrics.record("total", Duration::from_micros(11)).is_none());
        }
        assert!(
            metrics
                .record("commit", Duration::from_micros(17))
                .is_none()
        );
        let summary = metrics.record("total", Duration::from_micros(23)).unwrap();
        assert_eq!(summary["transactions"], 100);
        assert_eq!(summary["phases"]["commit"]["samples"], 100);
        assert_eq!(summary["phases"]["commit"]["total_micros"], 710);
        assert_eq!(summary["phases"]["commit"]["maximum_micros"], 17);
        assert_eq!(summary["phases"]["total"]["total_micros"], 1112);
        assert_eq!(summary["phases"]["total"]["maximum_micros"], 23);
    }
}
