use light_core::{AttributeKey, AttributeValue, FixtureId, MergeMode, TimedValue};
use light_playback::{AutomaticPlaybackTransition, PlaybackContribution, SequenceMasterSource};
use std::collections::HashMap;

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

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ApplicableSequenceMaster {
    pub(crate) source: SequenceMasterSource,
    pub(crate) scale: f32,
}

#[derive(Clone)]
pub(crate) struct EngineContribution {
    value: TimedValue,
    sequence_master: Option<ApplicableSequenceMaster>,
}

impl EngineContribution {
    pub(crate) fn unscaled(value: TimedValue) -> Self {
        Self {
            value,
            sequence_master: None,
        }
    }
}

impl From<PlaybackContribution> for EngineContribution {
    fn from(contribution: PlaybackContribution) -> Self {
        Self {
            value: contribution.value,
            sequence_master: Some(ApplicableSequenceMaster {
                source: contribution.source,
                scale: contribution.sequence_master,
            }),
        }
    }
}

#[derive(Default)]
pub(crate) struct ResolvedAttributes {
    pub(crate) values: HashMap<(FixtureId, AttributeKey), AttributeValue>,
    pub(crate) sequence_masters: HashMap<(FixtureId, AttributeKey), ApplicableSequenceMaster>,
    pub(crate) automatic_playback_transitions: Vec<AutomaticPlaybackTransition>,
}

pub(crate) fn resolve_engine_contributions(
    values: impl IntoIterator<Item = EngineContribution>,
) -> ResolvedAttributes {
    let mut winners: HashMap<(FixtureId, AttributeKey), EngineContribution> = HashMap::new();
    for candidate in values {
        let key = (
            candidate.value.fixture_id,
            candidate.value.attribute.clone(),
        );
        let replace = match winners.get(&key) {
            None => true,
            Some(current) if candidate.value.priority != current.value.priority => {
                candidate.value.priority > current.value.priority
            }
            Some(current) if candidate.value.merge_mode == MergeMode::Htp => {
                candidate.value.value.normalized().unwrap_or(0.0)
                    > current.value.value.normalized().unwrap_or(0.0)
            }
            Some(current) => candidate.value.changed_at > current.value.changed_at,
        };
        if replace {
            winners.insert(key, candidate);
        }
    }
    let mut resolved = ResolvedAttributes::default();
    for (key, winner) in winners {
        resolved.values.insert(key.clone(), winner.value.value);
        if let Some(sequence_master) = winner.sequence_master {
            resolved.sequence_masters.insert(key, sequence_master);
        }
    }
    resolved
}
