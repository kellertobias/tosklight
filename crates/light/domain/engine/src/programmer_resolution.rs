use crate::{
    ContributionBatch, ContributionSourceId, Engine, EngineContribution,
    ProgrammerTransitionSource, ResolvedContributionIndex, RuntimeGeneration, replaces_source,
    value_for_ordered_position,
};
use chrono::{DateTime, Utc};
use light_core::{AttributeKey, MergeMode, ProgrammerId, TimedValue};
use light_programmer::{GroupProgrammerValue, ProgrammerOutputState};
use std::{cell::RefCell, collections::HashSet};
use std::{collections::HashMap, sync::Arc};

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
        programmers
            .into_iter()
            .flat_map(|programmer| {
                self.resolve_programmer(
                    programmer,
                    generation,
                    now,
                    underlay,
                    sampled,
                    has_replacements,
                )
            })
            .map(EngineContribution::unscaled)
            .collect()
    }

    fn resolve_programmer(
        &self,
        programmer: ProgrammerOutputState,
        generation: &RuntimeGeneration,
        now: DateTime<Utc>,
        underlay: Option<&ResolvedContributionIndex<'_>>,
        sampled: &[ContributionBatch],
        has_replacements: bool,
    ) -> Vec<TimedValue> {
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
            now,
            underlay,
            sampled,
            programmer_id: id,
            priority,
            has_replacements,
            active_transition_keys: RefCell::new(HashSet::new()),
        };
        let mut contributions = resolver.fixture_values(&values, ProgrammerValueSource::Live);
        for action in transient_values.iter() {
            contributions.extend(resolver.fixture_values(
                &action.values,
                ProgrammerValueSource::Transient(&action.source),
            ));
        }
        contributions
            .extend(resolver.fixture_values(&preload_active, ProgrammerValueSource::Preload));
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
        source: ProgrammerValueSource<'_>,
    ) -> Vec<TimedValue> {
        let context = self.source_context(source);
        values
            .iter()
            .filter_map(|value| self.resolve_value(value.clone(), &context))
            .collect()
    }

    fn group_values(
        &self,
        group_values: &GroupValues,
        preload_values: &GroupValues,
    ) -> Vec<TimedValue> {
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
    ) -> Vec<TimedValue> {
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
                        self.resolve_value(value, context)
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

fn programmer_winners(values: Vec<TimedValue>) -> Vec<TimedValue> {
    // Rebuilt every frame from the operator's live edits, so it is sized for what it is about to
    // hold rather than regrown as it fills, and hashed with the desk's hasher rather than SipHash.
    let mut winners =
        rustc_hash::FxHashMap::with_capacity_and_hasher(values.len(), rustc_hash::FxBuildHasher);
    for value in values {
        let key = (value.fixture_id, value.attribute.clone());
        let replace = winners
            .get(&key)
            .is_none_or(|current: &TimedValue| supersedes(&value, current));
        if replace {
            winners.insert(key, value);
        }
    }
    winners
        .into_values()
        .map(|mut value| {
            value.merge_mode = if value.attribute.is_intensity() {
                MergeMode::Htp
            } else {
                MergeMode::Ltp
            };
            value
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
