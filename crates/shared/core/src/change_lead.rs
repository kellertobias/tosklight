//! TL-659: the starts an output frame has to carry, for the desk's change lead time.
//!
//! The change lead time is the time from when a Cue or Dynamic should start outputting (its
//! triggered or scheduled start instant) to when the output frame that first carries it is sent.
//! A start is marked here by the domain that owns it, under that domain's own lock, and claimed by
//! the first output frame sampled at or after its instant, under the same lock. The output side
//! then records the lead once that frame is on the wire.
//!
//! Marking and claiming are constant-time over a fixed number of slots and never allocate, so
//! the ledger can sit on the output path.

/// Pending start instants, at most this many distinct ones between two frames.
const SLOTS: usize = 8;

/// Start instants, in microseconds of application time, waiting for their first output frame.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ChangeLeadLedger {
    slots: [Option<i64>; SLOTS],
}

impl ChangeLeadLedger {
    /// Remembers that something should start outputting at `intended_micros`.
    ///
    /// A frame records only its earliest carried start, so two starts claimed by one frame need
    /// one slot. When every slot is taken, the start joins the slot nearest to it, keeping the
    /// earlier instant: starts that close together share their first frame.
    pub fn mark(&mut self, intended_micros: i64) {
        if let Some(slot) = self.slots.iter_mut().find(|slot| slot.is_none()) {
            *slot = Some(intended_micros);
            return;
        }
        let nearest = self
            .slots
            .iter_mut()
            .flatten()
            .min_by_key(|pending| pending.abs_diff(intended_micros))
            .expect("a full ledger has a pending start");
        *nearest = (*nearest).min(intended_micros);
    }

    /// Takes every start due at or before `sampled_micros` and returns the earliest of them.
    ///
    /// The caller is the output frame sampled at that instant: it carries each of these starts,
    /// and its lead is measured from the earliest.
    pub fn claim(&mut self, sampled_micros: i64) -> Option<i64> {
        let mut earliest: Option<i64> = None;
        for slot in &mut self.slots {
            if let Some(intended) = *slot
                && intended <= sampled_micros
            {
                *slot = None;
                earliest = Some(earliest.map_or(intended, |current| current.min(intended)));
            }
        }
        earliest
    }

    /// Whether any start is still waiting for its frame.
    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(Option::is_none)
    }
}

/// The earlier of two optional start instants.
pub fn earliest_start(first: Option<i64>, second: Option<i64>) -> Option<i64> {
    match (first, second) {
        (Some(first), Some(second)) => Some(first.min(second)),
        (first, second) => first.or(second),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_claims_only_the_starts_that_are_due() {
        let mut ledger = ChangeLeadLedger::default();
        ledger.mark(1_000);
        ledger.mark(5_000);
        ledger.mark(800);

        assert_eq!(ledger.claim(2_000), Some(800), "earliest due start");
        assert_eq!(
            ledger.claim(4_999),
            None,
            "the scheduled start is not due yet"
        );
        assert_eq!(ledger.claim(5_000), Some(5_000));
        assert!(ledger.is_empty());
    }

    #[test]
    fn a_full_ledger_keeps_the_earlier_of_two_neighbouring_starts() {
        let mut ledger = ChangeLeadLedger::default();
        for slot in 0..SLOTS as i64 {
            ledger.mark(slot * 1_000_000);
        }
        ledger.mark(3_000_100);

        assert_eq!(ledger.claim(2_500_000), Some(0));
        assert_eq!(
            ledger.claim(3_000_100),
            Some(3_000_000),
            "the merged start is carried by the frame of its earlier neighbour"
        );
        ledger.mark(7_000_100);
        ledger.mark(6_999_900);
        assert_eq!(ledger.claim(7_000_000), Some(4_000_000));
    }

    #[test]
    fn the_earliest_start_ignores_a_missing_side() {
        assert_eq!(earliest_start(Some(4), Some(2)), Some(2));
        assert_eq!(earliest_start(None, Some(2)), Some(2));
        assert_eq!(earliest_start(Some(4), None), Some(4));
        assert_eq!(earliest_start(None, None), None);
    }
}
