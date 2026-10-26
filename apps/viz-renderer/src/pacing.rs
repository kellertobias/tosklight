//! Deciding when the next frame is due.
//!
//! Display-driven modes estimate refresh timing from how long acquisition waits for a drawable.
//! Draft and 3D Lines additionally limit steady scheduled rendering to 60 Hz.
//! The drawable is only released on the refresh, so a frame started any earlier
//! spends the difference blocked inside the graphics driver — on the same thread that delivers
//! mouse and keyboard events. An unpaced window ends up seconds behind the operator's hand.

use std::time::{Duration, Instant};
use viz_scene::{RenderQuality, ViewMode};

/// Scheduled animation ceiling only. Input/reveal redraws retain their immediate exception.
/// Phase belongs to successful submissions; pending requests cannot advance it or queue bursts.
#[derive(Default)]
pub struct LowGpuSchedule {
    due: Option<Instant>,
    pending: bool,
    diagnostics: PacingDiagnostics,
}
/// Bounded CPU observations only; reason predicates may overlap on one successful frame.
#[derive(Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PacingDiagnostics {
    successful_low_gpu_frames: u64,
    late_admission_reanchors: u64,
    render_crossed_deadline_reanchors: u64,
    adaptive_above_period_frames: u64,
    adaptive_deadline_extensions: u64,
    adaptive_interval_nanos_range: Option<(u64, u64)>,
}
impl PacingDiagnostics {
    pub fn merge(&mut self, other: Self) {
        self.successful_low_gpu_frames += other.successful_low_gpu_frames;
        self.late_admission_reanchors += other.late_admission_reanchors;
        self.render_crossed_deadline_reanchors += other.render_crossed_deadline_reanchors;
        self.adaptive_above_period_frames += other.adaptive_above_period_frames;
        self.adaptive_deadline_extensions += other.adaptive_deadline_extensions;
        if let Some((low, high)) = other.adaptive_interval_nanos_range {
            self.adaptive_interval_nanos_range = Some(
                self.adaptive_interval_nanos_range
                    .map_or((low, high), |(a, b)| (a.min(low), b.max(high))),
            );
        }
    }
    fn observe(&mut self, adaptive: Duration) {
        self.successful_low_gpu_frames += 1;
        let nanos = adaptive.as_nanos().min(u64::MAX as u128) as u64;
        self.adaptive_interval_nanos_range = Some(
            self.adaptive_interval_nanos_range
                .map_or((nanos, nanos), |(low, high)| {
                    (low.min(nanos), high.max(nanos))
                }),
        );
    }
}

pub const LOW_GPU_PERIOD: Duration = Duration::from_nanos(16_666_667);

pub fn low_gpu_mode(quality: RenderQuality, mode: ViewMode) -> bool {
    quality == RenderQuality::Draft || mode == ViewMode::Lines3d
}

impl LowGpuSchedule {
    pub fn take_diagnostics(&mut self) -> PacingDiagnostics {
        std::mem::take(&mut self.diagnostics)
    }

    /// Winit coalesces redraws, but retain explicit ownership until delivery as well.
    pub fn request(&mut self) -> bool {
        if self.pending {
            return false;
        }
        self.pending = true;
        true
    }
    pub fn delivered(&mut self) {
        self.pending = false;
    }
    /// Idle, hidden, mode transitions and failed attempts owe no catch-up work.
    pub fn suspend(&mut self) {
        self.due = None;
    }
    pub fn succeeded(
        &mut self,
        started: Instant,
        completed: Instant,
        adaptive: Duration,
    ) -> Instant {
        self.diagnostics.observe(adaptive);
        if self
            .due
            .is_some_and(|due| started.saturating_duration_since(due) >= LOW_GPU_PERIOD)
        {
            self.diagnostics.late_admission_reanchors += 1;
        }
        let mut next = match self.due {
            Some(due) if started >= due && started.duration_since(due) < LOW_GPU_PERIOD => {
                due + LOW_GPU_PERIOD
            }
            _ => started + LOW_GPU_PERIOD,
        };
        // A render that itself exhausted the next slot must not cause a catch-up pair.
        if completed >= next {
            self.diagnostics.render_crossed_deadline_reanchors += 1;
            next = completed + LOW_GPU_PERIOD;
        }
        // Respect the acquisition controller's slower backoff. Ignoring its output while
        // continuing to integrate acquire error caused windup in the native diagnostic.
        if adaptive > LOW_GPU_PERIOD {
            self.diagnostics.adaptive_above_period_frames += 1;
            self.diagnostics.adaptive_deadline_extensions += u64::from(completed + adaptive > next);
            next = next.max(completed + adaptive);
        }
        self.due = Some(next);
        next
    }
    pub fn retry(&mut self, now: Instant, existing: Instant) -> Instant {
        self.suspend();
        existing.max(now + LOW_GPU_PERIOD)
    }
}

