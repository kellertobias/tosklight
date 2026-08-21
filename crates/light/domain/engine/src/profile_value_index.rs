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

    /// Every value one fixture owns, borrowed.
    fn borrowed_values(
        &self,
        fixture_id: FixtureId,
    ) -> Box<dyn Iterator<Item = (&'a AttributeKey, &'a AttributeValue)> + '_> {
        match self {
            Self::Dense(frame) => Box::new(
                frame
                    .slots()
                    .fixture_slots(fixture_id)
                    .iter()
                    .filter_map(move |slot| {
                        Some((frame.slots().attribute_key(*slot), frame.value(*slot)?))
                    }),
            ),
            Self::Scanned { values, .. } => match values.get(&fixture_id) {
                Some(values) => Box::new(values.iter().copied()),
                None => Box::new(std::iter::empty()),
            },
        }
    }

    fn borrowed_sequence_masters(
        &self,
        fixture_id: FixtureId,
    ) -> Box<dyn Iterator<Item = (&'a AttributeKey, ApplicableSequenceMaster)> + '_> {
        match self {
            Self::Dense(frame) => Box::new(
                frame
                    .slots()
                    .fixture_slots(fixture_id)
                    .iter()
                    .filter_map(move |slot| {
                        Some((
                            frame.slots().attribute_key(*slot),
                            frame.sequence_master(*slot)?,
                        ))
                    }),
            ),
            Self::Scanned {
                sequence_masters, ..
            } => match sequence_masters.get(&fixture_id) {
                Some(masters) => Box::new(masters.iter().copied()),
                None => Box::new(std::iter::empty()),
            },
        }
    }

    pub(crate) fn values(&self, fixture_id: FixtureId) -> crate::HeadValues {
        self.borrowed_values(fixture_id)
            .map(|(attribute, value)| (attribute.clone(), value.clone()))
            .collect()
    }

    pub(crate) fn value(
        &self,
        fixture_id: FixtureId,
        attribute: &AttributeKey,
    ) -> Option<&'a AttributeValue> {
        if let Self::Dense(frame) = self {
            let slot = frame.slots().slot(fixture_id, attribute)?;
            return frame.value(slot);
        }
        self.borrowed_values(fixture_id)
            .find_map(|(candidate, value)| (candidate == attribute).then_some(value))
    }

    pub(crate) fn value_named(
        &self,
        fixture_id: FixtureId,
        attribute: &str,
    ) -> Option<&'a AttributeValue> {
        self.borrowed_values(fixture_id)
            .find_map(|(candidate, value)| (candidate.0 == attribute).then_some(value))
    }

    pub(crate) fn sequence_masters(&self, fixture_id: FixtureId) -> crate::HeadSequenceMasters {
        self.borrowed_sequence_masters(fixture_id)
            .map(|(attribute, master)| (attribute.clone(), master))
            .collect()
    }

    pub(crate) fn sequence_master(
        &self,
        fixture_id: FixtureId,
        attribute: &AttributeKey,
    ) -> Option<ApplicableSequenceMaster> {
        if let Self::Dense(frame) = self {
            let slot = frame.slots().slot(fixture_id, attribute)?;
            return frame.sequence_master(slot);
        }
        self.borrowed_sequence_masters(fixture_id)
            .find_map(|(candidate, master)| (candidate == attribute).then_some(master))
    }

    pub(crate) fn sequence_master_named(
        &self,
        fixture_id: FixtureId,
        attribute: &str,
    ) -> Option<ApplicableSequenceMaster> {
        self.borrowed_sequence_masters(fixture_id)
            .find_map(|(candidate, master)| (candidate.0 == attribute).then_some(master))
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
