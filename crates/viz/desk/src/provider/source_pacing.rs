//! A minimum source-read START gap, independent of configuration events and render deadlines.
use std::time::{Duration, Instant};
const SOURCE_PERIOD: Duration = Duration::from_millis(100);

pub(super) struct SourcePacing {
    next_start: Instant,
}
impl SourcePacing {
    pub(super) fn new(now: Instant) -> Self {
        Self { next_start: now }
    }
    pub(super) fn deadline(&self) -> Instant {
        self.next_start
    }
    pub(super) fn completed(&mut self, actual_start: Instant) {
        // A single in-flight future owns each attempt. Failures consume the same minimum gap;
        // long requests allow one next attempt immediately, never a burst of missed slots.
        self.next_start = actual_start + SOURCE_PERIOD;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn initial_read_and_event_flood_keep_exact_minimum_start_gap() {
        let origin = Instant::now();
        let mut clock = SourcePacing::new(origin);
        let mut starts = Vec::new();
        for event in 0..1000 {
            let now = origin + Duration::from_millis(event);
            // Configuration traffic is processed at every step; only source reads are gated.
            if now >= clock.deadline() {
                starts.push(event);
                clock.completed(now);
            }
        }
        assert_eq!(starts, (0..10).map(|n| n * 100).collect::<Vec<_>>());
    }
    #[test]
    fn failed_and_slow_reads_have_no_completion_delay_or_catch_up_bursts() {
        let origin = Instant::now();
        let mut clock = SourcePacing::new(origin);
        clock.completed(origin); // Failed immediately: retry still waits until +100ms.
        assert_eq!(clock.deadline(), origin + SOURCE_PERIOD);
        let slow_start = origin + Duration::from_millis(100);
        clock.completed(slow_start); // Completes at +1500ms in the host loop.
        let next_actual_start = origin + Duration::from_millis(1500);
        assert!(clock.deadline() <= next_actual_start);
        clock.completed(next_actual_start);
        assert_eq!(clock.deadline(), origin + Duration::from_millis(1600));
    }
    #[test]
    fn first_poll_time_not_future_creation_drives_the_next_deadline() {
        let created = Instant::now();
        let mut clock = SourcePacing::new(created);
        let actual_start = created + Duration::from_millis(450);
        clock.completed(actual_start);
        assert_eq!(clock.deadline(), created + Duration::from_millis(550));
    }
    #[test]
    fn reconnect_after_backoff_can_read_immediately_without_old_catch_up() {
        let origin = Instant::now();
        let mut first = SourcePacing::new(origin);
        first.completed(origin);
        let reconnected = origin + Duration::from_secs(2);
        let mut next = SourcePacing::new(reconnected);
        assert_eq!(next.deadline(), reconnected);
        next.completed(reconnected);
        assert_eq!(next.deadline(), reconnected + SOURCE_PERIOD);
    }
}
