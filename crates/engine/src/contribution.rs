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
    if points.len() == 1 || count <= 1 {
        return AttributeValue::Normalized(points[0]);
    }
    let position = index as f32 * (points.len() - 1) as f32 / (count - 1) as f32;
    let left = position.floor() as usize;
    let right = position.ceil() as usize;
    let progress = position - left as f32;
    AttributeValue::Normalized(points[left] + (points[right] - points[left]) * progress)
}

pub(crate) type ApplicableSequenceMaster = crate::ContributionSequenceMaster;

pub(crate) struct EngineContribution {
    value: TimedValue,
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
        let replace = self
            .winners
            .get(&key)
            .is_none_or(|current| contribution_wins(value, current.value()));
        if replace {
            self.winners.insert(key, candidate);
        }
    }
}

impl EngineContribution {
    pub(crate) fn unscaled(value: TimedValue) -> Self {
        Self {
            value,
            sequence_master: None,
        }
    }
}

impl EngineContribution {
    pub(crate) fn from_playback(contribution: PlaybackContribution) -> Self {
        Self {
            value: contribution.value,
            sequence_master: Some(ApplicableSequenceMaster::new(
                contribution.source,
                contribution.sequence_master,
            )),
        }
    }
}

#[derive(Default)]
pub(crate) struct ResolvedAttributes {
    pub(crate) values: HashMap<(FixtureId, AttributeKey), AttributeValue>,
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

    pub(crate) fn add_unscaled(&mut self, value: TimedValue) {
        self.add(EngineContribution::unscaled(value));
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
            fixture_id, attribute, value, priority, changed_at, merge_mode, None,
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
        sequence_master: Option<ApplicableSequenceMaster>,
    ) {
        let winners = self.winners.entry(fixture_id).or_default();
        let replace = winners.get(attribute).is_none_or(|current| {
            borrowed_winner_wins(value, priority, changed_at, merge_mode, current)
        });
        if replace {
            winners.insert(
                attribute.clone(),
                EngineWinner {
                    value: value.clone(),
                    priority,
                    changed_at,
                    merge_mode,
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
    sequence_master: Option<ApplicableSequenceMaster>,
}

fn contribution_wins(candidate: &TimedValue, current: &TimedValue) -> bool {
    if candidate.priority != current.priority {
        candidate.priority > current.priority
    } else if candidate.merge_mode == MergeMode::Htp {
        candidate.value.normalized().unwrap_or(0.0) > current.value.normalized().unwrap_or(0.0)
    } else {
        candidate.changed_at > current.changed_at
    }
}

fn winner_wins(candidate: &EngineWinner, current: &EngineWinner) -> bool {
    if candidate.priority != current.priority {
        candidate.priority > current.priority
    } else if candidate.merge_mode == MergeMode::Htp {
        candidate.value.normalized().unwrap_or(0.0) > current.value.normalized().unwrap_or(0.0)
    } else {
        candidate.changed_at > current.changed_at
    }
}

fn borrowed_winner_wins(
    value: &AttributeValue,
    priority: i16,
    changed_at: DateTime<Utc>,
    merge_mode: MergeMode,
    current: &EngineWinner,
) -> bool {
    if priority != current.priority {
        priority > current.priority
    } else if merge_mode == MergeMode::Htp {
        value.normalized().unwrap_or(0.0) > current.value.normalized().unwrap_or(0.0)
    } else {
        changed_at > current.changed_at
    }
}
