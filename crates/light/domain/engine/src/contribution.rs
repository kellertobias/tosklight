use chrono::{DateTime, Utc};
use light_core::{AttributeKey, AttributeValue, FixtureId, MergeMode, TimedValue};
use light_playback::{AutomaticPlaybackTransition, PlaybackContribution};
use rustc_hash::FxHashMap;
use std::collections::{HashMap, hash_map::Entry};

pub(crate) fn value_for_ordered_position(
    value: &AttributeValue,
    index: usize,
    count: usize,
) -> AttributeValue {
    let AttributeValue::Spread(points) = value else {
        return value.clone();
    };
    if points.is_empty() {
        return AttributeValue::Normalized(0.0);
    }
    // Shared deterministic anchor rule — every surface resolves stored spreads identically.
    AttributeValue::Normalized(light_core::spread_position(points, index, count))
}

pub(crate) type ApplicableSequenceMaster = crate::ContributionSequenceMaster;

pub(crate) struct EngineContribution {
    value: TimedValue,
    transition_ordinal: Option<u64>,
    sequence_master: Option<ApplicableSequenceMaster>,
}

/// Borrowed arbitration result for intermediate lookups during one render.
///
/// The index owns neither addresses nor values, so resolving the playback underlay and optional
/// Move-in-Black base does not clone every contribution before final arbitration.
pub(crate) struct ResolvedContributionIndex<'a> {
    winners: HashMap<(FixtureId, &'a AttributeKey), IndexedContribution<'a>>,
}

#[derive(Clone, Copy)]
enum IndexedContribution<'a> {
    Engine(&'a EngineContribution),
    Sample(&'a crate::ContributionSample),
}

impl<'a> IndexedContribution<'a> {
    fn value(self) -> &'a TimedValue {
        match self {
            Self::Engine(contribution) => &contribution.value,
            Self::Sample(sample) => sample.value(),
        }
    }

    fn transition_ordinal(self) -> Option<u64> {
        match self {
            Self::Engine(contribution) => contribution.transition_ordinal,
            Self::Sample(sample) => sample.transition_ordinal(),
        }
    }
}

impl<'a> ResolvedContributionIndex<'a> {
    pub(crate) fn new(values: &'a [EngineContribution]) -> Self {
        let mut index = Self {
            winners: HashMap::with_capacity(values.len()),
        };
        for candidate in values {
            index.add(IndexedContribution::Engine(candidate));
        }
        index
    }

    pub(crate) fn extend_sampled(
        &mut self,
        samples: impl IntoIterator<Item = &'a crate::ContributionSample>,
    ) {
        for sample in samples {
            self.add(IndexedContribution::Sample(sample));
        }
    }

    pub(crate) fn value(
        &self,
        fixture_id: FixtureId,
        attribute: &AttributeKey,
    ) -> Option<&AttributeValue> {
        self.winners
            .get(&(fixture_id, attribute))
            .map(|winner| &winner.value().value)
    }

    fn add(&mut self, candidate: IndexedContribution<'a>) {
        let value = candidate.value();
        let key = (value.fixture_id, &value.attribute);
        let replace = self.winners.get(&key).is_none_or(|current| {
            contribution_wins(
                value,
                candidate.transition_ordinal(),
                current.value(),
                current.transition_ordinal(),
            )
        });
        if replace {
            self.winners.insert(key, candidate);
        }
    }
}

impl EngineContribution {
    pub(crate) fn unscaled(value: TimedValue) -> Self {
        Self {
            value,
            transition_ordinal: None,
            sequence_master: None,
        }
    }

    pub(crate) fn from_playback(contribution: PlaybackContribution) -> Self {
        Self {
            value: contribution.value,
            transition_ordinal: Some(contribution.transition_ordinal),
            sequence_master: Some(ApplicableSequenceMaster::new(
                contribution.source,
                contribution.sequence_master,
            )),
        }
    }

