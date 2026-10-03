//! Indexed fold of authored Dynamic edits.
//!
//! A row is dropped when any conflicting row is later than it, or precedes it in storage
//! order without being earlier (see [`super::merge_dynamic_address_values`]). Every row that
//! can conflict with a row lies in one of at most three buckets of that row, and within one
//! bucket the "is dropped by some member" question reduces to a few maxima, so the fold is
//! O(n log n) instead of comparing every pair.

use crate::{DynamicAddressValue, DynamicSemanticValue, DynamicTrackKey};
use light_core::{AttributeKey, FixtureId};
use rustc_hash::FxHashMap;
use uuid::Uuid;

/// Below this the pairwise scan is cheaper than building the buckets.
const PAIRWISE_LIMIT: usize = 24;

pub(super) fn fold(rows: Vec<&DynamicAddressValue>) -> Vec<&DynamicAddressValue> {
    if rows.len() <= PAIRWISE_LIMIT {
        return pairwise(rows);
    }
    let index = BucketIndex::build(&rows);
    rows.iter()
        .enumerate()
        .filter(|(position, row)| !index.dropped(*position, row))
        .map(|(_, row)| *row)
        .collect()
}

/// The reference relation, comparing every pair.
pub(super) fn pairwise(rows: Vec<&DynamicAddressValue>) -> Vec<&DynamicAddressValue> {
    rows.iter()
        .enumerate()
        .filter(|(index, value)| {
            !rows.iter().enumerate().any(|(other_index, other)| {
                other_index != *index
                    && super::dynamic_conflicts(value, other)
                    && (super::dynamic_address_edit_is_later(other, value)
                        || (other_index < *index
                            && !super::dynamic_address_edit_is_later(value, other)))
            })
        })
        .map(|(_, value)| *value)
        .collect()
}

/// Rows that conflict with a row `r` are exactly the union of:
/// - `exact`: the same fixture, owner and track key;
/// - `unlinked` (when `r` is a whole-owner Release): the same fixture and owner, no instance;
/// - `releases` (when `r` has no instance): Release rows of the same fixture and owner;
/// - `instance` (when `r` is an instance-wide Off): every row of that instance;
/// - `instance_offs` (when `r` is a lane of an instance): the Off rows of that instance.
#[derive(Default)]
struct BucketIndex {
    buckets: Vec<Bucket>,
    exact: FxHashMap<(FixtureId, AttributeKey, DynamicTrackKey), usize>,
    unlinked: FxHashMap<(FixtureId, AttributeKey), usize>,
    releases: FxHashMap<(FixtureId, AttributeKey), usize>,
    instance: FxHashMap<Uuid, usize>,
    instance_offs: FxHashMap<Uuid, usize>,
}

impl BucketIndex {
    fn build(rows: &[&DynamicAddressValue]) -> Self {
        let mut index = Self::default();
        for (position, row) in rows.iter().enumerate() {
            let track = row.value.track_key();
            let address = (row.fixture_id, row.attribute.clone());
            let exact = (row.fixture_id, row.attribute.clone(), track);
            Self::push(&mut index.buckets, &mut index.exact, exact, position, row);
            match track.instance_link {
                None => {
                    Self::push(
                        &mut index.buckets,
                        &mut index.unlinked,
                        address.clone(),
                        position,
                        row,
                    );
                    if is_release(row) {
                        Self::push(
                            &mut index.buckets,
                            &mut index.releases,
                            address,
                            position,
                            row,
                        );
                    }
                }
                Some(link) => {
                    Self::push(&mut index.buckets, &mut index.instance, link, position, row);
                    if track.lane_id.is_none() {
                        Self::push(
                            &mut index.buckets,
                            &mut index.instance_offs,
                            link,
                            position,
                            row,
                        );
                    }
                }
            }
        }
        for bucket in &mut index.buckets {
            bucket.finish();
        }
        index
    }

    fn push<K: std::hash::Hash + Eq>(
        buckets: &mut Vec<Bucket>,
        map: &mut FxHashMap<K, usize>,
        key: K,
        position: usize,
        row: &DynamicAddressValue,
    ) {
        let bucket = *map.entry(key).or_insert_with(|| {
            buckets.push(Bucket::default());
            buckets.len() - 1
        });
        buckets[bucket].add(position, row);
    }

