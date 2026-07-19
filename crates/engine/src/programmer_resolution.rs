use crate::{
    ContributionBatch, ContributionSourceId, Engine, EngineContribution,
    ProgrammerTransitionSource, ResolvedContributionIndex, RuntimeGeneration, replaces_source,
    value_for_ordered_position,
};
use chrono::{DateTime, Utc};
use light_core::{AttributeKey, MergeMode, ProgrammerId, TimedValue};
use light_programmer::{GroupDefinition, GroupProgrammerValue, ProgrammerState, resolve_group};
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
    transition: Option<ProgrammerTransitionSource>,
    replacement: Option<ContributionSourceId>,
}

struct ProgrammerValueResolver<'a> {
    engine: &'a Engine,
    generation: &'a RuntimeGeneration,
    now: DateTime<Utc>,
    groups: &'a HashMap<String, GroupDefinition>,
    underlay: Option<&'a ResolvedContributionIndex<'a>>,
    sampled: &'a [ContributionBatch],
    programmer_id: ProgrammerId,
    priority: i16,
    has_replacements: bool,
}

pub(crate) fn programmers_need_underlay(programmers: &[ProgrammerState]) -> bool {
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
            .chain(&programmer.preload_active)
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
        programmers: Vec<ProgrammerState>,
        generation: &RuntimeGeneration,
        now: DateTime<Utc>,
        groups: &HashMap<String, GroupDefinition>,
        underlay: Option<&ResolvedContributionIndex<'_>>,
        sampled: &[ContributionBatch],
    ) -> Vec<EngineContribution> {
        let has_replacements = sampled.iter().any(ContributionBatch::has_replacements);
        programmers
            .into_iter()
            .flat_map(|programmer| {
                self.resolve_programmer(
                    programmer,
                    generation,
                    now,
                    groups,
                    underlay,
                    sampled,
                    has_replacements,
                )
            })
            .map(EngineContribution::unscaled)
            .collect()
    }

    #[allow(clippy::too_many_arguments)]
    fn resolve_programmer(
        &self,
        programmer: ProgrammerState,
        generation: &RuntimeGeneration,
        now: DateTime<Utc>,
        groups: &HashMap<String, GroupDefinition>,
        underlay: Option<&ResolvedContributionIndex<'_>>,
        sampled: &[ContributionBatch],
        has_replacements: bool,
    ) -> Vec<TimedValue> {
        let ProgrammerState {
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
            groups,
            underlay,
            sampled,
            programmer_id: id,
            priority,
            has_replacements,
        };
        let mut contributions = resolver.fixture_values(values, ProgrammerValueSource::Live);
        for action in transient_values {
            contributions.extend(resolver.fixture_values(
                action.values,
                ProgrammerValueSource::Transient(&action.source),
            ));
        }
        contributions
            .extend(resolver.fixture_values(preload_active, ProgrammerValueSource::Preload));
        contributions.extend(resolver.group_values(group_values, preload_group_active));
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
        let underlying =
            underlay.and_then(|values| values.value(value.fixture_id, &value.attribute));
        let snap = generation.attribute_is_snap(value.fixture_id, &value.attribute);
        self.faded_programmer_value(value, now, underlying, programmer_id, source, snap)
    }
}

impl ProgrammerValueResolver<'_> {
    fn fixture_values(
        &self,
        values: Vec<TimedValue>,
        source: ProgrammerValueSource<'_>,
    ) -> Vec<TimedValue> {
        let context = self.source_context(source, values.iter().any(|value| value.fade));
        values
            .into_iter()
            .filter_map(|value| self.resolve_value(value, &context))
            .collect()
    }

    fn group_values(
        &self,
        group_values: GroupValues,
        preload_values: GroupValues,
    ) -> Vec<TimedValue> {
        let mut resolved = Vec::new();
        for (group_id, attributes) in group_values {
            resolved.extend(self.one_group(
                &group_id,
                attributes,
                ProgrammerValueSource::Group(&group_id),
            ));
        }
        for (group_id, attributes) in preload_values {
            resolved.extend(self.one_group(
                &group_id,
                attributes,
                ProgrammerValueSource::PreloadGroup(&group_id),
            ));
        }
        resolved
    }

    fn one_group(
        &self,
        group_id: &str,
        attributes: GroupAttributes,
        source: ProgrammerValueSource<'_>,
    ) -> Vec<TimedValue> {
        let Ok(fixtures) = resolve_group(group_id, self.groups) else {
            return Vec::new();
        };
        let context =
            self.source_context(source, attributes.values().any(|attribute| attribute.fade));
        let count = fixtures.len();
        fixtures
            .into_iter()
            .enumerate()
            .flat_map(|(index, fixture_id)| {
                attributes.iter().filter_map({
                    let context = &context;
                    move |(attribute, scoped)| {
                        let value = TimedValue {
                            fixture_id,
                            attribute: attribute.clone(),
                            value: value_for_ordered_position(&scoped.value, index, count),
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

    fn source_context(&self, source: ProgrammerValueSource<'_>, fades: bool) -> SourceContext {
        SourceContext {
            transition: fades.then(|| source.transition()),
            replacement: self
                .has_replacements
                .then(|| source.replacement(self.programmer_id)),
        }
    }

    fn resolve_value(&self, value: TimedValue, source: &SourceContext) -> Option<TimedValue> {
        let value = if value.fade {
            self.engine.resolve_programmer_fade(
                value,
                self.generation,
                self.now,
                self.underlay,
                self.programmer_id,
                source
                    .transition
                    .clone()
                    .expect("faded sources have transition identity"),
            )
        } else {
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

fn programmer_winners(values: Vec<TimedValue>) -> Vec<TimedValue> {
    let mut winners = HashMap::new();
    for value in values {
        let key = (value.fixture_id, value.attribute.clone());
        let replace = winners.get(&key).is_none_or(|current: &TimedValue| {
            (value.changed_at, value.programmer_order)
                > (current.changed_at, current.programmer_order)
        });
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
