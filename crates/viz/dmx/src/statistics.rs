//! Rolling per-universe input health.
//!
//! The operator's question is "is this universe healthy right now?", and the honest answer needs
//! history: a single dropped frame thirty seconds ago is worth a warning, a second of heavy loss
//! is worth an alarm, and a healthy stream should read as healthy. One-second buckets over a
//! thirty-second window answer all three without keeping per-packet history.

use std::collections::VecDeque;

/// How far back the health grade looks.
pub const WINDOW_SECONDS: u64 = 30;
/// Below this rate a universe is unusable for live work.
pub const CRITICAL_HZ: f32 = 20.0;
/// Below this rate a universe is degraded but still usable.
pub const DEGRADED_HZ: f32 = 30.0;
/// Share of broken frames within one second that counts as an alarm.
pub const CRITICAL_BROKEN_SHARE: f32 = 0.20;

use viz_scene::UniverseGrade;

#[derive(Clone, Copy, Debug, Default)]
struct Bucket {
    second: u64,
    accepted: u32,
    broken: u32,
}

/// One second of counts, kept for the window.
#[derive(Debug, Default)]
pub struct UniverseStatistics {
    buckets: VecDeque<Bucket>,
    total_accepted: u64,
    total_broken: u64,
}

impl UniverseStatistics {
    /// Record one accepted frame at `micros` since the shared epoch.
    pub fn accept(&mut self, micros: u64) {
        self.total_accepted += 1;
        self.bucket(micros).accepted += 1;
    }

    /// Record one frame that could not be used: malformed, out of order, or refused.
    pub fn broken(&mut self, micros: u64) {
        self.total_broken += 1;
        self.bucket(micros).broken += 1;
    }

    /// The bucket for `micros`, creating it when this is a new second.
    ///
    /// A frame is normally counted into the newest second, but a receiver thread can hand over a
    /// timestamp a moment behind the newest one, so an existing bucket is reused rather than a
    /// second one being appended out of order.
    fn bucket(&mut self, micros: u64) -> &mut Bucket {
        let second = micros / 1_000_000;
        let newest = self.buckets.back().map(|bucket| bucket.second);
        if newest.is_none_or(|newest| second > newest) {
            self.buckets.push_back(Bucket {
                second,
                accepted: 0,
                broken: 0,
            });
            self.trim(second);
        }
        let index = self
            .buckets
            .iter()
            .rposition(|bucket| bucket.second == second)
            // Older than everything still in the window: fold it into the oldest bucket kept.
            .unwrap_or(0);
        self.buckets
            .get_mut(index)
            .expect("the window always holds at least one bucket")
    }

    fn trim(&mut self, now_second: u64) {
        while let Some(front) = self.buckets.front() {
            if now_second.saturating_sub(front.second) >= WINDOW_SECONDS {
                self.buckets.pop_front();
            } else {
                break;
            }
        }
    }

    /// Frames per second over the last few complete seconds.
    ///
    /// The current second is excluded because it is still filling and would read low.
    pub fn rate_hz(&self, now_micros: u64) -> f32 {
        let now_second = now_micros / 1_000_000;
        let recent: Vec<&Bucket> = self
            .buckets
            .iter()
            .filter(|bucket| {
                let age = now_second.saturating_sub(bucket.second);
                (1..=3).contains(&age)
            })
            .collect();
        if recent.is_empty() {
            // Nothing complete yet: fall back to the current second so a fresh stream still reads.
            return self
                .buckets
                .back()
                .filter(|bucket| bucket.second == now_second)
                .map(|bucket| bucket.accepted as f32)
                .unwrap_or(0.0);
        }
        let total: u32 = recent.iter().map(|bucket| bucket.accepted).sum();
        total as f32 / recent.len() as f32
    }

    /// Whether any second inside the window lost more than the critical share.
    pub fn had_heavy_loss(&self) -> bool {
        self.buckets.iter().any(|bucket| {
            let total = bucket.accepted + bucket.broken;
            total > 0 && bucket.broken as f32 / total as f32 > CRITICAL_BROKEN_SHARE
        })
    }

    /// Whether any frame inside the window was broken at all.
    pub fn had_any_loss(&self) -> bool {
        self.buckets.iter().any(|bucket| bucket.broken > 0)
    }