    fn dropped(&self, position: usize, row: &DynamicAddressValue) -> bool {
        let track = row.value.track_key();
        let address = (row.fixture_id, row.attribute.clone());
        let probe = |bucket: Option<&usize>| {
            bucket.is_some_and(|bucket| self.buckets[*bucket].drops(position, row))
        };
        if probe(
            self.exact
                .get(&(row.fixture_id, row.attribute.clone(), track)),
        ) {
            return true;
        }
        match track.instance_link {
            None => {
                (is_release(row) && probe(self.unlinked.get(&address)))
                    || probe(self.releases.get(&address))
            }
            Some(link) if track.lane_id.is_none() => probe(self.instance.get(&link)),
            Some(link) => probe(self.instance_offs.get(&link)),
        }
    }
}

fn is_release(row: &DynamicAddressValue) -> bool {
    matches!(row.value, DynamicSemanticValue::Release)
}

/// The edit order compares `programmer_order` when both rows have one, and
/// `(changed_at_millis, programmer_order)` otherwise. Splitting members by whether they have
/// an order turns "some member drops this row" into comparisons against maxima.
#[derive(Default)]
struct Bucket {
    /// Storage positions, ascending (rows are added in storage order).
    positions: Vec<usize>,
    /// Running maximum, over members `0..=k`, of the order of ordered members.
    ordered_prefix: Vec<Option<u64>>,
    /// Running maximum, over members `0..=k`, of the change time of unordered members.
    unordered_prefix: Vec<Option<u64>>,
    ordered_order_max: Option<u64>,
    ordered_changed_max: Option<u64>,
    unordered_changed_max: Option<u64>,
}

impl Bucket {
    fn add(&mut self, position: usize, row: &DynamicAddressValue) {
        debug_assert!(self.positions.last().is_none_or(|last| *last < position));
        let previous = |prefix: &[Option<u64>]| prefix.last().copied().flatten();
        let (ordered, unordered) = if row.programmer_order > 0 {
            self.ordered_order_max = self.ordered_order_max.max(Some(row.programmer_order));
            self.ordered_changed_max = self.ordered_changed_max.max(Some(row.changed_at_millis));
            (
                previous(&self.ordered_prefix).max(Some(row.programmer_order)),
                previous(&self.unordered_prefix),
            )
        } else {
            self.unordered_changed_max =
                self.unordered_changed_max.max(Some(row.changed_at_millis));
            (
                previous(&self.ordered_prefix),
                previous(&self.unordered_prefix).max(Some(row.changed_at_millis)),
            )
        };
        self.positions.push(position);
        self.ordered_prefix.push(ordered);
        self.unordered_prefix.push(unordered);
    }

    fn finish(&mut self) {
        self.positions.shrink_to_fit();
    }

    /// Whether a member conflicting with `row` is later than it, or precedes it without being
    /// earlier. All comparisons are strict, so `row` itself never counts.
    fn drops(&self, position: usize, row: &DynamicAddressValue) -> bool {
        let before = self.positions.partition_point(|member| *member < position);
        let prefix = |values: &[Option<u64>]| before.checked_sub(1).and_then(|last| values[last]);
        let changed = Some(row.changed_at_millis);
        if row.programmer_order > 0 {
            let order = Some(row.programmer_order);
            // Ordered member: later if its order is greater; a preceding member with an order
            // that is not smaller is not earlier. Unordered member: later, and not earlier,
            // only with a greater change time.
            self.ordered_order_max > order
                || prefix(&self.ordered_prefix) >= order
                || self.unordered_changed_max > changed
        } else {
            // Unordered member: later with a greater change time; a preceding one with an equal
            // change time is not earlier. Ordered member: later with an equal or greater change
            // time, since its order breaks the tie.
            self.unordered_changed_max > changed
                || prefix(&self.unordered_prefix) >= changed
                || self.ordered_changed_max >= changed
        }
    }
}

#[cfg(test)]
mod tests;
