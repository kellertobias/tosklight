use crate::{
    ContributionBatch, ContributionSourceId, Engine, EngineContribution, OutputContinuityState,
    ProgrammerTransitionSource, ResolvedContributionIndex, RuntimeGeneration, replaces_source,
};
use chrono::{DateTime, Utc};
use light_core::{
    AttributeKey, FixtureId, FrameAddress, FrameAddressResolver, MergeMode, ProgrammerId,
    TimedValue,
};
use light_programmer::{GroupProgrammerValue, ProgrammerOutputState};
use std::collections::HashSet;
use std::{collections::HashMap, sync::Arc};

/// A stored value on its way to the frame, with where the frame keeps it when that is known.
pub(crate) type Addressed = (
    TimedValue,
    Option<FrameAddress>,
    Option<Arc<crate::contribution_batch::ContributionOrigin>>,
    Option<Arc<crate::ContributionFamilyEvidence>>,
    crate::programmer_fade::Pending,
);

/// Where one shared vector of Programmer values lives in one generation's frame.
///
/// The registry hands the engine the same `Arc` on every tick until the operator edits, so the
/// vector's identity and the generation together say whether the addresses still hold.
#[derive(Debug)]
struct AddressedValues {
    generation: u64,
    values: std::sync::Weak<Vec<TimedValue>>,
    addresses: Arc<[Option<FrameAddress>]>,
}

/// Every active Programmer's remembered addresses, by Programmer and value lane.
#[derive(Debug, Default)]
pub(crate) struct ProgrammerAddressMemo {
    lanes: HashMap<(ProgrammerId, ValueLane), AddressedValues>,
    group_lanes: HashMap<(ProgrammerId, ValueLane), CompiledGroupLane>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum ValueLane {
    Live,
    Preload,
}

impl ProgrammerAddressMemo {
    /// The addresses of `values`, resolved now only if this vector has not been seen in this
    /// generation before.
    fn addresses(
        &mut self,
        programmer_id: ProgrammerId,
        lane: ValueLane,
        values: &Arc<Vec<TimedValue>>,
        resolver: &dyn FrameAddressResolver,
    ) -> Arc<[Option<FrameAddress>]> {
        let generation = resolver.generation();
        if let Some(known) = self.lanes.get(&(programmer_id, lane))
            && known.generation == generation
            && known.values.as_ptr() == Arc::as_ptr(values)
            && known.values.strong_count() > 0
        {
            return Arc::clone(&known.addresses);
        }
        let addresses = values
            .iter()
            .map(|value| resolver.frame_address(value.fixture_id, &value.attribute))
            .collect::<Arc<[_]>>();
        self.lanes.insert(
            (programmer_id, lane),
            AddressedValues {
                generation,
                values: Arc::downgrade(values),
                addresses: Arc::clone(&addresses),
            },
        );
        addresses
    }

