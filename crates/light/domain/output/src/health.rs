//! Scheduler and delivery health reported to operators.

use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

/// Inclusive upper bounds for output scheduler tick-duration histogram buckets.
///
/// The final JavaScript-safe bound is the overflow bucket for any practical runtime tick.
pub const OUTPUT_TICK_DURATION_BUCKET_BOUNDS_MICROS: [u64; 20] = [
    250,
    500,
    750,
    1_000,
    1_250,
    1_500,
    2_000,
    3_000,
    4_000,
    6_000,
    8_000,
    12_000,
    16_000,
    24_000,
    32_000,
    48_000,
    64_000,
    100_000,
    1_000_000,
    9_007_199_254_740_991,
];

/// Exclusive frame-rate bounds an operator watches for dropped output cadence.
///
/// Each bucket counts the frames delivered slower than its bound, so the counts are cumulative
/// and the last bucket contains every frame that missed the slowest supported desk rate.
pub const OUTPUT_FRAME_RATE_BUCKET_BOUNDS_HZ: [f32; 5] = [20.0, 30.0, 38.0, 40.0, 44.0];

/// Span of the rolling window behind every `recent_*` reading.
pub const OUTPUT_RECENT_WINDOW: Duration = Duration::from_secs(60);

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct OutputHealth {
    pub frames_sent: u64,
    pub packets_sent: u64,
    pub send_errors: u64,
    pub deadline_misses: u64,
    pub maximum_lateness_micros: u64,
    /// Nominal output rate the scheduler is configured to run at.
    pub frame_hz: f32,
    pub last_tick_micros: u64,
    pub maximum_tick_micros: u64,
    pub tick_duration_bucket_counts: [u64; OUTPUT_TICK_DURATION_BUCKET_BOUNDS_MICROS.len()],
    pub scheduler_utilization: f32,
    /// Slowest measured frame rate over the last [`OUTPUT_RECENT_WINDOW`].
    pub recent_frame_hz_minimum: f32,
    /// Fastest measured frame rate over the last [`OUTPUT_RECENT_WINDOW`].
    pub recent_frame_hz_maximum: f32,
    /// Mean measured frame rate over the last [`OUTPUT_RECENT_WINDOW`].
    pub recent_frame_hz_average: f32,
    /// Frames in the last [`OUTPUT_RECENT_WINDOW`] delivered below each
    /// [`OUTPUT_FRAME_RATE_BUCKET_BOUNDS_HZ`] bound.
    pub recent_frame_rate_bucket_counts: [u64; OUTPUT_FRAME_RATE_BUCKET_BOUNDS_HZ.len()],
    /// Send errors observed in the last [`OUTPUT_RECENT_WINDOW`].
    pub recent_send_errors: u64,
    #[serde(skip)]
    window: RecentOutputWindow,
}

impl OutputHealth {
    /// Records one delivered frame.
    ///
    /// Recording stays O(1) amortized because it runs on the output thread for every frame. The
    /// readings themselves are derived in [`Self::refresh_recent`], which the far rarer snapshot
    /// path calls.
    pub fn record_frame(&mut self, at: Instant) {
        if let Some(previous) = self.window.last_frame_at {
            let elapsed = at.saturating_duration_since(previous).as_secs_f64();
            if elapsed > 0.0 {
                self.window.frames.push_back((at, (1.0 / elapsed) as f32));
            }
        }
        self.window.last_frame_at = Some(at);
        self.window.discard_before(at);
    }

    /// Records one failed send.
    pub fn record_send_error(&mut self, at: Instant) {
        self.window.send_errors.push_back(at);
        self.window.discard_before(at);
    }

