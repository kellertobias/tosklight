use crate::contribution::ApplicableSequenceMaster;
use light_core::{AttributeKey, AttributeValue, FixtureId};
use rustc_hash::FxHashMap;
use std::collections::HashMap;

/// Per-fixture view of one render's resolved values.
///
/// A frame resolved into slots is read straight through its numbering: the patch already grouped
/// each fixture's slots when it compiled, so there is nothing to index. A projection assembled
/// from maps someone else handed over — visualization overrides, notably — is scanned into
/// per-fixture lists once instead of once per head.
pub(crate) enum ProfileValueIndex<'a> {
    Dense(&'a crate::contribution::ResolvedFrame),
    Scanned {
        values: HashMap<FixtureId, Vec<(&'a AttributeKey, &'a AttributeValue)>>,
        sequence_masters: HashMap<FixtureId, Vec<(&'a AttributeKey, ApplicableSequenceMaster)>>,
    },
}

impl<'a> ProfileValueIndex<'a> {
    pub(crate) fn new(
        values: &'a crate::FrameValues,
        sequence_masters: &'a FxHashMap<(FixtureId, AttributeKey), ApplicableSequenceMaster>,
    ) -> Self {
        match values.frame() {
            Some(frame) => Self::Dense(frame),
            None => Self::Scanned {
                values: index_values(values.values()),
                sequence_masters: index_sequence_masters(sequence_masters),
            },
        }
    }

    pub(crate) fn values(&self, fixture_id: FixtureId) -> crate::HeadValues {
        let mut values = crate::HeadValues::default();
        match self {
            Self::Dense(frame) => {
                for slot in frame.slots().fixture_slots(fixture_id) {
                    if let Some(value) = frame.value(*slot) {
                        values.insert(frame.slots().attribute_key(*slot).clone(), value.clone());
                    }
                }
                for (attribute, winner) in frame.overflow(fixture_id) {
                    values.insert(attribute.clone(), winner.value.clone());
                }
            }
            Self::Scanned {
                values: scanned, ..
            } => {
                for (attribute, value) in scanned.get(&fixture_id).into_iter().flatten() {
                    values.insert((*attribute).clone(), (*value).clone());
                }
            }
        }
        values
    }

    pub(crate) fn value(
        &self,
        fixture_id: FixtureId,
        attribute: &AttributeKey,
    ) -> Option<&'a AttributeValue> {
        match self {
            Self::Dense(frame) => match frame.slots().slot(fixture_id, attribute) {
                Some(slot) => frame.value(slot),
                None => frame
                    .overflow(fixture_id)
                    .iter()
                    .find(|(candidate, _)| candidate == attribute)
                    .map(|(_, winner)| &winner.value),
            },
            Self::Scanned { values, .. } => values
                .get(&fixture_id)?
                .iter()
                .find_map(|(candidate, value)| (*candidate == attribute).then_some(*value)),
        }
    }

    pub(crate) fn value_named(
        &self,
        fixture_id: FixtureId,
        attribute: &str,
    ) -> Option<&'a AttributeValue> {
        match self {
            Self::Dense(frame) => {
                for slot in frame.slots().fixture_slots(fixture_id) {
                    if frame.slots().attribute_key(*slot).0 == attribute {
                        return frame.value(*slot);
                    }
                }
                frame
                    .overflow(fixture_id)
                    .iter()
                    .find(|(candidate, _)| candidate.0 == attribute)
                    .map(|(_, winner)| &winner.value)
            }
            Self::Scanned { values, .. } => values
                .get(&fixture_id)?
                .iter()
                .find_map(|(candidate, value)| (candidate.0 == attribute).then_some(*value)),
        }
    }

    pub(crate) fn sequence_masters(&self, fixture_id: FixtureId) -> crate::HeadSequenceMasters {
        let mut masters = crate::HeadSequenceMasters::default();
        match self {
            Self::Dense(frame) => {
                for slot in frame.slots().fixture_slots(fixture_id) {
                    if let Some(master) = frame.sequence_master(*slot) {
                        masters.insert(frame.slots().attribute_key(*slot).clone(), master);
                    }
                }
                for (attribute, winner) in frame.overflow(fixture_id) {
                    if let Some(master) = winner.sequence_master {
                        masters.insert(attribute.clone(), master);
                    }
                }
            }
            Self::Scanned {
                sequence_masters, ..
            } => {
                for (attribute, master) in sequence_masters.get(&fixture_id).into_iter().flatten() {
                    masters.insert((*attribute).clone(), *master);
                }
            }
        }
        masters
    }

    pub(crate) fn sequence_master(
        &self,
        fixture_id: FixtureId,
        attribute: &AttributeKey,
    ) -> Option<ApplicableSequenceMaster> {
        match self {
            Self::Dense(frame) => match frame.slots().slot(fixture_id, attribute) {
                Some(slot) => frame.sequence_master(slot),
                None => frame
                    .overflow(fixture_id)
                    .iter()
                    .find(|(candidate, _)| candidate == attribute)
                    .and_then(|(_, winner)| winner.sequence_master),
            },
            Self::Scanned {
                sequence_masters, ..
            } => sequence_masters
                .get(&fixture_id)?
                .iter()
                .find_map(|(candidate, master)| (*candidate == attribute).then_some(*master)),
        }
    }

    pub(crate) fn sequence_master_named(
        &self,
        fixture_id: FixtureId,
        attribute: &str,
    ) -> Option<ApplicableSequenceMaster> {
        match self {
            Self::Dense(frame) => {
                for slot in frame.slots().fixture_slots(fixture_id) {
                    if frame.slots().attribute_key(*slot).0 == attribute {
                        return frame.sequence_master(*slot);
                    }
                }
                frame
                    .overflow(fixture_id)
                    .iter()
                    .find(|(candidate, _)| candidate.0 == attribute)
                    .and_then(|(_, winner)| winner.sequence_master)
            }
            Self::Scanned {
                sequence_masters, ..
            } => sequence_masters
                .get(&fixture_id)?
                .iter()
                .find_map(|(candidate, master)| (candidate.0 == attribute).then_some(*master)),
        }
    }
}

fn index_values(
    values: &crate::ResolvedValues,
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
    masters: &FxHashMap<(FixtureId, AttributeKey), ApplicableSequenceMaster>,
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
