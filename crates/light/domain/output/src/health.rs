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

/// Frame-rate thresholds an operator watches for dropped output cadence.
///
/// The bounds split the rate axis into disjoint bands, so a delivered frame is counted exactly
/// once and the bucket counts sum to the frames in the window. There is one more bucket than
/// there are bounds: the first holds every frame slower than the lowest bound, bucket `n + 1`
/// holds every frame from bound `n` up to but not including bound `n + 1`, and the last holds
/// every frame at or above the highest bound.
pub const OUTPUT_FRAME_RATE_BUCKET_BOUNDS_HZ: [f32; 9] =
    [20.0, 30.0, 38.0, 40.0, 44.0, 48.0, 52.0, 56.0, 60.0];

/// Buckets reported for [`OUTPUT_FRAME_RATE_BUCKET_BOUNDS_HZ`]: the below-lowest band plus one
/// at-or-above band per bound.
pub const OUTPUT_FRAME_RATE_BUCKET_COUNT: usize = OUTPUT_FRAME_RATE_BUCKET_BOUNDS_HZ.len() + 1;

/// Span of the rolling window behind every `recent_*` reading.
pub const OUTPUT_RECENT_WINDOW: Duration = Duration::from_secs(60);

/// Inclusive upper bounds, in hertz, of the bands a delivered frame is counted into.
///
/// A frame's rate is the reciprocal of the time it took, so these bands are the frame-time budgets
/// an operator judges a desk by, read from the rate end: the first band holds every frame slower
/// than 10 Hz and the last holds every frame at or above the fastest band's floor. Counts run
/// from show start, so a measurement window is the difference between two readings.
pub const OUTPUT_FRAME_RATE_BAND_BOUNDS_HZ: [f32; 34] = [
    10.0, 15.0, 20.0, 25.0, 30.0, 35.0, 40.0, 43.0, 46.0, 49.0, 52.0, 55.0, 58.0, 61.0, 64.0, 67.0,
    70.0, 73.0, 76.0, 79.0, 82.0, 85.0, 88.0, 91.0, 94.0, 97.0, 100.0, 103.0, 106.0, 109.0, 112.0,
    115.0, 118.0, 120.0,
];

/// The single [`OUTPUT_FRAME_RATE_BUCKET_BOUNDS_HZ`] bucket a measured rate belongs to.
///
/// The bands are disjoint: a frame slower than the lowest bound lands in bucket zero, a frame at
/// or above the highest bound lands in the last bucket, and everything between lands in the one
/// band whose floor it reaches.
pub fn recent_frame_rate_bucket(frame_hz: f32) -> usize {
    OUTPUT_FRAME_RATE_BUCKET_BOUNDS_HZ
        .iter()
        .position(|bound| frame_hz < *bound)
        .unwrap_or(OUTPUT_FRAME_RATE_BUCKET_COUNT - 1)
}

