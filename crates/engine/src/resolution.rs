use std::{collections::HashMap, sync::atomic::Ordering};

use chrono::{DateTime, Utc};
use light_core::{AttributeKey, AttributeValue, MergeMode, ProgrammerId, TimedValue};
use light_playback::{
    ActivePlayback, AutomaticPlaybackTransition, MoveInBlackCandidate, PlaybackTickResult,
};
use light_programmer::{GroupDefinition, GroupProgrammerValue, ProgrammerState, resolve_group};

use super::{
    Engine, EngineContribution, EngineSnapshot, ProgrammerTransitionSource, ResolvedAttributes,
    resolve_engine_contributions, snapshot_attribute_is_snap, value_for_ordered_position,
};

struct PlaybackResolution {
    contributions: Vec<EngineContribution>,
    move_in_black_candidates: Vec<MoveInBlackCandidate>,
    active_playbacks: Vec<ActivePlayback>,
    automatic_transitions: Vec<AutomaticPlaybackTransition>,
}

type GroupValues = HashMap<String, HashMap<AttributeKey, GroupProgrammerValue>>;
type GroupAttributes = HashMap<AttributeKey, GroupProgrammerValue>;

impl Engine {
    /// Advance scheduler-owned runtime exactly once on the authoritative output path.
    pub(super) fn resolved_attributes_for_render(
        &self,
        snapshot: &EngineSnapshot,
        now: DateTime<Utc>,
    ) -> ResolvedAttributes {
        self.resolve_attributes(snapshot, now, true)
    }

    /// Read the current projection without consuming an automatic transition before output can
    /// return it to the application boundary.
    pub(super) fn resolved_attributes_at(
        &self,
        snapshot: &EngineSnapshot,
        now: DateTime<Utc>,
    ) -> ResolvedAttributes {
        self.resolve_attributes(snapshot, now, false)
    }

    fn resolve_attributes(
        &self,
        snapshot: &EngineSnapshot,
        now: DateTime<Utc>,
        advance_playback: bool,
    ) -> ResolvedAttributes {
        let mut playback = self.resolve_playback(snapshot, now, advance_playback);
        let underlay = resolve_engine_contributions(playback.contributions.clone()).values;
        let groups = group_index(snapshot);
        playback
            .contributions
            .extend(self.programmer_contributions(snapshot, now, &groups, &underlay));
        playback
            .contributions
            .extend(group_contributions(snapshot, &groups, now));
        let base = resolve_engine_contributions(playback.contributions.clone());
        playback.contributions.extend(
            self.move_in_black_contributions(
                snapshot,
                playback.move_in_black_candidates,
                &playback.active_playbacks,
                &base.values,
                now,
            )
            .into_iter()
            .map(EngineContribution::unscaled),
        );
        let mut resolved = resolve_engine_contributions(playback.contributions);
        resolved.automatic_playback_transitions = playback.automatic_transitions;
        resolved
    }

    fn resolve_playback(
        &self,
        snapshot: &EngineSnapshot,
        now: DateTime<Utc>,
        advance: bool,
    ) -> PlaybackResolution {
        let timecode = self.timecode_frame.load(Ordering::Relaxed);
        let mut playback = self.playback.write();
        let transitions = if advance {
            let PlaybackTickResult { transitions } =
                playback.tick(now, (timecode != u64::MAX).then_some(timecode));
            transitions
        } else {
            Vec::new()
        };
        let contributions = playback
            .contributions_with_context_at(now, |fixture_id, attribute| {
                snapshot_attribute_is_snap(snapshot, fixture_id, attribute)
            })
            .into_iter()
            .map(EngineContribution::from)
            .collect();
        PlaybackResolution {
            contributions,
            move_in_black_candidates: playback.move_in_black_candidates(),
            active_playbacks: playback.runtime(),
            automatic_transitions: transitions,
        }
    }

    fn programmer_contributions(
        &self,
        snapshot: &EngineSnapshot,
        now: DateTime<Utc>,
        groups: &HashMap<String, GroupDefinition>,
        underlay: &HashMap<(light_core::FixtureId, AttributeKey), AttributeValue>,
    ) -> Vec<EngineContribution> {
        self.programmers
            .active()
            .into_iter()
            .flat_map(|programmer| {
                self.resolve_programmer(programmer, snapshot, now, groups, underlay)
            })
            .map(EngineContribution::unscaled)
            .collect()
    }