/// Frame pacing for a display whose refresh rate the window system will not name: 60 Hz.
pub const DEFAULT_FRAME_INTERVAL: Duration = Duration::from_micros(16_667);

/// How much of a refresh interval a frame is given before the next one is due, as a starting
/// point. The measured wait takes over from the first presented frame.
pub const FRAME_PACE: f64 = 0.9;

/// How long a frame is allowed to sit waiting for the display to release a drawable before the
/// pace is eased back. Some wait is wanted — it is the margin that keeps every refresh fed, and
/// measured runs drop frames without it — but it is time the event loop cannot deliver input in.
/// Two milliseconds of a sixteen-millisecond frame keeps both.
pub const ACQUIRE_TARGET: f64 = 0.002;

/// How much of the difference between the measured wait and the target is corrected each frame.
/// Gentle enough that one late frame does not swing the pace, quick enough to follow a display
/// that has changed its refresh rate within a few frames.
pub const PACE_CORRECTION: f64 = 0.25;

/// How often to present while the window is hidden behind another one. Nothing is visible, so the
/// only reason to come round at all is to notice that the window is back.
pub const HIDDEN_FRAME_INTERVAL: Duration = Duration::from_millis(100);

/// Move the pace towards presenting each frame just as the display is ready for it.
///
/// A drawable is only released when the display refreshes, so a frame that starts too early spends
/// the difference blocked inside the driver — on the thread that delivers mouse and keyboard
/// events, which is why an unpaced visualizer ends up seconds behind the operator's hand. A
/// nominal refresh rate is not enough to pace by: a display can vary its rate, and a heavy frame
/// misses a refresh anyway. So the measured wait steers the pace: wait too long and the next frame
/// falls due later, wait for nothing and it falls due sooner.
pub fn paced_interval(current: Duration, waited: Duration) -> Duration {
    let error = waited.as_secs_f64() - ACQUIRE_TARGET;
    let interval = current.as_secs_f64() + error * PACE_CORRECTION;
    Duration::from_secs_f64(interval.clamp(0.002, 0.05))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn low_gpu_selection_does_not_change_other_quality_modes() {
        for quality in [
            RenderQuality::Standard,
            RenderQuality::High,
            RenderQuality::Ultra,
        ] {
            assert!(!low_gpu_mode(quality, ViewMode::Full3d));
            assert!(low_gpu_mode(quality, ViewMode::Lines3d));
        }
        assert!(low_gpu_mode(RenderQuality::Draft, ViewMode::Full3d));
    }

    /// How long a frame would wait for a display refreshing every `period` seconds when the pace
    /// brings it round `interval` after the last one was presented.
    fn wait_for(period: f64, interval: Duration) -> Duration {
        Duration::from_secs_f64((period - interval.as_secs_f64()).max(0.0))
    }

    /// The pace has to find the display on its own, from nothing but how long each frame waited,
    /// and settle just short of the refresh: short enough that every refresh still gets a frame,
    /// early enough that the wait — the part of the cycle where input cannot be delivered — is
    /// down to a moment.
    #[test]
    fn the_pace_settles_just_short_of_the_refresh_whatever_the_display() {
        for period in [1.0 / 60.0, 1.0 / 120.0, 1.0 / 240.0] {
            let mut interval = DEFAULT_FRAME_INTERVAL;
            let mut waited = wait_for(period, interval);
            for _ in 0..200 {
                interval = paced_interval(interval, waited);
                waited = wait_for(period, interval);
            }
            assert!(
                waited.as_secs_f64() < 0.0025,
                "a {:.0} Hz display still left {:.2} ms of every frame blocked",
                1.0 / period,
                waited.as_secs_f64() * 1000.0
            );
            assert!(
                interval.as_secs_f64() < period,
                "the pace overshot the refresh and would drop frames: {interval:?}"
            );
            assert!(
                interval.as_secs_f64() > period * 0.5,
                "the pace collapsed to half the refresh rate: {interval:?}"
            );
        }
    }

    /// A display slower than the frames it is fed must not leave the loop spinning: the pace has
    /// to back off to whatever the GPU can actually present.
    #[test]
    fn a_frame_slower_than_the_display_pushes_the_pace_out() {
        let mut interval = Duration::from_millis(4);
        for _ in 0..200 {
            interval = paced_interval(interval, wait_for(0.04, interval));
        }
        assert!(
            interval.as_secs_f64() > 0.03,
            "the pace stayed ahead of a 25 Hz display: {interval:?}"
        );
    }
}

#[cfg(test)]
#[path = "pacing_schedule_tests.rs"]
mod schedule_tests;
