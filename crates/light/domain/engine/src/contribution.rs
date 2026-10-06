use chrono::{DateTime, Utc};
use light_core::{AttributeKey, AttributeValue, FixtureId, MergeMode, TimedValue};
use light_playback::{AutomaticPlaybackTransition, PlaybackContribution, SequenceMasterSource};
use rustc_hash::FxHashMap;
use std::collections::{HashMap, hash_map::Entry};

mod offered_origin;
mod parallel_offers;
mod playback_evidence;
pub(crate) use offered_origin::OfferedOrigin;
pub(crate) use playback_evidence::PlaybackEvidenceCache;

pub(crate) struct EngineContribution {
    value: TimedValue,
    transition_ordinal: Option<u64>,
    /// The Playback this value came from, for source replacement and tracing. None for every
    /// other source.
    playback_source: Option<SequenceMasterSource>,
    /// Where the producer already knows this pair lives; read by number when it belongs to the
    /// frame's generation, by name otherwise.
    address: Option<light_core::FrameAddress>,
    origin: Option<std::sync::Arc<crate::contribution_batch::ContributionOrigin>>,
    family_evidence: Option<std::sync::Arc<crate::ContributionFamilyEvidence>>,
    /// Runtime-only live Position crossing behind the held value (TL-544 G1).
    pending_transition: Option<std::sync::Arc<light_core::programming::PendingFamilyTransition>>,
}

#[cfg(test)]
impl EngineContribution {
    pub(crate) fn timed_value(&self) -> &TimedValue {
        &self.value
    }

    /// Every field, for tests comparing two contribution lists in order.
    pub(crate) fn describe(&self) -> String {
        format!(
            "{:?} {:?} {:?} {:?} {:?} {:?}",
            self.value,
            self.transition_ordinal,
            self.playback_source,
            self.address,
            self.origin,
            self.family_evidence
        )
    }
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
        Self::from_contributions(values.iter())
    }

    pub(crate) fn from_contributions(
        values: impl IntoIterator<Item = &'a EngineContribution>,
    ) -> Self {
        let values = values.into_iter();
        let (minimum, maximum) = values.size_hint();
        let mut index = Self {
            winners: HashMap::with_capacity(maximum.unwrap_or(minimum)),
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

    /// Retain the exact winning underlay at a transition boundary. An independent sample
    /// without producer evidence stays unknown, even if it equals a known authored value.
    pub(crate) fn family_evidence(
        &self,
        fixture_id: FixtureId,
        attribute: &AttributeKey,
    ) -> Option<std::sync::Arc<crate::ContributionFamilyEvidence>> {
        match self.winners.get(&(fixture_id, attribute))? {
            IndexedContribution::Sample(sample) => sample.family_evidence().cloned(),
            IndexedContribution::Engine(contribution) => contribution.family_evidence.clone(),
        }
    }

    /// The live Position crossing behind the winning underlay, if it is still moving.
    pub(crate) fn pending_transition(
        &self,
        fixture_id: FixtureId,
        attribute: &AttributeKey,
    ) -> Option<&std::sync::Arc<light_core::programming::PendingFamilyTransition>> {
        match self.winners.get(&(fixture_id, attribute))? {
            IndexedContribution::Sample(_) => None,
            IndexedContribution::Engine(contribution) => contribution.pending_transition.as_ref(),
        }
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
            playback_source: None,
            address: None,
            origin: None,
            family_evidence: None,
            pending_transition: None,
        }
    }

    /// The live Position crossing behind this held value (TL-544 G1).
    pub(crate) fn with_pending_transition(
        mut self,
        pending: Option<std::sync::Arc<light_core::programming::PendingFamilyTransition>>,
    ) -> Self {
        self.pending_transition = pending;
        self
    }

    /// Say where this contribution's pair lives, when the producer knows.
    pub(crate) fn at(mut self, address: Option<light_core::FrameAddress>) -> Self {
        self.address = address;
        self
    }

    pub(crate) fn with_origin(
        mut self,
        origin: Option<std::sync::Arc<crate::contribution_batch::ContributionOrigin>>,
    ) -> Self {
        self.origin = origin;
        self
    }

    pub(crate) fn with_family_evidence(
        mut self,
        evidence: Option<std::sync::Arc<crate::ContributionFamilyEvidence>>,
    ) -> Self {
        self.family_evidence = evidence;
        self
    }

    pub(crate) fn from_playback(
        contribution: PlaybackContribution,
        evidence_cache: &mut PlaybackEvidenceCache,
    ) -> Self {
        // Endpoint value proof cannot reconstruct historical authorship after restore, pause,
        // manual crossfade or interruption. Only the producer's retained evidence qualifies.
        let family_evidence = contribution
            .family_evidence
            .as_ref()
            .map(|evidence| evidence_cache.project(evidence));
        Self {
            value: contribution.value,
            transition_ordinal: Some(contribution.transition_ordinal),
            playback_source: Some(contribution.source),
            address: contribution.address,
            origin: None,
            family_evidence,
            pending_transition: contribution.pending_transition,
        }
    }

    pub(crate) fn fixture_id(&self) -> FixtureId {
        self.value.fixture_id
    }

    pub(crate) fn attribute(&self) -> &AttributeKey {
        &self.value.attribute
    }

    pub(crate) fn replaced_by(&self, sampled: &[crate::ContributionBatch]) -> bool {
        self.playback_source.is_some_and(|source| {
            crate::replaces_source(
                sampled,
                &crate::ContributionSourceId::playback(source),
                &self.value,
            )
        })
    }

    pub(crate) fn playback_value(&self) -> Option<(SequenceMasterSource, &TimedValue)> {
        self.playback_source.map(|source| (source, &self.value))
    }
}