/// The band a delivered frame's measured rate belongs to.
///
/// Anything slower than the first bound lands in the first band and anything at or above the last
/// bound lands in the last, so no frame is lost off either end of the histogram.
pub fn frame_rate_band(frame_hz: f32) -> usize {
    OUTPUT_FRAME_RATE_BAND_BOUNDS_HZ
        .iter()
        .position(|bound| frame_hz < *bound)
        .unwrap_or(OUTPUT_FRAME_RATE_BAND_BOUNDS_HZ.len() - 1)
}

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
    /// Frames in the last [`OUTPUT_RECENT_WINDOW`] by [`OUTPUT_FRAME_RATE_BUCKET_BOUNDS_HZ`]
    /// band: index zero is slower than the lowest bound, index `n + 1` reaches bound `n` without
    /// reaching bound `n + 1`, and the last is at or above the highest bound. The bands are
    /// disjoint, so every frame is counted once.
    pub recent_frame_rate_bucket_counts: [u64; OUTPUT_FRAME_RATE_BUCKET_COUNT],
    /// Send errors observed in the last [`OUTPUT_RECENT_WINDOW`].
    pub recent_send_errors: u64,
    /// Frames delivered in each [`OUTPUT_FRAME_RATE_BAND_BOUNDS_HZ`] band since show start.
    ///
    /// Empty until the first frame is measured, then one entry per band.
    pub frame_rate_band_counts: Vec<u64>,
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
                let frame_hz = (1.0 / elapsed) as f32;
                self.window.frames.push_back((at, frame_hz));
                if self.frame_rate_band_counts.is_empty() {
                    self.frame_rate_band_counts = vec![0; OUTPUT_FRAME_RATE_BAND_BOUNDS_HZ.len()];
                }
                self.frame_rate_band_counts[frame_rate_band(frame_hz)] += 1;
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
        self.recent_frame_rate_bucket_counts = [0; OUTPUT_FRAME_RATE_BUCKET_COUNT];
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
            self.recent_frame_rate_bucket_counts[recent_frame_rate_bucket(*frame_hz)] += 1;
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
    fn a_frame_counts_into_exactly_one_band() {
        let mut health = OutputHealth::default();
        let start = origin();
        health.record_frame(start);
        // 25 Hz: reaches 20 without reaching 30, so only that band counts it.
        health.record_frame(start + Duration::from_millis(40));
        health.refresh_recent(start + Duration::from_millis(40));

        assert_eq!(
            health.recent_frame_rate_bucket_counts,
            [0, 1, 0, 0, 0, 0, 0, 0, 0, 0]
        );

        // 62 Hz is above the highest bound, so only the overflow band counts it.
        let mut fast = OutputHealth::default();
        fast.record_frame(start);
        fast.record_frame(start + Duration::from_micros(16_129));
        fast.refresh_recent(start + Duration::from_micros(16_129));

        assert_eq!(
            fast.recent_frame_rate_bucket_counts,
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 1]
        );

        // 12 Hz reaches none of them, so only the below-lowest band counts it.
        let mut slow = OutputHealth::default();
        slow.record_frame(start);
        slow.record_frame(start + Duration::from_millis(83));
        slow.refresh_recent(start + Duration::from_millis(83));

        assert_eq!(
            slow.recent_frame_rate_bucket_counts,
            [1, 0, 0, 0, 0, 0, 0, 0, 0, 0]
        );
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
        assert_eq!(
            health.recent_frame_rate_bucket_counts,
            [0; OUTPUT_FRAME_RATE_BUCKET_COUNT]
        );
    }

    #[test]
    fn every_delivered_frame_lands_in_exactly_one_rate_band() {
        let mut health = OutputHealth::default();
        let start = origin();
        health.record_frame(start);
        // 40 Hz, 25 Hz and 2 Hz: one in the 40-43 band, one in the 25-30 band, one below 10.
        health.record_frame(start + Duration::from_millis(25));
        health.record_frame(start + Duration::from_millis(65));
        health.record_frame(start + Duration::from_millis(565));

        let counts = &health.frame_rate_band_counts;
        assert_eq!(counts.len(), OUTPUT_FRAME_RATE_BAND_BOUNDS_HZ.len());
        assert_eq!(counts.iter().sum::<u64>(), 3, "no frame is lost");
        assert_eq!(counts[frame_rate_band(40.0)], 1);
        assert_eq!(counts[frame_rate_band(25.0)], 1);
        assert_eq!(counts[frame_rate_band(2.0)], 1);
    }

    #[test]
    fn a_rate_off_either_end_still_lands_in_a_band() {
        assert_eq!(frame_rate_band(0.5), 0, "slower than the first bound");
        assert_eq!(
            frame_rate_band(500.0),
            OUTPUT_FRAME_RATE_BAND_BOUNDS_HZ.len() - 1,
            "faster than the last bound"
        );
        // The bands are ordered, so a faster frame never lands in an earlier band.
        let mut previous = 0;
        for rate in 1..600 {
            let band = frame_rate_band(rate as f32 / 4.0);
            assert!(band >= previous);
            previous = band;
        }
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
        assert!(health.frame_rate_band_counts.is_empty());
    }
}
