use std::time::Duration;

const FRAME_INTERVAL: Duration = Duration::from_nanos(16_666_667);
const OCCLUSION_PROBE_INTERVAL: Duration = Duration::from_millis(50);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DrawOutcome {
    Submitted,
    CleanSkipped,
    HiddenSkipped,
    BusySkipped,
    PacedSkipped,
    Retry,
    Occluded,
}

#[derive(Clone)]
pub(super) struct Schedule {
    pub visible: bool,
    pub occluded: bool,
    dirty: bool,
    next_submission: Option<Duration>,
    next_probe: Duration,
    next_retry: Duration,
}

impl Default for Schedule {
    fn default() -> Self {
        Self {
            visible: true,
            occluded: false,
            dirty: true,
            next_submission: None,
            next_probe: Duration::ZERO,
            next_retry: Duration::ZERO,
        }
    }
}

impl Schedule {
    /// Small polling ceilings bound completion/input latency, while the final fractional
    /// frame interval remains exact rather than rounding the 60 Hz deadline to a pump tick.
    pub fn next_poll_delay(&self, now: Duration, busy: bool) -> Duration {
        let active_poll = Duration::from_millis(2);
        let idle_poll = Duration::from_millis(8);
        if busy {
            return active_poll;
        }
        if !self.visible {
            return idle_poll;
        }
        if self.occluded {
            return self.next_probe.saturating_sub(now).min(idle_poll);
        }
        if !self.dirty {
            return idle_poll;
        }
        let frame_due = self.next_submission.unwrap_or(Duration::ZERO);
        frame_due
            .max(self.next_retry)
            .saturating_sub(now)
            .min(active_poll)
    }

    pub fn dirty(&mut self) {
        self.dirty = true;
    }

    pub fn set_visible(&mut self, visible: bool) {
        if self.visible != visible {
            self.visible = visible;
            self.dirty = true;
            if visible {
                self.next_probe = Duration::ZERO;
                self.next_submission = None;
                self.next_retry = Duration::ZERO;
            }
        }
    }

    pub fn admission(&self, now: Duration, busy: bool) -> Result<bool, DrawOutcome> {
        if !self.visible {
            return Err(DrawOutcome::HiddenSkipped);
        }
        if busy {
            return Err(DrawOutcome::BusySkipped);
        }
        if self.occluded {
            return if now >= self.next_probe {
                Ok(true)
            } else {
                Err(DrawOutcome::Occluded)
            };
        }
        if !self.dirty {
            return Err(DrawOutcome::CleanSkipped);
        }
        if now < self.next_retry {
            return Err(DrawOutcome::PacedSkipped);
        }
        if self.next_submission.is_some_and(|deadline| now < deadline) {
            return Err(DrawOutcome::PacedSkipped);
        }
        Ok(false)
    }

    pub fn acquired_occluded(&mut self, now: Duration) {
        self.occluded = true;
        self.next_probe = now + OCCLUSION_PROBE_INTERVAL;
        self.dirty = true;
    }

    pub fn revealed(&mut self) {
        self.occluded = false;
        self.dirty = true;
        self.next_retry = Duration::ZERO;
        self.next_submission = None;
    }

    pub fn retry(&mut self, now: Duration) {
        self.next_retry = now + FRAME_INTERVAL;
    }

