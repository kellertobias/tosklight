//! Generation-scoped newest-picture admission, separate from retained GPU credit ownership.
use viz_helper::protocol::{FrameId, SHARED_FRAME_SLOTS};

#[derive(Default)]
pub(super) struct ReadySequence {
    generation: u64,
    per_slot: [u64; SHARED_FRAME_SLOTS],
    newest: u64,
}
impl ReadySequence {
    pub fn reset(&mut self, generation: u64) {
        *self = Self {
            generation,
            ..Self::default()
        };
    }

    /// `false` returns a genuinely older other-slot picture to its producer. A duplicate or
    /// replay within the same slot is a protocol error, never a new producer credit.
    pub fn accept(&mut self, frame: FrameId) -> Result<bool, String> {
        if !frame.is_valid() || frame.generation != self.generation {
            return Err("invalid Stage ready-frame generation or slot".into());
        }
        if self.per_slot.contains(&frame.sequence) {
            return Err("duplicate Stage ready-frame sequence across slots".into());
        }
        let last = &mut self.per_slot[usize::from(frame.slot)];
        if frame.sequence <= *last {
            return Err("stale or duplicate Stage ready-frame sequence".into());
        }
        *last = frame.sequence;
        if frame.sequence <= self.newest {
            return Ok(false);
        }
        self.newest = frame.sequence;
        Ok(true)
    }
}

pub(super) fn copy_identity_valid(frame: FrameId) -> bool {
    frame.is_valid() && frame.slot == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(generation: u64, slot: u8, sequence: u64) -> FrameId {
        FrameId {
            generation,
            slot,
            sequence,
        }
    }
    #[test]
    fn reversed_other_slot_completion_never_regresses_the_newest_image() {
        let mut order = ReadySequence::default();
        order.reset(1);
        assert!(order.accept(frame(1, 0, 1)).unwrap());
        assert!(order.accept(frame(1, 2, 3)).unwrap());
        assert!(!order.accept(frame(1, 1, 2)).unwrap());
        assert!(order.accept(frame(1, 1, 2)).is_err());
        assert!(order.accept(frame(1, 1, 4)).unwrap());
        assert!(order.accept(frame(1, 2, 3)).is_err());
        assert!(order.accept(frame(1, 0, 3)).is_err());
    }
    #[test]
    fn resize_resets_order_but_never_admits_an_old_generation() {
        let mut order = ReadySequence::default();
        order.reset(1);
        assert!(order.accept(frame(1, 2, 100)).unwrap());
        order.reset(2);
        assert!(order.accept(frame(1, 1, 101)).is_err());
        assert!(order.accept(frame(3, 1, 1)).is_err());
        assert!(order.accept(frame(2, 0, 1)).unwrap());
        assert!(order.accept(frame(2, 3, 2)).is_err());
        assert!(order.accept(frame(0, 0, 1)).is_err());
        assert!(order.accept(frame(2, 1, 0)).is_err());
        assert!(order.accept(frame(2, 1, 2)).unwrap());
    }
    #[test]
    fn copy_retains_exactly_one_slot_despite_the_larger_shared_capacity() {
        assert!(copy_identity_valid(frame(1, 0, 1)));
        for slot in [1, 2] {
            let shared = frame(1, slot, 2);
            assert!(shared.is_valid());
            assert!(!copy_identity_valid(shared));
        }
        assert!(!copy_identity_valid(frame(0, 0, 1)));
        assert!(!copy_identity_valid(frame(1, 0, 0)));
    }
}
