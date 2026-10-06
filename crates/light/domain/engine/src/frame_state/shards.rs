//! One fill's slots cut into contiguous ranges, offered to on several threads (TL-639 round 7).
//!
//! Arbitration of a slot reads and writes only that slot, so a phase of offers whose slots are
//! known can be split by slot range: each range takes its offers in their original order, and
//! every slot sees exactly the sequence of offers the single-threaded fill would give it. The
//! one cross-slot state is the fill's first-write order ([`FrameState::occupied`]); each range
//! records its first writes with the offer's position in the phase, and the merge appends them
//! in that order, which is the order the single-threaded fill would have recorded.
use super::*;

/// One contiguous range of a fill's slots.
pub(crate) struct FrameShard<'a> {
    start: usize,
    epoch: u32,
    stamp: &'a mut [u32],
    winners: &'a mut [SlotWinner],
    /// This range's first writes in the phase: (offer position, slot index).
    touched: Vec<(u32, u32)>,
}

/// How a fill is cut: ranges of `size` slots.
#[derive(Clone, Copy)]
pub(crate) struct ShardPlan {
    size: usize,
    count: usize,
}

impl ShardPlan {
    /// The range holding `slot`.
    pub(crate) fn shard_of(&self, slot: Slot) -> usize {
        (slot.index() / self.size).min(self.count - 1)
    }

    pub(crate) fn count(&self) -> usize {
        self.count
    }
}

impl FrameState {
    /// How this fill's slots are cut into (at most) `count` ranges.
    pub(crate) fn shard_plan(&self, count: usize) -> ShardPlan {
        let count = count.clamp(1, self.winners.len().max(1));
        ShardPlan {
            size: self.winners.len().div_ceil(count).max(1),
            count,
        }
    }

    /// The ranges of `plan`, in slot order, for one phase; each records its first writes into
    /// one of `touched` (emptied, kept for their room).
    pub(crate) fn shards(
        &mut self,
        plan: ShardPlan,
        touched: &mut Vec<Vec<(u32, u32)>>,
    ) -> Vec<FrameShard<'_>> {
        touched.resize_with(plan.count, Vec::new);
        let epoch = self.epoch;
        let mut shards = Vec::with_capacity(plan.count);
        let mut stamp = self.stamp.as_mut_slice();
        let mut winners = self.winners.as_mut_slice();
        let mut start = 0;
        for _ in 0..plan.count {
            let size = plan.size.min(winners.len());
            let (range_stamp, rest_stamp) = stamp.split_at_mut(size);
            let (range_winners, rest_winners) = winners.split_at_mut(size);
            shards.push(FrameShard {
                start,
                epoch,
                stamp: range_stamp,
                winners: range_winners,
                touched: std::mem::take(&mut touched[shards.len()]),
            });
            (stamp, winners) = (rest_stamp, rest_winners);
            start += size;
        }
        shards
    }

    /// Append the first writes of one phase of `offers` offers, recorded by its ranges, in
    /// offer order (`by_position` is room for the merge). Each offer writes at most one slot
    /// first, so the positions are distinct.
    pub(crate) fn merge_touched(
        &mut self,
        offers: usize,
        touched: &[Vec<(u32, u32)>],
        by_position: &mut Vec<u32>,
    ) {
        const NONE: u32 = u32::MAX;
        by_position.clear();
        by_position.resize(offers, NONE);
        for &(position, index) in touched.iter().flatten() {
            by_position[position as usize] = index;
        }
        self.touched
            .extend(by_position.iter().copied().filter(|index| *index != NONE));
    }
}

impl FrameShard<'_> {
    /// [`FrameState::offer_with_origin`] for a slot of this range, the phase's `position`-th
    /// offer.
    pub(crate) fn offer_with_origin(
        &mut self,
        position: u32,
        slot: Slot,
        offer: Offer,
        traced: (
            crate::contribution::OfferedOrigin<'_>,
            Option<&std::sync::Arc<crate::ContributionFamilyEvidence>>,
        ),
        build: impl FnOnce(&mut SlotWinner),
    ) {
        let index = slot.index();
        let Some(local) = index
            .checked_sub(self.start)
            .filter(|local| *local < self.winners.len())
        else {
            unreachable!("a shard is offered only its own slots");
        };
        let winner = &mut self.winners[local];
        let Some(first) = arbitrate(self.epoch, &mut self.stamp[local], winner, offer) else {
            return;
        };
        if first {
            self.touched.push((position, index as u32));
        }
        take_traced(winner, traced);
        build(winner);
    }

    /// This range's first writes, for [`FrameState::merge_touched`].
    pub(crate) fn into_touched(self) -> Vec<(u32, u32)> {
        self.touched
    }
}
