use chrono::{DateTime, Utc};
use std::{
    fmt::Debug,
    sync::{Arc, RwLock},
    time::Instant,
};

/// Monotonic clock used by render loops. Wall clock is intentionally not used for frame deadlines.
#[derive(Clone, Debug)]
pub struct EngineClock {
    started: Instant,
}

/// Application time used for lighting behavior. Scheduler deadlines deliberately use `Instant`
/// so a manually controlled test clock cannot affect real-time I/O health.
pub trait ApplicationClock: Debug + Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}

pub type SharedClock = Arc<dyn ApplicationClock>;

#[derive(Debug, Default)]
pub struct SystemClock;

impl ApplicationClock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

#[derive(Debug)]
pub struct ManualClock {
    now: RwLock<DateTime<Utc>>,
}

impl ManualClock {
    pub fn new(now: DateTime<Utc>) -> Self {
        Self {
            now: RwLock::new(now),
        }
    }

    pub fn set(&self, now: DateTime<Utc>) {
        *self.now.write().expect("manual clock lock poisoned") = now;
    }

    pub fn advance_millis(&self, millis: i64) -> DateTime<Utc> {
        let mut now = self.now.write().expect("manual clock lock poisoned");
        *now += chrono::Duration::milliseconds(millis);
        *now
    }
}

impl ApplicationClock for ManualClock {
    fn now(&self) -> DateTime<Utc> {
        *self.now.read().expect("manual clock lock poisoned")
    }
}

impl Default for EngineClock {
    fn default() -> Self {
        Self {
            started: Instant::now(),
        }
    }
}

impl EngineClock {
    pub fn elapsed_micros(&self) -> u64 {
        self.started.elapsed().as_micros() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_application_time_only_moves_when_advanced() {
        let start = DateTime::parse_from_rfc3339("2020-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let clock = ManualClock::new(start);
        assert_eq!(clock.now(), start);
        assert_eq!(clock.now(), start);
        assert_eq!(
            clock.advance_millis(30_000),
            start + chrono::Duration::seconds(30)
        );
    }
}