    pub fn total_accepted(&self) -> u64 {
        self.total_accepted
    }

    pub fn total_broken(&self) -> u64 {
        self.total_broken
    }

    /// Grade this universe.
    ///
    /// `stale` means the source-loss timeout has expired, which is always critical.
    pub fn grade(&self, now_micros: u64, stale: bool) -> UniverseGrade {
        if self.total_accepted == 0 {
            return UniverseGrade::Waiting;
        }
        let rate = self.rate_hz(now_micros);
        if stale || rate < CRITICAL_HZ || self.had_heavy_loss() {
            return UniverseGrade::Critical;
        }
        if rate < DEGRADED_HZ || self.had_any_loss() {
            return UniverseGrade::Degraded;
        }
        UniverseGrade::Healthy
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seconds(value: u64) -> u64 {
        value * 1_000_000
    }

    /// Fill `count` seconds at `hz`, starting at second `from`.
    fn stream(statistics: &mut UniverseStatistics, from: u64, count: u64, hz: u32) {
        for second in from..from + count {
            for frame in 0..hz {
                statistics.accept(seconds(second) + u64::from(frame) * 1_000);
            }
        }
    }

    #[test]
    fn a_universe_with_nothing_yet_is_waiting() {
        let statistics = UniverseStatistics::default();
        assert_eq!(statistics.grade(seconds(1), false), UniverseGrade::Waiting);
    }

    #[test]
    fn a_clean_forty_hertz_stream_is_healthy() {
        let mut statistics = UniverseStatistics::default();
        stream(&mut statistics, 0, 6, 40);
        let now = seconds(6);
        assert!((statistics.rate_hz(now) - 40.0).abs() < 0.001);
        assert_eq!(statistics.grade(now, false), UniverseGrade::Healthy);
    }

    #[test]
    fn one_broken_frame_inside_the_window_degrades_but_does_not_alarm() {
        let mut statistics = UniverseStatistics::default();
        stream(&mut statistics, 0, 6, 44);
        statistics.broken(seconds(3));
        assert_eq!(
            statistics.grade(seconds(6), false),
            UniverseGrade::Degraded,
            "a singular broken frame is a warning, not an alarm"
        );
    }

    #[test]
    fn one_second_of_heavy_loss_alarms_for_the_whole_window() {
        let mut statistics = UniverseStatistics::default();
        stream(&mut statistics, 0, 1, 40);
        // Second 1: 30 accepted, 20 broken — 40 per cent lost.
        for frame in 0..30 {
            statistics.accept(seconds(1) + frame * 1_000);
        }
        for frame in 0..20 {
            statistics.broken(seconds(1) + frame * 1_000);
        }
        stream(&mut statistics, 2, 5, 44);
        assert_eq!(statistics.grade(seconds(6), false), UniverseGrade::Critical);
        // Once the bad second falls out of the window the universe recovers on its own.
        stream(&mut statistics, 7, WINDOW_SECONDS + 2, 44);
        assert_eq!(
            statistics.grade(seconds(WINDOW_SECONDS + 9), false),
            UniverseGrade::Healthy
        );
    }

    #[test]
    fn a_slow_stream_alarms_below_twenty_hertz() {
        let mut statistics = UniverseStatistics::default();
        stream(&mut statistics, 0, 6, 15);
        assert_eq!(statistics.grade(seconds(6), false), UniverseGrade::Critical);
    }

    #[test]
    fn a_rate_drop_short_of_the_alarm_only_degrades() {
        let mut statistics = UniverseStatistics::default();
        stream(&mut statistics, 0, 6, 25);
        assert_eq!(statistics.grade(seconds(6), false), UniverseGrade::Degraded);
    }

    #[test]
    fn a_stale_source_is_always_critical() {
        let mut statistics = UniverseStatistics::default();
        stream(&mut statistics, 0, 6, 44);
        assert_eq!(statistics.grade(seconds(6), true), UniverseGrade::Critical);
    }

    #[test]
    fn the_window_never_grows_without_bound() {
        let mut statistics = UniverseStatistics::default();
        stream(&mut statistics, 0, 300, 44);
        assert!(statistics.buckets.len() <= WINDOW_SECONDS as usize);
        assert_eq!(statistics.total_accepted(), 300 * 44);
    }
}
