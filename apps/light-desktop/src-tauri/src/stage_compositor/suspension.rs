/// Portable suspension clears once, retaining the resident front independently of the blank
/// drawable. Native owned-layer hiding requires no submission at all.
#[derive(Default)]
pub(super) struct Suspension {
    pub pending: bool,
    pub in_flight: bool,
}

impl Suspension {
    pub fn hide(&mut self, native_hidden: bool) {
        self.pending = !native_hidden && !self.in_flight;
    }
    pub fn reveal(&mut self) {
        self.pending = false;
    }
    pub fn submitted(&mut self) {
        self.pending = false;
        self.in_flight = true;
    }
    pub fn completed(&mut self) {
        self.in_flight = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn portable_hide_clears_once_and_reveal_does_not_cancel_an_inflight_clear() {
        let mut state = Suspension::default();
        state.hide(false);
        assert!(state.pending);
        state.submitted();
        assert!(!state.pending);
        state.reveal();
        state.hide(false);
        assert!(!state.pending);
        assert!(state.in_flight);
        state.completed();
        assert!(!state.pending && !state.in_flight);
        state.hide(true);
        assert!(!state.pending);
    }
}
