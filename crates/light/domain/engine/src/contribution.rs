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
    /// Everything the frame resolved, by name — populated only when there is no dense frame to
    /// read instead. A frame that holds the whole show leaves these empty and is materialised at
    /// the boundary, once, if anyone asks.
    pub(crate) values: ResolvedValues,
    pub(crate) changed_at: ResolvedChangedAt,
    pub(crate) sequence_masters: FxHashMap<(FixtureId, AttributeKey), ApplicableSequenceMaster>,
    pub(crate) automatic_playback_transitions: Vec<AutomaticPlaybackTransition>,
    /// The frame these values were resolved into, kept so the render can read by slot rather than
    /// by name. Absent for callers that assemble a projection from maps they were handed.
    pub(crate) frame: Option<ResolvedFrame>,
}

impl ResolvedAttributes {
    /// This frame's values as the boundary sees them, by name and on demand.
    ///
    /// Takes the frame with it, so the pooled buffer lives exactly as long as something can still
    /// read the values it holds.
    pub(crate) fn named_values(&mut self) -> crate::FrameValues {
        match self.frame.take() {
            Some(frame) => crate::FrameValues::from_frame(frame),
            None => crate::FrameValues::from_maps(
                std::mem::take(&mut self.values),
                std::mem::take(&mut self.changed_at),
            ),
        }
    }

    /// Take an attribute over after arbitration, as a Freeze and a Group colour do.
    ///
    /// Writes the frame and the maps together. Anything that changed only one of them would leave
    /// projection reading a different value than the boundary reports.
    pub(crate) fn override_value(
        &mut self,
        fixture_id: FixtureId,
        attribute: &AttributeKey,
        value: AttributeValue,
        changed_at: Option<DateTime<Utc>>,
    ) {
        // The holder of an attribute after an override is the override, so an underlying Cue
        // master must not go on scaling what it no longer decides. `force` clears it on the dense
        // path; the map path removes it here.
        if let Some(frame) = self.frame.as_mut() {
            if let Some(slot) = frame.slots().slot(fixture_id, attribute) {
                frame.force_at(slot, value, changed_at);
            }
            return;
        }
        let key = (fixture_id, attribute.clone());
        if let Some(changed_at) = changed_at {
            self.changed_at.insert(key.clone(), changed_at);
        }
        self.sequence_masters.remove(&key);
        self.values.insert(key, value);
    }
}

/// One frame's dense storage together with the numbering that addresses it.
///
/// Owns the borrowed buffer for as long as anything reads the frame, and hands it back to its
/// pool when dropped. Reclaiming by hand would be one forgotten call away from a desk that
/// allocates a fresh frame every tick.
pub(crate) struct ResolvedFrame {
    slots: std::sync::Arc<crate::SlotTable>,
    state: Option<crate::FrameState>,
    pool: Option<std::sync::Arc<crate::FramePool>>,
    /// Values for pairs the compiled patch could not number, grouped by the fixture that owns
    /// them. Empty for a show whose sources only name attributes their fixtures declare, which is
    /// every show that has not been sent something unexpected.
    ///
    /// Kept beside the frame rather than instead of it: one unrecognised name from a hardware
    /// surface or an HTTP client should cost the desk that one value's lookup, not the whole
    /// frame's dense reading.
    overflow: FxHashMap<FixtureId, Vec<(AttributeKey, EngineWinner)>>,
}

impl Drop for ResolvedFrame {
    fn drop(&mut self) {
        if let (Some(pool), Some(state)) = (self.pool.take(), self.state.take()) {
            pool.give_back(state);
        }
    }
}