#[derive(Default)]
pub(crate) struct ResolvedAttributes {
    /// Everything the frame resolved, by name — populated only when there is no dense frame to
    /// read instead. A frame that holds the whole show leaves these empty and is materialised at
    /// the boundary, once, if anyone asks.
    pub(crate) values: ResolvedValues,
    pub(crate) changed_at: ResolvedChangedAt,
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

    /// Master every level value (Intensity, Volume) by `factor(owner, fixture_index)`, on the
    /// dense frame and on the map path alike.
    ///
    /// A level nobody contributed is its profile default, so where its factor is not full it is
    /// held at default × factor, with no source. A full factor leaves the frame untouched.
    pub(crate) fn scale_levels(
        &mut self,
        slots: &crate::SlotTable,
        factor: &mut impl FnMut(FixtureId, Option<u32>) -> f32,
    ) {
        if let Some(frame) = self.frame.as_mut() {
            frame.scale_levels(&mut *factor);
            return;
        }
        for ((owner, attribute), value) in &mut self.values {
            if attribute.is_level()
                && let Some(level) = value.normalized()
            {
                *value = AttributeValue::Normalized(level * factor(*owner, None));
            }
        }
        // The map path has no epochs: an unsourced level is simply a missing name.
        for level in slots.level_slots() {
            let Some(default) = level.default else {
                continue;
            };
            let (owner, attribute) = slots.pair(level.slot);
            let key = (owner, attribute.clone());
            if self.values.contains_key(&key) {
                continue;
            }
            let scale = factor(owner, Some(level.root));
            if scale != 1.0 {
                self.values
                    .insert(key, AttributeValue::Normalized(default * scale));
            }
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
        if let Some(frame) = self.frame.as_mut() {
            if let Some(slot) = frame.slots().slot(fixture_id, attribute) {
                frame.force_at(slot, value, changed_at);
            } else {
                frame.force_overflow(fixture_id, attribute, value, changed_at);
            }
            return;
        }
        let key = (fixture_id, attribute.clone());
        if let Some(changed_at) = changed_at {
            self.changed_at.insert(key.clone(), changed_at);
        }
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
    /// Borrow the actual winner without materializing a named frame, including unnumbered pairs.
    pub(crate) fn winner(
        &self,
        fixture: FixtureId,
        attribute: &AttributeKey,
    ) -> Option<&EngineWinner> {
        match self.slots.slot(fixture, attribute) {
            Some(slot) => self.state.as_ref()?.get(slot),
            None => self
                .overflow(fixture)
                .iter()
                .find(|(key, _)| key == attribute)
                .map(|(_, winner)| winner),
        }
    }

    /// A prepared family projection replaces the resolved payload without rerunning arbitration.
    pub(crate) fn project_family(
        &mut self,
        fixture: FixtureId,
        attribute: &AttributeKey,
        value: AttributeValue,
        metadata: crate::FamilyProjectionMetadata,
    ) -> bool {
        if let Some(slot) = self.slots.slot(fixture, attribute) {
            return self
                .state
                .as_mut()
                .is_some_and(|state| state.project_family(slot, value, metadata));
        }
        let Some((_, winner)) = self
            .overflow
            .get_mut(&fixture)
            .and_then(|values| values.iter_mut().find(|(key, _)| key == attribute))
        else {
            return false;
        };
        metadata.apply(winner, value);
        true
    }

    pub(crate) fn origin(
        &self,
        fixture: FixtureId,
        attribute: &AttributeKey,
    ) -> Option<&crate::contribution_batch::ContributionOrigin> {
        self.slots
            .slot(fixture, attribute)
            .and_then(|slot| self.state.as_ref()?.get(slot)?.origin.as_deref())
            .or_else(|| {
                self.overflow(fixture)
                    .iter()
                    .find(|(key, _)| key == attribute)
                    .and_then(|(_, winner)| winner.origin.as_deref())
            })
    }
    pub(crate) fn family_evidence(
        &self,
        fixture: FixtureId,
        attribute: &AttributeKey,
    ) -> Option<&std::sync::Arc<crate::ContributionFamilyEvidence>> {
        self.slots
            .slot(fixture, attribute)
            .and_then(|slot| self.state.as_ref()?.get(slot)?.family_evidence.as_ref())
            .or_else(|| {
                self.overflow(fixture)
                    .iter()
                    .find(|(key, _)| key == attribute)
                    .and_then(|(_, winner)| winner.family_evidence.as_ref())
            })
    }
    pub(crate) fn pending_transition(
        &self,
        fixture: FixtureId,
        attribute: &AttributeKey,
    ) -> Option<&std::sync::Arc<light_core::programming::PendingFamilyTransition>> {
        self.winner(fixture, attribute)?.pending_transition.as_ref()
    }
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

    /// The raw parameter holding a pair, before the output-parameter masters (Freeze included).
    pub(crate) fn raw_value(
        &self,
        fixture: FixtureId,
        attribute: &AttributeKey,
    ) -> Option<&AttributeValue> {
        self.winner(fixture, attribute)?.raw_value()
    }

    /// When the value holding a slot last changed.
    pub(crate) fn changed_at(&self, slot: crate::Slot) -> Option<DateTime<Utc>> {
        self.state.as_ref()?.get(slot)?.output_changed_at()
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

    pub(crate) fn slots(&self) -> &crate::SlotTable {
        &self.slots
    }

    /// Master every level value (Intensity, Volume) by its fixture's factor, keeping who decided
    /// it, and hold an unsourced level at its mastered profile default. `factor(owner, index)`.
    ///
    /// The unnumbered overflow only ever holds contributed values: every level a definition
    /// declares is numbered, so an unsourced level always has a slot.
    pub(crate) fn scale_levels(&mut self, mut factor: impl FnMut(FixtureId, Option<u32>) -> f32) {
        if let Some(state) = self.state.as_mut() {
            for level in self.slots.level_slots() {
                let (owner, _) = self.slots.pair(level.slot);
                let scale = factor(owner, Some(level.root));
                if scale == 1.0 {
                    continue;
                }
                if state.get(level.slot).is_some() {
                    state.scale_level(level.slot, scale);
                } else if let Some(default) = level.default {
                    state.fill_unsourced_level(
                        level.slot,
                        AttributeValue::Normalized(default * scale),
                    );
                }
            }
        }
        for (owner, values) in &mut self.overflow {
            for (attribute, winner) in values.iter_mut() {
                if attribute.is_level()
                    && let Some(level) = winner.value.normalized()
                {
                    let scale = factor(*owner, None);
                    if scale != 1.0 {
                        winner.pre_master.get_or_insert(Some(winner.value.clone()));
                        winner.value = AttributeValue::Normalized(level * scale);
                    }
                }
            }
        }
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

    pub(crate) fn force_overflow(
        &mut self,
        fixture: FixtureId,
        attribute: &AttributeKey,
        value: AttributeValue,
        changed_at: Option<DateTime<Utc>>,
    ) {
        let Some((_, winner)) = self
            .overflow
            .get_mut(&fixture)
            .and_then(|values| values.iter_mut().find(|(key, _)| key == attribute))
        else {
            return;
        };
        winner.value = value;
        if let Some(changed_at) = changed_at {
            winner.changed_at = changed_at;
            winner.projected_changed_at = None;
        }
        winner.origin = None;
        winner.family_evidence = None;
        winner.pending_transition = None;
        winner.pre_master = None;
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
    trace_sources: bool,
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
            trace_sources: false,
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
            trace_sources: false,
        }
    }

    pub(crate) fn extend(&mut self, values: impl IntoIterator<Item = EngineContribution>) {
        for value in values {
            self.add(value);
        }
    }

    pub(crate) fn extend_borrowed_contributions<'c>(
        &mut self,
        values: impl IntoIterator<Item = &'c EngineContribution>,
    ) {
        for candidate in values {
            let value = &candidate.value;
            let (origin, family_evidence) =
                parallel_offers::contribution_trace(self.trace_sources, candidate);
            self.add_borrowed(
                value.fixture_id,
                &value.attribute,
                &value.value,
                value.priority,
                value.changed_at,
                value.merge_mode,
                candidate.transition_ordinal,
                candidate.address,
                origin,
                family_evidence,
                candidate.pending_transition.as_ref(),
            );
        }
    }

    pub(crate) fn tracing_sources(mut self) -> Self {
        self.trace_sources = true;
        self
    }

    pub(crate) fn add_playback_unscaled(&mut self, value: TimedValue, transition_ordinal: u64) {
        self.add(EngineContribution {
            value,
            transition_ordinal: Some(transition_ordinal),
            playback_source: None,
            address: None,
            origin: None,
            family_evidence: None,
            pending_transition: None,
        });
    }

    pub(crate) fn extend_borrowed_samples<'s>(
        &mut self,
        samples: impl IntoIterator<Item = &'s crate::ContributionSample>,
    ) {
        for sample in samples {
            let value = sample.value();
            let (origin, family_evidence) =
                parallel_offers::sample_trace(self.trace_sources, sample);
            self.add_borrowed(
                value.fixture_id,
                &value.attribute,
                &value.value,
                value.priority,
                value.changed_at,
                value.merge_mode,
                sample.transition_ordinal(),
                sample.address(),
                origin,
                family_evidence,
                None,
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
            fixture_id,
            attribute,
            value,
            priority,
            changed_at,
            merge_mode,
            None,
            None,
            OfferedOrigin::None,
            None,
            None,
        );
    }

    /// Offer a value straight to its slot. Group programming reaches the resolver this way: the
    /// generation worked out where every member's attribute lives, and a member that does not
    /// have the attribute was left out then.
    pub(crate) fn add_slot_unscaled(
        &mut self,
        slot: crate::Slot,
        value: &AttributeValue,
        priority: i16,
        changed_at: DateTime<Utc>,
        merge_mode: MergeMode,
    ) {
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
        address: Option<light_core::FrameAddress>,
        origin: OfferedOrigin<'_>,
        family_evidence: Option<&std::sync::Arc<crate::ContributionFamilyEvidence>>,
        pending_transition: Option<
            &std::sync::Arc<light_core::programming::PendingFamilyTransition>,
        >,
    ) {
        match self.slot_for(fixture_id, attribute, address) {
            Some(slot) => self.frame.offer_with_origin(
                slot,
                crate::Offer {
                    priority,
                    changed_at,
                    merge_mode,
                    transition_ordinal,
                    normalized: value.normalized().unwrap_or(0.0),
                },
                (origin, family_evidence),
                |winner| {
                    winner.value = value.clone();
                    winner.pending_transition = pending_transition.cloned();
                },
            ),
            None => self.offer_overflow(
                fixture_id,
                attribute,
                EngineWinner {
                    value: value.clone(),
                    priority,
                    changed_at,
                    projected_changed_at: None,
                    merge_mode,
                    transition_ordinal,
                    origin: origin.resolve(None),
                    family_evidence: family_evidence.cloned(),
                    pending_transition: pending_transition.cloned(),
                    pre_master: None,
                },
            ),
        }
    }

    fn add(&mut self, candidate: EngineContribution) {
        let EngineContribution {
            value,
            transition_ordinal,
            playback_source,
            address,
            mut origin,
            family_evidence,
            pending_transition,
        } = candidate;
        if self.trace_sources && origin.is_none() {
            origin = playback_source.map(|source| {
                crate::contribution_batch::ContributionOrigin::with_transition_ordinal(
                    crate::ContributionSourceId::playback(source),
                    &value,
                    transition_ordinal,
                )
            });
        }
        let family_evidence = self.trace_sources.then_some(family_evidence).flatten();
        let TimedValue {
            fixture_id,
            attribute,
            value,
            priority,
            changed_at,
            merge_mode,
            ..
        } = value;
        let slot = match address {
            Some(address) if address.generation == self.slots.generation() => {
                Some(crate::Slot::from_index(address.slot as usize))
            }
            _ => self.slots.slot(fixture_id, &attribute),
        };
        match slot {
            Some(slot) => {
                let level = value.normalized().unwrap_or(0.0);
                let mut carried = Some((value, pending_transition));
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
                        if let Some((value, pending_transition)) = carried.take() {
                            winner.value = value;
                            winner.pending_transition = pending_transition;
                        }
                        winner.origin = origin;
                        winner.family_evidence = family_evidence;
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
                    projected_changed_at: None,
                    merge_mode,
                    transition_ordinal,
                    origin,
                    family_evidence,
                    pending_transition,
                    pre_master: None,
                },
            ),
        }
    }

    /// A pair's slot: a number from this generation is trusted as it stands; anything else is
    /// a name.
    fn slot_for(
        &self,
        fixture_id: FixtureId,
        attribute: &AttributeKey,
        address: Option<light_core::FrameAddress>,
    ) -> Option<crate::Slot> {
        match address {
            Some(address) if address.generation == self.slots.generation() => {
                Some(crate::Slot::from_index(address.slot as usize))
            }
            _ => self.slots.slot(fixture_id, attribute),
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
mod transition_order_tests;

#[cfg(test)]
mod frame_address_tests;
