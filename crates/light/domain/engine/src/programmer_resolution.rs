use crate::{
    ContributionBatch, ContributionSourceId, Engine, EngineContribution,
    ProgrammerTransitionSource, ResolvedContributionIndex, RuntimeGeneration, replaces_source,
    value_for_ordered_position,
};
use chrono::{DateTime, Utc};
use light_core::{
    AttributeKey, FixtureId, FrameAddress, FrameAddressResolver, MergeMode, ProgrammerId,
    TimedValue,
};
use light_programmer::{GroupProgrammerValue, ProgrammerOutputState};
use std::{cell::RefCell, collections::HashSet};
use std::{collections::HashMap, sync::Arc};

/// A stored value on its way to the frame, with where the frame keeps it when that is known.
type Addressed = (TimedValue, Option<FrameAddress>);

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
    }
}

type GroupValues = HashMap<String, HashMap<AttributeKey, GroupProgrammerValue>>;
type GroupAttributes = HashMap<AttributeKey, GroupProgrammerValue>;

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

struct ProgrammerValueResolver<'a> {
    engine: &'a Engine,
    generation: &'a RuntimeGeneration,
    /// Where this generation keeps each pair. Values the memo could not cover — Group and
    /// transient values, built fresh each tick — ask it directly, so every lane keys the same
    /// pair the same way and the later edit still wins.
    addresser: &'a crate::FrameAddresser,
    now: DateTime<Utc>,
    underlay: Option<&'a ResolvedContributionIndex<'a>>,
    sampled: &'a [ContributionBatch],
    programmer_id: ProgrammerId,
    priority: i16,
    has_replacements: bool,
    active_transition_keys: RefCell<HashSet<crate::ProgrammerTransitionKey>>,
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
    pub(crate) fn programmer_contributions(
        &self,
        programmers: Vec<ProgrammerOutputState>,
        generation: &RuntimeGeneration,
        now: DateTime<Utc>,
        underlay: Option<&ResolvedContributionIndex<'_>>,
        sampled: &[ContributionBatch],
    ) -> Vec<EngineContribution> {
        let has_replacements = sampled.iter().any(ContributionBatch::has_replacements);
        let active_programmers = programmers
            .iter()
            .map(|programmer| programmer.id)
            .collect::<HashSet<_>>();
        self.programmer_transitions
            .lock()
            .retain(|key, _| active_programmers.contains(&key.programmer_id));
        self.programmer_addresses
            .lock()
            .retain_programmers(&active_programmers);
        let addresser = crate::FrameAddresser::new(Arc::clone(generation.slots()));
        programmers
            .into_iter()
            .flat_map(|programmer| {
                self.resolve_programmer(
                    programmer,
                    generation,
                    &addresser,
                    now,
                    underlay,
                    sampled,
                    has_replacements,
                )
            })
            .map(|(value, address)| EngineContribution::unscaled(value).at(address))
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
        sampled: &[ContributionBatch],
        has_replacements: bool,
    ) -> Vec<Addressed> {
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
        let resolver = ProgrammerValueResolver {
            engine: self,
            generation,
            addresser,
            now,
            underlay,
            sampled,
            programmer_id: id,
            priority,
            has_replacements,
            active_transition_keys: RefCell::new(HashSet::new()),
        };
        let (live_addresses, preload_addresses) = {
            let mut memo = self.programmer_addresses.lock();
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
        let active_transition_keys = resolver.active_transition_keys.into_inner();
        self.programmer_transitions
            .lock()
            .retain(|key, _| key.programmer_id != id || active_transition_keys.contains(key));
        programmer_winners(contributions)
    }

    fn resolve_programmer_fade(
        &self,
        value: TimedValue,
        generation: &RuntimeGeneration,
        now: DateTime<Utc>,
        underlay: Option<&ResolvedContributionIndex<'_>>,
        programmer_id: ProgrammerId,
        source: ProgrammerTransitionSource,
    ) -> TimedValue {
        let group_color_underlay = (*value.attribute.0 == *"color")
            .then(|| self.group_color_for_fixture(generation, value.fixture_id))
            .flatten()
            .map(|(value, _)| value);
        let underlying = group_color_underlay.as_ref().or_else(|| {
            underlay
                .and_then(|values| values.value(value.fixture_id, &value.attribute))
                .or_else(|| generation.default_value(value.fixture_id, &value.attribute))
        });
        // An indexed or control attribute names a state, not a level: play mode, media folder and
        // file, gobo and colour wheel slots. Interpolating one walks the operator's selection
        // through every slot in between and only arrives at the chosen one when the Programmer
        // fade ends, which is why an Audio Player took the fade time to change source or transport.
        // Cue transitions already snap these; the Programmer now agrees, whatever the profile's
        // own channel flag says.
        let snap = generation.attribute_is_snap(value.fixture_id, &value.attribute)
            || light_playback::attribute_uses_snap_transition(&value.attribute);
        self.faded_programmer_value(value, now, underlying, programmer_id, source, snap)
    }
}