impl ResolvedFrame {
    /// Values this frame could not number, for one fixture.
    pub(crate) fn overflow(&self, fixture_id: FixtureId) -> &[(AttributeKey, EngineWinner)] {
        // Checked for emptiness before hashing: a show whose sources name attributes their
        // fixtures declare asks this once per head per frame and the answer is always nothing.
        if self.overflow.is_empty() {
            return &[];
        }
        self.overflow
            .get(&fixture_id)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    /// Whether anything this frame resolved could not be numbered.
    pub(crate) fn has_overflow(&self) -> bool {
        !self.overflow.is_empty()
    }

    /// Every unnumbered value, with the fixture that owns it.
    pub(crate) fn overflowed(
        &self,
    ) -> impl Iterator<Item = (FixtureId, &AttributeKey, &EngineWinner)> {
        self.overflow.iter().flat_map(|(fixture_id, values)| {
            values
                .iter()
                .map(move |(attribute, winner)| (*fixture_id, attribute, winner))
        })
    }

    /// The value holding a slot, or nothing when nothing contributed to it this frame.
    pub(crate) fn value(&self, slot: crate::Slot) -> Option<&AttributeValue> {
        self.state.as_ref()?.get(slot).map(|winner| &winner.value)
    }

    /// The sequence master scaling a slot, if the winning source carried one.
    /// When the value holding a slot last changed.
    pub(crate) fn changed_at(&self, slot: crate::Slot) -> Option<DateTime<Utc>> {
        Some(self.state.as_ref()?.get(slot)?.changed_at)
    }

    /// Every slot this frame wrote, with the value that won it.
    pub(crate) fn occupied(&self) -> impl Iterator<Item = (crate::Slot, &crate::SlotWinner)> {
        self.state.iter().flat_map(|state| state.occupied())
    }

    /// How many slots this frame wrote.
    pub(crate) fn occupied_len(&self) -> usize {
        self.state
            .as_ref()
            .map_or(0, crate::FrameState::occupied_len)
    }

    pub(crate) fn sequence_master(&self, slot: crate::Slot) -> Option<ApplicableSequenceMaster> {
        self.state
            .as_ref()?
            .get(slot)
            .and_then(|winner| winner.sequence_master)
    }

    pub(crate) fn slots(&self) -> &crate::SlotTable {
        &self.slots
    }

    /// Write a value into a slot whatever holds it, as a Freeze does.
    /// Take a slot over, optionally restamping when it changed.
    pub(crate) fn force_at(
        &mut self,
        slot: crate::Slot,
        value: AttributeValue,
        changed_at: Option<DateTime<Utc>>,
    ) {
        if let Some(state) = self.state.as_mut() {
            state.force_at(slot, value, changed_at);
        }
    }
}

/// Arbitrates one frame's contributions into slot-addressed storage.
///
/// The storage is borrowed from the generation's pool and handed back when the frame is finished
/// with, so the merge itself allocates nothing. A pair the compiled patch cannot produce has no
/// slot; rather than lose an operator's value, those few land in an overflow map. In a show whose
/// sources all name attributes their fixtures declare, that map stays empty and is never touched.
pub(crate) struct EngineContributionResolver<'a> {
    slots: &'a std::sync::Arc<crate::SlotTable>,
    pool: Option<std::sync::Arc<crate::FramePool>>,
    frame: crate::FrameState,
    overflow: FxHashMap<(FixtureId, AttributeKey), EngineWinner>,
}

