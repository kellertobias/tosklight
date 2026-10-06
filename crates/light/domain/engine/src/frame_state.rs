//! One frame's worth of storage, addressed by slot and reused from one frame to the next.
//!
//! The arrays are sized once per generation, when the patch compiles, and then filled and refilled
//! for as long as that patch stands. Nothing here is cleared between frames: each slot carries the
//! epoch it was last written in, so a slot no source contributed to this frame reads as empty
//! without anyone having to walk the array to blank it.

use chrono::{DateTime, Utc};
use light_core::{AttributeValue, MergeMode};

use crate::Slot;

/// What a candidate offers a slot, apart from the value itself.
#[derive(Clone, Copy)]
pub(crate) struct Offer {
    pub(crate) priority: i16,
    pub(crate) changed_at: DateTime<Utc>,
    pub(crate) merge_mode: MergeMode,
    pub(crate) transition_ordinal: Option<u64>,
    /// The candidate's level, for the HTP comparison. Ignored under every other merge mode.
    pub(crate) normalized: f32,
}

/// The value holding a slot after arbitration.
#[derive(Clone)]
pub(crate) struct SlotWinner {
    pub(crate) value: AttributeValue,
    pub(crate) priority: i16,
    pub(crate) changed_at: DateTime<Utc>,
    /// A post-static projection has its own output timestamp, including explicit unknown.
    /// The original `changed_at` remains the static arbitration stamp.
    pub(crate) projected_changed_at: Option<Option<DateTime<Utc>>>,
    pub(crate) merge_mode: MergeMode,
    pub(crate) transition_ordinal: Option<u64>,
    pub(crate) origin: Option<std::sync::Arc<crate::contribution_batch::ContributionOrigin>>,
    pub(crate) family_evidence:
        Option<std::sync::Arc<crate::contribution_batch::ContributionFamilyEvidence>>,
    /// Runtime-only live Position crossing behind the held `value` (TL-544 G1).
    pub(crate) pending_transition:
        Option<std::sync::Arc<light_core::programming::PendingFamilyTransition>>,
    /// What the raw resolution held before the masters changed this level: `None` when no master
    /// touched it (`value` is raw), `Some(None)` for an unsourced level the masters filled at its
    /// default, `Some(Some(raw))` for a mastered contribution. Freeze and family projection write
    /// a raw value and clear it.
    pub(crate) pre_master: Option<Option<AttributeValue>>,
}

impl SlotWinner {
    /// The value before the output-parameter masters: the raw parameter, or the Freeze or family
    /// value that replaced it. `None` when only the masters' default fill holds the slot.
    pub(crate) fn raw_value(&self) -> Option<&AttributeValue> {
        match &self.pre_master {
            None => Some(&self.value),
            Some(raw) => raw.as_ref(),
        }
    }
}

impl Default for SlotWinner {
    fn default() -> Self {
        Self {
            value: AttributeValue::Normalized(0.0),
            priority: 0,
            changed_at: DateTime::<Utc>::MIN_UTC,
            projected_changed_at: None,
            merge_mode: MergeMode::Ltp,
            transition_ordinal: None,
            origin: None,
            family_evidence: None,
            pending_transition: None,
            pre_master: None,
        }
    }
}