    pub fn submitted(&mut self, now: Duration) {
        // Preserve 60 Hz phase for ordinary jitter. A whole missed slot instead re-anchors,
        // preventing a near-immediate second composition after a non-integral long stall.
        let previous = self.next_submission.unwrap_or(now);
        self.next_submission = Some(if now.saturating_sub(previous) >= FRAME_INTERVAL {
            now + FRAME_INTERVAL
        } else {
            previous + FRAME_INTERVAL
        });
        self.dirty = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recurring_late_submissions_preserve_sixty_hertz_phase() {
        let mut schedule = Schedule::default();
        schedule.submitted(Duration::ZERO);
        for index in 1..=600_u32 {
            let now = FRAME_INTERVAL * index + Duration::from_micros(500);
            schedule.dirty();
            assert_eq!(schedule.admission(now, false), Ok(false));
            schedule.submitted(now);
            assert_eq!(schedule.next_submission, Some(FRAME_INTERVAL * (index + 1)));
            schedule.dirty();
            assert_eq!(
                schedule.admission(now, false),
                Err(DrawOutcome::PacedSkipped)
            );
        }
    }

    #[test]
    fn stalled_compositor_skips_whole_slots_and_reveal_resets_phase() {
        let mut schedule = Schedule::default();
        schedule.submitted(Duration::ZERO);
        schedule.dirty();
        let late = Duration::from_secs(5) + Duration::from_millis(16);
        assert_eq!(schedule.admission(late, false), Ok(false));
        schedule.submitted(late);
        let next = schedule.next_submission.unwrap();
        assert_eq!(next, late + FRAME_INTERVAL);
        schedule.dirty();
        assert_eq!(
            schedule.admission(late, false),
            Err(DrawOutcome::PacedSkipped)
        );
        assert_eq!(
            schedule.admission(late + FRAME_INTERVAL - Duration::from_nanos(1), false),
            Err(DrawOutcome::PacedSkipped)
        );
        assert_eq!(schedule.admission(late + FRAME_INTERVAL, false), Ok(false));
        schedule.set_visible(false);
        schedule.set_visible(true);
        assert_eq!(schedule.admission(late, false), Ok(false));
        schedule.submitted(late);
        assert_eq!(schedule.next_submission, Some(late + FRAME_INTERVAL));
    }

    #[test]
    fn poll_deadlines_keep_fractional_frame_remainders_without_a_minimum_floor() {
        let mut schedule = Schedule::default();
        assert_eq!(
            schedule.next_poll_delay(Duration::ZERO, false),
            Duration::ZERO
        );
        schedule.submitted(Duration::ZERO);
        assert_eq!(
            schedule.next_poll_delay(Duration::from_millis(1), false),
            Duration::from_millis(8)
        );
        schedule.dirty();
        assert_eq!(
            schedule.next_poll_delay(Duration::from_millis(1), false),
            Duration::from_millis(2)
        );
        assert_eq!(
            schedule.next_poll_delay(Duration::from_millis(16), false),
            Duration::from_nanos(666_667)
        );
        assert_eq!(
            schedule.next_poll_delay(FRAME_INTERVAL, false),
            Duration::ZERO
        );
        schedule.retry(Duration::from_millis(20));
        assert_eq!(
            schedule.next_poll_delay(Duration::from_millis(36), false),
            Duration::from_nanos(666_667)
        );
        schedule.set_visible(false);
        assert_eq!(
            schedule.next_poll_delay(Duration::from_secs(1), true),
            Duration::from_millis(2)
        );
        assert_eq!(
            schedule.next_poll_delay(Duration::from_secs(1), false),
            Duration::from_millis(8)
        );
    }

    #[test]
    fn clean_and_hidden_ticks_do_not_admit_surface_work() {
        let mut schedule = Schedule::default();
        schedule.submitted(Duration::ZERO);
        assert_eq!(
            schedule.admission(Duration::from_secs(1), false),
            Err(DrawOutcome::CleanSkipped)
        );
        schedule.set_visible(false);
        assert_eq!(
            schedule.admission(Duration::from_secs(1), false),
            Err(DrawOutcome::HiddenSkipped)
        );
        schedule.set_visible(true);
        assert_eq!(schedule.admission(Duration::from_secs(1), false), Ok(false));
    }

    #[test]
    fn dirty_work_survives_busy_pacing_and_failed_acquisition() {
        let mut schedule = Schedule::default();
        schedule.submitted(Duration::ZERO);
        schedule.dirty();
        assert_eq!(
            schedule.admission(Duration::from_millis(2), true),
            Err(DrawOutcome::BusySkipped)
        );
        assert_eq!(
            schedule.admission(Duration::from_millis(16), false),
            Err(DrawOutcome::PacedSkipped)
        );
        let now = Duration::from_nanos(16_666_667);
        assert_eq!(schedule.admission(now, false), Ok(false));
        // A failed acquire backs off without consuming the dirty picture.
        schedule.retry(now);
        assert_eq!(
            schedule.admission(now + Duration::from_millis(1), false),
            Err(DrawOutcome::PacedSkipped)
        );
        assert_eq!(
            schedule.admission(now + Duration::from_millis(17), false),
            Ok(false)
        );
        schedule.submitted(now + Duration::from_millis(17));
        assert_eq!(
            schedule.admission(now + Duration::from_millis(40), false),
            Err(DrawOutcome::CleanSkipped)
        );
    }

    #[test]
    fn occlusion_uses_bounded_probes_and_reveal_requires_a_fresh_draw() {
        let mut schedule = Schedule::default();
        schedule.acquired_occluded(Duration::ZERO);
        assert_eq!(
            schedule.admission(Duration::from_millis(49), false),
            Err(DrawOutcome::Occluded)
        );
        assert_eq!(
            schedule.admission(Duration::from_millis(50), false),
            Ok(true)
        );
        assert_eq!(
            schedule.next_poll_delay(Duration::from_millis(49), false),
            Duration::from_millis(1)
        );
        schedule.acquired_occluded(Duration::from_millis(50));
        assert_eq!(
            schedule.admission(Duration::from_millis(99), false),
            Err(DrawOutcome::Occluded)
        );
        schedule.revealed();
        assert_eq!(
            schedule.admission(Duration::from_millis(100), false),
            Ok(false)
        );
    }
}
