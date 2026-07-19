use crate::{
    ContributionBatch, ContributionSourceId, Engine, EngineContribution,
    ProgrammerTransitionSource, ResolvedContributionIndex, RuntimeGeneration, replaces_source,
    value_for_ordered_position,
};
use chrono::{DateTime, Utc};
use light_core::{AttributeKey, MergeMode, ProgrammerId, TimedValue};
use light_programmer::{GroupDefinition, GroupProgrammerValue, ProgrammerState, resolve_group};
use std::collections::HashMap;
use std::sync::Arc;

type GroupValues = HashMap<String, HashMap<AttributeKey, GroupProgrammerValue>>;
type GroupAttributes = HashMap<AttributeKey, GroupProgrammerValue>;

struct SourcedProgrammerValue {
    value: TimedValue,
    source: ProgrammerTransitionSource,
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
        programmers
            .into_iter()
            .flat_map(|programmer| {
                self.resolve_programmer(programmer, generation, now, groups, underlay, sampled)
            })
            .collect()
    }

    fn resolve_programmer(
        &self,
        programmer: ProgrammerState,
        generation: &RuntimeGeneration,
        now: DateTime<Utc>,
        groups: &HashMap<String, GroupDefinition>,
        underlay: Option<&ResolvedContributionIndex<'_>>,
        sampled: &[ContributionBatch],
    ) -> Vec<EngineContribution> {
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
        let mut contributions = values
            .into_iter()
            .map(|value| (value, ProgrammerTransitionSource::Programmer))
            .chain(transient_values.into_iter().flat_map(|action| {
                let source: Arc<str> = action.source.into();
                action.values.into_iter().map(move |value| {
                    (
                        value,
                        ProgrammerTransitionSource::Transient(Arc::clone(&source)),
                    )
                })
            }))
            .chain(
                preload_active
                    .into_iter()
                    .map(|value| (value, ProgrammerTransitionSource::Preload)),
            )
            .map(|(value, source)| SourcedProgrammerValue {
                value: self.resolve_programmer_fade(
                    value,
                    generation,
                    now,
                    underlay,
                    id,
                    source.clone(),
                ),
                source,
            })
            .collect::<Vec<_>>();
        contributions.extend(self.resolve_group_programming(
            group_values,
            preload_group_active,
            generation,
            now,
            groups,
            underlay,
            id,
            priority,
        ));
        if sampled.iter().any(ContributionBatch::has_replacements) {
            contributions.retain(|contribution| {
                let source = contribution_source(id, &contribution.source);
                !replaces_source(sampled, &source, &contribution.value)
            });
        }
        programmer_winners(contributions)
            .into_iter()
            .map(|winner| EngineContribution::unscaled(winner.value))
            .collect()
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
        if !value.fade {
            return value;
        }
        let underlying =
            underlay.and_then(|values| values.value(value.fixture_id, &value.attribute));
        let snap = generation.attribute_is_snap(value.fixture_id, &value.attribute);
        self.faded_programmer_value(value, now, underlying, programmer_id, source, snap)
    }

    #[allow(clippy::too_many_arguments)]
    fn resolve_group_programming(
        &self,
        group_values: GroupValues,
        preload_values: GroupValues,
        generation: &RuntimeGeneration,
        now: DateTime<Utc>,
        groups: &HashMap<String, GroupDefinition>,
        underlay: Option<&ResolvedContributionIndex<'_>>,
        programmer_id: ProgrammerId,
        priority: i16,
    ) -> Vec<SourcedProgrammerValue> {
        group_values
            .into_iter()
            .map(|(id, values)| (id.clone(), values, ProgrammerTransitionSource::Group(id)))
            .chain(preload_values.into_iter().map(|(id, values)| {
                (
                    id.clone(),
                    values,
                    ProgrammerTransitionSource::PreloadGroup(id),
                )
            }))
            .flat_map(|(group_id, attributes, source)| {
                self.resolve_one_group(
                    &group_id,
                    attributes,
                    source,
                    generation,
                    now,
                    groups,
                    underlay,
                    programmer_id,
                    priority,
                )
            })
            .collect()
    }

    #[allow(clippy::too_many_arguments)]
    fn resolve_one_group(
        &self,
        group_id: &str,
        attributes: GroupAttributes,
        source: ProgrammerTransitionSource,
        generation: &RuntimeGeneration,
        now: DateTime<Utc>,
        groups: &HashMap<String, GroupDefinition>,
        underlay: Option<&ResolvedContributionIndex<'_>>,
        programmer_id: ProgrammerId,
        priority: i16,
    ) -> Vec<SourcedProgrammerValue> {
        let Ok(fixtures) = resolve_group(group_id, groups) else {
            return Vec::new();
        };
        let count = fixtures.len();
        fixtures
            .into_iter()
            .enumerate()
            .flat_map(|(index, fixture_id)| {
                attributes.iter().map({
                    let source = source.clone();
                    move |(attribute, scoped)| {
                        let value = TimedValue {
                            fixture_id,
                            attribute: attribute.clone(),
                            value: value_for_ordered_position(&scoped.value, index, count),
                            priority,
                            changed_at: scoped.changed_at,
                            programmer_order: scoped.programmer_order,
                            merge_mode: MergeMode::Ltp,
                            fade: scoped.fade,
                            fade_millis: scoped.fade_millis,
                            delay_millis: scoped.delay_millis,
                        };
                        SourcedProgrammerValue {
                            value: self.resolve_programmer_fade(
                                value,
                                generation,
                                now,
                                underlay,
                                programmer_id,
                                source.clone(),
                            ),
                            source: source.clone(),
                        }
                    }
                })
            })
            .collect()
    }
}

fn programmer_winners(values: Vec<SourcedProgrammerValue>) -> Vec<SourcedProgrammerValue> {
    let mut winners = HashMap::new();
    for value in values {
        let key = (value.value.fixture_id, value.value.attribute.clone());
        let replace = winners
            .get(&key)
            .is_none_or(|current: &SourcedProgrammerValue| {
                (value.value.changed_at, value.value.programmer_order)
                    > (current.value.changed_at, current.value.programmer_order)
            });
        if replace {
            winners.insert(key, value);
        }
    }
    winners
        .into_values()
        .map(|mut value| {
            value.value.merge_mode = if value.value.attribute.is_intensity() {
                MergeMode::Htp
            } else {
                MergeMode::Ltp
            };
            value
        })
        .collect()
}

fn contribution_source(
    programmer_id: ProgrammerId,
    source: &ProgrammerTransitionSource,
) -> ContributionSourceId {
    match source {
        ProgrammerTransitionSource::Programmer => ContributionSourceId::programmer(programmer_id),
        ProgrammerTransitionSource::Preload => ContributionSourceId::preload(programmer_id),
        ProgrammerTransitionSource::Transient(source) => {
            ContributionSourceId::programmer_transient(programmer_id, Arc::clone(source))
        }
        ProgrammerTransitionSource::Group(group_id) => {
            ContributionSourceId::programmer_group(programmer_id, group_id.as_str())
        }
        ProgrammerTransitionSource::PreloadGroup(group_id) => {
            ContributionSourceId::preload_group(programmer_id, group_id.as_str())
        }
    }
}
