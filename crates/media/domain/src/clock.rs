//! The per-output render clock.
//!
//! Each output owns its clock. Outputs on displays with different refresh rates never share a
//! global frame counter, and playback advances from monotonic timestamps rather than assuming one
//! update equals 1/60 second.
//!
//! The clock reads no time itself — the renderer stamps every call — so its behavior is
//! deterministic and testable without a display.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::command::Timestamp;
use crate::output::PresentationMode;

/// How many recent intervals the measured cadence averages over. Roughly a second at 60 Hz:
/// long enough to be steady, short enough to notice a monitor changing mode.
const CADENCE_WINDOW: usize = 60;

/// What the output actually achieved, as opposed to what it asked for.
///
/// `DisplaySynchronized` records this rather than hard-coding 60 Hz, so diagnostics report the
/// cadence the surface really presented at.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeasuredCadence {
    /// Frames presented since the clock was created or reset.
    pub frames: u64,
    /// Mean interval across the recent window, once there is more than one frame to measure.
    pub mean_interval: Option<Duration>,
    /// The most recent interval.
    pub last_interval: Option<Duration>,
}

impl MeasuredCadence {
    /// The mean cadence in frames per second, when one has been measured.
    pub fn frames_per_second(&self) -> Option<f32> {
        self.mean_interval
            .filter(|interval| !interval.is_zero())
            .map(|interval| 1.0 / interval.as_secs_f32())
    }
}

/// One output's presentation pacing.
#[derive(Debug, Clone)]
pub struct RenderClock {
    mode: PresentationMode,
    /// The deadline a fixed-rate output is scheduled against. Deadlines advance by exact
    /// intervals rather than from the last present, so a late frame does not push every later
    /// frame late with it.
    next_deadline: Option<Timestamp>,
    last_present: Option<Timestamp>,
    intervals: Vec<Duration>,
    frames: u64,
}

impl RenderClock {
    pub fn new(mode: PresentationMode) -> Self {
        Self {
            mode,
            next_deadline: None,
            last_present: None,
            intervals: Vec::with_capacity(CADENCE_WINDOW),
            frames: 0,
        }
    }

    pub const fn mode(&self) -> PresentationMode {
        self.mode
    }

    /// The interval a fixed-rate output targets. Display-synchronized and unlocked outputs have
    /// no target of their own: the surface paces one and nothing paces the other.
    pub const fn target_interval(&self) -> Option<Duration> {
        match self.mode {
            PresentationMode::FixedFps { frames_per_second } if frames_per_second > 0 => {
                Some(Duration::from_nanos(1_000_000_000 / frames_per_second as u64))
            }
            _ => None,
        }
    }

    /// Whether the output should present now.
    ///
    /// A display-synchronized output always says yes and lets the surface block; an unlocked one
    /// always says yes and never blocks. Only a fixed-rate output withholds a frame.
    pub fn should_present(&self, now: Timestamp) -> bool {
        match self.next_deadline {
            Some(deadline) => now >= deadline,
            None => true,
        }
    }

    /// How long a fixed-rate output should wait before its next frame.
    pub fn time_until_deadline(&self, now: Timestamp) -> Duration {
        self.next_deadline
            .map_or(Duration::ZERO, |deadline| deadline.since(now))
    }

    /// Records that a frame was presented.
    pub fn record_present(&mut self, now: Timestamp) {
        if let Some(previous) = self.last_present {
            let interval = now.since(previous);
            if self.intervals.len() == CADENCE_WINDOW {
                self.intervals.remove(0);
            }
            self.intervals.push(interval);
        }
        self.last_present = Some(now);
        self.frames += 1;

        if let Some(target) = self.target_interval() {
            // Advance from the deadline, not from now, so a run of on-time frames stays exactly
            // on rate. If the output has fallen more than a whole interval behind, resynchronize
            // rather than trying to catch up with a burst.
            let next = self.next_deadline.unwrap_or(now).plus(target);
            self.next_deadline = Some(if next <= now { now.plus(target) } else { next });
        }
    }

    /// What this output actually achieved.
    pub fn measured(&self) -> MeasuredCadence {
        let mean = if self.intervals.is_empty() {
            None
        } else {
            let total: u64 = self.intervals.iter().map(|interval| interval.as_micros() as u64).sum();
            Some(Duration::from_micros(total / self.intervals.len() as u64))
        };
        MeasuredCadence {
            frames: self.frames,
            mean_interval: mean,
            last_interval: self.intervals.last().copied(),
        }
    }