    fn retain_programmers(&mut self, active: &HashSet<ProgrammerId>) {
        self.lanes
            .retain(|(programmer_id, _), _| active.contains(programmer_id));
        self.group_lanes.retain(|(id, _), _| active.contains(id));
    }
}

type GroupValues = HashMap<String, HashMap<AttributeKey, GroupProgrammerValue>>;
#[derive(Clone, Debug)]
struct CompiledGroupEntry {
    group: Arc<str>,
    fixture: FixtureId,
    attribute: AttributeKey,
    scoped: GroupProgrammerValue,
    address: Option<FrameAddress>,
}
#[derive(Debug)]
struct CompiledGroupLane {
    source: std::sync::Weak<GroupValues>,
    rankings: std::sync::Weak<HashMap<String, light_dynamics::RankedSelection>>,
    models: std::sync::Weak<crate::ProfileEncodingIndex>,
    slots: u64,
    entries: Arc<[CompiledGroupEntry]>,
    // Kept once per source generation for the coherent-frame diagnostic projection.
    _unresolved: Vec<(String, AttributeKey, light_core::programming::IntentError)>,
}
impl ProgrammerAddressMemo {
    fn group_entries(
        &mut self,
        programmer: ProgrammerId,
        lane: ValueLane,
        source: &Arc<GroupValues>,
        generation: &RuntimeGeneration,
        addresser: &crate::FrameAddresser,
    ) -> Arc<[CompiledGroupEntry]> {
        let rankings = generation.group_rankings_arc();
        let models = generation.profile_encodings_arc();
        if let Some(known) = self.group_lanes.get(&(programmer, lane))
            && known.source.as_ptr() == Arc::as_ptr(source)
            && known.source.strong_count() > 0
            && known.rankings.as_ptr() == Arc::as_ptr(&rankings)
            && known.rankings.strong_count() > 0
            && known.models.as_ptr() == Arc::as_ptr(&models)
            && known.models.strong_count() > 0
            && known.slots == addresser.generation()
        {
            return Arc::clone(&known.entries);
        }
        let mut entries = Vec::new();
        let mut unresolved = Vec::new();
        let mut groups = source.iter().collect::<Vec<_>>();
        groups.sort_by_key(|(id, _)| *id);
        for (id, attributes) in groups {
            let Some(ranking) = rankings.get(id) else {
                continue;
            };
            let group: Arc<str> = id.as_str().into();
            let mut attributes = attributes.iter().collect::<Vec<_>>();
            attributes.sort_by_key(|(attribute, _)| *attribute);
            for (attribute, scoped) in attributes {
                match crate::group_programming::compile_group_values(&scoped.value, ranking) {
                    Ok(values) => {
                        for (fixture, value) in values {
                            let mut scoped = scoped.clone();
                            scoped.value = value;
                            entries.push(CompiledGroupEntry {
                                group: Arc::clone(&group),
                                fixture,
                                attribute: attribute.clone(),
                                scoped,
                                address: addresser.frame_address(fixture, attribute),
                            });
                        }
                    }
                    Err(error) => unresolved.push((id.clone(), attribute.clone(), error)),
                }
            }
        }
        let entries: Arc<[_]> = entries.into();
        self.group_lanes.insert(
            (programmer, lane),
            CompiledGroupLane {
                source: Arc::downgrade(source),
                rankings: Arc::downgrade(&rankings),
                models: Arc::downgrade(&models),
                slots: addresser.generation(),
                entries: Arc::clone(&entries),
                _unresolved: unresolved,
            },
        );
        entries
    }
}

#[derive(Clone, Copy)]
enum ProgrammerValueSource<'a> {
    Live,
    Preload,
    Transient(&'a str),
    Group(&'a str),
    PreloadGroup(&'a str),
}

struct SourceContext {
    transition: ProgrammerTransitionSource,
    replacement: Option<ContributionSourceId>,
}

struct ProgrammerValueResolver<'a, 'continuity> {
    addresses: &'a parking_lot::Mutex<ProgrammerAddressMemo>,
    generation: &'a RuntimeGeneration,
    /// Where this generation keeps each pair. Values the memo could not cover — Group and
    /// transient values, built fresh each tick — ask it directly, so every lane keys the same
    /// pair the same way and the later edit still wins.
    addresser: &'a crate::FrameAddresser,
    now: DateTime<Utc>,
    underlay: Option<&'a ResolvedContributionIndex<'a>>,
    programmer_id: ProgrammerId,
    priority: i16,
    has_replacements: bool,
    trace_sources: bool,
    default_fade_millis: u64,
    group_colors: &'a HashMap<String, crate::engine::GroupColorContribution>,
    transitions: &'continuity mut crate::programmer_memo::ProgrammerTransitions,
    active_transition_keys: rustc_hash::FxHashSet<crate::ProgrammerTransitionKey>,
}

pub(crate) fn programmers_need_underlay(programmers: &[ProgrammerOutputState]) -> bool {
    programmers.iter().any(|programmer| {
        programmer
            .values
            .iter()
            .chain(
                programmer
                    .transient_values
                    .iter()
                    .flat_map(|action| &action.values),
            )
            .chain(programmer.preload_active.iter())
            .any(|value| value.fade)
            || programmer
                .group_values
                .values()
                .chain(programmer.preload_group_active.values())
                .flat_map(HashMap::values)
                .any(|value| value.fade)
    })
}

impl Engine {
    /// Evaluate captured sources against one lane's history and compiled-address cache.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn programmer_contributions_with_state(
        &self,
        programmers: Vec<ProgrammerOutputState>,
        generation: &RuntimeGeneration,
        now: DateTime<Utc>,
        underlay: Option<&ResolvedContributionIndex<'_>>,
        sampled: &[ContributionBatch],
        trace_sources: bool,
        continuity: &mut OutputContinuityState,
        default_fade_millis: u64,
        group_colors: &HashMap<String, crate::engine::GroupColorContribution>,
        addresses: &parking_lot::Mutex<ProgrammerAddressMemo>,
    ) -> ProgrammerContributions {
        let has_replacements = sampled.iter().any(ContributionBatch::has_replacements);
        // TL-639: without a fade the evaluation before the replacement filter depends only on
        // the captured vectors, the generation, these flags and the transition history.
        let flags = (trace_sources, has_replacements);
        let memoizable = !programmers_need_underlay(&programmers);
        let transitions = continuity.programmer_transitions.version();
        let kept = memoizable
            .then(|| {
                self.programmer_memo.lock().find(
                    &programmers,
                    generation.identity(),
                    flags,
                    transitions,
                )
            })
            .flatten();
        let (resolved, winners) = match kept {
            Some(kept) => (kept.resolved, Some(kept.winners)),
            None => {
                let kept_states = memoizable.then(|| programmers.clone());
                let resolved = Arc::new(self.resolve_programmers(
                    programmers,
                    generation,
                    now,
                    underlay,
                    has_replacements,
                    trace_sources,
                    continuity,
                    default_fade_millis,
                    group_colors,
                    addresses,
                ));
                let winners = kept_states
                    .filter(|_| continuity.programmer_transitions.version() == transitions)
                    .map(|states| {
                        self.programmer_memo.lock().keep(
                            states,
                            generation.identity(),
                            flags,
                            transitions,
                            Arc::clone(&resolved),
                        )
                    });
                (resolved, winners)
            }
        };
        match winners {
            Some(winners) => {
                let removed = removed_by(&resolved, sampled);
                ProgrammerContributions::Shared(
                    winners.get_or_arbitrate(removed, || arbitrate(&resolved, sampled)),
                )
            }
            None => ProgrammerContributions::Owned(arbitrate(&resolved, sampled)),
        }
    }

    /// Every Programmer's values in order, before the sampled-replacement filter.
    #[allow(clippy::too_many_arguments)]
    fn resolve_programmers(
        &self,
        programmers: Vec<ProgrammerOutputState>,
        generation: &RuntimeGeneration,
        now: DateTime<Utc>,
        underlay: Option<&ResolvedContributionIndex<'_>>,
        has_replacements: bool,
        trace_sources: bool,
        continuity: &mut OutputContinuityState,
        default_fade_millis: u64,
        group_colors: &HashMap<String, crate::engine::GroupColorContribution>,
        addresses: &parking_lot::Mutex<ProgrammerAddressMemo>,
    ) -> Vec<crate::programmer_memo::ResolvedProgrammerValues> {
        let active_programmers = programmers
            .iter()
            .map(|programmer| programmer.id)
            .collect::<HashSet<_>>();
        continuity
            .programmer_transitions
            .retain(|key, _| active_programmers.contains(&key.programmer_id));
        addresses.lock().retain_programmers(&active_programmers);
        let addresser = crate::FrameAddresser::new(Arc::clone(generation.slots()));
        programmers
            .into_iter()
            .map(|programmer| {
                self.resolve_programmer(
                    programmer,
                    generation,
                    &addresser,
                    now,
                    underlay,
                    has_replacements,
                    trace_sources,
                    &mut continuity.programmer_transitions,
                    default_fade_millis,
                    group_colors,
                    addresses,
                )
            })
            .collect()
    }

    #[allow(clippy::too_many_arguments)]
    fn resolve_programmer(
        &self,
        programmer: ProgrammerOutputState,
        generation: &RuntimeGeneration,
        addresser: &crate::FrameAddresser,
        now: DateTime<Utc>,
        underlay: Option<&ResolvedContributionIndex<'_>>,
        has_replacements: bool,
        trace_sources: bool,
        transitions: &mut crate::programmer_memo::ProgrammerTransitions,
        default_fade_millis: u64,
        group_colors: &HashMap<String, crate::engine::GroupColorContribution>,
        addresses: &parking_lot::Mutex<ProgrammerAddressMemo>,
    ) -> crate::programmer_memo::ResolvedProgrammerValues {
        let ProgrammerOutputState {
            id,
            priority,
            values,
            transient_values,
            group_values,
            preload_active,
            preload_group_active,
            ..
        } = programmer;
        let mut resolver = ProgrammerValueResolver {
            addresses,
            generation,
            addresser,
            now,
            underlay,
            programmer_id: id,
            priority,
            has_replacements,
            trace_sources,
            default_fade_millis,
            group_colors,
            transitions,
            active_transition_keys: Default::default(),
        };
        let (live_addresses, preload_addresses) = {
            let mut memo = addresses.lock();
            (
                memo.addresses(id, ValueLane::Live, &values, addresser),
                memo.addresses(id, ValueLane::Preload, &preload_active, addresser),
            )
        };
        let mut contributions =
            resolver.fixture_values(&values, &live_addresses, ProgrammerValueSource::Live);
        for action in transient_values.iter() {
            contributions.extend(resolver.fixture_values(
                &action.values,
                &[],
                ProgrammerValueSource::Transient(&action.source),
            ));
        }
        contributions.extend(resolver.fixture_values(
            &preload_active,
            &preload_addresses,
            ProgrammerValueSource::Preload,
        ));
        contributions.extend(resolver.group_values(&group_values, &preload_group_active));
        let active_transition_keys = resolver.active_transition_keys;
        resolver
            .transitions
            .retain(|key, _| key.programmer_id != id || active_transition_keys.contains(key));
        contributions
    }
}

impl ProgrammerValueResolver<'_, '_> {
    fn resolve_programmer_fade(
        &mut self,
        value: TimedValue,
        source: ProgrammerTransitionSource,
    ) -> Option<crate::programmer_fade::ProgrammerSample> {
        let group_color_underlay = (*value.attribute.0 == *"color")
            .then(|| {
                Engine::group_color_for_fixture_from(
                    self.group_colors,
                    self.generation,
                    value.fixture_id,
                )
            })
            .flatten()
            .map(|(value, _)| value);
        let underlying = group_color_underlay.as_ref().or_else(|| {
            self.underlay
                .and_then(|values| values.value(value.fixture_id, &value.attribute))
                .or_else(|| {
                    self.generation
                        .default_value(value.fixture_id, &value.attribute)
                })
        });
        // An indexed or control attribute names a state, not a level: play mode, media folder and
        // file, gobo and colour wheel slots. Interpolating one walks the operator's selection
        // through every slot in between and only arrives at the chosen one when the Programmer
        // fade ends, which is why an Audio Player took the fade time to change source or transport.
        // Cue transitions already snap these; the Programmer now agrees, whatever the profile's
        // own channel flag says.
        let snap = self
            .generation
            .attribute_is_snap(value.fixture_id, &value.attribute)
            || light_playback::attribute_uses_snap_transition(&value.attribute);
        let fixture_id = value.fixture_id;
        let attribute = value.attribute.clone();
        let underlay = self.underlay;
        let underlay_is_known = group_color_underlay.is_none();
        let underlying_pending = underlay_is_known
            .then(|| {
                underlay?
                    .pending_transition(fixture_id, &attribute)
                    .cloned()
            })
            .flatten();
        crate::programmer_fade::faded_programmer_value(
            self.transitions,
            self.default_fade_millis,
            value,
            self.now,
            underlying,
            || {
                underlay_is_known
                    .then(|| underlay?.family_evidence(fixture_id, &attribute))
                    .flatten()
            },
            underlying_pending,
            self.programmer_id,
            source,
            snap,
        )
    }
}

impl ProgrammerValueResolver<'_, '_> {
    /// Borrowed rather than owned: the Programmer's stored values are shared with every other
    /// reader, so a value is copied here only if it survives to contribute.
    fn fixture_values(
        &mut self,
        values: &[TimedValue],
        addresses: &[Option<FrameAddress>],
        source: ProgrammerValueSource<'_>,
    ) -> crate::programmer_memo::ResolvedProgrammerValues {
        let context = self.source_context(source);
        // A remembered slice answers for its vector, absent addresses included; only a lane
        // nobody remembers asks the generation.
        let remembered = !addresses.is_empty();
        values
            .iter()
            .enumerate()
            .filter_map(|(index, value)| {
                let address = if remembered {
                    addresses.get(index).copied().flatten()
                } else {
                    self.addresser
                        .frame_address(value.fixture_id, &value.attribute)
                };
                self.resolve_value(value.clone(), &context)
                    .map(|(value, evidence, pending)| {
                        let origin = self
                            .trace_sources
                            .then(|| context.replacement.as_ref())
                            .flatten()
                            .map(|source| {
                                crate::contribution_batch::ContributionOrigin::new(
                                    source.clone(),
                                    &value,
                                )
                            });
                        let evidence = self.trace_sources.then_some(evidence).flatten();
                        (
                            (value, address, origin, evidence, pending),
                            context.replacement.clone(),
                        )
                    })
            })
            .collect()
    }

    fn group_values(
        &mut self,
        group_values: &Arc<GroupValues>,
        preload_values: &Arc<GroupValues>,
    ) -> crate::programmer_memo::ResolvedProgrammerValues {
        let mut resolved = Vec::new();
        for (lane, source) in [
            (ValueLane::Live, group_values),
            (ValueLane::Preload, preload_values),
        ] {
            let entries = self.addresses.lock().group_entries(
                self.programmer_id,
                lane,
                source,
                self.generation,
                self.addresser,
            );
            for entry in entries.iter() {
                let context = self.source_context(match lane {
                    ValueLane::Live => ProgrammerValueSource::Group(&entry.group),
                    ValueLane::Preload => ProgrammerValueSource::PreloadGroup(&entry.group),
                });
                let scoped = &entry.scoped;
                let value = TimedValue {
                    fixture_id: entry.fixture,
                    attribute: entry.attribute.clone(),
                    value: scoped.value.clone(),
                    priority: self.priority,
                    changed_at: scoped.changed_at,
                    programmer_order: scoped.programmer_order,
                    merge_mode: MergeMode::Ltp,
                    fade: scoped.fade,
                    fade_millis: scoped.fade_millis,
                    delay_millis: scoped.delay_millis,
                };
                if let Some((value, evidence, pending)) = self.resolve_value(value, &context) {
                    let origin = self
                        .trace_sources
                        .then(|| context.replacement.as_ref())
                        .flatten()
                        .map(|source| {
                            crate::contribution_batch::ContributionOrigin::new(
                                source.clone(),
                                &value,
                            )
                        });
                    let evidence = self.trace_sources.then_some(evidence).flatten();
                    resolved.push((
                        (value, entry.address, origin, evidence, pending),
                        context.replacement.clone(),
                    ));
                }
            }
        }
        resolved
    }

    fn source_context(&self, source: ProgrammerValueSource<'_>) -> SourceContext {
        SourceContext {
            transition: source.transition(),
            replacement: (self.has_replacements || self.trace_sources)
                .then(|| source.replacement(self.programmer_id)),
        }
    }

    fn resolve_value(
        &mut self,
        value: TimedValue,
        source: &SourceContext,
    ) -> Option<crate::programmer_fade::ProgrammerSample> {
        let transition_key = crate::programmer_fade::programmer_transition_key(
            &value,
            self.programmer_id,
            source.transition.clone(),
        );
        self.active_transition_keys.insert(transition_key.clone());
        let (value, evidence, pending) = if value.fade {
            self.resolve_programmer_fade(value, source.transition.clone())?
        } else {
            let evidence = crate::programmer_fade::track_immediate_programmer_value(
                self.transitions,
                transition_key,
                &value,
            );
            (value, evidence, None)
        };
        // The sampled-replacement filter runs on the collected values (TL-639).
        Some((value, evidence, pending))
    }
}

impl ProgrammerValueSource<'_> {
    fn transition(self) -> ProgrammerTransitionSource {
        match self {
            Self::Live => ProgrammerTransitionSource::Programmer,
            Self::Preload => ProgrammerTransitionSource::Preload,
            Self::Transient(source) => ProgrammerTransitionSource::Transient(Arc::from(source)),
            Self::Group(group_id) => ProgrammerTransitionSource::Group(Arc::from(group_id)),
            Self::PreloadGroup(group_id) => {
                ProgrammerTransitionSource::PreloadGroup(Arc::from(group_id))
            }
        }
    }

    fn replacement(self, programmer_id: ProgrammerId) -> ContributionSourceId {
        match self {
            Self::Live => ContributionSourceId::programmer(programmer_id),
            Self::Preload => ContributionSourceId::preload(programmer_id),
            Self::Transient(source) => {
                ContributionSourceId::programmer_transient(programmer_id, source)
            }
            Self::Group(group_id) => {
                ContributionSourceId::programmer_group(programmer_id, group_id)
            }
            Self::PreloadGroup(group_id) => {
                ContributionSourceId::preload_group(programmer_id, group_id)
            }
        }
    }
}

/// True when `value` is the later operator edit.
///
/// The registry hands every live edit a monotonic order from one desk-wide counter, so two edits
/// that carry an order are ranked by it alone. The wall clock is not monotonic: two edits made in
/// the same instant can be stamped out of sequence, which used to hand LTP to the earlier one.
/// Order zero means no counter ever stamped the value — a legacy stored value restored with a
/// fresh timestamp — so those still rank by time, which is what keeps a restored value current.
fn supersedes(value: &TimedValue, current: &TimedValue) -> bool {
    crate::ContributionReleaseCutoff {
        changed_at: value.changed_at,
        programmer_order: value.programmer_order,
    }
    .supersedes(current.changed_at, current.programmer_order)
}

/// How the winners map tells one pair from another: by number when the frame has one, by name
/// otherwise. Every lane asks the same generation, so one pair never gets both keys.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum WinnerKey {
    Address(FrameAddress),
    Name(FixtureId, AttributeKey),
}

