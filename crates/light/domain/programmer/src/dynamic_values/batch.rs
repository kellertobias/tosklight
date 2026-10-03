//! Indexed application of a large Dynamic/FAT gesture.
//!
//! Applying one mutation removes every stored row it replaces and appends the new row. Done
//! row by row, a gesture over thousands of targets compares every mutation with every stored
//! row while it holds the Programmer lock. Every row a mutation removes lies in exactly one or
//! two buckets of that mutation, and every member of such a bucket is removed, so each stored
//! row is visited a bounded number of times.

use super::DynamicProgrammerValueMutation;
use light_core::{AttributeKey, FixtureId};
use light_dynamics::{DynamicAddressValue, DynamicSemanticValue, DynamicTrackKey};
use std::collections::HashMap;
use uuid::Uuid;

/// Below this many mutation/row pairs (rows stored and appended) the row-by-row path is cheaper.
pub(super) const INDEXED_PAIRS: usize = 4_096;

/// Rows in storage order; a removed row leaves `None`, so the final order is the order the
/// row-by-row path produces. Buckets hold slot numbers and may hold removed slots.
#[derive(Default)]
pub(super) struct DynamicValueIndex {
    slots: Vec<Option<DynamicAddressValue>>,
    /// Same fixture, owner and track key.
    exact: HashMap<(FixtureId, AttributeKey, DynamicTrackKey), Vec<usize>>,
    /// Same fixture and owner, no instance (what a whole-owner Release replaces).
    unlinked: HashMap<(FixtureId, AttributeKey), Vec<usize>>,
    /// Same fixture, owner and instance (what an exact Release removes).
    owner_instance: HashMap<(FixtureId, AttributeKey, Option<Uuid>), Vec<usize>>,
    /// Every row of an instance (what an instance-wide Off replaces).
    instance: HashMap<Uuid, Vec<usize>>,
    /// The instance-wide Off rows of an instance (what one lane replaces).
    instance_offs: HashMap<Uuid, Vec<usize>>,
}

/// Which buckets hold the rows a mutation removes.
enum Removal {
    Buckets {
        exact: Option<(FixtureId, AttributeKey, DynamicTrackKey)>,
        unlinked: Option<(FixtureId, AttributeKey)>,
        instance: Option<Uuid>,
        instance_offs: Option<Uuid>,
    },
    OwnerInstance((FixtureId, AttributeKey, Option<Uuid>)),
}

impl DynamicValueIndex {
    pub(super) fn new(values: &[DynamicAddressValue]) -> Self {
        let mut index = Self::default();
        for value in values {
            index.push(value.clone());
        }
        index
    }

    pub(super) fn push(&mut self, value: DynamicAddressValue) {
        let slot = self.slots.len();
        let track = value.value.track_key();
        let owner = (value.fixture_id, value.attribute.clone());
        self.exact
            .entry((owner.0, owner.1.clone(), track))
            .or_default()
            .push(slot);
        self.owner_instance
            .entry((owner.0, owner.1.clone(), track.instance_link))
            .or_default()
            .push(slot);
        match track.instance_link {
            None => self.unlinked.entry(owner).or_default().push(slot),
            Some(link) => {
                self.instance.entry(link).or_default().push(slot);
                if track.lane_id.is_none() {
                    self.instance_offs.entry(link).or_default().push(slot);
                }
            }
        }
        self.slots.push(Some(value));
    }

    /// Whether applying `mutation` to the current rows would change them (the row-by-row
    /// `mutation_changes`).
    pub(super) fn changes(&self, mutation: &DynamicProgrammerValueMutation) -> bool {
        let mut replaced = self.removed_slots(mutation);
        match mutation {
            DynamicProgrammerValueMutation::Set { value, .. } => {
                replaced.sort_unstable();
                replaced.dedup();
                replaced.len() != 1 || self.slots[replaced[0]].as_ref().unwrap().value != *value
            }
            DynamicProgrammerValueMutation::Release { .. } => !replaced.is_empty(),
        }
    }

    /// Removes every row `mutation` replaces (the row-by-row `retain`).
    pub(super) fn remove(&mut self, mutation: &DynamicProgrammerValueMutation) {
        for slot in self.removed_slots(mutation) {
            self.slots[slot] = None;
        }
        // Every member of a used bucket is gone; drop the lists so they are not walked again.
        match removal(mutation) {
            Removal::Buckets {
                exact,
                unlinked,
                instance,
                instance_offs,
            } => {
                exact.map(|key| self.exact.remove(&key));
                unlinked.map(|key| self.unlinked.remove(&key));
                instance.map(|key| self.instance.remove(&key));
                instance_offs.map(|key| self.instance_offs.remove(&key));
            }
            Removal::OwnerInstance(key) => {
                self.owner_instance.remove(&key);
            }
        }
    }

    pub(super) fn into_values(self) -> Vec<DynamicAddressValue> {
        self.slots.into_iter().flatten().collect()
    }

    /// Live slots `mutation` removes; a slot can appear twice.
    fn removed_slots(&self, mutation: &DynamicProgrammerValueMutation) -> Vec<usize> {
        let mut removed = Vec::new();
        let mut live = |slots: Option<&Vec<usize>>| {
            removed.extend(
                slots
                    .into_iter()
                    .flatten()
                    .copied()
                    .filter(|slot| self.slots[*slot].is_some()),
            );
        };
        match removal(mutation) {
            Removal::Buckets {
                exact,
                unlinked,
                instance,
                instance_offs,
            } => {
                live(exact.and_then(|key| self.exact.get(&key)));
                live(unlinked.and_then(|key| self.unlinked.get(&key)));
                live(instance.and_then(|key| self.instance.get(&key)));
                live(instance_offs.and_then(|key| self.instance_offs.get(&key)));
            }
            Removal::OwnerInstance(key) => live(self.owner_instance.get(&key)),
        }
        removed
    }
}

/// `DynamicSemanticValue::replaces_address` and `same_track`, expressed as buckets.
fn removal(mutation: &DynamicProgrammerValueMutation) -> Removal {
    match mutation {
        DynamicProgrammerValueMutation::Set {
            fixture_id,
            attribute,
            value,
        } => {
            if matches!(value, DynamicSemanticValue::Release) {
                return Removal::Buckets {
                    exact: None,
                    unlinked: Some((*fixture_id, attribute.clone())),
                    instance: None,
                    instance_offs: None,
                };
            }
            let track = value.track_key();
            let (instance, instance_offs) = match (track.instance_link, track.lane_id) {
                (Some(link), None) => (Some(link), None),
                (Some(link), Some(_)) => (None, Some(link)),
                (None, _) => (None, None),
            };
            Removal::Buckets {
                // An instance-wide Off's exact matches are rows of its instance.
                exact: instance
                    .is_none()
                    .then(|| (*fixture_id, attribute.clone(), track)),
                unlinked: None,
                instance,
                instance_offs,
            }
        }
        DynamicProgrammerValueMutation::Release {
            fixture_id,
            attribute,
            instance_link,
        } => Removal::OwnerInstance((*fixture_id, attribute.clone(), *instance_link)),
    }
}

#[cfg(test)]
mod tests;
