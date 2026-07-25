use crate::{ResolvedAttributes, contribution::ApplicableSequenceMaster};
use light_core::{AttributeKey, AttributeValue, FixtureId};
use std::collections::HashMap;

/// Per-fixture view of one render's flat resolved maps.
///
/// Building this once avoids scanning every resolved fixture for every profile head. Values stay
/// borrowed until a head needs its private mutable copy for safe-state and color projection.
pub(crate) struct ProfileValueIndex<'a> {
    values: HashMap<FixtureId, Vec<(&'a AttributeKey, &'a AttributeValue)>>,
    sequence_masters: HashMap<FixtureId, Vec<(&'a AttributeKey, ApplicableSequenceMaster)>>,
}

impl<'a> ProfileValueIndex<'a> {
    pub(crate) fn new(resolved: &'a ResolvedAttributes) -> Self {
        Self {
            values: index_values(&resolved.values),
            sequence_masters: index_sequence_masters(&resolved.sequence_masters),
        }
    }

    pub(crate) fn values(&self, fixture_id: FixtureId) -> HashMap<AttributeKey, AttributeValue> {
        let Some(values) = self.values.get(&fixture_id) else {
            return HashMap::new();
        };
        values
            .iter()
            .map(|(attribute, value)| ((*attribute).clone(), (*value).clone()))
            .collect()
    }

    pub(crate) fn sequence_masters(
        &self,
        fixture_id: FixtureId,
    ) -> HashMap<AttributeKey, ApplicableSequenceMaster> {
        let Some(masters) = self.sequence_masters.get(&fixture_id) else {
            return HashMap::new();
        };
        masters
            .iter()
            .map(|(attribute, master)| ((*attribute).clone(), *master))
            .collect()
    }
}

fn index_values(
    values: &HashMap<(FixtureId, AttributeKey), AttributeValue>,
) -> HashMap<FixtureId, Vec<(&AttributeKey, &AttributeValue)>> {
    let mut indexed = HashMap::<FixtureId, Vec<_>>::new();
    for ((fixture_id, attribute), value) in values {
        indexed
            .entry(*fixture_id)
            .or_default()
            .push((attribute, value));
    }
    indexed
}

fn index_sequence_masters(
    masters: &HashMap<(FixtureId, AttributeKey), ApplicableSequenceMaster>,
) -> HashMap<FixtureId, Vec<(&AttributeKey, ApplicableSequenceMaster)>> {
    let mut indexed = HashMap::<FixtureId, Vec<_>>::new();
    for ((fixture_id, attribute), master) in masters {
        indexed
            .entry(*fixture_id)
            .or_default()
            .push((attribute, *master));
    }
    indexed
}