/// Positions (in evaluation order) of the values `sampled` replaces.
fn removed_by(
    resolved: &[crate::programmer_memo::ResolvedProgrammerValues],
    sampled: &[ContributionBatch],
) -> Vec<u32> {
    resolved
        .iter()
        .flatten()
        .enumerate()
        .filter(|(_, ((value, ..), replacement))| {
            replacement
                .as_ref()
                .is_some_and(|source| replaces_source(sampled, source, value))
        })
        .map(|(index, _)| index as u32)
        .collect()
}

/// Every Programmer's winning contributions after the sampled-replacement filter.
fn arbitrate(
    resolved: &[crate::programmer_memo::ResolvedProgrammerValues],
    sampled: &[ContributionBatch],
) -> Vec<EngineContribution> {
    resolved
        .iter()
        .flat_map(|values| {
            programmer_winners(
                values
                    .iter()
                    .filter(|((value, ..), replacement)| {
                        !replacement
                            .as_ref()
                            .is_some_and(|source| replaces_source(sampled, source, value))
                    })
                    .map(|(addressed, _)| addressed.clone())
                    .collect(),
            )
        })
        .map(|(value, address, origin, evidence, pending)| {
            EngineContribution::unscaled(value)
                .at(address)
                .with_origin(origin)
                .with_family_evidence(evidence)
                .with_pending_transition(pending)
        })
        .collect()
}

