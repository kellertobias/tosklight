//! A controlled measurement condition; ordinary operator scheduling is unchanged.
use std::time::{Duration, Instant};

pub(super) struct BenchmarkPacing {
    interval: Option<Duration>,
    deadline: Option<Instant>,
}

impl BenchmarkPacing {
    pub fn from_environment(benchmark: bool) -> Result<Self, String> {
        let value = std::env::var("LIGHT_STAGE_PACKAGED_BENCH_HELPER_HZ").ok();
        Self::new(benchmark, value.as_deref())
    }

    fn new(benchmark: bool, value: Option<&str>) -> Result<Self, String> {
        let interval = match (benchmark, value) {
            (true, Some("60")) => Some(Duration::from_secs_f64(1.0 / 60.0)),
            (true, Some(_)) => {
                return Err("LIGHT_STAGE_PACKAGED_BENCH_HELPER_HZ must be 60 or absent".to_owned());
            }
            _ => None,
        };
        Ok(Self {
            interval,
            deadline: None,
        })
    }

    pub fn sleep_for(&mut self, now: Instant) -> Option<Duration> {
        let interval = self.interval?;
        let deadline = self.deadline.unwrap_or(now) + interval;
        // Missed slots are discarded; no burst can catch up and inflate matched-rate evidence.
        self.deadline = Some(if deadline <= now { now } else { deadline });
        Some(deadline.saturating_duration_since(now))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_explicit_benchmark_condition_changes_scheduling() {
        let now = Instant::now();
        assert_eq!(
            BenchmarkPacing::new(false, Some("60"))
                .unwrap()
                .sleep_for(now),
            None
        );
        assert_eq!(
            BenchmarkPacing::new(true, None).unwrap().sleep_for(now),
            None
        );
        assert!(BenchmarkPacing::new(true, Some("120")).is_err());
    }

    #[test]
    fn keeps_deadlines_and_discards_missed_slots_without_catchup_bursts() {
        let mut pacing = BenchmarkPacing::new(true, Some("60")).unwrap();
        let now = Instant::now();
        let interval = Duration::from_secs_f64(1.0 / 60.0);
        assert_eq!(pacing.sleep_for(now), Some(interval));
        assert_eq!(
            pacing.sleep_for(now + interval + Duration::from_millis(2)),
            Some(interval - Duration::from_millis(2))
        );
        let late = now + Duration::from_secs(1);
        assert_eq!(pacing.sleep_for(late), Some(Duration::ZERO));
        assert_eq!(pacing.sleep_for(late), Some(interval));
    }
}