impl ProgrammerValueResolver<'_> {
    /// Borrowed rather than owned: the Programmer's stored values are shared with every other
    /// reader, so a value is copied here only if it survives to contribute.
    fn fixture_values(
        &self,
        values: &[TimedValue],
        addresses: &[Option<FrameAddress>],
        source: ProgrammerValueSource<'_>,
    ) -> Vec<Addressed> {
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
                    .map(|value| (value, address))
            })
            .collect()
    }

    fn group_values(
        &self,
        group_values: &GroupValues,
        preload_values: &GroupValues,
    ) -> Vec<Addressed> {
        let mut resolved = Vec::new();
        for (group_id, attributes) in group_values {
            resolved.extend(self.one_group(
                group_id,
                attributes,
                ProgrammerValueSource::Group(group_id),
            ));
        }
        for (group_id, attributes) in preload_values {
            resolved.extend(self.one_group(
                group_id,
                attributes,
                ProgrammerValueSource::PreloadGroup(group_id),
            ));
        }
        resolved
    }

    fn one_group(
        &self,
        group_id: &str,
        attributes: &GroupAttributes,
        source: ProgrammerValueSource<'_>,
    ) -> Vec<Addressed> {
        let Some(ranking) = self.generation.group_ranking(group_id) else {
            return Vec::new();
        };
        let context = self.source_context(source);
        let count = ranking.rank_count;
        ranking
            .ordered_fixture_ids
            .iter()
            .copied()
            .flat_map(|fixture_id| {
                let rank = ranking.rank_by_fixture[&fixture_id];
                attributes.iter().filter_map({
                    let context = &context;
                    move |(attribute, scoped)| {
                        let value = TimedValue {
                            fixture_id,
                            attribute: attribute.clone(),
                            value: value_for_ordered_position(&scoped.value, rank, count),
                            priority: self.priority,
                            changed_at: scoped.changed_at,
                            programmer_order: scoped.programmer_order,
                            merge_mode: MergeMode::Ltp,
                            fade: scoped.fade,
                            fade_millis: scoped.fade_millis,
                            delay_millis: scoped.delay_millis,
                        };
                        let address = self.addresser.frame_address(fixture_id, attribute);
                        self.resolve_value(value, context)
                            .map(|value| (value, address))
                    }
                })
            })
            .collect()
    }

    fn source_context(&self, source: ProgrammerValueSource<'_>) -> SourceContext {
        SourceContext {
            transition: source.transition(),
            replacement: self
                .has_replacements
                .then(|| source.replacement(self.programmer_id)),
        }
    }

    fn resolve_value(&self, value: TimedValue, source: &SourceContext) -> Option<TimedValue> {
        let transition_key = self.engine.programmer_transition_key(
            &value,
            self.programmer_id,
            source.transition.clone(),
        );
        self.active_transition_keys
            .borrow_mut()
            .insert(transition_key.clone());
        let value = if value.fade {
            self.engine.resolve_programmer_fade(
                value,
                self.generation,
                self.now,
                self.underlay,
                self.programmer_id,
                source.transition.clone(),
            )
        } else {
            self.engine
                .track_immediate_programmer_value(transition_key, &value);
            value
        };
        let replaced = source
            .replacement
            .as_ref()
            .is_some_and(|source| replaces_source(self.sampled, source, &value));
        (!replaced).then_some(value)
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
    if value.programmer_order > 0 && current.programmer_order > 0 {
        return value.programmer_order > current.programmer_order;
    }
    (value.changed_at, value.programmer_order) > (current.changed_at, current.programmer_order)
}

/// How the winners map tells one pair from another: by number when the frame has one, by name
/// otherwise. Every lane asks the same generation, so one pair never gets both keys.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum WinnerKey {
    Address(FrameAddress),
    Name(FixtureId, AttributeKey),
}

fn programmer_winners(values: Vec<Addressed>) -> Vec<Addressed> {
    // Rebuilt every frame from the operator's live edits, so it is sized for what it is about to
    // hold rather than regrown as it fills, and hashed with the desk's hasher rather than SipHash.
    let mut winners =
        rustc_hash::FxHashMap::with_capacity_and_hasher(values.len(), rustc_hash::FxBuildHasher);
    for (value, address) in values {
        let key = match address {
            Some(address) => WinnerKey::Address(address),
            None => WinnerKey::Name(value.fixture_id, value.attribute.clone()),
        };
        let replace = winners
            .get(&key)
            .is_none_or(|(current, _): &Addressed| supersedes(&value, current));
        if replace {
            winners.insert(key, (value, address));
        }
    }
    winners
        .into_values()
        .map(|(mut value, address)| {
            value.merge_mode = if value.attribute.is_intensity() {
                MergeMode::Htp
            } else {
                MergeMode::Ltp
            };
            (value, address)
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