impl SlotWinner {
    pub(crate) fn output_changed_at(&self) -> Option<DateTime<Utc>> {
        self.projected_changed_at.unwrap_or(Some(self.changed_at))
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

    /// Scale a level value this fill resolved, keeping who decided it.
    pub(crate) fn scale_level(&mut self, slot: Slot, factor: f32) {
        if !self.is_current(slot) {
            return;
        }
        if let Some(winner) = self.winners.get_mut(slot.index())
            && let Some(level) = winner.value.normalized()
        {
            winner.pre_master.get_or_insert(Some(winner.value.clone()));
            winner.value = AttributeValue::Normalized(level * factor);
        }
    }

    /// Hold a level nobody contributed at its mastered profile default.
    ///
    /// The value has no source: no origin, no family evidence, no transition and an unknown
    /// change time, so nothing reading the frame can mistake it for a programmed or played value.
    pub(crate) fn fill_unsourced_level(&mut self, slot: Slot, value: AttributeValue) {
        let index = slot.index();
        if index >= self.winners.len() || self.stamp[index] == self.epoch {
            return;
        }
        self.stamp[index] = self.epoch;
        self.touched.push(index as u32);
        let winner = &mut self.winners[index];
        winner.value = value;
        winner.priority = i16::MIN;
        winner.changed_at = DateTime::<Utc>::MIN_UTC;
        winner.projected_changed_at = Some(None);
        winner.merge_mode = MergeMode::Htp;
        winner.transition_ordinal = None;
        winner.origin = None;
        winner.family_evidence = None;
        winner.pending_transition = None;
        winner.pre_master = Some(None);
    }

    /// Update one already-resolved family after composition, without offering another LTP vote.
    /// Its baseline rank stays intact; the projection explicitly chooses source/master metadata.
    pub(crate) fn project_family(
        &mut self,
        slot: Slot,
        value: AttributeValue,
        metadata: crate::FamilyProjectionMetadata,
    ) -> bool {
        if !self.is_current(slot) {
            return false;
        }
        metadata.apply(&mut self.winners[slot.index()], value);
        true
    }

    /// Offer a value for a slot, keeping whichever of the two the merge rules prefer.
    ///
    /// `build` is only called when the candidate actually wins, so a losing contribution costs a
    /// comparison rather than a clone.
    pub(crate) fn offer(&mut self, slot: Slot, offer: Offer, build: impl FnOnce(&mut SlotWinner)) {
        if let Some(winner) = self.win(slot, offer) {
            winner.origin = None;
            winner.family_evidence = None;
            build(winner);
        }
    }

    /// [`Self::offer`] with a traced origin and family evidence (TL-639 round 7): a winning
    /// offer whose origin or evidence the slot already holds (from this fill or the last) keeps
    /// it, any other takes its own. Nothing is built or counted for a losing offer.
    pub(crate) fn offer_with_origin(
        &mut self,
        slot: Slot,
        offer: Offer,
        (origin, evidence): (
            crate::contribution::OfferedOrigin<'_>,
            Option<&std::sync::Arc<crate::ContributionFamilyEvidence>>,
        ),
        build: impl FnOnce(&mut SlotWinner),
    ) {
        if let Some(winner) = self.win(slot, offer) {
            winner.origin = origin.resolve(winner.origin.take());
            winner.family_evidence = match (winner.family_evidence.take(), evidence) {
                (Some(held), Some(offered)) if std::sync::Arc::ptr_eq(&held, offered) => Some(held),
                (_, offered) => offered.cloned(),
            };
            build(winner);
        }
    }

    /// Arbitrate `offer` against the slot's holder; when it wins, stamp the slot and return its
    /// winner with every field but the value, the origin and the evidence reset for the
    /// candidate.
    fn win(&mut self, slot: Slot, offer: Offer) -> Option<&mut SlotWinner> {
        let index = slot.index();
        if index >= self.winners.len() {
            return None;
        }
        if self.stamp[index] == self.epoch {
            let current = &self.winners[index];
            let wins = if offer.priority != current.priority {
                offer.priority > current.priority
            } else if offer.merge_mode == MergeMode::Htp {
                offer.normalized > current.value.normalized().unwrap_or(0.0)
            } else {
                ltp_wins(
                    offer.changed_at,
                    offer.transition_ordinal,
                    current.changed_at,
                    current.transition_ordinal,
                )
            };
            if !wins {
                return None;
            }
        } else {
            self.stamp[index] = self.epoch;
            self.touched.push(index as u32);
        }
        let winner = &mut self.winners[index];
        winner.priority = offer.priority;
        winner.changed_at = offer.changed_at;
        winner.projected_changed_at = None;
        winner.merge_mode = offer.merge_mode;
        winner.transition_ordinal = offer.transition_ordinal;
        winner.pending_transition = None;
        winner.pre_master = None;
        Some(winner)
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
        winner.origin = None;
        winner.family_evidence = None;
        winner.pending_transition = None;
        winner.pre_master = None;
    }

    /// Take a slot over, optionally restamping when its value changed.
    pub(crate) fn force_at(
        &mut self,
        slot: Slot,
        value: AttributeValue,
        changed_at: Option<DateTime<Utc>>,
    ) {
        self.force(slot, value);
        if let (Some(changed_at), Some(index)) = (changed_at, Some(slot.index()))
            && index < self.winners.len()
        {
            self.winners[index].changed_at = changed_at;
            self.winners[index].projected_changed_at = None;
        }
    }

    /// Every slot written this fill, in the order it was first written.
    pub(crate) fn occupied(&self) -> impl Iterator<Item = (Slot, &SlotWinner)> {
        self.touched.iter().map(move |index| {
            (
                Slot::from_index(*index as usize),
                &self.winners[*index as usize],
            )
        })
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
            Offer {
                priority,
                changed_at: DateTime::from_timestamp(at, 0).unwrap(),
                merge_mode: MergeMode::Ltp,
                transition_ordinal: None,
                normalized: level,
            },
            |winner| winner.value = value,
        );
    }

