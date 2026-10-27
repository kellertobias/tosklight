/// Detachment or a completed blank permits a terminal queue fence, never immediate GPU drop.
/// Admission skips, acquisition failures, and completion of an older picture do not qualify.
#[derive(Default, PartialEq, Eq)]
pub(super) enum Retirement {
    #[default]
    Active,
    AwaitingBlank,
    BlankInFlight,
    AwaitingQueue,
    Complete,
}

impl Retirement {
    pub fn begin(&mut self, detached: bool) {
        if *self == Self::Active {
            *self = if detached {
                Self::AwaitingQueue
            } else {
                Self::AwaitingBlank
            };
        }
    }

    pub fn submitted(&mut self, blank: bool) {
        if *self == Self::AwaitingBlank && blank {
            *self = Self::BlankInFlight;
        }
    }

    pub fn completed(&mut self) {
        if *self == Self::BlankInFlight {
            *self = Self::AwaitingQueue;
        }
    }

    pub fn awaiting_queue(&self) -> bool {
        *self == Self::AwaitingQueue
    }

    pub fn window_gone(&mut self) {
        if !self.complete() {
            *self = Self::AwaitingQueue;
        }
    }

    pub fn queue_completed(&mut self) {
        if self.awaiting_queue() {
            *self = Self::Complete;
        }
    }

    pub fn closing(&self) -> bool {
        *self != Self::Active
    }

    pub fn complete(&self) -> bool {
        *self == Self::Complete
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_picture_completion_and_skipped_clear_cannot_finish_retirement() {
        let mut state = Retirement::default();
        state.begin(false);
        state.completed(); // A composition already in flight when close began.
        assert!(!state.complete());
        state.submitted(false);
        state.completed();
        assert!(!state.complete());
        // Skips/timeouts never invoke submitted; repeated begin is also harmless.
        state.begin(false);
        assert!(!state.complete());
        state.submitted(true);
        assert!(!state.complete());
        state.completed();
        assert!(state.awaiting_queue());
        assert!(!state.complete());
        state.queue_completed();
        assert!(state.complete());
        state.begin(false);
        assert!(state.complete());
    }

    #[test]
    fn native_detachment_needs_terminal_queue_completion_but_no_clear() {
        let mut state = Retirement::default();
        state.begin(true);
        assert!(state.closing());
        assert!(state.awaiting_queue());
        state.completed(); // An old picture callback cannot stand in for the queue fence.
        assert!(!state.complete());
        state.queue_completed();
        assert!(state.complete());
    }

    #[test]
    fn queue_completion_before_blank_does_not_retire_portable_surface() {
        let mut state = Retirement::default();
        state.begin(false);
        state.queue_completed();
        assert!(!state.complete());
        state.submitted(true);
        state.queue_completed();
        assert!(!state.complete());
        state.completed();
        assert!(state.awaiting_queue());
        state.queue_completed();
        assert!(state.complete());
    }

    #[test]
    fn destroyed_window_needs_no_blank_but_still_waits_for_terminal_queue() {
        let mut state = Retirement::default();
        state.begin(false);
        state.window_gone();
        state.completed();
        assert!(state.awaiting_queue());
        assert!(!state.complete());
        state.window_gone();
        state.queue_completed();
        assert!(state.complete());
        state.window_gone();
        assert!(state.complete());
    }
}