    pub(crate) fn fixture_id(&self) -> FixtureId {
        self.value.fixture_id
    }

    pub(crate) fn attribute(&self) -> &AttributeKey {
        &self.value.attribute
    }
}

#[derive(Default)]
pub(crate) struct ResolvedAttributes {
    pub(crate) values: ResolvedValues,
    pub(crate) changed_at: ResolvedChangedAt,
    pub(crate) sequence_masters: FxHashMap<(FixtureId, AttributeKey), ApplicableSequenceMaster>,
    pub(crate) automatic_playback_transitions: Vec<AutomaticPlaybackTransition>,
}

/// Arbitrates one frame's contributions into slot-addressed storage.
///
/// The storage is borrowed from the generation's pool and handed back when the frame is finished
/// with, so the merge itself allocates nothing. A pair the compiled patch cannot produce has no
/// slot; rather than lose an operator's value, those few land in an overflow map. In a show whose
/// sources all name attributes their fixtures declare, that map stays empty and is never touched.
pub(crate) struct EngineContributionResolver<'a> {
    slots: &'a crate::SlotTable,
    pool: Option<&'a crate::FramePool>,
    frame: crate::FrameState,
    overflow: FxHashMap<(FixtureId, AttributeKey), EngineWinner>,
}

impl<'a> EngineContributionResolver<'a> {
    /// Storage for one frame of this generation's shape.
    pub(crate) fn for_generation(
        slots: &'a crate::SlotTable,
        pool: &'a crate::FramePool,
    ) -> Self {
        // A pool that has nothing left is a stalled consumer, not a reason to stall the desk: this
        // frame gets its own storage and simply is not the one that gets reused.
        let frame = pool.take().unwrap_or_else(|| {
            let mut state = crate::FrameState::for_generation(slots.generation(), slots.len());
            state.begin();
            state
        });
        Self {
            slots,
            pool: Some(pool),
            frame,
            overflow: FxHashMap::default(),
        }
    }

    /// Storage for one frame without a pool behind it, for callers that resolve once rather than
    /// every tick.
    pub(crate) fn unpooled(slots: &'a crate::SlotTable) -> Self {
        let mut frame = crate::FrameState::for_generation(slots.generation(), slots.len());
        frame.begin();
        Self {
            slots,
            pool: None,
            frame,
            overflow: FxHashMap::default(),
        }
    }

    pub(crate) fn extend(&mut self, values: impl IntoIterator<Item = EngineContribution>) {
        for value in values {
            self.add(value);
        }
    }

    pub(crate) fn add_playback_unscaled(&mut self, value: TimedValue, transition_ordinal: u64) {
        self.add(EngineContribution {
            value,
            transition_ordinal: Some(transition_ordinal),
            sequence_master: None,
        });
    }