    /// Forgets the measured history without forgetting the mode.
    ///
    /// Called when a monitor changes, a refresh rate changes, or a surface is recreated: the
    /// cadence from before the change says nothing about the cadence after it.
    pub fn reset(&mut self) {
        self.next_deadline = None;
        self.last_present = None;
        self.intervals.clear();
        self.frames = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIXTY: PresentationMode = PresentationMode::FixedFps { frames_per_second: 60 };

    fn micros(value: u64) -> Timestamp {
        Timestamp::from_micros(value)
    }

    #[test]
    fn a_display_synchronized_output_never_withholds_a_frame() {
        let mut clock = RenderClock::new(PresentationMode::DisplaySynchronized);
        assert_eq!(clock.target_interval(), None);
        for frame in 0..10 {
            let now = micros(frame * 16_667);
            assert!(clock.should_present(now));
            clock.record_present(now);
        }
        assert_eq!(clock.measured().frames, 10);
    }

    #[test]
    fn an_unlocked_output_never_withholds_a_frame_either() {
        let mut clock = RenderClock::new(PresentationMode::Unlocked);
        assert_eq!(clock.target_interval(), None);
        clock.record_present(micros(0));
        assert!(clock.should_present(micros(1)));
    }

    #[test]
    fn a_fixed_rate_output_targets_its_own_interval() {
        let clock = RenderClock::new(SIXTY);
        assert_eq!(clock.target_interval(), Some(Duration::from_nanos(16_666_666)));

        let thirty = RenderClock::new(PresentationMode::FixedFps { frames_per_second: 30 });
        assert_eq!(thirty.target_interval(), Some(Duration::from_nanos(33_333_333)));
    }

    #[test]
    fn a_fixed_rate_output_withholds_a_frame_until_its_deadline() {
        let mut clock = RenderClock::new(SIXTY);
        assert!(clock.should_present(micros(0)), "the first frame has no deadline to wait for");
        clock.record_present(micros(0));

        assert!(!clock.should_present(micros(16_000)));
        assert!(clock.time_until_deadline(micros(16_000)) > Duration::ZERO);
        assert!(clock.should_present(micros(16_666)));
    }

    #[test]
    fn deadlines_advance_by_exact_intervals_so_one_late_frame_does_not_drift_the_rest() {
        let mut clock = RenderClock::new(SIXTY);
        clock.record_present(micros(0));
        // Presented three microseconds late.
        clock.record_present(micros(16_669));
        // The next deadline is still on the original grid, not 16_669 + interval.
        assert!(clock.should_present(micros(33_334)));
    }

    #[test]
    fn an_output_that_has_fallen_a_whole_interval_behind_resynchronizes() {
        let mut clock = RenderClock::new(SIXTY);
        clock.record_present(micros(0));
        // A long stall: a whole second passes before the next frame.
        clock.record_present(micros(1_000_000));
        assert!(!clock.should_present(micros(1_000_100)), "no catch-up burst");
        assert!(clock.should_present(micros(1_016_667)));
    }

    #[test]
    fn measured_cadence_reports_what_actually_happened() {
        let mut clock = RenderClock::new(PresentationMode::DisplaySynchronized);
        assert_eq!(clock.measured().mean_interval, None, "one frame measures nothing");

        for frame in 0..=10u64 {
            clock.record_present(micros(frame * 10_000));
        }
        let measured = clock.measured();
        assert_eq!(measured.frames, 11);
        assert_eq!(measured.mean_interval, Some(Duration::from_micros(10_000)));
        assert_eq!(measured.last_interval, Some(Duration::from_micros(10_000)));
        assert_eq!(measured.frames_per_second(), Some(100.0));
    }

    #[test]
    fn the_cadence_window_forgets_old_intervals() {
        let mut clock = RenderClock::new(PresentationMode::DisplaySynchronized);
        // A slow start that must not weigh on the average for ever.
        clock.record_present(micros(0));
        clock.record_present(micros(1_000_000));
        let mut now = 1_000_000;
        for _ in 0..CADENCE_WINDOW {
            now += 10_000;
            clock.record_present(micros(now));
        }
        assert_eq!(clock.measured().mean_interval, Some(Duration::from_micros(10_000)));
    }

    #[test]
    fn a_surface_change_resets_the_measurement_but_not_the_mode() {
        let mut clock = RenderClock::new(SIXTY);
        clock.record_present(micros(0));
        clock.record_present(micros(16_667));
        clock.reset();

        assert_eq!(clock.measured(), MeasuredCadence::default());
        assert_eq!(clock.mode(), SIXTY);
        assert!(clock.should_present(micros(20_000)), "the deadline grid restarts too");
    }

    #[test]
    fn a_zero_frame_rate_has_no_target_rather_than_dividing_by_zero() {
        let clock = RenderClock::new(PresentationMode::FixedFps { frames_per_second: 0 });
        assert_eq!(clock.target_interval(), None);
        assert!(clock.should_present(Timestamp::ZERO));
    }
}