impl<'a> EngineContributionResolver<'a> {
    /// Storage for one frame of this generation's shape.
    pub(crate) fn for_generation(
        slots: &'a std::sync::Arc<crate::SlotTable>,
        pool: &'a std::sync::Arc<crate::FramePool>,
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
            pool: Some(std::sync::Arc::clone(pool)),
            frame,
            overflow: FxHashMap::default(),
        }
    }

    /// Storage for one frame without a pool behind it, for callers that resolve once rather than
    /// every tick.
    #[cfg(test)]
    pub(crate) fn unpooled(slots: &'a std::sync::Arc<crate::SlotTable>) -> Self {
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
                sample.address(),
            );
        }
    }

    #[cfg(test)]
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
            fixture_id, attribute, value, priority, changed_at, merge_mode, None, None, None,
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
            crate::Offer {
                priority,
                changed_at,
                merge_mode,
                transition_ordinal: None,
                normalized: value.normalized().unwrap_or(0.0),
            },
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
        address: Option<light_core::FrameAddress>,
    ) {
        // A number from this generation is trusted as it stands; anything else is a name.
        let slot = match address {
            Some(address) if address.generation == self.slots.generation() => {
                Some(crate::Slot::from_index(address.slot as usize))
            }
            _ => self.slots.slot(fixture_id, attribute),
        };
        match slot {
            Some(slot) => self.frame.offer(
                slot,
                crate::Offer {
                    priority,
                    changed_at,
                    merge_mode,
                    transition_ordinal,
                    normalized: value.normalized().unwrap_or(0.0),
                },
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
                    crate::Offer {
                        priority,
                        changed_at,
                        merge_mode,
                        transition_ordinal,
                        normalized: level,
                    },
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

    /// Hand the frame to the boundary that still speaks in names, with the storage it was filled
    /// into and whatever the compiled patch could not number.
    pub(crate) fn finish(mut self) -> ResolvedAttributes {
        // Grouped by fixture so a head reads its own unnumbered values without walking everyone
        // else's. Normally there are none and this costs an empty map.
        let mut overflow: FxHashMap<FixtureId, Vec<(AttributeKey, EngineWinner)>> =
            FxHashMap::default();
        for ((fixture_id, attribute), winner) in std::mem::take(&mut self.overflow) {
            overflow
                .entry(fixture_id)
                .or_default()
                .push((attribute, winner));
        }
        ResolvedAttributes {
            frame: Some(ResolvedFrame {
                slots: std::sync::Arc::clone(self.slots),
                state: Some(self.frame),
                pool: self.pool.take(),
                overflow,
            }),
            ..ResolvedAttributes::default()
        }
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

/// The show's resolved values for one frame. A frame writes and reads these tens of thousands of
/// times, and the keys are already unique, so they are hashed for speed rather than against an
/// adversary.
pub type ResolvedValues = FxHashMap<(FixtureId, AttributeKey), AttributeValue>;
pub type ResolvedChangedAt = FxHashMap<(FixtureId, AttributeKey), DateTime<Utc>>;

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
    ) -> crate::FrameValues {
        let fixture = crate::frame_slots::legacy_test_fixture(fixture_id, attributes);
        let slots =
            std::sync::Arc::new(crate::SlotTable::compile(1, std::slice::from_ref(&fixture)));
        let mut resolver = EngineContributionResolver::unpooled(&slots);
        resolver.extend(contributions);
        resolver.finish().named_values()
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

    /// A value the compiled patch could not number must still reach the boundary and projection,
    /// and must not cost the rest of the frame its dense reading. One unrecognised name from a
    /// hardware surface or an HTTP client is a lookup, not a slower desk.
    /// The slot table is compiled from the patch and from nothing else, so an inbound surface —
    /// OSC, HTTP, anything that names an attribute freely — cannot grow it. That is the bound the
    /// item asks for, and it is zero rather than a limit: a name the patch never declared is an
    /// overflow entry on the frame that made it, discarded with that frame.
    #[test]
    fn an_inbound_name_cannot_grow_the_slot_table() {
        let fixture_id = FixtureId::new();
        let fixture = crate::frame_slots::legacy_test_fixture(fixture_id, &["intensity"]);
        let slots =
            std::sync::Arc::new(crate::SlotTable::compile(1, std::slice::from_ref(&fixture)));
        let numbered = slots.len();
        for index in 0..64 {
            let mut resolver = EngineContributionResolver::unpooled(&slots);
            resolver.add_borrowed_unscaled(
                fixture_id,
                &AttributeKey(format!("inbound{index}").into()),
                &AttributeValue::Normalized(0.5),
                0,
                Utc::now(),
                MergeMode::Ltp,
            );
            let _ = resolver.finish();
        }
        assert_eq!(
            slots.len(),
            numbered,
            "sixty-four names the patch never declared leave the numbering exactly as it was"
        );
    }

    #[test]
    fn a_value_the_patch_never_numbered_costs_itself_a_lookup_not_the_frame() {
        let fixture_id = FixtureId::new();
        let undeclared = AttributeKey("neverPatched".into());
        let fixture = crate::frame_slots::legacy_test_fixture(fixture_id, &["intensity"]);
        let slots =
            std::sync::Arc::new(crate::SlotTable::compile(1, std::slice::from_ref(&fixture)));
        let mut resolver = EngineContributionResolver::unpooled(&slots);
        resolver.add_borrowed_unscaled(
            fixture_id,
            &undeclared,
            &AttributeValue::Normalized(0.42),
            0,
            Utc::now(),
            MergeMode::Ltp,
        );
        let mut resolved = resolver.finish();
        let values = resolved.named_values();
        assert_eq!(
            values.value(fixture_id, &undeclared),
            Some(&AttributeValue::Normalized(0.42)),
            "an operator's value is never lost to a name the patch did not declare"
        );
        assert!(
            values.is_dense(),
            "one unnumbered name costs that value a lookup, not the whole frame its dense reading"
        );
        assert!(values.has_unnumbered_values());
        assert_eq!(
            values.values()[&(fixture_id, undeclared)],
            AttributeValue::Normalized(0.42),
            "and it is there when the boundary asks for everything by name"
        );
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
            resolved[&(fixture_id, AttributeKey::intensity())],
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
            resolved[&(fixture_id, AttributeKey::intensity())],
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
            resolved[&(fixture_id, AttributeKey("pan".into()))],
            AttributeValue::Normalized(0.8)
        );
    }
}

#[cfg(test)]
mod frame_address_tests {
    use super::*;
    use light_core::FrameAddressResolver;

    fn sample(fixture_id: FixtureId, level: f32) -> crate::ContributionSample {
        crate::ContributionSample::independent(TimedValue {
            fixture_id,
            attribute: AttributeKey::intensity(),
            value: AttributeValue::Normalized(level),
            priority: 10,
            changed_at: Utc::now(),
            programmer_order: 0,
            merge_mode: MergeMode::Ltp,
            fade: false,
            fade_millis: None,
            delay_millis: None,
        })
    }

    /// A sample that says where its pair lives is read there, and lands where the name would.
    #[test]
    fn an_addressed_sample_lands_where_its_name_would() {
        let fixture_id = FixtureId::new();
        let fixture = crate::frame_slots::legacy_test_fixture(fixture_id, &["intensity", "pan"]);
        let slots =
            std::sync::Arc::new(crate::SlotTable::compile(3, std::slice::from_ref(&fixture)));
        let addresser = crate::FrameAddresser::new(std::sync::Arc::clone(&slots));
        let address = addresser
            .frame_address(fixture_id, &AttributeKey::intensity())
            .expect("a declared pair has an address");
        assert_eq!(address.generation, 3);
        assert!(
            addresser
                .frame_address(fixture_id, &AttributeKey("zoom".into()))
                .is_none(),
            "an undeclared pair has none"
        );

        let mut resolver = EngineContributionResolver::unpooled(&slots);
        resolver.extend_borrowed_samples([&sample(fixture_id, 0.75).at(Some(address))]);
        let values = resolver.finish().named_values();
        assert_eq!(
            values.value(fixture_id, &AttributeKey::intensity()),
            Some(&AttributeValue::Normalized(0.75))
        );
    }

    /// A number from another patch generation is not trusted: the sample is read by name, as a
    /// producer that has not caught up with a repatch would need.
    #[test]
    fn an_address_from_another_generation_falls_back_to_the_name() {
        let fixture_id = FixtureId::new();
        let fixture = crate::frame_slots::legacy_test_fixture(fixture_id, &["pan", "intensity"]);
        let slots =
            std::sync::Arc::new(crate::SlotTable::compile(4, std::slice::from_ref(&fixture)));
        let stale = light_core::FrameAddress {
            generation: 3,
            slot: 0,
        };
        let mut resolver = EngineContributionResolver::unpooled(&slots);
        resolver.extend_borrowed_samples([&sample(fixture_id, 0.5).at(Some(stale))]);
        let values = resolver.finish().named_values();
        assert_eq!(
            values.value(fixture_id, &AttributeKey::intensity()),
            Some(&AttributeValue::Normalized(0.5))
        );
        assert_eq!(values.value(fixture_id, &AttributeKey("pan".into())), None);
    }
}
