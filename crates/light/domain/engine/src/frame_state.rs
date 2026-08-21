//! One frame's worth of storage, addressed by slot and reused from one frame to the next.
//!
//! The arrays are sized once per generation, when the patch compiles, and then filled and refilled
//! for as long as that patch stands. Nothing here is cleared between frames: each slot carries the
//! epoch it was last written in, so a slot no source contributed to this frame reads as empty
//! without anyone having to walk the array to blank it.

use chrono::{DateTime, Utc};
use light_core::{AttributeValue, MergeMode};

use crate::Slot;
use crate::contribution::ApplicableSequenceMaster;

/// The value holding a slot after arbitration.
#[derive(Clone)]
pub(crate) struct SlotWinner {
    pub(crate) value: AttributeValue,
    pub(crate) priority: i16,
    pub(crate) changed_at: DateTime<Utc>,
    pub(crate) merge_mode: MergeMode,
    pub(crate) transition_ordinal: Option<u64>,
    pub(crate) sequence_master: Option<ApplicableSequenceMaster>,
}

impl Default for SlotWinner {
    fn default() -> Self {
        Self {
            value: AttributeValue::Normalized(0.0),
            priority: 0,
            changed_at: DateTime::<Utc>::MIN_UTC,
            merge_mode: MergeMode::Ltp,
            transition_ordinal: None,
            sequence_master: None,
        }
    }
}

/// Slot-addressed storage for one frame.
///
/// Belongs to a pool rather than to a frame: a frame borrows it, fills it, is read from it, and
/// hands it back. That is the whole point — a render allocates when the patch changes, not when
/// the clock ticks.
pub(crate) struct FrameState {
    /// The slot numbering these indices address. A frame carrying a different tag than the table
    /// a reader holds is reporting that the show was repatched underneath it.
    generation: u64,
    /// Which fill this is. Bumped rather than blanking `winners`.
    epoch: u32,
    /// The epoch each slot was last written in.
    stamp: Vec<u32>,
    winners: Vec<SlotWinner>,
    /// Slots written this fill, so reading a sparse frame does not scan the whole table.
    touched: Vec<u32>,
}

impl FrameState {
    /// Storage for a generation's shape. Called when a patch compiles, never in the frame loop.
    pub(crate) fn for_generation(generation: u64, slots: usize) -> Self {
        Self {
            generation,
            epoch: 0,
            stamp: vec![0; slots],
            winners: vec![SlotWinner::default(); slots],
            touched: Vec::with_capacity(slots),
        }
    }

    /// The generation whose numbering these slots follow.
    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }

    /// How many slots this storage was shaped for.
    pub(crate) fn capacity(&self) -> usize {
        self.winners.len()
    }

    /// Begin a fill. Every slot reads as empty again without a single write to `winners`.
    pub(crate) fn begin(&mut self) {
        self.touched.clear();
        // A wrap would make stale slots from 4 billion frames ago look current, so the one time it
        // happens the stamps are blanked properly.
        match self.epoch.checked_add(1) {
            Some(next) => self.epoch = next,
            None => {
                self.stamp.iter_mut().for_each(|stamp| *stamp = 0);
                self.epoch = 1;
            }
        }
    }

    fn is_current(&self, slot: Slot) -> bool {
        self.stamp
            .get(slot.index())
            .is_some_and(|stamp| *stamp == self.epoch)
    }

    /// The winner holding this slot in the current fill, if anything contributed to it.
    pub(crate) fn get(&self, slot: Slot) -> Option<&SlotWinner> {
        self.is_current(slot)
            .then(|| self.winners.get(slot.index()))
            .flatten()
    }

    /// Offer a value for a slot, keeping whichever of the two the merge rules prefer.
    ///
    /// `build` is only called when the candidate actually wins, so a losing contribution costs a
    /// comparison rather than a clone. `normalized` is the candidate's level for the HTP
    /// comparison, and is ignored for every other merge mode.
    pub(crate) fn offer(
        &mut self,
        slot: Slot,
        priority: i16,
        changed_at: DateTime<Utc>,
        merge_mode: MergeMode,
        transition_ordinal: Option<u64>,
        normalized: f32,
        build: impl FnOnce(&mut SlotWinner),
    ) {
        let index = slot.index();
        if index >= self.winners.len() {
            return;
        }
        let held = self.stamp[index] == self.epoch;
        if held {
            let current = &self.winners[index];
            let wins = if priority != current.priority {
                priority > current.priority
            } else if merge_mode == MergeMode::Htp {
                normalized > current.value.normalized().unwrap_or(0.0)
            } else {
                ltp_wins(
                    changed_at,
                    transition_ordinal,
                    current.changed_at,
                    current.transition_ordinal,
                )
            };
            if !wins {
                return;
            }
        } else {
            self.stamp[index] = self.epoch;
            self.touched.push(index as u32);
        }
        let winner = &mut self.winners[index];
        winner.priority = priority;
        winner.changed_at = changed_at;
        winner.merge_mode = merge_mode;
        winner.transition_ordinal = transition_ordinal;
        winner.sequence_master = None;
        build(winner);
    }

    /// Write a value into a slot regardless of what holds it, as a Freeze does when it takes the
    /// final say over an attribute.
    pub(crate) fn force(&mut self, slot: Slot, value: AttributeValue) {
        let index = slot.index();
        if index >= self.winners.len() {
            return;
        }
        if self.stamp[index] != self.epoch {
            self.stamp[index] = self.epoch;
            self.touched.push(index as u32);
        }
        let winner = &mut self.winners[index];
        winner.value = value;
        winner.sequence_master = None;
    }

    /// Every slot written this fill, in the order it was first written.
    pub(crate) fn occupied(&self) -> impl Iterator<Item = (Slot, &SlotWinner)> {
        self.touched
            .iter()
            .map(move |index| (Slot::from_index(*index as usize), &self.winners[*index as usize]))
    }

    /// How many slots this fill wrote.
    pub(crate) fn occupied_len(&self) -> usize {
        self.touched.len()
    }
}