    /// TL-639 round 7: a traced offer keeps the origin and evidence its slot already holds when
    /// they describe the same contribution, across fills and within one; a losing offer builds
    /// nothing; anything else holds exactly what an eager build would have.
    #[test]
    fn traced_offers_keep_an_equal_origin_and_evidence_and_build_only_for_a_winner() {
        use crate::contribution::OfferedOrigin;
        use crate::contribution_batch::ContributionSourceId;
        use std::borrow::Cow;
        let source = ContributionSourceId::programmer_transient(
            light_core::ProgrammerId(uuid::Uuid::from_u128(1)),
            "a",
        );
        let evidence = std::sync::Arc::new(crate::ContributionFamilyEvidence::new(Vec::new()));
        let timed = |at: i64| light_core::TimedValue {
            fixture_id: light_core::FixtureId(uuid::Uuid::from_u128(2)),
            attribute: light_core::AttributeKey("pan".into()),
            value: AttributeValue::Normalized(0.5),
            priority: 0,
            changed_at: DateTime::from_timestamp(at, 0).unwrap(),
            programmer_order: 0,
            merge_mode: MergeMode::Ltp,
            fade: false,
            fade_millis: None,
            delay_millis: None,
        };
        let traced = |state: &mut FrameState, value: &light_core::TimedValue| {
            state.offer_with_origin(
                Slot::from_index(0),
                Offer {
                    priority: 0,
                    changed_at: value.changed_at,
                    merge_mode: MergeMode::Ltp,
                    transition_ordinal: None,
                    normalized: 0.5,
                },
                (
                    OfferedOrigin::Built {
                        source: Cow::Borrowed(&source),
                        value,
                        transition_ordinal: None,
                    },
                    Some(&evidence),
                ),
                |winner| winner.value = value.value.clone(),
            )
        };
        let held = |state: &FrameState| {
            let winner = state.get(Slot::from_index(0)).unwrap();
            (
                winner.origin.clone().unwrap(),
                winner.family_evidence.clone().unwrap(),
            )
        };
        let mut state = state();
        state.begin();
        traced(&mut state, &timed(10));
        let (first, first_evidence) = held(&state);
        assert!(std::sync::Arc::ptr_eq(&first_evidence, &evidence));
        // An earlier (losing) offer changes nothing; the next fill keeps both allocations.
        traced(&mut state, &timed(5));
        assert!(std::sync::Arc::ptr_eq(&held(&state).0, &first));
        state.begin();
        traced(&mut state, &timed(10));
        assert!(std::sync::Arc::ptr_eq(&held(&state).0, &first));
        // A later edit wins with its own origin, equal to an eager build.
        let later = timed(20);
        traced(&mut state, &later);
        let (origin, _) = held(&state);
        assert!(!std::sync::Arc::ptr_eq(&origin, &first));
        assert!(origin.describes(&source, &later, None));
        // A plain offer clears both, as before.
        offer(&mut state, 0, 0.9, 0, 30);
        let winner = state.get(Slot::from_index(0)).unwrap();
        assert!(winner.origin.is_none() && winner.family_evidence.is_none());
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
                Offer {
                    priority: 0,
                    changed_at: DateTime::from_timestamp(at, 0).unwrap(),
                    merge_mode: MergeMode::Htp,
                    transition_ordinal: None,
                    normalized: level,
                },
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