    pub(crate) fn extend_borrowed_samples<'s>(
        &mut self,
        samples: impl IntoIterator<Item = &'s crate::ContributionSample>,
    ) {
        for sample in samples {
            let value = sample.value();
            self.add_borrowed(
                value.fixture_id,
                &value.attribute,
                &value.value,
                value.priority,
                value.changed_at,
                value.merge_mode,
                sample.transition_ordinal(),
                sample.sequence_master(),
            );
        }
    }

    pub(crate) fn add_borrowed_unscaled(
        &mut self,
        fixture_id: FixtureId,
        attribute: &AttributeKey,
        value: &AttributeValue,
        priority: i16,
        changed_at: DateTime<Utc>,
        merge_mode: MergeMode,
    ) {
        self.add_borrowed(
            fixture_id, attribute, value, priority, changed_at, merge_mode, None, None,
        );
    }

    /// The number for an attribute name, so a caller applying one attribute across many fixtures
    /// hashes the name once instead of once per fixture.
    pub(crate) fn attribute_id(&self, attribute: &AttributeKey) -> Option<light_core::AttributeId> {
        self.slots.attribute_id(attribute)
    }

    /// Offer a value for a pair already reduced to numbers.
    ///
    /// A pair this fixture cannot produce is dropped here rather than stored and ignored later.
    /// Group programming reaches the resolver this way: a Group that programs an attribute one of
    /// its members does not have leaves that member alone.
    pub(crate) fn add_numbered_unscaled(
        &mut self,
        fixture_id: FixtureId,
        attribute: light_core::AttributeId,
        value: &AttributeValue,
        priority: i16,
        changed_at: DateTime<Utc>,
        merge_mode: MergeMode,
    ) {
        let Some(slot) = self.slots.slot_of(fixture_id, attribute) else {
            return;
        };
        self.frame.offer(
            slot,
            priority,
            changed_at,
            merge_mode,
            None,
            value.normalized().unwrap_or(0.0),
            |winner| winner.value = value.clone(),
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn add_borrowed(
        &mut self,
        fixture_id: FixtureId,
        attribute: &AttributeKey,
        value: &AttributeValue,
        priority: i16,
        changed_at: DateTime<Utc>,
        merge_mode: MergeMode,
        transition_ordinal: Option<u64>,
        sequence_master: Option<ApplicableSequenceMaster>,
    ) {
        match self.slots.slot(fixture_id, attribute) {
            Some(slot) => self.frame.offer(
                slot,
                priority,
                changed_at,
                merge_mode,
                transition_ordinal,
                value.normalized().unwrap_or(0.0),
                |winner| {
                    winner.value = value.clone();
                    winner.sequence_master = sequence_master;
                },
            ),
            None => self.offer_overflow(
                fixture_id,
                attribute,
                EngineWinner {
                    value: value.clone(),
                    priority,
                    changed_at,
                    merge_mode,
                    transition_ordinal,
                    sequence_master,
                },
            ),
        }
    }

    fn add(&mut self, candidate: EngineContribution) {
        let EngineContribution {
            value,
            transition_ordinal,
            sequence_master,
        } = candidate;
        let TimedValue {
            fixture_id,
            attribute,
            value,
            priority,
            changed_at,
            merge_mode,
            ..
        } = value;
        match self.slots.slot(fixture_id, &attribute) {
            Some(slot) => {
                let level = value.normalized().unwrap_or(0.0);
                let mut carried = Some(value);
                self.frame.offer(
                    slot,
                    priority,
                    changed_at,
                    merge_mode,
                    transition_ordinal,
                    level,
                    |winner| {
                        if let Some(value) = carried.take() {
                            winner.value = value;
                        }
                        winner.sequence_master = sequence_master;
                    },
                );
            }
            None => self.offer_overflow(
                fixture_id,
                &attribute,
                EngineWinner {
                    value,
                    priority,
                    changed_at,
                    merge_mode,
                    transition_ordinal,
                    sequence_master,
                },
            ),
        }
    }

    fn offer_overflow(
        &mut self,
        fixture_id: FixtureId,
        attribute: &AttributeKey,
        candidate: EngineWinner,
    ) {
        match self.overflow.entry((fixture_id, attribute.clone())) {
            Entry::Vacant(entry) => {
                entry.insert(candidate);
            }
            Entry::Occupied(mut entry) => {
                if winner_wins(&candidate, entry.get()) {
                    entry.insert(candidate);
                }
            }
        }
    }

    /// Force a value into a slot whatever holds it, as a Freeze does when it takes the final say.
    pub(crate) fn freeze(
        &mut self,
        fixture_id: FixtureId,
        attribute: &AttributeKey,
        value: &AttributeValue,
    ) {
        match self.slots.slot(fixture_id, attribute) {
            Some(slot) => self.frame.force(slot, value.clone()),
            None => {
                if let Some(winner) = self.overflow.get_mut(&(fixture_id, attribute.clone())) {
                    winner.value = value.clone();
                    winner.sequence_master = None;
                }
            }
        }
    }

    /// The values resolved so far, by name. Built only for the Move-in-Black base, which is asked
    /// for solely when a Cue actually has candidates to move in the dark.
    pub(crate) fn values(&self) -> crate::ResolvedValues {
        let mut values = crate::ResolvedValues::with_capacity_and_hasher(
            self.frame.occupied_len() + self.overflow.len(),
            Default::default(),
        );
        for (slot, winner) in self.frame.occupied() {
            let (fixture_id, attribute) = self.slots.pair(slot);
            values.insert((fixture_id, attribute.clone()), winner.value.clone());
        }
        for ((fixture_id, attribute), winner) in &self.overflow {
            values.insert((*fixture_id, attribute.clone()), winner.value.clone());
        }
        values
    }

    /// Hand the frame to the boundary that still speaks in names, and the storage back to the pool.
    pub(crate) fn finish(mut self) -> ResolvedAttributes {
        let winner_count = self.frame.occupied_len() + self.overflow.len();
        let mut resolved = ResolvedAttributes {
            values: ResolvedValues::with_capacity_and_hasher(winner_count, Default::default()),
            changed_at: ResolvedChangedAt::with_capacity_and_hasher(
                winner_count,
                Default::default(),
            ),
            ..ResolvedAttributes::default()
        };
        for (slot, winner) in self.frame.occupied() {
            let (fixture_id, attribute) = self.slots.pair(slot);
            let key = (fixture_id, attribute.clone());
            resolved.changed_at.insert(key.clone(), winner.changed_at);
            if let Some(sequence_master) = winner.sequence_master {
                resolved
                    .sequence_masters
                    .insert(key.clone(), sequence_master);
            }
            resolved.values.insert(key, winner.value.clone());
        }
        for ((fixture_id, attribute), winner) in std::mem::take(&mut self.overflow) {
            let key = (fixture_id, attribute);
            resolved.changed_at.insert(key.clone(), winner.changed_at);
            if let Some(sequence_master) = winner.sequence_master {
                resolved
                    .sequence_masters
                    .insert(key.clone(), sequence_master);
            }
            resolved.values.insert(key, winner.value);
        }
        if let Some(pool) = self.pool {
            pool.give_back(self.frame);
        }
        resolved
    }
}

type EngineWinner = crate::SlotWinner;

fn contribution_wins(
    candidate: &TimedValue,
    candidate_ordinal: Option<u64>,
    current: &TimedValue,
    current_ordinal: Option<u64>,
) -> bool {
    if candidate.priority != current.priority {
        candidate.priority > current.priority
    } else if candidate.merge_mode == MergeMode::Htp {
        candidate.value.normalized().unwrap_or(0.0) > current.value.normalized().unwrap_or(0.0)
    } else {
        ltp_wins(
            candidate.changed_at,
            candidate_ordinal,
            current.changed_at,
            current_ordinal,
        )
    }
}

fn winner_wins(candidate: &EngineWinner, current: &EngineWinner) -> bool {
    if candidate.priority != current.priority {
        candidate.priority > current.priority
    } else if candidate.merge_mode == MergeMode::Htp {
        candidate.value.normalized().unwrap_or(0.0) > current.value.normalized().unwrap_or(0.0)
    } else {
        ltp_wins(
            candidate.changed_at,
            candidate.transition_ordinal,
            current.changed_at,
            current.transition_ordinal,
        )
    }
}

fn borrowed_winner_wins(
    value: &AttributeValue,
    priority: i16,
    changed_at: DateTime<Utc>,
    merge_mode: MergeMode,
    transition_ordinal: Option<u64>,
    current: &EngineWinner,
) -> bool {
    if priority != current.priority {
        priority > current.priority
    } else if merge_mode == MergeMode::Htp {
        value.normalized().unwrap_or(0.0) > current.value.normalized().unwrap_or(0.0)
    } else {
        ltp_wins(
            changed_at,
            transition_ordinal,
            current.changed_at,
            current.transition_ordinal,
        )
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
mod transition_order_tests {
    use super::*;
    use light_core::CueListId;
    use light_playback::SequenceMasterSource;

    /// A show of one fixture that declares the attributes these tests arbitrate.
    fn resolved(
        fixture_id: FixtureId,
        attributes: &[&str],
        contributions: impl IntoIterator<Item = EngineContribution>,
    ) -> ResolvedAttributes {
        let fixture = crate::frame_slots::legacy_test_fixture(fixture_id, attributes);
        let slots = crate::SlotTable::compile(1, std::slice::from_ref(&fixture));
        let mut resolver = EngineContributionResolver::unpooled(&slots);
        resolver.extend(contributions);
        resolver.finish()
    }

    fn playback_value(
        fixture_id: FixtureId,
        value: f32,
        merge_mode: MergeMode,
        changed_at: DateTime<Utc>,
        transition_ordinal: u64,
    ) -> EngineContribution {
        EngineContribution::from_playback(PlaybackContribution {
            value: TimedValue {
                fixture_id,
                attribute: AttributeKey::intensity(),
                value: AttributeValue::Normalized(value),
                priority: 10,
                changed_at,
                programmer_order: 0,
                merge_mode,
                fade: false,
                fade_millis: None,
                delay_millis: None,
            },
            transition_ordinal,
            sequence_master: 1.0,
            source: SequenceMasterSource {
                playback_number: None,
                playback_identity: None,
                cue_list_id: CueListId::new(),
                temporary: false,
            },
        })
    }

    #[test]
    fn equal_timestamp_playback_ltp_uses_transition_order() {
        let fixture_id = FixtureId::new();
        let at = Utc::now();
        let resolved = resolved(
            fixture_id,
            &["intensity"],
            [
                playback_value(fixture_id, 0.8, MergeMode::Ltp, at, 4),
                playback_value(fixture_id, 0.2, MergeMode::Ltp, at, 5),
            ],
        );
        assert_eq!(
            resolved.values[&(fixture_id, AttributeKey::intensity())],
            AttributeValue::Normalized(0.2)
        );
    }

    #[test]
    fn equal_timestamp_playback_htp_ignores_transition_order() {
        let fixture_id = FixtureId::new();
        let at = Utc::now();
        let resolved = resolved(
            fixture_id,
            &["intensity"],
            [
                playback_value(fixture_id, 0.8, MergeMode::Htp, at, 4),
                playback_value(fixture_id, 0.2, MergeMode::Htp, at, 5),
            ],
        );
        assert_eq!(
            resolved.values[&(fixture_id, AttributeKey::intensity())],
            AttributeValue::Normalized(0.8)
        );
    }

    #[test]
    fn equal_timestamp_non_playback_ltp_does_not_use_playback_order() {
        let fixture_id = FixtureId::new();
        let at = Utc::now();
        let value = |normalized| {
            EngineContribution::unscaled(TimedValue {
                fixture_id,
                attribute: AttributeKey("pan".into()),
                value: AttributeValue::Normalized(normalized),
                priority: 10,
                changed_at: at,
                programmer_order: 0,
                merge_mode: MergeMode::Ltp,
                fade: false,
                fade_millis: None,
                delay_millis: None,
            })
        };
        let resolved = resolved(fixture_id, &["pan"], [value(0.8), value(0.2)]);
        assert_eq!(
            resolved.values[&(fixture_id, AttributeKey("pan".into()))],
            AttributeValue::Normalized(0.8)
        );
    }
}

/// The show's resolved values for one frame. A frame writes and reads these tens of thousands of
/// times, and the keys are already unique, so they are hashed for speed rather than against an
/// adversary.
pub type ResolvedValues = FxHashMap<(FixtureId, AttributeKey), AttributeValue>;
pub type ResolvedChangedAt = FxHashMap<(FixtureId, AttributeKey), DateTime<Utc>>;
