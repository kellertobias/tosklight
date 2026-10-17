//! Counter sampling clocks belong to the exact health clone, never the enclosing HTTP request.
use super::*;
use light_wire::v2::runtime::RuntimeOutputCounterSample;
use std::{
    sync::LazyLock,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

static ORIGIN: LazyLock<Instant> = LazyLock::new(Instant::now);

impl OutputResource {
    /// The returned counters and both clock observations share one health critical section.
    pub(in crate::runtime) fn health_sample(
        &self,
    ) -> Result<(OutputHealth, RuntimeOutputCounterSample), &'static str> {
        let origin = *ORIGIN;
        sample(&self.health, || {
            let monotonic = Instant::now();
            let epoch = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| "output sample clock predates Unix epoch")?;
            let sample = RuntimeOutputCounterSample {
                sampled_at_epoch_micros: u64::try_from(epoch.as_micros())
                    .map_err(|_| "output sample clock exceeds supported range")?,
                monotonic_micros: u64::try_from(
                    monotonic.saturating_duration_since(origin).as_micros(),
                )
                .map_err(|_| "output monotonic sample exceeds supported range")?,
            };
            Ok((monotonic, sample))
        })
    }
}

fn sample(
    health: &std::sync::Mutex<OutputHealth>,
    clock: impl FnOnce() -> Result<(Instant, RuntimeOutputCounterSample), &'static str>,
) -> Result<(OutputHealth, RuntimeOutputCounterSample), &'static str> {
    let mut health = health.lock().expect("output health mutex poisoned");
    let (now, sampled) = clock()?;
    health.refresh_recent(now);
    Ok((health.clone(), sampled))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clock_is_read_under_the_counter_lock_and_the_returned_clone_is_stable() {
        let mut initial = OutputHealth::default();
        initial.frames_sent = 71;
        initial.deadline_misses = 3;
        let health = std::sync::Mutex::new(initial);
        let stamp = RuntimeOutputCounterSample {
            sampled_at_epoch_micros: 123_456,
            monotonic_micros: 678,
        };
        let (captured, actual) = sample(&health, || {
            assert!(
                health.try_lock().is_err(),
                "a concurrent output update cannot split the clock and counters"
            );
            Ok((Instant::now(), stamp.clone()))
        })
        .unwrap();
        health.lock().unwrap().frames_sent += 1;
        assert_eq!(captured.frames_sent, 71);
        assert_eq!(captured.deadline_misses, 3);
        assert_eq!(actual, stamp);
    }
    #[test]
    fn failed_clock_read_never_returns_an_untimestamped_counter_clone() {
        let health = std::sync::Mutex::new(OutputHealth::default());
        assert_eq!(
            sample(&health, || Err("clock unavailable")).unwrap_err(),
            "clock unavailable"
        );
        assert!(health.try_lock().is_ok());
    }
}