    fn resolve_programmer(
        &self,
        programmer: ProgrammerState,
        snapshot: &EngineSnapshot,
        now: DateTime<Utc>,
        groups: &HashMap<String, GroupDefinition>,
        underlay: &HashMap<(light_core::FixtureId, AttributeKey), AttributeValue>,
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
        let mut contributions = values
            .into_iter()
            .map(|value| (value, ProgrammerTransitionSource::Programmer))
            .chain(
                transient_values
                    .into_iter()
                    .flat_map(|action| action.values)
                    .map(|value| (value, ProgrammerTransitionSource::Programmer)),
            )
            .chain(
                preload_active
                    .into_iter()
                    .map(|value| (value, ProgrammerTransitionSource::Preload)),
            )
            .map(|(value, source)| {
                self.resolve_programmer_fade(value, snapshot, now, underlay, id, source)
            })
            .collect::<Vec<_>>();
        contributions.extend(self.resolve_group_programming(
            group_values,
            preload_group_active,
            snapshot,
            now,
            groups,
            underlay,
            id,
            priority,
        ));
        programmer_winners(contributions)
    }

    fn resolve_programmer_fade(
        &self,
        value: TimedValue,
        snapshot: &EngineSnapshot,
        now: DateTime<Utc>,
        underlay: &HashMap<(light_core::FixtureId, AttributeKey), AttributeValue>,
        programmer_id: ProgrammerId,
        source: ProgrammerTransitionSource,
    ) -> TimedValue {
        if !value.fade {
            return value;
        }
        let underlying = underlay.get(&(value.fixture_id, value.attribute.clone()));
        let snap = snapshot_attribute_is_snap(snapshot, value.fixture_id, &value.attribute);
        self.faded_programmer_value(value, now, underlying, programmer_id, source, snap)
    }

    #[allow(clippy::too_many_arguments)]
    fn resolve_group_programming(
        &self,
        group_values: GroupValues,
        preload_values: GroupValues,
        snapshot: &EngineSnapshot,
        now: DateTime<Utc>,
        groups: &HashMap<String, GroupDefinition>,
        underlay: &HashMap<(light_core::FixtureId, AttributeKey), AttributeValue>,
        programmer_id: ProgrammerId,
        priority: i16,
    ) -> Vec<TimedValue> {
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
                    snapshot,
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
        snapshot: &EngineSnapshot,
        now: DateTime<Utc>,
        groups: &HashMap<String, GroupDefinition>,
        underlay: &HashMap<(light_core::FixtureId, AttributeKey), AttributeValue>,
        programmer_id: ProgrammerId,
        priority: i16,
    ) -> Vec<TimedValue> {
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
                        self.resolve_programmer_fade(
                            value,
                            snapshot,
                            now,
                            underlay,
                            programmer_id,
                            source.clone(),
                        )
                    }
                })
            })
            .collect()
    }
}

fn group_index(snapshot: &EngineSnapshot) -> HashMap<String, GroupDefinition> {
    snapshot
        .groups
        .iter()
        .map(|group| (group.id.clone(), group.clone()))
        .collect()
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

fn group_contributions(
    snapshot: &EngineSnapshot,
    groups: &HashMap<String, GroupDefinition>,
    now: DateTime<Utc>,
) -> Vec<EngineContribution> {
    snapshot
        .groups
        .iter()
        .flat_map(|group| {
            resolve_group(&group.id, groups)
                .unwrap_or_default()
                .into_iter()
                .flat_map(move |fixture_id| {
                    group.programming.iter().map(move |(attribute, value)| {
                        EngineContribution::unscaled(TimedValue {
                            fixture_id,
                            attribute: attribute.clone(),
                            value: value.clone(),
                            priority: 0,
                            changed_at: now,
                            programmer_order: 0,
                            merge_mode: if attribute.is_intensity() {
                                MergeMode::Htp
                            } else {
                                MergeMode::Ltp
                            },
                            fade: false,
                            fade_millis: None,
                            delay_millis: None,
                        })
                    })
                })
        })
        .collect()
}