    /// Derives every rolling reading from the frames and errors still inside the window.
    ///
    /// A desk that stopped producing frames must not keep reporting the last healthy window, so
    /// this expires stale samples against `now` rather than against the last recorded sample.
    pub fn refresh_recent(&mut self, now: Instant) {
        self.window.discard_before(now);
        self.recent_send_errors = self.window.send_errors.len() as u64;
        self.recent_frame_rate_bucket_counts = [0; OUTPUT_FRAME_RATE_BUCKET_BOUNDS_HZ.len()];
        if self.window.frames.is_empty() {
            self.recent_frame_hz_minimum = 0.0;
            self.recent_frame_hz_maximum = 0.0;
            self.recent_frame_hz_average = 0.0;
            return;
        }
        let mut minimum = f32::MAX;
        let mut maximum = 0.0_f32;
        let mut total = 0.0_f64;
        for (_, frame_hz) in &self.window.frames {
            minimum = minimum.min(*frame_hz);
            maximum = maximum.max(*frame_hz);
            total += f64::from(*frame_hz);
            for (bucket, bound) in OUTPUT_FRAME_RATE_BUCKET_BOUNDS_HZ.iter().enumerate() {
                if *frame_hz < *bound {
                    self.recent_frame_rate_bucket_counts[bucket] += 1;
                }
            }
        }
        self.recent_frame_hz_minimum = minimum;
        self.recent_frame_hz_maximum = maximum;
        self.recent_frame_hz_average = (total / self.window.frames.len() as f64) as f32;
    }

    /// Clears every counter that an operator reads as "since show start".
    ///
    /// The configured rate survives because it describes the desk, not the loaded show.
    pub fn reset_for_new_show(&mut self) {
        let frame_hz = self.frame_hz;
        *self = Self {
            frame_hz,
            ..Self::default()
        };
    }
}

#[derive(Clone, Debug, Default)]
struct RecentOutputWindow {
    frames: VecDeque<(Instant, f32)>,
    send_errors: VecDeque<Instant>,
    last_frame_at: Option<Instant>,
}

impl RecentOutputWindow {
    /// Drops every sample older than [`OUTPUT_RECENT_WINDOW`] before `now`.
    fn discard_before(&mut self, now: Instant) {
        let Some(horizon) = now.checked_sub(OUTPUT_RECENT_WINDOW) else {
            return;
        };
        while self.frames.front().is_some_and(|(at, _)| *at < horizon) {
            self.frames.pop_front();
        }
        while self.send_errors.front().is_some_and(|at| *at < horizon) {
            self.send_errors.pop_front();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn origin() -> Instant {
        Instant::now() - Duration::from_secs(3_600)
    }

    #[test]
    fn measured_frame_rate_comes_from_the_interval_between_delivered_frames() {
        let mut health = OutputHealth::default();
        let start = origin();
        health.record_frame(start);
        health.record_frame(start + Duration::from_millis(25));
        health.record_frame(start + Duration::from_millis(75));
        health.refresh_recent(start + Duration::from_millis(75));

        assert_eq!(health.recent_frame_hz_maximum.round(), 40.0);
        assert_eq!(health.recent_frame_hz_minimum.round(), 20.0);
        assert_eq!(health.recent_frame_hz_average.round(), 30.0);
    }

    #[test]
    fn slow_frames_fall_into_every_bucket_they_are_below() {
        let mut health = OutputHealth::default();
        let start = origin();
        health.record_frame(start);
        // 25 Hz: below 30, 38, 40 and 44, but not below 20.
        health.record_frame(start + Duration::from_millis(40));
        health.refresh_recent(start + Duration::from_millis(40));

        assert_eq!(health.recent_frame_rate_bucket_counts, [0, 1, 1, 1, 1]);
    }

    #[test]
    fn readings_outside_the_window_stop_counting() {
        let mut health = OutputHealth::default();
        let start = origin();
        health.record_frame(start);
        health.record_frame(start + Duration::from_millis(100));
        health.record_send_error(start + Duration::from_millis(100));
        health.refresh_recent(start + Duration::from_millis(100));
        assert_eq!(health.recent_send_errors, 1);

        health.refresh_recent(start + OUTPUT_RECENT_WINDOW + Duration::from_secs(1));

        assert_eq!(health.recent_send_errors, 0);
        assert_eq!(health.recent_frame_hz_average, 0.0);
        assert_eq!(health.recent_frame_rate_bucket_counts, [0; 5]);
    }

    #[test]
    fn opening_a_show_restarts_the_totals_but_keeps_the_configured_rate() {
        let mut health = OutputHealth {
            frame_hz: 44.0,
            send_errors: 7,
            frames_sent: 900,
            ..OutputHealth::default()
        };
        health.record_send_error(origin());

        health.reset_for_new_show();

        assert_eq!(health.frame_hz, 44.0);
        assert_eq!(health.send_errors, 0);
        assert_eq!(health.frames_sent, 0);
        assert_eq!(health.recent_send_errors, 0);
    }
}
