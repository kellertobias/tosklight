use chrono::{DateTime, Utc};
use light_core::{AttributeKey, AttributeValue, FixtureId, MergeMode, TimedValue};
use light_playback::{AutomaticPlaybackTransition, PlaybackContribution};
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
    pub(crate) values: HashMap<(FixtureId, AttributeKey), AttributeValue>,
    pub(crate) changed_at: HashMap<(FixtureId, AttributeKey), DateTime<Utc>>,
    pub(crate) sequence_masters: HashMap<(FixtureId, AttributeKey), ApplicableSequenceMaster>,
    pub(crate) automatic_playback_transitions: Vec<AutomaticPlaybackTransition>,
}

#[derive(Default)]
pub(crate) struct EngineContributionResolver {
    winners: HashMap<FixtureId, HashMap<AttributeKey, EngineWinner>>,
}

impl EngineContributionResolver {
    pub(crate) fn new(values: impl IntoIterator<Item = EngineContribution>) -> Self {
        let mut resolver = Self::default();
        resolver.extend(values);
        resolver
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

    pub(crate) fn extend_borrowed_samples<'a>(
        &mut self,
        samples: impl IntoIterator<Item = &'a crate::ContributionSample>,
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
        let winners = self.winners.entry(fixture_id).or_default();
        let replace = winners.get(attribute).is_none_or(|current| {
            borrowed_winner_wins(
                value,
                priority,
                changed_at,
                merge_mode,
                transition_ordinal,
                current,
            )
        });
        if replace {
            winners.insert(
                attribute.clone(),
                EngineWinner {
                    value: value.clone(),
                    priority,
                    changed_at,
                    merge_mode,
                    transition_ordinal,
                    sequence_master,
                },
            );
        }
    }

    pub(crate) fn values(&self) -> HashMap<(FixtureId, AttributeKey), AttributeValue> {
        self.winners
            .iter()
            .flat_map(|(fixture_id, attributes)| {
                attributes.iter().map(move |(attribute, winner)| {
                    ((*fixture_id, attribute.clone()), winner.value.clone())
                })
            })
            .collect()
    }

    pub(crate) fn finish(self) -> ResolvedAttributes {
        let mut resolved = ResolvedAttributes::default();
        for (fixture_id, attributes) in self.winners {
            for (attribute, winner) in attributes {
                let key = (fixture_id, attribute);
                resolved.changed_at.insert(key.clone(), winner.changed_at);
                resolved.values.insert(key.clone(), winner.value);
                if let Some(sequence_master) = winner.sequence_master {
                    resolved.sequence_masters.insert(key, sequence_master);
                }
            }
        }
        resolved
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
        let candidate = EngineWinner {
            value,
            priority,
            changed_at,
            merge_mode,
            transition_ordinal,
            sequence_master,
        };
        match self.winners.entry(fixture_id).or_default().entry(attribute) {
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
}

struct EngineWinner {
    value: AttributeValue,
    priority: i16,
    changed_at: DateTime<Utc>,
    merge_mode: MergeMode,
    transition_ordinal: Option<u64>,
    sequence_master: Option<ApplicableSequenceMaster>,
}

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
        let resolved = EngineContributionResolver::new([
            playback_value(fixture_id, 0.8, MergeMode::Ltp, at, 4),
            playback_value(fixture_id, 0.2, MergeMode::Ltp, at, 5),
        ])
        .finish();
        assert_eq!(
            resolved.values[&(fixture_id, AttributeKey::intensity())],
            AttributeValue::Normalized(0.2)
        );
    }

    #[test]
    fn equal_timestamp_playback_htp_ignores_transition_order() {
        let fixture_id = FixtureId::new();
        let at = Utc::now();
        let resolved = EngineContributionResolver::new([
            playback_value(fixture_id, 0.8, MergeMode::Htp, at, 4),
            playback_value(fixture_id, 0.2, MergeMode::Htp, at, 5),
        ])
        .finish();
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
        let resolved = EngineContributionResolver::new([value(0.8), value(0.2)]).finish();
        assert_eq!(
            resolved.values[&(fixture_id, AttributeKey("pan".into()))],
            AttributeValue::Normalized(0.8)
        );
    }
}