fn ltp_wins(
    candidate_at: DateTime<Utc>,
    candidate_ordinal: Option<u64>,
    current_at: DateTime<Utc>,
    current_ordinal: Option<u64>,
) -> bool {
    candidate_at > current_at
        || (candidate_at == current_at
            && matches!(
                (candidate_ordinal, current_ordinal),
                (Some(candidate), Some(current)) if candidate > current
            ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> FrameState {
        FrameState::for_generation(1, 4)
    }

    fn offer(state: &mut FrameState, slot: usize, level: f32, priority: i16, at: i64) {
        let value = AttributeValue::Normalized(level);
        state.offer(
            Slot::from_index(slot),
            priority,
            DateTime::from_timestamp(at, 0).unwrap(),
            MergeMode::Ltp,
            None,
            level,
            |winner| winner.value = value,
        );
    }

    #[test]
    fn a_slot_nobody_contributed_to_reads_as_empty() {
        let mut state = state();
        state.begin();
        offer(&mut state, 1, 0.5, 0, 10);
        assert!(state.get(Slot::from_index(0)).is_none());
        assert!(state.get(Slot::from_index(1)).is_some());
    }

    #[test]
    fn last_frames_values_do_not_survive_into_this_one() {
        let mut state = state();
        state.begin();
        offer(&mut state, 2, 0.9, 0, 10);
        state.begin();
        assert!(state.get(Slot::from_index(2)).is_none());
        assert_eq!(state.occupied_len(), 0);
    }

    #[test]
    fn a_later_value_takes_an_ltp_slot_from_an_earlier_one() {
        let mut state = state();
        state.begin();
        offer(&mut state, 0, 0.2, 0, 10);
        offer(&mut state, 0, 0.8, 0, 20);
        assert_eq!(
            state.get(Slot::from_index(0)).unwrap().value,
            AttributeValue::Normalized(0.8)
        );
        offer(&mut state, 0, 0.4, 0, 15);
        assert_eq!(
            state.get(Slot::from_index(0)).unwrap().value,
            AttributeValue::Normalized(0.8)
        );
    }

    #[test]
    fn priority_outranks_recency() {
        let mut state = state();
        state.begin();
        offer(&mut state, 0, 0.2, 10, 10);
        offer(&mut state, 0, 0.8, 5, 20);
        assert_eq!(
            state.get(Slot::from_index(0)).unwrap().value,
            AttributeValue::Normalized(0.2)
        );
    }

    #[test]
    fn the_higher_level_takes_an_htp_slot() {
        let mut state = state();
        state.begin();
        let write = |state: &mut FrameState, level: f32, at: i64| {
            let value = AttributeValue::Normalized(level);
            state.offer(
                Slot::from_index(0),
                0,
                DateTime::from_timestamp(at, 0).unwrap(),
                MergeMode::Htp,
                None,
                level,
                |winner| winner.value = value,
            );
        };
        write(&mut state, 0.9, 10);
        write(&mut state, 0.3, 20);
        assert_eq!(
            state.get(Slot::from_index(0)).unwrap().value,
            AttributeValue::Normalized(0.9)
        );
    }

    #[test]
    fn a_freeze_takes_a_slot_from_whatever_held_it() {
        let mut state = state();
        state.begin();
        offer(&mut state, 3, 0.2, 100, 99);
        state.force(Slot::from_index(3), AttributeValue::Normalized(0.7));
        assert_eq!(
            state.get(Slot::from_index(3)).unwrap().value,
            AttributeValue::Normalized(0.7)
        );
        assert_eq!(state.occupied_len(), 1);
    }

    #[test]
    fn only_the_slots_this_frame_wrote_are_walked() {
        let mut state = state();
        state.begin();
        offer(&mut state, 3, 0.2, 0, 10);
        offer(&mut state, 1, 0.4, 0, 10);
        let walked = state.occupied().map(|(slot, _)| slot).collect::<Vec<_>>();
        assert_eq!(walked, vec![Slot::from_index(3), Slot::from_index(1)]);
    }
}