/// The Programmer contributions of one resolution: computed for it, or the winners kept with an
/// unchanged evaluation (TL-639 round 2). Both hold the same values in the same order.
pub(crate) enum ProgrammerContributions {
    Owned(Vec<EngineContribution>),
    Shared(Arc<Vec<EngineContribution>>),
}

impl std::ops::Deref for ProgrammerContributions {
    type Target = [EngineContribution];

    fn deref(&self) -> &[EngineContribution] {
        match self {
            Self::Owned(values) => values,
            Self::Shared(values) => values,
        }
    }
}

impl ProgrammerContributions {
    /// Offer every contribution to `resolver` in order: moved when owned, borrowed when kept.
    pub(crate) fn offer_to(
        self,
        resolver: &mut crate::contribution::EngineContributionResolver<'_>,
    ) {
        match self {
            Self::Owned(values) => resolver.extend(values),
            Self::Shared(values) => resolver.extend_borrowed_contributions(values.iter()),
        }
    }
}

fn programmer_winners(values: Vec<Addressed>) -> Vec<Addressed> {
    // Rebuilt every frame from the operator's live edits, so it is sized for what it is about to
    // hold rather than regrown as it fills, and hashed with the desk's hasher rather than SipHash.
    let mut winners =
        rustc_hash::FxHashMap::with_capacity_and_hasher(values.len(), rustc_hash::FxBuildHasher);
    for (value, address, origin, evidence, pending) in values {
        let key = match address {
            Some(address) => WinnerKey::Address(address),
            None => WinnerKey::Name(value.fixture_id, value.attribute.clone()),
        };
        let replace = winners
            .get(&key)
            .is_none_or(|(current, ..): &Addressed| supersedes(&value, current));
        if replace {
            winners.insert(key, (value, address, origin, evidence, pending));
        }
    }
    winners
        .into_values()
        .map(|(mut value, address, origin, evidence, pending)| {
            value.merge_mode = if value.attribute.is_intensity() {
                MergeMode::Htp
            } else {
                MergeMode::Ltp
            };
            (value, address, origin, evidence, pending)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use light_core::{AttributeValue, FixtureId};

    fn value(changed_at_millis: i64, programmer_order: u64) -> TimedValue {
        TimedValue {
            fixture_id: FixtureId::new(),
            attribute: AttributeKey("pan".into()),
            value: AttributeValue::Normalized(0.5),
            priority: 0,
            changed_at: DateTime::from_timestamp_millis(changed_at_millis).expect("a timestamp"),
            programmer_order,
            merge_mode: MergeMode::Ltp,
            fade: false,
            fade_millis: None,
            delay_millis: None,
        }
    }

    /// The wall clock can hand two edits made in the same instant timestamps that run backwards.
    /// The desk-wide counter cannot, so the operator's second edit still wins.
    #[test]
    fn a_later_edit_wins_even_when_the_clock_stamped_it_earlier() {
        let first = value(1_000, 1);
        let second = value(999, 2);
        assert!(supersedes(&second, &first));
        assert!(!supersedes(&first, &second));
    }

    #[test]
    fn edits_that_share_one_timestamp_rank_by_the_counter() {
        let first = value(1_000, 1);
        let second = value(1_000, 2);
        assert!(supersedes(&second, &first));
        assert!(!supersedes(&first, &second));
    }

    /// A legacy stored value is restored without a counter order and with a fresh timestamp, which
    /// is what keeps it current against values that were stored alongside it.
    #[test]
    fn an_uncounted_legacy_value_still_ranks_by_time() {
        let stored = value(1_000, 42);
        let restored_legacy = value(2_000, 0);
        assert!(supersedes(&restored_legacy, &stored));
        assert!(!supersedes(&stored, &restored_legacy));
    }
}

#[cfg(test)]
mod address_memo_tests {
    use super::*;
    use light_core::AttributeValue;
    use std::cell::Cell;

    struct CountingAddresser {
        generation: u64,
        asked: Cell<usize>,
    }

    impl FrameAddressResolver for CountingAddresser {
        fn generation(&self) -> u64 {
            self.generation
        }

        fn frame_address(&self, _: FixtureId, _: &AttributeKey) -> Option<FrameAddress> {
            self.asked.set(self.asked.get() + 1);
            Some(FrameAddress {
                generation: self.generation,
                slot: 3,
            })
        }
    }

    fn stored(count: usize) -> Arc<Vec<TimedValue>> {
        Arc::new(
            (0..count)
                .map(|_| TimedValue {
                    fixture_id: FixtureId::new(),
                    attribute: AttributeKey::intensity(),
                    value: AttributeValue::Normalized(0.5),
                    priority: 0,
                    changed_at: Utc::now(),
                    programmer_order: 1,
                    merge_mode: MergeMode::Ltp,
                    fade: false,
                    fade_millis: None,
                    delay_millis: None,
                })
                .collect(),
        )
    }

    /// The registry hands the engine the same vector until the operator edits, so its addresses
    /// are resolved once; an edit is a new vector and a repatch is a new generation, and either
    /// resolves again.
    #[test]
    fn addresses_are_resolved_once_per_vector_and_generation() {
        let programmer = ProgrammerId::new();
        let mut memo = ProgrammerAddressMemo::default();
        let resolver = CountingAddresser {
            generation: 4,
            asked: Cell::new(0),
        };
        let values = stored(3);
        let first = memo.addresses(programmer, ValueLane::Live, &values, &resolver);
        assert_eq!(first.len(), 3);
        assert_eq!(resolver.asked.get(), 3);
        let again = memo.addresses(programmer, ValueLane::Live, &values, &resolver);
        assert_eq!(
            resolver.asked.get(),
            3,
            "the same vector is not asked about again"
        );
        assert!(Arc::ptr_eq(&first, &again));

        let edited = stored(2);
        memo.addresses(programmer, ValueLane::Live, &edited, &resolver);
        assert_eq!(resolver.asked.get(), 5, "an edit is a new vector");

        let repatched = CountingAddresser {
            generation: 5,
            asked: Cell::new(0),
        };
        let after = memo.addresses(programmer, ValueLane::Live, &edited, &repatched);
        assert_eq!(repatched.asked.get(), 2, "a new generation is asked again");
        assert_eq!(after[0].map(|address| address.generation), Some(5));

        memo.addresses(programmer, ValueLane::Preload, &values, &repatched);
        assert_eq!(repatched.asked.get(), 5, "lanes are remembered apart");
        memo.retain_programmers(&HashSet::new());
        assert!(memo.lanes.is_empty());
    }
}

#[cfg(test)]
mod group_memo_tests {
    use super::*;
    use light_core::{AttributeValue, programming::*};
    use light_programmer::GroupDefinition;
    fn generation(members: Vec<FixtureId>) -> Arc<RuntimeGeneration> {
        let groups = vec![GroupDefinition {
            id: "1".into(),
            fixtures: members,
            ..Default::default()
        }];
        Arc::new(RuntimeGeneration::new(
            crate::EngineSnapshot {
                groups: groups.clone().into(),
                ..Default::default()
            },
            Arc::new(parking_lot::RwLock::new(
                light_playback::PlaybackEngine::default(),
            )),
            Arc::new(
                groups
                    .into_iter()
                    .map(|group| (group.id.clone(), group))
                    .collect(),
            ),
            Arc::new(Default::default()),
            Arc::new(Default::default()),
        ))
    }
    #[test]
    fn group_rank_plan_is_shared_until_its_source_or_rank_generation_changes() {
        let a = FixtureId::new();
        let b = FixtureId::new();
        let c = FixtureId::new();
        let generation = generation(vec![a, b]);
        let value = AttributeValue::GroupFamily(Arc::new(GroupFamilyAssignment {
            owner: ProgrammingOwner::Position,
            template: AttributeValue::Position(Arc::new(PositionIntent::Angles {
                pan_degrees: ScalarIntent::Spread(vec![-270.0, 270.0]),
                tilt_degrees: ScalarIntent::Value(0.0),
            })),
            members: std::collections::BTreeMap::from([(
                a.0,
                AttributeValue::Position(Arc::new(PositionIntent::target(
                    TargetReference::Origin,
                    [1.0, 2.0, 3.0],
                ))),
            )]),
        }));
        let source = Arc::new(HashMap::from([(
            "1".into(),
            HashMap::from([(
                ProgrammingOwner::Position.key(),
                GroupProgrammerValue {
                    value,
                    changed_at: Utc::now(),
                    programmer_order: 3,
                    fade: false,
                    fade_millis: None,
                    delay_millis: None,
                },
            )]),
        )]));
        let programmer = ProgrammerId::new();
        let mut memo = ProgrammerAddressMemo::default();
        let addresser = crate::FrameAddresser::new(Arc::clone(generation.slots()));
        let first = memo.group_entries(
            programmer,
            ValueLane::Live,
            &source,
            &generation,
            &addresser,
        );
        let again = memo.group_entries(
            programmer,
            ValueLane::Live,
            &source,
            &generation,
            &addresser,
        );
        assert!(Arc::ptr_eq(&first, &again));
        assert_eq!(first.len(), 2);
        assert!(
            first
                .iter()
                .all(|entry| entry.scoped.value.spread_control_points() == 0
                    && !matches!(entry.scoped.value, AttributeValue::GroupFamily(_)))
        );
        let preload = memo.group_entries(
            programmer,
            ValueLane::Preload,
            &source,
            &generation,
            &addresser,
        );
        assert!(!Arc::ptr_eq(&first, &preload));
        let (master, _) = RuntimeGeneration::with_group_master(&generation, "1", 0.5);
        assert!(Arc::ptr_eq(
            &first,
            &memo.group_entries(programmer, ValueLane::Live, &source, &master, &addresser)
        ));
        let mut snapshot = generation.snapshot().clone();
        snapshot.groups = vec![GroupDefinition {
            id: "1".into(),
            fixtures: vec![b, c],
            ..Default::default()
        }]
        .into();
        let groups = Arc::new(
            snapshot
                .groups
                .iter()
                .map(|group| (group.id.clone(), group.clone()))
                .collect(),
        );
        let changed = RuntimeGeneration::replacing(
            &generation,
            Arc::new(snapshot),
            generation.playback_arc(),
            groups,
            generation.profile_encodings_arc(),
            generation.profile_projections_arc(),
            crate::GroupMasterLevels::Preserved,
        );
        assert_eq!(
            generation.slots().generation(),
            changed.slots().generation(),
            "membership does not change slots"
        );
        let next = memo.group_entries(programmer, ValueLane::Live, &source, &changed, &addresser);
        assert!(!Arc::ptr_eq(&first, &next));
        assert_eq!(
            next.iter().map(|entry| entry.fixture).collect::<Vec<_>>(),
            vec![b, c]
        );
        assert_eq!(
            next[1].scoped.value,
            AttributeValue::Position(Arc::new(PositionIntent::angles(270.0, 0.0)))
        );
        memo.retain_programmers(&HashSet::new());
        assert!(memo.group_lanes.is_empty());
    }
}
