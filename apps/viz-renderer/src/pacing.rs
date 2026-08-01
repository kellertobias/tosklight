//! Deciding when the next frame is due.
//!
//! The visualizer presents one frame per display refresh, and works out when that refresh is due
//! from how long each frame waited for the display to hand back a drawable. This is not a frame
//! rate limiter: the drawable is only released on the refresh, so a frame started any earlier
//! spends the difference blocked inside the graphics driver — on the same thread that delivers
//! mouse and keyboard events. An unpaced window ends up seconds behind the operator's hand.

use std::time::Duration;

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
